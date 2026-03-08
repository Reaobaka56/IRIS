//! Fuzz target: Lexer
//!
//! Feeds arbitrary byte strings to the IRIS lexer. The lexer must never
//! panic — it should return `Ok(tokens)` or `Err(ParseError)`.
//!
//! Run with `cargo fuzz` (nightly).  On stable CI, `cargo build` compiles this
//! as a normal binary and `fn main` smoke-tests the corpus directory instead.

#![cfg_attr(fuzzing, no_main)]

#[cfg(fuzzing)]
use libfuzzer_sys::fuzz_target;

fn run(data: &[u8]) {
    if let Ok(src) = std::str::from_utf8(data) {
        let mut lexer = iris::parser::lexer::Lexer::new(src);
        // Must not panic — Ok or Err are both acceptable
        let _ = lexer.tokenize();
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
    println!("fuzz_lexer: smoke-tested {} corpus files", count);
}
