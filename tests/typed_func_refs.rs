//! Tests for the declared types of locals that hold function references.

use waffle::wasmparser::{Validator, WasmFeatures};
use waffle::{FrontendOptions, Module, OptOptions};

fn expanded_roundtrip(wat: &str) -> Vec<u8> {
    let bytes = wat::parse_str(wat).unwrap();
    let mut module = Module::from_wasm_bytes(&bytes, &FrontendOptions::default()).unwrap();
    module.expand_all_funcs().unwrap();
    module.per_func_body(|func| func.optimize(&OptOptions::default()));
    module.to_wasm_bytes().unwrap()
}

#[test]
fn ref_func_without_function_references() {
    let bytes = expanded_roundtrip(
        r#"(module
             (table 1 funcref)
             (elem (i32.const 0) func 0)
             (func (drop (ref.func 0))))"#,
    );
    Validator::new_with_features(WasmFeatures::WASM2)
        .validate_all(&bytes)
        .unwrap();
}
