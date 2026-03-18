//! Schema versioning and migration for config files.

use anyhow::{Result, bail};

/// The current config schema version.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Registry of migration functions.
///
/// Index 0 = v1→v2, index 1 = v2→v3, etc.
/// Currently empty since we're at v1.
fn migrations() -> Vec<fn(toml::Value) -> Result<toml::Value>> {
    vec![]
}

/// Migrate a raw TOML config value to the current schema version.
///
/// - Missing `schema_version` is treated as v1.
/// - Future versions (> current) are rejected.
/// - Applies sequential migrations from file version to current.
///
/// # Errors
///
/// Returns an error if the config has a future schema version or if any
/// migration step fails.
pub fn migrate(mut raw: toml::Value) -> Result<toml::Value> {
    let file_version = raw
        .get("schema_version")
        .and_then(toml::Value::as_integer)
        .map_or(1, |v| u32::try_from(v).unwrap_or(1));

    if file_version > CURRENT_SCHEMA_VERSION {
        bail!(
            "config schema_version {file_version} is from a newer version of Nux \
             (current: {CURRENT_SCHEMA_VERSION})"
        );
    }

    let registry = migrations();

    for version in file_version..CURRENT_SCHEMA_VERSION {
        let idx = (version - 1) as usize;
        let migration_fn = registry.get(idx).ok_or_else(|| {
            anyhow::anyhow!(
                "missing migration function for v{version} to v{}",
                version + 1
            )
        })?;
        raw = migration_fn(raw).map_err(|e| {
            anyhow::anyhow!("migration from v{version} to v{} failed: {e}", version + 1)
        })?;
    }

    // Update schema_version to current.
    if let Some(table) = raw.as_table_mut() {
        table.insert(
            "schema_version".to_owned(),
            toml::Value::Integer(i64::from(CURRENT_SCHEMA_VERSION)),
        );
    }

    Ok(raw)
}
