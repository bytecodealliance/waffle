//! Tests for tables with an initializer expression.

use waffle::entity::EntityRef;
use waffle::wasmparser::{ElementItems, Parser, Payload, Validator, WasmFeatures};
use waffle::{FrontendError, FrontendOptions, Func, Module, OptOptions, Table};

/// The features that table initializer expressions and `ref.func` in
/// function bodies require.
const FEATURES: WasmFeatures = WasmFeatures::WASM1
    .union(WasmFeatures::REFERENCE_TYPES)
    .union(WasmFeatures::FUNCTION_REFERENCES);

/// Returns the lazy output and the expanded and optimized output for
/// `wat`.
fn outputs(wat: &str) -> [Vec<u8>; 2] {
    let bytes = wat::parse_str(wat).unwrap();
    Validator::new_with_features(FEATURES)
        .validate_all(&bytes)
        .unwrap();
    let opts = FrontendOptions::default();
    let lazy = Module::from_wasm_bytes(&bytes, &opts)
        .unwrap()
        .to_wasm_bytes()
        .unwrap();
    let mut module = Module::from_wasm_bytes(&bytes, &opts).unwrap();
    module.expand_all_funcs().unwrap();
    module.per_func_body(|func| func.optimize(&OptOptions::default()));
    let expanded = module.to_wasm_bytes().unwrap();
    [lazy, expanded]
}

/// Returns the contents of table 0 of `bytes`.
fn table_contents(bytes: &[u8]) -> Vec<Func> {
    let module = Module::from_wasm_bytes(bytes, &FrontendOptions::default()).unwrap();
    module.tables[Table::from(0u32)]
        .func_elements
        .clone()
        .unwrap()
}

/// Returns the number of items in all element segments of `bytes`.
fn element_item_count(bytes: &[u8]) -> u32 {
    let mut count = 0;
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::ElementSection(reader) = payload.unwrap() {
            for element in reader {
                count += match element.unwrap().items {
                    ElementItems::Functions(items) => items.count(),
                    ElementItems::Expressions(_, items) => items.count(),
                };
            }
        }
    }
    count
}

#[test]
fn element_segments_overwrite_table_initializer() {
    let wat = r#"(module
                   (table 4 funcref (ref.func 0))
                   (elem (i32.const 1) funcref (ref.null func) (ref.func 1) (ref.func 2))
                   (elem (i32.const 3) func 0)
                   (func)
                   (func)
                   (func))"#;
    let expected = vec![
        Func::from(0u32),
        Func::invalid(),
        Func::from(1u32),
        Func::from(0u32),
    ];
    for output in outputs(wat) {
        Validator::new_with_features(FEATURES)
            .validate_all(&output)
            .unwrap();
        assert_eq!(table_contents(&output), expected);
        // Entries 1 and 2 differ from the initializer. Entry 3 is written
        // with function 2, which keeps it declared, and then with
        // function 0.
        assert_eq!(element_item_count(&output), 4);
    }
}

#[test]
fn unsupported_table_initializer_is_rejected() {
    let bytes = wat::parse_str(
        r#"(module
             (import "" "" (global funcref))
             (table 1 funcref (global.get 0)))"#,
    )
    .unwrap();
    let err = Module::from_wasm_bytes(&bytes, &FrontendOptions::default())
        .err()
        .expect("module was accepted");
    assert!(
        matches!(
            err.downcast_ref::<FrontendError>(),
            Some(FrontendError::UnsupportedFeature(_))
        ),
        "unexpected error: {:?}",
        err
    );
}
