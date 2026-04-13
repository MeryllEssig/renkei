mod checks;
mod report;
mod types;

#[cfg(test)]
mod tests;

pub use types::DoctorReport;

use crate::backend::BackendRegistry;
use crate::config::{BackendId, Config};
use crate::error::Result;
use crate::package_store::PackageStore;

pub fn run_doctor(config: &Config, registry: &BackendRegistry) -> Result<bool> {
    let store = PackageStore::load(config)?;
    let scope_label = config.scope_label();

    if store.packages().is_empty() {
        println!("No packages installed ({scope_label}).");
        return Ok(true);
    }

    let detected = registry.detect(config);
    let backend_ok = !detected.is_empty();
    let backend_statuses = registry.status(config);

    let claude_dirs = config.backend(BackendId::Claude);
    let settings = crate::json_file::read_json_or_empty(&claude_dirs.settings_path.unwrap())?;
    let claude_config = crate::json_file::read_json_or_empty(&claude_dirs.config_path.unwrap())?;

    let mut packages: Vec<_> = store
        .packages()
        .iter()
        .map(|(name, entry)| (name.as_str(), entry))
        .collect();
    packages.sort_by_key(|(name, _)| *name);

    let report = DoctorReport::build(
        &packages,
        &settings,
        &claude_config,
        backend_ok,
        store.cache(),
        config,
    );
    let output = report.format(scope_label, &backend_statuses);
    print!("{}", output);

    Ok(report.is_healthy())
}
