//! Conversion pass that creates "maximal SSA": only local uses (no
//! uses of defs in other blocks), with all values explicitly passed
//! through blockparams. This makes some other transforms easier
//! because it removes the need to worry about adding blockparams when
//! mutating the CFG (all possible blockparams are already there!).

use crate::cfg::CFGInfo;
use crate::entity::PerEntity;
use crate::ir::{Block, FunctionBody, Value, ValueDef};
use fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::collections::BTreeSet;

pub(crate) fn run(body: &mut FunctionBody, cut_blocks: Option<HashSet<Block>>, cfg: &CFGInfo) {
    MaxSSAPass::new(cut_blocks).run(body, cfg);
}

struct MaxSSAPass {
    /// Blocks at which all live values must cross through blockparams
    /// (or if None, then all blocks).
    cut_blocks: Option<HashSet<Block>>,
    /// Additional block args that must be passed to each block, in
    /// order. Value numbers are *original* values.
    new_args: PerEntity<Block, Vec<Value>>,
    /// For each block, a value map: from original value to local copy
    /// of value.
    value_map: HashMap<(Block, Value), Value>,
}

impl MaxSSAPass {
    fn new(cut_blocks: Option<HashSet<Block>>) -> Self {
        Self {
            cut_blocks,
            new_args: PerEntity::default(),
            value_map: HashMap::default(),
        }
    }

    fn run(mut self, body: &mut FunctionBody, cfg: &CFGInfo) {
        for block in body.blocks.iter() {
            self.visit(body, cfg, block);
        }
        self.update(body);
    }

    fn visit(&mut self, body: &mut FunctionBody, cfg: &CFGInfo, block: Block) {
        // For each use in the block, process the use. Collect all
        // uses first to deduplicate and allow more efficient
        // processing (and to appease the borrow checker).
        let mut uses = BTreeSet::default();
        for &inst in &body.blocks[block].insts {
            match &body.values[inst] {
                &ValueDef::Operator(_, args, _) => {
                    for &arg in &body.arg_pool[args] {
                        let arg = body.resolve_alias(arg);
                        uses.insert(arg);
                    }
                }
                &ValueDef::PickOutput(value, ..) => {
                    let value = body.resolve_alias(value);
                    uses.insert(value);
                }
                _ => {}
            }
        }
        body.blocks[block].terminator.visit_uses(|u| {
            let u = body.resolve_alias(u);
            uses.insert(u);
        });

        for u in uses {
            self.visit_use(body, cfg, block, u);
        }
    }

    fn visit_use(&mut self, body: &mut FunctionBody, cfg: &CFGInfo, block: Block, value: Value) {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Phase {
            Pre,
            Post,
        }

        let mut stack = vec![(block, value, Phase::Pre, None)];

        while let Some(&(block, value, phase, _)) = stack.last() {
            match phase {
                Phase::Pre => {
                    if self.value_map.contains_key(&(block, value)) {
                        stack.pop();
                        continue;
                    }
                    if cfg.def_block[value] == block {
                        stack.pop();
                        continue;
                    }

                    self.new_args[block].push(value);

                    // Create a placeholder value.
                    let ty = body.values[value].ty(&body.type_pool).unwrap();
                    let blockparam = body.add_blockparam(block, ty);
                    self.value_map.insert((block, value), blockparam);

                    stack.pop();
                    stack.push((block, value, Phase::Post, Some(blockparam)));

                    // Recursively visit preds and use the value there, to
                    // ensure they have the value available as well.
                    for i in 0..body.blocks[block].preds.len() {
                        // Don't borrow for whole loop while iterating (`body` is
                        // taken as mut by recursion, but we don't add preds).
                        let pred = body.blocks[block].preds[i];
                        stack.push((pred, value, Phase::Pre, None));
                    }
                }

                Phase::Post => {
                    let Some((_, _, _, Some(blockparam))) = stack.pop() else {
                        unreachable!()
                    };

                    // If all preds have the same value, and this is not a
                    // cut-block, rewrite the blockparam to an alias instead.
                    if !self.is_cut_block(block) {
                        if let Some(pred_value) = iter_all_same(
                            body.blocks[block]
                                .preds
                                .iter()
                                .map(|&pred| *self.value_map.get(&(pred, value)).unwrap_or(&value))
                                .filter(|&val| val != blockparam),
                        ) {
                            body.blocks[block].params.pop();
                            self.new_args[block].pop();
                            body.values[blockparam] = ValueDef::Alias(pred_value);
                            self.value_map.insert((block, value), pred_value);
                        }
                    }
                }
            }
        }
    }

    fn is_cut_block(&self, block: Block) -> bool {
        self.cut_blocks
            .as_ref()
            .map(|cut_blocks| cut_blocks.contains(&block))
            .unwrap_or(true)
    }

    fn update_branch_args(&mut self, body: &mut FunctionBody) {
        for (block, blockdata) in body.blocks.entries_mut() {
            blockdata.terminator.update_targets(|target| {
                for &new_arg in &self.new_args[target.block] {
                    let actual_value = self
                        .value_map
                        .get(&(block, new_arg))
                        .copied()
                        .unwrap_or(new_arg);
                    target.args.push(actual_value);
                }
            });
        }
    }

    fn update_uses(&mut self, body: &mut FunctionBody, block: Block) {
        let resolve = |body: &FunctionBody, value: Value| {
            let value = body.resolve_alias(value);
            self.value_map
                .get(&(block, value))
                .copied()
                .unwrap_or(value)
        };

        for i in 0..body.blocks[block].insts.len() {
            let inst = body.blocks[block].insts[i];
            let mut def = std::mem::take(&mut body.values[inst]);
            match &mut def {
                ValueDef::Operator(_, args, _) => {
                    for i in 0..args.len() {
                        let val = body.arg_pool[*args][i];
                        let val = resolve(body, val);
                        body.arg_pool[*args][i] = val;
                    }
                }
                ValueDef::PickOutput(value, ..) => {
                    *value = resolve(body, *value);
                }
                ValueDef::Alias(value) => {
                    *value = resolve(body, *value);
                }
                _ => {}
            }
            body.values[inst] = def;
        }
        let mut term = std::mem::take(&mut body.blocks[block].terminator);
        term.update_uses(|u| {
            *u = resolve(body, *u);
        });
        body.blocks[block].terminator = term;
    }

    fn update(&mut self, body: &mut FunctionBody) {
        self.update_branch_args(body);
        for block in body.blocks.iter() {
            self.update_uses(body, block);
        }
    }
}

fn iter_all_same<Item: PartialEq + Eq + Copy, I: Iterator<Item = Item>>(iter: I) -> Option<Item> {
    let mut item = None;
    for val in iter {
        if *item.get_or_insert(val) != val {
            return None;
        }
    }
    item
}
