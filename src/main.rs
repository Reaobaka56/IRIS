use std::path::PathBuf;
use std::process;

use iris::cli::{parse_args, ParseArgsResult};
use iris::diagnostics::{render_error, render_error_colored, render_error_colored_with_file};

/// Returns `true` if stderr is connected to a terminal (for colored output).
fn is_stderr_tty() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        let handle = std::io::stderr().as_raw_handle();
        // Enable virtual terminal processing on Windows 10+
        unsafe {
            let mut mode: u32 = 0;
            if winapi_GetConsoleMode(handle, &mut mode) != 0 {
                let _ = winapi_SetConsoleMode(handle, mode | 0x0004);
                return true;
            }
        }
        false
    }
    #[cfg(not(windows))]
    {
        unsafe { libc_isatty(2) != 0 }
    }
}

#[cfg(windows)]
extern "system" {
    fn GetConsoleMode(handle: *mut std::ffi::c_void, mode: *mut u32) -> i32;
    fn SetConsoleMode(handle: *mut std::ffi::c_void, mode: u32) -> i32;
}

#[cfg(windows)]
use GetConsoleMode as winapi_GetConsoleMode;
#[cfg(windows)]
use SetConsoleMode as winapi_SetConsoleMode;

#[cfg(not(windows))]
extern "C" {
    fn isatty(fd: i32) -> i32;
}
#[cfg(not(windows))]
use isatty as libc_isatty;

/// 64 MB stack — Windows default is only 1 MB, which overflows on deeply
/// nested IRIS expressions during recursive IR lowering.
const STACK_SIZE: usize = 64 * 1024 * 1024;

fn main() {
    let builder = std::thread::Builder::new().stack_size(STACK_SIZE);
    let handler = builder
        .spawn(run)
        .expect("failed to spawn main thread with enlarged stack");
    if let Err(e) = handler.join() {
        eprintln!("error: {:?}", e);
        process::exit(1);
    }
}

fn run() {
    let args: Vec<String> = std::env::args().collect();

    match parse_args(&args) {
        Ok(ParseArgsResult::Help) => {
            print!("{}", iris::cli::help_text());
            process::exit(0);
        }
        Ok(ParseArgsResult::Version) => {
            print!("{}", iris::cli::version_text());
            process::exit(0);
        }
        Ok(ParseArgsResult::Repl) => {
            run_repl();
        }
        Ok(ParseArgsResult::Lsp) => {
            if let Err(e) = iris::lsp::run_lsp_server() {
                eprintln!("LSP server error: {}", e);
                process::exit(1);
            }
        }
        Ok(ParseArgsResult::Dap) => {
            if let Err(e) = iris::dap::run_dap_server() {
                eprintln!("DAP server error: {}", e);
                process::exit(1);
            }
        }
        Ok(ParseArgsResult::Pkg) => {
            if let Err(e) = iris::pkg::run_pkg_command(&args) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
        Ok(ParseArgsResult::Bench) => {
            if let Err(e) = iris::bench::run_bench_command(&args) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
        Ok(ParseArgsResult::Test) => {
            if let Err(e) = iris::test_runner::run_test_command(&args) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
        Ok(ParseArgsResult::Profile) => {
            if let Err(e) = iris::profiler::run_profile_command(&args) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
        Ok(ParseArgsResult::Args(cli)) => {
            if cli.emit == iris::EmitKind::Binary {
                let source = std::fs::read_to_string(&cli.path).unwrap_or_default();
                let module = match iris::compile_file_to_module_with_opts(
                    &cli.path,
                    cli.dump_ir_after.as_deref(),
                ) {
                    Ok(m) => m,
                    Err(e) => {
                        if is_stderr_tty() {
                            eprint!("{}", render_error_colored_with_file(&source, &e, &cli.path.display().to_string()));
                        } else {
                            eprint!("{}", render_error(&source, &e));
                        }
                        process::exit(1);
                    }
                };
                let output_path = cli.output.unwrap_or_else(|| {
                    let stem = cli
                        .path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("iris_out");
                    PathBuf::from(format!("{}{}", stem, std::env::consts::EXE_SUFFIX))
                });
                match iris::codegen::build_binary(&module, &output_path) {
                    Ok(path) => {
                        eprintln!("wrote binary: {}", path.display());
                        if cli.run_after_build {
                            // Canonicalize so Command finds the binary in the
                            // current directory on Windows (relative paths
                            // without ".\" are not searched).
                            let run_path =
                                std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
                            let status = std::process::Command::new(&run_path)
                                .status()
                                .unwrap_or_else(|e| {
                                    eprintln!("error: could not run binary: {}", e);
                                    process::exit(1);
                                });
                            process::exit(status.code().unwrap_or(1));
                        }
                    }
                    Err(e) => {
                        eprintln!("error: {}", e);
                        process::exit(1);
                    }
                }
                return;
            }

            let source = std::fs::read_to_string(&cli.path).unwrap_or_default();
            match iris::compile_file_with_full_opts(
                &cli.path,
                cli.emit,
                cli.max_steps,
                cli.max_depth,
                cli.dump_ir_after.as_deref(),
            ) {
                Ok(output) => {
                    if let Some(out_path) = cli.output {
                        if let Err(e) = std::fs::write(&out_path, &output) {
                            eprintln!("error: cannot write '{}': {}", out_path.display(), e);
                            process::exit(1);
                        }
                    } else {
                        print!("{}", output);
                    }
                }
                Err(e) => {
                    if is_stderr_tty() {
                        eprint!("{}", render_error_colored_with_file(&source, &e, &cli.path.display().to_string()));
                    } else {
                        eprint!("{}", render_error(&source, &e));
                    }
                    process::exit(1);
                }
            }
        }
        Err(msg) => {
            eprintln!("error: {}", msg);
            eprintln!("{}", iris::cli::help_text());
            process::exit(1);
        }
    }
}

fn run_repl() {
    use std::io::{BufRead, Write};
    let mut repl = iris::ReplState::new();
    let version = env!("CARGO_PKG_VERSION");
    eprintln!("\x1b[1;36mIRIS {}\x1b[0m REPL  (type \x1b[1m:help\x1b[0m for commands, \x1b[1m:quit\x1b[0m to exit)", version);
    eprintln!();
    let stdin = std::io::stdin();
    let mut accumulator = String::new();
    let mut brace_depth: i32 = 0;

    loop {
        // Show continuation prompt when inside a multi-line block.
        if brace_depth > 0 {
            eprint!("\x1b[90m...\x1b[0m ");
        } else {
            eprint!("\x1b[1;32m>>\x1b[0m ");
        }
        let _ = std::io::stderr().flush();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => {
                // EOF (Ctrl+D) — flush any pending accumulator then exit.
                if !accumulator.trim().is_empty() {
                    run_repl_input(&mut repl, accumulator.trim());
                }
                eprintln!();
                break;
            }
            Ok(_) => {}
        }

        // Track brace depth for multiline input.
        for ch in line.chars() {
            if ch == '{' {
                brace_depth += 1;
            }
            if ch == '}' {
                brace_depth -= 1;
            }
        }
        accumulator.push_str(&line);

        // Only evaluate when braces are balanced.
        if brace_depth <= 0 {
            brace_depth = 0;
            let input = accumulator.trim().to_owned();
            accumulator.clear();
            if !input.is_empty() {
                run_repl_input(&mut repl, &input);
            }
        }
    }
}

fn run_repl_input(repl: &mut iris::ReplState, input: &str) {
    match repl.eval(input) {
        Ok(s) if !s.is_empty() => {
            println!("{}", s);
            // Show timing for expressions (not for meta-commands which start with :).
            if !input.trim_start().starts_with(':') {
                if let Some(d) = repl.last_elapsed() {
                    eprintln!("\x1b[90m  ({:.3}ms)\x1b[0m", d.as_secs_f64() * 1000.0);
                }
            }
        }
        Ok(_) => {}
        Err(e) => {
            // Use the rich diagnostic renderer when possible.
            // In the REPL the "source" is the input line itself.
            let rendered = render_error_colored(input, &e);
            if rendered.trim().is_empty() {
                eprintln!("\x1b[1;31merror\x1b[0m: {}", e);
            } else {
                eprint!("{}", rendered);
            }
        }
    }
}
