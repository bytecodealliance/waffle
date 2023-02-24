use super::{Block, Type, Value};
use crate::Operator;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ValueDef {
    BlockParam(Block, usize, Type),
    Operator(Operator, Vec<Value>, Vec<Type>),
    PickOutput(Value, usize, Type),
    Alias(Value),
    Placeholder(Type),
    Trace(usize, Vec<Value>),
    #[default]
    None,
}

impl ValueDef {
    pub fn ty(&self) -> Option<Type> {
        match self {
            &ValueDef::BlockParam(_, _, ty) => Some(ty),
            &ValueDef::Operator(_, _, ref tys) if tys.len() == 0 => None,
            &ValueDef::Operator(_, _, ref tys) if tys.len() == 1 => Some(tys[0]),
            &ValueDef::PickOutput(_, _, ty) => Some(ty),
            &ValueDef::Placeholder(ty) => Some(ty),
            &ValueDef::Trace(_, _) => None,
            _ => None,
        }
    }

    pub fn tys(&self) -> &[Type] {
        match self {
            &ValueDef::Operator(_, _, ref tys) => &tys[..],
            &ValueDef::BlockParam(_, _, ref ty)
            | &ValueDef::PickOutput(_, _, ref ty)
            | &ValueDef::Placeholder(ref ty) => std::slice::from_ref(ty),
            _ => &[],
        }
    }

    pub fn visit_uses<F: FnMut(Value)>(&self, mut f: F) {
        match self {
            &ValueDef::BlockParam { .. } => {}
            &ValueDef::Operator(_, ref args, _) => {
                for &arg in args {
                    f(arg);
                }
            }
            &ValueDef::PickOutput(from, ..) => f(from),
            &ValueDef::Alias(value) => f(value),
            &ValueDef::Placeholder(_) => {}
            &ValueDef::Trace(_, ref args) => {
                for &arg in args {
                    f(arg);
                }
            }
            &ValueDef::None => panic!(),
        }
    }

    pub fn update_uses<F: FnMut(&mut Value)>(&mut self, mut f: F) {
        match self {
            &mut ValueDef::BlockParam { .. } => {}
            &mut ValueDef::Operator(_, ref mut args, _) => {
                for arg in args {
                    f(arg);
                }
            }
            &mut ValueDef::PickOutput(ref mut from, ..) => f(from),
            &mut ValueDef::Alias(ref mut value) => f(value),
            &mut ValueDef::Placeholder(_) => {}
            &mut ValueDef::Trace(_, ref mut args) => {
                for arg in args {
                    f(arg);
                }
            }
            &mut ValueDef::None => panic!(),
        }
    }
}
