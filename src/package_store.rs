use std::collections::HashMap;

use crate::config::Config;
use crate::error::Result;
use crate::install_cache::{InstallCache, PackageEntry};
use crate::lockfile::{Lockfile, LockfileEntry};

pub struct PackageStore {
    cache: InstallCache,
    lockfile: Lockfile,
}

impl PackageStore {
    /// Load both install cache and lockfile. Missing lockfile is tolerated.
    pub fn load(config: &Config) -> Result<Self> {
        let cache = InstallCache::load(config)?;
        let lockfile = Lockfile::load(&config.lockfile_path())?;
        Ok(Self { cache, lockfile })
    }

    /// Save both files to disk.
    pub fn save(&self, config: &Config) -> Result<()> {
        self.cache.save(config)?;
        self.lockfile.save(&config.lockfile_path())?;
        Ok(())
    }

    /// Record an install: upsert into both cache and lockfile.
    pub fn record_install(&mut self, name: &str, entry: PackageEntry) {
        self.cache.upsert_package(name, entry);
        let pkg = self.cache.packages.get(name).unwrap();
        self.lockfile
            .upsert(name, LockfileEntry::from_package_entry(pkg));
    }

    /// Record an install from lockfile replay: upsert into cache only.
    /// Skips lockfile update to avoid cycles during `install_from_lockfile`.
    pub fn record_install_from_lockfile(&mut self, name: &str, entry: PackageEntry) {
        self.cache.upsert_package(name, entry);
    }

    /// Remove a package from both cache and lockfile.
    pub fn remove(&mut self, name: &str) {
        self.cache.packages.remove(name);
        self.lockfile.remove(name);
    }

    /// Get a package entry by name.
    #[allow(dead_code)]
    pub fn get(&self, name: &str) -> Option<&PackageEntry> {
        self.cache.packages.get(name)
    }

    /// Check if a package exists.
    pub fn contains(&self, name: &str) -> bool {
        self.cache.packages.contains_key(name)
    }

    /// Access all packages (read-only).
    pub fn packages(&self) -> &HashMap<String, PackageEntry> {
        &self.cache.packages
    }

    /// Access the lockfile (read-only, for install_from_lockfile).
    #[allow(dead_code)]
    pub fn lockfile(&self) -> &Lockfile {
        &self.lockfile
    }

    /// Access the install cache (read-only, for cleanup/doctor/list).
    pub fn cache(&self) -> &InstallCache {
        &self.cache
    }

    /// Access the install cache mutably (for conflict resolution on force-overwrite).
    pub fn cache_mut(&mut self) -> &mut InstallCache {
        &mut self.cache
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install_cache::BackendDeployment;
    use tempfile::tempdir;

    fn make_config(dir: &std::path::Path) -> Config {
        Config::with_home_dir(dir.to_path_buf())
    }

    fn make_entry(version: &str, integrity: &str) -> PackageEntry {
        PackageEntry {
            version: version.to_string(),
            source: "local".to_string(),
            source_path: "/tmp/pkg".to_string(),
            integrity: integrity.to_string(),
            archive_path: "/tmp/a.tar.gz".to_string(),
            deployed: {
                let mut m = HashMap::new();
                m.insert(
                    "claude".to_string(),
                    BackendDeployment {
                        artifacts: vec![],
                        mcp_servers: vec![],
                    },
                );
                m
            },
            resolved: None,
            tag: None,
            member: None,
        }
    }

    #[test]
    fn test_load_empty() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path());
        let store = PackageStore::load(&config).unwrap();
        assert!(store.packages().is_empty());
        assert!(store.lockfile().packages.is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path());

        let mut store = PackageStore::load(&config).unwrap();
        store.record_install("@test/pkg", make_entry("1.0.0", "abc123"));
        store.save(&config).unwrap();

        let loaded = PackageStore::load(&config).unwrap();
        assert!(loaded.contains("@test/pkg"));
        assert_eq!(loaded.get("@test/pkg").unwrap().version, "1.0.0");
        assert!(loaded.lockfile().packages.contains_key("@test/pkg"));
    }

    #[test]
    fn test_record_install_writes_both() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path());

        let mut store = PackageStore::load(&config).unwrap();
        store.record_install("@test/pkg", make_entry("2.0.0", "def456"));

        // Cache has entry
        assert!(store.contains("@test/pkg"));
        assert_eq!(store.get("@test/pkg").unwrap().version, "2.0.0");

        // Lockfile has entry with sha256- prefix
        let lf_entry = store.lockfile().packages.get("@test/pkg").unwrap();
        assert_eq!(lf_entry.version, "2.0.0");
        assert_eq!(lf_entry.integrity, "sha256-def456");
    }

    #[test]
    fn test_record_install_from_lockfile_skips_lockfile() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path());

        let mut store = PackageStore::load(&config).unwrap();
        store.record_install_from_lockfile("@test/pkg", make_entry("1.0.0", "abc"));

        // Cache has entry
        assert!(store.contains("@test/pkg"));

        // Lockfile does NOT have entry
        assert!(!store.lockfile().packages.contains_key("@test/pkg"));
    }

    #[test]
    fn test_remove_cleans_both() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path());

        let mut store = PackageStore::load(&config).unwrap();
        store.record_install("@test/pkg", make_entry("1.0.0", "abc"));
        assert!(store.contains("@test/pkg"));

        store.remove("@test/pkg");
        assert!(!store.contains("@test/pkg"));
        assert!(!store.lockfile().packages.contains_key("@test/pkg"));
    }

    #[test]
    fn test_remove_tolerates_missing_lockfile_entry() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path());

        let mut store = PackageStore::load(&config).unwrap();
        // Record with from_lockfile=true so lockfile has no entry
        store.record_install_from_lockfile("@test/pkg", make_entry("1.0.0", "abc"));

        // Remove should not panic
        store.remove("@test/pkg");
        assert!(!store.contains("@test/pkg"));
    }

    #[test]
    fn test_packages_iteration() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path());

        let mut store = PackageStore::load(&config).unwrap();
        store.record_install("@test/a", make_entry("1.0.0", "aaa"));
        store.record_install("@test/b", make_entry("2.0.0", "bbb"));

        assert_eq!(store.packages().len(), 2);
        assert!(store.packages().contains_key("@test/a"));
        assert!(store.packages().contains_key("@test/b"));
    }

    #[test]
    fn test_cache_mut_allows_modification() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path());

        let mut store = PackageStore::load(&config).unwrap();
        store.record_install("@test/pkg", make_entry("1.0.0", "abc"));

        // Modify via cache_mut
        store.cache_mut().packages.remove("@test/pkg");
        assert!(!store.contains("@test/pkg"));
    }

    #[test]
    fn test_save_persists_both_files() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path());

        let mut store = PackageStore::load(&config).unwrap();
        store.record_install("@test/pkg", make_entry("1.0.0", "hash"));
        store.save(&config).unwrap();

        // Verify both files exist
        assert!(config.install_cache_path().exists());
        assert!(config.lockfile_path().exists());

        // Verify lockfile content
        let lf_content = std::fs::read_to_string(config.lockfile_path()).unwrap();
        let lf: serde_json::Value = serde_json::from_str(&lf_content).unwrap();
        assert_eq!(lf["packages"]["@test/pkg"]["integrity"], "sha256-hash");
    }
}
