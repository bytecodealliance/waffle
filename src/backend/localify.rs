//! Localification: a simple form of register allocation that picks
//! locations for SSA values in Wasm locals.

use crate::backend::treeify::Trees;
use crate::cfg::CFGInfo;
use crate::entity::{EntityVec, PerEntity};
use crate::ir::{Block, FunctionBody, Local, Type, Value, ValueDef};
use smallvec::{smallvec, SmallVec};
use std::collections::{HashMap, HashSet};
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

    /// Precise liveness for each block: live Values at the end.
    block_end_live: PerEntity<Block, HashSet<Value>>,

    /// Liveranges for each Value, in an arbitrary index space
    /// (concretely, the span of first to last instruction visit step
    /// index in an RPO walk over the function body).
    ranges: HashMap<Value, Range<usize>>,
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
            if self.trees.owner.contains_key(&inst) || self.trees.remat.contains(&inst) {
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
        if self.trees.owner.contains_key(&value) {
            // If this is a treeified value, then don't process the use,
            // but process the instruction directly here.
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
            block_end_live: PerEntity::default(),
            ranges: HashMap::default(),
            points: 0,
        }
    }

    fn compute_liveness(&mut self) {
        struct LivenessVisitor {
            live: HashSet<Value>,
        }
        impl Visitor for LivenessVisitor {
            fn visit_use(&mut self, value: Value) {
                self.live.insert(value);
            }
            fn visit_def(&mut self, value: Value) {
                self.live.remove(&value);
            }
        }

        let mut workqueue: Vec<Block> = self.cfg.rpo.values().cloned().collect();
        let mut workqueue_set: HashSet<Block> = workqueue.iter().cloned().collect();
        while let Some(block) = workqueue.pop() {
            workqueue_set.remove(&block);
            let live = self.block_end_live[block].clone();
            let mut visitor = BlockVisitor::new(self.body, self.trees, LivenessVisitor { live });
            visitor.visit_block(block);
            let live = visitor.visitor.live;

            for &pred in &self.body.blocks[block].preds {
                let pred_live = &mut self.block_end_live[pred];
                let mut changed = false;
                for &value in &live {
                    if pred_live.insert(value) {
                        changed = true;
                    }
                }
                if changed && workqueue_set.insert(pred) {
                    workqueue.push(pred);
                }
            }
        }
    }

    fn find_ranges(&mut self) {
        let mut point = 0;

        struct LiveRangeVisitor<'b> {
            point: &'b mut usize,
            live: HashMap<Value, usize>,
            ranges: &'b mut HashMap<Value, Range<usize>>,
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
                self.live.entry(value).or_insert(*self.point);
            }
            fn visit_def(&mut self, value: Value) {
                let range = if let Some(start) = self.live.remove(&value) {
                    start..(*self.point + 1)
                } else {
                    *self.point..(*self.point + 1)
                };
                let existing_range = self.ranges.entry(value).or_insert(range.clone());
                existing_range.start = std::cmp::min(existing_range.start, range.start);
                existing_range.end = std::cmp::max(existing_range.end, range.end);
            }
        }

        for &block in self.cfg.rpo.values().rev() {
            let visitor = LiveRangeVisitor {
                live: HashMap::default(),
                point: &mut point,
                ranges: &mut self.ranges,
            };
            let mut visitor = BlockVisitor::new(&self.body, &self.trees, visitor);
            // Live-outs to succ blocks: in this block-local
            // handling, model them as uses as the end of the block.
            for &livein in &self.block_end_live[block] {
                let livein = self.body.resolve_alias(livein);
                visitor.visitor.visit_use(livein);
            }
            // Visit all insts.
            visitor.visit_block(block);
            // Live-ins from pred blocks: anything still live has a
            // virtual def at top of block.
            let still_live = visitor.visitor.live.keys().cloned().collect::<Vec<_>>();
            for live in still_live {
                visitor.visitor.visit_def(live);
            }
        }

        self.points = point + 1;
    }

    fn allocate(&mut self) {
        // Sort values by ranges' starting points, then value to break ties.
        let mut ranges: Vec<(Value, std::ops::Range<usize>)> =
            self.ranges.iter().map(|(k, v)| (*k, v.clone())).collect();
        ranges.sort_unstable_by_key(|(val, range)| (range.start, *val));

        // Keep a list of expiring Locals by expiry point.
        let mut expiring: HashMap<usize, SmallVec<[(Type, Local); 8]>> = HashMap::new();

        // Iterate over allocation space, processing range starts (at
        // which point we allocate) and ends (at which point we add to
        // the freelist).
        let mut range_idx = 0;
        let mut freelist: HashMap<Type, Vec<Local>> = HashMap::new();

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
        self.compute_liveness();
        self.find_ranges();
        self.allocate();
        self.results
    }
}
