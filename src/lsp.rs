//! Language Server Protocol (LSP) implementation for IRIS.
//!
//! [`LspState`] provides the core document/diagnostic API, testable without I/O.
//! [`run_lsp_server`] wraps it in a JSON-RPC Content-Length message loop on
//! stdin/stdout, compatible with any LSP client (VSCode, Neovim, etc.).

use std::collections::HashMap;

use crate::diagnostics::{byte_to_line_col, error_byte_offset};
use crate::EmitKind;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single diagnostic (error or warning) for the LSP client.
#[derive(Debug, Clone)]
pub struct LspDiagnostic {
    /// 0-based start line.
    pub line: u32,
    /// 0-based start character (UTF-16 code unit index).
    pub character: u32,
    /// 0-based end line (same as `line` for single-line errors).
    pub end_line: u32,
    /// 0-based end character.
    pub end_character: u32,
    /// Human-readable message.
    pub message: String,
    /// 1 = Error, 2 = Warning, 3 = Information, 4 = Hint.
    pub severity: u8,
    /// Optional diagnostic code (e.g. "E0001").
    pub code: Option<String>,
}

/// A code action (quick fix or refactoring) for the LSP client.
#[derive(Debug, Clone)]
pub struct CodeAction {
    pub title: String,
    pub kind: String,
    pub edit_uri: String,
    /// (start_line, start_col, end_line, end_col) — 0-based.
    pub edit_range: (u32, u32, u32, u32),
    pub new_text: String,
    pub diagnostic_message: Option<String>,
}

/// An inlay hint displayed inline in the editor.
#[derive(Debug, Clone)]
pub struct InlayHint {
    pub line: u32,
    pub character: u32,
    pub label: String,
    /// 1 = Type, 2 = Parameter.
    pub kind: u8,
}

/// Persistent LSP server state: one entry per open document.
#[derive(Default)]
pub struct LspState {
    /// URI → source text.
    documents: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Static keyword + builtin completion list
// ---------------------------------------------------------------------------

static STATIC_COMPLETIONS: &[&str] = &[
    "def", "val", "var", "for", "while", "loop", "if", "else", "when",
    "return", "break", "continue", "choice", "record", "const", "type",
    "extern", "trait", "impl", "bring", "pub", "async", "await", "spawn",
    "par", "in",
    // builtins
    "print", "panic", "assert", "len", "concat", "sin", "cos", "sqrt",
    "abs", "floor", "ceil", "exp", "log", "min", "max", "pow", "clamp",
    "some", "none", "ok", "err", "is_some", "is_ok", "unwrap",
    "list", "push", "map", "cell", "cell_get", "cell_set",
    "to_str", "format", "read_line", "read_i64", "read_f64",
    "grad", "grad_of", "zeros", "ones", "fill", "linspace",
    "split", "join", "contains", "starts_with", "ends_with",
    "to_upper", "to_lower", "trim", "repeat",
    // Phase 104 builtins
    "http_get", "http_post", "json_stringify",
    "regex_match", "regex_find_all", "regex_replace",
    "datetime_now", "datetime_timestamp", "datetime_format",
    "cwd", "list_dir", "path_join", "mkdir", "remove_file",
    "type_of", "random", "random_range",
    "hash", "base64_encode", "base64_decode",
    "char_at", "str_reverse",
    // Phase 105 builtins
    "chan_try_recv", "chan_len", "select", "timeout", "thread_count",
    "deque_new", "deque_push_front", "deque_push_back", "deque_pop_front",
    "deque_pop_back", "deque_len", "deque_front", "deque_back",
    "sorted_keys",
    "bitset_new", "bitset_set", "bitset_get", "bitset_count", "bitset_clear",
    "ffi_open", "ffi_call", "ffi_close",
    // Phase 106: expanded FFI — C, Python, Rust
    "ffi_call_i64", "ffi_call_f64", "ffi_call_str", "ffi_call_void", "ffi_call_args",
    "python_eval", "python_exec", "python_call", "python_version",
    "rust_lib_open", "rust_call_i64", "rust_call_f64", "rust_call_void",
    "env_get", "env_set", "exit_code", "exec_cmd", "pid",
    "uuid", "sha256", "hex_encode", "hex_decode",
    "str_pad_left", "str_pad_right", "str_chars", "str_bytes", "str_count",
    "math_pi", "math_e", "math_inf", "is_nan", "is_inf",
    "list_map", "list_filter", "list_reduce", "list_any", "list_all",
    "list_zip", "list_enumerate", "list_flatten", "list_unique",
    "list_reverse", "list_sorted", "list_sum", "list_min", "list_max",
    "list_index_of", "list_count", "list_take", "list_drop",
];

// ---------------------------------------------------------------------------
// LspState implementation
// ---------------------------------------------------------------------------

impl LspState {
    /// Creates an empty LSP state.
    pub fn new() -> Self { Self::default() }

    /// Called when the editor opens a document. Returns initial diagnostics.
    pub fn open_document(&mut self, uri: &str, text: &str) -> Vec<LspDiagnostic> {
        self.documents.insert(uri.to_owned(), text.to_owned());
        self.diagnose(uri)
    }

    /// Called when the editor changes a document. Returns updated diagnostics.
    pub fn update_document(&mut self, uri: &str, text: &str) -> Vec<LspDiagnostic> {
        self.documents.insert(uri.to_owned(), text.to_owned());
        self.diagnose(uri)
    }

    /// Called when the editor closes a document.
    pub fn close_document(&mut self, uri: &str) {
        self.documents.remove(uri);
    }

    /// Returns hover information (type signature) for the identifier at the given position.
    pub fn hover(&self, uri: &str, line: u32, character: u32) -> Option<String> {
        let source = self.documents.get(uri)?;
        let byte = line_col_to_byte(source, line, character);
        let ident = ident_at_byte(source, byte)?;

        // 1. Try AST-based lookup (works even when lowering / type-check fails)
        if let Some(ast) = parse_source(source) {
            // User-defined functions
            for func in &ast.functions {
                if func.name.name == ident {
                    let params: Vec<String> = func.params.iter()
                        .map(|p| format!("{}: {}", p.name.name, ast_type_str(&p.ty)))
                        .collect();
                    let ret = ast_type_str(&func.return_ty);
                    let vis = if func.is_pub { "pub " } else { "" };
                    return Some(format!("```iris\n{}def {}({}) -> {}\n```", vis, ident, params.join(", "), ret));
                }
            }

            // Struct (record) definitions
            for s in &ast.structs {
                if s.name.name == ident {
                    let fields: Vec<String> = s.fields.iter()
                        .map(|f| format!("    {}: {}", f.name.name, ast_type_str(&f.ty)))
                        .collect();
                    return Some(format!("```iris\nrecord {} {{\n{}\n}}\n```", ident, fields.join(",\n")));
                }
            }

            // Enum (choice) definitions
            for e in &ast.enums {
                if e.name.name == ident {
                    let variants: Vec<String> = e.variants.iter()
                        .map(|v| {
                            if v.fields.is_empty() {
                                format!("    {}", v.name.name)
                            } else {
                                let tys: Vec<String> = v.fields.iter().map(ast_type_str).collect();
                                format!("    {}({})", v.name.name, tys.join(", "))
                            }
                        })
                        .collect();
                    return Some(format!("```iris\nchoice {} {{\n{}\n}}\n```", ident, variants.join(",\n")));
                }
                // Also match variant names
                for v in &e.variants {
                    if v.name.name == ident {
                        if v.fields.is_empty() {
                            return Some(format!("```iris\n{}.{}\n```\nVariant of `{}`", e.name.name, ident, e.name.name));
                        } else {
                            let tys: Vec<String> = v.fields.iter().map(ast_type_str).collect();
                            return Some(format!("```iris\n{}.{}({})\n```\nVariant of `{}`", e.name.name, ident, tys.join(", "), e.name.name));
                        }
                    }
                }
            }

            // Constants
            for c in &ast.consts {
                if c.name.name == ident {
                    let ty_s = c.ty.as_ref().map(|t| ast_type_str(t)).unwrap_or_else(|| "(inferred)".into());
                    return Some(format!("```iris\nconst {}: {}\n```", ident, ty_s));
                }
            }

            // Type aliases
            for ta in &ast.type_aliases {
                if ta.name == ident {
                    return Some(format!("```iris\ntype {} = {}\n```", ident, ast_type_str(&ta.ty)));
                }
            }

            // Extern functions
            for ef in &ast.extern_fns {
                if ef.name.name == ident {
                    let params: Vec<String> = ef.params.iter()
                        .map(|p| format!("{}: {}", p.name.name, ast_type_str(&p.ty)))
                        .collect();
                    return Some(format!("```iris\nextern def {}({}) -> {}\n```", ident, params.join(", "), ast_type_str(&ef.ret_ty)));
                }
            }

            // Local variables — walk function bodies to find val/var bindings
            for func in &ast.functions {
                if let Some(info) = find_binding_in_block(&func.body, ident, byte) {
                    return Some(info);
                }
            }
        }

        // 2. Try IR-based lookup (if file compiles successfully, gives richer type info)
        let module_name = uri_to_module_name(uri);
        if let Ok(module) = crate::compile_to_module(source, &module_name) {
            for func in module.functions() {
                let bare = func.name.split("__").next().unwrap_or(&func.name);
                if bare == ident {
                    let params: Vec<String> = func.params.iter()
                        .map(|p| format!("{}: {:?}", p.name, p.ty))
                        .collect();
                    return Some(format!("```iris\ndef {}({}) -> {:?}\n```", bare, params.join(", "), func.return_ty));
                }
            }
        }

        // 3. Built-in function signatures
        if let Some(sig) = builtin_hover(ident) {
            return Some(sig);
        }

        // 4. Keywords
        if let Some(kw) = keyword_hover(ident) {
            return Some(kw);
        }

        None
    }

    /// Returns completion candidates for the given position.
    pub fn completions(&self, uri: &str) -> Vec<String> {
        let mut items: Vec<String> = STATIC_COMPLETIONS.iter().map(|s| s.to_string()).collect();

        if let Some(source) = self.documents.get(uri) {
            let module_name = uri_to_module_name(uri);
            if let Ok(module) = crate::compile_to_module(source, &module_name) {
                for func in module.functions() {
                    let bare = func.name.split("__").next().unwrap_or(&func.name);
                    if !bare.starts_with("__") {
                        items.push(bare.to_owned());
                    }
                }
            }
        }
        items.sort();
        items.dedup();
        items
    }

    /// Returns the definition location for the identifier at the given position.
    /// Returns `(uri, start_line, start_char, end_line, end_char)` on success.
    pub fn definition(&self, uri: &str, line: u32, character: u32) -> Option<(String, u32, u32, u32, u32)> {
        let source = self.documents.get(uri)?;
        let byte = line_col_to_byte(source, line, character);
        let ident = ident_at_byte(source, byte)?;

        let ast = parse_source(source)?;
        let def_byte = definition_byte_of(&ast, ident)?;
        let (start_line, start_char) = byte_to_lsp_pos(source, def_byte);
        let end_char = start_char + ident.len() as u32;
        Some((uri.to_owned(), start_line, start_char, start_line, end_char))
    }

    /// Returns document symbols (outline) for the given document.
    /// Each entry is `(name, kind, start_line, start_char, end_line, end_char)`.
    /// SymbolKind: 12=Function, 23=Struct, 10=Enum, 14=Constant, 26=TypeParameter
    pub fn document_symbols(&self, uri: &str) -> Vec<(String, u32, u32, u32, u32, u32)> {
        let source = match self.documents.get(uri) { Some(s) => s, None => return vec![] };
        let ast = match parse_source(source) { Some(a) => a, None => return vec![] };
        let mut symbols = Vec::new();

        for func in &ast.functions {
            let (sl, sc) = byte_to_lsp_pos(source, func.name.span.start.0);
            let end_byte = func.span.end.0.min(source.len() as u32);
            let (el, ec) = byte_to_lsp_pos(source, end_byte);
            symbols.push((func.name.name.clone(), 12u32, sl, sc, el, ec));
        }
        for s in &ast.structs {
            let (sl, sc) = byte_to_lsp_pos(source, s.name.span.start.0);
            let end_byte = s.span.end.0.min(source.len() as u32);
            let (el, ec) = byte_to_lsp_pos(source, end_byte);
            symbols.push((s.name.name.clone(), 23u32, sl, sc, el, ec));
        }
        for e in &ast.enums {
            let (sl, sc) = byte_to_lsp_pos(source, e.name.span.start.0);
            let end_byte = e.span.end.0.min(source.len() as u32);
            let (el, ec) = byte_to_lsp_pos(source, end_byte);
            symbols.push((e.name.name.clone(), 10u32, sl, sc, el, ec));
        }
        for c in &ast.consts {
            let (sl, sc) = byte_to_lsp_pos(source, c.name.span.start.0);
            let (el, ec) = (sl, sc + c.name.name.len() as u32);
            symbols.push((c.name.name.clone(), 14u32, sl, sc, el, ec));
        }
        for ta in &ast.type_aliases {
            let (sl, sc) = byte_to_lsp_pos(source, ta.span.start.0);
            let (el, ec) = (sl, sc + ta.name.len() as u32);
            symbols.push((ta.name.clone(), 26u32, sl, sc, el, ec));
        }

        // Sort by line for a predictable outline order.
        symbols.sort_by_key(|s| s.2);
        symbols
    }

    /// Returns signature help for a function call at the given position.
    /// Returns `(label, parameter_labels, active_parameter_index)`.
    pub fn signature_help(&self, uri: &str, line: u32, character: u32) -> Option<(String, Vec<String>, usize)> {
        let source = self.documents.get(uri)?;
        let (func_name, active_param) = find_call_context(source, line, character)?;
        let ast = parse_source(source)?;

        // Find the function definition.
        let func = ast.functions.iter().find(|f| f.name.name == func_name)?;

        let param_labels: Vec<String> = func.params.iter()
            .map(|p| format!("{}: {}", p.name.name, ast_type_str(&p.ty)))
            .collect();
        let ret = ast_type_str(&func.return_ty);
        let label = format!("def {}({}) -> {}", func_name, param_labels.join(", "), ret);

        Some((label, param_labels, active_param))
    }

    /// Returns a formatted version of the document source.
    pub fn format(&self, uri: &str) -> Option<String> {
        let source = self.documents.get(uri)?;
        Some(format_iris(source))
    }

    // ------------------------------------------------------------------
    // Code actions (quick fixes)
    // ------------------------------------------------------------------

    /// Returns code actions (quick fixes) for the given range.
    pub fn code_actions(&self, uri: &str, range_start_line: u32, range_start_col: u32,
                        _range_end_line: u32, _range_end_col: u32)
        -> Vec<CodeAction>
    {
        let Some(source) = self.documents.get(uri) else { return Vec::new() };
        let mut actions = Vec::new();

        // Run diagnostics to find actionable errors/warnings.
        let diags = self.diagnose(uri);

        for diag in &diags {
            // Quick fix: suggest adding missing `bring` for undefined variable that looks like a stdlib function
            if diag.message.contains("cannot find") || diag.message.contains("undefined") {
                if let Some(name) = extract_quoted_name(&diag.message) {
                    if let Some(bring_stmt) = suggest_bring_for_name(&name) {
                        actions.push(CodeAction {
                            title: format!("Add 'bring {}' to imports", bring_stmt.trim_start_matches("bring ")),
                            kind: "quickfix".to_owned(),
                            edit_uri: uri.to_owned(),
                            edit_range: (0, 0, 0, 0),
                            new_text: format!("{}\n", bring_stmt),
                            diagnostic_message: Some(diag.message.clone()),
                        });
                    }
                }
            }

            // Quick fix: remove unused variable
            if diag.severity == 2 && diag.message.contains("unused") {
                if let Some(name) = extract_quoted_name(&diag.message) {
                    actions.push(CodeAction {
                        title: format!("Prefix with underscore: _{}", name),
                        kind: "quickfix".to_owned(),
                        edit_uri: uri.to_owned(),
                        edit_range: (diag.line, diag.character, diag.end_line, diag.end_character),
                        new_text: format!("_{}", name),
                        diagnostic_message: Some(diag.message.clone()),
                    });
                }
            }

            // Quick fix: suggest closing brace for unterminated blocks
            if diag.message.contains("missing a closing brace") || diag.message.contains("unexpected end of file") {
                let line_count = source.lines().count() as u32;
                actions.push(CodeAction {
                    title: "Add closing '}'".to_owned(),
                    kind: "quickfix".to_owned(),
                    edit_uri: uri.to_owned(),
                    edit_range: (line_count, 0, line_count, 0),
                    new_text: "}\n".to_owned(),
                    diagnostic_message: Some(diag.message.clone()),
                });
            }

            // Quick fix: missing semicolon
            if diag.message.contains("expected ';'") || diag.message.contains("expected semicolon") {
                actions.push(CodeAction {
                    title: "Add missing semicolon".to_owned(),
                    kind: "quickfix".to_owned(),
                    edit_uri: uri.to_owned(),
                    edit_range: (diag.end_line, diag.end_character, diag.end_line, diag.end_character),
                    new_text: ";".to_owned(),
                    diagnostic_message: Some(diag.message.clone()),
                });
            }

            // Quick fix: type mismatch — suggest explicit cast
            if diag.message.contains("type mismatch") || diag.message.contains("expected") && diag.message.contains("found") {
                // Extract expected/found types
                if let (Some(expected), Some(found)) = (
                    extract_type_from_mismatch(&diag.message, "expected"),
                    extract_type_from_mismatch(&diag.message, "found"),
                ) {
                    if is_numeric_type(&expected) && is_numeric_type(&found) {
                        actions.push(CodeAction {
                            title: format!("Cast to {}", expected),
                            kind: "quickfix".to_owned(),
                            edit_uri: uri.to_owned(),
                            edit_range: (diag.line, diag.character, diag.end_line, diag.end_character),
                            new_text: format!("/* cast to {} */", expected),
                            diagnostic_message: Some(diag.message.clone()),
                        });
                    }
                }
            }

            // Quick fix: BP005 empty function body -> insert panic("todo")
            if diag.code.as_deref() == Some("BP005") {
                if let Some(name) = extract_quoted_name(&diag.message) {
                    // Find the opening brace of this function and insert after it
                    let func_line = diag.line;
                    if let Some(line_text) = source.lines().nth(func_line as usize) {
                        if let Some(brace_pos) = line_text.rfind('{') {
                            actions.push(CodeAction {
                                title: format!("Add placeholder body to '{}'", name),
                                kind: "quickfix".to_owned(),
                                edit_uri: uri.to_owned(),
                                edit_range: (func_line + 1, 0, func_line + 1, 0),
                                new_text: "    panic(\"todo: implement\")\n".to_owned(),
                                diagnostic_message: Some(diag.message.clone()),
                            });
                            let _ = brace_pos;
                        }
                    }
                }
            }

            // Quick fix: BP002 missing doc comment -> add template
            if diag.code.as_deref() == Some("BP002") {
                if let Some(name) = extract_quoted_name(&diag.message) {
                    actions.push(CodeAction {
                        title: format!("Add doc comment for '{}'", name),
                        kind: "quickfix".to_owned(),
                        edit_uri: uri.to_owned(),
                        edit_range: (diag.line, 0, diag.line, 0),
                        new_text: format!("// {} — TODO: describe this function.\n", name),
                        diagnostic_message: Some(diag.message.clone()),
                    });
                }
            }

            // Quick fix: BP004 non-snake_case -> suggest rename
            if diag.code.as_deref() == Some("BP004") {
                if let Some(name) = extract_quoted_name(&diag.message) {
                    let snake = to_snake_case(&name);
                    if snake != name {
                        actions.push(CodeAction {
                            title: format!("Rename '{}' to '{}'", name, snake),
                            kind: "quickfix".to_owned(),
                            edit_uri: uri.to_owned(),
                            edit_range: (diag.line, diag.character, diag.end_line, diag.end_character),
                            new_text: snake,
                            diagnostic_message: Some(diag.message.clone()),
                        });
                    }
                }
            }

            // Quick fix: BP006 redundant double semicolon -> remove one
            if diag.code.as_deref() == Some("BP006") {
                actions.push(CodeAction {
                    title: "Remove redundant semicolon".to_owned(),
                    kind: "quickfix".to_owned(),
                    edit_uri: uri.to_owned(),
                    edit_range: (diag.line, diag.character, diag.end_line, diag.end_character),
                    new_text: String::new(),
                    diagnostic_message: Some(diag.message.clone()),
                });
            }
        }

        // Source action: extract variable at cursor position
        let byte = line_col_to_byte(source, range_start_line, range_start_col) as usize;
        if let Some(word) = ident_at_byte(source, byte as u32) {
            if word.len() > 1 && !is_keyword(word) {
                actions.push(CodeAction {
                    title: format!("Extract '{}' to variable", word),
                    kind: "refactor.extract".to_owned(),
                    edit_uri: uri.to_owned(),
                    edit_range: (range_start_line, 0, range_start_line, 0),
                    new_text: format!("    val extracted_{} = {};\n", word, word),
                    diagnostic_message: None,
                });
            }
        }

        // Source action: wrap selection in if block
        actions.push(CodeAction {
            title: "Wrap in if condition".to_owned(),
            kind: "refactor.extract".to_owned(),
            edit_uri: uri.to_owned(),
            edit_range: (range_start_line, 0, range_start_line, 0),
            new_text: "if true {\n".to_owned(),
            diagnostic_message: None,
        });

        actions
    }

    // ------------------------------------------------------------------
    // Inlay hints
    // ------------------------------------------------------------------

    /// Returns inlay hints for the document — type annotations on `val`/`var` bindings.
    /// Each hint is (line, character, label, kind) where kind is 1=Type, 2=Parameter.
    pub fn inlay_hints(&self, uri: &str) -> Vec<InlayHint> {
        let Some(source) = self.documents.get(uri) else { return Vec::new() };
        let Some(ast) = parse_source(source) else { return Vec::new() };
        let mut hints = Vec::new();

        // Walk all function bodies looking for val/var bindings without explicit types.
        for func in &ast.functions {
            self.collect_inlay_hints_from_stmts(&func.body.stmts, source, &mut hints);
        }

        hints
    }

    fn collect_inlay_hints_from_stmts(&self, stmts: &[crate::parser::ast::AstStmt], source: &str, hints: &mut Vec<InlayHint>) {
        use crate::parser::ast::AstStmt;
        for stmt in stmts {
            match stmt {
                AstStmt::Let { name, ty, .. } => {
                    // Only add hint if no explicit type annotation
                    if ty.is_none() {
                        let (line, col) = byte_to_lsp_pos(source, name.span.end.0);
                        hints.push(InlayHint {
                            line,
                            character: col,
                            label: ": (inferred)".to_owned(),
                            kind: 1, // Type
                        });
                    }
                }
                AstStmt::While { body, .. } | AstStmt::Loop { body, .. } => {
                    self.collect_inlay_hints_from_stmts(&body.stmts, source, hints);
                }
                AstStmt::ForRange { body, .. } | AstStmt::ForEach { body, .. } | AstStmt::ParFor { body, .. } => {
                    self.collect_inlay_hints_from_stmts(&body.stmts, source, hints);
                }
                AstStmt::Spawn { body, .. } => {
                    self.collect_inlay_hints_from_stmts(body, source, hints);
                }
                _ => {}
            }
        }
    }

    // ------------------------------------------------------------------
    // Find references
    // ------------------------------------------------------------------

    /// Finds all occurrences of the identifier at (line, character) in the document.
    /// Returns Vec<(start_line, start_col, end_line, end_col)>.
    pub fn references(&self, uri: &str, line: u32, character: u32) -> Vec<(u32, u32, u32, u32)> {
        let Some(source) = self.documents.get(uri) else { return Vec::new() };
        let byte = line_col_to_byte(source, line, character) as u32;
        let Some(target) = ident_at_byte(source, byte) else { return Vec::new() };
        let mut refs = Vec::new();

        // Simple text-based reference search: find all occurrences of the identifier
        // bounded by non-identifier characters.
        let target_bytes = target.as_bytes();
        let src_bytes = source.as_bytes();
        let mut i = 0usize;
        while i < src_bytes.len() {
            if i + target_bytes.len() <= src_bytes.len()
                && &src_bytes[i..i + target_bytes.len()] == target_bytes
            {
                // Check boundaries
                let before_ok = i == 0 || !is_ident_char(src_bytes[i - 1]);
                let after_ok = i + target_bytes.len() >= src_bytes.len()
                    || !is_ident_char(src_bytes[i + target_bytes.len()]);
                if before_ok && after_ok {
                    let (sl, sc) = byte_to_lsp_pos(source, i as u32);
                    let (el, ec) = byte_to_lsp_pos(source, (i + target_bytes.len()) as u32);
                    refs.push((sl, sc, el, ec));
                }
            }
            i += 1;
        }

        refs
    }

    // ------------------------------------------------------------------
    // Rename
    // ------------------------------------------------------------------

    /// Renames all occurrences of the identifier at (line, character) to `new_name`.
    /// Returns Vec<(start_line, start_col, end_line, end_col, new_text)>.
    pub fn rename(&self, uri: &str, line: u32, character: u32, new_name: &str) -> Vec<(u32, u32, u32, u32, String)> {
        let refs = self.references(uri, line, character);
        refs.into_iter()
            .map(|(sl, sc, el, ec)| (sl, sc, el, ec, new_name.to_owned()))
            .collect()
    }

    // ------------------------------------------------------------------
    // Private
    // ------------------------------------------------------------------

    fn diagnose(&self, uri: &str) -> Vec<LspDiagnostic> {
        let Some(source) = self.documents.get(uri) else { return Vec::new() };
        let module_name = uri_to_module_name(uri);
        let mut diags = Vec::new();

        // Try file-based compilation first (resolves bring declarations).
        // Fall back to in-memory compilation for unsaved / untitled files.
        let compile_result = if let Some(path) = uri_to_file_path(uri) {
            crate::compile_file_text(source, &path, EmitKind::Ir)
        } else {
            crate::compile(source, &module_name, EmitKind::Ir)
        };

        if let Err(e) = compile_result {
            let (line, character) = if let Some(byte) = error_byte_offset(&e) {
                let (l, c) = byte_to_line_col(source, byte);
                (l.saturating_sub(1), c.saturating_sub(1))
            } else {
                (0, 0)
            };
            let code = Some(e.diagnostic_code().to_owned());
            diags.push(LspDiagnostic {
                line,
                character,
                end_line: line,
                end_character: character + 1,
                message: format!("{}", e),
                severity: 1,
                code,
            });
        }

        // Collect dead-variable warnings directly from the single-file AST.
        // This works even when bring resolution fails (warnings are per-file).
        if let Some(ast) = parse_source(source) {
            for w in crate::pass::find_unused_vars(&ast) {
                let (line, character) = if let Some(sp) = w.span {
                    let (l, c) = byte_to_line_col(source, sp.start.0);
                    (l.saturating_sub(1), c.saturating_sub(1))
                } else {
                    (0u32, 0u32)
                };
                diags.push(LspDiagnostic {
                    line,
                    character,
                    end_line: line,
                    end_character: character + 1,
                    message: w.message,
                    severity: 2,
                    code: Some("W0001".to_owned()),
                });
            }

            // ── Best-practice diagnostics ──

            // BP001: Long function body (> 50 lines) — information hint
            for func in &ast.functions {
                let func_start = byte_to_line_col(source, func.span.start.0).0;
                let func_end = byte_to_line_col(source, func.span.end.0).0;
                let func_lines = func_end.saturating_sub(func_start);
                if func_lines > 50 {
                    let (line, character) = byte_to_lsp_pos(source, func.name.span.start.0);
                    diags.push(LspDiagnostic {
                        line,
                        character,
                        end_line: line,
                        end_character: character + func.name.name.len() as u32,
                        message: format!(
                            "Function '{}' is {} lines long. Consider splitting into smaller functions.",
                            func.name.name, func_lines
                        ),
                        severity: 4, // Hint
                        code: Some("BP001".to_owned()),
                    });
                }
            }

            // BP002: Missing doc comment on pub functions — hint
            let source_lines: Vec<&str> = source.lines().collect();
            for func in &ast.functions {
                if func.is_pub {
                    let (func_line, _) = byte_to_lsp_pos(source, func.span.start.0);
                    let has_doc = if func_line > 0 {
                        let prev_line = source_lines.get((func_line - 1) as usize).unwrap_or(&"");
                        prev_line.trim().starts_with("//")
                    } else {
                        false
                    };
                    if !has_doc {
                        diags.push(LspDiagnostic {
                            line: func_line,
                            character: 0,
                            end_line: func_line,
                            end_character: 3,
                            message: format!(
                                "Public function '{}' is missing a doc comment. Add a // comment above.",
                                func.name.name
                            ),
                            severity: 4, // Hint
                            code: Some("BP002".to_owned()),
                        });
                    }
                }
            }

            // BP003: Function with many parameters (> 5) — hint
            for func in &ast.functions {
                if func.params.len() > 5 {
                    let (line, character) = byte_to_lsp_pos(source, func.name.span.start.0);
                    diags.push(LspDiagnostic {
                        line,
                        character,
                        end_line: line,
                        end_character: character + func.name.name.len() as u32,
                        message: format!(
                            "Function '{}' has {} parameters. Consider using a record type.",
                            func.name.name, func.params.len()
                        ),
                        severity: 4,
                        code: Some("BP003".to_owned()),
                    });
                }
            }

            // BP004: Non-snake_case function/variable naming convention — hint
            for func in &ast.functions {
                if !is_snake_case(&func.name.name) && func.name.name != "main" {
                    let (line, character) = byte_to_lsp_pos(source, func.name.span.start.0);
                    diags.push(LspDiagnostic {
                        line,
                        character,
                        end_line: line,
                        end_character: character + func.name.name.len() as u32,
                        message: format!(
                            "Function '{}' should use snake_case naming convention.",
                            func.name.name
                        ),
                        severity: 4,
                        code: Some("BP004".to_owned()),
                    });
                }
            }

            // BP005: Empty function body — warning
            for func in &ast.functions {
                if func.body.stmts.is_empty() && func.body.tail.is_none() {
                    let (line, character) = byte_to_lsp_pos(source, func.name.span.start.0);
                    diags.push(LspDiagnostic {
                        line,
                        character,
                        end_line: line,
                        end_character: character + func.name.name.len() as u32,
                        message: format!(
                            "Function '{}' has an empty body. Add implementation or use `panic(\"todo\")`.",
                            func.name.name
                        ),
                        severity: 2,
                        code: Some("BP005".to_owned()),
                    });
                }
            }

            // BP006: Redundant trailing semicolons on last statement in block — hint
            // Detect lines ending with `;;`
            for (i, line_str) in source_lines.iter().enumerate() {
                let trimmed = line_str.trim();
                if trimmed.ends_with(";;") {
                    let col = line_str.len().saturating_sub(1) as u32;
                    diags.push(LspDiagnostic {
                        line: i as u32,
                        character: col,
                        end_line: i as u32,
                        end_character: col + 1,
                        message: "Redundant double semicolon.".to_owned(),
                        severity: 4,
                        code: Some("BP006".to_owned()),
                    });
                }
            }
        }

        diags
    }
}

// ---------------------------------------------------------------------------
// AST helpers
// ---------------------------------------------------------------------------

fn parse_source(source: &str) -> Option<crate::parser::ast::AstModule> {
    use crate::parser::lexer::Lexer;
    use crate::parser::parse::Parser;
    let tokens = Lexer::new(source).tokenize().ok()?;
    Parser::new(&tokens).parse_module().ok()
}

/// Finds the byte offset of the definition of `name` in the AST.
fn definition_byte_of(ast: &crate::parser::ast::AstModule, name: &str) -> Option<u32> {
    for func in &ast.functions {
        if func.name.name == name {
            return Some(func.name.span.start.0);
        }
    }
    for s in &ast.structs {
        if s.name.name == name {
            return Some(s.name.span.start.0);
        }
    }
    for e in &ast.enums {
        if e.name.name == name {
            return Some(e.name.span.start.0);
        }
    }
    for c in &ast.consts {
        if c.name.name == name {
            return Some(c.name.span.start.0);
        }
    }
    for ta in &ast.type_aliases {
        if ta.name == name {
            return Some(ta.span.start.0);
        }
    }
    None
}

/// Converts a byte offset to a 0-based (line, character) LSP position.
fn byte_to_lsp_pos(source: &str, byte: u32) -> (u32, u32) {
    let byte = byte as usize;
    let prefix = if byte <= source.len() { &source[..byte] } else { source };
    let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
    let col = prefix.rfind('\n').map(|i| byte - i - 1).unwrap_or(byte) as u32;
    (line, col)
}

/// Converts an AstType to a display string.
fn ast_type_str(ty: &crate::parser::ast::AstType) -> String {
    use crate::parser::ast::{AstType, AstScalarKind};
    match ty {
        AstType::Scalar(k, _) => match k {
            AstScalarKind::I64 => "i64",
            AstScalarKind::I32 => "i32",
            AstScalarKind::F64 => "f64",
            AstScalarKind::F32 => "f32",
            AstScalarKind::Bool => "bool",
            AstScalarKind::U8 => "u8",
            AstScalarKind::I8 => "i8",
            AstScalarKind::U32 => "u32",
            AstScalarKind::U64 => "u64",
            AstScalarKind::USize => "usize",
        }.to_owned(),
        AstType::Named(n, _) => n.clone(),
        AstType::Tuple(ts, _) => {
            let inner: Vec<String> = ts.iter().map(ast_type_str).collect();
            format!("({})", inner.join(", "))
        }
        AstType::List(t, _) => format!("list<{}>", ast_type_str(t)),
        AstType::Map(k, v, _) => format!("map<{}, {}>", ast_type_str(k), ast_type_str(v)),
        AstType::Option(t, _) => format!("option<{}>", ast_type_str(t)),
        AstType::Result(t, e, _) => format!("result<{}, {}>", ast_type_str(t), ast_type_str(e)),
        AstType::Fn { params, ret, .. } => {
            let ps: Vec<String> = params.iter().map(ast_type_str).collect();
            format!("({}) -> {}", ps.join(", "), ast_type_str(ret))
        }
        _ => "?".to_owned(),
    }
}

/// Finds the function name and active parameter index at the cursor position.
/// Scans backwards to find the innermost unclosed `(` and the identifier before it.
fn find_call_context(source: &str, line: u32, character: u32) -> Option<(String, usize)> {
    let cursor_byte = line_col_to_byte(source, line, character) as usize;
    let prefix = &source[..cursor_byte.min(source.len())];

    let mut depth = 0i32;
    let mut open_pos = None;
    for (i, ch) in prefix.char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    open_pos = Some(i);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    let open = open_pos?;
    // Count commas between open_pos and cursor to find active parameter.
    let between = &prefix[open + 1..];
    let mut depth2 = 0i32;
    let active_param = between.chars().filter(|&c| {
        if c == '(' || c == '[' { depth2 += 1; }
        if c == ')' || c == ']' { depth2 -= 1; }
        c == ',' && depth2 == 0
    }).count();

    // Find identifier immediately before `(`.
    let before_paren = prefix[..open].trim_end();
    let ident_end = before_paren.len();
    let ident_start = before_paren.rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let func_name = &before_paren[ident_start..ident_end];
    if func_name.is_empty() { return None; }

    Some((func_name.to_owned(), active_param))
}

// ---------------------------------------------------------------------------
// Hover helpers
// ---------------------------------------------------------------------------

/// Searches a block (and nested blocks) for a val/var binding matching `name`
/// whose span encompasses the cursor `byte` position.
fn find_binding_in_block(block: &crate::parser::ast::AstBlock, name: &str, _byte: u32) -> Option<String> {
    for stmt in &block.stmts {
        match stmt {
            crate::parser::ast::AstStmt::Let { name: ident, ty, .. } => {
                if ident.name == name {
                    let ty_str = ty.as_ref().map(|t| ast_type_str(t)).unwrap_or_else(|| "(inferred)".to_owned());
                    // Detect if it was `val` or `var` by checking the source at the span.
                    return Some(format!("```iris\nval {}: {}\n```", name, ty_str));
                }
            }
            crate::parser::ast::AstStmt::ForRange { var, body, .. } => {
                if var.name == name {
                    return Some(format!("```iris\nfor {}: i64\n```\nLoop variable", name));
                }
                if let Some(info) = find_binding_in_block(body, name, _byte) {
                    return Some(info);
                }
            }
            crate::parser::ast::AstStmt::While { body, .. } => {
                if let Some(info) = find_binding_in_block(body, name, _byte) {
                    return Some(info);
                }
            }
            crate::parser::ast::AstStmt::Loop { body, .. } => {
                if let Some(info) = find_binding_in_block(body, name, _byte) {
                    return Some(info);
                }
            }
            crate::parser::ast::AstStmt::ParFor { var, body, .. } => {
                if var.name == name {
                    return Some(format!("```iris\npar for {}: i64\n```\nParallel loop variable", name));
                }
                if let Some(info) = find_binding_in_block(body, name, _byte) {
                    return Some(info);
                }
            }
            crate::parser::ast::AstStmt::ForEach { var, body, .. } => {
                if var.name == name {
                    return Some(format!("```iris\nfor {} in ...\n```\nIterator variable", name));
                }
                if let Some(info) = find_binding_in_block(body, name, _byte) {
                    return Some(info);
                }
            }
            crate::parser::ast::AstStmt::LetTuple { names, .. } => {
                for n in names {
                    if n.name == name {
                        return Some(format!("```iris\nval {} (destructured)\n```", name));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Returns hover info for a built-in function.
fn builtin_hover(name: &str) -> Option<String> {
    let sig = match name {
        // Math
        "sin"   => "def sin(x: f32) -> f32\nSine of x (radians)",
        "cos"   => "def cos(x: f32) -> f32\nCosine of x (radians)",
        "tan"   => "def tan(x: f32) -> f32\nTangent of x (radians)",
        "exp"   => "def exp(x: f32) -> f32\ne^x",
        "log"   => "def log(x: f32) -> f32\nNatural logarithm",
        "log2"  => "def log2(x: f32) -> f32\nBase-2 logarithm",
        "sqrt"  => "def sqrt(x: f32) -> f32\nSquare root",
        "abs"   => "def abs(x: f32) -> f32\nAbsolute value",
        "floor" => "def floor(x: f32) -> f32\nFloor",
        "ceil"  => "def ceil(x: f32) -> f32\nCeiling",
        "round" => "def round(x: f32) -> f32\nRound to nearest",
        "sign"  => "def sign(x: f32) -> f32\nSign: -1, 0, or 1",
        "pow"   => "def pow(base: f32, exp: f32) -> f32\nPower",
        "min"   => "def min(a: f32, b: f32) -> f32\nMinimum of two values",
        "max"   => "def max(a: f32, b: f32) -> f32\nMaximum of two values",
        "clamp" => "def clamp(x: f32, lo: f32, hi: f32) -> f32\nClamp x to [lo, hi]",
        // String
        "len"         => "def len(s: str) -> i64\nByte length of string (or collection size)",
        "concat"      => "def concat(a: str, b: str) -> str\nConcatenate two strings",
        "contains"    => "def contains(s: str, sub: str) -> bool\nTest if s contains sub",
        "starts_with" => "def starts_with(s: str, prefix: str) -> bool\nPrefix test",
        "ends_with"   => "def ends_with(s: str, suffix: str) -> bool\nSuffix test",
        "to_upper"    => "def to_upper(s: str) -> str\nConvert to uppercase",
        "to_lower"    => "def to_lower(s: str) -> str\nConvert to lowercase",
        "trim"        => "def trim(s: str) -> str\nStrip leading/trailing whitespace",
        "repeat"      => "def repeat(s: str, n: i64) -> str\nRepeat string n times",
        "to_str"      => "def to_str(v: T) -> str\nConvert any value to string",
        "split"       => "def split(s: str, delim: str) -> list<str>\nSplit by delimiter",
        "join"        => "def join(parts: list<str>, delim: str) -> str\nJoin with delimiter",
        "slice"       => "def slice(s: str, start: i64, end: i64) -> str\nSubstring [start, end)",
        "find"        => "def find(s: str, sub: str) -> option<i64>\nFind first occurrence index",
        "str_replace" => "def str_replace(s: str, old: str, new: str) -> str\nReplace all occurrences",
        "parse_i64"   => "def parse_i64(s: str) -> option<i64>\nParse integer from string",
        "parse_f64"   => "def parse_f64(s: str) -> option<f64>\nParse float from string",
        // I/O
        "print"     => "def print(v: T) -> ()\nPrint value to stdout with newline",
        "read_line" => "def read_line() -> str\nRead a line from stdin",
        "read_i64"  => "def read_i64() -> i64\nRead and parse integer from stdin",
        "read_f64"  => "def read_f64() -> f64\nRead and parse float from stdin",
        // List
        "push"          => "def push(lst: list<T>, v: T) -> ()\nAppend element to list",
        "pop"           => "def pop(lst: list<T>) -> option<T>\nRemove and return last element",
        "list_len"      => "def list_len(lst: list<T>) -> i64\nNumber of elements",
        "list_get"      => "def list_get(lst: list<T>, i: i64) -> T\nGet element by index",
        "list_set"      => "def list_set(lst: list<T>, i: i64, v: T) -> ()\nSet element by index",
        "list_contains" => "def list_contains(lst: list<T>, v: T) -> bool\nMembership test",
        "list_sort"     => "def list_sort(lst: list<T>) -> ()\nSort list in-place",
        "list_concat"   => "def list_concat(a: list<T>, b: list<T>) -> list<T>\nConcatenate two lists",
        "list_slice"    => "def list_slice(lst: list<T>, start: i64, end: i64) -> list<T>\nSlice [start, end)",
        // Map
        "map_set"      => "def map_set(m: map<K,V>, key: K, val: V) -> ()\nInsert or update entry",
        "map_get"      => "def map_get(m: map<K,V>, key: K) -> option<V>\nLookup value by key",
        "map_contains" => "def map_contains(m: map<K,V>, key: K) -> bool\nCheck if key exists",
        "map_remove"   => "def map_remove(m: map<K,V>, key: K) -> ()\nRemove entry by key",
        "map_len"      => "def map_len(m: map<K,V>) -> i64\nNumber of entries",
        "map_keys"     => "def map_keys(m: map<K,V>) -> list<K>\nAll keys",
        "map_values"   => "def map_values(m: map<K,V>) -> list<V>\nAll values",
        // Option / Result
        "some"    => "def some(v: T) -> option<T>\nWrap value in Some",
        "is_some" => "def is_some(opt: option<T>) -> bool\nTrue if option has a value",
        "is_ok"   => "def is_ok(r: result<T,E>) -> bool\nTrue if result is Ok",
        "unwrap"  => "def unwrap(opt: option<T>) -> T\nExtract value (panics on none/err)",
        "ok"      => "def ok(v: T) -> result<T, E>\nCreate a success result",
        "err"     => "def err(e: E) -> result<T, E>\nCreate an error result",
        // Concurrency
        "channel"      => "def channel() -> channel<T>\nCreate a new channel",
        "send"         => "def send(ch: channel<T>, v: T) -> ()\nSend value to channel",
        "recv"         => "def recv(ch: channel<T>) -> T\nReceive value from channel (blocking)",
        "atomic"       => "def atomic(v: T) -> atomic<T>\nCreate an atomic value",
        "atomic_load"  => "def atomic_load(a: atomic<T>) -> T\nRead atomically",
        "atomic_store" => "def atomic_store(a: atomic<T>, v: T) -> ()\nWrite atomically",
        "atomic_add"   => "def atomic_add(a: atomic<T>, v: T) -> T\nAtomically add and return new value",
        // Time
        "time_now_ms"   => "def time_now_ms() -> i64\nCurrent time in milliseconds since epoch",
        "sleep_ms" => "def sleep_ms(ms: i64) -> i64\nSleep for ms milliseconds",
        // Database (SQLite)
        "db_open"  => "def db_open(path: str) -> i64\nOpen a SQLite database, returns handle",
        "db_exec"  => "def db_exec(db: i64, sql: str) -> i64\nExecute SQL (INSERT/UPDATE/DELETE/CREATE), returns 0 on success",
        "db_query" => "def db_query(db: i64, sql: str) -> list<list<str>>\nQuery SQL (SELECT), returns rows of string columns",
        "db_close" => "def db_close(db: i64) -> i64\nClose a database handle",
        // Control
        "panic"  => "def panic(msg: str) -> !\nAbort with error message",
        "assert" => "def assert(cond: bool) -> ()\nAssert condition (panics if false)",
        // Grad
        "grad"    => "def grad(value: f64, tangent: f64) -> grad<f64>\nCreate dual number for autodiff",
        "grad_of" => "def grad_of(g: grad<f64>) -> f64\nExtract gradient (tangent) value",
        // File I/O
        "file_read_all"  => "def file_read_all(path: str) -> result<str, str>\nRead entire file",
        "file_write_all" => "def file_write_all(path: str, content: str) -> result<(), str>\nWrite file",
        "file_exists"    => "def file_exists(path: str) -> bool\nCheck if file exists",
        "file_lines"     => "def file_lines(path: str) -> list<str>\nRead file as list of lines",
        // Process
        "process_args" => "def process_args() -> list<str>\nGet command-line arguments",
        "env_var"      => "def env_var(name: str) -> option<str>\nGet environment variable",
        "process_exit" => "def process_exit(code: i64) -> ()\nExit with code",
        // TCP
        "tcp_listen"  => "def tcp_listen(port: i64) -> i64\nBind and listen, returns fd",
        "tcp_accept"  => "def tcp_accept(fd: i64) -> i64\nAccept connection, returns fd",
        "tcp_connect" => "def tcp_connect(host: str, port: i64) -> i64\nConnect to server",
        "tcp_read"    => "def tcp_read(fd: i64) -> str\nRead a line from connection",
        "tcp_write"   => "def tcp_write(fd: i64, data: str) -> ()\nWrite data to connection",
        "tcp_close"   => "def tcp_close(fd: i64) -> ()\nClose connection",
        // Tensor
        "einsum"   => "def einsum(notation: str, ...) -> tensor<...>\nEinstein summation",
        "sparsify" => "def sparsify(t: tensor<T, S>) -> sparse<T, S>\nConvert to sparse representation",
        "densify"  => "def densify(s: sparse<T, S>) -> tensor<T, S>\nConvert sparse to dense",
        "zeros"    => "def zeros(shape: [i64]) -> tensor<f32, S>\nCreate zero-filled tensor",
        "ones"     => "def ones(shape: [i64]) -> tensor<f32, S>\nCreate one-filled tensor",
        "fill"     => "def fill(shape: [i64], v: f32) -> tensor<f32, S>\nCreate tensor filled with v",
        "linspace" => "def linspace(start: f64, end: f64, n: i64) -> tensor<f64, [N]>\nEvenly-spaced values",
        // Phase 104 builtins
        "http_get"           => "def http_get(url: str) -> str\nHTTP GET request, returns response body",
        "http_post"          => "def http_post(url: str, body: str) -> str\nHTTP POST request, returns response body",
        "json_stringify"     => "def json_stringify(value: any) -> str\nSerialize value to JSON string",
        "regex_match"        => "def regex_match(pattern: str, text: str) -> bool\nTest if text matches regex pattern (. * + ? ^ $)",
        "regex_find_all"     => "def regex_find_all(pattern: str, text: str) -> list<str>\nFind all matches of pattern in text",
        "regex_replace"      => "def regex_replace(pattern: str, text: str, replacement: str) -> str\nReplace pattern matches with replacement",
        "datetime_now"       => "def datetime_now() -> str\nCurrent UTC date-time in ISO 8601 format",
        "datetime_timestamp" => "def datetime_timestamp() -> f64\nCurrent Unix timestamp in seconds",
        "datetime_format"    => "def datetime_format(fmt: str) -> str\nFormat current time (%Y %m %d %H %M %S)",
        "cwd"                => "def cwd() -> str\nGet current working directory",
        "list_dir"           => "def list_dir(path: str) -> list<str>\nList directory entries",
        "path_join"          => "def path_join(a: str, b: str) -> str\nJoin two path components",
        "mkdir"              => "def mkdir(path: str) -> bool\nCreate directory (recursive), returns success",
        "remove_file"        => "def remove_file(path: str) -> bool\nDelete a file, returns success",
        "type_of"            => "def type_of(value: any) -> str\nReturn the runtime type name",
        "random"             => "def random() -> f64\nRandom float in [0, 1)",
        "random_range"       => "def random_range(lo: i64, hi: i64) -> i64\nRandom integer in [lo, hi)",
        "hash"               => "def hash(s: str) -> i64\nDJB2 hash of string",
        "base64_encode"      => "def base64_encode(s: str) -> str\nBase64 encode a string",
        "base64_decode"      => "def base64_decode(s: str) -> str\nBase64 decode a string",
        "char_at"            => "def char_at(s: str, i: i64) -> str\nGet character at index",
        "str_reverse"        => "def str_reverse(s: str) -> str\nReverse a string",
        // Phase 105 hover docs
        "chan_try_recv"      => "def chan_try_recv(ch: chan<T>) -> Option<T>\nNon-blocking receive from channel",
        "chan_len"           => "def chan_len(ch: chan<T>) -> i64\nNumber of pending messages in channel",
        "select"             => "def select(ch1, ch2, ...) -> i64\nReturn index of first ready channel, or -1",
        "timeout"            => "def timeout(ms: i64) -> bool\nSleep for ms milliseconds, returns true",
        "thread_count"       => "def thread_count() -> i64\nNumber of available CPU threads",
        "deque_new"          => "def deque_new() -> deque<T>\nCreate an empty double-ended queue",
        "deque_push_front"   => "def deque_push_front(dq, val) -> deque<T>\nPush value to front of deque",
        "deque_push_back"    => "def deque_push_back(dq, val) -> deque<T>\nPush value to back of deque",
        "deque_pop_front"    => "def deque_pop_front(dq) -> T\nRemove and return front element",
        "deque_pop_back"     => "def deque_pop_back(dq) -> T\nRemove and return back element",
        "deque_len"          => "def deque_len(dq) -> i64\nNumber of elements in deque",
        "deque_front"        => "def deque_front(dq) -> T\nPeek at front element without removing",
        "deque_back"         => "def deque_back(dq) -> T\nPeek at back element without removing",
        "sorted_keys"        => "def sorted_keys(m: map<str,T>) -> list<str>\nReturn map keys in sorted order",
        "bitset_new"         => "def bitset_new() -> bitset\nCreate an empty bit set",
        "bitset_set"         => "def bitset_set(bs, pos: i64) -> bitset\nSet bit at position",
        "bitset_get"         => "def bitset_get(bs, pos: i64) -> bool\nGet bit at position",
        "bitset_count"       => "def bitset_count(bs) -> i64\nCount number of set bits",
        "bitset_clear"       => "def bitset_clear(bs, pos: i64) -> bitset\nClear bit at position",
        "ffi_open"           => "def ffi_open(path: str) -> i64\nOpen dynamic library, returns handle (-1 on error)",
        "ffi_call"           => "def ffi_call(handle: i64, name: str) -> i64\nCall zero-arg function in loaded library",
        "ffi_close"          => "def ffi_close(handle: i64) -> bool\nClose dynamic library handle",
        // Expanded C FFI
        "ffi_call_i64"       => "def ffi_call_i64(handle: i64, name: str, args...: i64) -> i64\nCall C function with i64 args, returns i64",
        "ffi_call_f64"       => "def ffi_call_f64(handle: i64, name: str, args...: i64) -> f64\nCall C function, returns f64",
        "ffi_call_str"       => "def ffi_call_str(handle: i64, name: str) -> str\nCall C function returning a C string",
        "ffi_call_void"      => "def ffi_call_void(handle: i64, name: str, args...: i64)\nCall C function returning void",
        "ffi_call_args"      => "def ffi_call_args(handle: i64, name: str, args...) -> i64\nCall C function with variadic arguments",
        // Python FFI
        "python_eval"        => "def python_eval(code: str) -> str\nEvaluate a Python expression, returns result as string",
        "python_exec"        => "def python_exec(code_or_path: str) -> i64\nExecute Python script/code, returns exit code",
        "python_call"        => "def python_call(module: str, func: str, args...) -> str\nCall a Python function from a module",
        "python_version"     => "def python_version() -> str\nGet installed Python version string",
        // Rust FFI (cdylib)
        "rust_lib_open"      => "def rust_lib_open(path: str) -> i64\nOpen a Rust cdylib (.dll/.so/.dylib)",
        "rust_call_i64"      => "def rust_call_i64(handle: i64, name: str, args...: i64) -> i64\nCall Rust extern \"C\" fn returning i64",
        "rust_call_f64"      => "def rust_call_f64(handle: i64, name: str, args...: i64) -> f64\nCall Rust extern \"C\" fn returning f64",
        "rust_call_void"     => "def rust_call_void(handle: i64, name: str, args...: i64)\nCall Rust extern \"C\" fn returning void",
        "env_get"            => "def env_get(key: str) -> str\nGet environment variable value",
        "env_set"            => "def env_set(key: str, val: str) -> bool\nSet environment variable",
        "exit_code"          => "def exit_code(code: i64)\nExit process with given code",
        "exec_cmd"           => "def exec_cmd(cmd: str) -> str\nExecute shell command, return stdout",
        "pid"                => "def pid() -> i64\nGet current process ID",
        "uuid"               => "def uuid() -> str\nGenerate a UUID v4 string",
        "sha256"             => "def sha256(s: str) -> str\nCompute SHA-256 hash (hex string)",
        "hex_encode"         => "def hex_encode(s: str) -> str\nEncode string to hexadecimal",
        "hex_decode"         => "def hex_decode(s: str) -> str\nDecode hexadecimal string",
        "str_pad_left"       => "def str_pad_left(s: str, width: i64, pad: str) -> str\nPad string on the left to given width",
        "str_pad_right"      => "def str_pad_right(s: str, width: i64, pad: str) -> str\nPad string on the right to given width",
        "str_chars"          => "def str_chars(s: str) -> list<str>\nSplit string into list of characters",
        "str_bytes"          => "def str_bytes(s: str) -> list<i64>\nGet list of byte values",
        "str_count"          => "def str_count(s: str, sub: str) -> i64\nCount occurrences of substring",
        "math_pi"            => "def math_pi() -> f64\nReturn pi (3.14159...)",
        "math_e"             => "def math_e() -> f64\nReturn Euler's number e (2.71828...)",
        "math_inf"           => "def math_inf() -> f64\nReturn positive infinity",
        "is_nan"             => "def is_nan(x: f64) -> bool\nCheck if value is NaN",
        "is_inf"             => "def is_inf(x: f64) -> bool\nCheck if value is infinite",
        "list_map"           => "def list_map(xs: list<T>, f: fn(T)->U) -> list<U>\nApply function to each element",
        "list_filter"        => "def list_filter(xs: list<T>, f: fn(T)->bool) -> list<T>\nKeep elements where f returns true",
        "list_reduce"        => "def list_reduce(xs: list<T>, init: U, f: fn(U,T)->U) -> U\nFold list from left",
        "list_any"           => "def list_any(xs: list<T>) -> bool\nTrue if any element is truthy",
        "list_all"           => "def list_all(xs: list<T>) -> bool\nTrue if all elements are truthy",
        "list_zip"           => "def list_zip(a: list<T>, b: list<U>) -> list<(T,U)>\nZip two lists into list of tuples",
        "list_enumerate"     => "def list_enumerate(xs: list<T>) -> list<(i64,T)>\nPair each element with its index",
        "list_flatten"       => "def list_flatten(xs: list<list<T>>) -> list<T>\nFlatten nested list one level",
        "list_unique"        => "def list_unique(xs: list<T>) -> list<T>\nRemove duplicate elements",
        "list_reverse"       => "def list_reverse(xs: list<T>) -> list<T>\nReturn reversed copy of list",
        "list_sorted"        => "def list_sorted(xs: list<T>) -> list<T>\nReturn sorted copy of list",
        "list_sum"           => "def list_sum(xs: list<num>) -> f64\nSum all numeric elements",
        "list_min"           => "def list_min(xs: list<T>) -> T\nReturn minimum element",
        "list_max"           => "def list_max(xs: list<T>) -> T\nReturn maximum element",
        "list_index_of"      => "def list_index_of(xs: list<T>, val: T) -> i64\nIndex of first occurrence, or -1",
        "list_count"         => "def list_count(xs: list<T>, val: T) -> i64\nCount occurrences of value",
        "list_take"          => "def list_take(xs: list<T>, n: i64) -> list<T>\nTake first n elements",
        "list_drop"          => "def list_drop(xs: list<T>, n: i64) -> list<T>\nDrop first n elements",
        _ => return None,
    };
    Some(format!("```iris\n{}\n```", sig.split_once('\n').map(|(s, _)| s).unwrap_or(sig)).to_owned()
        + &sig.split_once('\n').map(|(_, d)| format!("\n\n{}", d)).unwrap_or_default())
}

/// Returns hover info for a keyword.
fn keyword_hover(name: &str) -> Option<String> {
    let info = match name {
        "def"      => "**def** — Define a function",
        "pub"      => "**pub** — Export a function or record for use from other files",
        "val"      => "**val** — Immutable binding (cannot be reassigned)",
        "var"      => "**var** — Mutable binding (can be reassigned)",
        "if"       => "**if** — Conditional expression: `if cond { ... } else { ... }`",
        "else"     => "**else** — Alternative branch of an if expression",
        "while"    => "**while** — Loop while condition is true",
        "for"      => "**for** — Range loop: `for i in start..end { ... }`",
        "loop"     => "**loop** — Infinite loop (break to exit)",
        "break"    => "**break** — Exit the innermost loop",
        "continue" => "**continue** — Skip to the next loop iteration",
        "return"   => "**return** — Early return from a function",
        "when"     => "**when** — Pattern match expression",
        "record"   => "**record** — Define a struct type with named fields",
        "choice"   => "**choice** — Define an enum type with variants",
        "const"    => "**const** — Compile-time constant",
        "type"     => "**type** — Type alias",
        "extern"   => "**extern** — Declare an external C function",
        "bring"    => "**bring** — Import a module: `bring std.math`",
        "trait"    => "**trait** — Define a trait (interface)",
        "impl"     => "**impl** — Implement a trait for a type",
        "spawn"    => "**spawn** — Launch a concurrent task",
        "par"      => "**par** — Parallel execution: `par for i in 0..n { ... }`",
        "async"    => "**async** — Mark a function as asynchronous",
        "await"    => "**await** — Wait for an async expression to complete",
        "true"     => "```iris\ntrue: bool\n```",
        "false"    => "```iris\nfalse: bool\n```",
        "none"     => "```iris\nnone: option<T>\n```\nAbsent value",
        _ => return None,
    };
    Some(info.to_owned())
}

// ---------------------------------------------------------------------------
// Simple formatter
// ---------------------------------------------------------------------------

/// Token-stream based IRIS formatter. Normalises indentation and spacing.
fn format_iris(source: &str) -> String {
    use crate::parser::lexer::{Lexer, Token};

    let spanned_tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(_) => return source.to_owned(),
    };

    let mut out = String::with_capacity(source.len() + 64);
    let mut indent = 0usize;
    let mut at_line_start = true;
    let mut prev_was_newline = false;
    let mut blank_lines = 0usize;

    // Helper: emit current indentation.
    let indent_str = |depth: usize| "    ".repeat(depth);

    // Top-level item starters that get a blank line before them.
    let is_top_level_kw = |t: &Token| matches!(
        t,
        Token::Def | Token::Record | Token::Choice | Token::Const | Token::Type
        | Token::Extern | Token::Trait | Token::Impl | Token::Pub
    );

    for (idx, spanned) in spanned_tokens.iter().enumerate() {
        let tok = &spanned.node;
        let tok_str = token_to_str(tok, source);
        if tok_str.is_empty() { continue; }

        // Emit blank line before top-level keywords (except at very start).
        if is_top_level_kw(tok) && indent == 0 && !out.is_empty() && blank_lines == 0 && !at_line_start {
            out.push('\n');
            blank_lines = 1;
        }

        // Newlines and indentation.
        if at_line_start {
            let ind = indent_str(indent);
            out.push_str(&ind);
            at_line_start = false;
        }

        // Opening brace: emit, then newline + increase indent.
        if tok_str == "{" {
            // Space before `{` if not at line start.
            if !out.ends_with(' ') && !out.ends_with('\n') {
                out.push(' ');
            }
            out.push('{');
            indent += 1;
            out.push('\n');
            at_line_start = true;
            blank_lines = 0;
            prev_was_newline = true;
            continue;
        }

        // Closing brace: decrease indent, then emit on its own line.
        if tok_str == "}" {
            if indent > 0 { indent -= 1; }
            if !out.ends_with('\n') { out.push('\n'); }
            out.push_str(&indent_str(indent));
            out.push('}');
            out.push('\n');
            at_line_start = true;
            blank_lines = 0;
            prev_was_newline = true;
            continue;
        }

        // Semicolons and angle brackets for generics — pass through.
        if tok_str == ";" {
            out.push(';');
            out.push('\n');
            at_line_start = true;
            blank_lines = 0;
            prev_was_newline = false;
            continue;
        }

        // Commas — no leading space, one trailing space.
        if tok_str == "," {
            // Remove trailing space before comma.
            if out.ends_with(' ') { out.pop(); }
            out.push(',');
            out.push(' ');
            prev_was_newline = false;
            continue;
        }

        // Operators that need surrounding spaces.
        let needs_space = matches!(tok_str.as_str(),
            "=" | "==" | "!=" | "<=" | ">=" |
            "+" | "-" | "*" | "/" | "%" | "&&" | "||" |
            "->" | "=>" | ".." | "..=" | ":" | "to"
        );

        if needs_space {
            if !out.ends_with(' ') && !out.ends_with('\n') { out.push(' '); }
            out.push_str(&tok_str);
            out.push(' ');
        } else if tok_str == "(" || tok_str == "[" || tok_str == "<" {
            // No space before open paren/bracket (function calls, indexing, generics).
            out.push_str(&tok_str);
        } else if tok_str == ")" || tok_str == "]" || tok_str == ">" {
            if out.ends_with(' ') { out.pop(); }
            out.push_str(&tok_str);
        } else {
            // Default: keyword or identifier — space between tokens unless at line start.
            let last = out.chars().last();
            let needs_sep = matches!(last, Some(c) if c.is_alphanumeric() || c == '_' || c == '"');
            if needs_sep && !tok_str.starts_with(|c: char| c == '.' || c == '(' || c == '[') {
                out.push(' ');
            }
            out.push_str(&tok_str);
        }

        let _ = (idx, prev_was_newline, spanned.span); // suppress unused warnings
        prev_was_newline = false;
        blank_lines = 0;
    }

    if !out.ends_with('\n') { out.push('\n'); }
    out
}

/// Returns the source text for a token (for formatting).
fn token_to_str(tok: &crate::parser::lexer::Token, _source: &str) -> String {
    use crate::parser::lexer::Token;
    match tok {
        Token::Def => "def".into(),
        Token::Val => "val".into(),
        Token::Var => "var".into(),
        Token::If => "if".into(),
        Token::Else => "else".into(),
        Token::When => "when".into(),
        Token::For => "for".into(),
        Token::While => "while".into(),
        Token::Loop => "loop".into(),
        Token::Break => "break".into(),
        Token::Continue => "continue".into(),
        Token::Return => "return".into(),
        Token::Record => "record".into(),
        Token::Choice => "choice".into(),
        Token::Const => "const".into(),
        Token::Type => "type".into(),
        Token::Extern => "extern".into(),
        Token::Trait => "trait".into(),
        Token::Impl => "impl".into(),
        Token::Pub => "pub".into(),
        Token::Bring => "bring".into(),
        Token::Async => "async".into(),
        Token::Await => "await".into(),
        Token::Spawn => "spawn".into(),
        Token::Par => "par".into(),
        Token::In => "in".into(),
        Token::To => "to".into(),
        Token::BoolLit(b) => if *b { "true".into() } else { "false".into() },
        // Type keywords
        Token::I64 => "i64".into(),
        Token::I32 => "i32".into(),
        Token::F64 => "f64".into(),
        Token::F32 => "f32".into(),
        Token::Bool => "bool".into(),
        Token::Str => "str".into(),
        Token::Tensor => "tensor".into(),
        Token::LBrace => "{".into(),
        Token::RBrace => "}".into(),
        Token::LParen => "(".into(),
        Token::RParen => ")".into(),
        Token::LBracket => "[".into(),
        Token::RBracket => "]".into(),
        Token::LAngle => "<".into(),
        Token::RAngle => ">".into(),
        Token::Comma => ",".into(),
        Token::Semi => ";".into(),
        Token::Colon => ":".into(),
        Token::Dot => ".".into(),
        Token::DotDot => "..".into(),
        Token::DotDotEq => "..=".into(),
        Token::Arrow => "->".into(),
        Token::FatArrow => "=>".into(),
        Token::Eq => "=".into(),
        Token::EqEq => "==".into(),
        Token::NotEq => "!=".into(),
        Token::LtEq => "<=".into(),
        Token::GtEq => ">=".into(),
        Token::Plus => "+".into(),
        Token::Minus => "-".into(),
        Token::Star => "*".into(),
        Token::Slash => "/".into(),
        Token::Percent => "%".into(),
        Token::Pipe => "|".into(),
        Token::AmpAmp => "&&".into(),
        Token::PipePipe => "||".into(),
        Token::Bang => "!".into(),
        Token::At => "@".into(),
        Token::Question => "?".into(),
        Token::Ident(s) => s.clone(),
        Token::IntLit(n) => n.to_string(),
        Token::FloatLit(f) => {
            if f.fract() == 0.0 { format!("{:.1}", f) } else { f.to_string() }
        }
        Token::StringLit(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Token::FStringLit(s) => format!("f\"{}\"", s),
        Token::Eof => String::new(),
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// LSP protocol server (JSON-RPC over stdin/stdout)
// ---------------------------------------------------------------------------

/// Runs the LSP server, reading JSON-RPC messages from stdin and writing
/// responses to stdout. Blocks until the client sends `exit`.
pub fn run_lsp_server() -> std::io::Result<()> {
    use std::io::Read;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut state = LspState::new();
    #[allow(unused_assignments)]
    let mut request_id: Option<serde_json::Value> = None;

    loop {
        // Read Content-Length header.
        let mut content_length: usize = 0;
        loop {
            let mut byte = [0u8];
            let mut chars = String::new();
            loop {
                stdin.lock().read_exact(&mut byte)?;
                if byte[0] == b'\r' { continue; }
                if byte[0] == b'\n' { break; }
                chars.push(byte[0] as char);
            }
            if chars.is_empty() { break; }
            if chars.to_lowercase().starts_with("content-length:") {
                let val = chars["content-length:".len()..].trim();
                content_length = val.parse().unwrap_or(0);
            }
        }
        if content_length == 0 { continue; }

        // Read body.
        let mut body = vec![0u8; content_length];
        stdin.lock().read_exact(&mut body)?;
        let body_str = String::from_utf8_lossy(&body);

        let msg: serde_json::Value = match serde_json::from_str(&body_str) {
            Ok(v) => v,
            Err(_) => continue,
        };

        request_id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(serde_json::Value::Null);

        match method {
            "initialize" => {
                let resp = make_response(request_id.clone(), serde_json::json!({
                    "capabilities": {
                        "textDocumentSync": 1,
                        "hoverProvider": true,
                        "completionProvider": { "triggerCharacters": [".", ":"] },
                        "definitionProvider": true,
                        "documentSymbolProvider": true,
                        "signatureHelpProvider": { "triggerCharacters": ["(", ","] },
                        "documentFormattingProvider": true,
                        "codeActionProvider": {
                            "codeActionKinds": ["quickfix", "refactor.extract"]
                        },
                        "inlayHintProvider": true,
                        "referencesProvider": true,
                        "renameProvider": {
                            "prepareProvider": false
                        }
                    },
                    "serverInfo": { "name": "iris-lsp", "version": "0.2.0" }
                }));
                write_message(&mut stdout.lock(), &resp)?;
            }
            "initialized" => { /* no-op */ }
            "textDocument/didOpen" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let text = params["textDocument"]["text"].as_str().unwrap_or("").to_owned();
                let diags = state.open_document(&uri, &text);
                let notif = make_diagnostics_notification(&uri, &diags);
                write_message(&mut stdout.lock(), &notif)?;
            }
            "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let text = params["contentChanges"][0]["text"].as_str()
                    .or_else(|| params["contentChanges"].as_array()
                        .and_then(|a| a.first())
                        .and_then(|c| c["text"].as_str()))
                    .unwrap_or("").to_owned();
                let diags = state.update_document(&uri, &text);
                let notif = make_diagnostics_notification(&uri, &diags);
                write_message(&mut stdout.lock(), &notif)?;
            }
            "textDocument/didSave" => { /* diagnostics already sent on change */ }
            "textDocument/didClose" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                state.close_document(uri);
            }
            "textDocument/hover" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
                let character = params["position"]["character"].as_u64().unwrap_or(0) as u32;
                let hover_text = state.hover(&uri, line, character);
                let result = hover_text.map(|t| serde_json::json!({
                    "contents": { "kind": "markdown", "value": t }
                })).unwrap_or(serde_json::Value::Null);
                write_message(&mut stdout.lock(), &make_response(request_id.clone(), result))?;
            }
            "textDocument/completion" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let items: Vec<serde_json::Value> = state.completions(&uri).into_iter()
                    .map(|label| serde_json::json!({ "label": label, "kind": 1 }))
                    .collect();
                write_message(&mut stdout.lock(), &make_response(
                    request_id.clone(),
                    serde_json::json!({ "isIncomplete": false, "items": items }),
                ))?;
            }
            "textDocument/definition" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
                let character = params["position"]["character"].as_u64().unwrap_or(0) as u32;
                let result = state.definition(&uri, line, character)
                    .map(|(def_uri, sl, sc, el, ec)| serde_json::json!({
                        "uri": def_uri,
                        "range": {
                            "start": { "line": sl, "character": sc },
                            "end":   { "line": el, "character": ec }
                        }
                    }))
                    .unwrap_or(serde_json::Value::Null);
                write_message(&mut stdout.lock(), &make_response(request_id.clone(), result))?;
            }
            "textDocument/documentSymbol" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let syms: Vec<serde_json::Value> = state.document_symbols(&uri).into_iter()
                    .map(|(name, kind, sl, sc, el, ec)| serde_json::json!({
                        "name": name,
                        "kind": kind,
                        "range": {
                            "start": { "line": sl, "character": sc },
                            "end":   { "line": el, "character": ec }
                        },
                        "selectionRange": {
                            "start": { "line": sl, "character": sc },
                            "end":   { "line": sl, "character": sc + name.len() as u32 }
                        }
                    }))
                    .collect();
                write_message(&mut stdout.lock(), &make_response(
                    request_id.clone(),
                    serde_json::json!(syms),
                ))?;
            }
            "textDocument/signatureHelp" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
                let character = params["position"]["character"].as_u64().unwrap_or(0) as u32;
                let result = state.signature_help(&uri, line, character)
                    .map(|(label, param_labels, active)| serde_json::json!({
                        "signatures": [{
                            "label": label,
                            "parameters": param_labels.iter().map(|p| {
                                serde_json::json!({ "label": p })
                            }).collect::<Vec<_>>()
                        }],
                        "activeSignature": 0,
                        "activeParameter": active
                    }))
                    .unwrap_or(serde_json::Value::Null);
                write_message(&mut stdout.lock(), &make_response(request_id.clone(), result))?;
            }
            "textDocument/formatting" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let source = state.documents.get(&uri).cloned().unwrap_or_default();
                let result = state.format(&uri)
                    .map(|formatted| {
                        let line_count = source.lines().count() as u32;
                        let last_line_len = source.lines().last().map(|l| l.len()).unwrap_or(0) as u32;
                        serde_json::json!([{
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end":   { "line": line_count, "character": last_line_len }
                            },
                            "newText": formatted
                        }])
                    })
                    .unwrap_or(serde_json::json!([]));
                write_message(&mut stdout.lock(), &make_response(request_id.clone(), result))?;
            }
            "textDocument/codeAction" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let range = &params["range"];
                let sl = range["start"]["line"].as_u64().unwrap_or(0) as u32;
                let sc = range["start"]["character"].as_u64().unwrap_or(0) as u32;
                let el = range["end"]["line"].as_u64().unwrap_or(0) as u32;
                let ec = range["end"]["character"].as_u64().unwrap_or(0) as u32;
                let actions = state.code_actions(&uri, sl, sc, el, ec);
                let result: Vec<serde_json::Value> = actions.into_iter().map(|a| {
                    let mut action = serde_json::json!({
                        "title": a.title,
                        "kind": a.kind,
                        "edit": {
                            "changes": {
                                &a.edit_uri: [{
                                    "range": {
                                        "start": { "line": a.edit_range.0, "character": a.edit_range.1 },
                                        "end":   { "line": a.edit_range.2, "character": a.edit_range.3 }
                                    },
                                    "newText": a.new_text
                                }]
                            }
                        }
                    });
                    if let Some(msg) = &a.diagnostic_message {
                        action["diagnostics"] = serde_json::json!([{
                            "message": msg,
                            "source": "iris"
                        }]);
                    }
                    action
                }).collect();
                write_message(&mut stdout.lock(), &make_response(
                    request_id.clone(),
                    serde_json::json!(result),
                ))?;
            }
            "textDocument/inlayHint" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let hints = state.inlay_hints(&uri);
                let result: Vec<serde_json::Value> = hints.into_iter().map(|h| {
                    serde_json::json!({
                        "position": { "line": h.line, "character": h.character },
                        "label": h.label,
                        "kind": h.kind,
                        "paddingLeft": true,
                    })
                }).collect();
                write_message(&mut stdout.lock(), &make_response(
                    request_id.clone(),
                    serde_json::json!(result),
                ))?;
            }
            "textDocument/references" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
                let character = params["position"]["character"].as_u64().unwrap_or(0) as u32;
                let refs = state.references(&uri, line, character);
                let result: Vec<serde_json::Value> = refs.into_iter().map(|(sl, sc, el, ec)| {
                    serde_json::json!({
                        "uri": &uri,
                        "range": {
                            "start": { "line": sl, "character": sc },
                            "end":   { "line": el, "character": ec }
                        }
                    })
                }).collect();
                write_message(&mut stdout.lock(), &make_response(
                    request_id.clone(),
                    serde_json::json!(result),
                ))?;
            }
            "textDocument/rename" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
                let character = params["position"]["character"].as_u64().unwrap_or(0) as u32;
                let new_name = params["newName"].as_str().unwrap_or("").to_owned();
                let edits = state.rename(&uri, line, character, &new_name);
                let text_edits: Vec<serde_json::Value> = edits.into_iter().map(|(sl, sc, el, ec, text)| {
                    serde_json::json!({
                        "range": {
                            "start": { "line": sl, "character": sc },
                            "end":   { "line": el, "character": ec }
                        },
                        "newText": text
                    })
                }).collect();
                let result = serde_json::json!({
                    "changes": { &uri: text_edits }
                });
                write_message(&mut stdout.lock(), &make_response(request_id.clone(), result))?;
            }
            "shutdown" => {
                write_message(&mut stdout.lock(), &make_response(request_id.clone(), serde_json::Value::Null))?;
            }
            "exit" => break,
            _ => {
                if request_id.is_some() {
                    write_message(&mut stdout.lock(), &make_response(request_id.clone(), serde_json::Value::Null))?;
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn uri_to_module_name(uri: &str) -> String {
    uri.rsplit('/').next()
        .and_then(|f| f.split('.').next())
        .unwrap_or("module")
        .to_owned()
}

/// Convert a `file://` URI to a filesystem `PathBuf`.
/// Returns `None` for non-file URIs (e.g. `untitled:`).
fn uri_to_file_path(uri: &str) -> Option<std::path::PathBuf> {
    // file:///C%3A/Users/… or file:///home/…
    let stripped = uri.strip_prefix("file:///")?;
    // Percent-decode the path.
    let decoded: String = percent_decode(stripped);
    // On Windows the path looks like "C:/Users/…"; on Unix "/home/…".
    // std::path::PathBuf handles both forms.
    let path = std::path::PathBuf::from(&decoded);
    if path.exists() { Some(path) } else { None }
}

/// Minimal %-decode (covers the most common LSP URI escapes).
fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                out.push(byte as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn line_col_to_byte(source: &str, line: u32, character: u32) -> u32 {
    let mut cur_line = 0u32;
    let mut cur_col = 0u32;
    for (i, ch) in source.char_indices() {
        if cur_line == line && cur_col == character {
            return i as u32;
        }
        if ch == '\n' {
            cur_line += 1;
            cur_col = 0;
        } else {
            cur_col += 1;
        }
    }
    source.len() as u32
}

fn ident_at_byte(source: &str, byte: u32) -> Option<&str> {
    let src = source;
    let byte = byte as usize;
    if byte >= src.len() { return None; }
    let start = src[..byte].rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let end = src[byte..].find(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| byte + i)
        .unwrap_or(src.len());
    if start < end { Some(&src[start..end]) } else { None }
}

fn make_response(id: Option<serde_json::Value>, result: serde_json::Value) -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })).unwrap_or_default()
}

fn make_diagnostics_notification(uri: &str, diags: &[LspDiagnostic]) -> String {
    let json_diags: Vec<serde_json::Value> = diags.iter().map(|d| {
        let mut diag = serde_json::json!({
            "range": {
                "start": { "line": d.line, "character": d.character },
                "end":   { "line": d.end_line, "character": d.end_character },
            },
            "severity": d.severity,
            "message": d.message,
            "source": "iris",
        });
        if let Some(code) = &d.code {
            diag["code"] = serde_json::json!(code);
        }
        diag
    }).collect();
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": { "uri": uri, "diagnostics": json_diags },
    })).unwrap_or_default()
}

fn write_message(writer: &mut impl std::io::Write, body: &str) -> std::io::Result<()> {
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    writer.flush()
}

// ---------------------------------------------------------------------------
// Code action helpers
// ---------------------------------------------------------------------------

/// Extracts a single-quoted name from an error message like "cannot find 'foo'".
fn extract_quoted_name(msg: &str) -> Option<String> {
    let start = msg.find('\'')?;
    let rest = &msg[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_owned())
}

/// Suggests a `bring` statement if `name` matches a known stdlib function or module.
fn suggest_bring_for_name(name: &str) -> Option<String> {
    // Map well-known function names to their stdlib modules.
    let stdlib_map: &[(&str, &str)] = &[
        // std.math
        ("sqrt", "std.math"), ("abs", "std.math"), ("sin", "std.math"),
        ("cos", "std.math"), ("tan", "std.math"), ("log", "std.math"),
        ("exp", "std.math"), ("pow", "std.math"), ("ceil", "std.math"),
        ("floor", "std.math"), ("round", "std.math"), ("min", "std.math"),
        ("max", "std.math"), ("pi", "std.math"),
        // std.string
        ("split", "std.string"), ("join", "std.string"), ("trim", "std.string"),
        ("replace", "std.string"), ("contains", "std.string"), ("starts_with", "std.string"),
        ("ends_with", "std.string"), ("to_upper", "std.string"), ("to_lower", "std.string"),
        ("len", "std.string"), ("substring", "std.string"), ("char_at", "std.string"),
        // std.fmt
        ("format", "std.fmt"), ("to_string", "std.fmt"), ("println", "std.fmt"),
        // std.json
        ("parse_json", "std.json"), ("to_json", "std.json"),
        // std.fs
        ("read_file", "std.fs"), ("write_file", "std.fs"), ("file_exists", "std.fs"),
        // std.time
        ("now", "std.time"), ("sleep", "std.time"),
        // std.http
        ("http_get", "std.http"), ("http_post", "std.http"),
        // std.kv
        ("kv_set", "std.kv"), ("kv_get", "std.kv"), ("kv_delete", "std.kv"),
        // std.csv
        ("parse_csv", "std.csv"), ("to_csv", "std.csv"),
        // std.set
        ("set_new", "std.set"), ("set_add", "std.set"), ("set_contains", "std.set"),
        // std.path
        ("path_join", "std.path"), ("path_parent", "std.path"), ("path_ext", "std.path"),
        // std.ffi
        ("ffi_open", "std.ffi"), ("ffi_call", "std.ffi"), ("ffi_close", "std.ffi"),
        ("ffi_call_i64", "std.ffi"), ("ffi_call_f64", "std.ffi"), ("ffi_call_str", "std.ffi"),
        ("python_eval", "std.ffi"), ("python_exec", "std.ffi"), ("python_call", "std.ffi"),
        ("python_version", "std.ffi"), ("rust_lib_open", "std.ffi"),
        ("rust_call_i64", "std.ffi"), ("rust_call_f64", "std.ffi"),
        // std.crypto
        ("uuid", "std.crypto"), ("sha256", "std.crypto"), ("hex_encode", "std.crypto"),
        ("hex_decode", "std.crypto"), ("hash_code", "std.crypto"),
        ("to_base64", "std.crypto"), ("from_base64", "std.crypto"),
        // std.os
        ("getenv", "std.os"), ("setenv", "std.os"), ("get_pid", "std.os"),
        ("shell", "std.os"), ("getcwd", "std.os"), ("readdir", "std.os"),
        ("exists", "std.os"), ("make_dir", "std.os"), ("cpu_count", "std.os"),
        // std.testing
        ("assert_eq", "std.testing"), ("assert_true", "std.testing"),
        ("assert_false", "std.testing"), ("assert_str_eq", "std.testing"),
        // std.log
        ("info", "std.log"), ("warn", "std.log"), ("error", "std.log"), ("debug", "std.log"),
    ];

    for (func, module) in stdlib_map {
        if name == *func {
            return Some(format!("bring {}", module));
        }
    }
    None
}

/// Checks if a byte value is a valid identifier character.
fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Checks if a word is a language keyword.
fn is_keyword(word: &str) -> bool {
    matches!(word,
        "def" | "val" | "var" | "for" | "while" | "loop" | "if" | "else" | "when"
        | "return" | "break" | "continue" | "bring" | "record" | "choice" | "const"
        | "type" | "extern" | "trait" | "impl" | "pub" | "in" | "to" | "true" | "false"
        | "async" | "await" | "spawn" | "par"
        | "i64" | "i32" | "f64" | "f32" | "bool" | "str" | "u8" | "i8" | "u32" | "u64" | "usize"
    )
}

/// Checks if a name follows snake_case convention.
fn is_snake_case(name: &str) -> bool {
    if name.is_empty() || name.starts_with('_') { return true; }
    name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Converts a camelCase or PascalCase name to snake_case.
fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 { result.push('_'); }
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

/// Extracts a type name from a type mismatch error ("expected i64, found f64").
fn extract_type_from_mismatch(msg: &str, keyword: &str) -> Option<String> {
    let lower = msg.to_lowercase();
    let idx = lower.find(keyword)?;
    let rest = &msg[idx + keyword.len()..];
    let rest = rest.trim_start_matches(|c: char| c == ' ' || c == ':' || c == '`' || c == '\'');
    let end = rest.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '<' && c != '>').unwrap_or(rest.len());
    let ty = rest[..end].trim();
    if ty.is_empty() { None } else { Some(ty.to_owned()) }
}

/// Checks if a type name is a numeric type (i32, i64, f32, f64).
fn is_numeric_type(ty: &str) -> bool {
    matches!(ty, "i32" | "i64" | "f32" | "f64" | "u8" | "u32" | "u64" | "usize")
}
