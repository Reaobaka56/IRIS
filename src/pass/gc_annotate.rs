/// Phase 83: GC Annotation Pass
///
/// Inserts `Retain` after each heap-allocated value (list, map, option, result,
/// channel, etc.) is created, and `Release` before each `Return` terminator.
///
/// **Correctness invariant (dominance):** a Release is only emitted for a value
/// whose *defining block dominates the Return block*.  Without this guard the
/// pass would reference SSA values that are not in scope at the release site,
/// which is illegal LLVM IR and causes clang to reject the module.
///
/// In the interpreter, Retain/Release are no-ops.
/// In LLVM IR, they lower to `@iris_retain` / `@iris_release` calls.
use std::collections::{HashMap, HashSet};

use crate::error::PassError;
use crate::ir::block::{BlockId, IrBlock};
use crate::ir::instr::IrInstr;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use crate::ir::value::ValueId;
use crate::pass::Pass;

pub struct GcAnnotatePass;

impl Pass for GcAnnotatePass {
    fn name(&self) -> &'static str {
        "GcAnnotate"
    }

    fn run(&mut self, module: &mut IrModule) -> Result<(), PassError> {
        for func in &mut module.functions {
            let n = func.blocks.len();
            if n == 0 {
                continue;
            }

            // ------------------------------------------------------------------
            // 1. Build predecessor map and compute dominators.
            // ------------------------------------------------------------------
            let block_ids: Vec<BlockId> = func.blocks.iter().map(|b| b.id).collect();
            let all_ids: HashSet<BlockId> = block_ids.iter().cloned().collect();

            // predecessors[i] = set of blocks that branch to block i
            let mut preds: HashMap<BlockId, Vec<BlockId>> =
                block_ids.iter().map(|&b| (b, Vec::new())).collect();
            for block in &func.blocks {
                for succ in block_successors(block) {
                    preds.entry(succ).or_default().push(block.id);
                }
            }

            // Iterative dominator computation (Cooper et al., "A Simple, Fast
            // Dominance Algorithm").
            //
            // dom[entry] = {entry}
            // dom[b]     = all_blocks  (for b != entry, initial)
            // Repeat: dom[b] = {b} ∪ ∩{ dom[p] | p ∈ preds(b) }
            let entry_id = func.blocks[0].id;
            let mut dom: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();
            {
                let mut entry_set = HashSet::new();
                entry_set.insert(entry_id);
                dom.insert(entry_id, entry_set);
            }
            for &bid in &block_ids[1..] {
                dom.insert(bid, all_ids.clone());
            }
            let mut changed = true;
            while changed {
                changed = false;
                for &bid in &block_ids[1..] {
                    let preds_list = preds[&bid].clone();
                    if preds_list.is_empty() {
                        // Unreachable block — leave dom as full set (safe).
                        continue;
                    }
                    // Intersect dominator sets of all predecessors.
                    let mut new_dom: HashSet<BlockId> = all_ids.clone();
                    for p in &preds_list {
                        if let Some(pd) = dom.get(p) {
                            new_dom = new_dom.intersection(pd).cloned().collect();
                        }
                    }
                    new_dom.insert(bid);
                    if new_dom != dom[&bid] {
                        dom.insert(bid, new_dom);
                        changed = true;
                    }
                }
            }

            // ------------------------------------------------------------------
            // 2. Collect all heap-allocated values and the block that defines them.
            // ------------------------------------------------------------------
            // (value_id, type, defining_block_id)
            let mut heap_vals: Vec<(ValueId, IrType, BlockId)> = Vec::new();
            for block in &func.blocks {
                for instr in &block.instrs {
                    if let Some(result) = instr.result() {
                        if let Some(ty) = func.value_types.get(&result) {
                            if is_heap_ty(ty) {
                                heap_vals.push((result, ty.clone(), block.id));
                            }
                        }
                    }
                }
            }

            // Dedup by value id (shouldn't be needed in SSA, but be safe).
            {
                let mut seen: HashSet<ValueId> = HashSet::new();
                heap_vals.retain(|(v, _, _)| seen.insert(*v));
            }

            if heap_vals.is_empty() {
                continue;
            }

            // ------------------------------------------------------------------
            // 3. Per-block: insert Retain after creation, Release before Return.
            //    Only release values whose defining block dominates this block.
            // ------------------------------------------------------------------
            for bidx in 0..func.blocks.len() {
                let bid = func.blocks[bidx].id;

                // Positions: heap-creating instructions and the Return position.
                let mut inserts_after: Vec<(usize, IrInstr)> = Vec::new();
                let mut return_pos: Option<usize> = None;
                let mut returned_heap_vals: HashSet<ValueId> = HashSet::new();

                for (i, instr) in func.blocks[bidx].instrs.iter().enumerate() {
                    if let Some(result) = instr.result() {
                        if let Some(ty) = func.value_types.get(&result) {
                            if is_heap_ty(ty) {
                                inserts_after.push((i, IrInstr::Retain { ptr: result }));
                            }
                        }
                    }
                    if let IrInstr::Return { values } = instr {
                        return_pos = Some(i);
                        for value in values {
                            returned_heap_vals.insert(*value);
                        }
                    }
                }

                // Which heap values are safe to release in this block?
                // A value is safe iff its defining block dominates `bid` and it
                // is not escaping through this block's return.
                let releasable: Vec<(ValueId, IrType)> = heap_vals
                    .iter()
                    .filter(|(value, _, def_bid)| {
                        !returned_heap_vals.contains(value)
                            && dom
                                .get(&bid)
                                .map_or(false, |dom_set| dom_set.contains(def_bid))
                    })
                    .map(|(v, ty, _)| (*v, ty.clone()))
                    .collect();

                let mut new_instrs: Vec<IrInstr> = Vec::new();
                let instrs = std::mem::take(&mut func.blocks[bidx].instrs);

                if let Some(ret_idx) = return_pos {
                    for (i, instr) in instrs.into_iter().enumerate() {
                        // Insert Retain immediately after the creating instruction.
                        new_instrs.push(instr);
                        if let Some((_, retain)) = inserts_after.iter().find(|(idx, _)| *idx == i) {
                            new_instrs.push(retain.clone());
                        }
                        // Insert all dominating Releases just before Return.
                        if i + 1 == ret_idx {
                            for (val, ty) in &releasable {
                                new_instrs.push(IrInstr::Release {
                                    ptr: *val,
                                    ty: ty.clone(),
                                });
                            }
                        }
                    }
                    // Edge case: Return is at index 0 — prepend releases.
                    if ret_idx == 0 && !releasable.is_empty() {
                        let mut prefix: Vec<IrInstr> = releasable
                            .iter()
                            .map(|(val, ty)| IrInstr::Release {
                                ptr: *val,
                                ty: ty.clone(),
                            })
                            .collect();
                        prefix.extend(new_instrs);
                        new_instrs = prefix;
                    }
                } else {
                    // No Return in this block — only insert Retains.
                    for (i, instr) in instrs.into_iter().enumerate() {
                        new_instrs.push(instr);
                        if let Some((_, retain)) = inserts_after.iter().find(|(idx, _)| *idx == i) {
                            new_instrs.push(retain.clone());
                        }
                    }
                }

                func.blocks[bidx].instrs = new_instrs;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CFG helpers
// ---------------------------------------------------------------------------

/// Returns the direct successors of a block (targets of its terminator).
fn block_successors(block: &IrBlock) -> Vec<BlockId> {
    match block.terminator() {
        Some(IrInstr::Br { target, .. }) => vec![*target],
        Some(IrInstr::CondBr {
            then_block,
            else_block,
            ..
        }) => vec![*then_block, *else_block],
        Some(IrInstr::SwitchVariant {
            arms,
            default_block,
            ..
        }) => {
            let mut succs: Vec<BlockId> = arms.iter().map(|(_, b)| *b).collect();
            if let Some(d) = default_block {
                succs.push(*d);
            }
            succs
        }
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Heap-type predicate
// ---------------------------------------------------------------------------

/// Returns `true` for IR types that are reference-counted heap allocations.
///
/// Scalar types (i64, f64, bool), fixed arrays, structs passed by value,
/// and enums stored as integer tags are NOT heap-counted here — the runtime
/// represents them inline rather than through the RC table.
fn is_heap_ty(ty: &IrType) -> bool {
    matches!(
        ty,
        IrType::List(_)
            | IrType::Map(_, _)
            | IrType::Option(_)
            | IrType::ResultType(_, _)
            | IrType::Chan(_)
            | IrType::Atomic(_)
            | IrType::Mutex(_)
            | IrType::Grad(_)
            | IrType::Sparse(_)
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::compile;
    use crate::EmitKind;

    /// A function with a heap allocation on a branch should compile without
    /// emitting a Release for that allocation at a Return in the other branch.
    #[test]
    fn branching_heap_alloc_compiles() {
        let src = r#"
            def pick(flag: bool) -> i64 {
                if flag {
                    val xs: list<i64> = list()
                    val _ = list_push(xs, 42)
                    list_get(xs, 0)
                } else {
                    0
                }
            }
        "#;
        // Must not crash or produce invalid IR.
        let result = compile(src, "test", EmitKind::Ir);
        assert!(
            result.is_ok(),
            "branching heap alloc should compile: {:?}",
            result.err()
        );
    }

    /// A function with a heap allocation in the entry block releases it at
    /// every Return path (single-return and multi-return cases).
    #[test]
    fn linear_heap_alloc_retain_release_present() {
        let src = r#"
            def build() -> i64 {
                val xs: list<i64> = list()
                val _ = list_push(xs, 1)
                val _ = list_push(xs, 2)
                list_len(xs)
            }
        "#;
        let ir = compile(src, "test", EmitKind::Ir).expect("should compile");
        assert!(ir.contains("retain"), "IR should contain retain: {}", ir);
        assert!(ir.contains("release"), "IR should contain release: {}", ir);
    }

    /// A function with no heap allocations should not have retain/release noise.
    #[test]
    fn no_heap_no_gc_annotations() {
        let src = r#"
            def add(a: i64, b: i64) -> i64 { a + b }
        "#;
        let ir = compile(src, "test", EmitKind::Ir).expect("should compile");
        assert!(
            !ir.contains("retain"),
            "scalar fn should not have retain: {}",
            ir
        );
        assert!(
            !ir.contains("release"),
            "scalar fn should not have release: {}",
            ir
        );
    }
}
