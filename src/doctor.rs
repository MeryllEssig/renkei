use crate::config::Config;
use crate::error::Result;
use crate::install_cache::InstallCache;

pub fn run_doctor(config: &Config, global: bool) -> Result<bool> {
    let cache = InstallCache::load(config)?;
    let scope_label = if global { "global" } else { "project" };

    if cache.packages.is_empty() {
        println!("No packages installed ({scope_label}).");
        return Ok(true);
    }

    Ok(true)
}
