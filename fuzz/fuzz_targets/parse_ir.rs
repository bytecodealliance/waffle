#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|module: waffle::fuzzing::SupportedModule| {
    let module = module.0;
    let _ = env_logger::try_init();
    let _parsed_module = waffle::fuzzing::parse(&module.to_bytes()[..]);
});
