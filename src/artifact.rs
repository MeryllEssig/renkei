use std::path::{Path, PathBuf};

use crate::error::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum ArtifactKind {
    Skill,
}

#[derive(Debug, Clone)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub name: String,
    pub source_path: PathBuf,
}

pub fn discover_artifacts(package_dir: &Path) -> Result<Vec<Artifact>> {
    let mut artifacts = Vec::new();

    let skills_dir = package_dir.join("skills");
    if skills_dir.is_dir() {
        for entry in std::fs::read_dir(&skills_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                artifacts.push(Artifact {
                    kind: ArtifactKind::Skill,
                    name,
                    source_path: path,
                });
            }
        }
    }

    artifacts.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(artifacts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_discover_single_skill() {
        let dir = tempdir().unwrap();
        let skills = dir.path().join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(skills.join("review.md"), "# Review").unwrap();

        let artifacts = discover_artifacts(dir.path()).unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "review");
        assert_eq!(artifacts[0].kind, ArtifactKind::Skill);
    }

    #[test]
    fn test_discover_multiple_skills() {
        let dir = tempdir().unwrap();
        let skills = dir.path().join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(skills.join("review.md"), "# Review").unwrap();
        std::fs::write(skills.join("lint.md"), "# Lint").unwrap();

        let artifacts = discover_artifacts(dir.path()).unwrap();
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].name, "lint");
        assert_eq!(artifacts[1].name, "review");
    }

    #[test]
    fn test_discover_no_skills_dir() {
        let dir = tempdir().unwrap();
        let artifacts = discover_artifacts(dir.path()).unwrap();
        assert!(artifacts.is_empty());
    }

    #[test]
    fn test_discover_empty_skills_dir() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("skills")).unwrap();
        let artifacts = discover_artifacts(dir.path()).unwrap();
        assert!(artifacts.is_empty());
    }

    #[test]
    fn test_discover_ignores_non_md() {
        let dir = tempdir().unwrap();
        let skills = dir.path().join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(skills.join("notes.txt"), "not a skill").unwrap();
        std::fs::write(skills.join("review.md"), "# Review").unwrap();

        let artifacts = discover_artifacts(dir.path()).unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "review");
    }
}
