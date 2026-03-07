use crate::ir::types::IrType;

/// An opaque, index-based reference to an SSA value within a function.
///
/// Invariant: `ValueId(n)` is only valid within the `IrFunction` that produced
/// it. Do not store `ValueId`s across function boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueId(pub u32);

impl std::fmt::Display for ValueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "%{}", self.0)
    }
}

/// A block parameter in SSA form.
///
/// IRIS uses block-parameter style SSA (as in MLIR/Swift SIL) rather than
/// explicit phi instructions. Entry-block parameters are function arguments.
#[derive(Debug, Clone)]
pub struct BlockParam {
    pub id: ValueId,
    pub ty: IrType,
    pub name: Option<String>,
}

/// The definition site of an SSA value.
/// Every `ValueId` in a function must have exactly one `ValueDef`.
#[derive(Debug, Clone)]
pub enum ValueDef {
    /// Defined as a block parameter (entry block params are function args).
    BlockParam { block: crate::ir::block::BlockId },
    /// Defined as the result of an instruction.
    InstrResult {
        block: crate::ir::block::BlockId,
        instr: crate::ir::instr::InstrId,
    },
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::block::BlockId;
    use crate::ir::instr::InstrId;
    use crate::ir::types::{DType, IrType};

    // -- ValueId --------------------------------------------------------------

    #[test]
    fn value_id_display() {
        assert_eq!(format!("{}", ValueId(0)), "%0");
        assert_eq!(format!("{}", ValueId(99)), "%99");
    }

    #[test]
    fn value_id_equality() {
        assert_eq!(ValueId(0), ValueId(0));
        assert_ne!(ValueId(0), ValueId(1));
    }

    #[test]
    fn value_id_ordering() {
        assert!(ValueId(0) < ValueId(1));
        assert!(ValueId(10) > ValueId(5));
    }

    #[test]
    fn value_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ValueId(0));
        set.insert(ValueId(1));
        set.insert(ValueId(0));
        assert_eq!(set.len(), 2);
    }

    // -- BlockParam -----------------------------------------------------------

    #[test]
    fn block_param_with_name() {
        let p = BlockParam {
            id: ValueId(0),
            ty: IrType::Scalar(DType::I64),
            name: Some("x".into()),
        };
        assert_eq!(p.name.as_deref(), Some("x"));
        assert_eq!(p.id, ValueId(0));
    }

    #[test]
    fn block_param_unnamed() {
        let p = BlockParam {
            id: ValueId(1),
            ty: IrType::Str,
            name: None,
        };
        assert!(p.name.is_none());
    }

    // -- ValueDef -------------------------------------------------------------

    #[test]
    fn value_def_block_param() {
        let def = ValueDef::BlockParam {
            block: BlockId(0),
        };
        assert!(matches!(def, ValueDef::BlockParam { block } if block == BlockId(0)));
    }

    #[test]
    fn value_def_instr_result() {
        let def = ValueDef::InstrResult {
            block: BlockId(1),
            instr: InstrId(3),
        };
        assert!(matches!(def, ValueDef::InstrResult { block, instr } if block == BlockId(1) && instr == InstrId(3)));
    }
}
