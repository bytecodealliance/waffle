//! Metadata on operators.

use crate::entity::EntityRef;
use crate::ir::{Module, Type, Value};
use crate::{MemoryArg, Operator};
use anyhow::Result;
use std::borrow::Cow;

/// Given a module and an existing operand stack for context, provide
/// the type(s) that a given operator requires as inputs.
pub fn op_inputs(
    module: &Module,
    op_stack: &[(Type, Value)],
    op: &Operator,
) -> Result<Cow<'static, [Type]>> {
    match op {
        &Operator::Unreachable | &Operator::Nop => Ok(Cow::Borrowed(&[])),

        &Operator::Call { function_index } => {
            let sig = module.funcs[function_index].sig();
            Ok(Vec::from(module.signatures[sig].params.clone()).into())
        }
        &Operator::CallIndirect { sig_index, .. } => {
            let mut params = module.signatures[sig_index].params.to_vec();
            params.push(Type::I32);
            Ok(params.into())
        }

        &Operator::Select => {
            let val_ty = op_stack[op_stack.len() - 2].0;
            Ok(vec![val_ty, val_ty, Type::I32].into())
        }
        &Operator::TypedSelect { ty } => Ok(vec![ty, ty, Type::I32].into()),

        &Operator::GlobalGet { .. } => Ok(Cow::Borrowed(&[])),
        &Operator::GlobalSet { global_index } => Ok(vec![module.globals[global_index].ty].into()),

        Operator::I32Load { .. }
        | Operator::I64Load { .. }
        | Operator::F32Load { .. }
        | Operator::F64Load { .. }
        | Operator::I32Load8S { .. }
        | Operator::I32Load8U { .. }
        | Operator::I32Load16S { .. }
        | Operator::I32Load16U { .. }
        | Operator::I64Load8S { .. }
        | Operator::I64Load8U { .. }
        | Operator::I64Load16S { .. }
        | Operator::I64Load16U { .. }
        | Operator::I64Load32S { .. }
        | Operator::I64Load32U { .. } => Ok(Cow::Borrowed(&[Type::I32])),

        Operator::I32Store { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::I32])),
        Operator::I64Store { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::I64])),
        Operator::F32Store { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::F32])),
        Operator::F64Store { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::F64])),
        Operator::I32Store8 { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::I32])),
        Operator::I32Store16 { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::I32])),
        Operator::I64Store8 { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::I64])),
        Operator::I64Store16 { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::I64])),
        Operator::I64Store32 { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::I64])),

        Operator::I32Const { .. }
        | Operator::I64Const { .. }
        | Operator::F32Const { .. }
        | Operator::F64Const { .. } => Ok(Cow::Borrowed(&[])),

        Operator::I32Eqz => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32Eq
        | Operator::I32Ne
        | Operator::I32LtS
        | Operator::I32LtU
        | Operator::I32GtS
        | Operator::I32GtU
        | Operator::I32LeS
        | Operator::I32LeU
        | Operator::I32GeS
        | Operator::I32GeU => Ok(Cow::Borrowed(&[Type::I32, Type::I32])),

        Operator::I64Eqz => Ok(Cow::Borrowed(&[Type::I64])),

        Operator::I64Eq
        | Operator::I64Ne
        | Operator::I64LtS
        | Operator::I64LtU
        | Operator::I64GtU
        | Operator::I64GtS
        | Operator::I64LeS
        | Operator::I64LeU
        | Operator::I64GeS
        | Operator::I64GeU => Ok(Cow::Borrowed(&[Type::I64, Type::I64])),

        Operator::F32Eq
        | Operator::F32Ne
        | Operator::F32Lt
        | Operator::F32Gt
        | Operator::F32Le
        | Operator::F32Ge => Ok(Cow::Borrowed(&[Type::F32, Type::F32])),

        Operator::F64Eq
        | Operator::F64Ne
        | Operator::F64Lt
        | Operator::F64Gt
        | Operator::F64Le
        | Operator::F64Ge => Ok(Cow::Borrowed(&[Type::F64, Type::F64])),

        Operator::I32Clz | Operator::I32Ctz | Operator::I32Popcnt => {
            Ok(Cow::Borrowed(&[Type::I32]))
        }

        Operator::I32Add
        | Operator::I32Sub
        | Operator::I32Mul
        | Operator::I32DivS
        | Operator::I32DivU
        | Operator::I32RemS
        | Operator::I32RemU
        | Operator::I32And
        | Operator::I32Or
        | Operator::I32Xor
        | Operator::I32Shl
        | Operator::I32ShrS
        | Operator::I32ShrU
        | Operator::I32Rotl
        | Operator::I32Rotr => Ok(Cow::Borrowed(&[Type::I32, Type::I32])),

        Operator::I64Clz | Operator::I64Ctz | Operator::I64Popcnt => {
            Ok(Cow::Borrowed(&[Type::I64]))
        }

        Operator::I64Add
        | Operator::I64Sub
        | Operator::I64Mul
        | Operator::I64DivS
        | Operator::I64DivU
        | Operator::I64RemS
        | Operator::I64RemU
        | Operator::I64And
        | Operator::I64Or
        | Operator::I64Xor
        | Operator::I64Shl
        | Operator::I64ShrS
        | Operator::I64ShrU
        | Operator::I64Rotl
        | Operator::I64Rotr => Ok(Cow::Borrowed(&[Type::I64, Type::I64])),

        Operator::F32Abs
        | Operator::F32Neg
        | Operator::F32Ceil
        | Operator::F32Floor
        | Operator::F32Trunc
        | Operator::F32Nearest
        | Operator::F32Sqrt => Ok(Cow::Borrowed(&[Type::F32])),

        Operator::F32Add
        | Operator::F32Sub
        | Operator::F32Mul
        | Operator::F32Div
        | Operator::F32Min
        | Operator::F32Max
        | Operator::F32Copysign => Ok(Cow::Borrowed(&[Type::F32, Type::F32])),

        Operator::F64Abs
        | Operator::F64Neg
        | Operator::F64Ceil
        | Operator::F64Floor
        | Operator::F64Trunc
        | Operator::F64Nearest
        | Operator::F64Sqrt => Ok(Cow::Borrowed(&[Type::F64])),

        Operator::F64Add
        | Operator::F64Sub
        | Operator::F64Mul
        | Operator::F64Div
        | Operator::F64Min
        | Operator::F64Max
        | Operator::F64Copysign => Ok(Cow::Borrowed(&[Type::F64, Type::F64])),

        Operator::I32WrapI64 => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I32TruncF32S => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::I32TruncF32U => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::I32TruncF64S => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::I32TruncF64U => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::I64ExtendI32S => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I64ExtendI32U => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I64TruncF32S => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::I64TruncF32U => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::I64TruncF64S => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::I64TruncF64U => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::F32ConvertI32S => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::F32ConvertI32U => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::F32ConvertI64S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::F32ConvertI64U => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::F32DemoteF64 => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::F64ConvertI32S => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::F64ConvertI32U => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::F64ConvertI64S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::F64ConvertI64U => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::F64PromoteF32 => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::I32Extend8S => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32Extend16S => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I64Extend8S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64Extend16S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64Extend32S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I32TruncSatF32S => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::I32TruncSatF32U => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::I32TruncSatF64S => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::I32TruncSatF64U => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::I64TruncSatF32S => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::I64TruncSatF32U => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::I64TruncSatF64S => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::I64TruncSatF64U => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::F32ReinterpretI32 => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::F64ReinterpretI64 => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I32ReinterpretF32 => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::I64ReinterpretF64 => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::TableGet { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::TableSet { table_index } => {
            Ok(vec![Type::I32, module.tables[*table_index].ty].into())
        }
        Operator::TableGrow { table_index } => {
            Ok(vec![Type::I32, module.tables[*table_index].ty].into())
        }
        Operator::TableSize { .. } => Ok(Cow::Borrowed(&[])),
        Operator::MemorySize { .. } => Ok(Cow::Borrowed(&[])),
        Operator::MemoryGrow { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::MemoryCopy { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::I32, Type::I32])),
        Operator::MemoryFill { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::I32, Type::I32])),

        Operator::V128Load { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load8x8S { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load8x8U { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load16x4S { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load16x4U { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load32x2S { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load32x2U { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load8Splat { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load16Splat { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load32Splat { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load64Splat { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load32Zero { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load64Zero { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Store { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::V128])),
        Operator::V128Load8Lane { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::V128])),
        Operator::V128Load16Lane { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::V128])),
        Operator::V128Load32Lane { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::V128])),
        Operator::V128Load64Lane { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::V128])),
        Operator::V128Store8Lane { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::V128])),
        Operator::V128Store16Lane { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::V128])),
        Operator::V128Store32Lane { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::V128])),
        Operator::V128Store64Lane { .. } => Ok(Cow::Borrowed(&[Type::I32, Type::V128])),

        Operator::V128Const { .. } => Ok(Cow::Borrowed(&[])),
        Operator::I8x16Shuffle { .. } => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16ExtractLaneS { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16ExtractLaneU { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16ReplaceLane { .. } => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I16x8ExtractLaneS { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtractLaneU { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ReplaceLane { .. } => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I32x4ExtractLane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ReplaceLane { .. } => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I64x2ExtractLane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ReplaceLane { .. } => Ok(Cow::Borrowed(&[Type::V128, Type::I64])),
        Operator::F32x4ExtractLane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4ReplaceLane { .. } => Ok(Cow::Borrowed(&[Type::V128, Type::F32])),
        Operator::F64x2ExtractLane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2ReplaceLane { .. } => Ok(Cow::Borrowed(&[Type::V128, Type::F64])),

        Operator::I8x16Swizzle => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16Splat => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I16x8Splat => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32x4Splat => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I64x2Splat => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::F32x4Splat => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::F64x2Splat => Ok(Cow::Borrowed(&[Type::F64])),

        Operator::I8x16Eq => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16Ne => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16LtS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16LtU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16GtS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16GtU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16LeS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16LeU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16GeS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16GeU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),

        Operator::I16x8Eq => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8Ne => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8LtS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8LtU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8GtS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8GtU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8LeS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8LeU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8GeS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8GeU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),

        Operator::I32x4Eq => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4Ne => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4LtS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4LtU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4GtS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4GtU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4LeS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4LeU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4GeS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4GeU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),

        Operator::I64x2Eq => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I64x2Ne => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I64x2LtS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I64x2GtS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I64x2LeS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I64x2GeS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),

        Operator::F32x4Eq => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F32x4Ne => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F32x4Lt => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F32x4Gt => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F32x4Le => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F32x4Ge => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),

        Operator::F64x2Eq => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F64x2Ne => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F64x2Lt => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F64x2Gt => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F64x2Le => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F64x2Ge => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),

        Operator::V128Not => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128And => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::V128AndNot => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::V128Or => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::V128Xor => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::V128Bitselect => Ok(Cow::Borrowed(&[Type::V128, Type::V128, Type::V128])),
        Operator::V128AnyTrue => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::I8x16Abs => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16Neg => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16Popcnt => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16AllTrue => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16Bitmask => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16NarrowI16x8S => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16NarrowI16x8U => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16Shl => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I8x16ShrS => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I8x16ShrU => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I8x16Add => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16AddSatS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16AddSatU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16Sub => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16SubSatS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16SubSatU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16MinS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16MinU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16MaxS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16MaxU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I8x16AvgrU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),

        Operator::I16x8ExtAddPairwiseI8x16S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtAddPairwiseI8x16U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Abs => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Neg => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Q15MulrSatS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8AllTrue => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Bitmask => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8NarrowI32x4S => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8NarrowI32x4U => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8ExtendLowI8x16S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtendHighI8x16S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtendLowI8x16U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtendHighI8x16U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Shl => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I16x8ShrS => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I16x8ShrU => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I16x8Add => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8AddSatS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8AddSatU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8Sub => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8SubSatS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8SubSatU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8Mul => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8MinS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8MinU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8MaxS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8MaxU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8AvgrU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8ExtMulLowI8x16S => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8ExtMulHighI8x16S => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8ExtMulLowI8x16U => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I16x8ExtMulHighI8x16U => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),

        Operator::I32x4ExtAddPairwiseI16x8S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtAddPairwiseI16x8U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4Abs => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4Neg => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4AllTrue => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4Bitmask => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtendLowI16x8S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtendHighI16x8S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtendLowI16x8U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtendHighI16x8U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4Shl => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I32x4ShrS => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I32x4ShrU => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I32x4Add => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4Sub => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4Mul => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4MinS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4MinU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4MaxS => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4MaxU => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4DotI16x8S => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4ExtMulLowI16x8S => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4ExtMulHighI16x8S => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4ExtMulLowI16x8U => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I32x4ExtMulHighI16x8U => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),

        Operator::I64x2Abs => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2Neg => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2AllTrue => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2Bitmask => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtendLowI32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtendHighI32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtendLowI32x4U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtendHighI32x4U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2Shl => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I64x2ShrS => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I64x2ShrU => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I64x2Add => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I64x2Sub => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I64x2Mul => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I64x2ExtMulLowI32x4S => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I64x2ExtMulHighI32x4S => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I64x2ExtMulLowI32x4U => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::I64x2ExtMulHighI32x4U => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),

        Operator::F32x4Ceil => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Floor => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Trunc => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Nearest => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Abs => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Neg => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Sqrt => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Add => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F32x4Sub => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F32x4Mul => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F32x4Div => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F32x4Min => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F32x4Max => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F32x4PMin => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F32x4PMax => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),

        Operator::F64x2Ceil => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Floor => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Trunc => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Nearest => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Abs => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Neg => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Sqrt => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Add => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F64x2Sub => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F64x2Mul => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F64x2Div => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F64x2Min => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F64x2Max => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F64x2PMin => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),
        Operator::F64x2PMax => Ok(Cow::Borrowed(&[Type::V128, Type::V128])),

        Operator::I32x4TruncSatF32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4TruncSatF32x4U => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::F32x4ConvertI32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4ConvertI32x4U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4TruncSatF64x2SZero => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4TruncSatF64x2UZero => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2ConvertLowI32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2ConvertLowI32x4U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4DemoteF64x2Zero => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2PromoteLowF32x4 => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::CallRef { sig_index } => {
            let mut params = module.signatures[*sig_index].params.to_vec();
            params.push(Type::TypedFuncRef(true, sig_index.index() as u32));
            Ok(params.into())
        }
        Operator::RefIsNull => Ok(vec![op_stack.last().unwrap().0].into()),
        Operator::RefNull { .. } => Ok(Cow::Borrowed(&[])),
        Operator::RefFunc { .. } => Ok(Cow::Borrowed(&[])),
    }
}

/// Given a module and an existing operand stack for context, provide
/// the type(s) that a given operator provides as outputs.
pub fn op_outputs(
    module: &Module,
    op_stack: &[(Type, Value)],
    op: &Operator,
) -> Result<Cow<'static, [Type]>> {
    match op {
        &Operator::Unreachable | &Operator::Nop => Ok(Cow::Borrowed(&[])),

        &Operator::Call { function_index } => {
            let sig = module.funcs[function_index].sig();
            Ok(Vec::from(module.signatures[sig].returns.clone()).into())
        }
        &Operator::CallIndirect { sig_index, .. } => {
            Ok(Vec::from(module.signatures[sig_index].returns.clone()).into())
        }

        &Operator::Select => {
            let val_ty = op_stack[op_stack.len() - 2].0;
            Ok(vec![val_ty].into())
        }
        &Operator::TypedSelect { ty } => Ok(vec![ty].into()),
        &Operator::GlobalGet { global_index } => Ok(vec![module.globals[global_index].ty].into()),
        &Operator::GlobalSet { .. } => Ok(Cow::Borrowed(&[])),

        Operator::I32Load { .. }
        | Operator::I32Load8S { .. }
        | Operator::I32Load8U { .. }
        | Operator::I32Load16S { .. }
        | Operator::I32Load16U { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I64Load { .. }
        | Operator::I64Load8S { .. }
        | Operator::I64Load8U { .. }
        | Operator::I64Load16S { .. }
        | Operator::I64Load16U { .. }
        | Operator::I64Load32S { .. }
        | Operator::I64Load32U { .. } => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::F32Load { .. } => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::F64Load { .. } => Ok(Cow::Borrowed(&[Type::F64])),

        Operator::I32Store { .. } => Ok(Cow::Borrowed(&[])),
        Operator::I64Store { .. } => Ok(Cow::Borrowed(&[])),
        Operator::F32Store { .. } => Ok(Cow::Borrowed(&[])),
        Operator::F64Store { .. } => Ok(Cow::Borrowed(&[])),
        Operator::I32Store8 { .. } => Ok(Cow::Borrowed(&[])),
        Operator::I32Store16 { .. } => Ok(Cow::Borrowed(&[])),
        Operator::I64Store8 { .. } => Ok(Cow::Borrowed(&[])),
        Operator::I64Store16 { .. } => Ok(Cow::Borrowed(&[])),
        Operator::I64Store32 { .. } => Ok(Cow::Borrowed(&[])),

        Operator::I32Const { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I64Const { .. } => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::F32Const { .. } => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::F64Const { .. } => Ok(Cow::Borrowed(&[Type::F64])),

        Operator::I32Eqz
        | Operator::I32Eq
        | Operator::I32Ne
        | Operator::I32LtS
        | Operator::I32LtU
        | Operator::I32GtS
        | Operator::I32GtU
        | Operator::I32LeS
        | Operator::I32LeU
        | Operator::I32GeS
        | Operator::I32GeU
        | Operator::I64Eqz
        | Operator::I64Eq
        | Operator::I64Ne
        | Operator::I64LtS
        | Operator::I64LtU
        | Operator::I64GtU
        | Operator::I64GtS
        | Operator::I64LeS
        | Operator::I64LeU
        | Operator::I64GeS
        | Operator::I64GeU
        | Operator::F32Eq
        | Operator::F32Ne
        | Operator::F32Lt
        | Operator::F32Gt
        | Operator::F32Le
        | Operator::F32Ge
        | Operator::F64Eq
        | Operator::F64Ne
        | Operator::F64Lt
        | Operator::F64Gt
        | Operator::F64Le
        | Operator::F64Ge => Ok(Cow::Borrowed(&[Type::I32])),

        Operator::I32Clz
        | Operator::I32Ctz
        | Operator::I32Popcnt
        | Operator::I32Add
        | Operator::I32Sub
        | Operator::I32Mul
        | Operator::I32DivS
        | Operator::I32DivU
        | Operator::I32RemS
        | Operator::I32RemU
        | Operator::I32And
        | Operator::I32Or
        | Operator::I32Xor
        | Operator::I32Shl
        | Operator::I32ShrS
        | Operator::I32ShrU
        | Operator::I32Rotl
        | Operator::I32Rotr => Ok(Cow::Borrowed(&[Type::I32])),

        Operator::I64Clz
        | Operator::I64Ctz
        | Operator::I64Popcnt
        | Operator::I64Add
        | Operator::I64Sub
        | Operator::I64Mul
        | Operator::I64DivS
        | Operator::I64DivU
        | Operator::I64RemS
        | Operator::I64RemU
        | Operator::I64And
        | Operator::I64Or
        | Operator::I64Xor
        | Operator::I64Shl
        | Operator::I64ShrS
        | Operator::I64ShrU
        | Operator::I64Rotl
        | Operator::I64Rotr => Ok(Cow::Borrowed(&[Type::I64])),

        Operator::F32Abs
        | Operator::F32Neg
        | Operator::F32Ceil
        | Operator::F32Floor
        | Operator::F32Trunc
        | Operator::F32Nearest
        | Operator::F32Sqrt
        | Operator::F32Add
        | Operator::F32Sub
        | Operator::F32Mul
        | Operator::F32Div
        | Operator::F32Min
        | Operator::F32Max
        | Operator::F32Copysign => Ok(Cow::Borrowed(&[Type::F32])),

        Operator::F64Abs
        | Operator::F64Neg
        | Operator::F64Ceil
        | Operator::F64Floor
        | Operator::F64Trunc
        | Operator::F64Nearest
        | Operator::F64Sqrt
        | Operator::F64Add
        | Operator::F64Sub
        | Operator::F64Mul
        | Operator::F64Div
        | Operator::F64Min
        | Operator::F64Max
        | Operator::F64Copysign => Ok(Cow::Borrowed(&[Type::F64])),

        Operator::I32WrapI64 => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32TruncF32S => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32TruncF32U => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32TruncF64S => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32TruncF64U => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I64ExtendI32S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64ExtendI32U => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64TruncF32S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64TruncF32U => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64TruncF64S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64TruncF64U => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::F32ConvertI32S => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::F32ConvertI32U => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::F32ConvertI64S => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::F32ConvertI64U => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::F32DemoteF64 => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::F64ConvertI32S => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::F64ConvertI32U => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::F64ConvertI64S => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::F64ConvertI64U => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::F64PromoteF32 => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::I32Extend8S => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32Extend16S => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I64Extend8S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64Extend16S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64Extend32S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I32TruncSatF32S => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32TruncSatF32U => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32TruncSatF64S => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32TruncSatF64U => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I64TruncSatF32S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64TruncSatF32U => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64TruncSatF64S => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64TruncSatF64U => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::F32ReinterpretI32 => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::F64ReinterpretI64 => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::I32ReinterpretF32 => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I64ReinterpretF64 => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::TableGet { table_index } => Ok(vec![module.tables[*table_index].ty].into()),
        Operator::TableSet { .. } => Ok(Cow::Borrowed(&[])),
        Operator::TableGrow { .. } => Ok(Cow::Borrowed(&[])),
        Operator::TableSize { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::MemorySize { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::MemoryGrow { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::MemoryCopy { .. } => Ok(Cow::Borrowed(&[])),
        Operator::MemoryFill { .. } => Ok(Cow::Borrowed(&[])),

        Operator::V128Load { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load8x8S { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::V128Load8x8U { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load16x4S { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load16x4U { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load32x2S { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load32x2U { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load8Splat { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load16Splat { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load32Splat { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load64Splat { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load32Zero { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load64Zero { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Store { .. } => Ok(Cow::Borrowed(&[])),
        Operator::V128Load8Lane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load16Lane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load32Lane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Load64Lane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Store8Lane { .. } => Ok(Cow::Borrowed(&[])),
        Operator::V128Store16Lane { .. } => Ok(Cow::Borrowed(&[])),
        Operator::V128Store32Lane { .. } => Ok(Cow::Borrowed(&[])),
        Operator::V128Store64Lane { .. } => Ok(Cow::Borrowed(&[])),

        Operator::V128Const { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16Shuffle { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16ExtractLaneS { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I8x16ExtractLaneU { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I8x16ReplaceLane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtractLaneS { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I16x8ExtractLaneU { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I16x8ReplaceLane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtractLane { .. } => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32x4ReplaceLane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtractLane { .. } => Ok(Cow::Borrowed(&[Type::I64])),
        Operator::I64x2ReplaceLane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4ExtractLane { .. } => Ok(Cow::Borrowed(&[Type::F32])),
        Operator::F32x4ReplaceLane { .. } => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2ExtractLane { .. } => Ok(Cow::Borrowed(&[Type::F64])),
        Operator::F64x2ReplaceLane { .. } => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::I8x16Swizzle => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16Splat => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Splat => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4Splat => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2Splat => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Splat => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Splat => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::I8x16Eq => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16Ne => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16LtS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16LtU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16GtS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16GtU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16LeS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16LeU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16GeS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16GeU => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::I16x8Eq => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Ne => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8LtS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8LtU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8GtS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8GtU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8LeS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8LeU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8GeS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8GeU => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::I32x4Eq => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4Ne => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4LtS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4LtU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4GtS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4GtU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4LeS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4LeU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4GeS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4GeU => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::I64x2Eq => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2Ne => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2LtS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2GtS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2LeS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2GeS => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::F32x4Eq => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Ne => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Lt => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Gt => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Le => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Ge => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::F64x2Eq => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Ne => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Lt => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Gt => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Le => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Ge => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::V128Not => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128And => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128AndNot => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Or => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Xor => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128Bitselect => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::V128AnyTrue => Ok(Cow::Borrowed(&[Type::I32])),

        Operator::I8x16Abs => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16Neg => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16Popcnt => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16AllTrue => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I8x16Bitmask => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16NarrowI16x8S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16NarrowI16x8U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16Shl => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16ShrS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16ShrU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16Add => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16AddSatS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16AddSatU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16Sub => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16SubSatS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16SubSatU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16MinS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16MinU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16MaxS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16MaxU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I8x16AvgrU => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::I16x8ExtAddPairwiseI8x16S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtAddPairwiseI8x16U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Abs => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Neg => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Q15MulrSatS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8AllTrue => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I16x8Bitmask => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8NarrowI32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8NarrowI32x4U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtendLowI8x16S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtendHighI8x16S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtendLowI8x16U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtendHighI8x16U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Shl => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I16x8ShrS => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I16x8ShrU => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I16x8Add => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8AddSatS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8AddSatU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Sub => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8SubSatS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8SubSatU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8Mul => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8MinS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8MinU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8MaxS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8MaxU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8AvgrU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtMulLowI8x16S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtMulHighI8x16S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtMulLowI8x16U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I16x8ExtMulHighI8x16U => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::I32x4ExtAddPairwiseI16x8S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtAddPairwiseI16x8U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4Abs => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4Neg => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4AllTrue => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I32x4Bitmask => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtendLowI16x8S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtendHighI16x8S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtendLowI16x8U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtendHighI16x8U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4Shl => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I32x4ShrS => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I32x4ShrU => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I32x4Add => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4Sub => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4Mul => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4MinS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4MinU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4MaxS => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4MaxU => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4DotI16x8S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtMulLowI16x8S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtMulHighI16x8S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtMulLowI16x8U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4ExtMulHighI16x8U => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::I64x2Abs => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2Neg => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2AllTrue => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::I64x2Bitmask => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtendLowI32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtendHighI32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtendLowI32x4U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtendHighI32x4U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2Shl => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I64x2ShrS => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I64x2ShrU => Ok(Cow::Borrowed(&[Type::V128, Type::I32])),
        Operator::I64x2Add => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2Sub => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2Mul => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtMulLowI32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtMulHighI32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtMulLowI32x4U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I64x2ExtMulHighI32x4U => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::F32x4Ceil => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Floor => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Trunc => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Nearest => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Abs => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Neg => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Sqrt => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Add => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Sub => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Mul => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Div => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Min => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4Max => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4PMin => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4PMax => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::F64x2Ceil => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Floor => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Trunc => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Nearest => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Abs => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Neg => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Sqrt => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Add => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Sub => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Mul => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Div => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Min => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2Max => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2PMin => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2PMax => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::I32x4TruncSatF32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4TruncSatF32x4U => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::F32x4ConvertI32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4ConvertI32x4U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4TruncSatF64x2SZero => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::I32x4TruncSatF64x2UZero => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2ConvertLowI32x4S => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2ConvertLowI32x4U => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F32x4DemoteF64x2Zero => Ok(Cow::Borrowed(&[Type::V128])),
        Operator::F64x2PromoteLowF32x4 => Ok(Cow::Borrowed(&[Type::V128])),

        Operator::CallRef { sig_index } => {
            Ok(Vec::from(module.signatures[*sig_index].returns.clone()).into())
        }
        Operator::RefIsNull => Ok(Cow::Borrowed(&[Type::I32])),
        Operator::RefNull { sig_index } => {
            Ok(vec![Type::TypedFuncRef(true, sig_index.index() as u32)].into())
        }
        Operator::RefFunc { func_index } => {
            let ty = module.funcs[*func_index].sig();
            Ok(vec![Type::TypedFuncRef(true, ty.index() as u32)].into())
        }
    }
}

/// Side-effects that an operator may have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideEffect {
    /// Operator can trap.
    Trap,
    /// Operator can read a memory.
    ReadMem,
    /// Operator can write a memory.
    WriteMem,
    /// Operator can read a global.
    ReadGlobal,
    /// Operator can write a global.
    WriteGlobal,
    /// Operator can read a table element.
    ReadTable,
    /// Operator can write a table element.
    WriteTable,
    /// Operator can read a local.
    ReadLocal,
    /// Operator can write a local.
    WriteLocal,
    /// Operator can have any of the above effects.
    All,
}

impl Operator {
    /// What side-effects can this operator have?
    pub fn effects(&self) -> &'static [SideEffect] {
        use SideEffect::*;

        match self {
            &Operator::Unreachable => &[Trap],
            &Operator::Nop => &[],

            &Operator::Call { .. } => &[All],
            &Operator::CallIndirect { .. } => &[All],

            &Operator::Select => &[],
            &Operator::TypedSelect { .. } => &[],
            &Operator::GlobalGet { .. } => &[ReadGlobal],
            &Operator::GlobalSet { .. } => &[WriteGlobal],

            Operator::I32Load { .. }
            | Operator::I32Load8S { .. }
            | Operator::I32Load8U { .. }
            | Operator::I32Load16S { .. }
            | Operator::I32Load16U { .. }
            | Operator::I64Load { .. }
            | Operator::I64Load8S { .. }
            | Operator::I64Load8U { .. }
            | Operator::I64Load16S { .. }
            | Operator::I64Load16U { .. }
            | Operator::I64Load32S { .. }
            | Operator::I64Load32U { .. }
            | Operator::F32Load { .. }
            | Operator::F64Load { .. } => &[Trap, ReadMem],

            Operator::I32Store { .. }
            | Operator::I64Store { .. }
            | Operator::F32Store { .. }
            | Operator::F64Store { .. }
            | Operator::I32Store8 { .. }
            | Operator::I32Store16 { .. }
            | Operator::I64Store8 { .. }
            | Operator::I64Store16 { .. }
            | Operator::I64Store32 { .. } => &[Trap, WriteMem],

            Operator::I32Const { .. }
            | Operator::I64Const { .. }
            | Operator::F32Const { .. }
            | Operator::F64Const { .. } => &[],

            Operator::I32Eqz
            | Operator::I32Eq
            | Operator::I32Ne
            | Operator::I32LtS
            | Operator::I32LtU
            | Operator::I32GtS
            | Operator::I32GtU
            | Operator::I32LeS
            | Operator::I32LeU
            | Operator::I32GeS
            | Operator::I32GeU
            | Operator::I64Eqz
            | Operator::I64Eq
            | Operator::I64Ne
            | Operator::I64LtS
            | Operator::I64LtU
            | Operator::I64GtU
            | Operator::I64GtS
            | Operator::I64LeS
            | Operator::I64LeU
            | Operator::I64GeS
            | Operator::I64GeU
            | Operator::F32Eq
            | Operator::F32Ne
            | Operator::F32Lt
            | Operator::F32Gt
            | Operator::F32Le
            | Operator::F32Ge
            | Operator::F64Eq
            | Operator::F64Ne
            | Operator::F64Lt
            | Operator::F64Gt
            | Operator::F64Le
            | Operator::F64Ge => &[],

            Operator::I32Clz
            | Operator::I32Ctz
            | Operator::I32Popcnt
            | Operator::I32Add
            | Operator::I32Sub
            | Operator::I32Mul
            | Operator::I32And
            | Operator::I32Or
            | Operator::I32Xor
            | Operator::I32Shl
            | Operator::I32ShrS
            | Operator::I32ShrU
            | Operator::I32Rotl
            | Operator::I32Rotr => &[],

            Operator::I32DivS | Operator::I32DivU | Operator::I32RemS | Operator::I32RemU => {
                &[Trap]
            }

            Operator::I64Clz
            | Operator::I64Ctz
            | Operator::I64Popcnt
            | Operator::I64Add
            | Operator::I64Sub
            | Operator::I64Mul
            | Operator::I64And
            | Operator::I64Or
            | Operator::I64Xor
            | Operator::I64Shl
            | Operator::I64ShrS
            | Operator::I64ShrU
            | Operator::I64Rotl
            | Operator::I64Rotr => &[],

            Operator::I64DivS | Operator::I64DivU | Operator::I64RemS | Operator::I64RemU => {
                &[Trap]
            }

            Operator::F32Abs
            | Operator::F32Neg
            | Operator::F32Ceil
            | Operator::F32Floor
            | Operator::F32Trunc
            | Operator::F32Nearest
            | Operator::F32Sqrt
            | Operator::F32Add
            | Operator::F32Sub
            | Operator::F32Mul
            | Operator::F32Div
            | Operator::F32Min
            | Operator::F32Max
            | Operator::F32Copysign => &[],

            Operator::F64Abs
            | Operator::F64Neg
            | Operator::F64Ceil
            | Operator::F64Floor
            | Operator::F64Trunc
            | Operator::F64Nearest
            | Operator::F64Sqrt
            | Operator::F64Add
            | Operator::F64Sub
            | Operator::F64Mul
            | Operator::F64Div
            | Operator::F64Min
            | Operator::F64Max
            | Operator::F64Copysign => &[],

            Operator::I32WrapI64 => &[],
            Operator::I32TruncF32S => &[Trap],
            Operator::I32TruncF32U => &[Trap],
            Operator::I32TruncF64S => &[Trap],
            Operator::I32TruncF64U => &[Trap],
            Operator::I64ExtendI32S => &[],
            Operator::I64ExtendI32U => &[],
            Operator::I64TruncF32S => &[Trap],
            Operator::I64TruncF32U => &[Trap],
            Operator::I64TruncF64S => &[Trap],
            Operator::I64TruncF64U => &[Trap],
            Operator::F32ConvertI32S => &[],
            Operator::F32ConvertI32U => &[],
            Operator::F32ConvertI64S => &[],
            Operator::F32ConvertI64U => &[],
            Operator::F32DemoteF64 => &[],
            Operator::F64ConvertI32S => &[],
            Operator::F64ConvertI32U => &[],
            Operator::F64ConvertI64S => &[],
            Operator::F64ConvertI64U => &[],
            Operator::F64PromoteF32 => &[],
            Operator::I32Extend8S => &[],
            Operator::I32Extend16S => &[],
            Operator::I64Extend8S => &[],
            Operator::I64Extend16S => &[],
            Operator::I64Extend32S => &[],
            Operator::I32TruncSatF32S => &[],
            Operator::I32TruncSatF32U => &[],
            Operator::I32TruncSatF64S => &[],
            Operator::I32TruncSatF64U => &[],
            Operator::I64TruncSatF32S => &[],
            Operator::I64TruncSatF32U => &[],
            Operator::I64TruncSatF64S => &[],
            Operator::I64TruncSatF64U => &[],
            Operator::F32ReinterpretI32 => &[],
            Operator::F64ReinterpretI64 => &[],
            Operator::I32ReinterpretF32 => &[],
            Operator::I64ReinterpretF64 => &[],
            Operator::TableGet { .. } => &[ReadTable, Trap],
            Operator::TableSet { .. } => &[WriteTable, Trap],
            Operator::TableGrow { .. } => &[WriteTable, Trap],
            Operator::TableSize { .. } => &[ReadTable],
            Operator::MemorySize { .. } => &[ReadMem],
            Operator::MemoryGrow { .. } => &[WriteMem, Trap],
            Operator::MemoryCopy { .. } => &[Trap, ReadMem, WriteMem],
            Operator::MemoryFill { .. } => &[Trap, WriteMem],

            Operator::V128Load { .. } => &[Trap, ReadMem],
            Operator::V128Load8x8S { .. } => &[Trap, ReadMem],
            Operator::V128Load8x8U { .. } => &[Trap, ReadMem],
            Operator::V128Load16x4S { .. } => &[Trap, ReadMem],
            Operator::V128Load16x4U { .. } => &[Trap, ReadMem],
            Operator::V128Load32x2S { .. } => &[Trap, ReadMem],
            Operator::V128Load32x2U { .. } => &[Trap, ReadMem],
            Operator::V128Load8Splat { .. } => &[Trap, ReadMem],
            Operator::V128Load16Splat { .. } => &[Trap, ReadMem],
            Operator::V128Load32Splat { .. } => &[Trap, ReadMem],
            Operator::V128Load64Splat { .. } => &[Trap, ReadMem],
            Operator::V128Load32Zero { .. } => &[Trap, ReadMem],
            Operator::V128Load64Zero { .. } => &[Trap, ReadMem],
            Operator::V128Store { .. } => &[Trap, WriteMem],
            Operator::V128Load8Lane { .. } => &[Trap, ReadMem],
            Operator::V128Load16Lane { .. } => &[Trap, ReadMem],
            Operator::V128Load32Lane { .. } => &[Trap, ReadMem],
            Operator::V128Load64Lane { .. } => &[Trap, ReadMem],
            Operator::V128Store8Lane { .. } => &[Trap, WriteMem],
            Operator::V128Store16Lane { .. } => &[Trap, WriteMem],
            Operator::V128Store32Lane { .. } => &[Trap, WriteMem],
            Operator::V128Store64Lane { .. } => &[Trap, WriteMem],

            Operator::V128Const { .. } => &[],
            Operator::I8x16Shuffle { .. } => &[],
            Operator::I8x16ExtractLaneS { .. } => &[],
            Operator::I8x16ExtractLaneU { .. } => &[],
            Operator::I8x16ReplaceLane { .. } => &[],
            Operator::I16x8ExtractLaneS { .. } => &[],
            Operator::I16x8ExtractLaneU { .. } => &[],
            Operator::I16x8ReplaceLane { .. } => &[],
            Operator::I32x4ExtractLane { .. } => &[],
            Operator::I32x4ReplaceLane { .. } => &[],
            Operator::I64x2ExtractLane { .. } => &[],
            Operator::I64x2ReplaceLane { .. } => &[],
            Operator::F32x4ExtractLane { .. } => &[],
            Operator::F32x4ReplaceLane { .. } => &[],
            Operator::F64x2ExtractLane { .. } => &[],
            Operator::F64x2ReplaceLane { .. } => &[],

            Operator::I8x16Swizzle => &[],
            Operator::I8x16Splat => &[],
            Operator::I16x8Splat => &[],
            Operator::I32x4Splat => &[],
            Operator::I64x2Splat => &[],
            Operator::F32x4Splat => &[],
            Operator::F64x2Splat => &[],

            Operator::I8x16Eq => &[],
            Operator::I8x16Ne => &[],
            Operator::I8x16LtS => &[],
            Operator::I8x16LtU => &[],
            Operator::I8x16GtS => &[],
            Operator::I8x16GtU => &[],
            Operator::I8x16LeS => &[],
            Operator::I8x16LeU => &[],
            Operator::I8x16GeS => &[],
            Operator::I8x16GeU => &[],

            Operator::I16x8Eq => &[],
            Operator::I16x8Ne => &[],
            Operator::I16x8LtS => &[],
            Operator::I16x8LtU => &[],
            Operator::I16x8GtS => &[],
            Operator::I16x8GtU => &[],
            Operator::I16x8LeS => &[],
            Operator::I16x8LeU => &[],
            Operator::I16x8GeS => &[],
            Operator::I16x8GeU => &[],

            Operator::I32x4Eq => &[],
            Operator::I32x4Ne => &[],
            Operator::I32x4LtS => &[],
            Operator::I32x4LtU => &[],
            Operator::I32x4GtS => &[],
            Operator::I32x4GtU => &[],
            Operator::I32x4LeS => &[],
            Operator::I32x4LeU => &[],
            Operator::I32x4GeS => &[],
            Operator::I32x4GeU => &[],

            Operator::I64x2Eq => &[],
            Operator::I64x2Ne => &[],
            Operator::I64x2LtS => &[],
            Operator::I64x2GtS => &[],
            Operator::I64x2LeS => &[],
            Operator::I64x2GeS => &[],

            Operator::F32x4Eq => &[],
            Operator::F32x4Ne => &[],
            Operator::F32x4Lt => &[],
            Operator::F32x4Gt => &[],
            Operator::F32x4Le => &[],
            Operator::F32x4Ge => &[],

            Operator::F64x2Eq => &[],
            Operator::F64x2Ne => &[],
            Operator::F64x2Lt => &[],
            Operator::F64x2Gt => &[],
            Operator::F64x2Le => &[],
            Operator::F64x2Ge => &[],

            Operator::V128Not => &[],
            Operator::V128And => &[],
            Operator::V128AndNot => &[],
            Operator::V128Or => &[],
            Operator::V128Xor => &[],
            Operator::V128Bitselect => &[],
            Operator::V128AnyTrue => &[],

            Operator::I8x16Abs => &[],
            Operator::I8x16Neg => &[],
            Operator::I8x16Popcnt => &[],
            Operator::I8x16AllTrue => &[],
            Operator::I8x16Bitmask => &[],
            Operator::I8x16NarrowI16x8S => &[],
            Operator::I8x16NarrowI16x8U => &[],
            Operator::I8x16Shl => &[],
            Operator::I8x16ShrS => &[],
            Operator::I8x16ShrU => &[],
            Operator::I8x16Add => &[],
            Operator::I8x16AddSatS => &[],
            Operator::I8x16AddSatU => &[],
            Operator::I8x16Sub => &[],
            Operator::I8x16SubSatS => &[],
            Operator::I8x16SubSatU => &[],
            Operator::I8x16MinS => &[],
            Operator::I8x16MinU => &[],
            Operator::I8x16MaxS => &[],
            Operator::I8x16MaxU => &[],
            Operator::I8x16AvgrU => &[],

            Operator::I16x8ExtAddPairwiseI8x16S => &[],
            Operator::I16x8ExtAddPairwiseI8x16U => &[],
            Operator::I16x8Abs => &[],
            Operator::I16x8Neg => &[],
            Operator::I16x8Q15MulrSatS => &[],
            Operator::I16x8AllTrue => &[],
            Operator::I16x8Bitmask => &[],
            Operator::I16x8NarrowI32x4S => &[],
            Operator::I16x8NarrowI32x4U => &[],
            Operator::I16x8ExtendLowI8x16S => &[],
            Operator::I16x8ExtendHighI8x16S => &[],
            Operator::I16x8ExtendLowI8x16U => &[],
            Operator::I16x8ExtendHighI8x16U => &[],
            Operator::I16x8Shl => &[],
            Operator::I16x8ShrS => &[],
            Operator::I16x8ShrU => &[],
            Operator::I16x8Add => &[],
            Operator::I16x8AddSatS => &[],
            Operator::I16x8AddSatU => &[],
            Operator::I16x8Sub => &[],
            Operator::I16x8SubSatS => &[],
            Operator::I16x8SubSatU => &[],
            Operator::I16x8Mul => &[],
            Operator::I16x8MinS => &[],
            Operator::I16x8MinU => &[],
            Operator::I16x8MaxS => &[],
            Operator::I16x8MaxU => &[],
            Operator::I16x8AvgrU => &[],
            Operator::I16x8ExtMulLowI8x16S => &[],
            Operator::I16x8ExtMulHighI8x16S => &[],
            Operator::I16x8ExtMulLowI8x16U => &[],
            Operator::I16x8ExtMulHighI8x16U => &[],

            Operator::I32x4ExtAddPairwiseI16x8S => &[],
            Operator::I32x4ExtAddPairwiseI16x8U => &[],
            Operator::I32x4Abs => &[],
            Operator::I32x4Neg => &[],
            Operator::I32x4AllTrue => &[],
            Operator::I32x4Bitmask => &[],
            Operator::I32x4ExtendLowI16x8S => &[],
            Operator::I32x4ExtendHighI16x8S => &[],
            Operator::I32x4ExtendLowI16x8U => &[],
            Operator::I32x4ExtendHighI16x8U => &[],
            Operator::I32x4Shl => &[],
            Operator::I32x4ShrS => &[],
            Operator::I32x4ShrU => &[],
            Operator::I32x4Add => &[],
            Operator::I32x4Sub => &[],
            Operator::I32x4Mul => &[],
            Operator::I32x4MinS => &[],
            Operator::I32x4MinU => &[],
            Operator::I32x4MaxS => &[],
            Operator::I32x4MaxU => &[],
            Operator::I32x4DotI16x8S => &[],
            Operator::I32x4ExtMulLowI16x8S => &[],
            Operator::I32x4ExtMulHighI16x8S => &[],
            Operator::I32x4ExtMulLowI16x8U => &[],
            Operator::I32x4ExtMulHighI16x8U => &[],

            Operator::I64x2Abs => &[],
            Operator::I64x2Neg => &[],
            Operator::I64x2AllTrue => &[],
            Operator::I64x2Bitmask => &[],
            Operator::I64x2ExtendLowI32x4S => &[],
            Operator::I64x2ExtendHighI32x4S => &[],
            Operator::I64x2ExtendLowI32x4U => &[],
            Operator::I64x2ExtendHighI32x4U => &[],
            Operator::I64x2Shl => &[],
            Operator::I64x2ShrS => &[],
            Operator::I64x2ShrU => &[],
            Operator::I64x2Add => &[],
            Operator::I64x2Sub => &[],
            Operator::I64x2Mul => &[],
            Operator::I64x2ExtMulLowI32x4S => &[],
            Operator::I64x2ExtMulHighI32x4S => &[],
            Operator::I64x2ExtMulLowI32x4U => &[],
            Operator::I64x2ExtMulHighI32x4U => &[],

            Operator::F32x4Ceil => &[],
            Operator::F32x4Floor => &[],
            Operator::F32x4Trunc => &[],
            Operator::F32x4Nearest => &[],
            Operator::F32x4Abs => &[],
            Operator::F32x4Neg => &[],
            Operator::F32x4Sqrt => &[],
            Operator::F32x4Add => &[],
            Operator::F32x4Sub => &[],
            Operator::F32x4Mul => &[],
            Operator::F32x4Div => &[],
            Operator::F32x4Min => &[],
            Operator::F32x4Max => &[],
            Operator::F32x4PMin => &[],
            Operator::F32x4PMax => &[],

            Operator::F64x2Ceil => &[],
            Operator::F64x2Floor => &[],
            Operator::F64x2Trunc => &[],
            Operator::F64x2Nearest => &[],
            Operator::F64x2Abs => &[],
            Operator::F64x2Neg => &[],
            Operator::F64x2Sqrt => &[],
            Operator::F64x2Add => &[],
            Operator::F64x2Sub => &[],
            Operator::F64x2Mul => &[],
            Operator::F64x2Div => &[],
            Operator::F64x2Min => &[],
            Operator::F64x2Max => &[],
            Operator::F64x2PMin => &[],
            Operator::F64x2PMax => &[],

            Operator::I32x4TruncSatF32x4S => &[],
            Operator::I32x4TruncSatF32x4U => &[],

            Operator::F32x4ConvertI32x4S => &[],
            Operator::F32x4ConvertI32x4U => &[],
            Operator::I32x4TruncSatF64x2SZero => &[],
            Operator::I32x4TruncSatF64x2UZero => &[],
            Operator::F64x2ConvertLowI32x4S => &[],
            Operator::F64x2ConvertLowI32x4U => &[],
            Operator::F32x4DemoteF64x2Zero => &[],
            Operator::F64x2PromoteLowF32x4 => &[],

            Operator::CallRef { .. } => &[All],
            Operator::RefIsNull => &[],
            Operator::RefNull { .. } => &[],
            Operator::RefFunc { .. } => &[],
        }
    }

    /// Is the operator pure (has no side-effects)?
    pub fn is_pure(&self) -> bool {
        self.effects().is_empty()
    }

    /// Is the operator a direct or indirect call?
    pub fn is_call(&self) -> bool {
        match self {
            Operator::Call { .. } | Operator::CallIndirect { .. } => true,
            _ => false,
        }
    }

    /// Does the operator access (read or write) memory?
    pub fn accesses_memory(&self) -> bool {
        self.effects().iter().any(|e| match e {
            SideEffect::ReadMem | SideEffect::WriteMem => true,
            _ => false,
        })
    }

    /// Is the operator an ordinary memory load?
    ///
    /// Note that this does not include all opcodes that read memory
    /// *state*, such as accessing the size of memory, or that read and
    /// *also* write memory, such as a memory copy; only "normal" load
    /// instructions.
    pub fn is_load(&self) -> bool {
        // We explicitly match on opcode rather than checking the
        // effects set, beacuse some operators may have `[ReadMem]` or
        // `[Trap, ReadMem]` but not be a "normal" load. For example,
        // `MemorySize` reads memory (or a property of it, anyway) but
        // is not a "normal" load.
        match self {
            Operator::I32Load { .. }
            | Operator::I64Load { .. }
            | Operator::F32Load { .. }
            | Operator::F64Load { .. }
            | Operator::I32Load8S { .. }
            | Operator::I32Load8U { .. }
            | Operator::I32Load16S { .. }
            | Operator::I32Load16U { .. }
            | Operator::I64Load8S { .. }
            | Operator::I64Load8U { .. }
            | Operator::I64Load16S { .. }
            | Operator::I64Load16U { .. }
            | Operator::I64Load32S { .. }
            | Operator::I64Load32U { .. }
            | Operator::V128Load { .. }
            | Operator::V128Load8x8S { .. }
            | Operator::V128Load8x8U { .. }
            | Operator::V128Load16x4S { .. }
            | Operator::V128Load16x4U { .. }
            | Operator::V128Load32x2S { .. }
            | Operator::V128Load32x2U { .. }
            | Operator::V128Load8Splat { .. }
            | Operator::V128Load16Splat { .. }
            | Operator::V128Load32Splat { .. }
            | Operator::V128Load64Splat { .. }
            | Operator::V128Load32Zero { .. }
            | Operator::V128Load64Zero { .. }
            | Operator::V128Load8Lane { .. }
            | Operator::V128Load16Lane { .. }
            | Operator::V128Load32Lane { .. }
            | Operator::V128Load64Lane { .. } => true,
            _ => false,
        }
    }

    /// Is the operator an ordinary memory store?
    ///
    /// Note that this does not include all opcodes that write memory
    /// *state*, such as growing memory, or that write and *also* read
    /// memory, such as a memory copy; only "normal" store instructions.
    pub fn is_store(&self) -> bool {
        // We explicitly match on opcode rather than checking the
        // effects set, beacuse some operators may have `[WriteMem]` or
        // `[Trap, WriteMem]` but not be a "normal" store. For example,
        // `MemoryFill` writes memory but is not a "normal" store.
        match self {
            Operator::I32Store { .. }
            | Operator::I64Store { .. }
            | Operator::F32Store { .. }
            | Operator::F64Store { .. }
            | Operator::I32Store8 { .. }
            | Operator::I32Store16 { .. }
            | Operator::I64Store8 { .. }
            | Operator::I64Store16 { .. }
            | Operator::I64Store32 { .. }
            | Operator::V128Store { .. }
            | Operator::V128Store8Lane { .. }
            | Operator::V128Store16Lane { .. }
            | Operator::V128Store32Lane { .. }
            | Operator::V128Store64Lane { .. } => true,
            _ => false,
        }
    }

    /// Call `f` on the operator's `MemoryArg`, if it has one.
    pub fn update_memory_arg<F: FnMut(&mut MemoryArg)>(&mut self, mut f: F) {
        match self {
            Operator::I32Load { memory }
            | Operator::I64Load { memory }
            | Operator::F32Load { memory }
            | Operator::F64Load { memory }
            | Operator::I32Load8S { memory }
            | Operator::I32Load8U { memory }
            | Operator::I32Load16S { memory }
            | Operator::I32Load16U { memory }
            | Operator::I64Load8S { memory }
            | Operator::I64Load8U { memory }
            | Operator::I64Load16S { memory }
            | Operator::I64Load16U { memory }
            | Operator::I64Load32S { memory }
            | Operator::I64Load32U { memory }
            | Operator::V128Load { memory }
            | Operator::V128Load8x8S { memory }
            | Operator::V128Load8x8U { memory }
            | Operator::V128Load16x4S { memory }
            | Operator::V128Load16x4U { memory }
            | Operator::V128Load32x2S { memory }
            | Operator::V128Load32x2U { memory }
            | Operator::V128Load8Splat { memory }
            | Operator::V128Load16Splat { memory }
            | Operator::V128Load32Splat { memory }
            | Operator::V128Load64Splat { memory }
            | Operator::V128Load32Zero { memory }
            | Operator::V128Load64Zero { memory }
            | Operator::V128Load8Lane { memory, .. }
            | Operator::V128Load16Lane { memory, .. }
            | Operator::V128Load32Lane { memory, .. }
            | Operator::V128Load64Lane { memory, .. }
            | Operator::I32Store { memory }
            | Operator::I64Store { memory }
            | Operator::F32Store { memory }
            | Operator::F64Store { memory }
            | Operator::I32Store8 { memory }
            | Operator::I32Store16 { memory }
            | Operator::I64Store8 { memory }
            | Operator::I64Store16 { memory }
            | Operator::I64Store32 { memory }
            | Operator::V128Store { memory }
            | Operator::V128Store8Lane { memory, .. }
            | Operator::V128Store16Lane { memory, .. }
            | Operator::V128Store32Lane { memory, .. }
            | Operator::V128Store64Lane { memory, .. } => f(memory),
            _ => {}
        }
    }

    /// Is the operator capable of trapping?
    pub fn can_trap(&self) -> bool {
        self.effects().contains(&SideEffect::Trap)
    }
}

impl std::fmt::Display for Operator {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            &Operator::Unreachable => write!(f, "unreachable")?,
            &Operator::Nop => write!(f, "nop")?,

            &Operator::Call { function_index } => write!(f, "call<{}>", function_index)?,
            &Operator::CallIndirect {
                sig_index,
                table_index,
            } => write!(f, "call_indirect<{}, {}>", sig_index, table_index)?,

            &Operator::Select => write!(f, "select")?,
            &Operator::TypedSelect { ty } => write!(f, "typed_select<{}>", ty)?,
            &Operator::GlobalGet { global_index, .. } => write!(f, "global_get<{}>", global_index)?,
            &Operator::GlobalSet { global_index, .. } => write!(f, "global_set<{}>", global_index)?,

            Operator::I32Load { memory } => write!(f, "i32load<{}>", memory)?,
            Operator::I32Load8S { memory } => write!(f, "i32load8s<{}>", memory)?,
            Operator::I32Load8U { memory } => write!(f, "i32load8u<{}>", memory)?,
            Operator::I32Load16S { memory } => write!(f, "i32load16s<{}>", memory)?,
            Operator::I32Load16U { memory } => write!(f, "i32load16u<{}>", memory)?,
            Operator::I64Load { memory } => write!(f, "i64load<{}>", memory)?,
            Operator::I64Load8S { memory } => write!(f, "i64load8s<{}>", memory)?,
            Operator::I64Load8U { memory } => write!(f, "i64load8u<{}>", memory)?,
            Operator::I64Load16S { memory } => write!(f, "i64load16s<{}>", memory)?,
            Operator::I64Load16U { memory } => write!(f, "i64load16u<{}>", memory)?,
            Operator::I64Load32S { memory } => write!(f, "i64load32s<{}>", memory)?,
            Operator::I64Load32U { memory } => write!(f, "i64load32u<{}>", memory)?,
            Operator::F32Load { memory } => write!(f, "f32load<{}>", memory)?,
            Operator::F64Load { memory } => write!(f, "f64load<{}>", memory)?,

            Operator::I32Store { memory } => write!(f, "i32store<{}>", memory)?,
            Operator::I64Store { memory } => write!(f, "i64store<{}>", memory)?,
            Operator::F32Store { memory } => write!(f, "f32store<{}>", memory)?,
            Operator::F64Store { memory } => write!(f, "f64store<{}>", memory)?,
            Operator::I32Store8 { memory } => write!(f, "i32store8<{}>", memory)?,
            Operator::I32Store16 { memory } => write!(f, "i32store16<{}>", memory)?,
            Operator::I64Store8 { memory } => write!(f, "i64store8<{}>", memory)?,
            Operator::I64Store16 { memory } => write!(f, "i64store16<{}>", memory)?,
            Operator::I64Store32 { memory } => write!(f, "i64store32<{}>", memory)?,

            Operator::I32Const { value } => write!(f, "i32const<{}>", value)?,
            Operator::I64Const { value } => write!(f, "i64const<{}>", value)?,
            Operator::F32Const { value } => write!(f, "f32const<{}>", value)?,
            Operator::F64Const { value } => write!(f, "f64const<{}>", value)?,

            Operator::I32Eqz => write!(f, "i32eqz")?,
            Operator::I32Eq => write!(f, "i32eq")?,
            Operator::I32Ne => write!(f, "i32ne")?,
            Operator::I32LtS => write!(f, "i32lts")?,
            Operator::I32LtU => write!(f, "i32ltu")?,
            Operator::I32GtS => write!(f, "i32gts")?,
            Operator::I32GtU => write!(f, "i32gtu")?,
            Operator::I32LeS => write!(f, "i32les")?,
            Operator::I32LeU => write!(f, "i32leu")?,
            Operator::I32GeS => write!(f, "i64ges")?,
            Operator::I32GeU => write!(f, "i32geu")?,
            Operator::I64Eqz => write!(f, "i64eqz")?,
            Operator::I64Eq => write!(f, "i64eq")?,
            Operator::I64Ne => write!(f, "i64ne")?,
            Operator::I64LtS => write!(f, "i64lts")?,
            Operator::I64LtU => write!(f, "i64ltu")?,
            Operator::I64GtU => write!(f, "i64gtu")?,
            Operator::I64GtS => write!(f, "i64gts")?,
            Operator::I64LeS => write!(f, "i64les")?,
            Operator::I64LeU => write!(f, "i64leu")?,
            Operator::I64GeS => write!(f, "i64ges")?,
            Operator::I64GeU => write!(f, "i64geu")?,
            Operator::F32Eq => write!(f, "f32eq")?,
            Operator::F32Ne => write!(f, "f32ne")?,
            Operator::F32Lt => write!(f, "f32lt")?,
            Operator::F32Gt => write!(f, "f32gt")?,
            Operator::F32Le => write!(f, "f32le")?,
            Operator::F32Ge => write!(f, "f32ge")?,
            Operator::F64Eq => write!(f, "f64eq")?,
            Operator::F64Ne => write!(f, "f64ne")?,
            Operator::F64Lt => write!(f, "f64lt")?,
            Operator::F64Gt => write!(f, "f64gt")?,
            Operator::F64Le => write!(f, "f64le")?,
            Operator::F64Ge => write!(f, "f64ge")?,

            Operator::I32Clz => write!(f, "i32clz")?,
            Operator::I32Ctz => write!(f, "i32ctz")?,
            Operator::I32Popcnt => write!(f, "i32popcnt")?,
            Operator::I32Add => write!(f, "i32add")?,
            Operator::I32Sub => write!(f, "i32sub")?,
            Operator::I32Mul => write!(f, "i32mul")?,
            Operator::I32And => write!(f, "i32and")?,
            Operator::I32Or => write!(f, "i32or")?,
            Operator::I32Xor => write!(f, "i32xor")?,
            Operator::I32Shl => write!(f, "i32shl")?,
            Operator::I32ShrS => write!(f, "i32shrs")?,
            Operator::I32ShrU => write!(f, "i32shru")?,
            Operator::I32Rotl => write!(f, "i32rotl")?,
            Operator::I32Rotr => write!(f, "i32rotr")?,

            Operator::I32DivS => write!(f, "i32divs")?,
            Operator::I32DivU => write!(f, "i32divu")?,
            Operator::I32RemS => write!(f, "i32rems")?,
            Operator::I32RemU => write!(f, "i32remu")?,

            Operator::I64Clz => write!(f, "i64clz")?,
            Operator::I64Ctz => write!(f, "i64ctz")?,
            Operator::I64Popcnt => write!(f, "i64popcnt")?,
            Operator::I64Add => write!(f, "i64add")?,
            Operator::I64Sub => write!(f, "i64sub")?,
            Operator::I64Mul => write!(f, "i64mul")?,
            Operator::I64And => write!(f, "i64and")?,
            Operator::I64Or => write!(f, "i64or")?,
            Operator::I64Xor => write!(f, "i64xor")?,
            Operator::I64Shl => write!(f, "i64shl")?,
            Operator::I64ShrS => write!(f, "i64shrs")?,
            Operator::I64ShrU => write!(f, "i64shru")?,
            Operator::I64Rotl => write!(f, "i64rotl")?,
            Operator::I64Rotr => write!(f, "i64rotr")?,

            Operator::I64DivS => write!(f, "i64divs")?,
            Operator::I64DivU => write!(f, "i64divu")?,
            Operator::I64RemS => write!(f, "i64rems")?,
            Operator::I64RemU => write!(f, "i64remu")?,

            Operator::F32Abs => write!(f, "f32abs")?,
            Operator::F32Neg => write!(f, "f32neg")?,
            Operator::F32Ceil => write!(f, "f32ceil")?,
            Operator::F32Floor => write!(f, "f32floor")?,
            Operator::F32Trunc => write!(f, "f32trunc")?,
            Operator::F32Nearest => write!(f, "f32nearest")?,
            Operator::F32Sqrt => write!(f, "f32sqrt")?,
            Operator::F32Add => write!(f, "f32add")?,
            Operator::F32Sub => write!(f, "f32sub")?,
            Operator::F32Mul => write!(f, "f32mul")?,
            Operator::F32Div => write!(f, "f32div")?,
            Operator::F32Min => write!(f, "f32min")?,
            Operator::F32Max => write!(f, "f32max")?,
            Operator::F32Copysign => write!(f, "f32copysign")?,

            Operator::F64Abs => write!(f, "f64abs")?,
            Operator::F64Neg => write!(f, "f64neg")?,
            Operator::F64Ceil => write!(f, "f64ceil")?,
            Operator::F64Floor => write!(f, "f64flor")?,
            Operator::F64Trunc => write!(f, "f64trunc")?,
            Operator::F64Nearest => write!(f, "f64nearest")?,
            Operator::F64Sqrt => write!(f, "f64sqrt")?,
            Operator::F64Add => write!(f, "f64add")?,
            Operator::F64Sub => write!(f, "f64sub")?,
            Operator::F64Mul => write!(f, "f64mul")?,
            Operator::F64Div => write!(f, "f64div")?,
            Operator::F64Min => write!(f, "f64min")?,
            Operator::F64Max => write!(f, "f64max")?,
            Operator::F64Copysign => write!(f, "f64copysign")?,

            Operator::I32WrapI64 => write!(f, "i32wrapi64")?,
            Operator::I32TruncF32S => write!(f, "i32truncf32s")?,
            Operator::I32TruncF32U => write!(f, "i32truncf32u")?,
            Operator::I32TruncF64S => write!(f, "i32truncf64s")?,
            Operator::I32TruncF64U => write!(f, "i32truncf64u")?,
            Operator::I64ExtendI32S => write!(f, "i64extendi32s")?,
            Operator::I64ExtendI32U => write!(f, "i64extendi32u")?,
            Operator::I64TruncF32S => write!(f, "i64truncf32s")?,
            Operator::I64TruncF32U => write!(f, "i64truncf32u")?,
            Operator::I64TruncF64S => write!(f, "i64truncf64s")?,
            Operator::I64TruncF64U => write!(f, "i64truncf64u")?,
            Operator::F32ConvertI32S => write!(f, "f32converti32s")?,
            Operator::F32ConvertI32U => write!(f, "f32converti32u")?,
            Operator::F32ConvertI64S => write!(f, "f32converti64s")?,
            Operator::F32ConvertI64U => write!(f, "f32converti64u")?,
            Operator::F32DemoteF64 => write!(f, "f32demotef64")?,
            Operator::F64ConvertI32S => write!(f, "f64converti32s")?,
            Operator::F64ConvertI32U => write!(f, "f64converti32u")?,
            Operator::F64ConvertI64S => write!(f, "f64converti64s")?,
            Operator::F64ConvertI64U => write!(f, "f64converti64u")?,
            Operator::F64PromoteF32 => write!(f, "f64promotef32")?,
            Operator::I32Extend8S => write!(f, "i32extend8s")?,
            Operator::I32Extend16S => write!(f, "i32extend16s")?,
            Operator::I64Extend8S => write!(f, "i64extend8s")?,
            Operator::I64Extend16S => write!(f, "i64extend16s")?,
            Operator::I64Extend32S => write!(f, "i64extend32s")?,
            Operator::I32TruncSatF32S => write!(f, "i32truncsatf32s")?,
            Operator::I32TruncSatF32U => write!(f, "i32truncsatf32u")?,
            Operator::I32TruncSatF64S => write!(f, "i32truncsatf64s")?,
            Operator::I32TruncSatF64U => write!(f, "i32truncsatf64u")?,
            Operator::I64TruncSatF32S => write!(f, "i64truncsatf32s")?,
            Operator::I64TruncSatF32U => write!(f, "i64truncsatf32u")?,
            Operator::I64TruncSatF64S => write!(f, "i64truncsatf64s")?,
            Operator::I64TruncSatF64U => write!(f, "i64truncsatf64u")?,
            Operator::F32ReinterpretI32 => write!(f, "f32reinterpreti32")?,
            Operator::F64ReinterpretI64 => write!(f, "f64reinterpreti64")?,
            Operator::I32ReinterpretF32 => write!(f, "i32reinterpretf32")?,
            Operator::I64ReinterpretF64 => write!(f, "i64reinterpretf64")?,
            Operator::TableGet { table_index, .. } => write!(f, "table_get<{}>", table_index)?,
            Operator::TableSet { table_index, .. } => write!(f, "table_set<{}>", table_index)?,
            Operator::TableGrow { table_index, .. } => write!(f, "table_grow<{}>", table_index)?,
            Operator::TableSize { table_index, .. } => write!(f, "table_size<{}>", table_index)?,
            Operator::MemorySize { mem } => write!(f, "memory_size<{}>", mem)?,
            Operator::MemoryGrow { mem } => write!(f, "memory_grow<{}>", mem)?,
            Operator::MemoryCopy { dst_mem, src_mem } => {
                write!(f, "memory_copy<{}, {}>", dst_mem, src_mem)?
            }
            Operator::MemoryFill { mem } => write!(f, "memory_fill<{}>", mem)?,

            Operator::V128Load { memory } => write!(f, "v128load<{}>", memory)?,
            Operator::V128Load8x8S { memory } => write!(f, "v128load8x8s<{}>", memory)?,
            Operator::V128Load8x8U { memory } => write!(f, "v128load8x8u<{}>", memory)?,
            Operator::V128Load16x4S { memory } => write!(f, "v128load16x4s<{}>", memory)?,
            Operator::V128Load16x4U { memory } => write!(f, "v128load16x4u<{}>", memory)?,
            Operator::V128Load32x2S { memory } => write!(f, "v128load32x2s<{}>", memory)?,
            Operator::V128Load32x2U { memory } => write!(f, "v128load32x2u<{}>", memory)?,
            Operator::V128Load8Splat { memory } => write!(f, "v128load8splat<{}>", memory)?,
            Operator::V128Load16Splat { memory } => write!(f, "v128load16splat<{}>", memory)?,
            Operator::V128Load32Splat { memory } => write!(f, "v128load32splat<{}>", memory)?,
            Operator::V128Load64Splat { memory } => write!(f, "v128load64splat<{}>", memory)?,
            Operator::V128Load32Zero { memory } => write!(f, "v128load32zero<{}>", memory)?,
            Operator::V128Load64Zero { memory } => write!(f, "v128load64zero<{}>", memory)?,
            Operator::V128Store { memory } => write!(f, "v128store<{}>", memory)?,
            Operator::V128Load8Lane { memory, lane } => {
                write!(f, "v128load8lane<{}, {}>", memory, lane)?
            }
            Operator::V128Load16Lane { memory, lane } => {
                write!(f, "v128load16lane<{}, {}>", memory, lane)?
            }
            Operator::V128Load32Lane { memory, lane } => {
                write!(f, "v128load32lane<{}, {}>", memory, lane)?
            }
            Operator::V128Load64Lane { memory, lane } => {
                write!(f, "v128load64lane<{}, {}>", memory, lane)?
            }
            Operator::V128Store8Lane { memory, lane } => {
                write!(f, "v128store8lane<{}, {}>", memory, lane)?
            }
            Operator::V128Store16Lane { memory, lane } => {
                write!(f, "v128store16lane<{}, {}>", memory, lane)?
            }
            Operator::V128Store32Lane { memory, lane } => {
                write!(f, "v128store32lane<{}, {}>", memory, lane)?
            }
            Operator::V128Store64Lane { memory, lane } => {
                write!(f, "v128store64lane<{}, {}>", memory, lane)?
            }

            Operator::V128Const { value } => write!(f, "v128const<{}>", value)?,
            Operator::I8x16Shuffle { lanes } => write!(f, "i8x16shuffle<{:?}>", lanes)?,
            Operator::I8x16ExtractLaneS { lane } => write!(f, "i8x16extractlanes<{}>", lane)?,
            Operator::I8x16ExtractLaneU { lane } => write!(f, "i8x16extractlaneu<{}>", lane)?,
            Operator::I8x16ReplaceLane { lane } => write!(f, "i8x16replacelane<{}>", lane)?,
            Operator::I16x8ExtractLaneS { lane } => write!(f, "i16x8extractlanes<{}>", lane)?,
            Operator::I16x8ExtractLaneU { lane } => write!(f, "i16x8extractlaneu<{}>", lane)?,
            Operator::I16x8ReplaceLane { lane } => write!(f, "i16x8replacelane<{}>", lane)?,
            Operator::I32x4ExtractLane { lane } => write!(f, "i32x4extractlane<{}>", lane)?,
            Operator::I32x4ReplaceLane { lane } => write!(f, "i32x4replacelane<{}>", lane)?,
            Operator::I64x2ExtractLane { lane } => write!(f, "i64x2extractlane<{}>", lane)?,
            Operator::I64x2ReplaceLane { lane } => write!(f, "i64x2replacelane<{}>", lane)?,
            Operator::F32x4ExtractLane { lane } => write!(f, "f32x4extractlane<{}>", lane)?,
            Operator::F32x4ReplaceLane { lane } => write!(f, "f32x4replacelane<{}>", lane)?,
            Operator::F64x2ExtractLane { lane } => write!(f, "f64x2extractlane<{}>", lane)?,
            Operator::F64x2ReplaceLane { lane } => write!(f, "f64x2replacelane<{}>", lane)?,

            Operator::I8x16Swizzle => write!(f, "i8x16swizzle")?,
            Operator::I8x16Splat => write!(f, "i8x16splat")?,
            Operator::I16x8Splat => write!(f, "i16x8splat")?,
            Operator::I32x4Splat => write!(f, "i32x4splat")?,
            Operator::I64x2Splat => write!(f, "i64x2splat")?,
            Operator::F32x4Splat => write!(f, "f32x4splat")?,
            Operator::F64x2Splat => write!(f, "f64x2splat")?,

            Operator::I8x16Eq => write!(f, "i8x16eq")?,
            Operator::I8x16Ne => write!(f, "i8x16ne")?,
            Operator::I8x16LtS => write!(f, "i8x16lts")?,
            Operator::I8x16LtU => write!(f, "i8x16ltu")?,
            Operator::I8x16GtS => write!(f, "i8x16gts")?,
            Operator::I8x16GtU => write!(f, "i8x16gtu")?,
            Operator::I8x16LeS => write!(f, "i8x16les")?,
            Operator::I8x16LeU => write!(f, "i8x16leu")?,
            Operator::I8x16GeS => write!(f, "i8x16ges")?,
            Operator::I8x16GeU => write!(f, "i8x16geu")?,

            Operator::I16x8Eq => write!(f, "i16x8eq")?,
            Operator::I16x8Ne => write!(f, "i16x8ne")?,
            Operator::I16x8LtS => write!(f, "i16x8lts")?,
            Operator::I16x8LtU => write!(f, "i16x8ltu")?,
            Operator::I16x8GtS => write!(f, "i16x8gts")?,
            Operator::I16x8GtU => write!(f, "i16x8gtu")?,
            Operator::I16x8LeS => write!(f, "i16x8les")?,
            Operator::I16x8LeU => write!(f, "i16x8leu")?,
            Operator::I16x8GeS => write!(f, "i16x8ges")?,
            Operator::I16x8GeU => write!(f, "i16x8geu")?,

            Operator::I32x4Eq => write!(f, "i32x4eq")?,
            Operator::I32x4Ne => write!(f, "i32x4ne")?,
            Operator::I32x4LtS => write!(f, "i32x4lts")?,
            Operator::I32x4LtU => write!(f, "i32x4ltu")?,
            Operator::I32x4GtS => write!(f, "i32x4gts")?,
            Operator::I32x4GtU => write!(f, "i32x4gtu")?,
            Operator::I32x4LeS => write!(f, "i32x4les")?,
            Operator::I32x4LeU => write!(f, "i32x4leu")?,
            Operator::I32x4GeS => write!(f, "i32x4ges")?,
            Operator::I32x4GeU => write!(f, "i32x4geu")?,

            Operator::I64x2Eq => write!(f, "i64x2eq")?,
            Operator::I64x2Ne => write!(f, "i64x2ne")?,
            Operator::I64x2LtS => write!(f, "i64x2lts")?,
            Operator::I64x2GtS => write!(f, "i64x2gts")?,
            Operator::I64x2LeS => write!(f, "i64x2les")?,
            Operator::I64x2GeS => write!(f, "i64x2ges")?,

            Operator::F32x4Eq => write!(f, "f32x4eq")?,
            Operator::F32x4Ne => write!(f, "f32x4ne")?,
            Operator::F32x4Lt => write!(f, "f32x4lt")?,
            Operator::F32x4Gt => write!(f, "f32x2gt")?,
            Operator::F32x4Le => write!(f, "f32x4le")?,
            Operator::F32x4Ge => write!(f, "f32x4ge")?,

            Operator::F64x2Eq => write!(f, "f64x2eq")?,
            Operator::F64x2Ne => write!(f, "f64x2ne")?,
            Operator::F64x2Lt => write!(f, "f64x2lt")?,
            Operator::F64x2Gt => write!(f, "f64x2gt")?,
            Operator::F64x2Le => write!(f, "f64x2le")?,
            Operator::F64x2Ge => write!(f, "f64x2ge")?,

            Operator::V128Not => write!(f, "v128not")?,
            Operator::V128And => write!(f, "v128and")?,
            Operator::V128AndNot => write!(f, "v128andnot")?,
            Operator::V128Or => write!(f, "v128or")?,
            Operator::V128Xor => write!(f, "v128xor")?,
            Operator::V128Bitselect => write!(f, "v128bitselect")?,
            Operator::V128AnyTrue => write!(f, "v128anytrue")?,

            Operator::I8x16Abs => write!(f, "i8x16abs")?,
            Operator::I8x16Neg => write!(f, "i8x16neg")?,
            Operator::I8x16Popcnt => write!(f, "i8x16popcnt")?,
            Operator::I8x16AllTrue => write!(f, "i8x16alltrue")?,
            Operator::I8x16Bitmask => write!(f, "i8x16bitmask")?,
            Operator::I8x16NarrowI16x8S => write!(f, "i8x16narrow16x8s")?,
            Operator::I8x16NarrowI16x8U => write!(f, "i8x16narrow16x8u")?,
            Operator::I8x16Shl => write!(f, "i8x16shl")?,
            Operator::I8x16ShrS => write!(f, "i8x16shrs")?,
            Operator::I8x16ShrU => write!(f, "i8x16shru")?,
            Operator::I8x16Add => write!(f, "i8x16add")?,
            Operator::I8x16AddSatS => write!(f, "i8x16addsats")?,
            Operator::I8x16AddSatU => write!(f, "i8x16addsatu")?,
            Operator::I8x16Sub => write!(f, "i8x16sub")?,
            Operator::I8x16SubSatS => write!(f, "i8x16subsats")?,
            Operator::I8x16SubSatU => write!(f, "i8x16subsatu")?,
            Operator::I8x16MinS => write!(f, "i8x16mins")?,
            Operator::I8x16MinU => write!(f, "i8x16minu")?,
            Operator::I8x16MaxS => write!(f, "i8x16maxs")?,
            Operator::I8x16MaxU => write!(f, "i8x16maxu")?,
            Operator::I8x16AvgrU => write!(f, "i8x16avgru")?,

            Operator::I16x8ExtAddPairwiseI8x16S => write!(f, "i16x8extaddpairwisei8x16s")?,
            Operator::I16x8ExtAddPairwiseI8x16U => write!(f, "i16x8extaddpairwisei8x16u")?,
            Operator::I16x8Abs => write!(f, "i16x8abs")?,
            Operator::I16x8Neg => write!(f, "i16x8neg")?,
            Operator::I16x8Q15MulrSatS => write!(f, "i16x8q15mulrsats")?,
            Operator::I16x8AllTrue => write!(f, "i16x8alltrue")?,
            Operator::I16x8Bitmask => write!(f, "i16x8bitmask")?,
            Operator::I16x8NarrowI32x4S => write!(f, "i16x8narrowi32x4s")?,
            Operator::I16x8NarrowI32x4U => write!(f, "i16x8narrowi32x4u")?,
            Operator::I16x8ExtendLowI8x16S => write!(f, "i16x8extendlowi8x16s")?,
            Operator::I16x8ExtendHighI8x16S => write!(f, "i16x8extendhighi8x16s")?,
            Operator::I16x8ExtendLowI8x16U => write!(f, "i16x8extendlowi8x16u")?,
            Operator::I16x8ExtendHighI8x16U => write!(f, "i16x8extendhighi8x16u")?,
            Operator::I16x8Shl => write!(f, "i16x8shl")?,
            Operator::I16x8ShrS => write!(f, "i16x8shrs")?,
            Operator::I16x8ShrU => write!(f, "i16x8shru")?,
            Operator::I16x8Add => write!(f, "i16x8add")?,
            Operator::I16x8AddSatS => write!(f, "i16x8addsats")?,
            Operator::I16x8AddSatU => write!(f, "i16x8addsatu")?,
            Operator::I16x8Sub => write!(f, "i16x8sub")?,
            Operator::I16x8SubSatS => write!(f, "i16x8subsats")?,
            Operator::I16x8SubSatU => write!(f, "i16x8subsatu")?,
            Operator::I16x8Mul => write!(f, "i16x8mul")?,
            Operator::I16x8MinS => write!(f, "i16x8mins")?,
            Operator::I16x8MinU => write!(f, "i16x8minu")?,
            Operator::I16x8MaxS => write!(f, "i16x8maxs")?,
            Operator::I16x8MaxU => write!(f, "i16x8maxu")?,
            Operator::I16x8AvgrU => write!(f, "i16x8avgru")?,
            Operator::I16x8ExtMulLowI8x16S => write!(f, "i16x8extmullowi8x16s")?,
            Operator::I16x8ExtMulHighI8x16S => write!(f, "i16x8extmulhighi8x16s")?,
            Operator::I16x8ExtMulLowI8x16U => write!(f, "i16x8extmullowi8x16u")?,
            Operator::I16x8ExtMulHighI8x16U => write!(f, "i16x8extmulhighi8x16u")?,

            Operator::I32x4ExtAddPairwiseI16x8S => write!(f, "i32x4extaddpairwisei16x8s")?,
            Operator::I32x4ExtAddPairwiseI16x8U => write!(f, "i32x4extaddpairwisei16x8u")?,
            Operator::I32x4Abs => write!(f, "i32x4abs")?,
            Operator::I32x4Neg => write!(f, "i32x4neg")?,
            Operator::I32x4AllTrue => write!(f, "i32x4alltrue")?,
            Operator::I32x4Bitmask => write!(f, "i32x4bitmask")?,
            Operator::I32x4ExtendLowI16x8S => write!(f, "i32x4extendlowi16x8s")?,
            Operator::I32x4ExtendHighI16x8S => write!(f, "i32x4extendhighi16x8s")?,
            Operator::I32x4ExtendLowI16x8U => write!(f, "i32x4extendlowi16x8u")?,
            Operator::I32x4ExtendHighI16x8U => write!(f, "i32x4extendhighi16x8u")?,
            Operator::I32x4Shl => write!(f, "i32x4shl")?,
            Operator::I32x4ShrS => write!(f, "i32x4shrs")?,
            Operator::I32x4ShrU => write!(f, "i32x4shru")?,
            Operator::I32x4Add => write!(f, "i32x4add")?,
            Operator::I32x4Sub => write!(f, "i32x4sub")?,
            Operator::I32x4Mul => write!(f, "i32x4mul")?,
            Operator::I32x4MinS => write!(f, "i32x4mins")?,
            Operator::I32x4MinU => write!(f, "i32x4minu")?,
            Operator::I32x4MaxS => write!(f, "i32x4maxs")?,
            Operator::I32x4MaxU => write!(f, "i32x4maxu")?,
            Operator::I32x4DotI16x8S => write!(f, "i32x4doti16x8s")?,
            Operator::I32x4ExtMulLowI16x8S => write!(f, "i32x4extmullowi16x8s")?,
            Operator::I32x4ExtMulHighI16x8S => write!(f, "i32x4extmulhighi16x8s")?,
            Operator::I32x4ExtMulLowI16x8U => write!(f, "i32x4extmullowi16x8u")?,
            Operator::I32x4ExtMulHighI16x8U => write!(f, "i32x4extmulhighi16x8u")?,

            Operator::I64x2Abs => write!(f, "i64x2abs")?,
            Operator::I64x2Neg => write!(f, "i64x2neg")?,
            Operator::I64x2AllTrue => write!(f, "i64x2alltrue")?,
            Operator::I64x2Bitmask => write!(f, "i64x2bitmask")?,
            Operator::I64x2ExtendLowI32x4S => write!(f, "i64x2extendlowi32x4s")?,
            Operator::I64x2ExtendHighI32x4S => write!(f, "i64x2extendhighi32x4s")?,
            Operator::I64x2ExtendLowI32x4U => write!(f, "i64x2extendlowi32x4u")?,
            Operator::I64x2ExtendHighI32x4U => write!(f, "i64x2extendhighi32x4u")?,
            Operator::I64x2Shl => write!(f, "i64x2shl")?,
            Operator::I64x2ShrS => write!(f, "i64x2shrs")?,
            Operator::I64x2ShrU => write!(f, "i64x2shru")?,
            Operator::I64x2Add => write!(f, "i64x2add")?,
            Operator::I64x2Sub => write!(f, "i64x2sub")?,
            Operator::I64x2Mul => write!(f, "i64x2mul")?,
            Operator::I64x2ExtMulLowI32x4S => write!(f, "i64x2extmullowi32x4s")?,
            Operator::I64x2ExtMulHighI32x4S => write!(f, "i64x2extmulhighi32x4s")?,
            Operator::I64x2ExtMulLowI32x4U => write!(f, "i64x2extmullowi32x4u")?,
            Operator::I64x2ExtMulHighI32x4U => write!(f, "i64x2extmulhighi32x4u")?,

            Operator::F32x4Ceil => write!(f, "f32x4ceil")?,
            Operator::F32x4Floor => write!(f, "f32x4floor")?,
            Operator::F32x4Trunc => write!(f, "f32x4trunc")?,
            Operator::F32x4Nearest => write!(f, "f32x4nearest")?,
            Operator::F32x4Abs => write!(f, "f32x4abs")?,
            Operator::F32x4Neg => write!(f, "f32x4neg")?,
            Operator::F32x4Sqrt => write!(f, "f32x4sqrt")?,
            Operator::F32x4Add => write!(f, "f32x4add")?,
            Operator::F32x4Sub => write!(f, "f32x4sub")?,
            Operator::F32x4Mul => write!(f, "f32x4mul")?,
            Operator::F32x4Div => write!(f, "f32x4div")?,
            Operator::F32x4Min => write!(f, "f32x4min")?,
            Operator::F32x4Max => write!(f, "f32x4max")?,
            Operator::F32x4PMin => write!(f, "f32x4pmin")?,
            Operator::F32x4PMax => write!(f, "f32x4pmax")?,

            Operator::F64x2Ceil => write!(f, "f64x2ceil")?,
            Operator::F64x2Floor => write!(f, "f64x2floor")?,
            Operator::F64x2Trunc => write!(f, "f64x2trunc")?,
            Operator::F64x2Nearest => write!(f, "f64x2nearest")?,
            Operator::F64x2Abs => write!(f, "f64x2abs")?,
            Operator::F64x2Neg => write!(f, "f64x2neg")?,
            Operator::F64x2Sqrt => write!(f, "f64x2sqrt")?,
            Operator::F64x2Add => write!(f, "f64x2add")?,
            Operator::F64x2Sub => write!(f, "f64x2sub")?,
            Operator::F64x2Mul => write!(f, "f64x2mul")?,
            Operator::F64x2Div => write!(f, "f64x2div")?,
            Operator::F64x2Min => write!(f, "f64x2min")?,
            Operator::F64x2Max => write!(f, "f64x2max")?,
            Operator::F64x2PMin => write!(f, "f64x2pmin")?,
            Operator::F64x2PMax => write!(f, "f64x2pmax")?,

            Operator::I32x4TruncSatF32x4S => write!(f, "i32x4truncsatf32x4s")?,
            Operator::I32x4TruncSatF32x4U => write!(f, "i32x4truncsatf32x4u")?,

            Operator::F32x4ConvertI32x4S => write!(f, "f32x4converti32x4s")?,
            Operator::F32x4ConvertI32x4U => write!(f, "f32x4converti32x4u")?,
            Operator::I32x4TruncSatF64x2SZero => write!(f, "i32x4truncsatf64x2szero")?,
            Operator::I32x4TruncSatF64x2UZero => write!(f, "i32x4truncsatf64x2uzero")?,
            Operator::F64x2ConvertLowI32x4S => write!(f, "f64x2convertlowi32x4s")?,
            Operator::F64x2ConvertLowI32x4U => write!(f, "f64x2convertlowi32x4u")?,
            Operator::F32x4DemoteF64x2Zero => write!(f, "f32x4demotef64x2zero")?,
            Operator::F64x2PromoteLowF32x4 => write!(f, "f64x2promotelowf32x4")?,

            Operator::CallRef { sig_index } => write!(f, "call_ref<{}>", sig_index)?,
            Operator::RefIsNull => write!(f, "ref_is_null")?,
            Operator::RefNull { sig_index } => write!(f, "ref_null<{}>", sig_index)?,
            Operator::RefFunc { func_index } => write!(f, "ref_func<{}>", func_index)?,
        }

        Ok(())
    }
}

/// Should we rematerialize this operator when generating Wasm
/// bytecode?
pub(crate) fn op_rematerialize(op: &Operator) -> bool {
    match op {
        // constants are much cheaper (in code space and in terms of
        // indirect effects on code quality) to always generate at
        // point-of-use rather than store in a Wasm local.
        &Operator::I32Const { .. }
        | &Operator::I64Const { .. }
        | &Operator::F32Const { .. }
        | &Operator::F64Const { .. } => true,
        _ => false,
    }
}
