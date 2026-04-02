use std::fmt;
use std::path::{Path, PathBuf};

use crate::artifact::{self, Artifact};
use crate::backend::Backend;
use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::manifest::{Manifest, ValidatedManifest};
use crate::package_store::PackageStore;

use super::cleanup::cleanup_previous_installation;
use super::deploy::{self, DeploymentResult};
use super::resolve::{self, ResolvedArtifacts};
use super::types::ConflictResolver;

/// Shared pipeline state for both `install_local` and `install_from_lock_entry`.
///
/// Holds the validated manifest, raw manifest, resolved backends, and discovered
/// artifacts. The three methods mirror the install phases: discover → resolve → deploy.
pub(crate) struct CorePipeline<'a> {
    pub manifest: ValidatedManifest,
    pub raw_manifest: Manifest,
    pub active_backends: Vec<&'a dyn Backend>,
    /// Canonical package directory (resolved in discover).
    pub package_dir: PathBuf,
    artifacts: Vec<Artifact>,
}

impl fmt::Debug for CorePipeline<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CorePipeline")
            .field("manifest", &self.manifest.full_name)
            .field("backends", &self.active_backends.len())
            .field("artifacts", &self.artifacts.len())
            .finish()
    }
}

impl<'a> CorePipeline<'a> {
    /// Phase 1: Load manifest, filter backends, discover artifacts.
    pub fn discover(
        package_dir: &Path,
        backends: &'a [&'a dyn Backend],
        force: bool,
    ) -> Result<Self> {
        let package_dir = package_dir
            .canonicalize()
            .map_err(|_| RenkeiError::ManifestNotFound(package_dir.to_path_buf()))?;

        let raw_manifest = Manifest::from_path(&package_dir)?;
        let manifest = raw_manifest.validate()?;

        let active_backends: Vec<&dyn Backend> = if force {
            backends.to_vec()
        } else {
            backends
                .iter()
                .filter(|b| manifest.backends.iter().any(|mb| mb == b.name()))
                .copied()
                .collect()
        };

        if active_backends.is_empty() {
            let detected_names: Vec<&str> = backends.iter().map(|b| b.name()).collect();
            return Err(RenkeiError::BackendNotDetected {
                required: manifest.backends.join(", "),
                detected: if detected_names.is_empty() {
                    "none".to_string()
                } else {
                    detected_names.join(", ")
                },
            });
        }

        let artifacts = artifact::discover_artifacts(&package_dir)?;
        if artifacts.is_empty() {
            return Err(RenkeiError::NoArtifactsFound(package_dir));
        }

        Ok(Self {
            manifest,
            raw_manifest,
            active_backends,
            package_dir,
            artifacts,
        })
    }

    /// Phase 2: Cleanup previous installation and resolve conflicts.
    /// Consumes `self` because `resolve_conflicts_and_rename` takes artifacts by value.
    /// Returns a `ResolvedPipeline` ready for deployment.
    pub fn cleanup_and_resolve(
        self,
        store: &mut PackageStore,
        conflict_resolver: &ConflictResolver,
        config: &Config,
    ) -> Result<ResolvedPipeline<'a>> {
        cleanup_previous_installation(&self.manifest.full_name, store.cache(), config);

        let resolved = resolve::resolve_conflicts_and_rename(
            self.artifacts,
            store.cache_mut(),
            &self.manifest.full_name,
            conflict_resolver,
        )?;

        Ok(ResolvedPipeline {
            manifest: self.manifest,
            raw_manifest: self.raw_manifest,
            active_backends: self.active_backends,
            package_dir: self.package_dir,
            resolved,
        })
    }
}

/// Pipeline after conflict resolution, ready for deployment.
pub(crate) struct ResolvedPipeline<'a> {
    pub manifest: ValidatedManifest,
    pub raw_manifest: Manifest,
    pub active_backends: Vec<&'a dyn Backend>,
    pub package_dir: PathBuf,
    pub resolved: ResolvedArtifacts,
}

impl<'a> ResolvedPipeline<'a> {
    /// Phase 3: Deploy artifacts to all active backends.
    pub fn deploy(&self, config: &Config) -> Result<DeploymentResult> {
        deploy::deploy_to_backends(
            &self.resolved.effective,
            &self.active_backends,
            &self.raw_manifest,
            config,
        )
    }
}
