//! Fuzzing irreducible control flow handling.
//!
//! 1. Generate a testcase with an arbitrary CFG.
//! 2. Compile it.
//!
//! (That's it.) Showing equivalence of execution is a harder problem
//! and is left as a future exercise.

#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::{fuzz_target, Corpus};
use waffle::{
    entity::PerEntity, Block, BlockTarget, FuncDecl, FunctionBody, Module, SignatureData,
    Terminator, Type,
};

#[derive(Clone, Debug, Arbitrary)]
struct CFG {
    num_blocks: u8,
    edges: Vec<(u8, u8)>,
}

impl CFG {
    fn to_module(&self) -> Option<Module<'_>> {
        let mut module = Module::empty();
        let sig = module.signatures.push(SignatureData {
            params: vec![Type::I32],
            returns: vec![],
        });

        let num_blocks = u32::from(std::cmp::max(1, self.num_blocks));

        let mut body = FunctionBody::new(&module, sig);

        // Entry block0 already present; add the rest.
        for _ in 1..num_blocks {
            let block = body.add_block();
            body.add_blockparam(block, Type::I32);
        }

        let mut edges_by_origin: PerEntity<Block, Vec<Block>> = PerEntity::default();
        for &(from, to) in &self.edges {
            if from >= self.num_blocks || to >= self.num_blocks {
                return None;
            }
            let from = Block::from(u32::from(from));
            let to = Block::from(u32::from(to));
            edges_by_origin[from].push(to);
        }

        for (block, def) in body.blocks.entries_mut() {
            let param = def.params[0].1;
            let dests = &edges_by_origin[block];
            let mut targets = dests
                .iter()
                .map(|&dest| BlockTarget {
                    block: dest,
                    args: vec![param],
                })
                .collect::<Vec<_>>();
            let terminator = match dests.len() {
                0 => Terminator::Return {
                    values: vec![param],
                },
                1 => Terminator::Br {
                    target: targets[0].clone(),
                },
                2 => Terminator::CondBr {
                    cond: param,
                    if_true: targets[0].clone(),
                    if_false: targets[1].clone(),
                },
                _ => {
                    let default = targets.pop().unwrap();
                    Terminator::Select {
                        value: param,
                        targets,
                        default,
                    }
                }
            };
            def.terminator = terminator;
        }

        body.recompute_edges();
        body.validate().unwrap();
        module
            .funcs
            .push(FuncDecl::Body(sig, "func0".to_string(), body));

        Some(module)
    }
}

fuzz_target!(|cfg: CFG| -> Corpus {
    let _ = env_logger::try_init();
    let module = match cfg.to_module() {
        Some(m) => m,
        None => return Corpus::Reject,
    };
    let _ = module.to_wasm_bytes().unwrap();
    Corpus::Keep
});
