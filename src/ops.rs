//! Operators.
//!
//! An operator is an entity in the IR that accepts some SSA value(s)
//! (or none) as inputs, and produces SSA value(s) (or none) as
//! outputs, when executed. The `Operator` itself contains all of its
//! arguments: for example, a constant value operator (say
//! `Operator::I32Const`) contains the constant value as a field of
//! the enum arm.
//!
//! The set of operators supported by waffle is meant to be 1-to-1
//! with Wasm: an operator represents and has the semantics of the
//! Wasm opcode of the same name. The only exceptions are around
//! features of Wasm bytecode that waffle "lifts" into its own SSA IR:
//! accesses to Wasm locals (these become the SSA dataflow itself) and
//! control flow (these become `Terminator` instructions).

use crate::{entity::EntityRef, Func, Global, Memory, Signature, Table, Type};
use std::convert::{TryFrom, TryInto};
pub use wasmparser::{Ieee32, Ieee64};

/// An argument to a memory load or store, specifying which memory,
/// alignment and an optional offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MemoryArg {
    pub align: u32,
    pub offset: u32,
    pub memory: Memory,
}

impl std::fmt::Display for MemoryArg {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}, align={}, offset={}",
            self.memory, self.align, self.offset
        )
    }
}

/// An operator in the IR, consuming arguments and producing results
/// when executed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operator {
    Unreachable,
    Nop,

    Call {
        function_index: Func,
    },
    CallIndirect {
        sig_index: Signature,
        table_index: Table,
    },
    Select,
    TypedSelect {
        ty: Type,
    },
    GlobalGet {
        global_index: Global,
    },
    GlobalSet {
        global_index: Global,
    },

    I32Load {
        memory: MemoryArg,
    },
    I64Load {
        memory: MemoryArg,
    },
    F32Load {
        memory: MemoryArg,
    },
    F64Load {
        memory: MemoryArg,
    },
    I32Load8S {
        memory: MemoryArg,
    },
    I32Load8U {
        memory: MemoryArg,
    },
    I32Load16S {
        memory: MemoryArg,
    },
    I32Load16U {
        memory: MemoryArg,
    },
    I64Load8S {
        memory: MemoryArg,
    },
    I64Load8U {
        memory: MemoryArg,
    },
    I64Load16S {
        memory: MemoryArg,
    },
    I64Load16U {
        memory: MemoryArg,
    },
    I64Load32S {
        memory: MemoryArg,
    },
    I64Load32U {
        memory: MemoryArg,
    },

    I32Store {
        memory: MemoryArg,
    },
    I64Store {
        memory: MemoryArg,
    },
    F32Store {
        memory: MemoryArg,
    },
    F64Store {
        memory: MemoryArg,
    },
    I32Store8 {
        memory: MemoryArg,
    },
    I32Store16 {
        memory: MemoryArg,
    },
    I64Store8 {
        memory: MemoryArg,
    },
    I64Store16 {
        memory: MemoryArg,
    },
    I64Store32 {
        memory: MemoryArg,
    },

    I32Const {
        value: u32,
    },
    I64Const {
        value: u64,
    },
    F32Const {
        value: u32,
    },
    F64Const {
        value: u64,
    },

    I32Eqz,
    I32Eq,
    I32Ne,
    I32LtS,
    I32LtU,
    I32GtS,
    I32GtU,
    I32LeS,
    I32LeU,
    I32GeS,
    I32GeU,

    I64Eqz,

    I64Eq,
    I64Ne,
    I64LtS,
    I64LtU,
    I64GtU,
    I64GtS,
    I64LeS,
    I64LeU,
    I64GeS,
    I64GeU,

    F32Eq,
    F32Ne,
    F32Lt,
    F32Gt,
    F32Le,
    F32Ge,

    F64Eq,
    F64Ne,
    F64Lt,
    F64Gt,
    F64Le,
    F64Ge,

    I32Clz,
    I32Ctz,
    I32Popcnt,

    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32DivU,
    I32RemS,
    I32RemU,
    I32And,
    I32Or,
    I32Xor,
    I32Shl,
    I32ShrS,
    I32ShrU,
    I32Rotl,
    I32Rotr,

    I64Clz,
    I64Ctz,
    I64Popcnt,

    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64DivU,
    I64RemS,
    I64RemU,
    I64And,
    I64Or,
    I64Xor,
    I64Shl,
    I64ShrS,
    I64ShrU,
    I64Rotl,
    I64Rotr,

    F32Abs,
    F32Neg,
    F32Ceil,
    F32Floor,
    F32Trunc,
    F32Nearest,
    F32Sqrt,

    F32Add,
    F32Sub,
    F32Mul,
    F32Div,
    F32Min,
    F32Max,
    F32Copysign,

    F64Abs,
    F64Neg,
    F64Ceil,
    F64Floor,
    F64Trunc,
    F64Nearest,
    F64Sqrt,

    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Min,
    F64Max,
    F64Copysign,

    I32WrapI64,
    I32TruncF32S,
    I32TruncF32U,
    I32TruncF64S,
    I32TruncF64U,
    I64ExtendI32S,
    I64ExtendI32U,
    I64TruncF32S,
    I64TruncF32U,
    I64TruncF64S,
    I64TruncF64U,
    F32ConvertI32S,
    F32ConvertI32U,
    F32ConvertI64S,
    F32ConvertI64U,
    F32DemoteF64,
    F64ConvertI32S,
    F64ConvertI32U,
    F64ConvertI64S,
    F64ConvertI64U,
    F64PromoteF32,
    I32Extend8S,
    I32Extend16S,
    I64Extend8S,
    I64Extend16S,
    I64Extend32S,
    I32TruncSatF32S,
    I32TruncSatF32U,
    I32TruncSatF64S,
    I32TruncSatF64U,
    I64TruncSatF32S,
    I64TruncSatF32U,
    I64TruncSatF64S,
    I64TruncSatF64U,
    F32ReinterpretI32,
    F64ReinterpretI64,
    I32ReinterpretF32,
    I64ReinterpretF64,
    TableGet {
        table_index: Table,
    },
    TableSet {
        table_index: Table,
    },
    TableGrow {
        table_index: Table,
    },
    TableSize {
        table_index: Table,
    },
    MemorySize {
        mem: Memory,
    },
    MemoryGrow {
        mem: Memory,
    },
    MemoryCopy {
        dst_mem: Memory,
        src_mem: Memory,
    },
    MemoryFill {
        mem: Memory,
    },

    V128Load {
        memory: MemoryArg,
    },
    V128Load8x8S {
        memory: MemoryArg,
    },
    V128Load8x8U {
        memory: MemoryArg,
    },
    V128Load16x4S {
        memory: MemoryArg,
    },
    V128Load16x4U {
        memory: MemoryArg,
    },
    V128Load32x2S {
        memory: MemoryArg,
    },
    V128Load32x2U {
        memory: MemoryArg,
    },
    V128Load8Splat {
        memory: MemoryArg,
    },
    V128Load16Splat {
        memory: MemoryArg,
    },
    V128Load32Splat {
        memory: MemoryArg,
    },
    V128Load64Splat {
        memory: MemoryArg,
    },
    V128Load32Zero {
        memory: MemoryArg,
    },
    V128Load64Zero {
        memory: MemoryArg,
    },
    V128Store {
        memory: MemoryArg,
    },
    V128Load8Lane {
        memory: MemoryArg,
        lane: u8,
    },
    V128Load16Lane {
        memory: MemoryArg,
        lane: u8,
    },
    V128Load32Lane {
        memory: MemoryArg,
        lane: u8,
    },
    V128Load64Lane {
        memory: MemoryArg,
        lane: u8,
    },
    V128Store8Lane {
        memory: MemoryArg,
        lane: u8,
    },
    V128Store16Lane {
        memory: MemoryArg,
        lane: u8,
    },
    V128Store32Lane {
        memory: MemoryArg,
        lane: u8,
    },
    V128Store64Lane {
        memory: MemoryArg,
        lane: u8,
    },

    V128Const {
        value: u128,
    },

    I8x16Shuffle {
        lanes: [u8; 16],
    },

    I8x16ExtractLaneS {
        lane: u8,
    },
    I8x16ExtractLaneU {
        lane: u8,
    },
    I8x16ReplaceLane {
        lane: u8,
    },
    I16x8ExtractLaneS {
        lane: u8,
    },
    I16x8ExtractLaneU {
        lane: u8,
    },
    I16x8ReplaceLane {
        lane: u8,
    },
    I32x4ExtractLane {
        lane: u8,
    },
    I32x4ReplaceLane {
        lane: u8,
    },
    I64x2ExtractLane {
        lane: u8,
    },
    I64x2ReplaceLane {
        lane: u8,
    },
    F32x4ExtractLane {
        lane: u8,
    },
    F32x4ReplaceLane {
        lane: u8,
    },
    F64x2ExtractLane {
        lane: u8,
    },
    F64x2ReplaceLane {
        lane: u8,
    },

    I8x16Swizzle,
    I8x16Splat,
    I16x8Splat,
    I32x4Splat,
    I64x2Splat,
    F32x4Splat,
    F64x2Splat,

    I8x16Eq,
    I8x16Ne,
    I8x16LtS,
    I8x16LtU,
    I8x16GtS,
    I8x16GtU,
    I8x16LeS,
    I8x16LeU,
    I8x16GeS,
    I8x16GeU,

    I16x8Eq,
    I16x8Ne,
    I16x8LtS,
    I16x8LtU,
    I16x8GtS,
    I16x8GtU,
    I16x8LeS,
    I16x8LeU,
    I16x8GeS,
    I16x8GeU,

    I32x4Eq,
    I32x4Ne,
    I32x4LtS,
    I32x4LtU,
    I32x4GtS,
    I32x4GtU,
    I32x4LeS,
    I32x4LeU,
    I32x4GeS,
    I32x4GeU,

    I64x2Eq,
    I64x2Ne,
    I64x2LtS,
    I64x2GtS,
    I64x2LeS,
    I64x2GeS,

    F32x4Eq,
    F32x4Ne,
    F32x4Lt,
    F32x4Gt,
    F32x4Le,
    F32x4Ge,

    F64x2Eq,
    F64x2Ne,
    F64x2Lt,
    F64x2Gt,
    F64x2Le,
    F64x2Ge,

    V128Not,
    V128And,
    V128AndNot,
    V128Or,
    V128Xor,
    V128Bitselect,
    V128AnyTrue,

    I8x16Abs,
    I8x16Neg,
    I8x16Popcnt,
    I8x16AllTrue,
    I8x16Bitmask,
    I8x16NarrowI16x8S,
    I8x16NarrowI16x8U,
    I8x16Shl,
    I8x16ShrS,
    I8x16ShrU,
    I8x16Add,
    I8x16AddSatS,
    I8x16AddSatU,
    I8x16Sub,
    I8x16SubSatS,
    I8x16SubSatU,
    I8x16MinS,
    I8x16MinU,
    I8x16MaxS,
    I8x16MaxU,
    I8x16AvgrU,

    I16x8ExtAddPairwiseI8x16S,
    I16x8ExtAddPairwiseI8x16U,
    I16x8Abs,
    I16x8Neg,
    I16x8Q15MulrSatS,
    I16x8AllTrue,
    I16x8Bitmask,
    I16x8NarrowI32x4S,
    I16x8NarrowI32x4U,
    I16x8ExtendLowI8x16S,
    I16x8ExtendHighI8x16S,
    I16x8ExtendLowI8x16U,
    I16x8ExtendHighI8x16U,
    I16x8Shl,
    I16x8ShrS,
    I16x8ShrU,
    I16x8Add,
    I16x8AddSatS,
    I16x8AddSatU,
    I16x8Sub,
    I16x8SubSatS,
    I16x8SubSatU,
    I16x8Mul,
    I16x8MinS,
    I16x8MinU,
    I16x8MaxS,
    I16x8MaxU,
    I16x8AvgrU,
    I16x8ExtMulLowI8x16S,
    I16x8ExtMulHighI8x16S,
    I16x8ExtMulLowI8x16U,
    I16x8ExtMulHighI8x16U,
    I32x4ExtAddPairwiseI16x8S,
    I32x4ExtAddPairwiseI16x8U,

    I32x4Abs,
    I32x4Neg,
    I32x4AllTrue,
    I32x4Bitmask,
    I32x4ExtendLowI16x8S,
    I32x4ExtendHighI16x8S,
    I32x4ExtendLowI16x8U,
    I32x4ExtendHighI16x8U,
    I32x4Shl,
    I32x4ShrS,
    I32x4ShrU,
    I32x4Add,
    I32x4Sub,
    I32x4Mul,
    I32x4MinS,
    I32x4MinU,
    I32x4MaxS,
    I32x4MaxU,
    I32x4DotI16x8S,
    I32x4ExtMulLowI16x8S,
    I32x4ExtMulHighI16x8S,
    I32x4ExtMulLowI16x8U,
    I32x4ExtMulHighI16x8U,
    I64x2Abs,
    I64x2Neg,
    I64x2AllTrue,
    I64x2Bitmask,
    I64x2ExtendLowI32x4S,
    I64x2ExtendHighI32x4S,
    I64x2ExtendLowI32x4U,
    I64x2ExtendHighI32x4U,
    I64x2Shl,
    I64x2ShrS,
    I64x2ShrU,
    I64x2Add,
    I64x2Sub,
    I64x2Mul,
    I64x2ExtMulLowI32x4S,
    I64x2ExtMulHighI32x4S,
    I64x2ExtMulLowI32x4U,
    I64x2ExtMulHighI32x4U,
    F32x4Ceil,
    F32x4Floor,
    F32x4Trunc,
    F32x4Nearest,
    F32x4Abs,
    F32x4Neg,
    F32x4Sqrt,
    F32x4Add,
    F32x4Sub,
    F32x4Mul,
    F32x4Div,
    F32x4Min,
    F32x4Max,
    F32x4PMin,
    F32x4PMax,
    F64x2Ceil,
    F64x2Floor,
    F64x2Trunc,
    F64x2Nearest,
    F64x2Abs,
    F64x2Neg,
    F64x2Sqrt,
    F64x2Add,
    F64x2Sub,
    F64x2Mul,
    F64x2Div,
    F64x2Min,
    F64x2Max,
    F64x2PMin,
    F64x2PMax,
    I32x4TruncSatF32x4S,
    I32x4TruncSatF32x4U,
    F32x4ConvertI32x4S,
    F32x4ConvertI32x4U,
    I32x4TruncSatF64x2SZero,
    I32x4TruncSatF64x2UZero,
    F64x2ConvertLowI32x4S,
    F64x2ConvertLowI32x4U,
    F32x4DemoteF64x2Zero,
    F64x2PromoteLowF32x4,

    CallRef {
        sig_index: Signature,
    },
    RefIsNull,
    /// `ty` is the result type: `FuncRef`, `ExternRef`, or a nullable
    /// `TypedFuncRef`.
    RefNull {
        ty: Type,
    },
    RefFunc {
        func_index: Func,
    },
}

#[test]
fn op_size() {
    assert_eq!(std::mem::size_of::<Operator>(), 32);
}

impl<'a, 'b> std::convert::TryFrom<&'b wasmparser::Operator<'a>> for Operator {
    type Error = ();

    fn try_from(op: &'b wasmparser::Operator<'a>) -> Result<Operator, Self::Error> {
        match op {
            &wasmparser::Operator::Unreachable => Ok(Operator::Unreachable),
            &wasmparser::Operator::Nop => Ok(Operator::Nop),
            &wasmparser::Operator::Call { function_index } => Ok(Operator::Call {
                function_index: Func::from(function_index),
            }),
            &wasmparser::Operator::CallIndirect {
                type_index,
                table_index,
                ..
            } => Ok(Operator::CallIndirect {
                sig_index: Signature::from(type_index),
                table_index: Table::from(table_index),
            }),
            &wasmparser::Operator::LocalSet { .. } => Err(()),
            &wasmparser::Operator::LocalTee { .. } => Err(()),
            &wasmparser::Operator::LocalGet { .. } => Err(()),
            &wasmparser::Operator::Select => Ok(Operator::Select),
            &wasmparser::Operator::TypedSelect { ty } => Ok(Operator::TypedSelect {
                ty: ty.try_into().map_err(|_| ())?,
            }),
            &wasmparser::Operator::GlobalGet { global_index } => Ok(Operator::GlobalGet {
                global_index: Global::from(global_index),
            }),
            &wasmparser::Operator::GlobalSet { global_index } => Ok(Operator::GlobalSet {
                global_index: Global::from(global_index),
            }),
            &wasmparser::Operator::I32Load { memarg } => Ok(Operator::I32Load {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I64Load { memarg } => Ok(Operator::I64Load {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::F32Load { memarg } => Ok(Operator::F32Load {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::F64Load { memarg } => Ok(Operator::F64Load {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I32Load8S { memarg } => Ok(Operator::I32Load8S {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I32Load8U { memarg } => Ok(Operator::I32Load8U {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I32Load16S { memarg } => Ok(Operator::I32Load16S {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I32Load16U { memarg } => Ok(Operator::I32Load16U {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I64Load8S { memarg } => Ok(Operator::I64Load8S {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I64Load8U { memarg } => Ok(Operator::I64Load8U {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I64Load16S { memarg } => Ok(Operator::I64Load16S {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I64Load16U { memarg } => Ok(Operator::I64Load16U {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I64Load32S { memarg } => Ok(Operator::I64Load32S {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I64Load32U { memarg } => Ok(Operator::I64Load32U {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I32Store { memarg } => Ok(Operator::I32Store {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I64Store { memarg } => Ok(Operator::I64Store {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::F32Store { memarg } => Ok(Operator::F32Store {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::F64Store { memarg } => Ok(Operator::F64Store {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I32Store8 { memarg } => Ok(Operator::I32Store8 {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I32Store16 { memarg } => Ok(Operator::I32Store16 {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I64Store8 { memarg } => Ok(Operator::I64Store8 {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I64Store16 { memarg } => Ok(Operator::I64Store16 {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I64Store32 { memarg } => Ok(Operator::I64Store32 {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::I32Const { value } => Ok(Operator::I32Const {
                value: value as u32,
            }),
            &wasmparser::Operator::I64Const { value } => Ok(Operator::I64Const {
                value: value as u64,
            }),
            &wasmparser::Operator::F32Const { value } => Ok(Operator::F32Const {
                value: value.bits(),
            }),
            &wasmparser::Operator::F64Const { value } => Ok(Operator::F64Const {
                value: value.bits(),
            }),
            &wasmparser::Operator::I32Eqz => Ok(Operator::I32Eqz),
            &wasmparser::Operator::I32Eq => Ok(Operator::I32Eq),
            &wasmparser::Operator::I32Ne => Ok(Operator::I32Ne),
            &wasmparser::Operator::I32LtS => Ok(Operator::I32LtS),
            &wasmparser::Operator::I32LtU => Ok(Operator::I32LtU),
            &wasmparser::Operator::I32GtS => Ok(Operator::I32GtS),
            &wasmparser::Operator::I32GtU => Ok(Operator::I32GtU),
            &wasmparser::Operator::I32LeS => Ok(Operator::I32LeS),
            &wasmparser::Operator::I32LeU => Ok(Operator::I32LeU),
            &wasmparser::Operator::I32GeS => Ok(Operator::I32GeS),
            &wasmparser::Operator::I32GeU => Ok(Operator::I32GeU),
            &wasmparser::Operator::I64Eqz => Ok(Operator::I64Eqz),
            &wasmparser::Operator::I64Eq => Ok(Operator::I64Eq),
            &wasmparser::Operator::I64Ne => Ok(Operator::I64Ne),
            &wasmparser::Operator::I64LtS => Ok(Operator::I64LtS),
            &wasmparser::Operator::I64LtU => Ok(Operator::I64LtU),
            &wasmparser::Operator::I64GtU => Ok(Operator::I64GtU),
            &wasmparser::Operator::I64GtS => Ok(Operator::I64GtS),
            &wasmparser::Operator::I64LeS => Ok(Operator::I64LeS),
            &wasmparser::Operator::I64LeU => Ok(Operator::I64LeU),
            &wasmparser::Operator::I64GeS => Ok(Operator::I64GeS),
            &wasmparser::Operator::I64GeU => Ok(Operator::I64GeU),
            &wasmparser::Operator::F32Eq => Ok(Operator::F32Eq),
            &wasmparser::Operator::F32Ne => Ok(Operator::F32Ne),
            &wasmparser::Operator::F32Lt => Ok(Operator::F32Lt),
            &wasmparser::Operator::F32Gt => Ok(Operator::F32Gt),
            &wasmparser::Operator::F32Le => Ok(Operator::F32Le),
            &wasmparser::Operator::F32Ge => Ok(Operator::F32Ge),
            &wasmparser::Operator::F64Eq => Ok(Operator::F64Eq),
            &wasmparser::Operator::F64Ne => Ok(Operator::F64Ne),
            &wasmparser::Operator::F64Lt => Ok(Operator::F64Lt),
            &wasmparser::Operator::F64Gt => Ok(Operator::F64Gt),
            &wasmparser::Operator::F64Le => Ok(Operator::F64Le),
            &wasmparser::Operator::F64Ge => Ok(Operator::F64Ge),
            &wasmparser::Operator::I32Clz => Ok(Operator::I32Clz),
            &wasmparser::Operator::I32Ctz => Ok(Operator::I32Ctz),
            &wasmparser::Operator::I32Popcnt => Ok(Operator::I32Popcnt),
            &wasmparser::Operator::I32Add => Ok(Operator::I32Add),
            &wasmparser::Operator::I32Sub => Ok(Operator::I32Sub),
            &wasmparser::Operator::I32Mul => Ok(Operator::I32Mul),
            &wasmparser::Operator::I32DivS => Ok(Operator::I32DivS),
            &wasmparser::Operator::I32DivU => Ok(Operator::I32DivU),
            &wasmparser::Operator::I32RemS => Ok(Operator::I32RemS),
            &wasmparser::Operator::I32RemU => Ok(Operator::I32RemU),
            &wasmparser::Operator::I32And => Ok(Operator::I32And),
            &wasmparser::Operator::I32Or => Ok(Operator::I32Or),
            &wasmparser::Operator::I32Xor => Ok(Operator::I32Xor),
            &wasmparser::Operator::I32Shl => Ok(Operator::I32Shl),
            &wasmparser::Operator::I32ShrS => Ok(Operator::I32ShrS),
            &wasmparser::Operator::I32ShrU => Ok(Operator::I32ShrU),
            &wasmparser::Operator::I32Rotl => Ok(Operator::I32Rotl),
            &wasmparser::Operator::I32Rotr => Ok(Operator::I32Rotr),
            &wasmparser::Operator::I64Clz => Ok(Operator::I64Clz),
            &wasmparser::Operator::I64Ctz => Ok(Operator::I64Ctz),
            &wasmparser::Operator::I64Popcnt => Ok(Operator::I64Popcnt),
            &wasmparser::Operator::I64Add => Ok(Operator::I64Add),
            &wasmparser::Operator::I64Sub => Ok(Operator::I64Sub),
            &wasmparser::Operator::I64Mul => Ok(Operator::I64Mul),
            &wasmparser::Operator::I64DivS => Ok(Operator::I64DivS),
            &wasmparser::Operator::I64DivU => Ok(Operator::I64DivU),
            &wasmparser::Operator::I64RemS => Ok(Operator::I64RemS),
            &wasmparser::Operator::I64RemU => Ok(Operator::I64RemU),
            &wasmparser::Operator::I64And => Ok(Operator::I64And),
            &wasmparser::Operator::I64Or => Ok(Operator::I64Or),
            &wasmparser::Operator::I64Xor => Ok(Operator::I64Xor),
            &wasmparser::Operator::I64Shl => Ok(Operator::I64Shl),
            &wasmparser::Operator::I64ShrS => Ok(Operator::I64ShrS),
            &wasmparser::Operator::I64ShrU => Ok(Operator::I64ShrU),
            &wasmparser::Operator::I64Rotl => Ok(Operator::I64Rotl),
            &wasmparser::Operator::I64Rotr => Ok(Operator::I64Rotr),
            &wasmparser::Operator::F32Abs => Ok(Operator::F32Abs),
            &wasmparser::Operator::F32Neg => Ok(Operator::F32Neg),
            &wasmparser::Operator::F32Ceil => Ok(Operator::F32Ceil),
            &wasmparser::Operator::F32Floor => Ok(Operator::F32Floor),
            &wasmparser::Operator::F32Trunc => Ok(Operator::F32Trunc),
            &wasmparser::Operator::F32Nearest => Ok(Operator::F32Nearest),
            &wasmparser::Operator::F32Sqrt => Ok(Operator::F32Sqrt),
            &wasmparser::Operator::F32Add => Ok(Operator::F32Add),
            &wasmparser::Operator::F32Sub => Ok(Operator::F32Sub),
            &wasmparser::Operator::F32Mul => Ok(Operator::F32Mul),
            &wasmparser::Operator::F32Div => Ok(Operator::F32Div),
            &wasmparser::Operator::F32Min => Ok(Operator::F32Min),
            &wasmparser::Operator::F32Max => Ok(Operator::F32Max),
            &wasmparser::Operator::F32Copysign => Ok(Operator::F32Copysign),
            &wasmparser::Operator::F64Abs => Ok(Operator::F64Abs),
            &wasmparser::Operator::F64Neg => Ok(Operator::F64Neg),
            &wasmparser::Operator::F64Ceil => Ok(Operator::F64Ceil),
            &wasmparser::Operator::F64Floor => Ok(Operator::F64Floor),
            &wasmparser::Operator::F64Trunc => Ok(Operator::F64Trunc),
            &wasmparser::Operator::F64Nearest => Ok(Operator::F64Nearest),
            &wasmparser::Operator::F64Sqrt => Ok(Operator::F64Sqrt),
            &wasmparser::Operator::F64Add => Ok(Operator::F64Add),
            &wasmparser::Operator::F64Sub => Ok(Operator::F64Sub),
            &wasmparser::Operator::F64Mul => Ok(Operator::F64Mul),
            &wasmparser::Operator::F64Div => Ok(Operator::F64Div),
            &wasmparser::Operator::F64Min => Ok(Operator::F64Min),
            &wasmparser::Operator::F64Max => Ok(Operator::F64Max),
            &wasmparser::Operator::F64Copysign => Ok(Operator::F64Copysign),
            &wasmparser::Operator::I32WrapI64 => Ok(Operator::I32WrapI64),
            &wasmparser::Operator::I32TruncF32S => Ok(Operator::I32TruncF32S),
            &wasmparser::Operator::I32TruncF32U => Ok(Operator::I32TruncF32U),
            &wasmparser::Operator::I32TruncF64S => Ok(Operator::I32TruncF64S),
            &wasmparser::Operator::I32TruncF64U => Ok(Operator::I32TruncF64U),
            &wasmparser::Operator::I64ExtendI32S => Ok(Operator::I64ExtendI32S),
            &wasmparser::Operator::I64ExtendI32U => Ok(Operator::I64ExtendI32U),
            &wasmparser::Operator::I64TruncF32S => Ok(Operator::I64TruncF32S),
            &wasmparser::Operator::I64TruncF32U => Ok(Operator::I64TruncF32U),
            &wasmparser::Operator::I64TruncF64S => Ok(Operator::I64TruncF64S),
            &wasmparser::Operator::I64TruncF64U => Ok(Operator::I64TruncF64U),
            &wasmparser::Operator::F32ConvertI32S => Ok(Operator::F32ConvertI32S),
            &wasmparser::Operator::F32ConvertI32U => Ok(Operator::F32ConvertI32U),
            &wasmparser::Operator::F32ConvertI64S => Ok(Operator::F32ConvertI64S),
            &wasmparser::Operator::F32ConvertI64U => Ok(Operator::F32ConvertI64U),
            &wasmparser::Operator::F32DemoteF64 => Ok(Operator::F32DemoteF64),
            &wasmparser::Operator::F64ConvertI32S => Ok(Operator::F64ConvertI32S),
            &wasmparser::Operator::F64ConvertI32U => Ok(Operator::F64ConvertI32U),
            &wasmparser::Operator::F64ConvertI64S => Ok(Operator::F64ConvertI64S),
            &wasmparser::Operator::F64ConvertI64U => Ok(Operator::F64ConvertI64U),
            &wasmparser::Operator::F64PromoteF32 => Ok(Operator::F64PromoteF32),
            &wasmparser::Operator::I32Extend8S => Ok(Operator::I32Extend8S),
            &wasmparser::Operator::I32Extend16S => Ok(Operator::I32Extend16S),
            &wasmparser::Operator::I64Extend8S => Ok(Operator::I64Extend8S),
            &wasmparser::Operator::I64Extend16S => Ok(Operator::I64Extend16S),
            &wasmparser::Operator::I64Extend32S => Ok(Operator::I64Extend32S),
            &wasmparser::Operator::I32TruncSatF32S => Ok(Operator::I32TruncSatF32S),
            &wasmparser::Operator::I32TruncSatF32U => Ok(Operator::I32TruncSatF32U),
            &wasmparser::Operator::I32TruncSatF64S => Ok(Operator::I32TruncSatF64S),
            &wasmparser::Operator::I32TruncSatF64U => Ok(Operator::I32TruncSatF64U),
            &wasmparser::Operator::I64TruncSatF32S => Ok(Operator::I64TruncSatF32S),
            &wasmparser::Operator::I64TruncSatF32U => Ok(Operator::I64TruncSatF32U),
            &wasmparser::Operator::I64TruncSatF64S => Ok(Operator::I64TruncSatF64S),
            &wasmparser::Operator::I64TruncSatF64U => Ok(Operator::I64TruncSatF64U),
            &wasmparser::Operator::F32ReinterpretI32 => Ok(Operator::F32ReinterpretI32),
            &wasmparser::Operator::F64ReinterpretI64 => Ok(Operator::F64ReinterpretI64),
            &wasmparser::Operator::I32ReinterpretF32 => Ok(Operator::I32ReinterpretF32),
            &wasmparser::Operator::I64ReinterpretF64 => Ok(Operator::I64ReinterpretF64),
            &wasmparser::Operator::TableGet { table } => Ok(Operator::TableGet {
                table_index: Table::from(table),
            }),
            &wasmparser::Operator::TableSet { table } => Ok(Operator::TableSet {
                table_index: Table::from(table),
            }),
            &wasmparser::Operator::TableGrow { table } => Ok(Operator::TableGrow {
                table_index: Table::from(table),
            }),
            &wasmparser::Operator::TableSize { table } => Ok(Operator::TableSize {
                table_index: Table::from(table),
            }),
            &wasmparser::Operator::MemorySize { mem, .. } => Ok(Operator::MemorySize {
                mem: Memory::from(mem),
            }),
            &wasmparser::Operator::MemoryGrow { mem, .. } => Ok(Operator::MemoryGrow {
                mem: Memory::from(mem),
            }),
            &wasmparser::Operator::MemoryCopy { dst_mem, src_mem } => Ok(Operator::MemoryCopy {
                dst_mem: Memory::from(dst_mem),
                src_mem: Memory::from(src_mem),
            }),
            &wasmparser::Operator::MemoryFill { mem } => Ok(Operator::MemoryFill {
                mem: Memory::from(mem),
            }),

            &wasmparser::Operator::V128Load { memarg } => Ok(Operator::V128Load {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load8x8S { memarg } => Ok(Operator::V128Load8x8S {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load8x8U { memarg } => Ok(Operator::V128Load8x8U {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load16x4S { memarg } => Ok(Operator::V128Load16x4S {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load16x4U { memarg } => Ok(Operator::V128Load16x4U {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load32x2S { memarg } => Ok(Operator::V128Load32x2S {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load32x2U { memarg } => Ok(Operator::V128Load32x2U {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load8Splat { memarg } => Ok(Operator::V128Load8Splat {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load16Splat { memarg } => Ok(Operator::V128Load16Splat {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load32Splat { memarg } => Ok(Operator::V128Load32Splat {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load64Splat { memarg } => Ok(Operator::V128Load64Splat {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load32Zero { memarg } => Ok(Operator::V128Load32Zero {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load64Zero { memarg } => Ok(Operator::V128Load64Zero {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Store { memarg } => Ok(Operator::V128Store {
                memory: memarg.into(),
            }),
            &wasmparser::Operator::V128Load8Lane { memarg, lane } => Ok(Operator::V128Load8Lane {
                memory: memarg.into(),
                lane,
            }),
            &wasmparser::Operator::V128Load16Lane { memarg, lane } => {
                Ok(Operator::V128Load16Lane {
                    memory: memarg.into(),
                    lane,
                })
            }
            &wasmparser::Operator::V128Load32Lane { memarg, lane } => {
                Ok(Operator::V128Load32Lane {
                    memory: memarg.into(),
                    lane,
                })
            }
            &wasmparser::Operator::V128Load64Lane { memarg, lane } => {
                Ok(Operator::V128Load64Lane {
                    memory: memarg.into(),
                    lane,
                })
            }
            &wasmparser::Operator::V128Store8Lane { memarg, lane } => {
                Ok(Operator::V128Store8Lane {
                    memory: memarg.into(),
                    lane,
                })
            }
            &wasmparser::Operator::V128Store16Lane { memarg, lane } => {
                Ok(Operator::V128Store16Lane {
                    memory: memarg.into(),
                    lane,
                })
            }
            &wasmparser::Operator::V128Store32Lane { memarg, lane } => {
                Ok(Operator::V128Store32Lane {
                    memory: memarg.into(),
                    lane,
                })
            }
            &wasmparser::Operator::V128Store64Lane { memarg, lane } => {
                Ok(Operator::V128Store64Lane {
                    memory: memarg.into(),
                    lane,
                })
            }

            &wasmparser::Operator::V128Const { value } => Ok(Operator::V128Const {
                value: value.i128() as u128,
            }),

            &wasmparser::Operator::I8x16Shuffle { lanes } => Ok(Operator::I8x16Shuffle { lanes }),

            &wasmparser::Operator::I8x16ExtractLaneS { lane } => {
                Ok(Operator::I8x16ExtractLaneS { lane })
            }
            &wasmparser::Operator::I8x16ExtractLaneU { lane } => {
                Ok(Operator::I8x16ExtractLaneU { lane })
            }
            &wasmparser::Operator::I8x16ReplaceLane { lane } => {
                Ok(Operator::I8x16ReplaceLane { lane })
            }
            &wasmparser::Operator::I16x8ExtractLaneS { lane } => {
                Ok(Operator::I16x8ExtractLaneS { lane })
            }
            &wasmparser::Operator::I16x8ExtractLaneU { lane } => {
                Ok(Operator::I16x8ExtractLaneU { lane })
            }
            &wasmparser::Operator::I16x8ReplaceLane { lane } => {
                Ok(Operator::I16x8ReplaceLane { lane })
            }
            &wasmparser::Operator::I32x4ExtractLane { lane } => {
                Ok(Operator::I32x4ExtractLane { lane })
            }
            &wasmparser::Operator::I32x4ReplaceLane { lane } => {
                Ok(Operator::I32x4ReplaceLane { lane })
            }
            &wasmparser::Operator::I64x2ExtractLane { lane } => {
                Ok(Operator::I64x2ExtractLane { lane })
            }
            &wasmparser::Operator::I64x2ReplaceLane { lane } => {
                Ok(Operator::I64x2ReplaceLane { lane })
            }
            &wasmparser::Operator::F32x4ExtractLane { lane } => {
                Ok(Operator::F32x4ExtractLane { lane })
            }
            &wasmparser::Operator::F32x4ReplaceLane { lane } => {
                Ok(Operator::F32x4ReplaceLane { lane })
            }
            &wasmparser::Operator::F64x2ExtractLane { lane } => {
                Ok(Operator::F64x2ExtractLane { lane })
            }
            &wasmparser::Operator::F64x2ReplaceLane { lane } => {
                Ok(Operator::F64x2ReplaceLane { lane })
            }

            &wasmparser::Operator::I8x16Swizzle => Ok(Operator::I8x16Swizzle),
            &wasmparser::Operator::I8x16Splat => Ok(Operator::I8x16Splat),
            &wasmparser::Operator::I16x8Splat => Ok(Operator::I16x8Splat),
            &wasmparser::Operator::I32x4Splat => Ok(Operator::I32x4Splat),
            &wasmparser::Operator::I64x2Splat => Ok(Operator::I64x2Splat),
            &wasmparser::Operator::F32x4Splat => Ok(Operator::F32x4Splat),
            &wasmparser::Operator::F64x2Splat => Ok(Operator::F64x2Splat),

            &wasmparser::Operator::I8x16Eq => Ok(Operator::I8x16Eq),
            &wasmparser::Operator::I8x16Ne => Ok(Operator::I8x16Ne),
            &wasmparser::Operator::I8x16LtS => Ok(Operator::I8x16LtS),
            &wasmparser::Operator::I8x16LtU => Ok(Operator::I8x16LtU),
            &wasmparser::Operator::I8x16GtS => Ok(Operator::I8x16GtS),
            &wasmparser::Operator::I8x16GtU => Ok(Operator::I8x16GtU),
            &wasmparser::Operator::I8x16LeS => Ok(Operator::I8x16LeS),
            &wasmparser::Operator::I8x16LeU => Ok(Operator::I8x16LeU),
            &wasmparser::Operator::I8x16GeS => Ok(Operator::I8x16GeS),
            &wasmparser::Operator::I8x16GeU => Ok(Operator::I8x16GeU),

            &wasmparser::Operator::I16x8Eq => Ok(Operator::I16x8Eq),
            &wasmparser::Operator::I16x8Ne => Ok(Operator::I16x8Ne),
            &wasmparser::Operator::I16x8LtS => Ok(Operator::I16x8LtS),
            &wasmparser::Operator::I16x8LtU => Ok(Operator::I16x8LtU),
            &wasmparser::Operator::I16x8GtS => Ok(Operator::I16x8GtS),
            &wasmparser::Operator::I16x8GtU => Ok(Operator::I16x8GtU),
            &wasmparser::Operator::I16x8LeS => Ok(Operator::I16x8LeS),
            &wasmparser::Operator::I16x8LeU => Ok(Operator::I16x8LeU),
            &wasmparser::Operator::I16x8GeS => Ok(Operator::I16x8GeS),
            &wasmparser::Operator::I16x8GeU => Ok(Operator::I16x8GeU),

            &wasmparser::Operator::I32x4Eq => Ok(Operator::I32x4Eq),
            &wasmparser::Operator::I32x4Ne => Ok(Operator::I32x4Ne),
            &wasmparser::Operator::I32x4LtS => Ok(Operator::I32x4LtS),
            &wasmparser::Operator::I32x4LtU => Ok(Operator::I32x4LtU),
            &wasmparser::Operator::I32x4GtS => Ok(Operator::I32x4GtS),
            &wasmparser::Operator::I32x4GtU => Ok(Operator::I32x4GtU),
            &wasmparser::Operator::I32x4LeS => Ok(Operator::I32x4LeS),
            &wasmparser::Operator::I32x4LeU => Ok(Operator::I32x4LeU),
            &wasmparser::Operator::I32x4GeS => Ok(Operator::I32x4GeS),
            &wasmparser::Operator::I32x4GeU => Ok(Operator::I32x4GeU),

            &wasmparser::Operator::I64x2Eq => Ok(Operator::I64x2Eq),
            &wasmparser::Operator::I64x2Ne => Ok(Operator::I64x2Ne),
            &wasmparser::Operator::I64x2LtS => Ok(Operator::I64x2LtS),
            &wasmparser::Operator::I64x2GtS => Ok(Operator::I64x2GtS),
            &wasmparser::Operator::I64x2LeS => Ok(Operator::I64x2LeS),
            &wasmparser::Operator::I64x2GeS => Ok(Operator::I64x2GeS),

            &wasmparser::Operator::F32x4Eq => Ok(Operator::F32x4Eq),
            &wasmparser::Operator::F32x4Ne => Ok(Operator::F32x4Ne),
            &wasmparser::Operator::F32x4Lt => Ok(Operator::F32x4Lt),
            &wasmparser::Operator::F32x4Gt => Ok(Operator::F32x4Gt),
            &wasmparser::Operator::F32x4Le => Ok(Operator::F32x4Le),
            &wasmparser::Operator::F32x4Ge => Ok(Operator::F32x4Ge),

            &wasmparser::Operator::F64x2Eq => Ok(Operator::F64x2Eq),
            &wasmparser::Operator::F64x2Ne => Ok(Operator::F64x2Ne),
            &wasmparser::Operator::F64x2Lt => Ok(Operator::F64x2Lt),
            &wasmparser::Operator::F64x2Gt => Ok(Operator::F64x2Gt),
            &wasmparser::Operator::F64x2Le => Ok(Operator::F64x2Le),
            &wasmparser::Operator::F64x2Ge => Ok(Operator::F64x2Ge),

            &wasmparser::Operator::V128Not => Ok(Operator::V128Not),
            &wasmparser::Operator::V128And => Ok(Operator::V128And),
            &wasmparser::Operator::V128AndNot => Ok(Operator::V128AndNot),
            &wasmparser::Operator::V128Or => Ok(Operator::V128Or),
            &wasmparser::Operator::V128Xor => Ok(Operator::V128Xor),
            &wasmparser::Operator::V128Bitselect => Ok(Operator::V128Bitselect),
            &wasmparser::Operator::V128AnyTrue => Ok(Operator::V128AnyTrue),

            &wasmparser::Operator::I8x16Abs => Ok(Operator::I8x16Abs),
            &wasmparser::Operator::I8x16Neg => Ok(Operator::I8x16Neg),
            &wasmparser::Operator::I8x16Popcnt => Ok(Operator::I8x16Popcnt),
            &wasmparser::Operator::I8x16AllTrue => Ok(Operator::I8x16AllTrue),
            &wasmparser::Operator::I8x16Bitmask => Ok(Operator::I8x16Bitmask),
            &wasmparser::Operator::I8x16NarrowI16x8S => Ok(Operator::I8x16NarrowI16x8S),
            &wasmparser::Operator::I8x16NarrowI16x8U => Ok(Operator::I8x16NarrowI16x8U),
            &wasmparser::Operator::I8x16Shl => Ok(Operator::I8x16Shl),
            &wasmparser::Operator::I8x16ShrS => Ok(Operator::I8x16ShrS),
            &wasmparser::Operator::I8x16ShrU => Ok(Operator::I8x16ShrU),
            &wasmparser::Operator::I8x16Add => Ok(Operator::I8x16Add),
            &wasmparser::Operator::I8x16AddSatS => Ok(Operator::I8x16AddSatS),
            &wasmparser::Operator::I8x16AddSatU => Ok(Operator::I8x16AddSatU),
            &wasmparser::Operator::I8x16Sub => Ok(Operator::I8x16Sub),
            &wasmparser::Operator::I8x16SubSatS => Ok(Operator::I8x16SubSatS),
            &wasmparser::Operator::I8x16SubSatU => Ok(Operator::I8x16SubSatU),
            &wasmparser::Operator::I8x16MinS => Ok(Operator::I8x16MinS),
            &wasmparser::Operator::I8x16MinU => Ok(Operator::I8x16MinU),
            &wasmparser::Operator::I8x16MaxS => Ok(Operator::I8x16MaxS),
            &wasmparser::Operator::I8x16MaxU => Ok(Operator::I8x16MaxU),
            &wasmparser::Operator::I8x16AvgrU => Ok(Operator::I8x16AvgrU),

            &wasmparser::Operator::I16x8ExtAddPairwiseI8x16S => {
                Ok(Operator::I16x8ExtAddPairwiseI8x16S)
            }
            &wasmparser::Operator::I16x8ExtAddPairwiseI8x16U => {
                Ok(Operator::I16x8ExtAddPairwiseI8x16U)
            }
            &wasmparser::Operator::I16x8Abs => Ok(Operator::I16x8Abs),
            &wasmparser::Operator::I16x8Neg => Ok(Operator::I16x8Neg),
            &wasmparser::Operator::I16x8Q15MulrSatS => Ok(Operator::I16x8Q15MulrSatS),
            &wasmparser::Operator::I16x8AllTrue => Ok(Operator::I16x8AllTrue),
            &wasmparser::Operator::I16x8Bitmask => Ok(Operator::I16x8Bitmask),
            &wasmparser::Operator::I16x8NarrowI32x4S => Ok(Operator::I16x8NarrowI32x4S),
            &wasmparser::Operator::I16x8NarrowI32x4U => Ok(Operator::I16x8NarrowI32x4U),
            &wasmparser::Operator::I16x8ExtendLowI8x16S => Ok(Operator::I16x8ExtendLowI8x16S),
            &wasmparser::Operator::I16x8ExtendHighI8x16S => Ok(Operator::I16x8ExtendHighI8x16S),
            &wasmparser::Operator::I16x8ExtendLowI8x16U => Ok(Operator::I16x8ExtendLowI8x16U),
            &wasmparser::Operator::I16x8ExtendHighI8x16U => Ok(Operator::I16x8ExtendHighI8x16U),
            &wasmparser::Operator::I16x8Shl => Ok(Operator::I16x8Shl),
            &wasmparser::Operator::I16x8ShrS => Ok(Operator::I16x8ShrS),
            &wasmparser::Operator::I16x8ShrU => Ok(Operator::I16x8ShrU),
            &wasmparser::Operator::I16x8Add => Ok(Operator::I16x8Add),
            &wasmparser::Operator::I16x8AddSatS => Ok(Operator::I16x8AddSatS),
            &wasmparser::Operator::I16x8AddSatU => Ok(Operator::I16x8AddSatU),
            &wasmparser::Operator::I16x8Sub => Ok(Operator::I16x8Sub),
            &wasmparser::Operator::I16x8SubSatS => Ok(Operator::I16x8SubSatS),
            &wasmparser::Operator::I16x8SubSatU => Ok(Operator::I16x8SubSatU),
            &wasmparser::Operator::I16x8Mul => Ok(Operator::I16x8Mul),
            &wasmparser::Operator::I16x8MinS => Ok(Operator::I16x8MinS),
            &wasmparser::Operator::I16x8MinU => Ok(Operator::I16x8MinU),
            &wasmparser::Operator::I16x8MaxS => Ok(Operator::I16x8MaxS),
            &wasmparser::Operator::I16x8MaxU => Ok(Operator::I16x8MaxU),
            &wasmparser::Operator::I16x8AvgrU => Ok(Operator::I16x8AvgrU),
            &wasmparser::Operator::I16x8ExtMulLowI8x16S => Ok(Operator::I16x8ExtMulLowI8x16S),
            &wasmparser::Operator::I16x8ExtMulHighI8x16S => Ok(Operator::I16x8ExtMulHighI8x16S),
            &wasmparser::Operator::I16x8ExtMulLowI8x16U => Ok(Operator::I16x8ExtMulLowI8x16U),
            &wasmparser::Operator::I16x8ExtMulHighI8x16U => Ok(Operator::I16x8ExtMulHighI8x16U),
            &wasmparser::Operator::I32x4ExtAddPairwiseI16x8S => {
                Ok(Operator::I32x4ExtAddPairwiseI16x8S)
            }
            &wasmparser::Operator::I32x4ExtAddPairwiseI16x8U => {
                Ok(Operator::I32x4ExtAddPairwiseI16x8U)
            }

            &wasmparser::Operator::I32x4Abs => Ok(Operator::I32x4Abs),
            &wasmparser::Operator::I32x4Neg => Ok(Operator::I32x4Neg),
            &wasmparser::Operator::I32x4AllTrue => Ok(Operator::I32x4AllTrue),
            &wasmparser::Operator::I32x4Bitmask => Ok(Operator::I32x4Bitmask),
            &wasmparser::Operator::I32x4ExtendLowI16x8S => Ok(Operator::I32x4ExtendLowI16x8S),
            &wasmparser::Operator::I32x4ExtendHighI16x8S => Ok(Operator::I32x4ExtendHighI16x8S),
            &wasmparser::Operator::I32x4ExtendLowI16x8U => Ok(Operator::I32x4ExtendLowI16x8U),
            &wasmparser::Operator::I32x4ExtendHighI16x8U => Ok(Operator::I32x4ExtendHighI16x8U),
            &wasmparser::Operator::I32x4Shl => Ok(Operator::I32x4Shl),
            &wasmparser::Operator::I32x4ShrS => Ok(Operator::I32x4ShrS),
            &wasmparser::Operator::I32x4ShrU => Ok(Operator::I32x4ShrU),
            &wasmparser::Operator::I32x4Add => Ok(Operator::I32x4Add),
            &wasmparser::Operator::I32x4Sub => Ok(Operator::I32x4Sub),
            &wasmparser::Operator::I32x4Mul => Ok(Operator::I32x4Mul),
            &wasmparser::Operator::I32x4MinS => Ok(Operator::I32x4MinS),
            &wasmparser::Operator::I32x4MinU => Ok(Operator::I32x4MinU),
            &wasmparser::Operator::I32x4MaxS => Ok(Operator::I32x4MaxS),
            &wasmparser::Operator::I32x4MaxU => Ok(Operator::I32x4MaxU),
            &wasmparser::Operator::I32x4DotI16x8S => Ok(Operator::I32x4DotI16x8S),
            &wasmparser::Operator::I32x4ExtMulLowI16x8S => Ok(Operator::I32x4ExtMulLowI16x8S),
            &wasmparser::Operator::I32x4ExtMulHighI16x8S => Ok(Operator::I32x4ExtMulHighI16x8S),
            &wasmparser::Operator::I32x4ExtMulLowI16x8U => Ok(Operator::I32x4ExtMulLowI16x8U),
            &wasmparser::Operator::I32x4ExtMulHighI16x8U => Ok(Operator::I32x4ExtMulHighI16x8U),
            &wasmparser::Operator::I64x2Abs => Ok(Operator::I64x2Abs),
            &wasmparser::Operator::I64x2Neg => Ok(Operator::I64x2Neg),
            &wasmparser::Operator::I64x2AllTrue => Ok(Operator::I64x2AllTrue),
            &wasmparser::Operator::I64x2Bitmask => Ok(Operator::I64x2Bitmask),
            &wasmparser::Operator::I64x2ExtendLowI32x4S => Ok(Operator::I64x2ExtendLowI32x4S),
            &wasmparser::Operator::I64x2ExtendHighI32x4S => Ok(Operator::I64x2ExtendHighI32x4S),
            &wasmparser::Operator::I64x2ExtendLowI32x4U => Ok(Operator::I64x2ExtendLowI32x4U),
            &wasmparser::Operator::I64x2ExtendHighI32x4U => Ok(Operator::I64x2ExtendHighI32x4U),
            &wasmparser::Operator::I64x2Shl => Ok(Operator::I64x2Shl),
            &wasmparser::Operator::I64x2ShrS => Ok(Operator::I64x2ShrS),
            &wasmparser::Operator::I64x2ShrU => Ok(Operator::I64x2ShrU),
            &wasmparser::Operator::I64x2Add => Ok(Operator::I64x2Add),
            &wasmparser::Operator::I64x2Sub => Ok(Operator::I64x2Sub),
            &wasmparser::Operator::I64x2Mul => Ok(Operator::I64x2Mul),
            &wasmparser::Operator::I64x2ExtMulLowI32x4S => Ok(Operator::I64x2ExtMulLowI32x4S),
            &wasmparser::Operator::I64x2ExtMulHighI32x4S => Ok(Operator::I64x2ExtMulHighI32x4S),
            &wasmparser::Operator::I64x2ExtMulLowI32x4U => Ok(Operator::I64x2ExtMulLowI32x4U),
            &wasmparser::Operator::I64x2ExtMulHighI32x4U => Ok(Operator::I64x2ExtMulHighI32x4U),
            &wasmparser::Operator::F32x4Ceil => Ok(Operator::F32x4Ceil),
            &wasmparser::Operator::F32x4Floor => Ok(Operator::F32x4Floor),
            &wasmparser::Operator::F32x4Trunc => Ok(Operator::F32x4Trunc),
            &wasmparser::Operator::F32x4Nearest => Ok(Operator::F32x4Nearest),
            &wasmparser::Operator::F32x4Abs => Ok(Operator::F32x4Abs),
            &wasmparser::Operator::F32x4Neg => Ok(Operator::F32x4Neg),
            &wasmparser::Operator::F32x4Sqrt => Ok(Operator::F32x4Sqrt),
            &wasmparser::Operator::F32x4Add => Ok(Operator::F32x4Add),
            &wasmparser::Operator::F32x4Sub => Ok(Operator::F32x4Sub),
            &wasmparser::Operator::F32x4Mul => Ok(Operator::F32x4Mul),
            &wasmparser::Operator::F32x4Div => Ok(Operator::F32x4Div),
            &wasmparser::Operator::F32x4Min => Ok(Operator::F32x4Min),
            &wasmparser::Operator::F32x4Max => Ok(Operator::F32x4Max),
            &wasmparser::Operator::F32x4PMin => Ok(Operator::F32x4PMin),
            &wasmparser::Operator::F32x4PMax => Ok(Operator::F32x4PMax),
            &wasmparser::Operator::F64x2Ceil => Ok(Operator::F64x2Ceil),
            &wasmparser::Operator::F64x2Floor => Ok(Operator::F64x2Floor),
            &wasmparser::Operator::F64x2Trunc => Ok(Operator::F64x2Trunc),
            &wasmparser::Operator::F64x2Nearest => Ok(Operator::F64x2Nearest),
            &wasmparser::Operator::F64x2Abs => Ok(Operator::F64x2Abs),
            &wasmparser::Operator::F64x2Neg => Ok(Operator::F64x2Neg),
            &wasmparser::Operator::F64x2Sqrt => Ok(Operator::F64x2Sqrt),
            &wasmparser::Operator::F64x2Add => Ok(Operator::F64x2Add),
            &wasmparser::Operator::F64x2Sub => Ok(Operator::F64x2Sub),
            &wasmparser::Operator::F64x2Mul => Ok(Operator::F64x2Mul),
            &wasmparser::Operator::F64x2Div => Ok(Operator::F64x2Div),
            &wasmparser::Operator::F64x2Min => Ok(Operator::F64x2Min),
            &wasmparser::Operator::F64x2Max => Ok(Operator::F64x2Max),
            &wasmparser::Operator::F64x2PMin => Ok(Operator::F64x2PMin),
            &wasmparser::Operator::F64x2PMax => Ok(Operator::F64x2PMax),
            &wasmparser::Operator::I32x4TruncSatF32x4S => Ok(Operator::I32x4TruncSatF32x4S),
            &wasmparser::Operator::I32x4TruncSatF32x4U => Ok(Operator::I32x4TruncSatF32x4U),
            &wasmparser::Operator::F32x4ConvertI32x4S => Ok(Operator::F32x4ConvertI32x4S),
            &wasmparser::Operator::F32x4ConvertI32x4U => Ok(Operator::F32x4ConvertI32x4U),
            &wasmparser::Operator::I32x4TruncSatF64x2SZero => Ok(Operator::I32x4TruncSatF64x2SZero),
            &wasmparser::Operator::I32x4TruncSatF64x2UZero => Ok(Operator::I32x4TruncSatF64x2UZero),
            &wasmparser::Operator::F64x2ConvertLowI32x4S => Ok(Operator::F64x2ConvertLowI32x4S),
            &wasmparser::Operator::F64x2ConvertLowI32x4U => Ok(Operator::F64x2ConvertLowI32x4U),
            &wasmparser::Operator::F32x4DemoteF64x2Zero => Ok(Operator::F32x4DemoteF64x2Zero),
            &wasmparser::Operator::F64x2PromoteLowF32x4 => Ok(Operator::F64x2PromoteLowF32x4),

            &wasmparser::Operator::CallRef { type_index } => Ok(Operator::CallRef {
                sig_index: Signature::from(type_index),
            }),
            &wasmparser::Operator::RefIsNull => Ok(Operator::RefIsNull),
            &wasmparser::Operator::RefNull { hty } => {
                let ty = wasmparser::RefType::new(true, hty).ok_or(())?;
                Ok(Operator::RefNull {
                    ty: ty.try_into().map_err(|_| ())?,
                })
            }
            &wasmparser::Operator::RefFunc { function_index } => Ok(Operator::RefFunc {
                func_index: Func::from(function_index),
            }),

            _ => Err(()),
        }
    }
}

impl std::convert::From<wasmparser::MemArg> for MemoryArg {
    fn from(value: wasmparser::MemArg) -> MemoryArg {
        MemoryArg {
            align: value.align as u32,
            offset: u32::try_from(value.offset).expect("offset too large"),
            memory: Memory::from(value.memory),
        }
    }
}

impl std::convert::From<MemoryArg> for wasm_encoder::MemArg {
    fn from(value: MemoryArg) -> wasm_encoder::MemArg {
        wasm_encoder::MemArg {
            offset: value.offset as u64,
            align: value.align,
            memory_index: value.memory.index() as u32,
        }
    }
}
