//! Tests for the reference types that the IR represents and the ones
//! the frontend rejects.

use waffle::wasmparser::Validator;
use waffle::{FrontendError, FrontendOptions, Module};

#[test]
fn externref_is_preserved() {
    let bytes = wat::parse_str("(module (func (result externref) (ref.null extern)))").unwrap();
    let module = Module::from_wasm_bytes(&bytes, &FrontendOptions::default()).unwrap();
    Validator::new()
        .validate_all(&module.to_wasm_bytes().unwrap())
        .unwrap();
}

/// Parses `wat` and expands every function body, and asserts that one of
/// the two steps returns `FrontendError::UnsupportedFeature`.
fn assert_unsupported(wat: &str) {
    let bytes = wat::parse_str(wat).unwrap();
    let result = Module::from_wasm_bytes(&bytes, &FrontendOptions::default())
        .and_then(|mut module| module.expand_all_funcs());
    let err = result
        .err()
        .unwrap_or_else(|| panic!("module was accepted: {}", wat));
    assert!(
        matches!(
            err.downcast_ref::<FrontendError>(),
            Some(FrontendError::UnsupportedFeature(_))
        ),
        "unexpected error for {}: {:?}",
        wat,
        err
    );
}

#[test]
fn unrepresentable_reference_types_are_unsupported() {
    assert_unsupported("(module (type (shared (func))))");
    assert_unsupported("(module (func (param (ref func))))");
    assert_unsupported("(module (func (param (ref extern))))");
    assert_unsupported("(module (table 1 anyref))");
    assert_unsupported("(module (import \"\" \"\" (global nullfuncref)))");
    assert_unsupported("(module (func (local i31ref)))");
    assert_unsupported("(module (func (block (result exnref) (unreachable)) (drop)))");
    assert_unsupported("(module (func (drop (ref.null any))))");
}
