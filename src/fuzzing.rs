//! Fuzzing-specific utilities.

use libfuzzer_sys::arbitrary;

/// Should this module be rejected early during fuzzing due to an
/// unsupported feature?
///
/// Public/exported only for access by fuzzers.
pub fn reject(bytes: &[u8]) -> bool {
    let parser = wasmparser::Parser::new(0);
    let mut has_start = false;
    let mut has_global_set = false;
    let mut num_globals = 0;
    for payload in parser.parse_all(bytes) {
        match payload.unwrap() {
            wasmparser::Payload::CodeSectionEntry(body) => {
                for op in body.get_operators_reader().unwrap() {
                    let op = op.unwrap();
                    match op {
                        wasmparser::Operator::GlobalSet { .. } => {
                            has_global_set = true;
                        }
                        _ => {}
                    }
                }
            }
            wasmparser::Payload::StartSection { .. } => {
                has_start = true;
            }
            wasmparser::Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.unwrap();
                    match &export.kind {
                        &wasmparser::ExternalKind::Global => {
                            num_globals += 1;
                        }
                        _ => {}
                    }
                }
            }
            wasmparser::Payload::MemorySection(reader) => {
                for mem in reader {
                    let mem = mem.unwrap();
                    if mem.maximum.is_none() || mem.maximum.unwrap() > 100 {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }

    if !has_start || !has_global_set || num_globals < 1 {
        return true;
    }

    false
}

/// Get the configuration that we expect fuzzing targets to use to
/// generate modules with `wasm_smith`.
fn fuzzing_config() -> wasm_smith::Config {
    wasm_smith::Config {
        min_funcs: 1,
        max_funcs: 1,
        min_memories: 1,
        max_memories: 1,
        min_globals: 10,
        max_globals: 10,
        min_tables: 0,
        max_tables: 0,
        min_imports: 0,
        max_imports: 0,
        min_exports: 12,
        max_exports: 12,
        allow_start_export: true,
        canonicalize_nans: true,
        max_memory32_bytes: 1 << 16,
        ..Default::default()
    }
}

/// A wrapper around `Module` that uses `arbitrary` to generate new
/// modules.
#[derive(Debug)]
pub struct ArbitraryModule(pub wasm_smith::Module);

impl<'a> arbitrary::Arbitrary<'a> for ArbitraryModule {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        Ok(Self(wasm_smith::Module::new(fuzzing_config(), u)?))
    }
}
