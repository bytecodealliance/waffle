//! Stackify implementation to produce structured control flow from an
//! arbitrary CFG.
//!
//! See the paper
//!
//! - Norman Ramsey. Beyond Relooper: recursive translation of
//!   unstructured control flow to structured control flow. In ICFP
//!   2022 (Functional Pearl). https://dl.acm.org/doi/10.1145/3547621
//!
//! for more details on how this algorithm works.

use crate::cfg::CFGInfo;
use crate::entity::EntityRef;
use crate::ir::{Block, BlockTarget, FunctionBody, Terminator, Type, Value};
use fxhash::FxHashSet as HashSet;
use std::convert::TryFrom;

#[derive(Clone, Debug)]
pub enum WasmBlock<'a> {
    /// A Wasm block that has the given contents and whose label jumps
    /// to the given CFG block exit.
    Block {
        body: Vec<WasmBlock<'a>>,
        out: Block,
    },
    /// A Wasm loop that has the given contents and whose label jumps
    /// to the given CFG block header.
    Loop {
        body: Vec<WasmBlock<'a>>,
        header: Block,
    },
    /// A leaf node: one CFG block.
    Leaf { block: Block },
    /// A translated unconditional branch.
    Br { target: WasmLabel },
    /// A translated conditional.
    If {
        cond: Value,
        if_true: Vec<WasmBlock<'a>>,
        if_false: Vec<WasmBlock<'a>>,
    },
    /// A translated select (switch).
    Select {
        selector: Value,
        targets: Vec<WasmLabel>,
        default: WasmLabel,
    },
    /// Blockparam transfer.
    BlockParams {
        from: &'a [Value],
        to: &'a [(Type, Value)],
    },
    /// A function return instruction.
    Return { values: &'a [Value] },
    /// An unreachable instruction.
    Unreachable,
}

/// A Wasm branch target label: number of scopes outward to branch to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WasmLabel(u32);
impl WasmLabel {
    fn new(i: usize) -> WasmLabel {
        WasmLabel(u32::try_from(i).unwrap())
    }
    fn add(&self, extra: usize) -> WasmLabel {
        WasmLabel(self.0.checked_add(u32::try_from(extra).unwrap()).unwrap())
    }
    pub fn index(&self) -> u32 {
        self.0
    }
}

pub struct Context<'a, 'b> {
    body: &'a FunctionBody,
    cfg: &'b CFGInfo,
    merge_nodes: HashSet<Block>,
    loop_headers: HashSet<Block>,
    ctrl_stack: Vec<CtrlEntry>,
    // Explicit recursion:
    // - Stack of actions/continuations.
    process_stack: Vec<StackEntry<'a>>,
    // - Stack of result/body vectors.
    result: Vec<Vec<WasmBlock<'a>>>,
    // - Stack of merge-node-children lists.
    merge_node_children: Vec<Vec<Block>>,
}

#[derive(Clone, Copy, Debug)]
enum CtrlEntry {
    Block { out: Block },
    Loop { header: Block },
    IfThenElse,
}

impl CtrlEntry {
    fn label(&self) -> Block {
        match self {
            CtrlEntry::Block { out } => *out,
            CtrlEntry::Loop { header } => *header,
            CtrlEntry::IfThenElse => Block::invalid(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum StackEntry<'a> {
    DomSubtree(Block),
    EndDomSubtree,
    NodeWithin(Block, usize),
    FinishLoop(Block),
    FinishBlock(Block),
    Else,
    FinishIf(Value),
    DoBranch(Block, &'a BlockTarget),
}

impl<'a, 'b> Context<'a, 'b> {
    pub fn new(body: &'a FunctionBody, cfg: &'b CFGInfo) -> anyhow::Result<Self> {
        let (merge_nodes, loop_headers) = Self::compute_merge_nodes_and_loop_headers(body, cfg)?;
        Ok(Self {
            body,
            cfg,
            merge_nodes,
            loop_headers,
            ctrl_stack: vec![],
            process_stack: vec![],
            result: vec![],
            merge_node_children: vec![],
        })
    }

    fn compute_merge_nodes_and_loop_headers(
        body: &FunctionBody,
        cfg: &CFGInfo,
    ) -> anyhow::Result<(HashSet<Block>, HashSet<Block>)> {
        let mut loop_headers = HashSet::default();
        let mut branched_once = HashSet::default();
        let mut merge_nodes = HashSet::default();

        for (block_rpo, &block) in cfg.rpo.entries() {
            for &succ in &body.blocks[block].succs {
                log::trace!(
                    "block {} ({}) rpo {} has succ {} ({})",
                    block,
                    body.blocks[block].desc,
                    block_rpo,
                    succ,
                    body.blocks[succ].desc,
                );
                let succ_rpo = cfg.rpo_pos[succ].unwrap();
                log::trace!(" -> succ rpo {}", succ_rpo);
                if succ_rpo <= block_rpo {
                    if !cfg.dominates(succ, block) {
                        anyhow::bail!(
                            "Irreducible control flow: edge from {} ({}) to {} ({})",
                            block,
                            body.blocks[block].desc,
                            succ,
                            body.blocks[succ].desc
                        );
                    }
                    // Backward branch.
                    loop_headers.insert(succ);
                } else {
                    // Forward branch.
                    if !branched_once.insert(succ) {
                        merge_nodes.insert(succ);
                    }
                }
            }
        }

        // Make any `select` target a "merge node" too, so it gets its
        // own block.
        for &block in cfg.rpo.values() {
            if let &Terminator::Select {
                ref targets,
                ref default,
                ..
            } = &body.blocks[block].terminator
            {
                for target in &targets[..] {
                    merge_nodes.insert(target.block);
                }
                merge_nodes.insert(default.block);
            }
        }

        Ok((merge_nodes, loop_headers))
    }

    pub fn compute(mut self) -> Vec<WasmBlock<'a>> {
        self.result.push(vec![]);
        self.process_stack
            .push(StackEntry::DomSubtree(self.cfg.entry));
        while let Some(top) = self.process_stack.pop() {
            self.process(top);
        }
        self.result.pop().unwrap()
    }

    fn process(&mut self, entry: StackEntry<'a>) {
        match entry {
            StackEntry::DomSubtree(block) => {
                self.handle_dom_subtree(block);
            }
            StackEntry::EndDomSubtree => {
                self.end_dom_subtree();
            }
            StackEntry::NodeWithin(block, start) => {
                self.node_within(block, start);
            }
            StackEntry::FinishLoop(header) => {
                self.finish_loop(header);
            }
            StackEntry::FinishBlock(out) => {
                self.finish_block(out);
            }
            StackEntry::Else => {
                self.else_();
            }
            StackEntry::FinishIf(cond) => {
                self.finish_if(cond);
            }
            StackEntry::DoBranch(source, target) => {
                self.do_branch(source, target);
            }
        }
    }

    fn handle_dom_subtree(&mut self, block: Block) {
        let mut merge_node_children = self
            .cfg
            .dom_children(block)
            .filter(|child| self.merge_nodes.contains(&child))
            .collect::<Vec<_>>();
        // Sort merge nodes so highest RPO number comes first.
        merge_node_children
            .sort_unstable_by_key(|&block| std::cmp::Reverse(self.cfg.rpo_pos[block]));

        let is_loop_header = self.loop_headers.contains(&block);

        log::trace!(
            "handle_dom_subtree: block {} merge_nodes {:?} loop_header {}",
            block,
            merge_node_children,
            is_loop_header
        );

        // `merge_node_children` stack entry is popped by `EndDomSubtree`.
        self.merge_node_children.push(merge_node_children);
        self.process_stack.push(StackEntry::EndDomSubtree);

        if is_loop_header {
            // Control stack and block-list-result-stack entries are
            // popped by `FinishLoop`.
            self.ctrl_stack.push(CtrlEntry::Loop { header: block });
            self.result.push(vec![]);
            self.process_stack.push(StackEntry::FinishLoop(block));
            self.process_stack.push(StackEntry::NodeWithin(block, 0));
        } else {
            // "tail-call" to `NodeWithin` step, but use existing
            // result-stack entry.
            self.process_stack.push(StackEntry::NodeWithin(block, 0));
        }
    }

    fn end_dom_subtree(&mut self) {
        self.merge_node_children.pop();
    }

    fn finish_loop(&mut self, header: Block) {
        self.ctrl_stack.pop();
        let body = self.result.pop().unwrap();
        self.result
            .last_mut()
            .unwrap()
            .push(WasmBlock::Loop { body, header });
    }

    fn resolve_target(ctrl_stack: &[CtrlEntry], target: Block) -> WasmLabel {
        log::trace!("resolve_target: {} in stack {:?}", target, ctrl_stack);
        WasmLabel(
            u32::try_from(
                ctrl_stack
                    .iter()
                    .rev()
                    .position(|frame| frame.label() == target)
                    .expect("Target must be in control stack"),
            )
            .expect("More than 2^32 frames"),
        )
    }

    fn do_branch(&mut self, source: Block, target: &'a BlockTarget) {
        let into = self.result.last_mut().unwrap();
        log::trace!("do_branch: {} -> {:?}", source, target);
        // This will be a branch to some entry in the control stack if
        // the target is either a merge block, or is a backward branch
        // (by RPO number).
        if self.merge_nodes.contains(&target.block)
            || self.cfg.rpo_pos[target.block] <= self.cfg.rpo_pos[source]
        {
            let index = Self::resolve_target(&self.ctrl_stack[..], target.block);
            Self::do_blockparam_transfer(
                &target.args[..],
                &self.body.blocks[target.block].params[..],
                into,
            );
            into.push(WasmBlock::Br { target: index });
        } else {
            // Otherwise, we must dominate the block, so just emit it inline.
            debug_assert!(self.cfg.dominates(source, target.block));
            Self::do_blockparam_transfer(
                &target.args[..],
                &self.body.blocks[target.block].params[..],
                into,
            );
            self.process_stack
                .push(StackEntry::DomSubtree(target.block));
        }
    }

    fn do_branch_select(
        &mut self,
        selector: Value,
        targets: &'a [BlockTarget],
        default: &'a BlockTarget,
    ) {
        let into = self.result.last_mut().unwrap();
        log::trace!("do_branch_select: {:?}, default {:?}", targets, default);
        let mut body = vec![WasmBlock::Select {
            selector,
            targets: (0..targets.len())
                .map(|i| WasmLabel::new(i))
                .collect::<Vec<_>>(),
            default: WasmLabel::new(targets.len()),
        }];

        let mut extra = targets.len() + 1;
        for target in targets.iter().chain(std::iter::once(default)) {
            extra -= 1;
            let outer_body = vec![
                WasmBlock::Block {
                    body,
                    out: Block::invalid(),
                },
                WasmBlock::BlockParams {
                    from: &target.args[..],
                    to: &self.body.blocks[target.block].params[..],
                },
                WasmBlock::Br {
                    target: Self::resolve_target(&self.ctrl_stack[..], target.block).add(extra),
                },
            ];
            body = outer_body;
        }

        into.extend(body.into_iter());
    }

    fn do_blockparam_transfer(
        from: &'a [Value],
        to: &'a [(Type, Value)],
        into: &mut Vec<WasmBlock<'a>>,
    ) {
        into.push(WasmBlock::BlockParams { from, to });
    }

    fn finish_block(&mut self, out: Block) {
        self.ctrl_stack.pop();
        let body = self.result.pop().unwrap();
        self.result
            .last_mut()
            .unwrap()
            .push(WasmBlock::Block { body, out });
    }

    fn else_(&mut self) {
        self.result.push(vec![]);
    }

    fn finish_if(&mut self, cond: Value) {
        let else_body = self.result.pop().unwrap();
        let if_body = self.result.pop().unwrap();
        self.ctrl_stack.pop();
        self.result.last_mut().unwrap().push(WasmBlock::If {
            cond,
            if_true: if_body,
            if_false: else_body,
        });
    }

    fn node_within(&mut self, block: Block, merge_node_start: usize) {
        let merge_nodes = self.merge_node_children.last().unwrap();
        log::trace!("node_within: block {} merge_nodes {:?}", block, merge_nodes);
        let merge_nodes = &merge_nodes[merge_node_start..];
        let into = self.result.last_mut().unwrap();

        if let Some(&first) = merge_nodes.first() {
            // Post-`first` body.
            self.process_stack.push(StackEntry::DomSubtree(first));
            // Block with `first` as its out-label (forward label).
            self.ctrl_stack.push(CtrlEntry::Block { out: first });
            self.result.push(vec![]);
            self.process_stack.push(StackEntry::FinishBlock(first));
            self.process_stack
                .push(StackEntry::NodeWithin(block, merge_node_start + 1));
        } else {
            // Leaf node: emit contents!
            into.push(WasmBlock::Leaf { block });
            match &self.body.blocks[block].terminator {
                &Terminator::Br { ref target } => {
                    self.process_stack.push(StackEntry::DoBranch(block, target));
                }
                &Terminator::CondBr {
                    cond,
                    ref if_true,
                    ref if_false,
                } => {
                    self.ctrl_stack.push(CtrlEntry::IfThenElse);
                    self.process_stack.push(StackEntry::FinishIf(cond));
                    self.process_stack
                        .push(StackEntry::DoBranch(block, if_false));
                    self.process_stack.push(StackEntry::Else);
                    self.process_stack
                        .push(StackEntry::DoBranch(block, if_true));
                    self.result.push(vec![]); // if-body
                }
                &Terminator::Select {
                    value,
                    ref targets,
                    ref default,
                } => {
                    self.do_branch_select(value, targets, default);
                }
                &Terminator::Return { ref values } => {
                    into.push(WasmBlock::Return { values });
                }
                &Terminator::Unreachable | &Terminator::None => {
                    into.push(WasmBlock::Unreachable);
                }
            }
        }
    }
}
