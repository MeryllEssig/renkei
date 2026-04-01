use std::path::Path;

use tempfile::TempDir;

use crate::error::{RenkeiError, Result};

pub fn clone_repo(url: &str, tag: Option<&str>) -> Result<TempDir> {
    let tmp = TempDir::new().map_err(|e| RenkeiError::GitCloneFailed {
        url: url.to_string(),
        reason: format!("failed to create temp directory: {e}"),
    })?;

    let mut cmd = std::process::Command::new("git");
    cmd.args(["clone", "--depth", "1"]);
    if let Some(t) = tag {
        cmd.args(["--branch", t]);
    }
    cmd.arg(url);
    cmd.arg(tmp.path());
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::null());

    let output = cmd.output().map_err(|e| RenkeiError::GitCloneFailed {
        url: url.to_string(),
        reason: format!("failed to execute git: {e}"),
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RenkeiError::GitCloneFailed {
            url: url.to_string(),
            reason: stderr.trim().to_string(),
        });
    }

    Ok(tmp)
}

pub fn resolve_head(repo_path: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|e| RenkeiError::GitCloneFailed {
            url: repo_path.to_string_lossy().to_string(),
            reason: format!("failed to resolve HEAD: {e}"),
        })?;

    if !output.status.success() {
        return Err(RenkeiError::GitCloneFailed {
            url: repo_path.to_string_lossy().to_string(),
            reason: "git rev-parse HEAD failed".to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    /// Creates a local bare repo with a valid renkei package committed.
    /// Returns (bare_dir TempDir, file:// URL, commit SHA).
    fn setup_test_repo(tag: Option<&str>) -> (TempDir, String, String) {
        let bare = TempDir::new().unwrap();
        Command::new("git")
            .args(["init", "--bare"])
            .arg(bare.path())
            .output()
            .unwrap();

        let work = TempDir::new().unwrap();
        Command::new("git")
            .args(["clone"])
            .arg(bare.path())
            .arg(work.path())
            .output()
            .unwrap();

        // Configure git user for commits
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(work.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(work.path())
            .output()
            .unwrap();

        // Create a valid renkei package
        fs::write(
            work.path().join("renkei.json"),
            r#"{"name":"@test/git-pkg","version":"1.0.0","description":"test","author":"t","license":"MIT","backends":["claude"]}"#,
        ).unwrap();
        let skills = work.path().join("skills");
        fs::create_dir_all(&skills).unwrap();
        fs::write(skills.join("review.md"), "# Review skill").unwrap();

        Command::new("git")
            .args(["add", "."])
            .current_dir(work.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(work.path())
            .output()
            .unwrap();

        if let Some(t) = tag {
            Command::new("git")
                .args(["tag", t])
                .current_dir(work.path())
                .output()
                .unwrap();
        }

        // Detect branch name (main or master)
        let branch_output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(work.path())
            .output()
            .unwrap();
        let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

        Command::new("git")
            .args(["push", "origin", &branch, "--tags"])
            .current_dir(work.path())
            .output()
            .unwrap();

        // Get commit SHA
        let sha_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(work.path())
            .output()
            .unwrap();
        let sha = String::from_utf8_lossy(&sha_output.stdout).trim().to_string();

        let url = format!("file://{}", bare.path().display());
        (bare, url, sha)
    }

    #[test]
    fn test_clone_valid_repo() {
        let (_bare, url, _sha) = setup_test_repo(None);
        let tmp = clone_repo(&url, None).unwrap();
        assert!(tmp.path().join("renkei.json").exists());
        assert!(tmp.path().join("skills/review.md").exists());
    }

    #[test]
    fn test_clone_with_tag() {
        let (_bare, url, _sha) = setup_test_repo(Some("v1.0.0"));
        let tmp = clone_repo(&url, Some("v1.0.0")).unwrap();
        assert!(tmp.path().join("renkei.json").exists());
    }

    #[test]
    fn test_clone_invalid_url() {
        let result = clone_repo("file:///nonexistent/repo", None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Git clone failed"));
    }

    #[test]
    fn test_clone_nonexistent_tag() {
        let (_bare, url, _sha) = setup_test_repo(None);
        let result = clone_repo(&url, Some("nonexistent-tag"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Git clone failed"));
    }

    #[test]
    fn test_resolve_head_returns_sha() {
        let (_bare, url, expected_sha) = setup_test_repo(None);
        let tmp = clone_repo(&url, None).unwrap();
        let sha = resolve_head(tmp.path()).unwrap();
        assert_eq!(sha.len(), 40);
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(sha, expected_sha);
    }

    #[test]
    fn test_tempdir_cleanup_on_drop() {
        let (_bare, url, _sha) = setup_test_repo(None);
        let path;
        {
            let tmp = clone_repo(&url, None).unwrap();
            path = tmp.path().to_path_buf();
            assert!(path.exists());
        }
        assert!(!path.exists(), "TempDir should be cleaned up on drop");
    }
}
