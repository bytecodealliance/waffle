use crate::entity::EntityRef;
use crate::{Block, BlockDef, FunctionBody, Terminator, ValueDef};

pub fn run(body: &mut FunctionBody) {
    log::trace!(
        "merge_blocks: running on func:\n{}\n",
        body.display_verbose("| ", None)
    );

    let mut pred_index = 0;
    while pred_index < body.blocks.len() {
        let pred_id = Block::new(pred_index);
        let pred = &body.blocks[pred_id];
        match &pred.terminator {
            Terminator::Br { target } => {
                let succ_id = target.block;
                let succ = &body.blocks[succ_id];
                if succ.preds.len() == 1 && succ_id != body.entry && succ_id != pred_id {
                    // found two blocks where one jumps to the other without any kind of branching -> merge blocks
                    assert_eq!(pred_id, succ.preds[0]);

                    // alias bbargs
                    assert_eq!(succ.params.len(), target.args.len());
                    for (&(_, from), &to) in succ.params.iter().zip(target.args.iter()) {
                        body.values[from] = ValueDef::Alias(to);
                        body.value_blocks[from] = pred_id;
                    }

                    // update preds of `succ.succs`
                    assert_eq!(succ.succs.len(), succ.pos_in_succ_pred.len());
                    for i in 0..succ.succs.len() {
                        let succ = &body.blocks[succ_id];
                        let succ_succ = succ.succs[i];
                        let pos_in_succ = succ.pos_in_succ_pred[i];
                        body.blocks[succ_succ].preds[pos_in_succ] = pred_id;
                    }

                    // reborrow as mut to move inst and terminator
                    let mut succ =
                        core::mem::replace(&mut body.blocks[succ_id], BlockDef::default());
                    let pred = &mut body.blocks[pred_id];
                    for inst in succ.insts.iter().copied() {
                        body.value_blocks[inst] = pred_id;
                    }
                    pred.insts.append(&mut succ.insts);
                    pred.terminator = succ.terminator;
                    pred.succs = succ.succs;
                    pred.pos_in_succ_pred = succ.pos_in_pred_succ;

                    // do NOT fall through, retry on the same block to inline succ blocks "recursively"
                    continue;
                }
            }
            _ => (),
        }
        pred_index += 1;
    }

    log::trace!(
        "merge_blocks: finished:\n{}\n",
        body.display_verbose("| ", None)
    );
}
