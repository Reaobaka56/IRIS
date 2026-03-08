//! Fuzz target: Parser
//!
//! Tokenizes arbitrary input, then feeds it to the parser in error-recovery
//! mode. The parser must never panic — partial ASTs with error lists are fine.
//!
//! Run with `cargo fuzz` (nightly).  On stable CI, `fn main` smoke-tests the
//! corpus directory instead.

#![cfg_attr(fuzzing, no_main)]

#[cfg(fuzzing)]
use libfuzzer_sys::fuzz_target;

fn run(data: &[u8]) {
    if let Ok(src) = std::str::from_utf8(data) {
        let mut lexer = iris::parser::lexer::Lexer::new(src);
        if let Ok(tokens) = lexer.tokenize() {
            let mut parser = iris::parser::parse::Parser::new(&tokens);
            // parse_module_recovering must never panic
            let (_module, _errors) = parser.parse_module_recovering();
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
    println!("fuzz_parser: smoke-tested {} corpus files", count);
}
