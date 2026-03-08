//! Fuzz target: Full compiler pipeline
//!
//! Runs the entire compile pipeline (lex → parse → lower → passes → codegen)
//! on arbitrary input. No stage should ever panic — errors are expected and
//! must be returned cleanly.
//!
//! Run with `cargo fuzz` (nightly).  On stable CI, `fn main` smoke-tests the
//! corpus directory instead.

#![cfg_attr(fuzzing, no_main)]

#[cfg(fuzzing)]
use libfuzzer_sys::fuzz_target;

fn run(data: &[u8]) {
    if let Ok(src) = std::str::from_utf8(data) {
        // compile_with_recovery: lex + parse (never panics)
        let (ast, _parse_errors) = iris::compile_with_recovery(src);

        // lowering: may fail with LowerError — must not panic
        let lower_result = iris::lower::lower(&ast, "fuzz");
        if let Ok(mut module) = lower_result {
            // Run the standard pass pipeline
            use iris::pass::{ConstFoldPass, CsePass, DcePass, OpExpandPass, Pass};
            use iris::pass::type_infer::TypeInferPass;
            use iris::pass::validate::ValidatePass;

            // Each pass must not panic
            let _ = ValidatePass.run(&mut module);
            let _ = TypeInferPass.run(&mut module);
            let _ = ConstFoldPass.run(&mut module);
            let _ = OpExpandPass.run(&mut module);
            let _ = DcePass.run(&mut module);
            let _ = CsePass.run(&mut module);

            // Codegen: must not panic on any IR (well-formed or malformed)
            let _ = iris::codegen::printer::emit_ir_text(&module);
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
    println!("fuzz_compiler: smoke-tested {} corpus files", count);
}
