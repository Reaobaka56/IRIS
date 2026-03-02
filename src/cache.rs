//! Incremental compilation cache.
//!
//! Caches parsed ASTs and compiled IR modules keyed by a SHA-256 content hash
//! of the source text. Artifacts are stored in `.iris/cache/` next to the
//! project root (or a global `~/.iris/cache/` fallback).
//!
//! ## Directory layout
//!
//! ```text
//! .iris/cache/
//!   <sha256_hex>.ast.json    — serialised AST (JSON)
//!   <sha256_hex>.ir.json     — serialised IrModule (JSON)
//!   manifest.json            — maps file paths → (hash, mtime)
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

// ---------------------------------------------------------------------------
// Content hashing (SHA-256, pure Rust — no external crate)
// ---------------------------------------------------------------------------

/// Computes a hex-encoded SHA-256 digest of `data` using a minimal pure-Rust
/// implementation. We avoid pulling in a crate for this single use.
pub fn sha256_hex(data: &[u8]) -> String {
    let hash = sha256(data);
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

/// SHA-256 (FIPS 180-4) — returns 32-byte digest.
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
        0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    // Pre-processing: pad the message
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // Process each 64-byte block
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut digest = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        digest[4 * i..4 * i + 4].copy_from_slice(&v.to_be_bytes());
    }
    digest
}

// ---------------------------------------------------------------------------
// Cache manifest — maps file paths → content hashes + modification times
// ---------------------------------------------------------------------------

/// Per-file metadata stored in the cache manifest.
#[derive(Debug, Clone)]
struct CacheEntry {
    /// SHA-256 hex digest of the source text.
    hash: String,
    /// Last-modified time (seconds since UNIX epoch) when the hash was computed.
    mtime_secs: u64,
    /// Size in bytes — quick pre-check before hashing.
    size: u64,
}

/// Persistent build cache.
///
/// The cache directory is located at `.iris/cache/` relative to the project
/// root (determined by walking up from the working directory looking for
/// `iris.toml` or `.iris/`).
pub struct BuildCache {
    /// Root of the cache directory (e.g. `.iris/cache/`).
    cache_dir: PathBuf,
    /// In-memory manifest: canonical path → entry.
    manifest: HashMap<PathBuf, CacheEntry>,
    /// Whether the cache is disabled (e.g. `--no-cache`).
    disabled: bool,
    /// Dirty flag — written back on `flush()`.
    dirty: bool,
}

impl BuildCache {
    // ------------------------------------------------------------------
    // Construction
    // ------------------------------------------------------------------

    /// Open (or create) the build cache rooted at the given project directory.
    pub fn open(project_dir: &Path) -> Self {
        let cache_dir = project_dir.join(".iris").join("cache");
        let manifest = Self::load_manifest(&cache_dir);
        Self { cache_dir, manifest, disabled: false, dirty: false }
    }

    /// A no-op cache that never stores or retrieves anything.
    pub fn disabled() -> Self {
        Self {
            cache_dir: PathBuf::new(),
            manifest: HashMap::new(),
            disabled: true,
            dirty: false,
        }
    }

    // ------------------------------------------------------------------
    // Source-level cache checking
    // ------------------------------------------------------------------

    /// Returns `true` if the source file at `path` is unchanged since its last
    /// cached compilation. Uses a two-tier check:
    /// 1. Quick: compare mtime + size against the manifest.
    /// 2. Full:  if mtime/size changed, re-hash and compare the SHA-256.
    pub fn is_fresh(&self, path: &Path, source: &str) -> bool {
        if self.disabled { return false; }
        let canon = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };
        if let Some(entry) = self.manifest.get(&canon) {
            // Quick check: if mtime+size haven't changed, trust the stored hash.
            if let Ok(meta) = fs::metadata(&canon) {
                let mtime = meta.modified().ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                if mtime == entry.mtime_secs && meta.len() == entry.size {
                    return true;
                }
            }
            // Full check: re-hash the source.
            sha256_hex(source.as_bytes()) == entry.hash
        } else {
            false
        }
    }

    /// Record that `path` with content `source` has been successfully compiled.
    pub fn mark_fresh(&mut self, path: &Path, source: &str) {
        if self.disabled { return; }
        let canon = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return,
        };
        let hash = sha256_hex(source.as_bytes());
        let (mtime_secs, size) = file_mtime_and_size(&canon);
        self.manifest.insert(canon, CacheEntry { hash, mtime_secs, size });
        self.dirty = true;
    }

    // ------------------------------------------------------------------
    // IR module cache (serialised to disk)
    // ------------------------------------------------------------------

    /// Try to load a cached `IrModule` for the given source hash.
    pub fn load_ir(&self, source_hash: &str) -> Option<crate::ir::module::IrModule> {
        if self.disabled { return None; }
        let path = self.cache_dir.join(format!("{}.ir.bin", source_hash));
        let data = fs::read(&path).ok()?;
        crate::codegen::ir_serial::deserialize_module(&data).ok()
    }

    /// Store a compiled `IrModule` under the given source hash.
    pub fn store_ir(&self, source_hash: &str, module: &crate::ir::module::IrModule) {
        if self.disabled { return; }
        let _ = fs::create_dir_all(&self.cache_dir);
        let path = self.cache_dir.join(format!("{}.ir.bin", source_hash));
        let data = crate::codegen::ir_serial::serialize_module(module);
        let _ = fs::write(&path, data);
    }

    /// Hash source text and return the hex digest for IR cache key.
    pub fn source_hash(source: &str) -> String {
        sha256_hex(source.as_bytes())
    }

    // ------------------------------------------------------------------
    // Persistence
    // ------------------------------------------------------------------

    /// Write the manifest to disk if it was modified.
    pub fn flush(&mut self) {
        if self.disabled || !self.dirty { return; }
        let _ = fs::create_dir_all(&self.cache_dir);
        let path = self.cache_dir.join("manifest.json");
        let mut out = String::from("{\n");
        let entries: Vec<_> = self.manifest.iter().collect();
        for (i, (file_path, entry)) in entries.iter().enumerate() {
            let key = file_path.to_string_lossy().replace('\\', "/");
            out.push_str(&format!(
                "  \"{}\": {{\"hash\":\"{}\",\"mtime\":{},\"size\":{}}}",
                escape_json(&key),
                entry.hash,
                entry.mtime_secs,
                entry.size,
            ));
            if i + 1 < entries.len() { out.push(','); }
            out.push('\n');
        }
        out.push('}');
        let _ = fs::write(&path, out);
        self.dirty = false;
    }

    /// Remove all cached artifacts.
    pub fn clean(&self) {
        if self.disabled { return; }
        let _ = fs::remove_dir_all(&self.cache_dir);
    }

    // ------------------------------------------------------------------
    // Internals
    // ------------------------------------------------------------------

    fn load_manifest(cache_dir: &Path) -> HashMap<PathBuf, CacheEntry> {
        let path = cache_dir.join("manifest.json");
        let data = match fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => return HashMap::new(),
        };
        parse_manifest_json(&data)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn file_mtime_and_size(path: &Path) -> (u64, u64) {
    if let Ok(meta) = fs::metadata(path) {
        let mtime = meta.modified().ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        (mtime, meta.len())
    } else {
        (0, 0)
    }
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Minimal JSON parser for the manifest file.
fn parse_manifest_json(data: &str) -> HashMap<PathBuf, CacheEntry> {
    let mut map = HashMap::new();
    // Very lightweight: split by lines, look for quoted keys.
    // Format: "path": {"hash":"...", "mtime":N, "size":N}
    for line in data.lines() {
        let line = line.trim().trim_end_matches(',');
        if line.starts_with('"') {
            if let Some(colon) = line.find("}: {") {
                // malformed — skip
                let _ = colon;
                continue;
            }
            // Split on first ": {"
            if let Some(sep) = line.find("\": {") {
                let key = &line[1..sep];
                let rest = &line[sep + 4..];
                let rest = rest.trim_end_matches('}');
                let hash = extract_json_str(rest, "hash").unwrap_or_default();
                let mtime = extract_json_num(rest, "mtime").unwrap_or(0);
                let size = extract_json_num(rest, "size").unwrap_or(0);
                if !hash.is_empty() {
                    map.insert(
                        PathBuf::from(key.replace("\\\\", "\\")),
                        CacheEntry { hash, mtime_secs: mtime, size },
                    );
                }
            }
        }
    }
    map
}

fn extract_json_str(s: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":\"", key);
    let start = s.find(&needle)? + needle.len();
    let end = s[start..].find('"')? + start;
    Some(s[start..end].to_string())
}

fn extract_json_num(s: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{}\":", key);
    let start = s.find(&needle)? + needle.len();
    let rest = s[start..].trim_start();
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_empty() {
        // SHA-256 of empty string
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_hello() {
        assert_eq!(
            sha256_hex(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn sha256_iris() {
        let input = b"def main() -> int { 0 }";
        let h = sha256_hex(input);
        assert_eq!(h.len(), 64);
        // Same input produces same hash
        assert_eq!(sha256_hex(input), h);
    }

    #[test]
    fn manifest_roundtrip() {
        let json = r#"{
  "C:/project/main.iris": {"hash":"abc123","mtime":1700000000,"size":42},
  "C:/project/lib.iris": {"hash":"def456","mtime":1700000001,"size":100}
}"#;
        let map = parse_manifest_json(json);
        assert_eq!(map.len(), 2);
        let entry = map.get(&PathBuf::from("C:/project/main.iris")).unwrap();
        assert_eq!(entry.hash, "abc123");
        assert_eq!(entry.mtime_secs, 1700000000);
        assert_eq!(entry.size, 42);
    }
}
