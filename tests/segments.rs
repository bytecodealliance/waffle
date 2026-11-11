//! Integration tests for passive and declarative segments in modules
//! whose function bodies are not expanded.

use waffle::wasmparser::{
    DataKind, ElementItems, ElementKind, Operator, Parser, Payload, Validator, WasmFeatures,
};
use waffle::{FrontendOptions, MemorySegment, Module};

/// A module whose code refers to passive data and element segments by
/// index, and takes a reference to a function that only a declarative
/// element segment declares.
///
/// Data segments: 0 active, 1 passive, 2 active.
/// Element segments: 0 active, 1 passive with a null entry, 2 active,
/// 3 declarative, 4 passive with a typed element type and a null entry.
const BULK_WAT: &str = r#"
(module
  (type $t (func))

  (memory 1)
  (table $tab 3 funcref)
  (table $typed 1 (ref null $t))

  (data (i32.const 0) "active-0")
  (data $p1 "passive-1")
  (data (i32.const 16) "active-2")

  (elem (table $tab) (i32.const 0) func $a)
  (elem $e1 funcref (ref.func $a) (ref.null func))
  (elem (table $tab) (i32.const 2) func $a)
  (elem declare func $declared)
  (elem $e4 (ref null $t) (ref.func $a) (ref.null $t))

  (func $a (type $t))
  (func $declared (type $t))
  (func
    (memory.init $p1 (i32.const 0) (i32.const 0) (i32.const 0))
    (table.init $tab $e1 (i32.const 0) (i32.const 0) (i32.const 0))
    (table.init $typed $e4 (i32.const 0) (i32.const 0) (i32.const 0))
    (drop (ref.func $declared))))
"#;

const A: u32 = 0;
const DECLARED: u32 = 1;

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Active,
    Passive,
    Declared,
}

/// The segments of a serialized module, in index order. Element items are
/// function indices, with `None` for `ref.null`.
struct Segments {
    data: Vec<(Mode, Vec<u8>)>,
    data_count: Option<u32>,
    elements: Vec<(Mode, Vec<Option<u32>>)>,
}

fn segments(bytes: &[u8]) -> Segments {
    let mut out = Segments {
        data: vec![],
        data_count: None,
        elements: vec![],
    };
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.unwrap() {
            Payload::DataCountSection { count, .. } => out.data_count = Some(count),
            Payload::DataSection(reader) => {
                for segment in reader {
                    let segment = segment.unwrap();
                    let mode = match segment.kind {
                        DataKind::Active { .. } => Mode::Active,
                        DataKind::Passive => Mode::Passive,
                    };
                    out.data.push((mode, segment.data.to_vec()));
                }
            }
            Payload::ElementSection(reader) => {
                for segment in reader {
                    let segment = segment.unwrap();
                    let mode = match segment.kind {
                        ElementKind::Active { .. } => Mode::Active,
                        ElementKind::Passive => Mode::Passive,
                        ElementKind::Declared => Mode::Declared,
                    };
                    let items = match segment.items {
                        ElementItems::Functions(reader) => {
                            reader.into_iter().map(|f| Some(f.unwrap())).collect()
                        }
                        ElementItems::Expressions(_, reader) => reader
                            .into_iter()
                            .map(|expr| {
                                let mut ops = expr.unwrap().get_operators_reader();
                                match ops.read().unwrap() {
                                    Operator::RefFunc { function_index } => Some(function_index),
                                    Operator::RefNull { .. } => None,
                                    op => panic!("unexpected element expression {:?}", op),
                                }
                            })
                            .collect(),
                    };
                    out.elements.push((mode, items));
                }
            }
            _ => {}
        }
    }
    out
}

fn validate(bytes: &[u8]) {
    if let Err(e) = Validator::new().validate_all(bytes) {
        panic!("output does not validate: {}", e);
    }
}

#[test]
fn unexpanded_bodies_keep_segment_indices() {
    let input = wat::parse_str(BULK_WAT).unwrap();
    let module = Module::from_wasm_bytes(&input, &FrontendOptions::default()).unwrap();
    let output = module.to_wasm_bytes().unwrap();
    validate(&output);

    let out = segments(&output);
    assert_eq!(
        out.data,
        vec![
            (Mode::Active, b"active-0".to_vec()),
            (Mode::Passive, b"passive-1".to_vec()),
            (Mode::Active, b"active-2".to_vec()),
        ]
    );
    assert_eq!(out.data_count, Some(3));
    assert_eq!(out.elements[1], (Mode::Passive, vec![Some(A), None]));
    assert_eq!(out.elements[3], (Mode::Declared, vec![Some(DECLARED)]));
    assert_eq!(out.elements[4], (Mode::Passive, vec![Some(A), None]));
}

#[test]
fn extra_active_segments_follow_the_preserved_ones() {
    let input = wat::parse_str(
        r#"
        (module
          (memory 1)
          (data "passive-0")
          (func (data.drop 0)))
        "#,
    )
    .unwrap();
    let mut module = Module::from_wasm_bytes(&input, &FrontendOptions::default()).unwrap();
    module.memories.values_mut().next().unwrap().segments = vec![MemorySegment {
        offset: 0,
        data: b"new-0".to_vec(),
    }];
    let output = module.to_wasm_bytes().unwrap();
    validate(&output);

    let out = segments(&output);
    assert_eq!(
        out.data,
        vec![
            (Mode::Passive, b"passive-0".to_vec()),
            (Mode::Active, b"new-0".to_vec()),
        ]
    );
    assert_eq!(out.data_count, Some(2));
}

#[test]
fn data_count_section_is_kept_without_passive_segments() {
    let input = wat::parse_str(
        r#"
        (module
          (memory 1)
          (data (i32.const 0) "active-0")
          (func (data.drop 0)))
        "#,
    )
    .unwrap();
    let module = Module::from_wasm_bytes(&input, &FrontendOptions::default()).unwrap();
    let output = module.to_wasm_bytes().unwrap();
    validate(&output);

    let out = segments(&output);
    assert_eq!(out.data_count, Some(1));
    assert_eq!(out.data, vec![(Mode::Active, b"active-0".to_vec())]);
}

/// Parses `bytes` and serializes the module again without expanding any
/// function body.
fn reserialize(bytes: &[u8]) -> Vec<u8> {
    let module = Module::from_wasm_bytes(bytes, &FrontendOptions::default()).unwrap();
    module.to_wasm_bytes().unwrap()
}

#[test]
fn fewer_active_data_segments_keep_original_indices_valid() {
    let input = wat::parse_str(
        r#"
        (module
          (memory 1)
          (data (i32.const 0) "active-0")
          (data (i32.const 16) "active-1")
          (func (data.drop 1)))
        "#,
    )
    .unwrap();
    let mut module = Module::from_wasm_bytes(&input, &FrontendOptions::default()).unwrap();
    module
        .memories
        .values_mut()
        .next()
        .unwrap()
        .segments
        .truncate(1);
    let output = module.to_wasm_bytes().unwrap();
    validate(&output);

    let out = segments(&output);
    assert_eq!(
        out.data,
        vec![
            (Mode::Active, b"active-0".to_vec()),
            (Mode::Passive, vec![]),
        ]
    );
    assert_eq!(out.data_count, Some(2));
}

#[test]
fn fewer_active_data_segments_without_data_count_are_not_padded() {
    let input = wat::parse_str(
        r#"
        (module
          (memory 1)
          (data (i32.const 0) "active-0")
          (data (i32.const 16) "active-1"))
        "#,
    )
    .unwrap();
    let mut module = Module::from_wasm_bytes(&input, &FrontendOptions::default()).unwrap();
    module
        .memories
        .values_mut()
        .next()
        .unwrap()
        .segments
        .truncate(1);
    let output = module.to_wasm_bytes().unwrap();
    validate(&output);

    let out = segments(&output);
    assert_eq!(out.data, vec![(Mode::Active, b"active-0".to_vec())]);
    assert_eq!(out.data_count, None);
}

#[test]
fn element_padding_is_passive_without_a_funcref_table_0() {
    let input = wat::parse_str(
        r#"
        (module
          (type $t (func))
          (table 0 (ref null $t))
          (elem (i32.const 0) (ref null $t)))
        "#,
    )
    .unwrap();
    let mut module = Module::from_wasm_bytes(&input, &FrontendOptions::default()).unwrap();
    // This padding applies to indices without a recorded active segment.
    module.active_element_segments.clear();
    let output = module.to_wasm_bytes().unwrap();
    validate(&output);

    assert_eq!(segments(&output).elements, vec![(Mode::Passive, vec![])]);
}

fn validate_wasm1(bytes: &[u8]) {
    let mut validator = Validator::new_with_features(WasmFeatures::WASM1);
    if let Err(e) = validator.validate_all(bytes) {
        panic!("module does not validate under WasmFeatures::WASM1: {}", e);
    }
}

#[test]
fn element_padding_needs_no_bulk_memory() {
    let input = wat::parse_str(
        "(module (table 1 funcref) (elem (i32.const 0) func 0) (elem (i32.const 0) func 0) (func))",
    )
    .unwrap();
    let output = reserialize(&input);
    validate_wasm1(&output);
    assert_eq!(
        segments(&output).elements.len(),
        segments(&input).elements.len()
    );

    // The last element segment is padding. It uses the 1.0 encoding,
    // which has no table index.
    let mut padding = None;
    for payload in Parser::new(0).parse_all(&output) {
        if let Payload::ElementSection(reader) = payload.unwrap() {
            padding = reader.into_iter().last().map(|segment| segment.unwrap());
        }
    }
    let padding = padding.unwrap();
    assert!(matches!(
        padding.kind,
        ElementKind::Active {
            table_index: None,
            ..
        }
    ));
    match padding.items {
        ElementItems::Functions(reader) => assert_eq!(reader.count(), 0),
        ElementItems::Expressions(..) => panic!("padding uses expressions"),
    }
}

fn validate_typed(bytes: &[u8]) {
    let features = WasmFeatures::WASM2 | WasmFeatures::FUNCTION_REFERENCES;
    if let Err(e) = Validator::new_with_features(features).validate_all(bytes) {
        panic!("module does not validate: {}", e);
    }
}

#[test]
fn indices_keep_the_element_type_of_their_input_segment() {
    let input = wat::parse_str(
        r#"
        (module
          (type $t (func))
          (table $f 1 funcref)
          (table $typed 1 (ref null $t))
          (elem (table $f) (i32.const 0) (ref null $t) (ref.func $g))
          (func $g (type $t)
            (table.init $typed 0 (i32.const 0) (i32.const 0) (i32.const 0))))
        "#,
    )
    .unwrap();
    let output = reserialize(&input);
    validate_typed(&output);

    // Index 0 holds an empty `(ref null $t)` segment on `$f`, and the
    // `funcref` segment for the contents of `$f` follows it.
    assert_eq!(
        segments(&output).elements,
        vec![(Mode::Active, vec![]), (Mode::Active, vec![Some(0)])]
    );
}
