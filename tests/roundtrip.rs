//! Integration test to ensure that roundtripping works.

use std::path::PathBuf;
use waffle::{FrontendOptions, Module, OptOptions};

fn get_wats() -> Vec<PathBuf> {
    let test_dir = std::env::current_dir()
        .unwrap()
        .join("tests")
        .join("roundtrip");
    let mut ret = vec![];
    for item in std::fs::read_dir(test_dir).unwrap() {
        let path = item.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) == Some("wat") {
            ret.push(path);
        }
    }
    ret.sort(); // Deterministic test order.
    ret
}

#[test]
fn idempotent_roundtrips() {
    for wat in get_wats() {
        let bytes1 = wat::parse_file(&wat).unwrap();
        let opts = FrontendOptions::default();
        let mut module1 = Module::from_wasm_bytes(&bytes1, &opts).unwrap();
        module1.expand_all_funcs().unwrap();
        module1.per_func_body(|func| func.optimize(&OptOptions::default()));
        let bytes2 = module1.to_wasm_bytes().unwrap();
        let mut module2 = Module::from_wasm_bytes(&bytes2, &opts).unwrap();
        module2.expand_all_funcs().unwrap();
        module2.per_func_body(|func| func.optimize(&OptOptions::default()));
        let bytes3 = module2.to_wasm_bytes().unwrap();
        assert_eq!(bytes2, bytes3);
    }
}
