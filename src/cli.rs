//! CLI argument parsing, exported from the library so integration tests can exercise it.

use std::path::PathBuf;

use crate::EmitKind;

/// Fully-parsed CLI arguments for a compilation request.
#[derive(Debug)]
pub struct CliArgs {
    pub path: PathBuf,
    pub emit: EmitKind,
    /// Write output to this file instead of stdout.
    pub output: Option<PathBuf>,
    /// If true, after building a binary run it (used with `iris run`).
    pub run_after_build: bool,
    /// Dump IR to stderr immediately after this pass completes.
    pub dump_ir_after: Option<String>,
    /// Maximum interpreter step count before aborting (default: 1 000 000).
    pub max_steps: usize,
    /// Maximum interpreter call depth before aborting (default: 500).
    pub max_depth: usize,
}

/// Result of `parse_args`.
#[derive(Debug)]
pub enum ParseArgsResult {
    /// Normal compilation/evaluation request.
    Args(CliArgs),
    /// `--help` was present; caller should print usage and exit 0.
    Help,
    /// `--version` was present; caller should print version and exit 0.
    Version,
    /// `repl` subcommand: start the interactive REPL.
    Repl,
    /// `lsp` subcommand: start the LSP server over stdin/stdout.
    Lsp,
    /// `dap` subcommand: start the DAP debug adapter over stdin/stdout.
    Dap,
    /// `pkg` subcommand: run the package manager.
    Pkg,
}

/// Parses command-line arguments (the full `std::env::args()` slice including `argv[0]`).
pub fn parse_args(args: &[String]) -> Result<ParseArgsResult, String> {
    let mut emit = EmitKind::Ir;
    let mut path: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut run_after_build = false;
    let mut dump_ir_after: Option<String> = None;
    let mut max_steps: usize = 1_000_000;
    let mut max_depth: usize = 500;
    let mut i = 1usize;

    if let Some(first) = args.get(i) {
        match first.as_str() {
            "build" => {
                emit = EmitKind::Binary;
                i += 1;
            }
            "run" => {
                emit = EmitKind::Binary;
                run_after_build = true;
                i += 1;
            }
            "repl" => return Ok(ParseArgsResult::Repl),
            "lsp"  => return Ok(ParseArgsResult::Lsp),
            "dap"  => return Ok(ParseArgsResult::Dap),
            "pkg"  => return Ok(ParseArgsResult::Pkg),
            _ => {}
        }
    }

    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => return Ok(ParseArgsResult::Help),
            "--version" | "-V" => return Ok(ParseArgsResult::Version),
            "--emit" => {
                i += 1;
                let kind = args
                    .get(i)
                    .ok_or_else(|| "--emit requires an argument".to_owned())?;
                emit = match kind.as_str() {
                    "ir" => EmitKind::Ir,
                    "llvm" => EmitKind::Llvm,
                    "llvm-complete" => EmitKind::LlvmComplete,
                    "cuda" => EmitKind::Cuda,
                    "simd" => EmitKind::Simd,
                    "jit" => EmitKind::Jit,
                    "pgo-instrument" => EmitKind::PgoInstrument,
                    "pgo-optimize" => EmitKind::PgoOptimize,
                    "graph" => EmitKind::Graph,
                    "onnx" => EmitKind::Onnx,
                    "onnx-binary" => EmitKind::OnnxBinary,
                    "eval" => EmitKind::Eval,
                    "binary" => EmitKind::Binary,
                    other => {
                        return Err(format!(
                            "unknown emit kind: '{}' (valid: ir, llvm, llvm-complete, cuda, simd, jit, pgo-instrument, pgo-optimize, graph, onnx, onnx-binary, eval, binary)",
                            other
                        ))
                    }
                };
            }
            "-o" => {
                i += 1;
                let file = args
                    .get(i)
                    .ok_or_else(|| "-o requires an argument".to_owned())?;
                output = Some(PathBuf::from(file));
            }
            "--dump-ir-after" => {
                i += 1;
                let name = args
                    .get(i)
                    .ok_or_else(|| "--dump-ir-after requires an argument".to_owned())?;
                dump_ir_after = Some(name.clone());
            }
            "--max-steps" => {
                i += 1;
                let n = args
                    .get(i)
                    .ok_or_else(|| "--max-steps requires an argument".to_owned())?;
                max_steps = n.parse::<usize>().map_err(|_| {
                    format!("--max-steps: '{}' is not a valid positive integer", n)
                })?;
            }
            "--max-depth" => {
                i += 1;
                let n = args
                    .get(i)
                    .ok_or_else(|| "--max-depth requires an argument".to_owned())?;
                max_depth = n.parse::<usize>().map_err(|_| {
                    format!("--max-depth: '{}' is not a valid positive integer", n)
                })?;
            }
            arg if !arg.starts_with('-') => {
                path = Some(PathBuf::from(arg));
            }
            other => return Err(format!("unknown argument: '{}'", other)),
        }
        i += 1;
    }

    let path = path.ok_or_else(|| "no input file specified".to_owned())?;
    Ok(ParseArgsResult::Args(CliArgs { path, emit, output, run_after_build, dump_ir_after, max_steps, max_depth }))
}

/// Returns the version string for the CLI (GCC-style verbose output).
pub fn version_text() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let build_date = option_env!("IRIS_BUILD_DATE").unwrap_or("unknown");
    let target = option_env!("IRIS_TARGET").unwrap_or("unknown");
    let host = option_env!("IRIS_HOST").unwrap_or("unknown");
    let profile = option_env!("IRIS_PROFILE").unwrap_or("unknown");
    let opt_level = option_env!("IRIS_OPT_LEVEL").unwrap_or("unknown");
    let git_hash = option_env!("IRIS_GIT_HASH").unwrap_or("unknown");
    let git_hash_short = option_env!("IRIS_GIT_HASH_SHORT").unwrap_or("unknown");
    let git_branch = option_env!("IRIS_GIT_BRANCH").unwrap_or("unknown");
    let git_dirty = option_env!("IRIS_GIT_DIRTY").unwrap_or("false");
    let rustc_ver = option_env!("IRIS_RUSTC_VERSION").unwrap_or("unknown");

    // Detect thread model.
    let thread_model = if cfg!(target_family = "windows") {
        "win32"
    } else {
        "posix"
    };

    let dirty_flag = if git_dirty == "true" { " (modified)" } else { "" };

    format!(
        "iris {version} ({git_hash_short} {build_date}){dirty}\n\
         IRIS — Intermediate Representation for Intelligent Systems\n\
         Copyright (C) 2024-2026 Moon & IRIS Project Contributors\n\
         License: GPL-2.0-or-later <https://www.gnu.org/licenses/old-licenses/gpl-2.0.html>\n\
         This is free software; you can redistribute it and/or modify it under\n\
         the terms of the GNU General Public License v2 (or later).\n\
         There is NO WARRANTY, to the extent permitted by law.\n\
         \n\
         Compiler:\n\
           Version:       {version}\n\
           Git commit:    {git_hash}\n\
           Git branch:    {git_branch}\n\
           Build date:    {build_date}\n\
         \n\
         Platform:\n\
           Target:        {target}\n\
           Host:          {host}\n\
           Thread model:  {thread_model}\n\
         \n\
         Build:\n\
           Profile:       {profile}\n\
           Opt level:     {opt_level}\n\
           Rust edition:  2021\n\
           Built with:    {rustc_ver}\n",
        version = version,
        git_hash_short = git_hash_short,
        git_hash = git_hash,
        git_branch = git_branch,
        build_date = build_date,
        dirty = dirty_flag,
        target = target,
        host = host,
        profile = profile,
        opt_level = opt_level,
        thread_model = thread_model,
        rustc_ver = rustc_ver,
    )
}

/// Returns the usage/help text for the CLI.
pub fn help_text() -> &'static str {
    "IRIS compiler\n\
     Usage: iris [subcommand] [options] <file.iris>\n\
     \n\
     Subcommands:\n\
       build                 Build native binary (same as --emit binary)\n\
       run                   Build and run the binary\n\
       repl                  Start an interactive REPL session\n\
       lsp                   Start the LSP server (JSON-RPC on stdin/stdout)\n\
       dap                   Start the DAP debug adapter (JSON-RPC on stdin/stdout)\n\
       pkg <cmd>             Package manager (init, add, remove, install, list, build, run)\n\
     \n\
     Options:\n\
       --emit <kind>         Output kind: ir (default), llvm, llvm-complete, cuda, simd,\n\
                             jit, pgo-instrument, pgo-optimize, graph, onnx, onnx-binary,\n\
                             eval, binary\n\
       -o <file>             Write output to <file> instead of stdout\n\
       --dump-ir-after <p>   Dump IR to stderr after pass <p> completes\n\
       --max-steps <n>       Max interpreter steps before abort (default: 1000000)\n\
       --max-depth <n>       Max call depth before abort (default: 500)\n\
       --help, -h            Print this help and exit\n\
       --version, -V         Print version and exit\n"
}
