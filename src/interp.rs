//! Waffle IR interpreter.

use crate::entity::{EntityRef, PerEntity};
use crate::ir::*;
use crate::ops::Operator;
use smallvec::{smallvec, SmallVec};

use std::collections::HashMap;

/// How large do we allow a Wasm memory to be when interpreting? Limit
/// the size somewhat (apply an implementation limit) so we do not
/// have unreasonably large state.
const MAX_PAGES: usize = 2048; // 2048 * 64KiB = 128MiB

/// Context for the IR interpreter. Corresponds roughly to Wasm module
/// state.
pub struct InterpContext {
    /// Contents of memories.
    pub memories: PerEntity<Memory, InterpMemory>,
    /// Contents of tables.
    pub tables: PerEntity<Table, InterpTable>,
    /// Values of globals.
    pub globals: PerEntity<Global, ConstVal>,
    /// Fuel remaining: allows deterministic stopping of execution.
    pub fuel: u64,
}

/// The state of one interpreter memory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InterpMemory {
    pub data: Vec<u8>,
    pub max_pages: usize,
}

/// The state of one interpreter table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InterpTable {
    pub elements: Vec<Func>,
}

/// One stack frame in the interpreted execution context.
#[derive(Debug, Clone, Default)]
pub struct InterpStackFrame {
    func: Func,
    cur_block: Block,
    values: HashMap<Value, SmallVec<[ConstVal; 2]>>,
}

/// The result of an interpreter session.
#[derive(Clone, Debug)]
pub enum InterpResult {
    /// The function returned with the given value(s).
    Ok(MultiVal),
    /// The module trapped.
    Trap(Func, Block, u32),
    /// The module ran out of fuel.
    OutOfFuel,
}

/// A constant concrete value during interpretation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ConstVal {
    I32(u32),
    I64(u64),
    F32(u32),
    F64(u64),
    #[default]
    None,
}

/// Representation of multiple result values.
type MultiVal = SmallVec<[ConstVal; 2]>;

impl InterpResult {
    /// Extract the return value(s), if normal return, otherwise
    /// produce an error.
    pub fn ok(self) -> anyhow::Result<MultiVal> {
        match self {
            InterpResult::Ok(vals) => Ok(vals),
            other => anyhow::bail!("Bad InterpResult: {:?}", other),
        }
    }
}

impl InterpContext {
    /// Construct a new interpreter context for the given module.
    pub fn new(module: &Module<'_>) -> anyhow::Result<Self> {
        let mut memories = PerEntity::default();
        for (memory, data) in module.memories.entries() {
            let mut interp_mem = InterpMemory {
                data: vec![0; data.initial_pages * WASM_PAGE],
                max_pages: data.maximum_pages.unwrap_or(MAX_PAGES),
            };
            for segment in &data.segments {
                let end = match segment.offset.checked_add(segment.data.len()) {
                    Some(end) => end,
                    None => anyhow::bail!("Data segment offset + length overflows"),
                };
                if end > interp_mem.data.len() {
                    anyhow::bail!("Data segment out of bounds");
                }
                interp_mem.data[segment.offset..end].copy_from_slice(&segment.data[..]);
            }
            memories[memory] = interp_mem;
        }

        let mut tables = PerEntity::default();
        for (table, data) in module.tables.entries() {
            let interp_table = InterpTable {
                elements: data.func_elements.clone().unwrap_or(vec![]),
            };
            tables[table] = interp_table;
        }

        let mut globals = PerEntity::default();
        for (global, data) in module.globals.entries() {
            globals[global] = match data.ty {
                Type::I32 => ConstVal::I32(data.value.unwrap_or(0) as u32),
                Type::I64 => ConstVal::I64(data.value.unwrap_or(0)),
                Type::F32 => ConstVal::F32(data.value.unwrap_or(0) as u32),
                Type::F64 => ConstVal::F64(data.value.unwrap_or(0)),
                _ => unimplemented!(),
            };
        }

        Ok(InterpContext {
            memories,
            tables,
            globals,
            fuel: u64::MAX,
        })
    }

    /// Call the given function with the given args, running the
    /// interpreter until fuel is exhausted or the function returns.
    pub fn call(&mut self, module: &Module<'_>, func: Func, args: &[ConstVal]) -> InterpResult {
        let body = match &module.funcs[func] {
            FuncDecl::Lazy(..) => panic!("Un-expanded function"),
            FuncDecl::Compiled(..) => panic!("Already-compiled function"),
            FuncDecl::Import(..) => {
                let import = &module.imports[func.index()];
                assert_eq!(import.kind, ImportKind::Func(func));
                return self.call_import(&import.name[..], args);
            }
            FuncDecl::Body(_, _, body) => body,
            FuncDecl::None => panic!("FuncDecl::None in call()"),
        };

        log::trace!(
            "Interp: entering func {}:\n{}\n",
            func,
            body.display_verbose("| ", Some(module))
        );
        log::trace!("args: {:?}", args);

        let mut frame = InterpStackFrame {
            func,
            cur_block: body.entry,
            values: HashMap::new(),
        };

        for (&arg, &(_, blockparam)) in args.iter().zip(body.blocks[body.entry].params.iter()) {
            log::trace!("Entry block param {} gets arg value {:?}", blockparam, arg);
            frame.values.insert(blockparam, smallvec![arg]);
        }

        loop {
            self.fuel -= 1;
            if self.fuel == 0 {
                return InterpResult::OutOfFuel;
            }

            log::trace!("Interpreting block {}", frame.cur_block);
            for (inst_idx, &inst) in body.blocks[frame.cur_block].insts.iter().enumerate() {
                log::trace!("Evaluating inst {}", inst);
                let result = match &body.values[inst] {
                    &ValueDef::Alias(_) => smallvec![],
                    &ValueDef::PickOutput(val, idx, _) => {
                        let val = body.resolve_alias(val);
                        smallvec![frame.values.get(&val).unwrap()[idx as usize]]
                    }
                    &ValueDef::Operator(Operator::Call { function_index }, args, _) => {
                        let args = body.arg_pool[args]
                            .iter()
                            .map(|&arg| {
                                let arg = body.resolve_alias(arg);
                                let multivalue = frame.values.get(&arg).unwrap();
                                assert_eq!(multivalue.len(), 1);
                                multivalue[0]
                            })
                            .collect::<Vec<_>>();
                        let result = self.call(module, function_index, &args[..]);
                        match result {
                            InterpResult::Ok(vals) => vals,
                            _ => return result,
                        }
                    }
                    &ValueDef::Operator(Operator::CallIndirect { table_index, .. }, args, _) => {
                        let args = body.arg_pool[args]
                            .iter()
                            .map(|&arg| {
                                let arg = body.resolve_alias(arg);
                                let multivalue = frame.values.get(&arg).unwrap();
                                assert_eq!(multivalue.len(), 1);
                                multivalue[0]
                            })
                            .collect::<Vec<_>>();
                        let idx = args.last().unwrap().as_u32().unwrap() as usize;
                        let func = self.tables[table_index].elements[idx];
                        let result = self.call(module, func, &args[..args.len() - 1]);
                        match result {
                            InterpResult::Ok(vals) => vals,
                            _ => return result,
                        }
                    }
                    &ValueDef::Operator(ref op, args, _) => {
                        let args = body.arg_pool[args]
                            .iter()
                            .map(|&arg| {
                                let arg = body.resolve_alias(arg);
                                let multivalue = frame
                                    .values
                                    .get(&arg)
                                    .ok_or_else(|| format!("Unset SSA value: {}", arg))
                                    .unwrap();
                                assert_eq!(multivalue.len(), 1);
                                multivalue[0]
                            })
                            .collect::<Vec<_>>();
                        let result = match const_eval(op, &args[..], Some(self)) {
                            Some(result) => result,
                            None => {
                                log::trace!("const_eval failed on {:?} args {:?}", op, args);
                                return InterpResult::Trap(
                                    frame.func,
                                    frame.cur_block,
                                    inst_idx as u32,
                                );
                            }
                        };
                        smallvec![result]
                    }
                    &ValueDef::None | &ValueDef::Placeholder(..) | &ValueDef::BlockParam(..) => {
                        unreachable!();
                    }
                };

                log::trace!("Inst {} gets result {:?}", inst, result);
                frame.values.insert(inst, result);
            }

            match &body.blocks[frame.cur_block].terminator {
                &Terminator::None => {
                    return InterpResult::Trap(frame.func, frame.cur_block, u32::MAX)
                }
                &Terminator::Unreachable => {
                    return InterpResult::Trap(frame.func, frame.cur_block, u32::MAX)
                }
                &Terminator::Br { ref target } => {
                    frame.apply_target(body, target);
                }
                &Terminator::CondBr {
                    cond,
                    ref if_true,
                    ref if_false,
                } => {
                    let cond = body.resolve_alias(cond);
                    let cond = frame.values.get(&cond).unwrap();
                    let cond = cond[0].as_u32().unwrap() != 0;
                    if cond {
                        frame.apply_target(body, if_true);
                    } else {
                        frame.apply_target(body, if_false);
                    }
                }
                &Terminator::Select {
                    value,
                    ref targets,
                    ref default,
                } => {
                    let value = body.resolve_alias(value);
                    let value = frame.values.get(&value).unwrap();
                    let value = value[0].as_u32().unwrap() as usize;
                    if value < targets.len() {
                        frame.apply_target(body, &targets[value]);
                    } else {
                        frame.apply_target(body, default);
                    }
                }
                &Terminator::Return { ref values } => {
                    let values = values
                        .iter()
                        .map(|&value| {
                            let value = body.resolve_alias(value);
                            frame.values.get(&value).unwrap()[0]
                        })
                        .collect();
                    log::trace!("returning from {}: {:?}", func, values);
                    return InterpResult::Ok(values);
                }
            }
        }
    }

    fn call_import(&mut self, name: &str, args: &[ConstVal]) -> InterpResult {
        panic!("Unknown import: {} with args: {:?}", name, args);
    }
}

impl InterpStackFrame {
    fn apply_target(&mut self, body: &FunctionBody, target: &BlockTarget) {
        // Collect blockparam args.
        let args = target
            .args
            .iter()
            .map(|&arg| {
                let arg = body.resolve_alias(arg);
                self.values.get(&arg).unwrap().clone()
            })
            .collect::<Vec<_>>();
        log::trace!("taking target {:?} with args {:?}", target, args);
        // Set blockparams.
        for (arg, &(_, param)) in args
            .into_iter()
            .zip(body.blocks[target.block].params.iter())
        {
            log::trace!("setting blockparam {} to {:?}", param, arg);
            self.values.insert(param, arg);
        }
        // Set current block.
        self.cur_block = target.block;
    }
}

impl ConstVal {
    pub fn as_u32(self) -> Option<u32> {
        match self {
            Self::I32(x) => Some(x),
            _ => None,
        }
    }

    pub fn meet(a: Option<ConstVal>, b: Option<ConstVal>) -> Option<ConstVal> {
        match (a, b) {
            (None, None) => None,
            (Some(a), None) | (None, Some(a)) => Some(a),
            (Some(a), Some(b)) if a == b => Some(a),
            _ => Some(ConstVal::None),
        }
    }
}

/// Constant-evaluate the given operator with the given arguments,
/// returning a constant result if possible to know.
pub fn const_eval(
    op: &Operator,
    vals: &[ConstVal],
    ctx: Option<&mut InterpContext>,
) -> Option<ConstVal> {
    match (op, vals) {
        (Operator::I32Const { value }, []) => Some(ConstVal::I32(*value)),
        (Operator::I64Const { value }, []) => Some(ConstVal::I64(*value)),
        (Operator::F32Const { value }, []) => Some(ConstVal::F32(*value)),
        (Operator::F64Const { value }, []) => Some(ConstVal::F64(*value)),
        (Operator::I32Eqz, [ConstVal::I32(a)]) => Some(ConstVal::I32(if *a == 0 { 1 } else { 0 })),
        (Operator::I32Eq, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(if a == b { 1 } else { 0 }))
        }
        (Operator::I32Ne, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(if a != b { 1 } else { 0 }))
        }
        (Operator::I32LtS, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(if (*a as i32) < (*b as i32) { 1 } else { 0 }))
        }
        (Operator::I32LtU, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(if a < b { 1 } else { 0 }))
        }
        (Operator::I32GtS, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(if (*a as i32) > (*b as i32) { 1 } else { 0 }))
        }
        (Operator::I32GtU, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(if a > b { 1 } else { 0 }))
        }
        (Operator::I32LeS, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(if (*a as i32) <= (*b as i32) {
                1
            } else {
                0
            }))
        }
        (Operator::I32LeU, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(if a <= b { 1 } else { 0 }))
        }
        (Operator::I32GeS, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(if (*a as i32) >= (*b as i32) {
                1
            } else {
                0
            }))
        }
        (Operator::I32GeU, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(if a >= b { 1 } else { 0 }))
        }
        (Operator::I64Eqz, [ConstVal::I64(a)]) => Some(ConstVal::I32(if *a == 0 { 1 } else { 0 })),
        (Operator::I64Eq, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I32(if a == b { 1 } else { 0 }))
        }
        (Operator::I64Ne, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I32(if a != b { 1 } else { 0 }))
        }
        (Operator::I64LtS, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I32(if (*a as i64) < (*b as i64) { 1 } else { 0 }))
        }
        (Operator::I64LtU, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I32(if a < b { 1 } else { 0 }))
        }
        (Operator::I64GtS, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I32(if (*a as i64) > (*b as i64) { 1 } else { 0 }))
        }
        (Operator::I64GtU, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I32(if a > b { 1 } else { 0 }))
        }
        (Operator::I64LeS, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I32(if (*a as i64) <= (*b as i64) {
                1
            } else {
                0
            }))
        }
        (Operator::I64LeU, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I32(if a <= b { 1 } else { 0 }))
        }
        (Operator::I64GeS, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I32(if (*a as i64) >= (*b as i64) {
                1
            } else {
                0
            }))
        }
        (Operator::I64GeU, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I32(if a >= b { 1 } else { 0 }))
        }

        (Operator::F32Eq, [ConstVal::F32(a), ConstVal::F32(b)]) => {
            Some(ConstVal::I32(if f32::from_bits(*a) == f32::from_bits(*b) {
                1
            } else {
                0
            }))
        }
        (Operator::F32Ne, [ConstVal::F32(a), ConstVal::F32(b)]) => {
            Some(ConstVal::I32(if f32::from_bits(*a) != f32::from_bits(*b) {
                1
            } else {
                0
            }))
        }
        (Operator::F32Lt, [ConstVal::F32(a), ConstVal::F32(b)]) => {
            Some(ConstVal::I32(if f32::from_bits(*a) < f32::from_bits(*b) {
                1
            } else {
                0
            }))
        }
        (Operator::F32Gt, [ConstVal::F32(a), ConstVal::F32(b)]) => {
            Some(ConstVal::I32(if f32::from_bits(*a) > f32::from_bits(*b) {
                1
            } else {
                0
            }))
        }
        (Operator::F32Le, [ConstVal::F32(a), ConstVal::F32(b)]) => {
            Some(ConstVal::I32(if f32::from_bits(*a) <= f32::from_bits(*b) {
                1
            } else {
                0
            }))
        }
        (Operator::F32Ge, [ConstVal::F32(a), ConstVal::F32(b)]) => {
            Some(ConstVal::I32(if f32::from_bits(*a) >= f32::from_bits(*b) {
                1
            } else {
                0
            }))
        }

        (Operator::F64Eq, [ConstVal::F64(a), ConstVal::F64(b)]) => {
            Some(ConstVal::I32(if f64::from_bits(*a) == f64::from_bits(*b) {
                1
            } else {
                0
            }))
        }
        (Operator::F64Ne, [ConstVal::F64(a), ConstVal::F64(b)]) => {
            Some(ConstVal::I32(if f64::from_bits(*a) != f64::from_bits(*b) {
                1
            } else {
                0
            }))
        }
        (Operator::F64Lt, [ConstVal::F64(a), ConstVal::F64(b)]) => {
            Some(ConstVal::I32(if f64::from_bits(*a) < f64::from_bits(*b) {
                1
            } else {
                0
            }))
        }
        (Operator::F64Gt, [ConstVal::F64(a), ConstVal::F64(b)]) => {
            Some(ConstVal::I32(if f64::from_bits(*a) > f64::from_bits(*b) {
                1
            } else {
                0
            }))
        }
        (Operator::F64Le, [ConstVal::F64(a), ConstVal::F64(b)]) => {
            Some(ConstVal::I32(if f64::from_bits(*a) <= f64::from_bits(*b) {
                1
            } else {
                0
            }))
        }
        (Operator::F64Ge, [ConstVal::F64(a), ConstVal::F64(b)]) => {
            Some(ConstVal::I32(if f64::from_bits(*a) >= f64::from_bits(*b) {
                1
            } else {
                0
            }))
        }

        (Operator::I32Clz, [ConstVal::I32(x)]) => Some(ConstVal::I32(x.leading_zeros())),
        (Operator::I32Ctz, [ConstVal::I32(x)]) => Some(ConstVal::I32(x.trailing_zeros())),
        (Operator::I32Popcnt, [ConstVal::I32(x)]) => Some(ConstVal::I32(x.count_ones())),

        (Operator::I32Add, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(a.wrapping_add(*b)))
        }
        (Operator::I32Sub, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(a.wrapping_sub(*b)))
        }
        (Operator::I32Mul, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(a.wrapping_mul(*b)))
        }
        (Operator::I32DivU, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(a.checked_div(*b)?))
        }
        (Operator::I32DivS, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32((*a as i32).checked_div(*b as i32)? as u32))
        }
        (Operator::I32RemU, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(a.checked_rem(*b)?))
        }
        (Operator::I32RemS, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32((*a as i32).checked_rem(*b as i32)? as u32))
        }
        (Operator::I32And, [ConstVal::I32(a), ConstVal::I32(b)]) => Some(ConstVal::I32(a & b)),
        (Operator::I32Or, [ConstVal::I32(a), ConstVal::I32(b)]) => Some(ConstVal::I32(a | b)),
        (Operator::I32Xor, [ConstVal::I32(a), ConstVal::I32(b)]) => Some(ConstVal::I32(a ^ b)),
        (Operator::I32Shl, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(a.wrapping_shl(*b)))
        }
        (Operator::I32ShrS, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32((*a as i32).wrapping_shr(*b) as u32))
        }
        (Operator::I32ShrU, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(a.wrapping_shr(*b)))
        }
        (Operator::I32Rotl, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(a.rotate_left(*b & 0x1f)))
        }
        (Operator::I32Rotr, [ConstVal::I32(a), ConstVal::I32(b)]) => {
            Some(ConstVal::I32(a.rotate_right(*b & 0x1f)))
        }

        (Operator::I64Clz, [ConstVal::I64(x)]) => Some(ConstVal::I64(x.leading_zeros() as u64)),
        (Operator::I64Ctz, [ConstVal::I64(x)]) => Some(ConstVal::I64(x.trailing_zeros() as u64)),
        (Operator::I64Popcnt, [ConstVal::I64(x)]) => Some(ConstVal::I64(x.count_ones() as u64)),

        (Operator::I64Add, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I64(a.wrapping_add(*b)))
        }
        (Operator::I64Sub, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I64(a.wrapping_sub(*b)))
        }
        (Operator::I64Mul, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I64(a.wrapping_mul(*b)))
        }
        (Operator::I64DivU, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I64(a.checked_div(*b)?))
        }
        (Operator::I64DivS, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I64((*a as i64).checked_div(*b as i64)? as u64))
        }
        (Operator::I64RemU, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I64(a.checked_rem(*b)?))
        }
        (Operator::I64RemS, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I64((*a as i64).checked_rem(*b as i64)? as u64))
        }
        (Operator::I64And, [ConstVal::I64(a), ConstVal::I64(b)]) => Some(ConstVal::I64(a & b)),
        (Operator::I64Or, [ConstVal::I64(a), ConstVal::I64(b)]) => Some(ConstVal::I64(a | b)),
        (Operator::I64Xor, [ConstVal::I64(a), ConstVal::I64(b)]) => Some(ConstVal::I64(a ^ b)),
        (Operator::I64Shl, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I64(a.wrapping_shl(*b as u32)))
        }
        (Operator::I64ShrS, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I64((*a as i64).wrapping_shr(*b as u32) as u64))
        }
        (Operator::I64ShrU, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I64(a.wrapping_shr(*b as u32)))
        }
        (Operator::I64Rotl, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I64(a.rotate_left((*b as u32) & 0x3f)))
        }
        (Operator::I64Rotr, [ConstVal::I64(a), ConstVal::I64(b)]) => {
            Some(ConstVal::I64(a.rotate_right((*b as u32) & 0x3f)))
        }

        (Operator::F32Abs, [ConstVal::F32(a)]) => {
            Some(ConstVal::F32(f32::from_bits(*a).abs().to_bits()))
        }
        (Operator::F32Neg, [ConstVal::F32(a)]) => {
            Some(ConstVal::F32((-f32::from_bits(*a)).to_bits()))
        }
        (Operator::F32Ceil, [ConstVal::F32(a)]) => {
            Some(ConstVal::F32(f32::from_bits(*a).ceil().to_bits()))
        }
        (Operator::F32Floor, [ConstVal::F32(a)]) => {
            Some(ConstVal::F32(f32::from_bits(*a).floor().to_bits()))
        }
        (Operator::F32Trunc, [ConstVal::F32(a)]) => {
            Some(ConstVal::F32(f32::from_bits(*a).trunc().to_bits()))
        }
        (Operator::F32Nearest, [ConstVal::F32(a)]) => Some(ConstVal::F32(
            f32::from_bits(*a).round_ties_even().to_bits(),
        )),
        (Operator::F32Sqrt, [ConstVal::F32(a)]) => {
            Some(ConstVal::F32(f32::from_bits(*a).sqrt().to_bits()))
        }
        (Operator::F32Add, [ConstVal::F32(a), ConstVal::F32(b)]) => Some(ConstVal::F32(
            (f32::from_bits(*a) + f32::from_bits(*b)).to_bits(),
        )),
        (Operator::F32Sub, [ConstVal::F32(a), ConstVal::F32(b)]) => Some(ConstVal::F32(
            (f32::from_bits(*a) - f32::from_bits(*b)).to_bits(),
        )),
        (Operator::F32Mul, [ConstVal::F32(a), ConstVal::F32(b)]) => Some(ConstVal::F32(
            (f32::from_bits(*a) * f32::from_bits(*b)).to_bits(),
        )),
        (Operator::F32Div, [ConstVal::F32(a), ConstVal::F32(b)]) => Some(ConstVal::F32(
            (f32::from_bits(*a) / f32::from_bits(*b)).to_bits(),
        )),
        (Operator::F32Min, [ConstVal::F32(a), ConstVal::F32(b)]) => Some(ConstVal::F32(
            f32_min(f32::from_bits(*a), f32::from_bits(*b)).to_bits(),
        )),
        (Operator::F32Max, [ConstVal::F32(a), ConstVal::F32(b)]) => Some(ConstVal::F32(
            f32_max(f32::from_bits(*a), f32::from_bits(*b)).to_bits(),
        )),
        (Operator::F32Copysign, [ConstVal::F32(a), ConstVal::F32(b)]) => Some(ConstVal::F32(
            f32::copysign(f32::from_bits(*a), f32::from_bits(*b)).to_bits(),
        )),

        (Operator::F64Abs, [ConstVal::F64(a)]) => {
            Some(ConstVal::F64(f64::from_bits(*a).abs().to_bits()))
        }
        (Operator::F64Neg, [ConstVal::F64(a)]) => {
            Some(ConstVal::F64((-f64::from_bits(*a)).to_bits()))
        }
        (Operator::F64Ceil, [ConstVal::F64(a)]) => {
            Some(ConstVal::F64(f64::from_bits(*a).ceil().to_bits()))
        }
        (Operator::F64Floor, [ConstVal::F64(a)]) => {
            Some(ConstVal::F64(f64::from_bits(*a).floor().to_bits()))
        }
        (Operator::F64Trunc, [ConstVal::F64(a)]) => {
            Some(ConstVal::F64(f64::from_bits(*a).trunc().to_bits()))
        }
        (Operator::F64Nearest, [ConstVal::F64(a)]) => Some(ConstVal::F64(
            f64::from_bits(*a).round_ties_even().to_bits(),
        )),
        (Operator::F64Sqrt, [ConstVal::F64(a)]) => {
            Some(ConstVal::F64(f64::from_bits(*a).sqrt().to_bits()))
        }
        (Operator::F64Add, [ConstVal::F64(a), ConstVal::F64(b)]) => Some(ConstVal::F64(
            (f64::from_bits(*a) + f64::from_bits(*b)).to_bits(),
        )),
        (Operator::F64Sub, [ConstVal::F64(a), ConstVal::F64(b)]) => Some(ConstVal::F64(
            (f64::from_bits(*a) - f64::from_bits(*b)).to_bits(),
        )),
        (Operator::F64Mul, [ConstVal::F64(a), ConstVal::F64(b)]) => Some(ConstVal::F64(
            (f64::from_bits(*a) * f64::from_bits(*b)).to_bits(),
        )),
        (Operator::F64Div, [ConstVal::F64(a), ConstVal::F64(b)]) => Some(ConstVal::F64(
            (f64::from_bits(*a) / f64::from_bits(*b)).to_bits(),
        )),
        (Operator::F64Min, [ConstVal::F64(a), ConstVal::F64(b)]) => Some(ConstVal::F64(
            f64_min(f64::from_bits(*a), f64::from_bits(*b)).to_bits(),
        )),
        (Operator::F64Max, [ConstVal::F64(a), ConstVal::F64(b)]) => Some(ConstVal::F64(
            f64_max(f64::from_bits(*a), f64::from_bits(*b)).to_bits(),
        )),
        (Operator::F64Copysign, [ConstVal::F64(a), ConstVal::F64(b)]) => Some(ConstVal::F64(
            f64::copysign(f64::from_bits(*a), f64::from_bits(*b)).to_bits(),
        )),

        (Operator::I32WrapI64, [ConstVal::I64(a)]) => Some(ConstVal::I32(*a as u32)),

        (Operator::I32TruncF32S, [ConstVal::F32(a)]) => {
            let a = f32::from_bits(*a);
            if a >= (i32::MIN as f32) && a <= (i32::MAX as f32) {
                Some(ConstVal::I32(a as i32 as u32))
            } else {
                None
            }
        }
        (Operator::I32TruncF32U, [ConstVal::F32(a)]) => {
            let a = f32::from_bits(*a);
            if a >= 0.0 && a <= (u32::MAX as f32) {
                Some(ConstVal::I32(a as u32))
            } else {
                None
            }
        }
        (Operator::I32TruncF64S, [ConstVal::F64(a)]) => {
            let a = f64::from_bits(*a);
            if a >= (i32::MIN as f64) && a <= (i32::MAX as f64) {
                Some(ConstVal::I32(a as i32 as u32))
            } else {
                None
            }
        }
        (Operator::I32TruncF64U, [ConstVal::F64(a)]) => {
            let a = f64::from_bits(*a);
            if a >= 0.0 && a <= (u32::MAX as f64) {
                Some(ConstVal::I32(a as u32))
            } else {
                None
            }
        }

        (Operator::I64TruncF32S, [ConstVal::F32(a)]) => {
            let a = f32::from_bits(*a);
            if a >= (i64::MIN as f32) && a <= (i64::MAX as f32) {
                Some(ConstVal::I64(a as i64 as u64))
            } else {
                None
            }
        }
        (Operator::I64TruncF32U, [ConstVal::F32(a)]) => {
            let a = f32::from_bits(*a);
            if a >= 0.0 && a <= (u64::MAX as f32) {
                Some(ConstVal::I64(a as u64))
            } else {
                None
            }
        }
        (Operator::I64TruncF64S, [ConstVal::F64(a)]) => {
            let a = f64::from_bits(*a);
            if a >= (i64::MIN as f64) && a <= (i64::MAX as f64) {
                Some(ConstVal::I64(a as i64 as u64))
            } else {
                None
            }
        }
        (Operator::I64TruncF64U, [ConstVal::F64(a)]) => {
            let a = f64::from_bits(*a);
            if a >= 0.0 && a <= (u64::MAX as f64) {
                Some(ConstVal::I64(a as u64))
            } else {
                None
            }
        }

        (Operator::I32TruncSatF32S, [ConstVal::F32(a)]) => {
            let a = f32::from_bits(*a);
            Some(ConstVal::I32(if a.is_nan() {
                0
            } else {
                a.min(i32::MAX as f32).max(i32::MIN as f32) as i32 as u32
            }))
        }
        (Operator::I32TruncSatF32U, [ConstVal::F32(a)]) => {
            let a = f32::from_bits(*a);
            Some(ConstVal::I32(if a.is_nan() {
                0
            } else {
                a.min(u32::MAX as f32).max(0.0) as u32
            }))
        }
        (Operator::I32TruncSatF64S, [ConstVal::F64(a)]) => {
            let a = f64::from_bits(*a);
            Some(ConstVal::I32(if a.is_nan() {
                0
            } else {
                a.min(i32::MAX as f64).max(i32::MIN as f64) as i32 as u32
            }))
        }
        (Operator::I32TruncSatF64U, [ConstVal::F64(a)]) => {
            let a = f64::from_bits(*a);
            Some(ConstVal::I32(if a.is_nan() {
                0
            } else {
                a.min(u32::MAX as f64).max(0.0) as u32
            }))
        }

        (Operator::I64TruncSatF32S, [ConstVal::F32(a)]) => {
            let a = f32::from_bits(*a);
            Some(ConstVal::I64(if a.is_nan() {
                0
            } else {
                a.min(i64::MAX as f32).max(i64::MIN as f32) as i64 as u64
            }))
        }
        (Operator::I64TruncSatF32U, [ConstVal::F32(a)]) => {
            let a = f32::from_bits(*a);
            Some(ConstVal::I64(if a.is_nan() {
                0
            } else {
                a.min(u64::MAX as f32).max(0.0) as u64
            }))
        }
        (Operator::I64TruncSatF64S, [ConstVal::F64(a)]) => {
            let a = f64::from_bits(*a);
            Some(ConstVal::I64(if a.is_nan() {
                0
            } else {
                a.min(i64::MAX as f64).max(i64::MIN as f64) as i64 as u64
            }))
        }
        (Operator::I64TruncSatF64U, [ConstVal::F64(a)]) => {
            let a = f64::from_bits(*a);
            Some(ConstVal::I64(if a.is_nan() {
                0
            } else {
                a.min(u64::MAX as f64).max(0.0) as u64
            }))
        }

        (Operator::F32ConvertI32S, [ConstVal::I32(a)]) => {
            Some(ConstVal::F32((*a as i32 as f32).to_bits()))
        }
        (Operator::F32ConvertI32U, [ConstVal::I32(a)]) => {
            Some(ConstVal::F32((*a as f32).to_bits()))
        }
        (Operator::F32ConvertI64S, [ConstVal::I64(a)]) => {
            Some(ConstVal::F32((*a as i64 as f32).to_bits()))
        }
        (Operator::F32ConvertI64U, [ConstVal::I64(a)]) => {
            Some(ConstVal::F32((*a as f32).to_bits()))
        }

        (Operator::F64ConvertI32S, [ConstVal::I32(a)]) => {
            Some(ConstVal::F64((*a as i32 as f64).to_bits()))
        }
        (Operator::F64ConvertI32U, [ConstVal::I32(a)]) => {
            Some(ConstVal::F64((*a as f64).to_bits()))
        }
        (Operator::F64ConvertI64S, [ConstVal::I64(a)]) => {
            Some(ConstVal::F64((*a as i64 as f64).to_bits()))
        }
        (Operator::F64ConvertI64U, [ConstVal::I64(a)]) => {
            Some(ConstVal::F64((*a as f64).to_bits()))
        }

        (Operator::F32DemoteF64, [ConstVal::F64(a)]) => {
            Some(ConstVal::F32((f64::from_bits(*a) as f32).to_bits()))
        }
        (Operator::F64PromoteF32, [ConstVal::F32(a)]) => {
            Some(ConstVal::F64((f32::from_bits(*a) as f64).to_bits()))
        }

        (Operator::F32ReinterpretI32, [ConstVal::I32(a)]) => Some(ConstVal::F32(*a)),
        (Operator::F64ReinterpretI64, [ConstVal::I64(a)]) => Some(ConstVal::F64(*a)),
        (Operator::I32ReinterpretF32, [ConstVal::F32(a)]) => Some(ConstVal::I32(*a)),
        (Operator::I64ReinterpretF64, [ConstVal::F64(a)]) => Some(ConstVal::I64(*a)),

        (Operator::I32Extend8S, [ConstVal::I32(a)]) => Some(ConstVal::I32(*a as i8 as i32 as u32)),
        (Operator::I32Extend16S, [ConstVal::I32(a)]) => {
            Some(ConstVal::I32(*a as i16 as i32 as u32))
        }
        (Operator::I64Extend8S, [ConstVal::I64(a)]) => Some(ConstVal::I64(*a as i8 as i64 as u64)),
        (Operator::I64Extend16S, [ConstVal::I64(a)]) => {
            Some(ConstVal::I64(*a as i16 as i64 as u64))
        }
        (Operator::I64Extend32S, [ConstVal::I64(a)]) => {
            Some(ConstVal::I64(*a as i32 as i64 as u64))
        }
        (Operator::I64ExtendI32S, [ConstVal::I32(a)]) => {
            Some(ConstVal::I64(*a as i32 as i64 as u64))
        }
        (Operator::I64ExtendI32U, [ConstVal::I32(a)]) => Some(ConstVal::I64(*a as u64)),

        (Operator::Select, [x, y, ConstVal::I32(k)]) => Some(if *k != 0 { *x } else { *y }),
        (Operator::TypedSelect { .. }, [x, y, ConstVal::I32(k)]) => {
            Some(if *k != 0 { *x } else { *y })
        }

        (Operator::GlobalGet { global_index }, []) => {
            ctx.map(|global| global.globals[*global_index])
        }
        (Operator::GlobalSet { global_index }, [x]) => ctx.map(|global| {
            global.globals[*global_index] = *x;
            ConstVal::None
        }),

        (Operator::TableGet { .. }, _)
        | (Operator::TableSet { .. }, _)
        | (Operator::TableGrow { .. }, _) => None,

        (Operator::TableSize { table_index }, []) => {
            ctx.map(|global| ConstVal::I32(global.tables[*table_index].elements.len() as u32))
        }

        (Operator::MemorySize { mem }, []) => {
            ctx.map(|global| ConstVal::I32((global.memories[*mem].data.len() / WASM_PAGE) as u32))
        }

        (Operator::MemoryGrow { mem }, [ConstVal::I32(amount)]) => ctx.and_then(|global| {
            let cur_pages = global.memories[*mem].data.len() / WASM_PAGE;
            let new_pages = cur_pages + (*amount as usize);
            if new_pages > global.memories[*mem].max_pages || new_pages > MAX_PAGES {
                None
            } else {
                global.memories[*mem].data.resize(new_pages * WASM_PAGE, 0);
                Some(ConstVal::I32(cur_pages as u32))
            }
        }),

        (Operator::Nop, []) => Some(ConstVal::None),
        (Operator::Unreachable, []) => None,

        (Operator::I32Load { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(4)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::I32(read_u32(
                &global.memories[memory.memory],
                addr,
            )))
        }),
        (Operator::I64Load { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(8)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::I64(read_u64(
                &global.memories[memory.memory],
                addr,
            )))
        }),
        (Operator::F32Load { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(4)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::F32(read_u32(
                &global.memories[memory.memory],
                addr,
            )))
        }),
        (Operator::F64Load { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(8)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::F64(read_u64(
                &global.memories[memory.memory],
                addr,
            )))
        }),
        (Operator::I32Load8S { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(1)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::I32(
                read_u8(&global.memories[memory.memory], addr) as i8 as i32 as u32,
            ))
        }),
        (Operator::I32Load8U { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(4)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::I32(
                read_u8(&global.memories[memory.memory], addr) as u32,
            ))
        }),
        (Operator::I32Load16S { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(2)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::I32(
                read_u16(&global.memories[memory.memory], addr) as i16 as i32 as u32,
            ))
        }),
        (Operator::I32Load16U { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(2)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::I32(
                read_u16(&global.memories[memory.memory], addr) as u32,
            ))
        }),
        (Operator::I64Load8S { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(1)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::I64(
                read_u8(&global.memories[memory.memory], addr) as i8 as i64 as u64,
            ))
        }),
        (Operator::I64Load8U { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(1)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::I64(
                read_u8(&global.memories[memory.memory], addr) as u64,
            ))
        }),
        (Operator::I64Load16S { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(2)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::I64(
                read_u16(&global.memories[memory.memory], addr) as i16 as i64 as u64,
            ))
        }),
        (Operator::I64Load16U { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(2)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::I64(
                read_u16(&global.memories[memory.memory], addr) as u64,
            ))
        }),
        (Operator::I64Load32S { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(4)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::I64(
                read_u32(&global.memories[memory.memory], addr) as i32 as i64 as u64,
            ))
        }),
        (Operator::I64Load32U { memory }, [ConstVal::I32(addr)]) => ctx.and_then(|global| {
            let addr = addr.checked_add(memory.offset)?;
            if addr.checked_add(4)? > global.memories[memory.memory].data.len() as u32 {
                return None;
            }
            Some(ConstVal::I64(
                read_u32(&global.memories[memory.memory], addr) as u64,
            ))
        }),
        (Operator::I32Store { memory }, [ConstVal::I32(addr), ConstVal::I32(data)]) => ctx
            .and_then(|global| {
                let addr = addr.checked_add(memory.offset)?;
                if addr.checked_add(4)? > global.memories[memory.memory].data.len() as u32 {
                    return None;
                }
                write_u32(&mut global.memories[memory.memory], addr, *data);
                Some(ConstVal::None)
            }),
        (Operator::I64Store { memory }, [ConstVal::I32(addr), ConstVal::I64(data)]) => ctx
            .and_then(|global| {
                let addr = addr.checked_add(memory.offset)?;
                if addr.checked_add(8)? > global.memories[memory.memory].data.len() as u32 {
                    return None;
                }
                write_u64(&mut global.memories[memory.memory], addr, *data);
                Some(ConstVal::None)
            }),
        (Operator::I32Store8 { memory }, [ConstVal::I32(addr), ConstVal::I32(data)]) => ctx
            .and_then(|global| {
                let addr = addr.checked_add(memory.offset)?;
                if addr.checked_add(1)? > global.memories[memory.memory].data.len() as u32 {
                    return None;
                }
                write_u8(&mut global.memories[memory.memory], addr, *data as u8);
                Some(ConstVal::None)
            }),
        (Operator::I32Store16 { memory }, [ConstVal::I32(addr), ConstVal::I32(data)]) => ctx
            .and_then(|global| {
                let addr = addr.checked_add(memory.offset)?;
                if addr.checked_add(2)? > global.memories[memory.memory].data.len() as u32 {
                    return None;
                }
                write_u16(&mut global.memories[memory.memory], addr, *data as u16);
                Some(ConstVal::None)
            }),
        (Operator::I64Store8 { memory }, [ConstVal::I32(addr), ConstVal::I64(data)]) => ctx
            .and_then(|global| {
                let addr = addr.checked_add(memory.offset)?;
                if addr.checked_add(1)? > global.memories[memory.memory].data.len() as u32 {
                    return None;
                }
                write_u8(&mut global.memories[memory.memory], addr, *data as u8);
                Some(ConstVal::None)
            }),
        (Operator::I64Store16 { memory }, [ConstVal::I32(addr), ConstVal::I64(data)]) => ctx
            .and_then(|global| {
                let addr = addr.checked_add(memory.offset)?;
                if addr.checked_add(2)? > global.memories[memory.memory].data.len() as u32 {
                    return None;
                }
                write_u16(&mut global.memories[memory.memory], addr, *data as u16);
                Some(ConstVal::None)
            }),
        (Operator::I64Store32 { memory }, [ConstVal::I32(addr), ConstVal::I64(data)]) => ctx
            .and_then(|global| {
                let addr = addr.checked_add(memory.offset)?;
                if addr.checked_add(4)? > global.memories[memory.memory].data.len() as u32 {
                    return None;
                }
                write_u32(&mut global.memories[memory.memory], addr, *data as u32);
                Some(ConstVal::None)
            }),
        (Operator::F32Store { memory }, [ConstVal::I32(addr), ConstVal::F32(data)]) => ctx
            .and_then(|global| {
                let addr = addr.checked_add(memory.offset)?;
                if addr.checked_add(4)? > global.memories[memory.memory].data.len() as u32 {
                    return None;
                }
                write_u32(&mut global.memories[memory.memory], addr, *data);
                Some(ConstVal::None)
            }),
        (Operator::F64Store { memory }, [ConstVal::I32(addr), ConstVal::F64(data)]) => ctx
            .and_then(|global| {
                let addr = addr.checked_add(memory.offset)?;
                if addr.checked_add(8)? > global.memories[memory.memory].data.len() as u32 {
                    return None;
                }
                write_u64(&mut global.memories[memory.memory], addr, *data);
                Some(ConstVal::None)
            }),
        (_, args) if args.iter().any(|&arg| arg == ConstVal::None) => None,
        _ => None,
    }
}

pub(crate) fn read_u8(mem: &InterpMemory, addr: u32) -> u8 {
    let addr = addr as usize;
    mem.data[addr]
}

pub(crate) fn read_u16(mem: &InterpMemory, addr: u32) -> u16 {
    use std::convert::TryInto;
    let addr = addr as usize;
    u16::from_le_bytes(mem.data[addr..(addr + 2)].try_into().unwrap())
}

pub(crate) fn read_u32(mem: &InterpMemory, addr: u32) -> u32 {
    use std::convert::TryInto;
    let addr = addr as usize;
    u32::from_le_bytes(mem.data[addr..(addr + 4)].try_into().unwrap())
}

pub(crate) fn read_u64(mem: &InterpMemory, addr: u32) -> u64 {
    use std::convert::TryInto;
    let addr = addr as usize;
    u64::from_le_bytes(mem.data[addr..(addr + 8)].try_into().unwrap())
}

pub(crate) fn write_u8(mem: &mut InterpMemory, addr: u32, data: u8) {
    let addr = addr as usize;
    mem.data[addr] = data;
}

pub(crate) fn write_u16(mem: &mut InterpMemory, addr: u32, data: u16) {
    let addr = addr as usize;
    mem.data[addr..(addr + 2)].copy_from_slice(&data.to_le_bytes()[..]);
}

pub(crate) fn write_u32(mem: &mut InterpMemory, addr: u32, data: u32) {
    let addr = addr as usize;
    mem.data[addr..(addr + 4)].copy_from_slice(&data.to_le_bytes()[..]);
}

pub(crate) fn write_u64(mem: &mut InterpMemory, addr: u32, data: u64) {
    let addr = addr as usize;
    mem.data[addr..(addr + 8)].copy_from_slice(&data.to_le_bytes()[..]);
}

// Min/max implementations with proper handling for negative-zero (as
// distinct from positive-zero): see
// https://github.com/wasmi-labs/wasmi/blob/6d3729c17e6d8bcabb8cd7fed0f6278f17f94e06/crates/core/src/value.rs#L575.

fn f32_min(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else if a > b {
        b
    } else if a == b {
        if a.is_sign_negative() && b.is_sign_positive() {
            a
        } else {
            b
        }
    } else {
        f32::NAN
    }
}
fn f32_max(a: f32, b: f32) -> f32 {
    if a < b {
        b
    } else if a > b {
        a
    } else if a == b {
        if a.is_sign_positive() && b.is_sign_negative() {
            a
        } else {
            b
        }
    } else {
        f32::NAN
    }
}
fn f64_min(a: f64, b: f64) -> f64 {
    if a < b {
        a
    } else if a > b {
        b
    } else if a == b {
        if a.is_sign_negative() && b.is_sign_positive() {
            a
        } else {
            b
        }
    } else {
        f64::NAN
    }
}
fn f64_max(a: f64, b: f64) -> f64 {
    if a < b {
        b
    } else if a > b {
        a
    } else if a == b {
        if a.is_sign_positive() && b.is_sign_negative() {
            a
        } else {
            b
        }
    } else {
        f64::NAN
    }
}
