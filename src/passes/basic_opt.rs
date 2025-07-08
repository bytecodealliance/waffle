//! Basic optimizations: GVN and constant-propagation/folding.

use crate::Operator;
use crate::cfg::CFGInfo;
use crate::interp::{ConstVal, const_eval};
use crate::ir::*;
use crate::passes::dom_pass::{DomtreePass, dom_pass};
use crate::pool::ListRef;
use crate::scoped_map::ScopedMap;
use smallvec::{SmallVec, smallvec};

#[derive(Clone, Debug)]
pub struct OptOptions {
    pub gvn: bool,
    pub cprop: bool,
    pub redundant_blockparams: bool,
}

impl std::default::Default for OptOptions {
    fn default() -> Self {
        OptOptions {
            gvn: true,
            cprop: true,
            redundant_blockparams: true,
        }
    }
}

pub(crate) fn basic_opt(body: &mut FunctionBody, cfg: &CFGInfo, options: &OptOptions) {
    loop {
        let mut pass = BasicOptPass {
            map: ScopedMap::default(),
            cfg,
            options,
            changed: false,
        };
        dom_pass::<BasicOptPass>(body, cfg, &mut pass);
        if !pass.changed {
            break;
        }
    }
}

#[derive(Debug)]
struct BasicOptPass<'a> {
    map: ScopedMap<(Operator, Vec<Value>, Vec<Type>), Value>,
    cfg: &'a CFGInfo,
    options: &'a OptOptions,
    changed: bool,
}

impl<'a> DomtreePass for BasicOptPass<'a> {
    fn enter(&mut self, block: Block, body: &mut FunctionBody) {
        log::trace!("pushing map level");
        self.map.push_level();
        self.optimize(block, body);
    }

    fn leave(&mut self, _block: Block, _body: &mut FunctionBody) {
        log::trace!("popping map level");
        self.map.pop_level();
    }
}

fn value_is_pure(value: Value, body: &FunctionBody) -> bool {
    match body.values[value] {
        ValueDef::Operator(op, ..) if op.is_pure() => true,
        _ => false,
    }
}

fn value_is_const(value: Value, body: &FunctionBody) -> ConstVal {
    match body.values[value] {
        ValueDef::Operator(Operator::I32Const { value }, _, _) => ConstVal::I32(value),
        ValueDef::Operator(Operator::I64Const { value }, _, _) => ConstVal::I64(value),
        ValueDef::Operator(Operator::F32Const { value }, _, _) => ConstVal::F32(value),
        ValueDef::Operator(Operator::F64Const { value }, _, _) => ConstVal::F64(value),
        _ => ConstVal::None,
    }
}

fn const_op(val: ConstVal) -> Operator {
    match val {
        ConstVal::I32(value) => Operator::I32Const { value },
        ConstVal::I64(value) => Operator::I64Const { value },
        ConstVal::F32(value) => Operator::F32Const { value },
        ConstVal::F64(value) => Operator::F64Const { value },
        _ => unreachable!(),
    }
}

fn remove_all_from_vec<T: Clone>(v: &mut Vec<T>, indices: &[usize]) {
    let mut out = 0;
    let mut indices_i = 0;
    for i in 0..v.len() {
        let keep = indices_i == indices.len() || indices[indices_i] != i;
        if keep {
            if out < i {
                v[out] = v[i].clone();
            }
            out += 1;
        } else {
            indices_i += 1;
        }
    }

    v.truncate(out);
}

impl<'a> BasicOptPass<'a> {
    fn optimize(&mut self, block: Block, body: &mut FunctionBody) {
        log::trace!("Basic opts: visiting block {block}");
        if self.options.redundant_blockparams && block != body.entry {
            // Pass over blockparams, checking all inputs. If all inputs
            // resolve to the same SSA value, remove the blockparam and
            // make it an alias of that value. If all inputs resolve to
            // the same constant value, remove the blockparam and insert a
            // new copy of that constant.
            let mut blockparams_to_remove: SmallVec<[usize; 4]> = smallvec![];
            let mut const_insts_to_insert: SmallVec<[Value; 4]> = smallvec![];
            for (i, &(ty, blockparam)) in body.blocks[block].params.iter().enumerate() {
                let mut inputs: SmallVec<[Value; 4]> = smallvec![];
                let mut const_val = None;
                for (&pred, &pos) in self.cfg.preds[block]
                    .iter()
                    .zip(self.cfg.pred_pos[block].iter())
                {
                    let input = body.blocks[pred]
                        .terminator
                        .visit_target(pos, |target| target.args[i]);
                    let input = body.resolve_alias(input);
                    if input != blockparam {
                        inputs.push(input);
                    }
                    const_val = ConstVal::meet(const_val, Some(value_is_const(input, body)));
                }
                let const_val = const_val.unwrap();

                log::trace!("blockparam {blockparam}: inputs {inputs:?}");
                assert!(inputs.len() > 0);
                if inputs.iter().all(|x| *x == inputs[0]) {
                    // All inputs are the same value; remove the
                    // blockparam and rewrite it as an alias of the one
                    // single value.
                    log::trace!(" -> all inputs are {}", inputs[0]);
                    body.values[blockparam] = ValueDef::Alias(inputs[0]);
                    blockparams_to_remove.push(i);
                } else if const_val != ConstVal::None {
                    // All inputs are the same constant; remove the
                    // blockparam and rewrite it as a new constant
                    // operator.
                    let ty = body.type_pool.single(ty);
                    body.values[blockparam] =
                        ValueDef::Operator(const_op(const_val), ListRef::default(), ty);
                    const_insts_to_insert.push(blockparam);
                    blockparams_to_remove.push(i);
                }
            }

            if !const_insts_to_insert.is_empty() || !blockparams_to_remove.is_empty() {
                self.changed = true;
            }

            for inst in const_insts_to_insert {
                body.blocks[block].insts.insert(0, inst);
            }

            remove_all_from_vec(&mut body.blocks[block].params, &blockparams_to_remove[..]);
            for (&pred, &pos) in self.cfg.preds[block]
                .iter()
                .zip(self.cfg.pred_pos[block].iter())
            {
                body.blocks[pred].terminator.update_target(pos, |target| {
                    remove_all_from_vec(&mut target.args, &blockparams_to_remove[..])
                });
            }
        }

        // Pass over instructions, updating in place.
        let mut i = 0;
        while i < body.blocks[block].insts.len() {
            let inst = body.blocks[block].insts[i];
            i += 1;

            // Resolve aliases in the arg lists.
            match body.values[inst].clone() {
                ValueDef::Operator(_, args, _) => {
                    for i in 0..args.len() {
                        let val = body.arg_pool[args][i];
                        let new_val = body.resolve_and_update_alias(val);
                        body.arg_pool[args][i] = new_val;
                        self.changed |= new_val != val;
                    }
                }
                ValueDef::PickOutput(val, idx, ty) => {
                    let updated = body.resolve_and_update_alias(val);
                    body.values[inst] = ValueDef::PickOutput(updated, idx, ty);
                    self.changed |= updated != val;
                }
                _ => {}
            }

            if value_is_pure(inst, body) {
                let mut value = body.values[inst].clone();
                log::trace!("optimizing {inst} ({value:?})");

                // Try to constant-propagate.
                if self.options.cprop {
                    if let ValueDef::Operator(op, args, ..) = &value {
                        let arg_values = body.arg_pool[*args]
                            .iter()
                            .map(|&arg| value_is_const(arg, body))
                            .collect::<Vec<_>>();
                        let const_val = match op {
                            Operator::I32Const { .. }
                            | Operator::I64Const { .. }
                            | Operator::F32Const { .. }
                            | Operator::F64Const { .. }
                            | Operator::V128Const { .. } => None,
                            _ => const_eval(op, &arg_values[..], None),
                        };
                        match const_val {
                            Some(ConstVal::I32(val)) => {
                                value = ValueDef::Operator(
                                    Operator::I32Const { value: val },
                                    ListRef::default(),
                                    body.single_type_list(Type::I32),
                                );
                                body.values[inst] = value.clone();
                                self.changed = true;
                            }
                            Some(ConstVal::I64(val)) => {
                                value = ValueDef::Operator(
                                    Operator::I64Const { value: val },
                                    ListRef::default(),
                                    body.single_type_list(Type::I64),
                                );
                                body.values[inst] = value.clone();
                                self.changed = true;
                            }
                            Some(ConstVal::F32(val)) => {
                                value = ValueDef::Operator(
                                    Operator::F32Const { value: val },
                                    ListRef::default(),
                                    body.single_type_list(Type::F32),
                                );
                                body.values[inst] = value.clone();
                                self.changed = true;
                            }
                            Some(ConstVal::F64(val)) => {
                                value = ValueDef::Operator(
                                    Operator::F64Const { value: val },
                                    ListRef::default(),
                                    body.single_type_list(Type::F64),
                                );
                                body.values[inst] = value.clone();
                                self.changed = true;
                            }
                            _ => {}
                        }
                    }
                }

                if self.options.gvn {
                    // GVN: look for already-existing copies of this
                    // value.
                    log::trace!("looking up {value:?} in GVN map");
                    if let ValueDef::Operator(op, args, ty) = &value {
                        let gvn_tuple = (
                            *op,
                            body.arg_pool[*args].to_vec(),
                            body.type_pool[*ty].to_vec(),
                        );
                        if let Some(orig) = self.map.get(&gvn_tuple) {
                            log::trace!("GVN: value {inst} ({gvn_tuple:?}) replaced by {orig}");
                            body.set_alias(inst, *orig);
                            i -= 1;
                            body.blocks[block].insts.remove(i);
                            self.changed = true;
                            continue;
                        }
                        log::trace!("inserting {gvn_tuple:?} -> {inst} into GVN map");
                        self.map.insert(gvn_tuple, inst);
                    }
                }
            }
        }

        // Resolve aliases in the terminator.
        let mut terminator = std::mem::take(&mut body.blocks[block].terminator);
        terminator.update_uses(|u| {
            *u = body.resolve_alias(*u);
        });
        body.blocks[block].terminator = terminator;
    }
}
