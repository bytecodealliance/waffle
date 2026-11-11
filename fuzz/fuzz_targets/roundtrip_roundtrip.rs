#![no_main]
use libfuzzer_sys::fuzz_target;

use waffle::{FrontendOptions, Module};

fuzz_target!(|module: waffle::fuzzing::SupportedModule| {
    let module = module.0;
    let _ = env_logger::try_init();
    log::debug!("original module: {:?}", module);
    let orig_bytes = module.to_bytes();
    let Some(parsed_module) = waffle::fuzzing::parse(&orig_bytes[..]) else {
        return;
    };
    let roundtrip_bytes = parsed_module.to_wasm_bytes().unwrap();
    if let Ok(filename) = std::env::var("ROUNDTRIP_WASM_SAVE") {
        std::fs::write(filename, &roundtrip_bytes[..]).unwrap();
    }
    let parsed_roundtrip_module =
        Module::from_wasm_bytes(&roundtrip_bytes[..], &FrontendOptions::default()).unwrap();
    let _ = parsed_roundtrip_module.to_wasm_bytes();
});
