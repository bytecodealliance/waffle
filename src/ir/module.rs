use super::{
    Func, FuncDecl, Global, Memory, ModuleDisplay, NOPPrintDecorator, PrintDecorator, Signature,
    Table, Type,
};
use crate::entity::{EntityRef, EntityVec};
use crate::ir::{Debug, DebugMap, FunctionBody};
use crate::{backend, frontend};
use anyhow::Result;
use std::collections::BTreeMap;

pub use crate::frontend::FrontendOptions;

/// A Wasm module, represented as a collection of IR entities.
///
/// The module retains a reference to the original Wasm module's bytes
/// in memory, so that function bodies can optionally refer to ranges
/// of bytecode in the original module without parsing, lifting to IR,
/// and recompiling them. A new module may be built without original
/// bytes (i.e., a `Module<'static>` with `orig_bytes: &[]`) using
/// `Module::empty()`.
///
/// The ordinary flow for a tool that processes a Wasm module is:
///
/// - Parse an existing Wasm module using `Module::from_wasm_bytes()`.
/// - For any functions where IR is required, parse the original
///   bytecode into IR using `Module::expand_func()`.
/// - Modify these function bodies (update the `FuncDecl`), and/or
///   append new function bodies to `funcs`.
/// - Compile the IR to a new Wasm module with
///   `Module::to_wasm_bytes()`.
#[derive(Clone, Debug)]
pub struct Module<'a> {
    /// The original Wasm module this module was parsed from, if
    /// any. Used only for "lazy function bodies", which retain a
    /// range that can refer into this slice.
    pub orig_bytes: Option<&'a [u8]>,
    /// The functions in this module: imports, un-expanded ("lazily
    /// parsed") functions, functions as IR, or IR compiled into new
    /// bytecode.
    pub funcs: EntityVec<Func, FuncDecl<'a>>,
    /// Type signatures, referred to by `funcs`, `imports` and
    /// `exports`.
    pub signatures: EntityVec<Signature, SignatureData>,
    /// Global variables in this module.
    pub globals: EntityVec<Global, GlobalData>,
    /// Tables in this module.
    pub tables: EntityVec<Table, TableData>,
    /// Imports into this module. Function imports must also have an
    /// entry at the appropriate function index in `funcs`.
    pub imports: Vec<Import>,
    /// Exports from this module.
    pub exports: Vec<Export>,
    /// Memories/heapds that this module contains.
    pub memories: EntityVec<Memory, MemoryData>,
    /// The "start function" invoked at instantiation, if any.
    pub start_func: Option<Func>,
    /// Debug-info associated with function bodies: interning pools
    /// for source file names and locations in those files.
    pub debug: Debug,
    /// Maps from original Wasm bytecode offsets to source locations.
    pub debug_map: DebugMap,
    /// Other custom sections retained for re-serialization.
    pub custom_sections: BTreeMap<String, &'a [u8]>,
    /// Passive data segments. Active data segments are part of
    /// `MemoryData::segments`.
    pub passive_data_segments: Vec<PassiveDataSegment>,
    /// Passive and declarative element segments. Active element
    /// segments are part of `TableData::func_elements`.
    pub element_segments: Vec<ElementSegment>,
    /// Functions that an active element segment wrote into a table
    /// entry and a later active element segment overwrote. For each
    /// function that nothing else in the output declares, the backend
    /// emits an active element segment that writes the function into
    /// its entry before the segments that write the table's final
    /// contents. The backend ignores records whose index is not below
    /// the length of their table's `func_elements`, and all records of
    /// a table whose `func_elements` is `None`.
    pub overwritten_elements: Vec<OverwrittenElement>,
    /// Whether the module had a DataCount section when it was
    /// parsed. The backend emits a DataCount section when this is set
    /// or when `passive_data_segments` is not empty.
    pub has_data_count: bool,
    /// The number of data segments the module had when it was parsed.
    /// When `has_data_count` is set, the backend emits at least this
    /// many data segments.
    pub orig_data_segment_count: usize,
    /// The number of element segments the module had when it was
    /// parsed. The backend emits at least this many element segments.
    pub orig_element_segment_count: usize,
}

/// A function signature definition.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignatureData {
    /// Parameters: a Wasm function may have zero or more primitive
    /// types as parameters.
    pub params: Vec<Type>,
    /// Returns: a Wasm function (when using the multivalue extension,
    /// which we assume to be present) may have zero or more primitive
    /// types as return values.
    pub returns: Vec<Type>,
}

/// The size of a single Wasm page, used in memory definitions.
pub const WASM_PAGE: usize = 0x1_0000; // 64KiB

/// A memory definition.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryData {
    /// How many Wasm pages (64KiB size) in the initial memory size?
    pub initial_pages: usize,
    /// How many Wasm pages (64KiB size) in the maximum memory size?
    pub maximum_pages: Option<usize>,
    /// Initialization data (initial image) for this memory.
    pub segments: Vec<MemorySegment>,
}

/// A segment of data in a memory's initial state.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemorySegment {
    /// The offset of this data.
    pub offset: usize,
    /// The data, overlaid on previously-existing data at this offset.
    pub data: Vec<u8>,
}

/// A table definition.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableData {
    /// The type of element in this table.
    pub ty: Type,
    /// The initial size (in elements) of this table.
    pub initial: u64,
    /// The maximum size (in elements), if any, of this table.
    pub max: Option<u64>,
    /// If this is a table of function references, the initial
    /// contents of the table. `null` funcrefs are represented by
    /// `Func::invalid()`.
    pub func_elements: Option<Vec<Func>>,
}

/// A passive data segment.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PassiveDataSegment {
    /// The index of this segment in the module's data segment index
    /// space. The backend emits the segment at this index.
    pub index: usize,
    /// The contents of this segment.
    pub data: Vec<u8>,
}

/// The mode of an `ElementSegment`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ElementSegmentKind {
    /// A passive segment, used by `table.init`.
    Passive,
    /// A declarative segment, which declares functions for use by
    /// `ref.func`.
    Declarative,
}

/// A passive or declarative element segment.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementSegment {
    /// The index of this segment in the module's element segment index
    /// space. The backend emits the segment at this index.
    pub index: usize,
    /// Whether this segment is passive or declarative.
    pub kind: ElementSegmentKind,
    /// The type of the segment's elements: `Type::FuncRef` or
    /// `Type::TypedFuncRef`.
    pub ty: Type,
    /// The segment's elements. `null` funcrefs are represented by
    /// `Func::invalid()`.
    pub elements: Vec<Func>,
}

/// A function that an active element segment wrote into a table entry
/// and a later active element segment overwrote.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OverwrittenElement {
    /// The table.
    pub table: Table,
    /// The index of the entry in the table.
    pub index: usize,
    /// The function that the entry held before it was overwritten.
    pub func: Func,
}

/// A global-variable definition.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalData {
    /// The type of this global variable.
    pub ty: Type,
    /// The initial value of this global variable, as a bundle of 64
    /// bits (all primitive types, `i32`/`i64`/`f32`/`f64`, can be
    /// represented in this way).
    pub value: Option<u64>,
    /// Whether this global variable is mutable.
    pub mutable: bool,
}

impl From<&wasmparser::FuncType> for SignatureData {
    fn from(fty: &wasmparser::FuncType) -> Self {
        Self {
            params: fty
                .params()
                .iter()
                .map(|&ty| ty.into())
                .collect::<Vec<Type>>(),
            returns: fty
                .results()
                .iter()
                .map(|&ty| ty.into())
                .collect::<Vec<Type>>(),
        }
    }
}
impl From<wasmparser::FuncType> for SignatureData {
    fn from(fty: wasmparser::FuncType) -> Self {
        (&fty).into()
    }
}

/// A module import definition.
#[derive(Clone, Debug)]
pub struct Import {
    /// The name of the module the import comes from.
    pub module: String,
    /// The name of the export within that module that this import
    /// comes from.
    pub name: String,
    /// The kind of import and its specific entity index.
    pub kind: ImportKind,
}

/// The kind of of a Wasm import, including the specific entity index
/// that the import corresponds to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportKind {
    /// An import of a table.
    Table(Table),
    /// An import of a function.
    Func(Func),
    /// An import of a global.
    Global(Global),
    /// An import of a memory.
    Memory(Memory),
}

impl std::fmt::Display for ImportKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ImportKind::Table(table) => write!(f, "{}", table)?,
            ImportKind::Func(func) => write!(f, "{}", func)?,
            ImportKind::Global(global) => write!(f, "{}", global)?,
            ImportKind::Memory(mem) => write!(f, "{}", mem)?,
        }
        Ok(())
    }
}

/// A module export definition.
#[derive(Clone, Debug)]
pub struct Export {
    /// The name of this export.
    pub name: String,
    /// The kind of export and its specific entity index.
    pub kind: ExportKind,
}

/// The kind of a Wasm export, including the specific entity index
/// that this export directive exports.
#[derive(Clone, Debug)]
pub enum ExportKind {
    /// An export of a table.
    Table(Table),
    /// An export of a function.
    Func(Func),
    /// An export of a global.
    Global(Global),
    /// An export of a memory.
    Memory(Memory),
}

impl std::fmt::Display for ExportKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ExportKind::Table(table) => write!(f, "{}", table)?,
            ExportKind::Func(func) => write!(f, "{}", func)?,
            ExportKind::Global(global) => write!(f, "{}", global)?,
            ExportKind::Memory(memory) => write!(f, "{}", memory)?,
        }
        Ok(())
    }
}

impl<'a> Module<'a> {
    /// Create a new empty Wasm module, ready for entities to be added.
    pub fn empty() -> Module<'static> {
        Module {
            orig_bytes: None,
            funcs: EntityVec::default(),
            signatures: EntityVec::default(),
            globals: EntityVec::default(),
            tables: EntityVec::default(),
            imports: vec![],
            exports: vec![],
            memories: EntityVec::default(),
            start_func: None,
            debug: Debug::default(),
            debug_map: DebugMap::default(),
            custom_sections: BTreeMap::default(),
            passive_data_segments: vec![],
            element_segments: vec![],
            overwritten_elements: vec![],
            has_data_count: false,
            orig_data_segment_count: 0,
            orig_element_segment_count: 0,
        }
    }

    /// Parse a WebAssembly module, as a slice of bytes in memory,
    /// into a waffle Module ready to be manipulated and recompile.
    pub fn from_wasm_bytes(bytes: &'a [u8], options: &FrontendOptions) -> Result<Self> {
        frontend::wasm_to_ir(bytes, options)
    }

    /// Take this module and strip its reference to the original
    /// bytes, producing a module with the same logical contents.
    ///
    /// Note that this has a few side-effects:
    /// - Any (non-debug) custom sections are lost; i.e., they will
    ///   not be roundtripped from the original Wasm module.
    /// - All function bodies are expanded to IR so they can be
    ///   recompiled into new bytecode. The bytecode should be
    ///   equivalent, but will not literally be the same bytecode as the
    ///   original module.
    pub fn without_orig_bytes(self) -> Module<'static> {
        Module {
            orig_bytes: None,
            funcs: EntityVec::from(
                self.funcs
                    .into_vec()
                    .into_iter()
                    .map(|decl| decl.without_orig_bytes())
                    .collect::<Vec<_>>(),
            ),
            signatures: self.signatures,
            globals: self.globals,
            tables: self.tables,
            imports: self.imports,
            exports: self.exports,
            memories: self.memories,
            start_func: self.start_func,
            debug: self.debug,
            debug_map: self.debug_map,
            custom_sections: BTreeMap::default(),
            passive_data_segments: self.passive_data_segments,
            element_segments: self.element_segments,
            overwritten_elements: self.overwritten_elements,
            has_data_count: self.has_data_count,
            orig_data_segment_count: self.orig_data_segment_count,
            orig_element_segment_count: self.orig_element_segment_count,
        }
    }

    /// Compile the module to Wasm bytecode.
    pub fn to_wasm_bytes(&self) -> Result<Vec<u8>> {
        backend::compile(self)
    }

    /// Perform some work on each function body with IR.
    pub fn per_func_body<F: Fn(&mut FunctionBody)>(&mut self, f: F) {
        for func_decl in self.funcs.values_mut() {
            if let Some(body) = func_decl.body_mut() {
                f(body);
            }
        }
    }

    /// Expand a function body, parsing its lazy reference to original
    /// bytecode into IR if needed.
    pub fn expand_func<'b>(&'b mut self, id: Func) -> Result<&'b mut FuncDecl<'a>> {
        if let FuncDecl::Lazy(..) = self.funcs[id] {
            // End the borrow. This is cheap (a slice copy).
            let mut func = self.funcs[id].clone();
            func.parse(self)?;
            self.funcs[id] = func;
        }
        Ok(&mut self.funcs[id])
    }

    /// Clone a function body *without* expanding it, and return a
    /// *new* function body with IR expanded. Useful when a tool
    /// appends new functions that are processed versions of an
    /// original function (which itself must remain as well).
    pub fn clone_and_expand_body(&self, id: Func) -> Result<FunctionBody> {
        let mut body = self.funcs[id].clone();
        body.parse(self)?;
        Ok(match body {
            FuncDecl::Body(_, _, body) => body,
            _ => unreachable!(),
        })
    }

    /// For all functions that are lazy references to initial
    /// bytecode, expand them into IR.
    pub fn expand_all_funcs(&mut self) -> Result<()> {
        for id in 0..self.funcs.len() {
            let id = Func::new(id);
            self.expand_func(id)?;
        }
        Ok(())
    }

    /// Return a wrapper that implements Display on this module,
    /// pretty-printing it as textual IR.
    pub fn display<'b>(&'b self) -> ModuleDisplay<'b, impl PrintDecorator>
    where
        'b: 'a,
    {
        ModuleDisplay::<NOPPrintDecorator> {
            module: self,
            decorators: None,
        }
    }

    /// Return a wrapper that implements Display on this module,
    /// pretty-printing it as textual IR with some additional text whose
    /// printing is described in Decorator.
    pub fn display_with_decorator<'b, PD: PrintDecorator>(
        &'b self,
        decorators: Box<dyn Fn(Func) -> PD>,
    ) -> ModuleDisplay<'b, PD>
    where
        'b: 'a,
    {
        ModuleDisplay {
            module: self,
            decorators: Some(decorators),
        }
    }

    /// Internal (used during parsing): create an empty module, with
    /// the given slice of original Wasm bytecode. Used during parsing
    /// and meant to be filled in as the Wasm bytecode is processed.
    pub(crate) fn with_orig_bytes(orig_bytes: &'a [u8]) -> Module<'a> {
        Module {
            orig_bytes: Some(orig_bytes),
            funcs: EntityVec::default(),
            signatures: EntityVec::default(),
            globals: EntityVec::default(),
            tables: EntityVec::default(),
            imports: vec![],
            exports: vec![],
            memories: EntityVec::default(),
            start_func: None,
            debug: Debug::default(),
            debug_map: DebugMap::default(),
            custom_sections: BTreeMap::default(),
            passive_data_segments: vec![],
            element_segments: vec![],
            overwritten_elements: vec![],
            has_data_count: false,
            orig_data_segment_count: 0,
            orig_element_segment_count: 0,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn empty_module_valid() {
        let module = Module::empty();
        let _ = module.to_wasm_bytes().unwrap();
    }
}
