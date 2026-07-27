//! Cross-block value id management for localification.
//!
//! Only SSA values that have a use in a different block from where they
//! are defined can appear in live-in/live-out sets. Those "cross-block"
//! values are assigned compact dense indices (`CrossBlockId`) so that
//! inter-block liveness bitsets and dense sets operate over a much
//! smaller space than the full `Value` space.

use std::num::NonZeroU32;
use std::ops::Index;

use crate::backend::treeify::Trees;
use crate::entity::EntityRef;
use crate::ir::{FunctionBody, Value, ValueDef};

/// A dense index into the compact set of cross-block SSA values.
///
/// Only values with a use outside their defining block get a
/// `CrossBlockId`; purely block-local values do not participate in
/// inter-block liveness and are represented by the absence of a value
/// in the two-way map.
///
/// The inner `NonZeroU32` stores the external 0-based ID plus one,
/// so that `Option<CrossBlockId>` benefits from Rust's niche
/// optimization and packs into a single `u32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrossBlockId(NonZeroU32);

impl CrossBlockId {
    /// Construct a `CrossBlockId` from a raw u32. Used when the id
    /// comes from iterating over a bitset or dense array.
    pub fn new(n: u32) -> Self {
        Self(NonZeroU32::new(n.checked_add(1).unwrap()).unwrap())
    }

    /// The inner numeric value, used for indexing into bitsets and
    /// dense arrays.
    pub fn as_u32(self) -> u32 {
        self.0.get() - 1
    }
}

/// Two-way map between `Value` (SSA value) and `CrossBlockId` (compact index).
///
/// Tracks which values are "cross-block" (used in a different block from
/// where they are defined). Only cross-block values can appear in
/// live-in/live-out sets.
#[derive(Clone, Debug, Default)]
pub struct CrossBlockValues {
    /// Compact id per value (`None` = block-local). Indexed by `Value` index.
    ids: Vec<Option<CrossBlockId>>,
    /// Compact id -> Value. Indexed by `CrossBlockId.0`.
    values: Vec<Value>,
}

impl CrossBlockValues {
    /// The number of cross-block values registered.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether no values have been registered yet.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Pre-allocate the `ids` array for the given number of total SSA values.
    pub fn init(&mut self, total_values: usize) {
        self.ids = vec![None; total_values];
    }

    /// Look up the cross-block id for a value, if it is cross-block.
    pub fn get(&self, value: Value) -> Option<CrossBlockId> {
        self.ids[value.index()]
    }

    /// Look up the value for a cross-block id.
    pub fn value(&self, id: CrossBlockId) -> Value {
        self.values[id.0.get() as usize - 1]
    }

    /// Register a value as cross-block, assigning it a new compact id.
    /// Returns the assigned id. If already registered, returns the existing id.
    pub fn insert(&mut self, value: Value) -> CrossBlockId {
        if let Some(id) = self.ids[value.index()] {
            return id;
        }
        let id = CrossBlockId::new(self.values.len() as u32);
        self.ids[value.index()] = Some(id);
        self.values.push(value);
        id
    }

    /// Check whether a value has been registered as cross-block.
    pub fn contains(&self, value: Value) -> bool {
        self.ids[value.index()].is_some()
    }

    pub fn ids_mut(&mut self) -> &mut Vec<Option<CrossBlockId>> {
        &mut self.ids
    }

    pub fn ids(&self) -> &Vec<Option<CrossBlockId>> {
        &self.ids
    }

    pub fn iter_values(&self) -> impl Iterator<Item = Value> + '_ {
        self.values.iter().copied()
    }

    /// Build the cross-block value map by scanning all uses in the
    /// function body.
    ///
    /// Only values that have a (proper) use outside the block where
    /// their def is visited are recorded.  A def the block walk skips
    /// (treeified or remat insts) never cancels its uses, so such
    /// values stay tracked everywhere, matching the walk's behavior.
    pub fn build(body: &FunctionBody, trees: &Trees) -> Self {
        // 1. Compute def_block for every value.
        let mut def_block = vec![u32::MAX; body.values.len()];
        for (block, block_def) in body.blocks.entries() {
            for &(_, param) in &block_def.params {
                def_block[param.index()] = block.index() as u32;
            }
            for &inst in &block_def.insts {
                if is_tree_or_remat(inst, trees) {
                    continue;
                }
                def_block[inst.index()] = block.index() as u32;
            }
        }

        // 2. Walk every block and record uses that cross block boundaries.
        let mut cross_blocks = CrossBlockValues::default();
        cross_blocks.init(body.values.len());

        for (block, block_def) in body.blocks.entries() {
            let cur = block.index() as u32;

            // Visit terminator uses.
            block_def.terminator.visit_uses(|u| {
                visit_use(u, body, trees, cur, &def_block, &mut cross_blocks);
            });

            // Visit instruction args (in reverse order per the body walk).
            for &inst in block_def.insts.iter().rev() {
                if is_tree_or_remat(inst, trees) {
                    continue;
                }
                if let ValueDef::Operator(_, args, _) = &body.values[inst] {
                    for &arg in &body.arg_pool[*args] {
                        visit_use(arg, body, trees, cur, &def_block, &mut cross_blocks);
                    }
                }
            }
        }

        cross_blocks
    }
}

/// Check whether a value is treeified (owned by another) or rematerialized.
fn is_tree_or_remat(value: Value, trees: &Trees) -> bool {
    trees.owner.contains_key(&value) || trees.remat.contains(&value)
}

/// Recursively resolve a use through aliases and treeified/remat values,
/// recording cross-block values into the map.
fn visit_use(
    value: Value,
    body: &FunctionBody,
    trees: &Trees,
    cur_block: u32,
    def_block: &[u32],
    cross_blocks: &mut CrossBlockValues,
) {
    let value = body.resolve_alias(value);
    if let ValueDef::PickOutput(value, _, _) = body.values[value] {
        visit_use(value, body, trees, cur_block, def_block, cross_blocks);
        return;
    }
    if is_tree_or_remat(value, trees) {
        // Treeified or remat: visit its args without recording the value itself.
        if let ValueDef::Operator(_, args, _) = body.values[value] {
            for &arg in &body.arg_pool[args] {
                visit_use(arg, body, trees, cur_block, def_block, cross_blocks);
            }
        }
        return;
    }
    // Proper use: record if it crosses block boundaries.
    if def_block[value.index()] != cur_block {
        cross_blocks.insert(value);
    }
}

impl Index<CrossBlockId> for CrossBlockValues {
    type Output = Value;

    fn index(&self, id: CrossBlockId) -> &Self::Output {
        &self.values[id.0.get() as usize - 1]
    }
}
