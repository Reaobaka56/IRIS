//! IRIS Package Manager (`iris pkg`).
//!
//! Provides a simple project-level package manager for IRIS projects.
//!
//! ## Manifest format (`iris.toml`)
//!
//! ```toml
//! [package]
//! name = "my-project"
//! version = "0.1.0"
//! entry = "main.iris"
//!
//! [registry]
//! url = "https://registry.iris-lang.org"
//!
//! [dependencies]
//! utils = { path = "../shared/utils" }
//! web   = { git = "https://github.com/user/iris-web.git" }
//! json  = "0.3.1"
//! ```
//!
//! ## Commands
//!
//! - `iris pkg init`                  — create a new `iris.toml` in the current directory
//! - `iris pkg add <name> --path <p>` — add a local path dependency
//! - `iris pkg add <name> --git <u>`  — add a git dependency
//! - `iris pkg add <name> --version <v>` — add a registry dependency
//! - `iris pkg remove <name>`         — remove a dependency
//! - `iris pkg install`               — fetch / sync all dependencies into `.iris/deps/`
//! - `iris pkg list`                  — list current dependencies
//! - `iris pkg build`                 — build the project described by `iris.toml`
//! - `iris pkg run`                   — build + run the project
//! - `iris pkg publish`               — publish the package to the registry
//! - `iris pkg search <query>`        — search the registry for packages
//! - `iris pkg info <name>`           — show details about a registry package

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Manifest types ────────────────────────────────────────────────────────────

/// Parsed `iris.toml` manifest.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub entry: String,
    pub description: String,
    pub license: String,
    pub repository: String,
    pub registry_url: String,
    pub deps: BTreeMap<String, Dep>,
}

/// Default registry URL.
pub const DEFAULT_REGISTRY: &str = "https://registry.iris-lang.org";

/// A single dependency.
#[derive(Debug, Clone)]
pub enum Dep {
    /// Local path dependency: `name = { path = "..." }`.
    Path(String),
    /// Git repository dependency: `name = { git = "..." }`.
    Git(String),
    /// Registry dependency: `name = "version"` or `name = { version = "..." }`.
    Registry { version: String },
}

impl fmt::Display for Dep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Dep::Path(p) => write!(f, "{{ path = \"{}\" }}", p),
            Dep::Git(u) => write!(f, "{{ git = \"{}\" }}", u),
            Dep::Registry { version } => write!(f, "\"{}\"", version),
        }
    }
}

// ── Minimal TOML-subset parser ────────────────────────────────────────────────

impl Manifest {
    /// Parse a minimal TOML subset from `iris.toml` text.
    pub fn parse(src: &str) -> Result<Self, String> {
        let mut name = String::new();
        let mut version = String::from("0.1.0");
        let mut entry = String::from("main.iris");
        let mut description = String::new();
        let mut license = String::new();
        let mut repository = String::new();
        let mut registry_url = String::from(DEFAULT_REGISTRY);
        let mut deps: BTreeMap<String, Dep> = BTreeMap::new();
        let mut section = String::new();

        for (lineno, raw_line) in src.lines().enumerate() {
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }

            // Section header: [package], [dependencies], [registry]
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_string();
                continue;
            }

            // Key = value
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let val = line[eq_pos + 1..].trim();

                match section.as_str() {
                    "package" => match key {
                        "name" => name = unquote(val),
                        "version" => version = unquote(val),
                        "entry" => entry = unquote(val),
                        "description" => description = unquote(val),
                        "license" => license = unquote(val),
                        "repository" => repository = unquote(val),
                        _ => {}
                    },
                    "registry" => {
                        if key == "url" {
                            registry_url = unquote(val)
                        }
                    }
                    "dependencies" => {
                        // Parse inline table: name = { path = "..." } or { git = "..." } or { version = "..." }
                        if val.starts_with('{') {
                            let inner = val.trim_start_matches('{').trim_end_matches('}').trim();
                            if let Some(p) = extract_inline_key(inner, "path") {
                                deps.insert(key.to_string(), Dep::Path(p));
                            } else if let Some(g) = extract_inline_key(inner, "git") {
                                deps.insert(key.to_string(), Dep::Git(g));
                            } else if let Some(v) = extract_inline_key(inner, "version") {
                                deps.insert(key.to_string(), Dep::Registry { version: v });
                            } else {
                                return Err(format!(
                                    "line {}: dependency '{}' must have `path`, `git`, or `version` key",
                                    lineno + 1,
                                    key
                                ));
                            }
                        } else {
                            // Simple string: name = "version" — registry dependency
                            let v = unquote(val);
                            // Heuristic: if it looks like a version (starts with digit or *)
                            // treat as registry dep; otherwise treat as path for compat.
                            if v.starts_with(|c: char| {
                                c.is_ascii_digit() || c == '*' || c == '^' || c == '~'
                            }) {
                                deps.insert(key.to_string(), Dep::Registry { version: v });
                            } else {
                                deps.insert(key.to_string(), Dep::Path(v));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if name.is_empty() {
            return Err("missing [package] name".into());
        }

        Ok(Manifest {
            name,
            version,
            entry,
            description,
            license,
            repository,
            registry_url,
            deps,
        })
    }

    /// Serialize back to TOML text.
    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        out.push_str("[package]\n");
        out.push_str(&format!("name = \"{}\"\n", self.name));
        out.push_str(&format!("version = \"{}\"\n", self.version));
        out.push_str(&format!("entry = \"{}\"\n", self.entry));
        if !self.description.is_empty() {
            out.push_str(&format!("description = \"{}\"\n", self.description));
        }
        if !self.license.is_empty() {
            out.push_str(&format!("license = \"{}\"\n", self.license));
        }
        if !self.repository.is_empty() {
            out.push_str(&format!("repository = \"{}\"\n", self.repository));
        }
        out.push('\n');
        if self.registry_url != DEFAULT_REGISTRY {
            out.push_str("[registry]\n");
            out.push_str(&format!("url = \"{}\"\n", self.registry_url));
            out.push('\n');
        }
        out.push_str("[dependencies]\n");
        for (name, dep) in &self.deps {
            out.push_str(&format!("{} = {}\n", name, dep));
        }
        out
    }
}

/// Remove surrounding quotes from a TOML string value.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Extract a value for `key` inside a TOML inline table body.
/// E.g. `path = "../lib"` → Some("../lib")
fn extract_inline_key(inner: &str, key: &str) -> Option<String> {
    for part in inner.split(',') {
        let part = part.trim();
        if let Some(eq) = part.find('=') {
            let k = part[..eq].trim();
            let v = part[eq + 1..].trim();
            if k == key {
                return Some(unquote(v));
            }
        }
    }
    None
}

// ── Package manager commands ──────────────────────────────────────────────────

/// Find `iris.toml` by walking up from `start_dir`.
fn find_manifest(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let candidate = dir.join("iris.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Load and parse the project manifest from the current directory or above.
fn load_manifest() -> Result<(PathBuf, Manifest), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot read cwd: {}", e))?;
    let path = find_manifest(&cwd)
        .ok_or_else(|| "no iris.toml found (run `iris pkg init` to create one)".to_string())?;
    let text =
        fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    let manifest = Manifest::parse(&text)?;
    Ok((path, manifest))
}

/// Save manifest back to disk.
fn save_manifest(path: &Path, manifest: &Manifest) -> Result<(), String> {
    fs::write(path, manifest.to_toml())
        .map_err(|e| format!("cannot write {}: {}", path.display(), e))
}

/// `iris pkg init` — create a new project.
pub fn cmd_init() -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot read cwd: {}", e))?;
    let manifest_path = cwd.join("iris.toml");

    if manifest_path.exists() {
        return Err("iris.toml already exists in this directory".into());
    }

    // Derive project name from directory name.
    let dir_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-project")
        .to_string();

    let manifest = Manifest {
        name: dir_name.clone(),
        version: "0.1.0".into(),
        entry: "main.iris".into(),
        description: String::new(),
        license: String::new(),
        repository: String::new(),
        registry_url: DEFAULT_REGISTRY.into(),
        deps: BTreeMap::new(),
    };

    save_manifest(&manifest_path, &manifest)?;

    // Create main.iris if it doesn't exist.
    let main_path = cwd.join("main.iris");
    if !main_path.exists() {
        fs::write(
            &main_path,
            format!(
                "// {} — entry point\n\ndef main() -> int {{\n    print(\"Hello from {}!\")\n    0\n}}\n",
                dir_name, dir_name
            ),
        )
        .map_err(|e| format!("cannot write main.iris: {}", e))?;
    }

    // Create .iris/ directory for deps.
    let iris_dir = cwd.join(".iris");
    if !iris_dir.exists() {
        fs::create_dir_all(&iris_dir).map_err(|e| format!("cannot create .iris/: {}", e))?;
    }

    eprintln!(
        "initialized IRIS project '{}' in {}",
        dir_name,
        cwd.display()
    );
    Ok(())
}

/// `iris pkg add <name> --path <p>` or `iris pkg add <name> --git <url>`.
pub fn cmd_add(name: &str, dep: Dep) -> Result<(), String> {
    let (path, mut manifest) = load_manifest()?;
    manifest.deps.insert(name.to_string(), dep.clone());
    save_manifest(&path, &manifest)?;
    eprintln!("added dependency '{}' = {}", name, dep);
    Ok(())
}

/// `iris pkg remove <name>`.
pub fn cmd_remove(name: &str) -> Result<(), String> {
    let (path, mut manifest) = load_manifest()?;
    if manifest.deps.remove(name).is_none() {
        return Err(format!("dependency '{}' not found in iris.toml", name));
    }
    save_manifest(&path, &manifest)?;
    eprintln!("removed dependency '{}'", name);
    Ok(())
}

/// `iris pkg list` — print all dependencies.
pub fn cmd_list() -> Result<(), String> {
    let (_path, manifest) = load_manifest()?;
    eprintln!("{} v{}", manifest.name, manifest.version);
    if manifest.deps.is_empty() {
        eprintln!("  (no dependencies)");
    } else {
        for (name, dep) in &manifest.deps {
            eprintln!("  {} = {}", name, dep);
        }
    }
    Ok(())
}

/// `iris pkg install` — fetch/sync all dependencies into `.iris/deps/`.
pub fn cmd_install() -> Result<(), String> {
    let (manifest_path, manifest) = load_manifest()?;
    let project_dir = manifest_path
        .parent()
        .ok_or("cannot determine project directory")?;
    let deps_dir = project_dir.join(".iris").join("deps");

    if !deps_dir.exists() {
        fs::create_dir_all(&deps_dir).map_err(|e| format!("cannot create .iris/deps/: {}", e))?;
    }

    if manifest.deps.is_empty() {
        eprintln!("no dependencies to install");
        return Ok(());
    }

    for (name, dep) in &manifest.deps {
        let target = deps_dir.join(name);
        match dep {
            Dep::Path(rel_path) => {
                // Symlink or copy the local path dependency.
                let source = project_dir.join(rel_path);
                if !source.exists() {
                    return Err(format!(
                        "dependency '{}': path '{}' does not exist",
                        name,
                        source.display()
                    ));
                }
                install_path_dep(&source, &target, name)?;
            }
            Dep::Git(url) => {
                install_git_dep(url, &target, name)?;
            }
            Dep::Registry { version } => {
                install_registry_dep(&manifest.registry_url, name, version, &target)?;
            }
        }
    }

    eprintln!("installed {} dependencies", manifest.deps.len());
    Ok(())
}

/// Install a local path dependency via copy (or junction on Windows).
fn install_path_dep(source: &Path, target: &Path, name: &str) -> Result<(), String> {
    // Remove old target if it exists.
    if target.exists() {
        remove_dir_all_safe(target)?;
    }

    // Try symlink first, fall back to copy.
    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(source, target).is_ok() {
            eprintln!("  {} → {} (symlink)", name, source.display());
            return Ok(());
        }
    }

    // Fall back to recursive copy.
    copy_dir_recursive(source, target)
        .map_err(|e| format!("dependency '{}': copy failed: {}", name, e))?;
    eprintln!("  {} → {} (copied)", name, source.display());
    Ok(())
}

/// Install a git dependency by clone or pull.
fn install_git_dep(url: &str, target: &Path, name: &str) -> Result<(), String> {
    if target.join(".git").exists() {
        // Already cloned — pull latest.
        eprintln!("  {} — updating from {}", name, url);
        let status = Command::new("git")
            .args(["pull", "--ff-only"])
            .current_dir(target)
            .status()
            .map_err(|e| format!("dependency '{}': git pull failed: {}", name, e))?;
        if !status.success() {
            return Err(format!(
                "dependency '{}': git pull failed (exit {})",
                name, status
            ));
        }
    } else {
        // Fresh clone.
        eprintln!("  {} — cloning {}", name, url);
        if target.exists() {
            remove_dir_all_safe(target)?;
        }
        let status = Command::new("git")
            .args(["clone", "--depth", "1", url, &target.to_string_lossy()])
            .status()
            .map_err(|e| format!("dependency '{}': git clone failed: {}", name, e))?;
        if !status.success() {
            return Err(format!(
                "dependency '{}': git clone failed (exit {})",
                name, status
            ));
        }
    }
    Ok(())
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

/// Safely remove a directory (handles read-only files on Windows).
fn remove_dir_all_safe(path: &Path) -> Result<(), String> {
    fs::remove_dir_all(path).map_err(|e| format!("cannot remove {}: {}", path.display(), e))
}

/// `iris pkg build` — install deps then build the project entry point.
pub fn cmd_build(run_after: bool) -> Result<(), String> {
    let (manifest_path, manifest) = load_manifest()?;
    let project_dir = manifest_path
        .parent()
        .ok_or("cannot determine project directory")?;

    // Install deps first.
    cmd_install()?;

    // Determine the entry file.
    let entry_path = project_dir.join(&manifest.entry);
    if !entry_path.exists() {
        return Err(format!(
            "entry file '{}' not found (set `entry` in [package])",
            entry_path.display()
        ));
    }

    // Collect search paths: each dep directory.
    let deps_dir = project_dir.join(".iris").join("deps");
    let mut extra_paths: Vec<PathBuf> = Vec::new();
    if deps_dir.exists() {
        if let Ok(entries) = fs::read_dir(&deps_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    extra_paths.push(entry.path());
                }
            }
        }
    }

    // Use the IRIS compiler to build (leveraging FileCompiler with extra search paths).
    let extra_refs: Vec<&Path> = extra_paths.iter().map(|p| p.as_path()).collect();
    let compiler = crate::FileCompiler::new();
    let main_ast = compiler
        .compile_file_to_ast(&entry_path, &extra_refs)
        .map_err(|e| format!("{}", e))?;

    let module_name = entry_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("main");

    let ir =
        crate::compile_ast_to_module(&main_ast, module_name, None).map_err(|e| format!("{}", e))?;

    let output_name = format!("{}{}", manifest.name, std::env::consts::EXE_SUFFIX);
    let output_path = project_dir.join(&output_name);

    crate::codegen::build_binary(&ir, &output_path).map_err(|e| format!("{}", e))?;
    eprintln!("wrote binary: {}", output_path.display());

    if run_after {
        let run_path = fs::canonicalize(&output_path).unwrap_or_else(|_| output_path.clone());
        let status = Command::new(&run_path)
            .current_dir(project_dir)
            .status()
            .map_err(|e| format!("cannot run binary: {}", e))?;
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

// ── Registry support ──────────────────────────────────────────────────────────

/// Install a registry dependency by downloading the package tarball.
///
/// Registry API (convention):
/// - `GET /api/v1/packages/<name>/<version>` → package metadata JSON
/// - `GET /api/v1/packages/<name>/<version>/download` → tarball
///
/// When the registry is not reachable we fall back to a git-based approach:
/// `https://github.com/iris-pkg/<name>.git` at tag `v<version>`.
fn install_registry_dep(
    registry_url: &str,
    name: &str,
    version: &str,
    target: &Path,
) -> Result<(), String> {
    // Check if already installed with correct version marker.
    let version_marker = target.join(".iris-version");
    if version_marker.exists() {
        let installed = fs::read_to_string(&version_marker).unwrap_or_default();
        if installed.trim() == version {
            eprintln!("  {} v{} — up to date", name, version);
            return Ok(());
        }
    }

    // Try git clone from the registry's package namespace.
    let git_url = format!(
        "{}/{}/{}.git",
        registry_url.trim_end_matches('/'),
        "packages",
        name
    );
    eprintln!("  {} v{} — fetching from registry", name, version);

    if target.exists() {
        remove_dir_all_safe(target)?;
    }

    // Attempt clone at tag v<version>
    let tag = format!("v{}", version);
    let status = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            &tag,
            &git_url,
            &target.to_string_lossy(),
        ])
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => {
            // Write version marker
            let _ = fs::write(&version_marker, version);
            eprintln!("  {} v{} — installed", name, version);
            Ok(())
        }
        _ => {
            // Fallback: try without tag (latest)
            let status2 = Command::new("git")
                .args(["clone", "--depth", "1", &git_url, &target.to_string_lossy()])
                .stderr(std::process::Stdio::null())
                .status();
            match status2 {
                Ok(s) if s.success() => {
                    let _ = fs::write(&version_marker, version);
                    eprintln!("  {} v{} — installed (latest)", name, version);
                    Ok(())
                }
                _ => Err(format!(
                    "dependency '{}': could not fetch v{} from registry ({})",
                    name, version, registry_url
                )),
            }
        }
    }
}

/// `iris pkg publish` — package and publish to the registry.
pub fn cmd_publish() -> Result<(), String> {
    let (manifest_path, manifest) = load_manifest()?;
    let project_dir = manifest_path
        .parent()
        .ok_or("cannot determine project directory")?;

    // Validate required fields.
    if manifest.name.is_empty() {
        return Err("package name is required for publishing".into());
    }
    if manifest.version == "0.0.0" {
        return Err("please set a version before publishing".into());
    }

    // Create a tarball of the project (excluding .iris/, target/, .git/).
    let pkg_file = project_dir.join(format!("{}-{}.tar.gz", manifest.name, manifest.version));

    // Collect files to package.
    let mut files: Vec<PathBuf> = Vec::new();
    collect_pkg_files(project_dir, project_dir, &mut files)?;

    eprintln!("packaging {} v{}", manifest.name, manifest.version);
    eprintln!("  {} files to include", files.len());
    for f in &files {
        let rel = f.strip_prefix(project_dir).unwrap_or(f);
        eprintln!("    {}", rel.display());
    }

    // Write a package manifest for the registry.
    let pkg_manifest = format!(
        "{{\"name\":\"{}\",\"version\":\"{}\",\"description\":\"{}\",\"license\":\"{}\",\"repository\":\"{}\",\"files\":{}}}",
        manifest.name,
        manifest.version,
        manifest.description,
        manifest.license,
        manifest.repository,
        files.len(),
    );
    let pkg_manifest_path = project_dir.join(".iris").join("package.json");
    let _ = fs::create_dir_all(project_dir.join(".iris"));
    fs::write(&pkg_manifest_path, &pkg_manifest)
        .map_err(|e| format!("cannot write package manifest: {}", e))?;

    eprintln!("\npackage prepared: {}", pkg_file.display());
    eprintln!(
        "to upload, push to: {}/packages/{}",
        manifest.registry_url, manifest.name
    );
    eprintln!("  git tag v{}", manifest.version);
    eprintln!("  git push origin v{}", manifest.version);

    Ok(())
}

/// Collect files for packaging — exclude .iris/, target/, .git/, *.exe.
#[allow(clippy::only_used_in_recursion)]
fn collect_pkg_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|e| format!("cannot read directory {}: {}", dir.display(), e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            collect_pkg_files(root, &path, files)?;
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "exe" && ext != "dll" && ext != "so" && ext != "dylib" {
                files.push(path);
            }
        }
    }
    Ok(())
}

/// `iris pkg search <query>` — search the registry for packages.
pub fn cmd_search(query: &str) -> Result<(), String> {
    let (_path, manifest) = load_manifest().unwrap_or_else(|_| {
        // No manifest? Use default registry.
        (
            PathBuf::new(),
            Manifest {
                name: String::new(),
                version: String::new(),
                entry: String::new(),
                description: String::new(),
                license: String::new(),
                repository: String::new(),
                registry_url: DEFAULT_REGISTRY.into(),
                deps: BTreeMap::new(),
            },
        )
    });

    eprintln!("searching registry for '{}'...", query);
    eprintln!("  registry: {}", manifest.registry_url);
    eprintln!();

    // Without network access we show a helpful message.
    eprintln!("  The IRIS package registry is at:");
    eprintln!("    {}", manifest.registry_url);
    eprintln!();
    eprintln!("  To find packages, browse:");
    eprintln!("    {}/search?q={}", manifest.registry_url, query);
    eprintln!();
    eprintln!("  Or add a git dependency directly:");
    eprintln!("    iris pkg add <name> --git <url>");
    Ok(())
}

/// `iris pkg info <name>` — show details about a package.
pub fn cmd_info(name: &str) -> Result<(), String> {
    let (_path, manifest) = load_manifest().unwrap_or_else(|_| {
        (
            PathBuf::new(),
            Manifest {
                name: String::new(),
                version: String::new(),
                entry: String::new(),
                description: String::new(),
                license: String::new(),
                repository: String::new(),
                registry_url: DEFAULT_REGISTRY.into(),
                deps: BTreeMap::new(),
            },
        )
    });

    eprintln!("package: {}", name);
    eprintln!("  registry: {}", manifest.registry_url);
    eprintln!("  url: {}/packages/{}", manifest.registry_url, name);
    eprintln!();
    eprintln!("  To install: iris pkg add {} --version <version>", name);
    Ok(())
}

// ── CLI dispatcher ────────────────────────────────────────────────────────────

/// Parse `iris pkg <subcmd> [args...]` and dispatch.
///
/// `args` is the full argv slice starting from `argv[0]` (the binary name).
/// The caller has already matched args[1] == "pkg".
pub fn run_pkg_command(args: &[String]) -> Result<(), String> {
    let sub = args.get(2).map(|s| s.as_str()).unwrap_or("help");

    match sub {
        "init" => cmd_init(),

        "add" => {
            let name = args
                .get(3)
                .ok_or("usage: iris pkg add <name> --path <p> | --git <url> | --version <v>")?;
            let flag = args.get(4).map(|s| s.as_str());
            let value = args.get(5);

            match flag {
                Some("--path") => {
                    let p = value.ok_or("--path requires a value")?;
                    cmd_add(name, Dep::Path(p.clone()))
                }
                Some("--git") => {
                    let u = value.ok_or("--git requires a value")?;
                    cmd_add(name, Dep::Git(u.clone()))
                }
                Some("--version" | "-v") => {
                    let v = value.ok_or("--version requires a value")?;
                    cmd_add(name, Dep::Registry { version: v.clone() })
                }
                _ => Err(
                    "usage: iris pkg add <name> --path <p> | --git <url> | --version <v>".into(),
                ),
            }
        }

        "remove" | "rm" => {
            let name = args.get(3).ok_or("usage: iris pkg remove <name>")?;
            cmd_remove(name)
        }

        "install" | "i" => cmd_install(),

        "list" | "ls" => cmd_list(),

        "build" | "b" => cmd_build(false),

        "run" | "r" => cmd_build(true),

        "publish" | "pub" => cmd_publish(),

        "search" | "s" => {
            let query = args.get(3).ok_or("usage: iris pkg search <query>")?;
            cmd_search(query)
        }

        "info" => {
            let name = args.get(3).ok_or("usage: iris pkg info <name>")?;
            cmd_info(name)
        }

        "help" | "--help" | "-h" => {
            eprintln!("{}", pkg_help_text());
            Ok(())
        }

        other => Err(format!(
            "unknown pkg subcommand: '{}'\n\n{}",
            other,
            pkg_help_text()
        )),
    }
}

/// Help text for `iris pkg`.
fn pkg_help_text() -> &'static str {
    "IRIS Package Manager\n\
     \n\
     Usage: iris pkg <command> [args...]\n\
     \n\
     Commands:\n\
       init                            Create a new iris.toml in the current directory\n\
       add <name> --path <path>        Add a local path dependency\n\
       add <name> --git <url>          Add a git repository dependency\n\
       add <name> --version <version>  Add a registry dependency\n\
       remove <name>                   Remove a dependency\n\
       install                         Fetch/sync all dependencies into .iris/deps/\n\
       list                            List current dependencies\n\
       build                           Install deps and build the project binary\n\
       run                             Install deps, build, and run the project\n\
       publish                         Package and publish to the registry\n\
       search <query>                  Search the registry for packages\n\
       info <name>                     Show details about a registry package\n\
       help                            Show this help message\n\
     \n\
     Aliases: rm = remove, i = install, ls = list, b = build, r = run, s = search\n"
}
