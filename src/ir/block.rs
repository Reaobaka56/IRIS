use crate::ir::instr::IrInstr;
use crate::ir::value::{BlockParam, ValueId};

/// An opaque index identifying a basic block within an `IrFunction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(pub u32);

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

/// A basic block in SSA form.
///
/// Invariants enforced by `IrFunctionBuilder::build()`:
/// 1. `instrs` is non-empty — at minimum a terminator must be present.
/// 2. Exactly one terminator exists and it is always the last element of `instrs`.
/// 3. `params` are considered defined before any instruction in this block.
/// 4. Each `ValueId` in `instrs` and `params` is unique within the function.
#[derive(Debug, Clone)]
pub struct IrBlock {
    pub id: BlockId,
    /// Block parameters model phi nodes (block-param SSA style).
    pub params: Vec<BlockParam>,
    /// Instructions in program order. Terminator is last.
    pub instrs: Vec<IrInstr>,
    /// Optional display name used by the pretty-printer.
    pub name: Option<String>,
}

impl IrBlock {
    pub fn new(id: BlockId, name: Option<String>) -> Self {
        Self {
            id,
            params: Vec::new(),
            instrs: Vec::new(),
            name,
        }
    }

    /// Returns the terminator instruction if the block is sealed.
    pub fn terminator(&self) -> Option<&IrInstr> {
        self.instrs.last().filter(|i| i.is_terminator())
    }

    /// A block is sealed when it ends with a terminator.
    pub fn is_sealed(&self) -> bool {
        self.terminator().is_some()
    }

    /// Iterates over all `ValueId`s used as operands across all instructions.
    pub fn all_operands(&self) -> impl Iterator<Item = ValueId> + '_ {
        self.instrs.iter().flat_map(|i| i.operands())
    }

    /// Iterates over all `ValueId`s defined in this block (params + instr results).
    pub fn all_defs(&self) -> impl Iterator<Item = ValueId> + '_ {
        let param_ids = self.params.iter().map(|p| p.id);
        let result_ids = self.instrs.iter().filter_map(|i| i.result());
        param_ids.chain(result_ids)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::types::{DType, IrType};
    use crate::ir::value::ValueId;

    #[test]
    fn block_id_display() {
        assert_eq!(format!("{}", BlockId(0)), "bb0");
        assert_eq!(format!("{}", BlockId(42)), "bb42");
    }

    #[test]
    fn block_id_equality() {
        assert_eq!(BlockId(0), BlockId(0));
        assert_ne!(BlockId(0), BlockId(1));
    }

    #[test]
    fn block_id_ordering() {
        assert!(BlockId(0) < BlockId(1));
        assert!(BlockId(5) > BlockId(3));
    }

    #[test]
    fn block_new_empty() {
        let b = IrBlock::new(BlockId(0), Some("entry".into()));
        assert_eq!(b.id, BlockId(0));
        assert_eq!(b.name.as_deref(), Some("entry"));
        assert!(b.params.is_empty());
        assert!(b.instrs.is_empty());
    }

    #[test]
    fn block_new_unnamed() {
        let b = IrBlock::new(BlockId(1), None);
        assert!(b.name.is_none());
    }

    #[test]
    fn block_not_sealed_when_empty() {
        let b = IrBlock::new(BlockId(0), None);
        assert!(!b.is_sealed());
        assert!(b.terminator().is_none());
    }

    #[test]
    fn block_sealed_with_return() {
        let mut b = IrBlock::new(BlockId(0), None);
        b.instrs.push(IrInstr::Return { values: vec![] });
        assert!(b.is_sealed());
        assert!(b.terminator().is_some());
    }

    #[test]
    fn block_sealed_with_br() {
        let mut b = IrBlock::new(BlockId(0), None);
        b.instrs.push(IrInstr::Br {
            target: BlockId(1),
            args: vec![],
        });
        assert!(b.is_sealed());
    }

    #[test]
    fn block_sealed_with_condbr() {
        let mut b = IrBlock::new(BlockId(0), None);
        b.instrs.push(IrInstr::CondBr {
            cond: ValueId(0),
            then_block: BlockId(1),
            then_args: vec![],
            else_block: BlockId(2),
            else_args: vec![],
        });
        assert!(b.is_sealed());
    }

    #[test]
    fn block_not_sealed_with_non_terminator() {
        let mut b = IrBlock::new(BlockId(0), None);
        b.instrs.push(IrInstr::ConstInt {
            result: ValueId(0),
            value: 42,
            ty: IrType::Scalar(DType::I64),
        });
        assert!(!b.is_sealed());
    }

    #[test]
    fn block_all_operands() {
        let mut b = IrBlock::new(BlockId(0), None);
        b.instrs.push(IrInstr::BinOp {
            result: ValueId(2),
            op: crate::ir::instr::BinOp::Add,
            lhs: ValueId(0),
            rhs: ValueId(1),
            ty: IrType::Scalar(DType::I64),
        });
        let ops: Vec<ValueId> = b.all_operands().collect();
        assert_eq!(ops, vec![ValueId(0), ValueId(1)]);
    }

    #[test]
    fn block_all_defs_params_and_results() {
        let mut b = IrBlock::new(BlockId(0), None);
        b.params.push(BlockParam {
            id: ValueId(0),
            ty: IrType::Scalar(DType::I64),
            name: Some("x".into()),
        });
        b.instrs.push(IrInstr::ConstInt {
            result: ValueId(1),
            value: 1,
            ty: IrType::Scalar(DType::I64),
        });
        let defs: Vec<ValueId> = b.all_defs().collect();
        assert_eq!(defs, vec![ValueId(0), ValueId(1)]);
    }
}
