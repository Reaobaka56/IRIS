//! Fuzz target: Lowerer
//!
//! Parses valid-ish IRIS source and feeds it directly to the lowerer.
//! The lowerer must return `Ok` or `Err(LowerError)` — never panic.
//!
//! Run with `cargo fuzz` (nightly).  On stable CI, `fn main` smoke-tests the
//! corpus directory instead.

#![cfg_attr(fuzzing, no_main)]

#[cfg(fuzzing)]
use libfuzzer_sys::fuzz_target;

fn run(data: &[u8]) {
    if let Ok(src) = std::str::from_utf8(data) {
        let (ast, errors) = iris::compile_with_recovery(src);
        // Only attempt lowering if parsing produced at least one function
        // (pure garbage won't exercise the lowerer meaningfully)
        if errors.len() < 10 && !ast.functions.is_empty() {
            let _ = iris::lower::lower(&ast, "fuzz");
        }
    }
}

#[cfg(fuzzing)]
fuzz_target!(|data: &[u8]| {
    run(data);
});

#[cfg(not(fuzzing))]
fn main() {
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus");
    let mut count = 0usize;
    if let Ok(entries) = std::fs::read_dir(&corpus) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("iris") {
                if let Ok(data) = std::fs::read(&path) {
                    run(&data);
                    count += 1;
                }
            }
        }
    }
    println!("fuzz_lowerer: smoke-tested {} corpus files", count);
}
