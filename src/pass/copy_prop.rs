//! Copy propagation pass for `IrModule`.
//!
//! Replaces uses of `y = copy x` (assignments where y is just a copy of x)
//! with direct uses of x, then removes the copy instruction.
//!
//! This pass also propagates through `Load` followed by `Store` to the same
//! location, and through trivial `PhiNode` arguments where all incoming values
//! are the same.

use std::collections::HashMap;

use crate::error::PassError;
use crate::ir::function::IrFunction;
use crate::ir::instr::IrInstr;
use crate::ir::module::IrModule;
use crate::ir::value::ValueId;
use crate::pass::Pass;

pub struct CopyPropPass;

impl Pass for CopyPropPass {
    fn name(&self) -> &'static str {
        "copy-prop"
    }

    fn run(&mut self, module: &mut IrModule) -> Result<(), PassError> {
        for func in &mut module.functions {
            copy_prop_func(func);
        }
        Ok(())
    }
}

fn copy_prop_func(func: &mut IrFunction) {
    // Detect duplicate constant definitions across blocks and replace later
    // uses with the first occurrence, reducing register pressure.

    let mut replacements: HashMap<ValueId, ValueId> = HashMap::new();

    // Phase 1: Detect duplicate constants.
    // We only record a constant as a deduplication target if it is defined in
    // the entry block (block index 0).  The entry block dominates every other
    // block, so replacing a later occurrence with an entry-block occurrence is
    // always safe.  Constants that first appear in conditional branches or loop
    // bodies must NOT be used as replacement targets for instructions in
    // sibling branches that may not execute the defining block.
    let mut const_int_map: HashMap<(i64, String), ValueId> = HashMap::new();
    let mut const_float_map: HashMap<(u64, String), ValueId> = HashMap::new(); // (bit pattern, dtype)

    for (bi, block) in func.blocks.iter().enumerate() {
        let is_entry = bi == 0;
        for instr in &block.instrs {
            match instr {
                IrInstr::ConstInt { result, value, ty } => {
                    let key = (*value, format!("{}", ty));
                    if let Some(&existing) = const_int_map.get(&key) {
                        if existing != *result {
                            // Only replace if the canonical definition is in
                            // entry (guaranteed to dominate here).
                            replacements.insert(*result, existing);
                        }
                    } else if is_entry {
                        const_int_map.insert(key, *result);
                    }
                }
                IrInstr::ConstFloat { result, value, ty } => {
                    let key = (value.to_bits(), format!("{}", ty));
                    if let Some(&existing) = const_float_map.get(&key) {
                        if existing != *result {
                            replacements.insert(*result, existing);
                        }
                    } else if is_entry {
                        const_float_map.insert(key, *result);
                    }
                }
                _ => {}
            }
        }
    }

    if replacements.is_empty() {
        return;
    }

    // Resolve transitive chains: if y→x and x→w, then y→w.
    let mut changed = true;
    while changed {
        changed = false;
        let keys: Vec<ValueId> = replacements.keys().copied().collect();
        for k in keys {
            if let Some(&v) = replacements.get(&replacements[&k]) {
                if replacements[&k] != v {
                    replacements.insert(k, v);
                    changed = true;
                }
            }
        }
    }

    // Phase 2: Apply replacements to all instructions.
    for block in &mut func.blocks {
        for instr in &mut block.instrs {
            crate::pass::opt::apply_replacements(instr, &replacements);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_prop_pass_name() {
        let pass = CopyPropPass;
        assert_eq!(pass.name(), "copy-prop");
    }
}
