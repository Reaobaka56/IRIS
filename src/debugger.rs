//! Trace-based debugger for IRIS programs.
//!
//! [`DebugSession`] compiles and executes a program, recording a trace of
//! executed instructions with their source positions and in-scope variable
//! snapshots. The trace can then be replayed step-by-step or advanced to the
//! next breakpoint, providing offline (post-mortem) debugging without requiring
//! coroutines or unsafe threading.
//!
//! ## Features
//!
//! - Source breakpoints with optional **hit-count** and **condition** expressions
//! - **Log-point** messages (evaluated and printed without stopping)
//! - Step forward / backward (time-travel)
//! - Step-over, step-into, step-out with function-level granularity
//! - Watch-expression evaluation against the current frame
//! - Variable mutation via `set_variable`
//! - Debug-console completions drawn from in-scope variables

use std::collections::HashMap;

use crate::error::Error;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single recorded execution step with source position and variable state.
#[derive(Debug, Clone)]
pub struct TraceEntry {
    /// Name of the function being executed.
    pub func_name: String,
    /// 1-based source line number.
    pub line: u32,
    /// 1-based source column number.
    pub column: u32,
    /// Snapshot of named variables at this point: (display_name, display_value).
    pub variables: Vec<(String, String)>,
    /// Call depth of this frame (0 = top-level / main).
    pub depth: u32,
}

/// Metadata attached to a breakpoint.
#[derive(Debug, Clone, Default)]
pub struct BreakpointInfo {
    /// Optional condition expression — breakpoint only fires when this evaluates to `true`.
    pub condition: Option<String>,
    /// Optional hit-count threshold (e.g. `">5"`). The breakpoint fires when
    /// the hit count satisfies the expression.
    pub hit_condition: Option<String>,
    /// Optional log message — if set, the breakpoint becomes a log-point:
    /// the message is emitted to the debug console and execution continues.
    pub log_message: Option<String>,
    /// Number of times execution has reached this breakpoint.
    pub hit_count: u64,
}

/// A debug session for a single IRIS source file.
///
/// # Usage
/// ```text
/// let mut session = DebugSession::new();
/// session.set_source(src);
/// session.set_breakpoint(3, None);
/// session.start().unwrap();
/// if let Some(frame) = session.continue_to_breakpoint() {
///     println!("stopped at line {}", frame.line);
/// }
/// ```
pub struct DebugSession {
    source: String,
    breakpoints: HashMap<u32, BreakpointInfo>,  // line -> info
    trace: Vec<TraceEntry>,
    cursor: usize,
    /// Log messages emitted by log-points during the last `continue` / `step`.
    pub pending_logs: Vec<String>,
    /// Whether to break on panics / runtime errors.
    pub break_on_exception: bool,
    /// Cached exception message from the last `start()` if the program panicked.
    pub exception_message: Option<String>,
}

impl Default for DebugSession {
    fn default() -> Self { Self::new() }
}

impl DebugSession {
    /// Creates an empty debug session.
    pub fn new() -> Self {
        Self {
            source: String::new(),
            breakpoints: HashMap::new(),
            trace: Vec::new(),
            cursor: 0,
            pending_logs: Vec::new(),
            break_on_exception: true,
            exception_message: None,
        }
    }

    /// Sets the IRIS source code to debug.
    pub fn set_source(&mut self, src: &str) {
        self.source = src.to_owned();
        self.trace.clear();
        self.cursor = 0;
        self.exception_message = None;
    }

    /// Registers a breakpoint at `line` (1-based) with optional metadata.
    pub fn set_breakpoint(&mut self, line: u32, info: Option<BreakpointInfo>) {
        self.breakpoints.insert(line, info.unwrap_or_default());
    }

    /// Removes a breakpoint.
    pub fn remove_breakpoint(&mut self, line: u32) {
        self.breakpoints.remove(&line);
    }

    /// Removes all breakpoints.
    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    /// Compiles the source and runs it, collecting a full execution trace.
    ///
    /// After this call, use `step()` / `continue_to_breakpoint()` to walk the trace.
    pub fn start(&mut self) -> Result<(), Error> {
        self.trace.clear();
        self.cursor = 0;
        self.exception_message = None;

        // Compile to module (runs all passes).
        let module = crate::compile_to_module(&self.source, "debug")?;

        // Collect the trace via the interpreter.
        let trace = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        {
            let t = std::rc::Rc::clone(&trace);
            crate::interp::collect_trace(&module, &self.source, t)?;
        }
        self.trace = std::rc::Rc::try_unwrap(trace)
            .map_err(|_| ())
            .unwrap_or_default()
            .into_inner();

        Ok(())
    }

    /// Returns the current trace frame (at `cursor`).
    pub fn current_frame(&self) -> Option<&TraceEntry> {
        self.trace.get(self.cursor)
    }

    /// Returns the total number of trace entries.
    pub fn trace_len(&self) -> usize {
        self.trace.len()
    }

    /// Returns current cursor position.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    // ── Stepping ──────────────────────────────────────────────────────────

    /// Advances the cursor by one step. Returns `false` if already at the end.
    pub fn step(&mut self) -> bool {
        if self.cursor + 1 < self.trace.len() {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    /// Steps backwards by one step. Returns `false` if already at the beginning.
    pub fn step_back(&mut self) -> bool {
        if self.cursor > 0 {
            self.cursor -= 1;
            true
        } else {
            false
        }
    }

    /// Step over: advance to the next trace entry at the same or lower call depth,
    /// skipping entries that are deeper (i.e. inside called functions).
    pub fn step_over(&mut self) -> bool {
        if self.cursor + 1 >= self.trace.len() {
            return false;
        }
        let current_depth = self.trace[self.cursor].depth;
        self.cursor += 1;
        while self.cursor < self.trace.len() {
            if self.trace[self.cursor].depth <= current_depth {
                return true;
            }
            self.cursor += 1;
        }
        // Reached end
        self.cursor = self.trace.len().saturating_sub(1);
        true
    }

    /// Step into: simply advance one step (enters function calls).
    pub fn step_into(&mut self) -> bool {
        self.step()
    }

    /// Step out: advance until the call depth is strictly lower than the current depth
    /// (i.e. we've returned from the current function).
    pub fn step_out(&mut self) -> bool {
        if self.cursor + 1 >= self.trace.len() {
            return false;
        }
        let current_depth = self.trace[self.cursor].depth;
        self.cursor += 1;
        while self.cursor < self.trace.len() {
            if self.trace[self.cursor].depth < current_depth {
                return true;
            }
            self.cursor += 1;
        }
        self.cursor = self.trace.len().saturating_sub(1);
        true
    }

    // ── Breakpoints ───────────────────────────────────────────────────────

    /// Advances the cursor to the next frame that matches a registered breakpoint
    /// (respecting conditions, hit counts, and log-points).
    ///
    /// Returns a reference to that frame, or `None` if no breakpoint is hit.
    pub fn continue_to_breakpoint(&mut self) -> Option<&TraceEntry> {
        self.pending_logs.clear();
        self.cursor += 1;
        while self.cursor < self.trace.len() {
            let line = self.trace[self.cursor].line;
            if let Some(bp) = self.breakpoints.get_mut(&line) {
                bp.hit_count += 1;

                // Check hit-count condition (e.g. ">5", "==3", "10" meaning ==10).
                if let Some(ref hc) = bp.hit_condition {
                    if !check_hit_condition(bp.hit_count, hc) {
                        self.cursor += 1;
                        continue;
                    }
                }

                // Check expression condition.
                if let Some(ref cond) = bp.condition {
                    let vars = &self.trace[self.cursor].variables;
                    if !evaluate_condition(vars, cond) {
                        self.cursor += 1;
                        continue;
                    }
                }

                // Log-point: emit message, don't stop.
                if let Some(ref msg) = bp.log_message {
                    let vars = &self.trace[self.cursor].variables;
                    let rendered = interpolate_log_message(msg, vars);
                    self.pending_logs.push(rendered);
                    self.cursor += 1;
                    continue;
                }

                return self.trace.get(self.cursor);
            }
            self.cursor += 1;
        }
        None
    }

    // ── Variable mutation ─────────────────────────────────────────────────

    /// Sets a variable's display value in the current frame.
    /// Returns `true` if the variable was found and updated.
    pub fn set_variable(&mut self, name: &str, value: &str) -> bool {
        if self.cursor >= self.trace.len() {
            return false;
        }
        let frame = &mut self.trace[self.cursor];
        for (n, v) in &mut frame.variables {
            if n == name {
                *v = value.to_owned();
                return true;
            }
        }
        false
    }

    // ── Query helpers ─────────────────────────────────────────────────────

    /// Returns all recorded trace entries.
    pub fn all_frames(&self) -> &[TraceEntry] {
        &self.trace
    }

    /// Returns a simulated call stack from the current cursor position.
    /// Walks backwards through the trace to find distinct function frames.
    pub fn all_visible_frames(&self) -> Vec<&TraceEntry> {
        if self.cursor >= self.trace.len() {
            return Vec::new();
        }
        let current = &self.trace[self.cursor];
        let mut frames = vec![current];

        // Walk backward to find caller frames (different function names)
        let mut seen_funcs = std::collections::HashSet::new();
        seen_funcs.insert(&current.func_name);

        for i in (0..self.cursor).rev() {
            let entry = &self.trace[i];
            if !seen_funcs.contains(&entry.func_name) {
                seen_funcs.insert(&entry.func_name);
                frames.push(entry);
            }
        }
        frames
    }

    /// Returns a list of variable names in scope at the current frame.
    /// Useful for providing debug-console completions.
    pub fn completions(&self) -> Vec<String> {
        self.current_frame()
            .map(|f| f.variables.iter().map(|(n, _)| n.clone()).collect())
            .unwrap_or_default()
    }

    /// Returns `true` when the cursor is at or past the last trace entry.
    pub fn is_finished(&self) -> bool {
        self.cursor >= self.trace.len()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Checks whether `count` satisfies a hit-condition string.
/// Supported forms: `"5"` (== 5), `">5"`, `">=5"`, `"<5"`, `"<=5"`, `"==5"`, `"%3"` (every 3rd).
fn check_hit_condition(count: u64, cond: &str) -> bool {
    let cond = cond.trim();
    if cond.is_empty() { return true; }

    if let Some(rest) = cond.strip_prefix(">=") {
        rest.trim().parse::<u64>().map_or(true, |n| count >= n)
    } else if let Some(rest) = cond.strip_prefix(">") {
        rest.trim().parse::<u64>().map_or(true, |n| count > n)
    } else if let Some(rest) = cond.strip_prefix("<=") {
        rest.trim().parse::<u64>().map_or(true, |n| count <= n)
    } else if let Some(rest) = cond.strip_prefix("<") {
        rest.trim().parse::<u64>().map_or(true, |n| count < n)
    } else if let Some(rest) = cond.strip_prefix("==") {
        rest.trim().parse::<u64>().map_or(true, |n| count == n)
    } else if let Some(rest) = cond.strip_prefix('%') {
        rest.trim().parse::<u64>().map_or(true, |n| n > 0 && count % n == 0)
    } else {
        // Plain number means == n.
        cond.parse::<u64>().map_or(true, |n| count == n)
    }
}

/// Evaluates a condition expression against the current variable snapshot.
/// Returns `true` if the expression evaluates to `true` (or the evaluation fails).
fn evaluate_condition(vars: &[(String, String)], expr: &str) -> bool {
    // Try simple comparisons first: "x > 5", "name == \"foo\""
    for op in &["==", "!=", ">=", "<=", ">", "<"] {
        if let Some(idx) = expr.find(op) {
            let lhs = expr[..idx].trim();
            let rhs = expr[idx + op.len()..].trim();
            if let Some(lhs_val) = find_var(vars, lhs) {
                return compare_values(&lhs_val, op, rhs);
            }
        }
    }
    // Fall back: if the expression is a variable name that is "true", fire.
    if let Some(val) = find_var(vars, expr.trim()) {
        return val == "true";
    }
    // Can't evaluate — default to firing.
    true
}

fn find_var(vars: &[(String, String)], name: &str) -> Option<String> {
    vars.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone())
}

fn compare_values(lhs: &str, op: &str, rhs: &str) -> bool {
    // Try numeric comparison first.
    if let (Ok(l), Ok(r)) = (lhs.parse::<f64>(), rhs.parse::<f64>()) {
        return match op {
            "==" => (l - r).abs() < f64::EPSILON,
            "!=" => (l - r).abs() >= f64::EPSILON,
            ">"  => l > r,
            ">=" => l >= r,
            "<"  => l < r,
            "<=" => l <= r,
            _ => false,
        };
    }
    // String comparison (strip surrounding quotes from rhs if present).
    let rhs_clean = rhs.trim_matches('"');
    match op {
        "==" => lhs == rhs_clean,
        "!=" => lhs != rhs_clean,
        _ => false,
    }
}

/// Interpolates `{varname}` placeholders in a log-point message.
fn interpolate_log_message(msg: &str, vars: &[(String, String)]) -> String {
    let mut result = String::new();
    let mut chars = msg.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            for ch in chars.by_ref() {
                if ch == '}' { break; }
                name.push(ch);
            }
            if let Some(val) = find_var(vars, name.trim()) {
                result.push_str(&val);
            } else {
                result.push('{');
                result.push_str(&name);
                result.push('}');
            }
        } else {
            result.push(c);
        }
    }
    result
}
