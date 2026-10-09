use std::path::PathBuf;
/// Parse before the Tokio runtime is created; startup policy is immutable thereafter.
pub(crate) fn configure() -> Result<Option<bool>, String> {
    let mut args = std::env::args().skip(1);
    let mut workdir = None;
    let mut outside = None;
    let mut interactive = false;
    let mut name = None;
    let mut data_dir = None;
    let mut web_port = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("Crabot {}", super::version::DISPLAY);
                return Ok(None);
            }
            "--cli" => interactive = true,
            "--name" => name = Some(args.next().ok_or("--name requires an instance alias")?),
            "--data-dir" => {
                data_dir = Some(PathBuf::from(
                    args.next().ok_or("--data-dir requires a path")?,
                ))
            }
            "--server-port" | "--web-port" => {
                web_port = Some(
                    args.next()
                        .ok_or("--web-port requires 0..65535")?
                        .parse::<u16>()
                        .map_err(|_| "invalid web port; expected 0..65535")?,
                )
            }
            "--workdir" => {
                workdir = Some(PathBuf::from(
                    args.next().ok_or("--workdir requires a directory")?,
                ))
            }
            "--outside-access" => {
                outside = Some(args.next().ok_or("--outside-access requires ask or deny")?)
            }
            "--help" | "-h" => {
                println!(
                    "agent-node [--version] [--cli] [--name ALIAS | --data-dir PATH] [--server-port PORT] [--workdir PATH] [--outside-access deny|ask]"
                );
                return Ok(None);
            }
            _ => return Err(format!("unknown startup option: {arg}")),
        }
    }
    if let Some(name) = &name {
        if name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(
                "instance name must be 1..64 letters, digits, underscores or hyphens".into(),
            );
        }
        if data_dir.is_some() {
            return Err("choose --name or --data-dir, not both".into());
        }
        data_dir = Some(agent_runtime::paths::user_home().join(format!(".crabot_{name}")));
    }
    super::startup_environment::StartupEnvironment::load(data_dir)?;
    let outside = outside
        .unwrap_or_else(|| std::env::var("AGENT_OUTSIDE_ACCESS").unwrap_or_else(|_| "deny".into()));
    if !matches!(outside.as_str(), "ask" | "deny") {
        return Err("outside-access must be ask or deny".into());
    }
    let root = super::startup_environment::absolute(
        &super::startup_environment::StartupEnvironment::launch_dir()?,
        workdir.unwrap_or_else(agent_runtime::paths::workdir),
    )
    .canonicalize()
    .map_err(|e| format!("invalid workdir: {e}"))?;
    if !root.is_dir() {
        return Err("workdir must be a directory".into());
    }
    // No application threads exist at this startup-only call site.
    unsafe {
        if let Some(name) = name {
            std::env::set_var("CRABOT_INSTANCE", name);
        }
        if let Some(port) = web_port {
            std::env::set_var("BIND_ADDR", format!("127.0.0.1:{port}"));
            std::env::set_var("CRABOT_WEB_PORT_EXPLICIT", "1");
        }
        std::env::set_var("AGENT_WORKDIR", root);
        std::env::set_var("AGENT_OUTSIDE_ACCESS", outside);
    }
    Ok(Some(interactive))
}
