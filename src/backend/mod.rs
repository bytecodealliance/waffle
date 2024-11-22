//! Backend: IR to Wasm.

use crate::cfg::CFGInfo;
use crate::entity::EntityRef;
use crate::ir::{ExportKind, FuncDecl, FunctionBody, ImportKind, Module, Type, Value, ValueDef};
use crate::Operator;
use anyhow::Result;
use rayon::prelude::*;
use std::borrow::Cow;

pub mod reducify;
use reducify::Reducifier;
pub mod stackify;
use stackify::{Context as StackifyContext, WasmBlock};
pub mod treeify;
use treeify::Trees;
pub mod localify;
use localify::Localifier;

pub struct WasmFuncBackend<'a> {
    body: Cow<'a, FunctionBody>,
    cfg: CFGInfo,
}

struct CompileContext<'a> {
    trees: Trees,
    ctrl: Vec<WasmBlock<'a>>,
    locals: Localifier,
}

macro_rules! op {
    ($name:tt) => {
        Some(wasm_encoder::Instruction::$name)
    };
}

impl<'a> WasmFuncBackend<'a> {
    pub fn compile(body: &'a FunctionBody) -> Result<wasm_encoder::Function> {
        body.validate()?;
        log::debug!("Backend compiling:\n{}\n", body.display_verbose("| ", None));
        // For ownership reasons (to avoid a self-referential struct
        // with the `Cow::Owned` case when the Reducifier modifies the
        // body), we have to run the Reducifier first, own its result
        // in this stack frame, then construct the `WasmFuncBackend`
        // state and run the rest of the compilation in `lower()`.
        let body = Reducifier::new(body).run();
        let cfg = CFGInfo::new(&body);
        let state = WasmFuncBackend { body, cfg };
        state.lower()
    }

    pub fn lower(&self) -> Result<wasm_encoder::Function> {
        log::debug!("CFG:\n{:?}\n", self.cfg);
        let trees = Trees::compute(&self.body);
        log::debug!("Trees:\n{:?}\n", trees);
        let ctrl = StackifyContext::new(&self.body, &self.cfg)?.compute();
        log::debug!("Ctrl:\n{:?}\n", ctrl);
        let locals = Localifier::compute(&self.body, &self.cfg, &trees);
        log::debug!("Locals:\n{:?}\n", locals);

        let ctx = CompileContext {
            trees,
            ctrl,
            locals,
        };

        let mut func = wasm_encoder::Function::new(
            ctx.locals
                .locals
                .values()
                .skip(self.body.blocks[self.body.entry].params.len())
                .map(|&ty| (1, wasm_encoder::ValType::from(ty)))
                .collect::<Vec<_>>(),
        );

        for block in &ctx.ctrl {
            self.lower_block(&ctx, block, &mut func);
        }

        // If the last block was a Block, Loop or If, then the type
        // may not match, so end with an Unreachable.
        match ctx.ctrl.last() {
            Some(&WasmBlock::Block { .. })
            | Some(&WasmBlock::Loop { .. })
            | Some(&WasmBlock::If { .. }) => {
                func.instruction(&wasm_encoder::Instruction::Unreachable);
            }
            _ => {}
        }
        func.instruction(&wasm_encoder::Instruction::End);

        log::debug!("Compiled to:\n{:?}\n", func);

        Ok(func)
    }

    fn lower_block(
        &self,
        ctx: &CompileContext<'_>,
        block: &WasmBlock<'_>,
        func: &mut wasm_encoder::Function,
    ) {
        match block {
            WasmBlock::Block { body, .. } => {
                func.instruction(&wasm_encoder::Instruction::Block(
                    wasm_encoder::BlockType::Empty,
                ));
                for sub_block in &body[..] {
                    self.lower_block(ctx, sub_block, func);
                }
                func.instruction(&wasm_encoder::Instruction::End);
            }
            WasmBlock::Loop { body, .. } => {
                func.instruction(&wasm_encoder::Instruction::Loop(
                    wasm_encoder::BlockType::Empty,
                ));
                for sub_block in &body[..] {
                    self.lower_block(ctx, sub_block, func);
                }
                func.instruction(&wasm_encoder::Instruction::End);
            }
            WasmBlock::Br { target } => {
                func.instruction(&wasm_encoder::Instruction::Br(target.index()));
            }
            WasmBlock::If {
                cond,
                if_true,
                if_false,
            } => {
                self.lower_value(ctx, *cond, func);
                func.instruction(&wasm_encoder::Instruction::If(
                    wasm_encoder::BlockType::Empty,
                ));
                for sub_block in &if_true[..] {
                    self.lower_block(ctx, sub_block, func);
                }
                if if_false.len() > 0 {
                    func.instruction(&wasm_encoder::Instruction::Else);
                    for sub_block in &if_false[..] {
                        self.lower_block(ctx, sub_block, func);
                    }
                }
                func.instruction(&wasm_encoder::Instruction::End);
            }
            WasmBlock::Select {
                selector,
                targets,
                default,
            } => {
                self.lower_value(ctx, *selector, func);
                func.instruction(&wasm_encoder::Instruction::BrTable(
                    Cow::Owned(
                        targets
                            .iter()
                            .map(|label| label.index())
                            .collect::<Vec<_>>(),
                    ),
                    default.index(),
                ));
            }
            WasmBlock::Leaf { block } => {
                for &inst in &self.body.blocks[*block].insts {
                    // If this value is "owned", do nothing: it will be lowered in
                    // the one place it's used.
                    if ctx.trees.owner.contains_key(&inst) || ctx.trees.remat.contains(&inst) {
                        continue;
                    }
                    if let &ValueDef::Operator(..) = &self.body.values[inst] {
                        self.lower_inst(ctx, inst, /* root = */ true, func);
                    }
                }
            }
            WasmBlock::BlockParams { from, to } => {
                debug_assert_eq!(from.len(), to.len());
                for (&from, &(to_ty, to)) in from.iter().zip(to.iter()) {
                    if ctx.locals.values[to].is_empty() {
                        continue;
                    }
                    let from_ty = self.body.values[self.body.resolve_alias(from)]
                        .ty(&self.body.type_pool)
                        .unwrap();
                    assert_eq!(from_ty, to_ty);
                    if ctx.locals.values[from].len() == 1 {
                        assert_eq!(from_ty, ctx.locals.locals[ctx.locals.values[from][0]]);
                    }
                    self.lower_value(ctx, from, func);
                }
                for &(to_ty, to) in to.iter().rev() {
                    if ctx.locals.values[to].is_empty() {
                        continue;
                    }
                    if ctx.locals.values[to].len() == 1 {
                        assert_eq!(to_ty, ctx.locals.locals[ctx.locals.values[to][0]]);
                    }
                    self.lower_set_value(ctx, to, func);
                }
            }
            WasmBlock::Return { values } => {
                for &value in &values[..] {
                    self.lower_value(ctx, value, func);
                }
                func.instruction(&wasm_encoder::Instruction::Return);
            }
            WasmBlock::Unreachable => {
                func.instruction(&wasm_encoder::Instruction::Unreachable);
            }
        }
    }

    fn lower_value(
        &self,
        ctx: &CompileContext<'_>,
        value: Value,
        func: &mut wasm_encoder::Function,
    ) {
        log::trace!("lower_value: value {}", value);
        let value = self.body.resolve_alias(value);
        if ctx.trees.remat.contains(&value) {
            self.lower_inst(ctx, value, /* root = */ false, func);
        } else {
            let local = match &self.body.values[value] {
                &ValueDef::BlockParam(..) | &ValueDef::Operator(..) => ctx.locals.values[value][0],
                &ValueDef::PickOutput(orig_value, idx, _) => {
                    ctx.locals.values[orig_value][idx as usize]
                }
                val => unreachable!("bad value ({}): {:?}", value, val),
            };
            func.instruction(&wasm_encoder::Instruction::LocalGet(local.index() as u32));
        }
    }

    fn lower_set_value(
        &self,
        ctx: &CompileContext<'a>,
        value: Value,
        func: &mut wasm_encoder::Function,
    ) {
        debug_assert_eq!(
            ctx.locals.values[value].len(),
            1,
            "Value {} has no local",
            value
        );
        let local = ctx.locals.values[value][0];
        func.instruction(&wasm_encoder::Instruction::LocalSet(local.index() as u32));
    }

    fn lower_inst(
        &self,
        ctx: &CompileContext<'a>,
        value: Value,
        root: bool,
        func: &mut wasm_encoder::Function,
    ) {
        log::trace!("lower_inst: value {} root {}", value, root);
        match &self.body.values[value] {
            &ValueDef::Operator(ref op, args, tys) => {
                for &arg in &self.body.arg_pool[args] {
                    let arg = self.body.resolve_alias(arg);
                    if ctx.trees.owner.contains_key(&arg) || ctx.trees.remat.contains(&arg) {
                        log::trace!(" -> arg {} is owned", arg);
                        self.lower_inst(ctx, arg, /* root = */ false, func);
                    } else {
                        self.lower_value(ctx, arg, func);
                    }
                }
                self.lower_op(op, func);
                if root {
                    for &local in &ctx.locals.values[value] {
                        func.instruction(
                            &wasm_encoder::Instruction::LocalSet(local.index() as u32),
                        );
                    }
                    let leftovers = tys.len() - ctx.locals.values[value].len();
                    for _ in 0..leftovers {
                        func.instruction(&wasm_encoder::Instruction::Drop);
                    }
                }
            }
            &ValueDef::PickOutput(..) => {
                self.lower_value(ctx, value, func);
            }
            def => unreachable!("Unexpected inst: {:?}", def),
        }
    }

    fn lower_op(&self, op: &Operator, func: &mut wasm_encoder::Function) {
        let inst = match op {
            Operator::Unreachable => Some(wasm_encoder::Instruction::Unreachable),
            Operator::Nop => None,
            Operator::Call { function_index } => Some(wasm_encoder::Instruction::Call(
                function_index.index() as u32,
            )),
            Operator::CallIndirect {
                sig_index,
                table_index,
            } => Some(wasm_encoder::Instruction::CallIndirect {
                type_index: sig_index.index() as u32,
                table_index: table_index.index() as u32,
            }),
            Operator::Select => Some(wasm_encoder::Instruction::Select),
            Operator::TypedSelect { ty } => Some(wasm_encoder::Instruction::TypedSelect(
                wasm_encoder::ValType::from(*ty),
            )),
            Operator::GlobalGet { global_index } => Some(wasm_encoder::Instruction::GlobalGet(
                global_index.index() as u32,
            )),
            Operator::GlobalSet { global_index } => Some(wasm_encoder::Instruction::GlobalSet(
                global_index.index() as u32,
            )),
            Operator::I32Load { memory } => Some(wasm_encoder::Instruction::I32Load(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I64Load { memory } => Some(wasm_encoder::Instruction::I64Load(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::F32Load { memory } => Some(wasm_encoder::Instruction::F32Load(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::F64Load { memory } => Some(wasm_encoder::Instruction::F64Load(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I32Load8S { memory } => Some(wasm_encoder::Instruction::I32Load8S(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I32Load8U { memory } => Some(wasm_encoder::Instruction::I32Load8U(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I32Load16S { memory } => Some(wasm_encoder::Instruction::I32Load16S(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I32Load16U { memory } => Some(wasm_encoder::Instruction::I32Load16U(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I64Load8S { memory } => Some(wasm_encoder::Instruction::I64Load8S(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I64Load8U { memory } => Some(wasm_encoder::Instruction::I64Load8U(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I64Load16S { memory } => Some(wasm_encoder::Instruction::I64Load16S(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I64Load16U { memory } => Some(wasm_encoder::Instruction::I64Load16U(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I64Load32S { memory } => Some(wasm_encoder::Instruction::I64Load32S(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I64Load32U { memory } => Some(wasm_encoder::Instruction::I64Load32U(
                wasm_encoder::MemArg::from(*memory),
            )),

            Operator::I32Store { memory } => Some(wasm_encoder::Instruction::I32Store(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I64Store { memory } => Some(wasm_encoder::Instruction::I64Store(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::F32Store { memory } => Some(wasm_encoder::Instruction::F32Store(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::F64Store { memory } => Some(wasm_encoder::Instruction::F64Store(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I32Store8 { memory } => Some(wasm_encoder::Instruction::I32Store8(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I32Store16 { memory } => Some(wasm_encoder::Instruction::I32Store16(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I64Store8 { memory } => Some(wasm_encoder::Instruction::I64Store8(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I64Store16 { memory } => Some(wasm_encoder::Instruction::I64Store16(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::I64Store32 { memory } => Some(wasm_encoder::Instruction::I64Store32(
                wasm_encoder::MemArg::from(*memory),
            )),

            Operator::I32Const { value } => {
                Some(wasm_encoder::Instruction::I32Const(*value as i32))
            }
            Operator::I64Const { value } => {
                Some(wasm_encoder::Instruction::I64Const(*value as i64))
            }
            Operator::F32Const { value } => {
                Some(wasm_encoder::Instruction::F32Const(f32::from_bits(*value)))
            }
            Operator::F64Const { value } => {
                Some(wasm_encoder::Instruction::F64Const(f64::from_bits(*value)))
            }

            Operator::I32Eqz => op!(I32Eqz),
            Operator::I32Eq => op!(I32Eq),
            Operator::I32Ne => op!(I32Ne),
            Operator::I32LtS => op!(I32LtS),
            Operator::I32LtU => op!(I32LtU),
            Operator::I32GtS => op!(I32GtS),
            Operator::I32GtU => op!(I32GtU),
            Operator::I32LeS => op!(I32LeS),
            Operator::I32LeU => op!(I32LeU),
            Operator::I32GeS => op!(I32GeS),
            Operator::I32GeU => op!(I32GeU),

            Operator::I64Eqz => op!(I64Eqz),

            Operator::I64Eq => op!(I64Eq),
            Operator::I64Ne => op!(I64Ne),
            Operator::I64LtS => op!(I64LtS),
            Operator::I64LtU => op!(I64LtU),
            Operator::I64GtU => op!(I64GtU),
            Operator::I64GtS => op!(I64GtS),
            Operator::I64LeS => op!(I64LeS),
            Operator::I64LeU => op!(I64LeU),
            Operator::I64GeS => op!(I64GeS),
            Operator::I64GeU => op!(I64GeU),

            Operator::F32Eq => op!(F32Eq),
            Operator::F32Ne => op!(F32Ne),
            Operator::F32Lt => op!(F32Lt),
            Operator::F32Gt => op!(F32Gt),
            Operator::F32Le => op!(F32Le),
            Operator::F32Ge => op!(F32Ge),

            Operator::F64Eq => op!(F64Eq),
            Operator::F64Ne => op!(F64Ne),
            Operator::F64Lt => op!(F64Lt),
            Operator::F64Gt => op!(F64Gt),
            Operator::F64Le => op!(F64Le),
            Operator::F64Ge => op!(F64Ge),

            Operator::I32Clz => op!(I32Clz),
            Operator::I32Ctz => op!(I32Ctz),
            Operator::I32Popcnt => op!(I32Popcnt),

            Operator::I32Add => op!(I32Add),
            Operator::I32Sub => op!(I32Sub),
            Operator::I32Mul => op!(I32Mul),
            Operator::I32DivS => op!(I32DivS),
            Operator::I32DivU => op!(I32DivU),
            Operator::I32RemS => op!(I32RemS),
            Operator::I32RemU => op!(I32RemU),
            Operator::I32And => op!(I32And),
            Operator::I32Or => op!(I32Or),
            Operator::I32Xor => op!(I32Xor),
            Operator::I32Shl => op!(I32Shl),
            Operator::I32ShrS => op!(I32ShrS),
            Operator::I32ShrU => op!(I32ShrU),
            Operator::I32Rotl => op!(I32Rotl),
            Operator::I32Rotr => op!(I32Rotr),

            Operator::I64Clz => op!(I64Clz),
            Operator::I64Ctz => op!(I64Ctz),
            Operator::I64Popcnt => op!(I64Popcnt),

            Operator::I64Add => op!(I64Add),
            Operator::I64Sub => op!(I64Sub),
            Operator::I64Mul => op!(I64Mul),
            Operator::I64DivS => op!(I64DivS),
            Operator::I64DivU => op!(I64DivU),
            Operator::I64RemS => op!(I64RemS),
            Operator::I64RemU => op!(I64RemU),
            Operator::I64And => op!(I64And),
            Operator::I64Or => op!(I64Or),
            Operator::I64Xor => op!(I64Xor),
            Operator::I64Shl => op!(I64Shl),
            Operator::I64ShrS => op!(I64ShrS),
            Operator::I64ShrU => op!(I64ShrU),
            Operator::I64Rotl => op!(I64Rotl),
            Operator::I64Rotr => op!(I64Rotr),

            Operator::F32Abs => op!(F32Abs),
            Operator::F32Neg => op!(F32Neg),
            Operator::F32Ceil => op!(F32Ceil),
            Operator::F32Floor => op!(F32Floor),
            Operator::F32Trunc => op!(F32Trunc),
            Operator::F32Nearest => op!(F32Nearest),
            Operator::F32Sqrt => op!(F32Sqrt),

            Operator::F32Add => op!(F32Add),
            Operator::F32Sub => op!(F32Sub),
            Operator::F32Mul => op!(F32Mul),
            Operator::F32Div => op!(F32Div),
            Operator::F32Min => op!(F32Min),
            Operator::F32Max => op!(F32Max),
            Operator::F32Copysign => op!(F32Copysign),

            Operator::F64Abs => op!(F64Abs),
            Operator::F64Neg => op!(F64Neg),
            Operator::F64Ceil => op!(F64Ceil),
            Operator::F64Floor => op!(F64Floor),
            Operator::F64Trunc => op!(F64Trunc),
            Operator::F64Nearest => op!(F64Nearest),
            Operator::F64Sqrt => op!(F64Sqrt),

            Operator::F64Add => op!(F64Add),
            Operator::F64Sub => op!(F64Sub),
            Operator::F64Mul => op!(F64Mul),
            Operator::F64Div => op!(F64Div),
            Operator::F64Min => op!(F64Min),
            Operator::F64Max => op!(F64Max),
            Operator::F64Copysign => op!(F64Copysign),

            Operator::I32WrapI64 => op!(I32WrapI64),
            Operator::I32TruncF32S => op!(I32TruncF32S),
            Operator::I32TruncF32U => op!(I32TruncF32U),
            Operator::I32TruncF64S => op!(I32TruncF64S),
            Operator::I32TruncF64U => op!(I32TruncF64U),
            Operator::I64ExtendI32S => op!(I64ExtendI32S),
            Operator::I64ExtendI32U => op!(I64ExtendI32U),
            Operator::I64TruncF32S => op!(I64TruncF32S),
            Operator::I64TruncF32U => op!(I64TruncF32U),
            Operator::I64TruncF64S => op!(I64TruncF64S),
            Operator::I64TruncF64U => op!(I64TruncF64U),
            Operator::F32ConvertI32S => op!(F32ConvertI32S),
            Operator::F32ConvertI32U => op!(F32ConvertI32U),
            Operator::F32ConvertI64S => op!(F32ConvertI64S),
            Operator::F32ConvertI64U => op!(F32ConvertI64U),
            Operator::F32DemoteF64 => op!(F32DemoteF64),
            Operator::F64ConvertI32S => op!(F64ConvertI32S),
            Operator::F64ConvertI32U => op!(F64ConvertI32U),
            Operator::F64ConvertI64S => op!(F64ConvertI64S),
            Operator::F64ConvertI64U => op!(F64ConvertI64U),
            Operator::F64PromoteF32 => op!(F64PromoteF32),
            Operator::I32Extend8S => op!(I32Extend8S),
            Operator::I32Extend16S => op!(I32Extend16S),
            Operator::I64Extend8S => op!(I64Extend8S),
            Operator::I64Extend16S => op!(I64Extend16S),
            Operator::I64Extend32S => op!(I64Extend32S),
            Operator::I32TruncSatF32S => op!(I32TruncSatF32S),
            Operator::I32TruncSatF32U => op!(I32TruncSatF32U),
            Operator::I32TruncSatF64S => op!(I32TruncSatF64S),
            Operator::I32TruncSatF64U => op!(I32TruncSatF64U),
            Operator::I64TruncSatF32S => op!(I64TruncSatF32S),
            Operator::I64TruncSatF32U => op!(I64TruncSatF32U),
            Operator::I64TruncSatF64S => op!(I64TruncSatF64S),
            Operator::I64TruncSatF64U => op!(I64TruncSatF64U),
            Operator::F32ReinterpretI32 => op!(F32ReinterpretI32),
            Operator::F64ReinterpretI64 => op!(F64ReinterpretI64),
            Operator::I32ReinterpretF32 => op!(I32ReinterpretF32),
            Operator::I64ReinterpretF64 => op!(I64ReinterpretF64),

            Operator::TableGet { table_index } => Some(wasm_encoder::Instruction::TableGet(
                table_index.index() as u32,
            )),
            Operator::TableSet { table_index } => Some(wasm_encoder::Instruction::TableSet(
                table_index.index() as u32,
            )),
            Operator::TableGrow { table_index } => Some(wasm_encoder::Instruction::TableGrow(
                table_index.index() as u32,
            )),
            Operator::TableSize { table_index } => Some(wasm_encoder::Instruction::TableSize(
                table_index.index() as u32,
            )),
            Operator::MemorySize { mem } => {
                Some(wasm_encoder::Instruction::MemorySize(mem.index() as u32))
            }
            Operator::MemoryGrow { mem } => {
                Some(wasm_encoder::Instruction::MemoryGrow(mem.index() as u32))
            }
            Operator::MemoryCopy { dst_mem, src_mem } => {
                Some(wasm_encoder::Instruction::MemoryCopy {
                    src_mem: src_mem.index() as u32,
                    dst_mem: dst_mem.index() as u32,
                })
            }
            Operator::MemoryFill { mem } => {
                Some(wasm_encoder::Instruction::MemoryFill(mem.index() as u32))
            }

            Operator::V128Load { memory } => Some(wasm_encoder::Instruction::V128Load(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::V128Load8x8S { memory } => Some(wasm_encoder::Instruction::V128Load8x8S(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::V128Load8x8U { memory } => Some(wasm_encoder::Instruction::V128Load8x8U(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::V128Load16x4S { memory } => Some(wasm_encoder::Instruction::V128Load16x4S(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::V128Load16x4U { memory } => Some(wasm_encoder::Instruction::V128Load16x4U(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::V128Load32x2S { memory } => Some(wasm_encoder::Instruction::V128Load32x2S(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::V128Load32x2U { memory } => Some(wasm_encoder::Instruction::V128Load32x2U(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::V128Load8Splat { memory } => Some(wasm_encoder::Instruction::V128Load8Splat(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::V128Load16Splat { memory } => Some(
                wasm_encoder::Instruction::V128Load16Splat(wasm_encoder::MemArg::from(*memory)),
            ),
            Operator::V128Load32Splat { memory } => Some(
                wasm_encoder::Instruction::V128Load32Splat(wasm_encoder::MemArg::from(*memory)),
            ),
            Operator::V128Load64Splat { memory } => Some(
                wasm_encoder::Instruction::V128Load64Splat(wasm_encoder::MemArg::from(*memory)),
            ),
            Operator::V128Load32Zero { memory } => Some(wasm_encoder::Instruction::V128Load32Zero(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::V128Load64Zero { memory } => Some(wasm_encoder::Instruction::V128Load64Zero(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::V128Store { memory } => Some(wasm_encoder::Instruction::V128Store(
                wasm_encoder::MemArg::from(*memory),
            )),
            Operator::V128Load8Lane { memory, lane } => {
                Some(wasm_encoder::Instruction::V128Load8Lane {
                    memarg: wasm_encoder::MemArg::from(*memory),
                    lane: *lane,
                })
            }
            Operator::V128Load16Lane { memory, lane } => {
                Some(wasm_encoder::Instruction::V128Load16Lane {
                    memarg: wasm_encoder::MemArg::from(*memory),
                    lane: *lane,
                })
            }
            Operator::V128Load32Lane { memory, lane } => {
                Some(wasm_encoder::Instruction::V128Load32Lane {
                    memarg: wasm_encoder::MemArg::from(*memory),
                    lane: *lane,
                })
            }
            Operator::V128Load64Lane { memory, lane } => {
                Some(wasm_encoder::Instruction::V128Load64Lane {
                    memarg: wasm_encoder::MemArg::from(*memory),
                    lane: *lane,
                })
            }
            Operator::V128Store8Lane { memory, lane } => {
                Some(wasm_encoder::Instruction::V128Store8Lane {
                    memarg: wasm_encoder::MemArg::from(*memory),
                    lane: *lane,
                })
            }
            Operator::V128Store16Lane { memory, lane } => {
                Some(wasm_encoder::Instruction::V128Store16Lane {
                    memarg: wasm_encoder::MemArg::from(*memory),
                    lane: *lane,
                })
            }
            Operator::V128Store32Lane { memory, lane } => {
                Some(wasm_encoder::Instruction::V128Store32Lane {
                    memarg: wasm_encoder::MemArg::from(*memory),
                    lane: *lane,
                })
            }
            Operator::V128Store64Lane { memory, lane } => {
                Some(wasm_encoder::Instruction::V128Store64Lane {
                    memarg: wasm_encoder::MemArg::from(*memory),
                    lane: *lane,
                })
            }
            Operator::V128Const { value } => {
                Some(wasm_encoder::Instruction::V128Const(*value as i128))
            }

            Operator::I8x16Shuffle { lanes } => {
                Some(wasm_encoder::Instruction::I8x16Shuffle(lanes.clone()))
            }

            Operator::I8x16ExtractLaneS { lane } => {
                Some(wasm_encoder::Instruction::I8x16ExtractLaneS(*lane))
            }
            Operator::I8x16ExtractLaneU { lane } => {
                Some(wasm_encoder::Instruction::I8x16ExtractLaneU(*lane))
            }
            Operator::I8x16ReplaceLane { lane } => {
                Some(wasm_encoder::Instruction::I8x16ReplaceLane(*lane))
            }
            Operator::I16x8ExtractLaneS { lane } => {
                Some(wasm_encoder::Instruction::I16x8ExtractLaneS(*lane))
            }
            Operator::I16x8ExtractLaneU { lane } => {
                Some(wasm_encoder::Instruction::I16x8ExtractLaneU(*lane))
            }
            Operator::I16x8ReplaceLane { lane } => {
                Some(wasm_encoder::Instruction::I16x8ReplaceLane(*lane))
            }
            Operator::I32x4ExtractLane { lane } => {
                Some(wasm_encoder::Instruction::I32x4ExtractLane(*lane))
            }
            Operator::I32x4ReplaceLane { lane } => {
                Some(wasm_encoder::Instruction::I32x4ReplaceLane(*lane))
            }
            Operator::I64x2ExtractLane { lane } => {
                Some(wasm_encoder::Instruction::I64x2ExtractLane(*lane))
            }
            Operator::I64x2ReplaceLane { lane } => {
                Some(wasm_encoder::Instruction::I64x2ReplaceLane(*lane))
            }
            Operator::F32x4ExtractLane { lane } => {
                Some(wasm_encoder::Instruction::F32x4ExtractLane(*lane))
            }
            Operator::F32x4ReplaceLane { lane } => {
                Some(wasm_encoder::Instruction::F32x4ReplaceLane(*lane))
            }
            Operator::F64x2ExtractLane { lane } => {
                Some(wasm_encoder::Instruction::F64x2ExtractLane(*lane))
            }
            Operator::F64x2ReplaceLane { lane } => {
                Some(wasm_encoder::Instruction::F64x2ReplaceLane(*lane))
            }

            Operator::I8x16Swizzle => Some(wasm_encoder::Instruction::I8x16Swizzle),
            Operator::I8x16Splat => Some(wasm_encoder::Instruction::I8x16Splat),
            Operator::I16x8Splat => Some(wasm_encoder::Instruction::I16x8Splat),
            Operator::I32x4Splat => Some(wasm_encoder::Instruction::I32x4Splat),
            Operator::I64x2Splat => Some(wasm_encoder::Instruction::I64x2Splat),
            Operator::F32x4Splat => Some(wasm_encoder::Instruction::F32x4Splat),
            Operator::F64x2Splat => Some(wasm_encoder::Instruction::F64x2Splat),

            Operator::I8x16Eq => Some(wasm_encoder::Instruction::I8x16Eq),
            Operator::I8x16Ne => Some(wasm_encoder::Instruction::I8x16Ne),
            Operator::I8x16LtS => Some(wasm_encoder::Instruction::I8x16LtS),
            Operator::I8x16LtU => Some(wasm_encoder::Instruction::I8x16LtU),
            Operator::I8x16GtS => Some(wasm_encoder::Instruction::I8x16GtS),
            Operator::I8x16GtU => Some(wasm_encoder::Instruction::I8x16GtU),
            Operator::I8x16LeS => Some(wasm_encoder::Instruction::I8x16LeS),
            Operator::I8x16LeU => Some(wasm_encoder::Instruction::I8x16LeU),
            Operator::I8x16GeS => Some(wasm_encoder::Instruction::I8x16GeS),
            Operator::I8x16GeU => Some(wasm_encoder::Instruction::I8x16GeU),

            Operator::I16x8Eq => Some(wasm_encoder::Instruction::I16x8Eq),
            Operator::I16x8Ne => Some(wasm_encoder::Instruction::I16x8Ne),
            Operator::I16x8LtS => Some(wasm_encoder::Instruction::I16x8LtS),
            Operator::I16x8LtU => Some(wasm_encoder::Instruction::I16x8LtU),
            Operator::I16x8GtS => Some(wasm_encoder::Instruction::I16x8GtS),
            Operator::I16x8GtU => Some(wasm_encoder::Instruction::I16x8GtU),
            Operator::I16x8LeS => Some(wasm_encoder::Instruction::I16x8LeS),
            Operator::I16x8LeU => Some(wasm_encoder::Instruction::I16x8LeU),
            Operator::I16x8GeS => Some(wasm_encoder::Instruction::I16x8GeS),
            Operator::I16x8GeU => Some(wasm_encoder::Instruction::I16x8GeU),

            Operator::I32x4Eq => Some(wasm_encoder::Instruction::I32x4Eq),
            Operator::I32x4Ne => Some(wasm_encoder::Instruction::I32x4Ne),
            Operator::I32x4LtS => Some(wasm_encoder::Instruction::I32x4LtS),
            Operator::I32x4LtU => Some(wasm_encoder::Instruction::I32x4LtU),
            Operator::I32x4GtS => Some(wasm_encoder::Instruction::I32x4GtS),
            Operator::I32x4GtU => Some(wasm_encoder::Instruction::I32x4GtU),
            Operator::I32x4LeS => Some(wasm_encoder::Instruction::I32x4LeS),
            Operator::I32x4LeU => Some(wasm_encoder::Instruction::I32x4LeU),
            Operator::I32x4GeS => Some(wasm_encoder::Instruction::I32x4GeS),
            Operator::I32x4GeU => Some(wasm_encoder::Instruction::I32x4GeU),

            Operator::I64x2Eq => Some(wasm_encoder::Instruction::I64x2Eq),
            Operator::I64x2Ne => Some(wasm_encoder::Instruction::I64x2Ne),
            Operator::I64x2LtS => Some(wasm_encoder::Instruction::I64x2LtS),
            Operator::I64x2GtS => Some(wasm_encoder::Instruction::I64x2GtS),
            Operator::I64x2LeS => Some(wasm_encoder::Instruction::I64x2LeS),
            Operator::I64x2GeS => Some(wasm_encoder::Instruction::I64x2GeS),

            Operator::F32x4Eq => Some(wasm_encoder::Instruction::F32x4Eq),
            Operator::F32x4Ne => Some(wasm_encoder::Instruction::F32x4Ne),
            Operator::F32x4Lt => Some(wasm_encoder::Instruction::F32x4Lt),
            Operator::F32x4Gt => Some(wasm_encoder::Instruction::F32x4Gt),
            Operator::F32x4Le => Some(wasm_encoder::Instruction::F32x4Le),
            Operator::F32x4Ge => Some(wasm_encoder::Instruction::F32x4Ge),

            Operator::F64x2Eq => Some(wasm_encoder::Instruction::F64x2Eq),
            Operator::F64x2Ne => Some(wasm_encoder::Instruction::F64x2Ne),
            Operator::F64x2Lt => Some(wasm_encoder::Instruction::F64x2Lt),
            Operator::F64x2Gt => Some(wasm_encoder::Instruction::F64x2Gt),
            Operator::F64x2Le => Some(wasm_encoder::Instruction::F64x2Le),
            Operator::F64x2Ge => Some(wasm_encoder::Instruction::F64x2Ge),

            Operator::V128Not => Some(wasm_encoder::Instruction::V128Not),
            Operator::V128And => Some(wasm_encoder::Instruction::V128And),
            Operator::V128AndNot => Some(wasm_encoder::Instruction::V128AndNot),
            Operator::V128Or => Some(wasm_encoder::Instruction::V128Or),
            Operator::V128Xor => Some(wasm_encoder::Instruction::V128Xor),
            Operator::V128Bitselect => Some(wasm_encoder::Instruction::V128Bitselect),
            Operator::V128AnyTrue => Some(wasm_encoder::Instruction::V128AnyTrue),

            Operator::I8x16Abs => Some(wasm_encoder::Instruction::I8x16Abs),
            Operator::I8x16Neg => Some(wasm_encoder::Instruction::I8x16Neg),
            Operator::I8x16Popcnt => Some(wasm_encoder::Instruction::I8x16Popcnt),
            Operator::I8x16AllTrue => Some(wasm_encoder::Instruction::I8x16AllTrue),
            Operator::I8x16Bitmask => Some(wasm_encoder::Instruction::I8x16Bitmask),
            Operator::I8x16NarrowI16x8S => Some(wasm_encoder::Instruction::I8x16NarrowI16x8S),
            Operator::I8x16NarrowI16x8U => Some(wasm_encoder::Instruction::I8x16NarrowI16x8U),
            Operator::I8x16Shl => Some(wasm_encoder::Instruction::I8x16Shl),
            Operator::I8x16ShrS => Some(wasm_encoder::Instruction::I8x16ShrS),
            Operator::I8x16ShrU => Some(wasm_encoder::Instruction::I8x16ShrU),
            Operator::I8x16Add => Some(wasm_encoder::Instruction::I8x16Add),
            Operator::I8x16AddSatS => Some(wasm_encoder::Instruction::I8x16AddSatS),
            Operator::I8x16AddSatU => Some(wasm_encoder::Instruction::I8x16AddSatU),
            Operator::I8x16Sub => Some(wasm_encoder::Instruction::I8x16Sub),
            Operator::I8x16SubSatS => Some(wasm_encoder::Instruction::I8x16SubSatS),
            Operator::I8x16SubSatU => Some(wasm_encoder::Instruction::I8x16SubSatU),
            Operator::I8x16MinS => Some(wasm_encoder::Instruction::I8x16MinS),
            Operator::I8x16MinU => Some(wasm_encoder::Instruction::I8x16MinU),
            Operator::I8x16MaxS => Some(wasm_encoder::Instruction::I8x16MaxS),
            Operator::I8x16MaxU => Some(wasm_encoder::Instruction::I8x16MaxU),
            Operator::I8x16AvgrU => Some(wasm_encoder::Instruction::I8x16AvgrU),

            Operator::I16x8ExtAddPairwiseI8x16S => {
                Some(wasm_encoder::Instruction::I16x8ExtAddPairwiseI8x16S)
            }
            Operator::I16x8ExtAddPairwiseI8x16U => {
                Some(wasm_encoder::Instruction::I16x8ExtAddPairwiseI8x16U)
            }
            Operator::I16x8Abs => Some(wasm_encoder::Instruction::I16x8Abs),
            Operator::I16x8Neg => Some(wasm_encoder::Instruction::I16x8Neg),
            Operator::I16x8Q15MulrSatS => Some(wasm_encoder::Instruction::I16x8Q15MulrSatS),
            Operator::I16x8AllTrue => Some(wasm_encoder::Instruction::I16x8AllTrue),
            Operator::I16x8Bitmask => Some(wasm_encoder::Instruction::I16x8Bitmask),
            Operator::I16x8NarrowI32x4S => Some(wasm_encoder::Instruction::I16x8NarrowI32x4S),
            Operator::I16x8NarrowI32x4U => Some(wasm_encoder::Instruction::I16x8NarrowI32x4U),
            Operator::I16x8ExtendLowI8x16S => Some(wasm_encoder::Instruction::I16x8ExtendLowI8x16S),
            Operator::I16x8ExtendHighI8x16S => {
                Some(wasm_encoder::Instruction::I16x8ExtendHighI8x16S)
            }
            Operator::I16x8ExtendLowI8x16U => Some(wasm_encoder::Instruction::I16x8ExtendLowI8x16U),
            Operator::I16x8ExtendHighI8x16U => {
                Some(wasm_encoder::Instruction::I16x8ExtendHighI8x16U)
            }
            Operator::I16x8Shl => Some(wasm_encoder::Instruction::I16x8Shl),
            Operator::I16x8ShrS => Some(wasm_encoder::Instruction::I16x8ShrS),
            Operator::I16x8ShrU => Some(wasm_encoder::Instruction::I16x8ShrU),
            Operator::I16x8Add => Some(wasm_encoder::Instruction::I16x8Add),
            Operator::I16x8AddSatS => Some(wasm_encoder::Instruction::I16x8AddSatS),
            Operator::I16x8AddSatU => Some(wasm_encoder::Instruction::I16x8AddSatU),
            Operator::I16x8Sub => Some(wasm_encoder::Instruction::I16x8Sub),
            Operator::I16x8SubSatS => Some(wasm_encoder::Instruction::I16x8SubSatS),
            Operator::I16x8SubSatU => Some(wasm_encoder::Instruction::I16x8SubSatU),
            Operator::I16x8Mul => Some(wasm_encoder::Instruction::I16x8Mul),
            Operator::I16x8MinS => Some(wasm_encoder::Instruction::I16x8MinS),
            Operator::I16x8MinU => Some(wasm_encoder::Instruction::I16x8MinU),
            Operator::I16x8MaxS => Some(wasm_encoder::Instruction::I16x8MaxS),
            Operator::I16x8MaxU => Some(wasm_encoder::Instruction::I16x8MaxU),
            Operator::I16x8AvgrU => Some(wasm_encoder::Instruction::I16x8AvgrU),
            Operator::I16x8ExtMulLowI8x16S => Some(wasm_encoder::Instruction::I16x8ExtMulLowI8x16S),
            Operator::I16x8ExtMulHighI8x16S => {
                Some(wasm_encoder::Instruction::I16x8ExtMulHighI8x16S)
            }
            Operator::I16x8ExtMulLowI8x16U => Some(wasm_encoder::Instruction::I16x8ExtMulLowI8x16U),
            Operator::I16x8ExtMulHighI8x16U => {
                Some(wasm_encoder::Instruction::I16x8ExtMulHighI8x16U)
            }

            Operator::I32x4ExtAddPairwiseI16x8S => {
                Some(wasm_encoder::Instruction::I32x4ExtAddPairwiseI16x8S)
            }
            Operator::I32x4ExtAddPairwiseI16x8U => {
                Some(wasm_encoder::Instruction::I32x4ExtAddPairwiseI16x8U)
            }
            Operator::I32x4Abs => Some(wasm_encoder::Instruction::I32x4Abs),
            Operator::I32x4Neg => Some(wasm_encoder::Instruction::I32x4Neg),
            Operator::I32x4AllTrue => Some(wasm_encoder::Instruction::I32x4AllTrue),
            Operator::I32x4Bitmask => Some(wasm_encoder::Instruction::I32x4Bitmask),
            Operator::I32x4ExtendLowI16x8S => Some(wasm_encoder::Instruction::I32x4ExtendLowI16x8S),
            Operator::I32x4ExtendHighI16x8S => {
                Some(wasm_encoder::Instruction::I32x4ExtendHighI16x8S)
            }
            Operator::I32x4ExtendLowI16x8U => Some(wasm_encoder::Instruction::I32x4ExtendLowI16x8U),
            Operator::I32x4ExtendHighI16x8U => {
                Some(wasm_encoder::Instruction::I32x4ExtendHighI16x8U)
            }
            Operator::I32x4Shl => Some(wasm_encoder::Instruction::I32x4Shl),
            Operator::I32x4ShrS => Some(wasm_encoder::Instruction::I32x4ShrS),
            Operator::I32x4ShrU => Some(wasm_encoder::Instruction::I32x4ShrU),
            Operator::I32x4Add => Some(wasm_encoder::Instruction::I32x4Add),
            Operator::I32x4Sub => Some(wasm_encoder::Instruction::I32x4Sub),
            Operator::I32x4Mul => Some(wasm_encoder::Instruction::I32x4Mul),
            Operator::I32x4MinS => Some(wasm_encoder::Instruction::I32x4MinS),
            Operator::I32x4MinU => Some(wasm_encoder::Instruction::I32x4MinU),
            Operator::I32x4MaxS => Some(wasm_encoder::Instruction::I32x4MaxS),
            Operator::I32x4MaxU => Some(wasm_encoder::Instruction::I32x4MaxU),
            Operator::I32x4DotI16x8S => Some(wasm_encoder::Instruction::I32x4DotI16x8S),
            Operator::I32x4ExtMulLowI16x8S => Some(wasm_encoder::Instruction::I32x4ExtMulLowI16x8S),
            Operator::I32x4ExtMulHighI16x8S => {
                Some(wasm_encoder::Instruction::I32x4ExtMulHighI16x8S)
            }
            Operator::I32x4ExtMulLowI16x8U => Some(wasm_encoder::Instruction::I32x4ExtMulLowI16x8U),
            Operator::I32x4ExtMulHighI16x8U => {
                Some(wasm_encoder::Instruction::I32x4ExtMulHighI16x8U)
            }

            Operator::I64x2Abs => Some(wasm_encoder::Instruction::I64x2Abs),
            Operator::I64x2Neg => Some(wasm_encoder::Instruction::I64x2Neg),
            Operator::I64x2AllTrue => Some(wasm_encoder::Instruction::I64x2AllTrue),
            Operator::I64x2Bitmask => Some(wasm_encoder::Instruction::I64x2Bitmask),
            Operator::I64x2ExtendLowI32x4S => Some(wasm_encoder::Instruction::I64x2ExtendLowI32x4S),
            Operator::I64x2ExtendHighI32x4S => {
                Some(wasm_encoder::Instruction::I64x2ExtendHighI32x4S)
            }
            Operator::I64x2ExtendLowI32x4U => Some(wasm_encoder::Instruction::I64x2ExtendLowI32x4U),
            Operator::I64x2ExtendHighI32x4U => {
                Some(wasm_encoder::Instruction::I64x2ExtendHighI32x4U)
            }
            Operator::I64x2Shl => Some(wasm_encoder::Instruction::I64x2Shl),
            Operator::I64x2ShrS => Some(wasm_encoder::Instruction::I64x2ShrS),
            Operator::I64x2ShrU => Some(wasm_encoder::Instruction::I64x2ShrU),
            Operator::I64x2Add => Some(wasm_encoder::Instruction::I64x2Add),
            Operator::I64x2Sub => Some(wasm_encoder::Instruction::I64x2Sub),
            Operator::I64x2Mul => Some(wasm_encoder::Instruction::I64x2Mul),
            Operator::I64x2ExtMulLowI32x4S => Some(wasm_encoder::Instruction::I64x2ExtMulLowI32x4S),
            Operator::I64x2ExtMulHighI32x4S => {
                Some(wasm_encoder::Instruction::I64x2ExtMulHighI32x4S)
            }
            Operator::I64x2ExtMulLowI32x4U => Some(wasm_encoder::Instruction::I64x2ExtMulLowI32x4U),
            Operator::I64x2ExtMulHighI32x4U => {
                Some(wasm_encoder::Instruction::I64x2ExtMulHighI32x4U)
            }

            Operator::F32x4Ceil => Some(wasm_encoder::Instruction::F32x4Ceil),
            Operator::F32x4Floor => Some(wasm_encoder::Instruction::F32x4Floor),
            Operator::F32x4Trunc => Some(wasm_encoder::Instruction::F32x4Trunc),
            Operator::F32x4Nearest => Some(wasm_encoder::Instruction::F32x4Nearest),
            Operator::F32x4Abs => Some(wasm_encoder::Instruction::F32x4Abs),
            Operator::F32x4Neg => Some(wasm_encoder::Instruction::F32x4Neg),
            Operator::F32x4Sqrt => Some(wasm_encoder::Instruction::F32x4Sqrt),
            Operator::F32x4Add => Some(wasm_encoder::Instruction::F32x4Add),
            Operator::F32x4Sub => Some(wasm_encoder::Instruction::F32x4Sub),
            Operator::F32x4Mul => Some(wasm_encoder::Instruction::F32x4Mul),
            Operator::F32x4Div => Some(wasm_encoder::Instruction::F32x4Div),
            Operator::F32x4Min => Some(wasm_encoder::Instruction::F32x4Min),
            Operator::F32x4Max => Some(wasm_encoder::Instruction::F32x4Max),
            Operator::F32x4PMin => Some(wasm_encoder::Instruction::F32x4PMin),
            Operator::F32x4PMax => Some(wasm_encoder::Instruction::F32x4PMax),

            Operator::F64x2Ceil => Some(wasm_encoder::Instruction::F64x2Ceil),
            Operator::F64x2Floor => Some(wasm_encoder::Instruction::F64x2Floor),
            Operator::F64x2Trunc => Some(wasm_encoder::Instruction::F64x2Trunc),
            Operator::F64x2Nearest => Some(wasm_encoder::Instruction::F64x2Nearest),
            Operator::F64x2Abs => Some(wasm_encoder::Instruction::F64x2Abs),
            Operator::F64x2Neg => Some(wasm_encoder::Instruction::F64x2Neg),
            Operator::F64x2Sqrt => Some(wasm_encoder::Instruction::F64x2Sqrt),
            Operator::F64x2Add => Some(wasm_encoder::Instruction::F64x2Add),
            Operator::F64x2Sub => Some(wasm_encoder::Instruction::F64x2Sub),
            Operator::F64x2Mul => Some(wasm_encoder::Instruction::F64x2Mul),
            Operator::F64x2Div => Some(wasm_encoder::Instruction::F64x2Div),
            Operator::F64x2Min => Some(wasm_encoder::Instruction::F64x2Min),
            Operator::F64x2Max => Some(wasm_encoder::Instruction::F64x2Max),
            Operator::F64x2PMin => Some(wasm_encoder::Instruction::F64x2PMin),
            Operator::F64x2PMax => Some(wasm_encoder::Instruction::F64x2PMax),

            Operator::I32x4TruncSatF32x4S => Some(wasm_encoder::Instruction::I32x4TruncSatF32x4S),
            Operator::I32x4TruncSatF32x4U => Some(wasm_encoder::Instruction::I32x4TruncSatF32x4U),

            Operator::F32x4ConvertI32x4S => Some(wasm_encoder::Instruction::F32x4ConvertI32x4S),
            Operator::F32x4ConvertI32x4U => Some(wasm_encoder::Instruction::F32x4ConvertI32x4U),
            Operator::I32x4TruncSatF64x2SZero => {
                Some(wasm_encoder::Instruction::I32x4TruncSatF64x2SZero)
            }
            Operator::I32x4TruncSatF64x2UZero => {
                Some(wasm_encoder::Instruction::I32x4TruncSatF64x2UZero)
            }
            Operator::F64x2ConvertLowI32x4S => {
                Some(wasm_encoder::Instruction::F64x2ConvertLowI32x4S)
            }
            Operator::F64x2ConvertLowI32x4U => {
                Some(wasm_encoder::Instruction::F64x2ConvertLowI32x4U)
            }
            Operator::F32x4DemoteF64x2Zero => Some(wasm_encoder::Instruction::F32x4DemoteF64x2Zero),
            Operator::F64x2PromoteLowF32x4 => Some(wasm_encoder::Instruction::F64x2PromoteLowF32x4),

            Operator::CallRef { sig_index } => {
                Some(wasm_encoder::Instruction::CallRef(sig_index.index() as u32))
            }
            Operator::RefIsNull => Some(wasm_encoder::Instruction::RefIsNull),
            Operator::RefNull { sig_index } => Some(wasm_encoder::Instruction::RefNull(
                wasm_encoder::HeapType::Concrete(sig_index.index() as u32),
            )),
            Operator::RefFunc { func_index } => {
                Some(wasm_encoder::Instruction::RefFunc(func_index.index() as u32))
            }
        };

        if let Some(inst) = inst {
            func.instruction(&inst);
        }
    }
}

pub fn compile(module: &Module<'_>) -> anyhow::Result<Vec<u8>> {
    let mut into_mod = wasm_encoder::Module::new();

    let mut types = wasm_encoder::TypeSection::new();
    for sig_data in module.signatures.values() {
        let params = sig_data
            .params
            .iter()
            .map(|&ty| wasm_encoder::ValType::from(ty));
        let returns = sig_data
            .returns
            .iter()
            .map(|&ty| wasm_encoder::ValType::from(ty));
        types.function(params, returns);
    }
    into_mod.section(&types);

    let mut imports = wasm_encoder::ImportSection::new();
    let mut num_func_imports = 0;
    let mut num_table_imports = 0;
    let mut num_global_imports = 0;
    let mut num_mem_imports = 0;
    for import in &module.imports {
        let entity = match &import.kind {
            &ImportKind::Func(func) => {
                num_func_imports += 1;
                let func = &module.funcs[func];
                wasm_encoder::EntityType::Function(func.sig().index() as u32)
            }
            &ImportKind::Table(table) => {
                num_table_imports += 1;
                let table = &module.tables[table];
                wasm_encoder::EntityType::Table(wasm_encoder::TableType {
                    element_type: wasm_encoder::RefType::from(table.ty),
                    minimum: table.initial,
                    maximum: table.max,
                    table64: false,
                })
            }
            &ImportKind::Global(global) => {
                num_global_imports += 1;
                let global = &module.globals[global];
                wasm_encoder::EntityType::Global(wasm_encoder::GlobalType {
                    val_type: wasm_encoder::ValType::from(global.ty),
                    mutable: global.mutable,
                    shared: false,
                })
            }
            &ImportKind::Memory(mem) => {
                num_mem_imports += 1;
                let mem = &module.memories[mem];
                wasm_encoder::EntityType::Memory(wasm_encoder::MemoryType {
                    memory64: false,
                    shared: false,
                    minimum: mem.initial_pages as u64,
                    maximum: mem.maximum_pages.map(|val| val as u64),
                    page_size_log2: None,
                })
            }
        };
        imports.import(&import.module[..], &import.name[..], entity);
    }

    into_mod.section(&imports);

    let mut funcs = wasm_encoder::FunctionSection::new();
    for (func, func_decl) in module.funcs.entries().skip(num_func_imports) {
        match func_decl {
            FuncDecl::Import(_, _) => anyhow::bail!("Import comes after func with body: {}", func),
            FuncDecl::Lazy(sig, _, _)
            | FuncDecl::Body(sig, _, _)
            | FuncDecl::Compiled(sig, _, _) => {
                funcs.function(sig.index() as u32);
            }
            FuncDecl::None => panic!("FuncDecl::None at compilation time"),
        }
    }
    into_mod.section(&funcs);

    let mut tables = wasm_encoder::TableSection::new();
    for table_data in module.tables.values().skip(num_table_imports) {
        tables.table(wasm_encoder::TableType {
            element_type: wasm_encoder::RefType::from(table_data.ty),
            minimum: table_data.initial,
            maximum: table_data.max,
            table64: false,
        });
    }
    into_mod.section(&tables);

    let mut memories = wasm_encoder::MemorySection::new();
    for mem_data in module.memories.values().skip(num_mem_imports) {
        memories.memory(wasm_encoder::MemoryType {
            minimum: mem_data.initial_pages as u64,
            maximum: mem_data.maximum_pages.map(|val| val as u64),
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
    }
    into_mod.section(&memories);

    let mut globals = wasm_encoder::GlobalSection::new();
    for global_data in module.globals.values().skip(num_global_imports) {
        globals.global(
            wasm_encoder::GlobalType {
                val_type: wasm_encoder::ValType::from(global_data.ty),
                mutable: global_data.mutable,
                shared: false,
            },
            &const_init(global_data.ty, global_data.value),
        );
    }
    into_mod.section(&globals);

    let mut exports = wasm_encoder::ExportSection::new();
    for export in &module.exports {
        match &export.kind {
            &ExportKind::Table(table) => {
                exports.export(
                    &export.name[..],
                    wasm_encoder::ExportKind::Table,
                    table.index() as u32,
                );
            }
            &ExportKind::Func(func) => {
                exports.export(
                    &export.name[..],
                    wasm_encoder::ExportKind::Func,
                    func.index() as u32,
                );
            }
            &ExportKind::Memory(mem) => {
                exports.export(
                    &export.name[..],
                    wasm_encoder::ExportKind::Memory,
                    mem.index() as u32,
                );
            }
            &ExportKind::Global(global) => {
                exports.export(
                    &export.name[..],
                    wasm_encoder::ExportKind::Global,
                    global.index() as u32,
                );
            }
        }
    }
    into_mod.section(&exports);

    if let Some(start) = module.start_func {
        let start = wasm_encoder::StartSection {
            function_index: start.index() as u32,
        };
        into_mod.section(&start);
    }

    let mut elem = wasm_encoder::ElementSection::new();
    for (table, table_data) in module.tables.entries() {
        if let Some(elts) = &table_data.func_elements {
            for (i, &elt) in elts.iter().enumerate() {
                if elt.is_valid() {
                    match table_data.ty {
                        Type::FuncRef => {
                            elem.active(
                                Some(table.index() as u32),
                                &wasm_encoder::ConstExpr::i32_const(i as i32),
                                wasm_encoder::Elements::Functions(&[elt.index() as u32]),
                            );
                        }
                        Type::TypedFuncRef(..) => {
                            elem.active(
                                Some(table.index() as u32),
                                &wasm_encoder::ConstExpr::i32_const(i as i32),
                                wasm_encoder::Elements::Expressions(
                                    table_data.ty.into(),
                                    &[wasm_encoder::ConstExpr::ref_func(elt.index() as u32)],
                                ),
                            );
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
    into_mod.section(&elem);

    let mut code = wasm_encoder::CodeSection::new();

    let bodies = module
        .funcs
        .entries()
        .skip(num_func_imports)
        .collect::<Vec<_>>()
        .par_iter()
        .map(|(func, func_decl)| -> Result<_> {
            match func_decl {
                FuncDecl::Lazy(_, _name, reader) => {
                    let data = &module.orig_bytes.unwrap()[reader.range()];
                    Ok(Cow::Borrowed(data))
                }
                FuncDecl::Compiled(_, _name, bytes) => Ok(Cow::Borrowed(&bytes[..])),
                FuncDecl::Body(_, name, body) => {
                    log::debug!("Compiling {} \"{}\"", func, name);
                    WasmFuncBackend::compile(body).map(|func| Cow::Owned(func.into_raw_body()))
                }
                FuncDecl::Import(_, _) => unreachable!("Should have skipped imports"),
                FuncDecl::None => panic!("FuncDecl::None at compilation time"),
            }
        })
        .collect::<Result<Vec<_>>>()?;

    for body in bodies {
        code.raw(&body);
    }
    into_mod.section(&code);

    let mut data = wasm_encoder::DataSection::new();
    for (mem, mem_data) in module.memories.entries() {
        for segment in &mem_data.segments {
            data.active(
                mem.index() as u32,
                &wasm_encoder::ConstExpr::i32_const(segment.offset as i32),
                segment.data.iter().copied(),
            );
        }
    }
    into_mod.section(&data);

    let mut names = wasm_encoder::NameSection::new();
    let mut func_names = wasm_encoder::NameMap::new();
    for (func, decl) in module.funcs.entries() {
        func_names.append(func.index() as u32, decl.name());
    }
    names.functions(&func_names);
    into_mod.section(&names);

    for (custom_name, &custom_data) in &module.custom_sections {
        let section = wasm_encoder::CustomSection {
            name: custom_name.into(),
            data: custom_data.into(),
        };
        into_mod.section(&section);
    }

    Ok(into_mod.finish())
}

fn const_init(ty: Type, value: Option<u64>) -> wasm_encoder::ConstExpr {
    let bits = value.unwrap_or(0);
    match ty {
        Type::I32 => wasm_encoder::ConstExpr::i32_const(bits as u32 as i32),
        Type::I64 => wasm_encoder::ConstExpr::i64_const(bits as i64),
        Type::F32 => wasm_encoder::ConstExpr::f32_const(f32::from_bits(bits as u32)),
        Type::F64 => wasm_encoder::ConstExpr::f64_const(f64::from_bits(bits as u64)),
        Type::TypedFuncRef(true, sig) if bits == 0 => {
            let hty = wasm_encoder::HeapType::Concrete(sig);
            wasm_encoder::ConstExpr::ref_null(hty)
        }
        _ => unimplemented!(),
    }
}
