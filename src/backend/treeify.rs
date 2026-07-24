//! Treeification: placing some values "under" others if only used
//! once, to generate more AST-like Wasm code.

use crate::entity::EntityRef;
use crate::ir::{FunctionBody, Value, ValueDef};
use crate::Operator;
use fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::convert::TryFrom;

/// One "argument slot" of an operator defining a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueArg(Value, u16);

#[derive(Clone, Debug)]
pub struct Trees {
    /// Is a value placed "under" the given arg slot of the given
    /// other value?
    pub owner: HashMap<Value, ValueArg>,
    /// For a given value that is defined by an operator, which
    /// Values, if any, live at each slot?
    pub owned: HashMap<ValueArg, Value>,
    /// Values that are regenerated every time they are used.
    pub remat: HashSet<Value>,
}

fn is_remat(op: &Operator) -> bool {
    // Only ops with no args can be always-rematerialized.
    match op {
        Operator::I32Const { .. }
        | Operator::I64Const { .. }
        | Operator::F32Const { .. }
        | Operator::F64Const { .. } => true,
        _ => false,
    }
}

impl Trees {
    pub fn compute(body: &FunctionBody) -> Trees {
        let mut owner = HashMap::default();
        let mut owned = HashMap::default();
        let mut remat = HashSet::default();
        let mut multi_use = HashSet::default();

        for block_def in body.blocks.values() {
            for &value in &block_def.insts {
                match &body.values[value] {
                    &ValueDef::Operator(op, args, _) => {
                        // Ignore operators with invalid args: these must
                        // always be unreachable.
                        if body.arg_pool[args].iter().any(|arg| arg.is_invalid()) {
                            continue;
                        }
                        // If this is an always-rematerialized operator,
                        // mark it as such and continue.
                        if is_remat(&op) {
                            remat.insert(value);
                            continue;
                        }

                        // For each of the args, if the value is produced
                        // by a single-output op and is movable, and is
                        // not already recorded in `multi_use`, place it
                        // in the arg slot. Otherwise if owned already
                        // somewhere else, undo that and put in
                        // `multi_use`.
                        for (i, &arg) in body.arg_pool[args].iter().enumerate() {
                            let arg = body.resolve_alias(arg);
                            if multi_use.contains(&arg) {
                                continue;
                            } else if let Some(old_owner) = owner.remove(&arg) {
                                owned.remove(&old_owner);
                                multi_use.insert(arg);
                            } else if Self::is_movable(body, arg) {
                                let pos = u16::try_from(i).unwrap();
                                let value_arg = ValueArg(value, pos);
                                owner.insert(arg, value_arg);
                                owned.insert(value_arg, arg);
                            }
                        }
                    }
                    &ValueDef::PickOutput(..) => {
                        // Can ignore use: multi-arity values are never treeified.
                    }
                    &ValueDef::BlockParam(..)
                    | &ValueDef::Alias(..)
                    | &ValueDef::Placeholder(..)
                    | &ValueDef::None => {}
                }
            }
        }
        for block in body.blocks.values() {
            block.terminator.visit_uses(|u| {
                let u = body.resolve_alias(u);
                if let Some(old_owner) = owner.remove(&u) {
                    owned.remove(&old_owner);
                }
            });
        }

        Trees {
            owner,
            owned,
            remat,
        }
    }

    fn is_single_output_op(body: &FunctionBody, value: Value) -> Option<Operator> {
        match &body.values[value] {
            &ValueDef::Operator(op, _, ref tys) if tys.len() == 1 => Some(op),
            _ => None,
        }
    }

    fn is_movable(body: &FunctionBody, value: Value) -> bool {
        Self::is_single_output_op(body, value)
            .map(|op| op.is_pure())
            .unwrap_or(false)
    }
}
