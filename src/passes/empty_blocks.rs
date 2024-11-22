//! Pass to remove empty blocks.

use crate::entity::EntityRef;
use crate::ir::{Block, BlockTarget, FunctionBody, Terminator};

/// Determines whether a block (i) has no blockparams, and (ii) is
/// solely a jump to another block. We can remove these blocks.
///
/// Why can't we remove blocks that are solely jumps but *do* have
/// blockparams? Because They still serve a purpose in SSA: they
/// define these blockparams as a join of multiple possible other
/// definitions in preds.
fn block_is_empty_jump(body: &FunctionBody, block: Block) -> Option<BlockTarget> {
    // Must be empty except for terminator, and must have no
    // blockparams, and must have an unconditional-branch terminator.
    if body.blocks[block].insts.len() > 0 {
        return None;
    }
    if body.blocks[block].params.len() > 0 {
        return None;
    }
    let target = match &body.blocks[block].terminator {
        &Terminator::Br { ref target } => target,
        _ => return None,
    };

    Some(target.clone())
}

fn rewrite_target(
    forwardings: &[Option<BlockTarget>],
    target: &BlockTarget,
) -> Option<BlockTarget> {
    if target.args.len() > 0 {
        return None;
    }
    forwardings[target.block.index()].clone()
}

pub(crate) fn run(body: &mut FunctionBody) {
    log::trace!(
        "empty_blocks: running on func:\n{}\n",
        body.display_verbose("| ", None)
    );

    // Identify empty blocks, and to where they should forward.
    let forwardings = body
        .blocks
        .iter()
        .map(|block| {
            if block != body.entry {
                block_is_empty_jump(body, block)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // Rewrite every target according to a forwarding (or potentially
    // a chain of composed forwardings).
    for block_data in body.blocks.values_mut() {
        block_data.terminator.update_targets(|target| {
            if let Some(new_target) = rewrite_target(&forwardings[..], target) {
                log::trace!("empty_blocks: replacing {:?} with {:?}", target, new_target);
                *target = new_target;
            }
        });
    }

    // Recompute preds/succs.
    body.recompute_edges();

    log::trace!(
        "empty_blocks: finished:\n{}\n",
        body.display_verbose("| ", None)
    );
}
