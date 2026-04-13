use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::Result;

/// Always-on exclusions when packaging or hashing a renkei package.
/// Tooling-specific build outputs and VCS noise that authors never intend
/// to ship inside a source tarball.
pub const DEFAULT_IGNORES: &[&str] = &[
    "node_modules/",
    "dist/",
    "build/",
    "target/",
    ".venv/",
    "venv/",
    "__pycache__/",
    ".pytest_cache/",
    "*.pyc",
    ".DS_Store",
    ".git/",
];

/// Resolve the effective ignore patterns for a directory: defaults first,
/// then any non-empty / non-comment line from `<root>/.rkignore` (gitignore
/// syntax) appended.
#[allow(dead_code)]
pub fn load_rkignore(root: &Path) -> Vec<String> {
    let mut patterns: Vec<String> = DEFAULT_IGNORES.iter().map(|s| s.to_string()).collect();
    let path = root.join(".rkignore");
    if let Ok(content) = std::fs::read_to_string(&path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            patterns.push(trimmed.to_string());
        }
    }
    patterns
}

/// Deterministic content hash of `root` after applying the rkignore patterns.
///
/// Walk order is alphabetical; each included file contributes its
/// relative path (`/`-normalized), unix mode bits (or 0 on non-unix), and
/// streamed contents, separated by NUL so that distinct boundaries cannot
/// collide. Honors `<root>/.rkignore` automatically; `extra_patterns` is
/// appended for callers that need ad-hoc additions. Reused by packaging
/// and install-time integrity tracking.
#[allow(dead_code)]
pub fn hash_directory(root: &Path, extra_patterns: &[String]) -> Result<String> {
    let mut all_patterns = load_rkignore(root);
    all_patterns.extend(extra_patterns.iter().cloned());

    let mut overrides = ignore::overrides::OverrideBuilder::new(root);
    for pat in &all_patterns {
        // gitignore semantics: a bare pattern excludes; we invert to make it an
        // explicit ignore in the override builder.
        let inverted = format!("!{pat}");
        overrides
            .add(&inverted)
            .map_err(|e| crate::error::RenkeiError::CacheError(format!("rkignore pattern error: {e}")))?;
    }
    let overrides = overrides
        .build()
        .map_err(|e| crate::error::RenkeiError::CacheError(format!("rkignore build error: {e}")))?;

    let walker = ignore::WalkBuilder::new(root)
        .standard_filters(false)
        .hidden(false)
        .overrides(overrides)
        .sort_by_file_path(|a, b| a.cmp(b))
        .build();

    let mut hasher = Sha256::new();
    for entry in walker {
        let entry = entry
            .map_err(|e| crate::error::RenkeiError::CacheError(format!("walk error: {e}")))?;
        let path = entry.path();
        let file_type = match entry.file_type() {
            Some(ft) => ft,
            None => continue,
        };
        if !file_type.is_file() {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(path);
        let rel_norm = rel.to_string_lossy().replace('\\', "/");
        let mode = unix_mode(path);

        hasher.update(rel_norm.as_bytes());
        hasher.update([0u8]);
        hasher.update(mode.to_le_bytes());
        hasher.update([0u8]);
        let mut reader = BufReader::new(File::open(path)?);
        std::io::copy(&mut reader, &mut hasher)?;
        hasher.update([0u8]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(unix)]
fn unix_mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode())
        .unwrap_or(0)
}

#[cfg(not(unix))]
fn unix_mode(_path: &Path) -> u32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(dir: &Path, rel: &str, content: &[u8]) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    #[test]
    fn test_load_rkignore_returns_defaults_when_missing() {
        let dir = tempdir().unwrap();
        let pats = load_rkignore(dir.path());
        assert_eq!(pats.len(), DEFAULT_IGNORES.len());
        assert!(pats.iter().any(|p| p == "node_modules/"));
        assert!(pats.iter().any(|p| p == ".git/"));
    }

    #[test]
    fn test_load_rkignore_appends_user_patterns() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join(".rkignore"),
            "# comment\n\ngenerated/\n*.log\n",
        )
        .unwrap();
        let pats = load_rkignore(dir.path());
        assert!(pats.iter().any(|p| p == "generated/"));
        assert!(pats.iter().any(|p| p == "*.log"));
        assert!(pats.iter().any(|p| p == "node_modules/"));
    }

    #[test]
    fn test_hash_directory_excludes_default_ignored_paths() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();

        write(a.path(), "src/main.rs", b"fn main() {}");
        write(a.path(), "node_modules/foo/index.js", b"junk1");

        write(b.path(), "src/main.rs", b"fn main() {}");
        write(b.path(), "target/debug/x.bin", b"junk2");

        let h_a = hash_directory(a.path(), &[]).unwrap();
        let h_b = hash_directory(b.path(), &[]).unwrap();
        assert_eq!(h_a, h_b);
    }

    #[test]
    fn test_hash_directory_changes_with_content() {
        let dir = tempdir().unwrap();
        write(dir.path(), "f.txt", b"v1");
        let h1 = hash_directory(dir.path(), &[]).unwrap();

        std::fs::write(dir.path().join("f.txt"), b"v2").unwrap();
        let h2 = hash_directory(dir.path(), &[]).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_directory_paths_are_part_of_hash() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();
        write(a.path(), "a.txt", b"same");
        write(b.path(), "b.txt", b"same");
        assert_ne!(
            hash_directory(a.path(), &[]).unwrap(),
            hash_directory(b.path(), &[]).unwrap()
        );
    }

    #[test]
    fn test_hash_directory_extra_patterns_take_effect() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();
        write(a.path(), "src/main.rs", b"fn main() {}");
        write(a.path(), "generated/x.rs", b"auto");

        write(b.path(), "src/main.rs", b"fn main() {}");

        let h_a = hash_directory(a.path(), &["generated/".to_string()]).unwrap();
        let h_b = hash_directory(b.path(), &["generated/".to_string()]).unwrap();
        assert_eq!(h_a, h_b);
    }

    #[test]
    fn test_hash_directory_includes_dotfiles_outside_default_ignores() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();
        write(a.path(), ".env.example", b"KEY=value");
        write(b.path(), ".env.example", b"KEY=other");
        assert_ne!(
            hash_directory(a.path(), &[]).unwrap(),
            hash_directory(b.path(), &[]).unwrap()
        );
    }

    #[test]
    fn test_hash_directory_honors_rkignore_file() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();

        std::fs::write(a.path().join(".rkignore"), "generated/\n").unwrap();
        write(a.path(), "src/main.rs", b"fn main() {}");
        write(a.path(), "generated/x.rs", b"auto");

        std::fs::write(b.path().join(".rkignore"), "generated/\n").unwrap();
        write(b.path(), "src/main.rs", b"fn main() {}");

        assert_eq!(
            hash_directory(a.path(), &[]).unwrap(),
            hash_directory(b.path(), &[]).unwrap()
        );
    }

    #[test]
    fn test_hash_directory_is_stable_across_repeat_calls() {
        let dir = tempdir().unwrap();
        write(dir.path(), "src/lib.rs", b"// content");
        write(dir.path(), "src/util.rs", b"// other");
        let h1 = hash_directory(dir.path(), &[]).unwrap();
        let h2 = hash_directory(dir.path(), &[]).unwrap();
        assert_eq!(h1, h2);
    }
}
