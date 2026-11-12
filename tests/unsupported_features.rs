//! Tests that the frontend rejects module features that the IR cannot
//! represent.

use waffle::{FrontendError, FrontendOptions, Module};

fn assert_unsupported(wat: &str) {
    let bytes = wat::parse_str(wat).unwrap();
    let err = Module::from_wasm_bytes(&bytes, &FrontendOptions::default())
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
fn memory64_and_shared_memories_are_unsupported() {
    assert_unsupported("(module (memory i64 1))");
    assert_unsupported("(module (import \"\" \"\" (memory 1 1 shared)))");
}

#[test]
fn table64_is_unsupported() {
    assert_unsupported("(module (table i64 1 funcref))");
    assert_unsupported("(module (import \"\" \"\" (table i64 1 funcref)))");
}
