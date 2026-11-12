#![no_main]
use libfuzzer_sys::fuzz_target;
use std::sync::atomic::{AtomicU64, Ordering};

use waffle::OptOptions;

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

    let mut config = wasmtime::Config::default();
    config.consume_fuel(true);
    let engine = wasmtime::Engine::new(&config).unwrap();
    let orig_module =
        wasmtime::Module::new(&engine, &orig_bytes[..]).expect("failed to parse original wasm");
    let mut orig_store = wasmtime::Store::new(&engine, ());
    orig_store.set_fuel(10000).unwrap();
    let orig_instance = wasmtime::Instance::new(&mut orig_store, &orig_module, &[]);
    let orig_instance = match orig_instance {
        Ok(orig_instance) => orig_instance,
        Err(e) => {
            log::info!("cannot run start on orig intsance ({:?}); discarding", e);
            return;
        }
    };

    let Some(mut parsed_module) = waffle::fuzzing::parse(&orig_bytes[..]) else {
        return;
    };
    if waffle::fuzzing::skip_unsupported(parsed_module.expand_all_funcs()).is_none() {
        return;
    }
    parsed_module.per_func_body(|body| body.optimize(&OptOptions::default()));
    let roundtrip_bytes = parsed_module.to_wasm_bytes().unwrap();

    if let Ok(filename) = std::env::var("FUZZ_DUMP_WASM") {
        std::fs::write(format!("{}_orig.wasm", filename), &orig_bytes[..]).unwrap();
        std::fs::write(format!("{}_roundtrip.wasm", filename), &roundtrip_bytes[..]).unwrap();
    }

    let total = TOTAL.fetch_add(1, Ordering::Relaxed);

    let roundtrip_module = wasmtime::Module::new(&engine, &roundtrip_bytes[..])
        .expect("failed to parse roundtripped wasm");
    let mut roundtrip_store = wasmtime::Store::new(&engine, ());
    // After roundtrip, fuel consumption rate may differ. That's fine;
    // what matters is that it terminated above without a trap (hence
    // halts in a reasonable time).
    roundtrip_store.set_fuel(u64::MAX).unwrap();
    let roundtrip_instance = wasmtime::Instance::new(&mut roundtrip_store, &roundtrip_module, &[])
        .expect("cannot instantiate roundtripped wasm");

    // Ensure exports are equal.

    let a_globals: Vec<_> = orig_instance
        .exports(&mut orig_store)
        .filter_map(|e| e.into_global())
        .collect();
    let a_globals: Vec<wasmtime::Val> = a_globals
        .into_iter()
        .map(|g| g.get(&mut orig_store))
        .collect();
    let a_mems: Vec<wasmtime::Memory> = orig_instance
        .exports(&mut orig_store)
        .filter_map(|e| e.into_memory())
        .collect();

    let b_globals: Vec<_> = roundtrip_instance
        .exports(&mut roundtrip_store)
        .filter_map(|e| e.into_global())
        .collect();
    let b_globals: Vec<wasmtime::Val> = b_globals
        .into_iter()
        .map(|g| g.get(&mut roundtrip_store))
        .collect();
    let b_mems: Vec<wasmtime::Memory> = roundtrip_instance
        .exports(&mut roundtrip_store)
        .filter_map(|e| e.into_memory())
        .collect();

    log::info!("a_globals = {:?}", a_globals);
    log::info!("b_globals = {:?}", b_globals);

    assert_eq!(a_globals.len(), b_globals.len());
    for (a, b) in a_globals.into_iter().zip(b_globals.into_iter()) {
        match (a, b) {
            (wasmtime::Val::I32(a), wasmtime::Val::I32(b)) => assert_eq!(a, b),
            (wasmtime::Val::I64(a), wasmtime::Val::I64(b)) => assert_eq!(a, b),
            (wasmtime::Val::F32(a), wasmtime::Val::F32(b)) => assert_eq!(a, b),
            (wasmtime::Val::F64(a), wasmtime::Val::F64(b)) => assert_eq!(a, b),
            _ => panic!("mismatching types"),
        }
    }

    assert_eq!(a_mems.len(), b_mems.len());
    for (a, b) in a_mems.into_iter().zip(b_mems.into_iter()) {
        let a_data = a.data(&orig_store);
        let b_data = b.data(&roundtrip_store);
        assert_eq!(a_data, b_data);
    }

    success(total);
});

static TOTAL: AtomicU64 = AtomicU64::new(0);
static SUCCESS: AtomicU64 = AtomicU64::new(0);

fn success(total: u64) {
    let value = SUCCESS.fetch_add(1, Ordering::Relaxed);
    if value % 100 == 0 {
        eprintln!("SUCCESS: {} / TOTAL: {}", value, total);
    }
}
