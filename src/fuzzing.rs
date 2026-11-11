//! Fuzzing-specific utilities.

use crate::{FrontendError, FrontendOptions, Module};
use libfuzzer_sys::arbitrary;

/// Returns the value in `result`, or `None` if `result` holds a
/// `FrontendError::UnsupportedFeature` or `FrontendError::TooLarge`.
/// Panics on every other error.
pub fn skip_unsupported<T>(result: anyhow::Result<T>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(e) => match e.downcast::<FrontendError>() {
            Ok(FrontendError::UnsupportedFeature(_)) | Ok(FrontendError::TooLarge(_)) => None,
            Ok(e) => panic!("Frontend error: {:?}", e),
            Err(e) => panic!("Other error: {:?}", e),
        },
    }
}

/// Parses `bytes` with `Module::from_wasm_bytes` and default
/// `FrontendOptions`. Errors are handled as in `skip_unsupported`.
pub fn parse(bytes: &[u8]) -> Option<Module<'_>> {
    skip_unsupported(Module::from_wasm_bytes(bytes, &FrontendOptions::default()))
}

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

/// Generates a `wasm_smith::Config` from `u` with GC, exceptions, tail
/// calls, memory64, relaxed SIMD and extended constant expressions turned
/// off.
fn supported_config(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<wasm_smith::Config> {
    let mut config: wasm_smith::Config = u.arbitrary()?;
    config.gc_enabled = false;
    config.exceptions_enabled = false;
    config.tail_call_enabled = false;
    config.memory64_enabled = false;
    config.relaxed_simd_enabled = false;
    config.extended_const_enabled = false;
    Ok(config)
}

/// A `wasm_smith::Module` generated with `supported_config`.
#[derive(Debug)]
pub struct SupportedModule(pub wasm_smith::Module);

impl<'a> arbitrary::Arbitrary<'a> for SupportedModule {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let config = supported_config(u)?;
        Ok(Self(wasm_smith::Module::new(config, u)?))
    }
}

/// Validates `bytes` with `wasmparser::Validator` under the features of
/// the configuration `module` was generated with. Panics with the
/// validator's message if validation fails.
pub fn validate_output(module: &wasm_smith::Module, bytes: &[u8]) {
    let mut validator = wasmparser::Validator::new_with_features(module.config().features());
    if let Err(e) = validator.validate_all(bytes) {
        panic!("Invalid output: {}", e);
    }
}
