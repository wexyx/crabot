use std::path::PathBuf;

/// Resolve defaults independently of the directory used to launch Crabot.
pub fn user_home() -> PathBuf {
    std::env::home_dir()
        .filter(|path| path.is_absolute())
        .expect("Cannot determine user home directory; configure HOME or USERPROFILE")
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("CRABOT_DATA_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| user_home().join(".crabot"))
}

pub fn workdir() -> PathBuf {
    std::env::var_os("AGENT_WORKDIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(user_home)
}

/// Durable assistant notes belong to the instance, not the user's home root.
pub fn memory_file() -> PathBuf {
    data_dir().join("work").join("memory.md")
}
