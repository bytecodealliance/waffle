//! Integration tests for the active element segments that the backend
//! emits for table contents.

use waffle::wasmparser::{ElementItems, ElementKind, Operator, Parser, Payload, Validator};
use waffle::{FrontendOptions, Module, OptOptions};

/// An active element segment as `(table, offset, function indices)`.
/// `ref.null` items are `NULL`.
type Segment = (u32, i32, Vec<u32>);

/// The function index that `active_segments` returns for a `ref.null`
/// item.
const NULL: u32 = u32::MAX;

/// The active element segments of a serialized module, in index order.
fn active_segments(bytes: &[u8]) -> Vec<Segment> {
    let mut out = vec![];
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::ElementSection(reader) = payload.unwrap() {
            for segment in reader {
                let segment = segment.unwrap();
                let (table, offset) = match segment.kind {
                    ElementKind::Active {
                        table_index,
                        offset_expr,
                    } => match offset_expr.get_operators_reader().read().unwrap() {
                        Operator::I32Const { value } => (table_index.unwrap_or(0), value),
                        op => panic!("unexpected offset expression {:?}", op),
                    },
                    _ => panic!("unexpected non-active element segment"),
                };
                let funcs = match segment.items {
                    ElementItems::Functions(reader) => {
                        reader.into_iter().map(|f| f.unwrap()).collect()
                    }
                    ElementItems::Expressions(_, reader) => reader
                        .into_iter()
                        .map(
                            |expr| match expr.unwrap().get_operators_reader().read().unwrap() {
                                Operator::RefFunc { function_index } => function_index,
                                Operator::RefNull { .. } => NULL,
                                op => panic!("unexpected element expression {:?}", op),
                            },
                        )
                        .collect(),
                };
                out.push((table, offset, funcs));
            }
        }
    }
    out
}

/// Parses `input`, expands and optimizes every function body, and
/// serializes the module again. Checks that the output validates.
fn roundtrip(input: &[u8]) -> Vec<u8> {
    let mut module = Module::from_wasm_bytes(input, &FrontendOptions::default()).unwrap();
    module.expand_all_funcs().unwrap();
    module.per_func_body(|func| func.optimize(&OptOptions::default()));
    let output = module.to_wasm_bytes().unwrap();
    if let Err(e) = Validator::new().validate_all(&output) {
        panic!("output does not validate: {}", e);
    }
    output
}

#[test]
fn null_entries_split_segments() {
    let input = wat::parse_str(
        r#"
        (module
          (table 4 funcref)
          (elem (i32.const 0)
            funcref (ref.func 0) (ref.func 1) (ref.null func) (ref.func 0))
          (func)
          (func))
        "#,
    )
    .unwrap();

    let output = roundtrip(&input);
    assert_eq!(
        active_segments(&output),
        vec![(0, 0, vec![0, 1]), (0, 3, vec![0])]
    );
}

#[test]
fn overwritten_table_entries_stay_declared() {
    let input = wat::parse_str(
        r#"
        (module
          (table 1 funcref)
          (elem (i32.const 0) func 0)
          (elem (i32.const 0) func 1)
          (func (result i32) (ref.is_null (ref.func 0)))
          (func))
        "#,
    )
    .unwrap();
    let module = Module::from_wasm_bytes(&input, &FrontendOptions::default()).unwrap();
    let output = module.to_wasm_bytes().unwrap();
    if let Err(e) = Validator::new().validate_all(&output) {
        panic!("output does not validate: {}", e);
    }
    assert_eq!(
        active_segments(&output),
        vec![(0, 0, vec![0]), (0, 0, vec![1])]
    );

    let nulled = wat::parse_str(
        r#"
        (module
          (table 2 funcref)
          (elem (i32.const 0) func 0 1)
          (elem (i32.const 0) funcref (ref.null func) (ref.null func))
          (func)
          (func))
        "#,
    )
    .unwrap();
    assert_eq!(
        active_segments(&roundtrip(&nulled)),
        vec![(0, 0, vec![0, 1]), (0, 0, vec![NULL, NULL])]
    );

    let exported = wat::parse_str(
        r#"
        (module
          (table 1 funcref)
          (elem (i32.const 0) func 0)
          (elem (i32.const 0) func 1)
          (func (export "f"))
          (func))
        "#,
    )
    .unwrap();
    assert_eq!(
        active_segments(&roundtrip(&exported)),
        vec![(0, 0, vec![1]), (0, 0, vec![])]
    );
}

#[test]
fn overwritten_entries_beyond_table_contents_are_not_written() {
    let input = wat::parse_str(
        r#"
        (module
          (table 2 funcref)
          (elem (i32.const 1) func 0)
          (elem (i32.const 1) func 1)
          (func)
          (func))
        "#,
    )
    .unwrap();
    let mut module = Module::from_wasm_bytes(&input, &FrontendOptions::default()).unwrap();
    let table = module.tables.values_mut().next().unwrap();
    table.initial = 1;
    table.func_elements.as_mut().unwrap().truncate(1);
    let output = module.to_wasm_bytes().unwrap();
    if let Err(e) = Validator::new().validate_all(&output) {
        panic!("output does not validate: {}", e);
    }
    assert_eq!(
        active_segments(&output),
        vec![(0, 0, vec![]), (0, 0, vec![])]
    );
}
