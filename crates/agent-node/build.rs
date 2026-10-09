fn main() {
    println!("cargo:rerun-if-env-changed=CRABOT_RELEASE_VERSION");
    let version = match std::env::var("CRABOT_RELEASE_VERSION") {
        Ok(tag) => {
            let value = tag.strip_prefix('v').unwrap_or(&tag);
            assert!(
                !value.is_empty()
                    && value.len() <= 96
                    && value.as_bytes()[0].is_ascii_digit()
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b".-+_".contains(&b)),
                "CRABOT_RELEASE_VERSION must be a version tag, e.g. v1.2.3 or v1.2.3-rc.1"
            );
            format!("v{value}")
        }
        Err(std::env::VarError::NotPresent) => {
            format!("v{}-dev", std::env::var("CARGO_PKG_VERSION").unwrap())
        }
        Err(error) => panic!("invalid CRABOT_RELEASE_VERSION: {error}"),
    };
    println!("cargo:rustc-env=CRABOT_BUILD_VERSION={version}");
    // Ladybug extensions are dlopened and resolve engine symbols at load time. A Rust
    // binary does not export those by default, so an installed extension fails with
    // "symbol not found in flat namespace" the moment it calls back into the engine.
    println!("cargo:rustc-link-arg=-rdynamic");
}
