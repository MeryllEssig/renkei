use std::fs::{self, File};
use std::path::Path;

use flate2::write::GzEncoder;
use flate2::Compression;
use owo_colors::OwoColorize;

use crate::cache::{append_filtered_dir, compute_sha256};
use crate::cli::BumpLevel;
use crate::error::{RenkeiError, Result};
use crate::json_file;
use crate::manifest::{Manifest, ValidatedManifest};
use crate::rkignore;

const PACKAGE_DIRS: &[&str] = &["skills", "hooks", "agents", "scripts"];

pub fn run_package(bump: Option<BumpLevel>) -> Result<()> {
    let cwd = std::env::current_dir()?;

    let validated = if let Some(level) = &bump {
        let manifest = Manifest::from_path(&cwd)?;
        manifest.validate()?;
        bump_version(&cwd.join("renkei.json"), level)?;
        Manifest::from_path(&cwd)?.validate()?
    } else {
        Manifest::from_path(&cwd)?.validate()?
    };

    let (archive_path, entries) = create_package_archive(&cwd, &validated)?;
    let hash = compute_sha256(&archive_path)?;
    let size = fs::metadata(&archive_path)?.len();

    print_summary(&validated, &entries, size, &hash, &archive_path);

    Ok(())
}

fn bump_version(manifest_path: &Path, level: &BumpLevel) -> Result<()> {
    let content = fs::read_to_string(manifest_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            RenkeiError::ManifestNotFound(manifest_path.to_path_buf())
        } else {
            RenkeiError::Io(e)
        }
    })?;
    let mut raw: serde_json::Value = serde_json::from_str(&content)?;

    let version_str = raw["version"]
        .as_str()
        .ok_or_else(|| RenkeiError::InvalidManifest("missing version field".into()))?;

    let mut version =
        semver::Version::parse(version_str).map_err(|e| RenkeiError::InvalidVersion {
            version: version_str.to_string(),
            reason: e.to_string(),
        })?;

    match level {
        BumpLevel::Patch => version.patch += 1,
        BumpLevel::Minor => {
            version.minor += 1;
            version.patch = 0;
        }
        BumpLevel::Major => {
            version.major += 1;
            version.minor = 0;
            version.patch = 0;
        }
    }
    version.pre = semver::Prerelease::EMPTY;
    version.build = semver::BuildMetadata::EMPTY;

    raw["version"] = serde_json::Value::String(version.to_string());
    json_file::write_json_pretty(manifest_path, &raw)?;

    Ok(())
}

fn create_package_archive(
    cwd: &Path,
    manifest: &ValidatedManifest,
) -> Result<(std::path::PathBuf, Vec<String>)> {
    let filename = format!(
        "{}-{}-{}.tar.gz",
        manifest.scope, manifest.short_name, manifest.version
    );
    let output = cwd.join(&filename);

    let file = File::create(&output).map_err(|e| {
        RenkeiError::CacheError(format!("Cannot create archive {}: {}", output.display(), e))
    })?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar_builder = tar::Builder::new(enc);

    let mut entries = Vec::new();

    tar_builder.append_path_with_name(cwd.join("renkei.json"), "renkei.json")?;
    entries.push("renkei.json".to_string());

    let pkg_patterns = rkignore::load_rkignore(cwd);
    for dir_name in PACKAGE_DIRS {
        let dir = cwd.join(dir_name);
        if dir.is_dir() {
            let added = append_filtered_dir(&mut tar_builder, &dir, dir_name, &pkg_patterns)?;
            entries.extend(added);
        }
    }

    let mcp_root = cwd.join("mcp");
    if mcp_root.is_dir() {
        let mcp_patterns = rkignore::load_mcp_ignores(cwd);
        let mut subdirs: Vec<_> = fs::read_dir(&mcp_root)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .collect();
        subdirs.sort_by_key(|e| e.file_name());
        for entry in subdirs {
            let dir = entry.path();
            let prefix = format!("mcp/{}", entry.file_name().to_string_lossy());
            let added = append_filtered_dir(&mut tar_builder, &dir, &prefix, &mcp_patterns)?;
            entries.extend(added);
        }
    }

    tar_builder.into_inner()?.finish()?;

    Ok((output, entries))
}

fn print_summary(
    manifest: &ValidatedManifest,
    entries: &[String],
    size: u64,
    hash: &str,
    archive_path: &Path,
) {
    println!(
        "\n{} {} {}",
        "Package".bold(),
        manifest.full_name.cyan(),
        format!("v{}", manifest.version).dimmed()
    );
    println!("\n{}:", "Files".bold());
    for entry in entries {
        println!("  {}", entry);
    }
    println!(
        "\n{} {}, {}",
        entries.len().to_string().bold(),
        if entries.len() == 1 { "file" } else { "files" },
        format_size(size)
    );
    println!("{} {}", "SHA-256:".dimmed(), hash.get(..16).unwrap_or(hash));
    println!(
        "\n{} {}",
        "Created".green().bold(),
        archive_path.file_name().unwrap().to_string_lossy()
    );
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_manifest(dir: &Path, version: &str) {
        fs::write(
            dir.join("renkei.json"),
            format!(
                r#"{{"name":"@test/sample","version":"{}","description":"test","author":"tester","license":"MIT","backends":["claude"]}}"#,
                version
            ),
        )
        .unwrap();
    }

    fn write_manifest_with_extras(dir: &Path, version: &str) {
        fs::write(
            dir.join("renkei.json"),
            format!(
                r#"{{"name":"@test/sample","version":"{}","description":"test","author":"tester","license":"MIT","backends":["claude"],"keywords":["review"],"mcp":{{"server":"test"}}}}"#,
                version
            ),
        )
        .unwrap();
    }

    fn setup_full_package(dir: &Path) {
        write_manifest(dir, "1.0.0");
        let skills = dir.join("skills");
        fs::create_dir_all(&skills).unwrap();
        fs::write(skills.join("review.md"), "# Review").unwrap();
        let agents = dir.join("agents");
        fs::create_dir_all(&agents).unwrap();
        fs::write(agents.join("deploy.md"), "# Deploy").unwrap();
        let hooks = dir.join("hooks");
        fs::create_dir_all(&hooks).unwrap();
        fs::write(hooks.join("lint.json"), "[]").unwrap();
        let scripts = dir.join("scripts");
        fs::create_dir_all(&scripts).unwrap();
        fs::write(scripts.join("build.sh"), "#!/bin/bash").unwrap();
    }

    // --- bump_version tests ---

    #[test]
    fn test_bump_patch() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), "1.2.3");
        bump_version(&dir.path().join("renkei.json"), &BumpLevel::Patch).unwrap();
        let manifest = Manifest::from_path(dir.path()).unwrap();
        assert_eq!(manifest.version, "1.2.4");
    }

    #[test]
    fn test_bump_minor() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), "1.2.3");
        bump_version(&dir.path().join("renkei.json"), &BumpLevel::Minor).unwrap();
        let manifest = Manifest::from_path(dir.path()).unwrap();
        assert_eq!(manifest.version, "1.3.0");
    }

    #[test]
    fn test_bump_major() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), "1.2.3");
        bump_version(&dir.path().join("renkei.json"), &BumpLevel::Major).unwrap();
        let manifest = Manifest::from_path(dir.path()).unwrap();
        assert_eq!(manifest.version, "2.0.0");
    }

    #[test]
    fn test_bump_clears_prerelease() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), "1.0.0-beta.1");
        bump_version(&dir.path().join("renkei.json"), &BumpLevel::Patch).unwrap();
        let manifest = Manifest::from_path(dir.path()).unwrap();
        assert_eq!(manifest.version, "1.0.1");
    }

    #[test]
    fn test_bump_rewrites_manifest() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), "0.1.0");
        bump_version(&dir.path().join("renkei.json"), &BumpLevel::Minor).unwrap();
        let content = fs::read_to_string(dir.path().join("renkei.json")).unwrap();
        let raw: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(raw["version"], "0.2.0");
    }

    #[test]
    fn test_bump_preserves_other_fields() {
        let dir = tempdir().unwrap();
        write_manifest_with_extras(dir.path(), "1.0.0");
        bump_version(&dir.path().join("renkei.json"), &BumpLevel::Patch).unwrap();
        let content = fs::read_to_string(dir.path().join("renkei.json")).unwrap();
        let raw: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(raw["version"], "1.0.1");
        assert_eq!(raw["keywords"][0], "review");
        assert_eq!(raw["mcp"]["server"], "test");
        assert_eq!(raw["name"], "@test/sample");
    }

    // --- archive tests ---

    #[test]
    fn test_archive_name_format() {
        let dir = tempdir().unwrap();
        setup_full_package(dir.path());
        let manifest = Manifest::from_path(dir.path()).unwrap().validate().unwrap();
        let (path, _) = create_package_archive(dir.path(), &manifest).unwrap();
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "test-sample-1.0.0.tar.gz"
        );
    }

    #[test]
    fn test_archive_contains_correct_entries() {
        let dir = tempdir().unwrap();
        setup_full_package(dir.path());
        // Add a file that should NOT be in the archive
        fs::write(dir.path().join("README.md"), "# README").unwrap();

        let manifest = Manifest::from_path(dir.path()).unwrap().validate().unwrap();
        let (archive_path, entries) = create_package_archive(dir.path(), &manifest).unwrap();

        assert!(entries.contains(&"renkei.json".to_string()));
        assert!(entries.contains(&"skills/review.md".to_string()));
        assert!(entries.contains(&"agents/deploy.md".to_string()));
        assert!(entries.contains(&"hooks/lint.json".to_string()));
        assert!(entries.contains(&"scripts/build.sh".to_string()));
        assert!(!entries.iter().any(|e| e.contains("README")));

        // Verify actual archive contents
        let file = File::open(&archive_path).unwrap();
        let dec = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(dec);
        let archive_entries: Vec<String> = archive
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(archive_entries.contains(&"renkei.json".to_string()));
        assert!(archive_entries.iter().any(|e| e.contains("review.md")));
        assert!(!archive_entries.iter().any(|e| e.contains("README")));
    }

    #[test]
    fn test_archive_includes_scripts() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), "1.0.0");
        let scripts = dir.path().join("scripts");
        fs::create_dir_all(&scripts).unwrap();
        fs::write(scripts.join("build.sh"), "#!/bin/bash").unwrap();

        let manifest = Manifest::from_path(dir.path()).unwrap().validate().unwrap();
        let (_, entries) = create_package_archive(dir.path(), &manifest).unwrap();
        assert!(entries.contains(&"scripts/build.sh".to_string()));
    }

    #[test]
    fn test_package_only_manifest() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), "1.0.0");
        let manifest = Manifest::from_path(dir.path()).unwrap().validate().unwrap();
        let (path, entries) = create_package_archive(dir.path(), &manifest).unwrap();
        assert_eq!(entries, vec!["renkei.json"]);
        assert!(path.exists());
    }

    #[test]
    fn test_archive_includes_mcp_filtered() {
        let dir = tempdir().unwrap();
        // Manifest declares mcp.foo with a prebuilt entrypoint, no build.
        fs::write(
            dir.path().join("renkei.json"),
            r#"{"name":"@test/sample","version":"1.0.0","description":"t","author":"x","license":"MIT","backends":["claude"],"mcp":{"foo":{"command":"node","entrypoint":"dist/index.js"}}}"#,
        )
        .unwrap();
        let mcp = dir.path().join("mcp/foo");
        fs::create_dir_all(mcp.join("dist")).unwrap();
        fs::write(mcp.join("dist/index.js"), "x").unwrap();
        fs::create_dir_all(mcp.join("node_modules/junk")).unwrap();
        fs::write(mcp.join("node_modules/junk/x.js"), "ignored").unwrap();

        let manifest = Manifest::from_path(dir.path()).unwrap().validate().unwrap();
        let (_, entries) = create_package_archive(dir.path(), &manifest).unwrap();
        assert!(entries.iter().any(|e| e == "mcp/foo/dist/index.js"));
        assert!(
            !entries.iter().any(|e| e.contains("node_modules")),
            "node_modules must be excluded: {:?}",
            entries
        );
    }

    #[test]
    fn test_archive_honors_root_rkignore() {
        let dir = tempdir().unwrap();
        setup_full_package(dir.path());
        fs::write(dir.path().join(".rkignore"), "agents/deploy.md\n").unwrap();

        let manifest = Manifest::from_path(dir.path()).unwrap().validate().unwrap();
        let (_, entries) = create_package_archive(dir.path(), &manifest).unwrap();
        assert!(entries.contains(&"skills/review.md".to_string()));
        assert!(
            !entries.iter().any(|e| e.ends_with("deploy.md")),
            ".rkignore must drop the file"
        );
    }

    // --- format_size tests ---

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn test_format_size_kb() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
    }

    #[test]
    fn test_format_size_mb() {
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 5), "5.0 MB");
    }
}
