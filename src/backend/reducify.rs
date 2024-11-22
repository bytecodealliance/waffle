//! Reducification: turning a potentially irreducible CFG into a
//! reducible CFG. We perform context-sensitive code duplication to
//! "peel off" the parts of loops that are reached by side-entrances,
//! branching back to the main loop as soon as control passes through
//! the loop header again.
//!
//! # Limitations
//!
//! ***WARNING*** EXPONENTIAL BLOWUP POTENTIAL ***WARNING***
//!
//! This pass is designed on the assumption that irreducible control
//! flow is rare, and needs to be handled somehow but it's OK to,
//! e.g., duplicate most of a loop body to do so. The tradeoff that
//! we're aiming for is that we want zero runtime overhead -- we do
//! not want a performance cliff if someone accidentally introduces an
//! irreducible edge -- and we're hoping that this remains rare. If
//! you feed this pass a state machine, or a fully-connected clique,
//! for example, or even a deep nest of loops, one can get much worse
//! than 2x code-size increase. You have been warned!
//!
//! In the future we may consider a hybrid approach where we start
//! with this algorithm, keep track of block-count increase, and abort
//! and move to a Relooper-style (dynamic label variable-based)
//! approach with no code duplication if a threshold is reached.
//!
//! ***WARNING*** EXPONENTIAL BLOWUP POTENTIAL ***WARNING***
//!
//! # Finding Loop Headers
//!
//! The basic idea is that we compute RPO and treat all backedges in
//! RPO (i.e., edges from rpo-index i to rpo-index j, where j <= i) as
//! loop backedges, with all blocks "under the edge" (with RPO indices
//! i..=j) in the loop. We then "properly nest" loops, so if we have,
//! e.g.:
//!
//! ```plain
//!     block0
//!     block1  |
//!     block2  | loop     |
//!     block3  |          |
//!     block4             | loop
//! ```
//!
//! we "fix the nesting" by pushing down the lower extent of the first
//! loop to block4. We do so in a single post-pass fixup scan that
//! keeps a stack, pushes when meeting a loop header, pops while the
//! innermost is no longer in the initial header-set, then ensures
//! that all header-blockson the stack are inserted into every
//! header-set it passes over.
//!
//! The effect of this is to compute a loop nest *as if* irreducible
//! edges (side loop entrances) did not exist. We'll fix them up later
//! with the code duplication.
//!
//! # Finding Irreducible Loop Headers
//!
//! After computing header-sets, find edges from B1 to B2 such that
//! headers(B2) - headers(B1) - {B2} is non-empty -- that is, we add a
//! header block (enter a new loop) going from B1 to B2, and that new
//! header block is not B2 itself. This is a "side-entrance" into a
//! loop, and is irreducible.
//!
//! # Duplicating Code
//!
//! We create blocks under contexts defined by "skipped
//! headers", where the context is computed at at an edge
//! (From, To) as (where `U` is set union, `-` is set
//! subtraction, `&` is set intersection, `!S` is the set
//! complement):
//!
//! ```plain
//!     Gen = (headers(To) - headers(From)) - {To}
//!         = headers(To) & !headers(From) & !{To}
//!     Kill = (headers(From) - headers(To)) U {To}
//!          = (headers(From) & !headers(To)) U {To}
//!
//! let ToContext = (FromContext - Kill) U Gen
//!               = (FromContext & !Kill) U Gen
//!               = (FromContext & !((headers(From) & !headers(To)) U {To})) U
//!                 (headers(To) & !headers(From) & !{To})
//!               = (FromContext & !((headers(From) U {To}) & (!headers(To) U {To}))) U
//!                 (headers(To) & !headers(From) & !{To})
//!               = (FromContext & (!(headers(From) U {To}) U !(!headers(To) U {To}))) U
//!                 (headers(To) & !headers(From) & !{To})
//!               = (FromContext & ((!headers(From) & !{To}) U (headers(To) & !{To}))) U
//!                 (headers(To) & !headers(From) & !{To})
//!               = (FromContext & !headers(From) & !{To}) U
//!                 (FromContext & headers(To) & !{To}) U
//!                 (headers(To) & !headers(From) & !{To})
//! ```
//!
//! invariant: for every B, we only ever have a context C where C c headers(B)
//!
//! then the first term goes away (FromContext & !headers(From)
//! = 0) and we can simplify to:
//!
//! ```plain
//! let ToContext = headers(To) & !{To} & (FromContext U !headers(From))
//! ```
//!
//! in other words: when entering a loop except through its
//! header, add to context; stay in that context inside the
//! loop; leave the context when we leave the loop.
//!
//! Note that in the case with no irreducible edges, this
//! becomes the special case where every context is {} and no
//! blocks are actually duplicated (but we returned early above
//! to avoid this no-op transform).
//!
//! Patching up use-def links is somewhat tricky. Consider the
//! CFG:
//!
//! ```plain
//!         1
//!        / \
//!       /   \
//!      2 --> 3
//!      2 <-- 3
//!           /
//!          4
//! ```
//!
//! Which is irreducible (it contains the canonical irreducible
//! graph 1->2, 2->3, 3->2) and has an exit-path with block 4
//! that is dominated by block 3. Block 4 can thus use values
//! defined in block 3, but if we perform elaboration as:
//!
//! ```plain
//!     1
//!   /  \__
//!  2<.<--3'
//!  v ^   |
//!  3-/  _|
//!   \ /
//!    4
//! ```
//!
//! that is, we have two copies of the block 3,and each has an
//! exit to the one copy of 4.
//!
//! Any values defined in 3 and used in 4 in the original CFG
//! will need to pass through blockparams to merge the two
//! versions in the elaborated CFG.
//!
//! To fix this problem, we perform a max-SSA cut at all blocks
//! that have an in-edge from a block with a larger header-set
//! (i.e., a loop exit edge) if the exited loop has a
//! side-entrance; this is the only way in which we can have a
//! merge-point between different copies of the same subgraph.

use crate::entity::EntityRef;
use crate::{cfg::CFGInfo, cfg::RPOIndex, entity::PerEntity, Block, FunctionBody, Value, ValueDef};
use fxhash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::{HashSet, VecDeque};

pub struct Reducifier<'a> {
    body: &'a FunctionBody,
    cfg: CFGInfo,
    blocks: PerEntity<Block, BlockState>,
}

#[derive(Debug, Clone, Default)]
struct BlockState {
    headers: FxHashSet<Block>,
    is_header: bool,
}

impl<'a> Reducifier<'a> {
    pub fn new(body: &'a FunctionBody) -> Reducifier<'a> {
        let cfg = CFGInfo::new(body);
        Reducifier {
            body,
            cfg,
            blocks: PerEntity::default(),
        }
    }

    pub fn run(&mut self) -> Cow<'a, FunctionBody> {
        // First, compute all of the loop header-sets.
        // - Start by computing RPO.
        // - Find backedges (edges (a, b) where rpo(b) <= rpo(a)).
        // - For each backedge, mark extent of rpo-indices "under"
        //   edge as within header.
        // - Do one forward pass to properly nest regions, keeping
        //   stack of headers when we entered their regions and
        //   enforcing LIFO by extending appropriately.
        let cfg = CFGInfo::new(&self.body);

        let mut has_irreducible = false;
        for (rpo, &block) in cfg.rpo.entries() {
            for &succ in &self.body.blocks[block].succs {
                let succ_rpo = cfg.rpo_pos[succ].unwrap();
                if succ_rpo.index() <= rpo.index() && !cfg.dominates(succ, block) {
                    has_irreducible = true;
                }
            }
        }
        if !has_irreducible {
            return Cow::Borrowed(self.body);
        }

        for (rpo, &block) in cfg.rpo.entries() {
            for &succ in &self.body.blocks[block].succs {
                let succ_rpo = cfg.rpo_pos[succ].unwrap();
                if succ_rpo.index() <= rpo.index() {
                    for i in succ_rpo.index()..=rpo.index() {
                        let b = cfg.rpo[RPOIndex::new(i)];
                        self.blocks[b].headers.insert(succ);
                        self.blocks[b].is_header = true;
                    }
                }
            }
        }

        let mut header_stack = vec![];
        for &block in cfg.rpo.values() {
            while let Some(innermost) = header_stack.last() {
                if !self.blocks[block].headers.contains(innermost) {
                    header_stack.pop();
                } else {
                    break;
                }
            }
            if self.blocks[block].is_header {
                header_stack.push(block);
            }

            for &header in &header_stack {
                self.blocks[block].headers.insert(header);
            }
        }

        // Now, check whether any irreducible edges exist: edges from
        // B1 to B2 where headers(B2) - headers(B1) - {B2} is not
        // empty (i.e., the edge jumps into a new loop -- adds a new
        // header -- without going through that header block).
        let mut irreducible_headers: FxHashSet<Block> = FxHashSet::default();
        for (block, data) in self.body.blocks.entries() {
            let headers = &self.blocks[block].headers;
            for &succ in &data.succs {
                log::trace!("examining edge {} -> {}", block, succ);
                for &succ_header in &self.blocks[succ].headers {
                    log::trace!("  successor {} has header {}", succ, succ_header);
                    if succ_header != succ && !headers.contains(&succ_header) {
                        log::trace!("    -> irreducible edge");
                        irreducible_headers.insert(succ_header);
                    }
                }
            }
        }

        if log::log_enabled!(log::Level::Trace) {
            for block in self.body.blocks.iter() {
                let mut headers = self.blocks[block]
                    .headers
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>();
                headers.sort();
                log::trace!("* {}: header set {:?}", block, headers);
            }
        }

        // Now, in the irreducible case, "elaborate" the CFG.

        // First do limited conversion to max-SSA to fix up references
        // across contexts.
        let mut cut_blocks = HashSet::default();
        for (block, data) in self.body.blocks.entries() {
            for &succ in &data.succs {
                // Loop exits
                for header in &self.blocks[block].headers {
                    if !self.blocks[succ].headers.contains(header)
                        && irreducible_headers.contains(header)
                    {
                        log::trace!("cut-block at loop exit: {}", succ);
                        cut_blocks.insert(succ);
                    }
                }
                // Loop side entries
                for header in &self.blocks[succ].headers {
                    if !self.blocks[block].headers.contains(header) && *header != succ {
                        log::trace!("cut-block at loop side entry: {}", succ);
                        cut_blocks.insert(succ);
                    }
                }
            }
        }

        let mut new_body = self.body.clone();
        let cfg = CFGInfo::new(&new_body);
        crate::passes::resolve_aliases::run(&mut new_body);
        crate::passes::maxssa::run(&mut new_body, Some(cut_blocks), &cfg);
        crate::passes::resolve_aliases::run(&mut new_body);

        log::trace!("after max-SSA run:\n{}\n", new_body.display("| ", None));

        // Implicitly, context {} has an identity-map from old block
        // number to new block number. We use the map only for
        // non-empty contexts.
        let mut context_map: FxHashMap<Vec<Block>, usize> = FxHashMap::default();
        let mut contexts: Vec<Vec<Block>> = vec![vec![]];
        context_map.insert(vec![], 0);
        let mut block_map: FxHashMap<(usize, Block), Block> = FxHashMap::default();
        let mut value_map: FxHashMap<(usize, Value), Value> = FxHashMap::default();

        // List of (ctx, new block) tuples for duplicated code.
        let mut cloned_blocks: Vec<(usize, Block)> = vec![];
        // Map from block in new body to (ctx, orig block) target, to
        // allow updating terminators.
        let mut terminators: FxHashMap<Block, Vec<(usize, Block)>> = FxHashMap::default();

        let mut queue: VecDeque<(usize, Block)> = VecDeque::new();
        let mut visited: FxHashSet<(usize, Block)> = FxHashSet::default();
        queue.push_back((0, new_body.entry));
        visited.insert((0, new_body.entry));
        while let Some((ctx, block)) = queue.pop_front() {
            log::trace!(
                "elaborate: block {} in context {} ({:?})",
                block,
                ctx,
                contexts[ctx]
            );

            // If this is a non-default context, replicate the block.
            let new_block = if ctx != 0 {
                log::trace!("cloning block {} in new context", block);
                let new_block = new_body.add_block();
                new_body.blocks[new_block].desc = format!("Cloned {}", block);
                let params = new_body.blocks[block].params.clone();
                for (ty, val) in params {
                    let blockparam = new_body.add_blockparam(new_block, ty);
                    value_map.insert((ctx, val), blockparam);
                }

                block_map.insert((ctx, block), new_block);
                cloned_blocks.push((ctx, new_block));

                // Copy over all value definitions, but don't rewrite
                // args yet -- we'll do a separate pass for that.
                let insts = new_body.blocks[block].insts.clone();
                for value in insts {
                    let def = new_body.values[value].clone();
                    let new_value = new_body.values.push(def);
                    value_map.insert((ctx, value), new_value);
                    new_body.blocks[new_block].insts.push(new_value);
                }

                // Copy over the terminator but don't update yet --
                // we'll do that later too.
                new_body.blocks[new_block].terminator = new_body.blocks[block].terminator.clone();

                new_block
            } else {
                block
            };

            // For every terminator, determine the target context:
            //
            // let ToContext = headers(To) & !{To} & (FromContext U !headers(From))
            let term = terminators.entry(new_block).or_insert_with(|| vec![]);
            let succs = new_body.blocks[block].succs.clone();
            for succ in succs {
                let mut ctx_blocks = self.blocks[succ]
                    .headers
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>();
                ctx_blocks.sort();
                ctx_blocks.retain(|&header_block| {
                    header_block != succ
                        && (contexts[ctx].contains(&header_block)
                            || !self.blocks[block].headers.contains(&header_block))
                });
                let to_ctx = *context_map.entry(ctx_blocks.clone()).or_insert_with(|| {
                    let id = contexts.len();
                    contexts.push(ctx_blocks);
                    id
                });
                log::trace!(
                    "elaborate: edge {} -> {} from ctx {:?} goes to ctx {:?}",
                    block,
                    succ,
                    contexts[ctx],
                    contexts[to_ctx]
                );

                term.push((to_ctx, succ));

                if visited.insert((to_ctx, succ)) {
                    log::trace!("enqueue block {} ctx {}", succ, to_ctx);
                    queue.push_back((to_ctx, succ));
                }
            }
        }

        // Second pass: rewrite args, and set up terminators. Both
        // happen in a second pass so that we have the block- and
        // value-map available for all blocks and values, regardless
        // of cycles or processing order.
        for (ctx, new_block) in cloned_blocks {
            for &inst in &new_body.blocks[new_block].insts {
                match &mut new_body.values[inst] {
                    ValueDef::Operator(_, args, _) => {
                        let new_args = new_body.arg_pool[*args]
                            .iter()
                            .map(|&val| value_map.get(&(ctx, val)).cloned().unwrap_or(val))
                            .collect::<SmallVec<[Value; 4]>>();
                        let new_args = new_body.arg_pool.from_iter(new_args.into_iter());
                        *args = new_args;
                    }
                    ValueDef::PickOutput(val, _, _) | ValueDef::Alias(val) => {
                        *val = value_map.get(&(ctx, *val)).cloned().unwrap_or(*val);
                    }
                    _ => unreachable!(),
                }
            }

            new_body.blocks[new_block]
                .terminator
                .update_uses(|u| *u = value_map.get(&(ctx, *u)).cloned().unwrap_or(*u));
        }

        for (block, block_def) in new_body.blocks.entries_mut() {
            log::trace!("processing terminators for block {}", block);
            let terms = match terminators.get(&block) {
                Some(t) => t,
                // If no entry in `terminators`, we didn't visit the
                // block; it must not be reachable.
                None => continue,
            };
            let mut terms = terms.iter();
            block_def.terminator.update_targets(|target| {
                let &(to_ctx, to_orig_block) = terms.next().unwrap();
                target.block = block_map
                    .get(&(to_ctx, to_orig_block))
                    .cloned()
                    .unwrap_or(to_orig_block);
            });
        }

        new_body.recompute_edges();

        log::trace!("After duplication:\n{}\n", new_body.display("| ", None));

        new_body.validate().unwrap();
        new_body.verify_reducible().unwrap();

        Cow::Owned(new_body)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        entity::EntityRef, BlockTarget, FuncDecl, Module, Operator, SignatureData, Terminator, Type,
    };

    #[test]
    fn test_irreducible() {
        let _ = env_logger::try_init();

        let mut module = Module::empty();
        let sig = module.signatures.push(SignatureData {
            params: vec![Type::I32, Type::I64, Type::F64],
            returns: vec![Type::I64],
        });
        let mut body = FunctionBody::new(&module, sig);

        let block1 = body.entry;
        let block2 = body.add_block();
        let block3 = body.add_block();
        let block4 = body.add_block();

        let arg0 = body.blocks[block1].params[0].1;
        let arg1 = body.blocks[block1].params[1].1;
        let arg2 = body.blocks[block1].params[2].1;

        body.set_terminator(
            block1,
            Terminator::CondBr {
                cond: arg0,
                if_true: BlockTarget {
                    block: block2,
                    args: vec![arg1],
                },
                if_false: BlockTarget {
                    block: block3,
                    args: vec![arg2],
                },
            },
        );

        let block2_param = body.add_blockparam(block2, Type::I64);
        let block3_param = body.add_blockparam(block3, Type::F64);

        let block2_param_cast = body.add_op(
            block2,
            Operator::F64ReinterpretI64,
            &[block2_param],
            &[Type::F64],
        );

        let block3_param_cast = body.add_op(
            block3,
            Operator::I64ReinterpretF64,
            &[block3_param],
            &[Type::I64],
        );

        body.set_terminator(
            block2,
            Terminator::Br {
                target: BlockTarget {
                    block: block3,
                    args: vec![block2_param_cast],
                },
            },
        );
        body.set_terminator(
            block3,
            Terminator::CondBr {
                cond: arg0,
                if_true: BlockTarget {
                    block: block2,
                    args: vec![block3_param_cast],
                },
                if_false: BlockTarget {
                    block: block4,
                    args: vec![],
                },
            },
        );

        body.set_terminator(
            block4,
            Terminator::Return {
                values: vec![block3_param_cast],
            },
        );

        log::debug!("Body:\n{}", body.display("| ", Some(&module)));

        body.validate().unwrap();

        let mut reducifier = Reducifier::new(&body);
        let new_body = reducifier.run();

        new_body.validate().unwrap();

        log::debug!("Reducified body:\n{}", body.display("| ", Some(&module)));

        let cfg = CFGInfo::new(&new_body);
        for (block, def) in new_body.blocks.entries() {
            for &succ in &def.succs {
                // For any edge to a block earlier in RPO, that block
                // must dominate us.
                if cfg.rpo_pos[succ].unwrap().index() <= cfg.rpo_pos[block].unwrap().index() {
                    assert!(cfg.dominates(succ, block));
                }
            }
        }

        // Now ensure we can generate a Wasm module (with reducible
        // control flow).
        module
            .funcs
            .push(FuncDecl::Body(sig, "func0".to_string(), body));
        let wasm = module.to_wasm_bytes().unwrap();
        log::debug!("wasm bytes: {:?}", wasm);
    }
}
