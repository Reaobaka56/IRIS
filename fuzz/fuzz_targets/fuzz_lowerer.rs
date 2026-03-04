//! Fuzz target: Lowerer
//!
//! Parses valid-ish IRIS source and feeds it directly to the lowerer.
//! The lowerer must return `Ok` or `Err(LowerError)` — never panic.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(src) = std::str::from_utf8(data) {
        let (ast, errors) = iris::compile_with_recovery(src);
        // Only attempt lowering if parsing produced at least one function
        // (pure garbage won't exercise the lowerer meaningfully)
        if errors.len() < 10 && !ast.functions.is_empty() {
            let _ = iris::lower::lower(&ast, "fuzz");
        }
    }
});
