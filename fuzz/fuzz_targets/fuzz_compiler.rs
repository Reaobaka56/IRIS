//! Fuzz target: Full compiler pipeline
//!
//! Runs the entire compile pipeline (lex → parse → lower → passes → codegen)
//! on arbitrary input. No stage should ever panic — errors are expected and
//! must be returned cleanly.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(src) = std::str::from_utf8(data) {
        // compile_with_recovery: lex + parse (never panics)
        let (ast, _parse_errors) = iris::compile_with_recovery(src);

        // lowering: may fail with LowerError — must not panic
        let lower_result = iris::lower::lower(&ast, "fuzz");
        if let Ok(mut module) = lower_result {
            // Run the standard pass pipeline
            use iris::pass::{
                ConstFoldPass, DcePass, CsePass, OpExpandPass, ShapeCheckPass, Pass,
            };
            use iris::pass::validate::ValidatePass;
            use iris::pass::type_infer::TypeInferPass;

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
});
