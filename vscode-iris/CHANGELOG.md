# IRIS Language Extension Changelog

## 0.3.0

### New Features

- **Native binary compilation** — `iris build file.iris -o output` compiles to a standalone executable via LLVM/clang; no external toolchain required when installed via the Windows installer
- **Closure support** — first-class closures with capture, higher-order functions, `list_map`, `list_filter`, `list_reduce` all invoke closures correctly
- **Merge sort and binary search** — standard library additions
- **Expanded fuzz corpus** — arithmetic, closures, concurrency, data structures, edge cases, lists, maps, methods, ML ops, pattern matching, strings, traits
- **Benchmark suite** — `benches/` directory with binary search, Collatz, hashmap, numerical, sieve, and tree benchmarks
- **Security hardening** — input validation and bounds checks in runtime
- **Profiler** — built-in performance profiling (`iris run --profile file.iris`)
- **Expanded stdlib** — ML/NN helpers (`src/stdlib/ml.iris`, `src/stdlib/nn.iris`)
- **Windows installer** — full self-contained installer bundles LLVM/clang + MinGW sysroot + VC++ runtime; installs to PATH automatically

### Improvements

- 1,384 tests passing (249 unit + 1,135 integration)
- Strength reduction and copy propagation passes
- Loop-invariant code motion (LICM) pass
- Improved LLVM IR emission: cleaner phi nodes, no double terminators
- Better diagnostics with span information
- LSP: inlay hints, code actions, and diagnostics improvements

### Bug Fixes

- Fixed phi predecessor handling after `Panic` blocks in LLVM IR (no double terminators at -O2)
- Fixed `for`/`while` body tail expression being silently dropped
- Fixed `spawn { }`, `while { }`, `for { }`, `loop { }` accepting optional trailing `;`
- Fixed `atomic(v)` alias for `atomic_new(v)`
- Fixed `Option`/`Result` unboxing in LLVM codegen

## 0.2.0

### New Features

- **Status bar** now shows real version from `iris --version` with rich tooltip (version, git commit, branch, build date, target, rustc)
- **Show Version Info** command and server-menu action — displays full GCC-style compiler info in the output panel
- **LSP best-practice diagnostics**: BP001 (long function), BP002 (missing doc comment), BP003 (too many params), BP004 (non-snake_case), BP005 (empty body), BP006 (double semicolons)
- **LSP code actions / auto-fix**: missing semicolons, type-mismatch casts, add doc comment, rename to snake_case, remove redundant semicolons, wrap in if-condition
- **C / Python / Rust FFI** builtins: `ffi_call_i64`, `ffi_call_f64`, `ffi_call_str`, `ffi_call_void`, `python_eval`, `python_exec`, `python_call`, `python_version`, `rust_lib_open`, `rust_call_i64`, `rust_call_f64`, `rust_call_void`
- **60+ new builtins** (Phase 105): async/concurrency, deque, sorted collections, bitset, OS/system, crypto/UUID, string extras, math constants, functional list operations
- Binary output now named after the source file (e.g., `hello.iris` → `hello.exe`)
- Verbose `iris --version` output: git commit, branch, build date, target, host, profile, rustc version

### Improvements

- Updated syntax grammar with all Phase 104/105/106 builtins and new types
- New snippets for FFI, error handling, concurrency, and more
- LSP completions and hover docs for all new builtins
- InlayHint and code-lens improvements
- Better error diagnostics from build/run output

### Bug Fixes

- `list_map`, `list_filter`, `list_reduce` now properly invoke closures (were stubs)
- Status bar correctly reads version from the installed iris binary

## 0.1.0

- Initial release
- Syntax highlighting for .iris files
- Language Server Protocol: hover, completions, diagnostics, goto-definition, document symbols, signature help, formatting
- Debug Adapter Protocol: breakpoints, step, variables, evaluate
- Commands: Run File (Ctrl+F5), Build Binary, Open REPL
- Snippets for common patterns
