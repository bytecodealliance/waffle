//! Resolve all aliases.

use crate::{FunctionBody, ValueDef};

pub fn run(body: &mut FunctionBody) {
    log::debug!(
        "Resolve aliases: running on:\n{}\n",
        body.display_verbose("| ", None),
    );
    for value in body.values.iter() {
        let mut value_def = std::mem::take(&mut body.values[value]);
        match &mut value_def {
            ValueDef::Operator(_, args, _) => {
                for i in 0..args.len() {
                    let val = body.arg_pool[*args][i];
                    let val = body.resolve_and_update_alias(val);
                    body.arg_pool[*args][i] = val;
                }
            }
            ValueDef::PickOutput(val, _idx, _ty) => {
                *val = body.resolve_alias(*val);
            }
            ValueDef::Alias(val) => {
                *val = body.resolve_alias(*val);
            }
            _ => {}
        }
        body.values[value] = value_def;
    }
    let mut blocks = std::mem::take(&mut body.blocks);
    for block in blocks.values_mut() {
        block.terminator.update_targets(|target| {
            for arg in &mut target.args {
                *arg = body.resolve_alias(*arg);
            }
        });
    }
    body.blocks = blocks;
}
