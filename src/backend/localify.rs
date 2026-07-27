//! Localification: a simple form of register allocation that picks
//! locations for SSA values in Wasm locals.
//!
//! Performance notes: liveness sets and live-range maps are keyed by
//! `Value`, a dense index space, so all per-value state lives in flat
//! arrays. The liveness fixpoint additionally runs over a compacted
//! index space of only the cross-block values (values with a use
//! outside their defining block), and we track cross-block liveness
//! with bitsets indexed by this index space.

use crate::backend::bitset::Bitset;
use crate::backend::cross_block_ids::{CrossBlockId, CrossBlockValues};
use crate::backend::dense_live_set::DenseLiveSet;
use crate::backend::treeify::Trees;
use crate::cfg::CFGInfo;
use crate::entity::{EntityRef, EntityVec, PerEntity};
use crate::ir::{Block, FunctionBody, Local, Type, Value, ValueDef};
use fxhash::FxHashMap as HashMap;
use smallvec::{smallvec, SmallVec};
use std::ops::Range;

#[derive(Clone, Debug, Default)]
pub struct Localifier {
    pub values: PerEntity<Value, SmallVec<[Local; 2]>>,
    pub locals: EntityVec<Local, Type>,
}

impl Localifier {
    pub fn compute(body: &FunctionBody, cfg: &CFGInfo, trees: &Trees) -> Self {
        Context::new(body, cfg, trees).compute()
    }
}

struct Context<'a> {
    body: &'a FunctionBody,
    cfg: &'a CFGInfo,
    trees: &'a Trees,
    results: Localifier,

    /// Two-way map between `Value` and `CrossBlockId` for the compacted
    /// inter-block value index space.
    cross_blocks: CrossBlockValues,

    /// Precise liveness for each block: cross-block values live at the
    /// end, as a bitset over compact ids.
    block_end_live: PerEntity<Block, Bitset>,

    /// Liveranges for each Value, in an arbitrary index space
    /// (concretely, the span of first to last instruction visit step
    /// index in an RPO walk over the function body). `usize::MAX`
    /// start means "no range recorded".
    ranges: Vec<Range<usize>>,
    /// Number of points.
    points: usize,
}

trait Visitor {
    fn visit_use(&mut self, _: Value) {}
    fn visit_def(&mut self, _: Value) {}
    fn post_inst(&mut self, _: Value) {}
    fn pre_inst(&mut self, _: Value) {}
    fn post_term(&mut self) {}
    fn pre_term(&mut self) {}
    fn post_params(&mut self) {}
    fn pre_params(&mut self) {}
}

fn is_tree_or_remat(value: Value, trees: &Trees) -> bool {
    trees.owner.contains_key(&value) || trees.remat.contains(&value)
}

struct BlockVisitor<'a, V: Visitor> {
    body: &'a FunctionBody,
    trees: &'a Trees,
    visitor: V,
}
impl<'a, V: Visitor> BlockVisitor<'a, V> {
    fn new(body: &'a FunctionBody, trees: &'a Trees, visitor: V) -> Self {
        log::trace!(
            "localify: running on:\n{}",
            body.display_verbose("| ", None)
        );
        Self {
            body,
            trees,
            visitor,
        }
    }
    fn visit_block(&mut self, block: Block) {
        self.visitor.post_term();
        self.body.blocks[block].terminator.visit_uses(|u| {
            self.visit_use(u);
        });
        self.visitor.pre_term();

        for &inst in self.body.blocks[block].insts.iter().rev() {
            if is_tree_or_remat(inst, self.trees) {
                continue;
            }
            self.visitor.post_inst(inst);
            self.visit_inst(inst, /* root = */ true);
            self.visitor.pre_inst(inst);
        }

        self.visitor.post_params();
        for &(_, param) in &self.body.blocks[block].params {
            self.visitor.visit_def(param);
        }
        self.visitor.pre_params();
    }
    fn visit_inst(&mut self, value: Value, root: bool) {
        // If this is an instruction...
        if let ValueDef::Operator(_, args, _) = &self.body.values[value] {
            // If root, we need to process the def.
            if root {
                self.visitor.visit_def(value);
            }
            // Handle uses.
            for &arg in &self.body.arg_pool[*args] {
                self.visit_use(arg);
            }
        }
    }
    fn visit_use(&mut self, value: Value) {
        let value = self.body.resolve_alias(value);
        if let ValueDef::PickOutput(value, _, _) = self.body.values[value] {
            self.visit_use(value);
            return;
        }
        if is_tree_or_remat(value, self.trees) {
            // If this is a treeified or rematerialized value, then
            // don't process the use, but process the instruction
            // directly here. (Lowering rematerializes remat values at
            // each use and never lowers their defs, so a remat use
            // must not keep a local alive: its def is never visited
            // and it would otherwise be tracked as live everywhere,
            // holding a dead local for the whole function.)
            self.visit_inst(value, /* root = */ false);
        } else {
            // Otherwise, this is a proper use.
            self.visitor.visit_use(value);
        }
    }
}

impl<'a> Context<'a> {
    fn new(body: &'a FunctionBody, cfg: &'a CFGInfo, trees: &'a Trees) -> Self {
        let mut results = Localifier::default();

        // Create locals for function args.
        for &(ty, value) in &body.blocks[body.entry].params {
            let param_local = results.locals.push(ty);
            results.values[value] = smallvec![param_local];
        }

        Self {
            body,
            cfg,
            trees,
            results,
            cross_blocks: CrossBlockValues::default(),
            block_end_live: PerEntity::default(),
            ranges: vec![usize::MAX..usize::MAX; body.values.len()],
            points: 0,
        }
    }

    /// Assign compact ids to the values with a (proper) use outside
    /// the block where their def is VISITED; only those can appear in
    /// a liveness set. A def the block walk skips (treeified or remat
    /// insts) never cancels its uses, so such values stay tracked
    /// everywhere, matching the walk's behavior.
    fn find_cross_block_values(&mut self) {
        self.cross_blocks = CrossBlockValues::build(self.body, self.trees);
    }

    fn compute_liveness(&mut self) {
        struct LivenessVisitor<'b> {
            cross_blocks: &'b CrossBlockValues,
            live: DenseLiveSet,
        }
        impl<'b> Visitor for LivenessVisitor<'b> {
            fn visit_use(&mut self, value: Value) {
                if let Some(id) = self.cross_blocks.get(value) {
                    self.live.set_live(id.as_u32());
                }
            }
            fn visit_def(&mut self, value: Value) {
                if let Some(id) = self.cross_blocks.get(value) {
                    self.live.set_dead(id.as_u32());
                }
            }
        }

        let mut visitor = BlockVisitor::new(
            self.body,
            self.trees,
            LivenessVisitor {
                cross_blocks: &self.cross_blocks,
                live: DenseLiveSet::new(self.cross_blocks.len()),
            },
        );
        let mut workqueue: Vec<Block> = self.cfg.rpo.values().cloned().collect();
        let mut workqueue_set = vec![true; self.body.blocks.len()];
        while let Some(block) = workqueue.pop() {
            workqueue_set[block.index()] = false;
            visitor.visitor.live.next_epoch();
            for id in self.block_end_live[block].iter() {
                visitor.visitor.live.set_live(id);
            }
            visitor.visit_block(block);

            // Insert the live-in ids straight into each pred's
            // live-out bitset.
            let live = &visitor.visitor.live;
            for &pred in &self.body.blocks[block].preds {
                let pred_live = &mut self.block_end_live[pred];
                let mut changed = false;
                for &id in live.touched() {
                    if live.is_live(id) && pred_live.insert(id) {
                        changed = true;
                    }
                }
                if changed && !workqueue_set[pred.index()] {
                    workqueue_set[pred.index()] = true;
                    workqueue.push(pred);
                }
            }
        }
    }

    fn find_ranges(&mut self) {
        let mut point = 0;

        struct LiveRangeVisitor<'b> {
            point: &'b mut usize,
            /// Live values in the current block walk; the range start
            /// (visit point of the last use seen) rides in `start`.
            live: DenseLiveSet,
            start: &'b mut [usize],
            ranges: &'b mut [Range<usize>],
        }
        impl<'b> LiveRangeVisitor<'b> {
            fn record_def(&mut self, value: Value, range: Range<usize>) {
                let existing = &mut self.ranges[value.index()];
                if existing.start == usize::MAX {
                    *existing = range;
                } else {
                    existing.start = std::cmp::min(existing.start, range.start);
                    existing.end = std::cmp::max(existing.end, range.end);
                }
            }
        }
        impl<'b> Visitor for LiveRangeVisitor<'b> {
            fn pre_params(&mut self) {
                *self.point += 1;
            }
            fn pre_inst(&mut self, _: Value) {
                *self.point += 1;
            }
            fn pre_term(&mut self) {
                *self.point += 1;
            }
            fn visit_use(&mut self, value: Value) {
                if self.live.set_live(value.index() as u32) {
                    self.start[value.index()] = *self.point;
                }
            }
            fn visit_def(&mut self, value: Value) {
                let range = if self.live.set_dead(value.index() as u32) {
                    self.start[value.index()]..(*self.point + 1)
                } else {
                    *self.point..(*self.point + 1)
                };
                self.record_def(value, range);
            }
        }

        let mut start = vec![0usize; self.body.values.len()];
        let mut live = DenseLiveSet::new(self.body.values.len());
        for &block in self.cfg.rpo.values().rev() {
            live.next_epoch();
            let visitor = LiveRangeVisitor {
                live,
                start: &mut start,
                point: &mut point,
                ranges: &mut self.ranges,
            };
            let mut visitor = BlockVisitor::new(&self.body, self.trees, visitor);
            // Live-outs to succ blocks: in this block-local
            // handling, model them as uses as the end of the block.
            for id in self.block_end_live[block].iter() {
                let livein = self.cross_blocks.value(CrossBlockId::new(id));
                let livein = self.body.resolve_alias(livein);
                visitor.visitor.visit_use(livein);
            }
            // Visit all insts.
            visitor.visit_block(block);
            // Live-ins from pred blocks: anything still live has a
            // virtual def at top of block.
            let still_live: Vec<Value> = visitor
                .visitor
                .live
                .touched()
                .iter()
                .copied()
                .filter(|&i| visitor.visitor.live.is_live(i))
                .map(|i| Value::new(i as usize))
                .collect();
            for v in still_live {
                visitor.visitor.visit_def(v);
            }
            live = visitor.visitor.live;
        }

        self.points = point + 1;
    }

    fn allocate(&mut self) {
        // Sort values by ranges' starting points, then value to break ties.
        let mut ranges: Vec<(Value, std::ops::Range<usize>)> = self
            .ranges
            .iter()
            .enumerate()
            .filter(|(_, r)| r.start != usize::MAX)
            .map(|(i, r)| (Value::new(i), r.clone()))
            .collect();
        ranges.sort_unstable_by_key(|(val, range)| (range.start, *val));

        // Keep a list of expiring Locals by expiry point.
        let mut expiring: HashMap<usize, SmallVec<[(Type, Local); 8]>> = HashMap::default();

        // Iterate over allocation space, processing range starts (at
        // which point we allocate) and ends (at which point we add to
        // the freelist).
        let mut range_idx = 0;
        let mut freelist: HashMap<Type, Vec<Local>> = HashMap::default();

        for i in 0..self.points {
            // Process ends. (Ends are exclusive, so we do them
            // first; another range can grab the local at the same
            // point index in this same iteration.)
            if let Some(expiring) = expiring.remove(&i) {
                for (ty, local) in expiring {
                    log::trace!(" -> expiring {} of type {} back to freelist", local, ty);
                    freelist.entry(ty).or_insert_with(|| vec![]).push(local);
                }
            }

            // Process starts.
            while range_idx < ranges.len() && ranges[range_idx].1.start == i {
                let (value, range) = ranges[range_idx].clone();
                range_idx += 1;
                log::trace!(
                    "localify: processing range for {}: {}..{}",
                    value,
                    range.start,
                    range.end
                );

                // If the value is an arg on block0, ignore; these
                // already have fixed locations.
                if let &ValueDef::BlockParam(b, _, _) = &self.body.values[value] {
                    if b == self.body.entry {
                        continue;
                    }
                }

                // Try getting a local from the freelist; if not,
                // allocate a new one.
                let mut allocs = smallvec![];
                let expiring = expiring.entry(range.end).or_insert_with(|| smallvec![]);
                for &ty in self.body.values[value].tys(&self.body.type_pool) {
                    let local = freelist
                        .get_mut(&ty)
                        .and_then(|v| v.pop())
                        .unwrap_or_else(|| {
                            log::trace!(" -> allocating new local of type {}", ty);
                            self.results.locals.push(ty)
                        });
                    log::trace!(" -> got local {} of type {}", local, ty);
                    allocs.push(local);
                    expiring.push((ty, local));
                }
                self.results.values[value] = allocs;
            }
        }
    }

    fn compute(mut self) -> Localifier {
        self.find_cross_block_values();
        self.compute_liveness();
        self.find_ranges();
        self.allocate();
        self.results
    }
}
