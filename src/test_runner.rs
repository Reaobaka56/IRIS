//! `iris test` — test runner for IRIS programs.
//!
//! Discovers all zero-argument functions whose names begin with `test_`
//! in a given `.iris` file (or every `*.iris` file in the current directory),
//! runs them through the interpreter, and reports PASS / FAIL / PANIC with
//! timing and a summary line.
//!
//! Exit code: 0 if all tests pass, 1 if any fail or panic, 2 for I/O errors.
//!
//! Usage:
//!   iris test [file.iris] [--filter <substr>] [--no-color]

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::error::InterpError;
use crate::interp::{self, IrValue};

// ── ANSI helpers ──────────────────────────────────────────────────────────────

const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[1;32m";
const RED: &str = "\x1b[1;31m";
const YELLOW: &str = "\x1b[1;33m";
const CYAN: &str = "\x1b[1;36m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

fn strip(s: &str) -> String {
    // Remove ANSI escapes for --no-color mode.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // skip until 'm'
            for ch in chars.by_ref() {
                if ch == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ── Outcome ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
enum Outcome {
    Pass { elapsed_ms: f64 },
    Fail { reason: String, elapsed_ms: f64 },
    Panic { msg: String, elapsed_ms: f64 },
    #[allow(dead_code)]
    Ignored,
}

// ── Run a single test function ────────────────────────────────────────────────

fn run_one(module: &crate::ir::module::IrModule, fn_name: &str) -> Outcome {
    let func = match module.functions().iter().find(|f| f.name == fn_name) {
        Some(f) => f,
        None => {
            return Outcome::Fail {
                reason: "function not found in compiled module".into(),
                elapsed_ms: 0.0,
            }
        }
    };

    let opts = interp::InterpOptions {
        max_steps: 10_000_000,
        max_depth: 1_000,
    };

    let t0 = Instant::now();
    let result = interp::eval_function_in_module_opts(module, func, &[], opts);
    let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;

    match result {
        Ok(vals) => {
            // A test passes if it returns `true` (Bool) or `0` (i64 sentinel).
            let passed = vals.iter().all(|v| match v {
                IrValue::Bool(b) => *b,
                IrValue::I64(0) => true,
                IrValue::I32(0) => true,
                IrValue::Unit => true,
                _ => false,
            });
            if passed {
                Outcome::Pass { elapsed_ms }
            } else {
                let got: Vec<String> = vals.iter().map(|v| format!("{}", v)).collect();
                Outcome::Fail {
                    reason: format!("returned {}", got.join(", ")),
                    elapsed_ms,
                }
            }
        }
        Err(InterpError::Panic { msg }) => Outcome::Panic { msg, elapsed_ms },
        Err(e) => Outcome::Fail {
            reason: format!("{}", e),
            elapsed_ms,
        },
    }
}

// ── Test a single file ────────────────────────────────────────────────────────

struct FileResult {
    #[allow(dead_code)]
    path: PathBuf,
    results: Vec<(String, Outcome)>,
}

fn test_file(path: &Path, filter: Option<&str>) -> Result<FileResult, String> {
    // Compile with bring resolution.
    let module = crate::compile_file_to_module(path)
        .map_err(|e| format!("{}: {}", path.display(), e))?;

    // Collect test functions: zero-arg, name starts with "test_".
    let test_fns: Vec<String> = module
        .functions()
        .iter()
        .filter(|f| f.name.starts_with("test_") && f.params.is_empty())
        .filter(|f| filter.map(|s| f.name.contains(s)).unwrap_or(true))
        .map(|f| f.name.clone())
        .collect();

    let mut results = Vec::new();
    for name in test_fns {
        let outcome = run_one(&module, &name);
        results.push((name, outcome));
    }

    Ok(FileResult {
        path: path.to_path_buf(),
        results,
    })
}

// ── Main entry point ──────────────────────────────────────────────────────────

/// Entry point for `iris test [file.iris] [--filter <s>] [--no-color]`.
pub fn run_test_command(args: &[String]) -> Result<(), String> {
    // Parse sub-arguments.
    let mut paths: Vec<PathBuf> = vec![];
    let mut filter: Option<String> = None;
    let mut color = true;
    let mut i = 2usize; // skip "iris" "test"
    while i < args.len() {
        match args[i].as_str() {
            "--filter" | "-f" => {
                i += 1;
                filter = Some(
                    args.get(i)
                        .ok_or("--filter requires an argument")?
                        .clone(),
                );
            }
            "--no-color" => color = false,
            arg if !arg.starts_with('-') => paths.push(PathBuf::from(arg)),
            other => return Err(format!("unknown test option: '{}'", other)),
        }
        i += 1;
    }

    // If no files given, discover *.iris in current directory.
    if paths.is_empty() {
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(&cwd).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("iris") {
                paths.push(p);
            }
        }
        paths.sort();
    }

    if paths.is_empty() {
        eprintln!("no .iris files found");
        return Err("no .iris files found".into());
    }

    let c = |s: &str| -> String {
        if color { s.to_owned() } else { strip(s) }
    };

    let mut total_pass = 0usize;
    let mut total_fail = 0usize;
    let mut total_panic = 0usize;
    let mut total_ignored = 0usize;

    for path in &paths {
        // Print file header.
        eprintln!(
            "\n{}running tests in {}{}{}\n",
            c(CYAN),
            c(BOLD),
            path.display(),
            c(RESET)
        );

        let file_result = match test_file(path, filter.as_deref()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}error:{} {}", c(RED), c(RESET), e);
                total_fail += 1;
                continue;
            }
        };

        if file_result.results.is_empty() {
            eprintln!("{}  (no test_ functions found){}", c(DIM), c(RESET));
            continue;
        }

        for (name, outcome) in &file_result.results {
            let (status, detail, ms) = match outcome {
                Outcome::Pass { elapsed_ms } => (
                    format!("{}PASS{}", c(GREEN), c(RESET)),
                    String::new(),
                    *elapsed_ms,
                ),
                Outcome::Fail { reason, elapsed_ms } => (
                    format!("{}FAIL{}", c(RED), c(RESET)),
                    format!(" — {}", reason),
                    *elapsed_ms,
                ),
                Outcome::Panic { msg, elapsed_ms } => (
                    format!("{}PANIC{}", c(RED), c(RESET)),
                    format!(" — {}", msg),
                    *elapsed_ms,
                ),
                Outcome::Ignored => (
                    format!("{}skip{}", c(YELLOW), c(RESET)),
                    String::new(),
                    0.0,
                ),
            };
            eprintln!(
                "  {}test {}{} ... {} {}({:.2}ms){}{}",
                c(DIM),
                c(RESET),
                name,
                status,
                c(DIM),
                ms,
                c(RESET),
                detail,
            );
            match outcome {
                Outcome::Pass { .. } => total_pass += 1,
                Outcome::Fail { .. } => total_fail += 1,
                Outcome::Panic { .. } => total_panic += 1,
                Outcome::Ignored => total_ignored += 1,
            }
        }
    }

    // Summary line.
    let total = total_pass + total_fail + total_panic + total_ignored;
    let failed = total_fail + total_panic;
    eprintln!();
    if failed == 0 {
        eprintln!(
            "{}test result: ok.{} {} passed; {} failed; {} ignored",
            c(GREEN),
            c(RESET),
            total_pass,
            0,
            total_ignored
        );
        Ok(())
    } else {
        eprintln!(
            "{}test result: FAILED.{} {} passed; {} failed; {} panicked; {} ignored",
            c(RED),
            c(RESET),
            total_pass,
            total_fail,
            total_panic,
            total_ignored
        );
        let _ = total; // suppress unused
        Err(format!(
            "{} test(s) failed",
            failed
        ))
    }
}
