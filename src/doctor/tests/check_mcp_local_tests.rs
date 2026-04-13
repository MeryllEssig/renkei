use std::collections::HashMap;
use std::fs::{self, File};
use std::path::Path;

use flate2::write::GzEncoder;
use flate2::Compression;
use tempfile::tempdir;

use crate::config::Config;
use crate::doctor::checks;
use crate::doctor::types::DiagnosticKind;
use crate::install_cache::{
    BackendDeployment, InstallCache, McpLocalEntry, McpLocalRef, PackageEntry,
};
use crate::rkignore;

fn write_fake_archive(path: &Path, manifest_json: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let file = File::create(path).unwrap();
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = tar::Builder::new(enc);
    let bytes = manifest_json.as_bytes();
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "renkei.json", bytes).unwrap();
    tar.into_inner().unwrap().finish().unwrap();
}

fn make_owner_pkg(archive_path: &Path) -> PackageEntry {
    let mut deployed = HashMap::new();
    deployed.insert(
        "claude".to_string(),
        BackendDeployment {
            artifacts: vec![],
            mcp_servers: vec!["my-srv".to_string()],
        },
    );
    PackageEntry {
        version: "1.0.0".to_string(),
        source: "local".to_string(),
        source_path: "/tmp/pkg".to_string(),
        integrity: "abc".to_string(),
        archive_path: archive_path.to_string_lossy().into_owned(),
        deployed,
        resolved: None,
        tag: None,
        member: None,
        mcp_local_sources: HashMap::new(),
    }
}

fn setup_folder_and_cache(
    home: &Path,
    owner: &str,
    server_name: &str,
    entrypoint: &str,
) -> (Config, InstallCache) {
    let config = Config::with_home_dir(home.to_path_buf());

    let folder = config.global_mcp_dir().join(server_name);
    fs::create_dir_all(folder.join("dist")).unwrap();
    fs::write(folder.join(entrypoint), b"console.log(1);").unwrap();
    fs::write(folder.join("src.txt"), b"original-source").unwrap();

    let patterns = rkignore::load_mcp_ignores(&folder);
    let sha = rkignore::hash_with_patterns(&folder, &patterns).unwrap();

    let archive_path = home.join("archives").join(format!("{server_name}.tar.gz"));
    let manifest_json = format!(
        r#"{{"name":"{owner}","version":"1.0.0","mcp":{{"{server_name}":{{"command":"node","entrypoint":"{entrypoint}"}}}}}}"#
    );
    write_fake_archive(&archive_path, &manifest_json);

    let mut cache = InstallCache {
        version: 3,
        packages: HashMap::new(),
        mcp_local: HashMap::new(),
    };
    cache
        .packages
        .insert(owner.to_string(), make_owner_pkg(&archive_path));
    cache.mcp_local.insert(
        server_name.to_string(),
        McpLocalEntry {
            owner_package: owner.to_string(),
            version: "1.0.0".to_string(),
            source_sha256: sha,
            referenced_by: vec![McpLocalRef {
                package: owner.to_string(),
                version: "1.0.0".to_string(),
                scope: "global".to_string(),
                project_root: None,
            }],
        },
    );
    (config, cache)
}

#[test]
fn test_all_green_when_folder_entrypoint_and_integrity_match() {
    let home = tempdir().unwrap();
    let (config, cache) =
        setup_folder_and_cache(home.path(), "@acme/pkg", "my-srv", "dist/index.js");
    let issues = checks::check_mcp_local(&cache, &config);
    assert!(issues.is_empty(), "unexpected issues: {:?}", issues);
}

#[test]
fn test_folder_deleted_reports_mcp_local_missing() {
    let home = tempdir().unwrap();
    let (config, cache) =
        setup_folder_and_cache(home.path(), "@acme/pkg", "my-srv", "dist/index.js");
    fs::remove_dir_all(config.global_mcp_dir().join("my-srv")).unwrap();

    let issues = checks::check_mcp_local(&cache, &config);
    assert_eq!(issues.len(), 1);
    assert!(matches!(&issues[0], DiagnosticKind::McpLocalMissing { name } if name == "my-srv"));
}

#[test]
fn test_source_tampered_reports_integrity_drift_warning() {
    let home = tempdir().unwrap();
    let (config, cache) =
        setup_folder_and_cache(home.path(), "@acme/pkg", "my-srv", "dist/index.js");
    // Tamper: modify a source file.
    fs::write(
        config.global_mcp_dir().join("my-srv").join("src.txt"),
        b"tampered",
    )
    .unwrap();

    let issues = checks::check_mcp_local(&cache, &config);
    assert!(
        issues.iter().any(
            |i| matches!(i, DiagnosticKind::McpLocalIntegrityDrift { name } if name == "my-srv")
        ),
        "expected drift warning, got: {:?}",
        issues
    );
}

#[test]
fn test_entrypoint_missing_reports_entrypoint_error() {
    let home = tempdir().unwrap();
    let (config, cache) =
        setup_folder_and_cache(home.path(), "@acme/pkg", "my-srv", "dist/index.js");
    fs::remove_file(config.global_mcp_dir().join("my-srv").join("dist/index.js")).unwrap();

    let issues = checks::check_mcp_local(&cache, &config);
    assert!(
        issues.iter().any(|i| matches!(i,
            DiagnosticKind::McpLocalEntrypointMissing { name, entrypoint }
                if name == "my-srv" && entrypoint == "dist/index.js")),
        "expected entrypoint error, got: {:?}",
        issues
    );
}

#[cfg(unix)]
#[test]
fn test_linked_install_identical_source_is_ok() {
    use std::os::unix::fs::symlink;
    let home = tempdir().unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    // "Workspace" source that the symlink will point to.
    let workspace = home.path().join("workspace").join("mcp").join("my-srv");
    fs::create_dir_all(workspace.join("dist")).unwrap();
    fs::write(workspace.join("dist/index.js"), b"x").unwrap();
    fs::write(workspace.join("src.txt"), b"orig").unwrap();

    let patterns = rkignore::load_mcp_ignores(&workspace);
    let sha = rkignore::hash_with_patterns(&workspace, &patterns).unwrap();

    let link = config.global_mcp_dir().join("my-srv");
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    symlink(&workspace, &link).unwrap();

    let archive_path = home.path().join("a.tar.gz");
    write_fake_archive(
        &archive_path,
        r#"{"name":"@acme/pkg","version":"1.0.0","mcp":{"my-srv":{"command":"node","entrypoint":"dist/index.js"}}}"#,
    );

    let mut cache = InstallCache {
        version: 3,
        packages: HashMap::new(),
        mcp_local: HashMap::new(),
    };
    cache
        .packages
        .insert("@acme/pkg".to_string(), make_owner_pkg(&archive_path));
    cache.mcp_local.insert(
        "my-srv".to_string(),
        McpLocalEntry {
            owner_package: "@acme/pkg".to_string(),
            version: "1.0.0".to_string(),
            source_sha256: sha,
            referenced_by: vec![McpLocalRef {
                package: "@acme/pkg".to_string(),
                version: "1.0.0".to_string(),
                scope: "global".to_string(),
                project_root: None,
            }],
        },
    );

    let issues = checks::check_mcp_local(&cache, &config);
    assert!(issues.is_empty(), "unexpected: {:?}", issues);
}

#[cfg(unix)]
#[test]
fn test_linked_install_modified_source_reports_drift() {
    use std::os::unix::fs::symlink;
    let home = tempdir().unwrap();
    let config = Config::with_home_dir(home.path().to_path_buf());

    let workspace = home.path().join("workspace").join("mcp").join("my-srv");
    fs::create_dir_all(workspace.join("dist")).unwrap();
    fs::write(workspace.join("dist/index.js"), b"x").unwrap();
    fs::write(workspace.join("src.txt"), b"orig").unwrap();

    let patterns = rkignore::load_mcp_ignores(&workspace);
    let sha = rkignore::hash_with_patterns(&workspace, &patterns).unwrap();

    let link = config.global_mcp_dir().join("my-srv");
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    symlink(&workspace, &link).unwrap();

    // Modify the workspace post-install.
    fs::write(workspace.join("src.txt"), b"mutated").unwrap();

    let archive_path = home.path().join("a.tar.gz");
    write_fake_archive(
        &archive_path,
        r#"{"name":"@acme/pkg","version":"1.0.0","mcp":{"my-srv":{"command":"node","entrypoint":"dist/index.js"}}}"#,
    );

    let mut cache = InstallCache {
        version: 3,
        packages: HashMap::new(),
        mcp_local: HashMap::new(),
    };
    cache
        .packages
        .insert("@acme/pkg".to_string(), make_owner_pkg(&archive_path));
    cache.mcp_local.insert(
        "my-srv".to_string(),
        McpLocalEntry {
            owner_package: "@acme/pkg".to_string(),
            version: "1.0.0".to_string(),
            source_sha256: sha,
            referenced_by: vec![McpLocalRef {
                package: "@acme/pkg".to_string(),
                version: "1.0.0".to_string(),
                scope: "global".to_string(),
                project_root: None,
            }],
        },
    );

    let issues = checks::check_mcp_local(&cache, &config);
    assert!(
        issues
            .iter()
            .any(|i| matches!(i, DiagnosticKind::McpLocalIntegrityDrift { .. })),
        "expected drift: {:?}",
        issues
    );
}
