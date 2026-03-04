//! Fuzz target: Parser
//!
//! Tokenizes arbitrary input, then feeds it to the parser in error-recovery
//! mode. The parser must never panic — partial ASTs with error lists are fine.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(src) = std::str::from_utf8(data) {
        let mut lexer = iris::parser::lexer::Lexer::new(src);
        if let Ok(tokens) = lexer.tokenize() {
            let mut parser = iris::parser::parse::Parser::new(&tokens);
            // parse_module_recovering must never panic
            let (_module, _errors) = parser.parse_module_recovering();
        }
    }
});
