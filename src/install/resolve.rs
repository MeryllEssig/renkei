use std::collections::HashMap;

use crate::artifact::{Artifact, ArtifactKind};
use crate::conflict;
use crate::error::{RenkeiError, Result};
use crate::frontmatter;
use crate::install_cache::InstallCache;

use super::types::ConflictResolver;

/// Artifacts with renames applied, paired with optional original names.
/// Owns temp files whose lifetimes must span deployment.
pub(crate) struct ResolvedArtifacts {
    pub effective: Vec<(Artifact, Option<String>)>,
    _temp_files: Vec<tempfile::NamedTempFile>,
    _temp_dirs: Vec<tempfile::TempDir>,
}

/// Detect conflicts, resolve via the callback, apply renames + frontmatter rewrite.
/// Mutates `install_cache` to remove overwritten artifacts from previous owners on force.
pub(crate) fn resolve_conflicts_and_rename(
    artifacts: Vec<Artifact>,
    install_cache: &mut InstallCache,
    manifest_name: &str,
    conflict_resolver: &ConflictResolver,
) -> Result<ResolvedArtifacts> {
    let conflicts = conflict::detect_conflicts(&artifacts, install_cache, manifest_name);

    // Build rename map and clean up previous owners on force-overwrite
    let mut renames: HashMap<(ArtifactKind, String), String> = HashMap::new();
    for c in &conflicts {
        match conflict_resolver(c)? {
            None => {
                // Force overwrite: remove the artifact from the previous owner's cache
                if let Some(owner_entry) = install_cache.packages.get_mut(&c.owner_package) {
                    for deployment in owner_entry.deployed.values_mut() {
                        deployment.artifacts.retain(|a| {
                            !(a.artifact_type == c.artifact_kind && a.name == c.artifact_name)
                        });
                    }
                }
            }
            Some(new_name) => {
                renames.insert((c.artifact_kind.clone(), c.artifact_name.clone()), new_name);
            }
        }
    }

    // Build effective artifacts (apply renames).
    let mut temp_files: Vec<tempfile::NamedTempFile> = Vec::new();
    let mut temp_dirs: Vec<tempfile::TempDir> = Vec::new();
    let effective: Vec<(Artifact, Option<String>)> = artifacts
        .into_iter()
        .map(|art| {
            let key = (art.kind.clone(), art.name.clone());
            if let Some(new_name) = renames.get(&key) {
                if art.kind == ArtifactKind::Skill {
                    // Skills are directories: read SKILL.md, rewrite frontmatter,
                    // create a temp dir mirroring the structure
                    let skill_md_path = art.source_path.join("SKILL.md");
                    let content = std::fs::read_to_string(&skill_md_path).map_err(|e| {
                        RenkeiError::DeploymentFailed(format!(
                            "Cannot read {}: {e}",
                            skill_md_path.display()
                        ))
                    })?;
                    let rewritten = frontmatter::replace_frontmatter_name(&content, new_name)?;

                    let tmp_dir = tempfile::tempdir().map_err(|e| {
                        RenkeiError::DeploymentFailed(format!("Cannot create temp dir: {e}"))
                    })?;
                    std::fs::write(tmp_dir.path().join("SKILL.md"), rewritten).map_err(|e| {
                        RenkeiError::DeploymentFailed(format!("Cannot write temp SKILL.md: {e}"))
                    })?;

                    // Copy subdirectories from original
                    for entry in std::fs::read_dir(&art.source_path).map_err(|e| {
                        RenkeiError::DeploymentFailed(format!(
                            "Cannot read skill dir {}: {e}",
                            art.source_path.display()
                        ))
                    })? {
                        let entry = entry?;
                        if entry.path().is_dir() {
                            crate::backend::copy_dir_recursive(
                                &entry.path(),
                                &tmp_dir.path().join(entry.file_name()),
                            )?;
                        }
                    }

                    let original_name = art.name;
                    let renamed_artifact = Artifact {
                        kind: art.kind,
                        name: new_name.to_string(),
                        source_path: tmp_dir.path().to_path_buf(),
                    };
                    temp_dirs.push(tmp_dir);
                    Ok((renamed_artifact, Some(original_name)))
                } else {
                    // Flat file artifacts (agents, hooks)
                    let content = std::fs::read_to_string(&art.source_path).map_err(|e| {
                        RenkeiError::DeploymentFailed(format!(
                            "Cannot read {}: {e}",
                            art.source_path.display()
                        ))
                    })?;
                    let rewritten = frontmatter::replace_frontmatter_name(&content, new_name)?;

                    let mut tmp = tempfile::NamedTempFile::new().map_err(|e| {
                        RenkeiError::DeploymentFailed(format!("Cannot create temp file: {e}"))
                    })?;
                    std::io::Write::write_all(&mut tmp, rewritten.as_bytes()).map_err(|e| {
                        RenkeiError::DeploymentFailed(format!("Cannot write temp file: {e}"))
                    })?;

                    let original_name = art.name;
                    let renamed_artifact = Artifact {
                        kind: art.kind,
                        name: new_name.to_string(),
                        source_path: tmp.path().to_path_buf(),
                    };
                    temp_files.push(tmp);
                    Ok((renamed_artifact, Some(original_name)))
                }
            } else {
                Ok((art, None))
            }
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ResolvedArtifacts {
        effective,
        _temp_files: temp_files,
        _temp_dirs: temp_dirs,
    })
}
