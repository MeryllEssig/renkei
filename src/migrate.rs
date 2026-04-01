use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use owo_colors::OwoColorize;

use crate::artifact::ArtifactKind;
use crate::error::{RenkeiError, Result};
use crate::hook;
use crate::json_file;
use crate::manifest::Manifest;

#[derive(Debug)]
struct DiscoveredFile {
    original_path: PathBuf,
    kind: ArtifactKind,
    filename: String,
    already_placed: bool,
}

const SKIP_FILES: &[&str] = &[
    "skills-lock.json",
    "package.json",
    "package-lock.json",
    "renkei.json",
    "tsconfig.json",
];

const SKIP_DIRS: &[&str] = &["node_modules", ".git", "target", ".claude"];

pub fn run_migrate(path_str: &str) -> Result<()> {
    let path = PathBuf::from(path_str)
        .canonicalize()
        .map_err(|_| RenkeiError::ManifestNotFound(PathBuf::from(path_str)))?;

    if path.join("renkei.json").exists() {
        return Err(RenkeiError::AlreadyRenkeiPackage(path));
    }

    let discovered = scan_directory(&path)?;

    if discovered.is_empty() {
        return Err(RenkeiError::NothingToMigrate(path));
    }

    let (skills, hooks, agents) = count_by_kind(&discovered);

    reorganize_files(&path, &discovered)?;
    generate_manifest(&path)?;

    Manifest::from_path(&path)?.validate()?;

    println!("{}", "Migrated successfully!".green().bold());
    println!();
    if skills > 0 {
        println!("  {} skill(s) → skills/", skills);
    }
    if hooks > 0 {
        println!("  {} hook(s)  → hooks/", hooks);
    }
    if agents > 0 {
        println!("  {} agent(s) → agents/", agents);
    }
    println!();
    println!("  Created {}", "renkei.json".bold());

    Ok(())
}

fn count_by_kind(discovered: &[DiscoveredFile]) -> (usize, usize, usize) {
    let skills = discovered
        .iter()
        .filter(|f| matches!(f.kind, ArtifactKind::Skill))
        .count();
    let hooks = discovered
        .iter()
        .filter(|f| matches!(f.kind, ArtifactKind::Hook))
        .count();
    let agents = discovered
        .iter()
        .filter(|f| matches!(f.kind, ArtifactKind::Agent))
        .count();
    (skills, hooks, agents)
}

fn scan_directory(path: &Path) -> Result<Vec<DiscoveredFile>> {
    let mut discovered = Vec::new();
    let mut subdirs = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if !SKIP_DIRS.contains(&dir_name.as_str()) {
                subdirs.push(entry_path);
            }
        } else if entry_path.is_file() {
            classify_file(&entry_path, &mut discovered)?;
        }
    }

    for subdir in subdirs {
        scan_entries(&subdir, &mut discovered)?;
    }

    Ok(discovered)
}

fn scan_entries(dir: &Path, discovered: &mut Vec<DiscoveredFile>) -> Result<()> {
    let parent_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let is_conventional = matches!(parent_name.as_str(), "skills" | "hooks" | "agents");

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let entry_path = entry.path();
        if !entry_path.is_file() {
            continue;
        }

        let filename = entry.file_name().to_string_lossy().to_string();
        if SKIP_FILES.contains(&filename.as_str()) {
            continue;
        }

        let ext = entry_path.extension().and_then(|e| e.to_str());
        match ext {
            Some("md") => {
                let kind = if parent_name == "agents" {
                    ArtifactKind::Agent
                } else {
                    ArtifactKind::Skill
                };
                discovered.push(DiscoveredFile {
                    original_path: entry_path,
                    kind,
                    filename,
                    already_placed: is_conventional,
                });
            }
            Some("json") => {
                if hook::parse_hook_file(&entry_path).is_ok() {
                    discovered.push(DiscoveredFile {
                        original_path: entry_path,
                        kind: ArtifactKind::Hook,
                        filename,
                        already_placed: is_conventional,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn classify_file(entry_path: &Path, discovered: &mut Vec<DiscoveredFile>) -> Result<()> {
    let filename = entry_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    if SKIP_FILES.contains(&filename.as_str()) {
        return Ok(());
    }

    let ext = entry_path.extension().and_then(|e| e.to_str());
    match ext {
        Some("md") => {
            discovered.push(DiscoveredFile {
                original_path: entry_path.to_path_buf(),
                kind: ArtifactKind::Skill,
                filename,
                already_placed: false,
            });
        }
        Some("json") => {
            if hook::parse_hook_file(entry_path).is_ok() {
                discovered.push(DiscoveredFile {
                    original_path: entry_path.to_path_buf(),
                    kind: ArtifactKind::Hook,
                    filename,
                    already_placed: false,
                });
            }
        }
        _ => {}
    }
    Ok(())
}

fn reorganize_files(root: &Path, discovered: &[DiscoveredFile]) -> Result<()> {
    let needed_dirs: HashSet<PathBuf> = discovered
        .iter()
        .filter(|f| !f.already_placed)
        .map(|f| root.join(f.kind.dir_name()))
        .collect();

    for dir in &needed_dirs {
        fs::create_dir_all(dir)?;
    }

    for file in discovered {
        if file.already_placed {
            continue;
        }

        let target_dir = root.join(file.kind.dir_name());
        let mut target = target_dir.join(&file.filename);

        let stem = Path::new(&file.filename)
            .file_stem()
            .unwrap()
            .to_string_lossy();
        let ext = Path::new(&file.filename)
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut counter = 1;
        while target.exists() {
            target = target_dir.join(format!("{}-{}.{}", stem, counter, ext));
            counter += 1;
        }

        fs::rename(&file.original_path, &target)?;
    }
    Ok(())
}

fn generate_manifest(path: &Path) -> Result<()> {
    let dir_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "package".to_string());

    let sanitized = sanitize_name(&dir_name);

    let manifest = serde_json::json!({
        "name": format!("@migrated/{}", sanitized),
        "version": "0.1.0",
        "description": format!("Migrated package from {}", dir_name),
        "author": "unknown",
        "license": "MIT",
        "backends": ["claude"]
    });

    json_file::write_json_pretty(&path.join("renkei.json"), &manifest)
}

fn sanitize_name(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();

    let mut result = String::new();
    let mut prev_hyphen = false;
    for c in s.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push(c);
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }

    result.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sanitize_name_basic() {
        assert_eq!(sanitize_name("my-package"), "my-package");
    }

    #[test]
    fn test_sanitize_name_spaces() {
        assert_eq!(sanitize_name("My Package"), "my-package");
    }

    #[test]
    fn test_sanitize_name_special_chars() {
        assert_eq!(sanitize_name("hello_world.v2"), "hello-world-v2");
    }

    #[test]
    fn test_sanitize_name_collapses_hyphens() {
        assert_eq!(sanitize_name("a--b---c"), "a-b-c");
    }

    #[test]
    fn test_sanitize_name_trims_hyphens() {
        assert_eq!(sanitize_name("-foo-"), "foo");
    }

    #[test]
    fn test_scan_finds_md_as_skills() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("review.md"),
            "---\nname: review\n---\nContent",
        )
        .unwrap();
        let discovered = scan_directory(dir.path()).unwrap();
        assert_eq!(discovered.len(), 1);
        assert!(matches!(discovered[0].kind, ArtifactKind::Skill));
    }

    #[test]
    fn test_scan_finds_hooks() {
        let dir = tempdir().unwrap();
        let hook_json = r#"[{"event": "before_tool", "command": "echo hi"}]"#;
        fs::write(dir.path().join("lint.json"), hook_json).unwrap();
        let discovered = scan_directory(dir.path()).unwrap();
        assert_eq!(discovered.len(), 1);
        assert!(matches!(discovered[0].kind, ArtifactKind::Hook));
    }

    #[test]
    fn test_scan_skips_non_hook_json() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("data.json"), r#"{"key": "value"}"#).unwrap();
        let discovered = scan_directory(dir.path()).unwrap();
        assert_eq!(discovered.len(), 0);
    }

    #[test]
    fn test_scan_skips_lockfile() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("skills-lock.json"), "{}").unwrap();
        let discovered = scan_directory(dir.path()).unwrap();
        assert_eq!(discovered.len(), 0);
    }

    #[test]
    fn test_scan_recognizes_agents_dir() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("agents")).unwrap();
        fs::write(dir.path().join("agents/helper.md"), "# Agent").unwrap();
        let discovered = scan_directory(dir.path()).unwrap();
        assert_eq!(discovered.len(), 1);
        assert!(matches!(discovered[0].kind, ArtifactKind::Agent));
        assert!(discovered[0].already_placed);
    }

    #[test]
    fn test_scan_skills_dir_already_placed() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("skills")).unwrap();
        fs::write(dir.path().join("skills/review.md"), "# Skill").unwrap();
        let discovered = scan_directory(dir.path()).unwrap();
        assert_eq!(discovered.len(), 1);
        assert!(discovered[0].already_placed);
    }

    #[test]
    fn test_reorganize_moves_files() {
        let dir = tempdir().unwrap();
        let md_path = dir.path().join("review.md");
        fs::write(&md_path, "content").unwrap();

        let discovered = vec![DiscoveredFile {
            original_path: md_path,
            kind: ArtifactKind::Skill,
            filename: "review.md".to_string(),
            already_placed: false,
        }];

        reorganize_files(dir.path(), &discovered).unwrap();
        assert!(dir.path().join("skills/review.md").exists());
        assert!(!dir.path().join("review.md").exists());
    }

    #[test]
    fn test_reorganize_skips_already_placed() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("skills")).unwrap();
        let md_path = dir.path().join("skills/review.md");
        fs::write(&md_path, "content").unwrap();

        let discovered = vec![DiscoveredFile {
            original_path: md_path.clone(),
            kind: ArtifactKind::Skill,
            filename: "review.md".to_string(),
            already_placed: true,
        }];

        reorganize_files(dir.path(), &discovered).unwrap();
        assert!(md_path.exists());
    }

    #[test]
    fn test_generate_manifest_creates_valid_json() {
        let dir = tempdir().unwrap();
        generate_manifest(dir.path()).unwrap();

        let manifest = Manifest::from_path(dir.path()).unwrap();
        assert!(manifest.validate().is_ok());
        assert!(manifest.name.starts_with("@migrated/"));
    }

    #[test]
    fn test_run_migrate_refuses_existing_package() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("renkei.json"), "{}").unwrap();
        fs::write(dir.path().join("review.md"), "# Skill").unwrap();

        let err = run_migrate(dir.path().to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("Already a Renkei package"));
    }

    #[test]
    fn test_run_migrate_refuses_empty_dir() {
        let dir = tempdir().unwrap();
        let err = run_migrate(dir.path().to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("Nothing to migrate"));
    }

    #[test]
    fn test_run_migrate_end_to_end() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("review.md"),
            "---\nname: review\n---\nReview code",
        )
        .unwrap();
        fs::write(
            dir.path().join("lint.json"),
            r#"[{"event": "before_tool", "command": "echo lint"}]"#,
        )
        .unwrap();

        run_migrate(dir.path().to_str().unwrap()).unwrap();

        assert!(dir.path().join("renkei.json").exists());
        assert!(dir.path().join("skills/review.md").exists());
        assert!(dir.path().join("hooks/lint.json").exists());

        let manifest = Manifest::from_path(dir.path()).unwrap();
        let validated = manifest.validate().unwrap();
        assert_eq!(validated.scope, "migrated");
    }
}
