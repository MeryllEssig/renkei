use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::manifest::ValidatedManifest;
use crate::rkignore;

pub fn archive_path(
    config: &Config,
    scope: &str,
    short_name: &str,
    version: &semver::Version,
) -> PathBuf {
    config
        .archives_dir()
        .join(format!("@{scope}"))
        .join(short_name)
        .join(format!("{version}.tar.gz"))
}

pub fn create_archive(
    package_dir: &Path,
    manifest: &ValidatedManifest,
    config: &Config,
) -> Result<(PathBuf, String)> {
    let path = archive_path(
        config,
        &manifest.scope,
        &manifest.short_name,
        &manifest.version,
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = File::create(&path).map_err(|e| {
        RenkeiError::CacheError(format!("Cannot create archive {}: {}", path.display(), e))
    })?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar_builder = tar::Builder::new(enc);

    tar_builder.append_path_with_name(package_dir.join("renkei.json"), "renkei.json")?;

    let pkg_patterns = rkignore::load_rkignore(package_dir);
    for dir_name in &["skills", "hooks", "agents", "scripts"] {
        let dir = package_dir.join(dir_name);
        if dir.is_dir() {
            append_filtered_dir(&mut tar_builder, &dir, dir_name, &pkg_patterns)?;
        }
    }

    let mcp_root = package_dir.join("mcp");
    if mcp_root.is_dir() {
        let mcp_patterns = rkignore::load_mcp_ignores(package_dir);
        let mut subdirs: Vec<_> = fs::read_dir(&mcp_root)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .collect();
        subdirs.sort_by_key(|e| e.file_name());
        for entry in subdirs {
            let dir = entry.path();
            let prefix = format!("mcp/{}", entry.file_name().to_string_lossy());
            append_filtered_dir(&mut tar_builder, &dir, &prefix, &mcp_patterns)?;
        }
    }

    tar_builder.into_inner()?.finish()?;

    let hash = compute_sha256(&path)?;
    Ok((path, hash))
}

/// Walk `root` with the rkignore-driven filter and append each surviving file
/// under `archive_prefix/<relative-path>` into `tar`. Returns the list of
/// archive-relative paths added (used by callers that report a file summary).
pub(crate) fn append_filtered_dir<W: Write>(
    tar: &mut tar::Builder<W>,
    root: &Path,
    archive_prefix: &str,
    patterns: &[String],
) -> Result<Vec<String>> {
    let walk = rkignore::build_walker(root, patterns)?;
    let mut added = Vec::new();
    for entry in walk {
        let entry =
            entry.map_err(|e| RenkeiError::CacheError(format!("rkignore walk: {e}")))?;
        let path = entry.path();
        let rel = match path.strip_prefix(root) {
            Ok(p) if !p.as_os_str().is_empty() => p,
            _ => continue,
        };
        if path.is_file() {
            let archive_path = format!("{}/{}", archive_prefix, rel.to_string_lossy());
            tar.append_path_with_name(path, &archive_path)?;
            added.push(archive_path);
        }
    }
    Ok(added)
}

pub(crate) fn compute_sha256(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut reader = BufReader::new(File::open(path)?);
    std::io::copy(&mut reader, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn compute_sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

pub(crate) fn extract_file_from_archive(archive_path: &Path, inner_path: &str) -> Result<Vec<u8>> {
    let file = File::open(archive_path).map_err(|e| {
        RenkeiError::CacheError(format!(
            "Cannot open archive {}: {}",
            archive_path.display(),
            e
        ))
    })?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().to_string();
        if path == inner_path {
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut buf)?;
            return Ok(buf);
        }
    }

    Err(RenkeiError::CacheError(format!(
        "File '{}' not found in archive {}",
        inner_path,
        archive_path.display()
    )))
}

pub(crate) fn extract_archive_to_dir(archive_path: &Path, dest: &Path) -> Result<()> {
    let file = File::open(archive_path).map_err(|e| {
        RenkeiError::CacheError(format!(
            "Cannot open archive {}: {}",
            archive_path.display(),
            e
        ))
    })?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);
    archive.unpack(dest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;
    use tempfile::tempdir;

    fn make_test_manifest() -> ValidatedManifest {
        ValidatedManifest {
            scope: "test".to_string(),
            short_name: "sample".to_string(),
            full_name: "@test/sample".to_string(),
            version: Version::new(0, 1, 0),
            install_scope: crate::manifest::ManifestScope::Any,
            description: "test".to_string(),
            author: "tester".to_string(),
            license: "MIT".to_string(),
            backends: vec!["claude".to_string()],
        }
    }

    fn setup_package(dir: &Path) {
        fs::write(
            dir.join("renkei.json"),
            r#"{"name":"@test/sample","version":"0.1.0","description":"test","author":"tester","license":"MIT","backends":["claude"]}"#,
        )
        .unwrap();
        let skill_dir = dir.join("skills/review");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# Review").unwrap();
    }

    #[test]
    fn test_create_archive() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        setup_package(pkg.path());

        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();

        let (archive_path, hash) = create_archive(pkg.path(), &manifest, &config).unwrap();

        let expected_path = home
            .path()
            .join(".renkei/archives/@test/sample/0.1.0.tar.gz");
        assert_eq!(archive_path, expected_path);
        assert!(archive_path.exists());
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_archive_contains_manifest_and_skills() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        setup_package(pkg.path());

        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();

        let (archive_path, _) = create_archive(pkg.path(), &manifest, &config).unwrap();

        let file = File::open(&archive_path).unwrap();
        let dec = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(dec);
        let entries: Vec<String> = archive
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().to_string())
            .collect();

        assert!(entries.contains(&"renkei.json".to_string()));
        assert!(entries.iter().any(|e| e.contains("review/SKILL.md")));
    }

    #[test]
    fn test_compute_sha256_bytes() {
        let hash = compute_sha256_bytes(b"hello world");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        // Same input should give same hash
        assert_eq!(hash, compute_sha256_bytes(b"hello world"));
        // Different input gives different hash
        assert_ne!(hash, compute_sha256_bytes(b"hello world!"));
    }

    #[test]
    fn test_extract_file_from_archive_valid() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        setup_package(pkg.path());

        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = create_archive(pkg.path(), &manifest, &config).unwrap();

        let content = extract_file_from_archive(&archive_path, "skills/review/SKILL.md").unwrap();
        assert_eq!(content, b"# Review");
    }

    #[test]
    fn test_extract_file_from_archive_manifest() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        setup_package(pkg.path());

        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = create_archive(pkg.path(), &manifest, &config).unwrap();

        let content = extract_file_from_archive(&archive_path, "renkei.json").unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&content).unwrap();
        assert_eq!(parsed["name"], "@test/sample");
    }

    #[test]
    fn test_extract_file_from_archive_missing_inner_path() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        setup_package(pkg.path());

        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = create_archive(pkg.path(), &manifest, &config).unwrap();

        let err = extract_file_from_archive(&archive_path, "nonexistent.md").unwrap_err();
        assert!(err.to_string().contains("not found in archive"));
    }

    #[test]
    fn test_extract_file_from_archive_missing_archive() {
        let err = extract_file_from_archive(Path::new("/nonexistent/archive.tar.gz"), "file.md")
            .unwrap_err();
        assert!(err.to_string().contains("Cannot open archive"));
    }

    fn list_archive_entries(archive_path: &Path) -> Vec<String> {
        let file = File::open(archive_path).unwrap();
        let dec = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(dec);
        archive
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn test_archive_excludes_node_modules_inside_mcp() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        setup_package(pkg.path());
        let mcp_dir = pkg.path().join("mcp/foo");
        fs::create_dir_all(mcp_dir.join("node_modules/junk")).unwrap();
        fs::write(mcp_dir.join("node_modules/junk/index.js"), "x").unwrap();
        fs::write(mcp_dir.join("entrypoint.js"), "module.exports={};").unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = create_archive(pkg.path(), &manifest, &config).unwrap();

        let entries = list_archive_entries(&archive_path);
        assert!(entries.iter().any(|e| e == "mcp/foo/entrypoint.js"));
        assert!(
            !entries.iter().any(|e| e.contains("node_modules")),
            "node_modules should be excluded: {:?}",
            entries
        );
    }

    #[test]
    fn test_archive_keeps_dist_inside_mcp() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        setup_package(pkg.path());
        let mcp_dir = pkg.path().join("mcp/foo/dist");
        fs::create_dir_all(&mcp_dir).unwrap();
        fs::write(mcp_dir.join("index.js"), "console.log(1)").unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = create_archive(pkg.path(), &manifest, &config).unwrap();

        let entries = list_archive_entries(&archive_path);
        assert!(
            entries.iter().any(|e| e == "mcp/foo/dist/index.js"),
            "dist/ must survive MCP-trimmed defaults: {:?}",
            entries
        );
    }

    #[test]
    fn test_archive_honors_rkignore_extension() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        setup_package(pkg.path());
        fs::write(pkg.path().join(".rkignore"), "skills/review/extra.md\n").unwrap();
        fs::write(pkg.path().join("skills/review/extra.md"), "ignored").unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = create_archive(pkg.path(), &manifest, &config).unwrap();

        let entries = list_archive_entries(&archive_path);
        assert!(entries.iter().any(|e| e.ends_with("SKILL.md")));
        assert!(
            !entries.iter().any(|e| e.contains("extra.md")),
            ".rkignore custom pattern must filter the file"
        );
    }

    #[test]
    fn test_extract_archive_to_dir() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        setup_package(pkg.path());

        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();
        let (archive_path, _) = create_archive(pkg.path(), &manifest, &config).unwrap();

        let dest = tempdir().unwrap();
        extract_archive_to_dir(&archive_path, dest.path()).unwrap();

        assert!(dest.path().join("renkei.json").exists());
        assert!(dest.path().join("skills/review/SKILL.md").exists());
    }
}
