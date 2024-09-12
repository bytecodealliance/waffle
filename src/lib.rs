//! WAFFLE Wasm analysis framework.
//!
//! waffle is a *decompiler and compiler* library for WebAssembly: it
//! defines an SSA-based IR (intermediate representation), with a
//! frontend that translates Wasm to this IR, and a backend that
//! compiles this IR back to Wasm. It can be used by programs that
//! need to transform/modify or add new code to Wasm modules.
//!
//! A good starting point is the `Module`: it can be constructed from
//! Wasm bytecode in memory with `Module::from_wasm_bytes()` and
//! recompiled to Wasm with `Module::to_wasm_bytes()`, after
//! modifications are performed or new code is added. A new module can
//! also be built from scratch with `Module::empty()`.

#![allow(dead_code)]

// Re-export wasmparser for easier use of the right version by our embedders.
pub use wasmparser;
// Likewise for wasm-encoder.
pub use wasm_encoder;

mod backend;
pub mod cfg;
pub mod entity;
mod errors;
mod frontend;
mod ir;
mod op_traits;
mod ops;
pub mod passes;
pub mod pool;
mod scoped_map;

pub use errors::*;
pub use ir::*;
pub use op_traits::SideEffect;
pub use ops::{Ieee32, Ieee64, MemoryArg, Operator};

mod interp;
pub use interp::*;

pub use passes::basic_opt::OptOptions;

#[cfg(feature = "fuzzing")]
pub mod fuzzing;
