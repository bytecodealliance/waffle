#![no_main]
use libfuzzer_sys::fuzz_target;

use waffle::{InterpContext, InterpResult, OptOptions};

fuzz_target!(|module: waffle::fuzzing::ArbitraryModule| {
    let module = module.0;
    let _ = env_logger::try_init();
    log::debug!("original module: {:?}", module);

    let orig_bytes = module.to_bytes();

    if waffle::fuzzing::reject(&orig_bytes[..]) {
        log::debug!("Discarding fuzz run. Body:\n{:?}", module);
        return;
    } else {
        log::info!("body: {:?}", module);
    }

    let Some(mut parsed_module) = waffle::fuzzing::parse(&orig_bytes[..]) else {
        return;
    };
    if waffle::fuzzing::skip_unsupported(parsed_module.expand_all_funcs()).is_none() {
        return;
    }

    let start = parsed_module.start_func.unwrap();

    let mut orig_ctx = match InterpContext::new(&parsed_module) {
        Ok(ctx) => ctx,
        Err(e) => {
            log::trace!("Rejecting due to instantiation error: {:?}", e);
            return;
        }
    };
    orig_ctx.fuel = 10000;

    match orig_ctx.call(&parsed_module, start, &[]) {
        InterpResult::OutOfFuel => {
            // Silently reject.
            log::trace!("Rejecting due to timeout in orig");
            return;
        }
        InterpResult::Trap(..) => {
            // Silently reject.
            log::trace!("Rejecting due to trap in orig");
            return;
        }
        InterpResult::Ok(_) => {}
    }

    let mut opt_module = parsed_module.clone();
    parsed_module.per_func_body(|body| body.optimize(&OptOptions::default()));
    opt_module.per_func_body(|body| body.convert_to_max_ssa(None));

    let mut opt_ctx = InterpContext::new(&opt_module).unwrap();
    // Allow a little leeway for opts to not actually optimize.
    opt_ctx.fuel = 20000;
    opt_ctx.call(&opt_module, start, &[]).ok().unwrap();

    log::trace!(
        "Orig ran in {} fuel; opt ran in {} fuel",
        10000 - orig_ctx.fuel,
        20000 - opt_ctx.fuel
    );

    assert_eq!(orig_ctx.memories, opt_ctx.memories);
    assert_eq!(orig_ctx.globals, opt_ctx.globals);
});
