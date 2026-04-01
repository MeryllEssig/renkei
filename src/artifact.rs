use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactKind {
    Skill,
    Agent,
    Hook,
}

impl ArtifactKind {
    pub fn dir_name(&self) -> &'static str {
        match self {
            ArtifactKind::Skill => "skills",
            ArtifactKind::Agent => "agents",
            ArtifactKind::Hook => "hooks",
        }
    }
}

impl fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactKind::Skill => write!(f, "skill"),
            ArtifactKind::Agent => write!(f, "agent"),
            ArtifactKind::Hook => write!(f, "hook"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub name: String,
    pub source_path: PathBuf,
}

pub fn discover_artifacts(package_dir: &Path) -> Result<Vec<Artifact>> {
    let mut artifacts = Vec::new();

    let scan_dirs: [(&str, ArtifactKind, &str); 3] = [
        ("skills", ArtifactKind::Skill, "md"),
        ("agents", ArtifactKind::Agent, "md"),
        ("hooks", ArtifactKind::Hook, "json"),
    ];

    for (dir_name, kind, file_ext) in &scan_dirs {
        let dir = package_dir.join(dir_name);
        if dir.is_dir() {
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == *file_ext) {
                    let name = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    artifacts.push(Artifact {
                        kind: kind.clone(),
                        name,
                        source_path: path,
                    });
                }
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
    fn test_discover_single_agent() {
        let dir = tempdir().unwrap();
        let agents = dir.path().join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(agents.join("deploy.md"), "# Deploy").unwrap();

        let artifacts = discover_artifacts(dir.path()).unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "deploy");
        assert_eq!(artifacts[0].kind, ArtifactKind::Agent);
    }

    #[test]
    fn test_discover_skill_and_agent() {
        let dir = tempdir().unwrap();
        let skills = dir.path().join("skills");
        let agents = dir.path().join("agents");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(skills.join("review.md"), "# Review").unwrap();
        std::fs::write(agents.join("deploy.md"), "# Deploy").unwrap();

        let artifacts = discover_artifacts(dir.path()).unwrap();
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].name, "deploy");
        assert_eq!(artifacts[0].kind, ArtifactKind::Agent);
        assert_eq!(artifacts[1].name, "review");
        assert_eq!(artifacts[1].kind, ArtifactKind::Skill);
    }

    #[test]
    fn test_discover_agents_ignores_non_md() {
        let dir = tempdir().unwrap();
        let agents = dir.path().join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(agents.join("notes.txt"), "not an agent").unwrap();
        std::fs::write(agents.join("deploy.md"), "# Deploy").unwrap();

        let artifacts = discover_artifacts(dir.path()).unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "deploy");
    }

    #[test]
    fn test_discover_empty_agents_dir() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("agents")).unwrap();
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

    #[test]
    fn test_discover_single_hook() {
        let dir = tempdir().unwrap();
        let hooks = dir.path().join("hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        std::fs::write(hooks.join("lint.json"), "[]").unwrap();

        let artifacts = discover_artifacts(dir.path()).unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "lint");
        assert_eq!(artifacts[0].kind, ArtifactKind::Hook);
    }

    #[test]
    fn test_discover_hooks_ignores_non_json() {
        let dir = tempdir().unwrap();
        let hooks = dir.path().join("hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        std::fs::write(hooks.join("lint.json"), "[]").unwrap();
        std::fs::write(hooks.join("notes.md"), "# Notes").unwrap();

        let artifacts = discover_artifacts(dir.path()).unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "lint");
    }

    #[test]
    fn test_discover_empty_hooks_dir() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("hooks")).unwrap();
        let artifacts = discover_artifacts(dir.path()).unwrap();
        assert!(artifacts.is_empty());
    }

    #[test]
    fn test_discover_skills_agents_and_hooks() {
        let dir = tempdir().unwrap();
        let skills = dir.path().join("skills");
        let agents = dir.path().join("agents");
        let hooks = dir.path().join("hooks");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::create_dir_all(&hooks).unwrap();
        std::fs::write(skills.join("review.md"), "# Review").unwrap();
        std::fs::write(agents.join("deploy.md"), "# Deploy").unwrap();
        std::fs::write(hooks.join("lint.json"), "[]").unwrap();

        let artifacts = discover_artifacts(dir.path()).unwrap();
        assert_eq!(artifacts.len(), 3);
        // sorted alphabetically: deploy, lint, review
        assert_eq!(artifacts[0].name, "deploy");
        assert_eq!(artifacts[0].kind, ArtifactKind::Agent);
        assert_eq!(artifacts[1].name, "lint");
        assert_eq!(artifacts[1].kind, ArtifactKind::Hook);
        assert_eq!(artifacts[2].name, "review");
        assert_eq!(artifacts[2].kind, ArtifactKind::Skill);
    }
}
