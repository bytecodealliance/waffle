#![no_main]
use libfuzzer_sys::fuzz_target;

use waffle::OptOptions;

fuzz_target!(|module: waffle::fuzzing::SupportedModule| {
    let module = module.0;
    let _ = env_logger::try_init();
    log::debug!("original module: {:?}", module);
    let orig_bytes = module.to_bytes();
    let Some(mut parsed_module) = waffle::fuzzing::parse(&orig_bytes[..]) else {
        return;
    };
    if waffle::fuzzing::skip_unsupported(parsed_module.expand_all_funcs()).is_none() {
        return;
    }
    parsed_module.per_func_body(|body| body.optimize(&OptOptions::default()));
    let _ = parsed_module.to_wasm_bytes();
});
