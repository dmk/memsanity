use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::schema::SuiteConfig;

pub fn load_suite_from_path<P: AsRef<Path>>(path: P) -> Result<SuiteConfig> {
    let path_ref = path.as_ref();
    let data = fs::read_to_string(path_ref)
        .with_context(|| format!("Failed to read YAML at {}", path_ref.display()))?;
    let suite: SuiteConfig = serde_yaml::from_str(&data)
        .with_context(|| format!("Failed to parse YAML at {}", path_ref.display()))?;
    Ok(suite)
}
