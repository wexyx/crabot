use lbug::SystemConfig;

const MIB: u64 = 1024 * 1024;
// This is a ceiling, not a preallocated 1 GiB slab. Large tool-result strings need
// workspace during column checkpoints in addition to the resident cached pages.
const DEFAULT_POOL_MIB: u64 = 1024;

/// Keep dirty history/WAL substantially below the pool, leaving room for indexes,
/// transaction-local columns and the checkpoint itself. WAL bytes are not the
/// same as the engine's in-memory footprint.
pub(super) fn configuration() -> Result<SystemConfig, String> {
    from_pool_setting(std::env::var("CRABOT_HISTORY_BUFFER_MIB").ok().as_deref())
}

fn from_pool_setting(value: Option<&str>) -> Result<SystemConfig, String> {
    let pool = match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => DEFAULT_POOL_MIB,
        Some(value) => value.parse::<u64>().map_err(|_| {
            "CRABOT_HISTORY_BUFFER_MIB must be an integer from 64 to 8192".to_string()
        })?,
    };
    if !(64..=8192).contains(&pool) {
        return Err("CRABOT_HISTORY_BUFFER_MIB must be an integer from 64 to 8192".into());
    }
    Ok(SystemConfig::default()
        .buffer_pool_size(pool * MIB)
        .max_db_size(1024 * MIB)
        .auto_checkpoint(true)
        .checkpoint_threshold(MIB as i64)
        .max_num_threads(2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_pool_configuration_without_mutating_process_environment() {
        for value in [None, Some(""), Some("64"), Some("256"), Some("8192")] {
            assert!(from_pool_setting(value).is_ok());
        }
        for value in ["0", "-1", "63", "8193", "unlimited"] {
            assert!(from_pool_setting(Some(value)).is_err());
        }
    }
}
