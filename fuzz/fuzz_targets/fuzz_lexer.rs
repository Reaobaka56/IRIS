//! Fuzz target: Lexer
//!
//! Feeds arbitrary byte strings to the IRIS lexer. The lexer must never
//! panic — it should return `Ok(tokens)` or `Err(ParseError)`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Only process valid UTF-8 — the lexer operates on &str
    if let Ok(src) = std::str::from_utf8(data) {
        let mut lexer = iris::parser::lexer::Lexer::new(src);
        // Must not panic — Ok or Err are both acceptable
        let _ = lexer.tokenize();
    }
});
