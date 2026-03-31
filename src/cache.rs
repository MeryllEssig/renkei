use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::manifest::ValidatedManifest;

pub fn create_archive(
    package_dir: &Path,
    manifest: &ValidatedManifest,
    config: &Config,
) -> Result<(PathBuf, String)> {
    let cache_dir = config
        .cache_dir()
        .join(format!("@{}", manifest.scope))
        .join(&manifest.short_name);
    fs::create_dir_all(&cache_dir)?;

    let archive_path = cache_dir.join(format!("{}.tar.gz", manifest.version));

    let file = File::create(&archive_path).map_err(|e| {
        RenkeiError::CacheError(format!(
            "Cannot create archive {}: {}",
            archive_path.display(),
            e
        ))
    })?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar_builder = tar::Builder::new(enc);

    tar_builder.append_path_with_name(package_dir.join("renkei.json"), "renkei.json")?;

    for dir_name in &["skills", "hooks", "agents"] {
        let dir = package_dir.join(dir_name);
        if dir.is_dir() {
            tar_builder.append_dir_all(*dir_name, &dir)?;
        }
    }

    tar_builder.into_inner()?.finish()?;

    let hash = compute_sha256(&archive_path)?;
    Ok((archive_path, hash))
}

fn compute_sha256(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut reader = BufReader::new(File::open(path)?);
    std::io::copy(&mut reader, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
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
        let skills = dir.join("skills");
        fs::create_dir_all(&skills).unwrap();
        fs::write(skills.join("review.md"), "# Review").unwrap();
    }

    #[test]
    fn test_create_archive() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        setup_package(pkg.path());

        let config = Config::with_home_dir(home.path().to_path_buf());
        let manifest = make_test_manifest();

        let (archive_path, hash) = create_archive(pkg.path(), &manifest, &config).unwrap();

        let expected_path = home.path().join(".renkei/cache/@test/sample/0.1.0.tar.gz");
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
        assert!(entries.iter().any(|e| e.contains("review.md")));
    }
}
