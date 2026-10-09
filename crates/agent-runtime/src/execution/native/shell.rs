use super::{NativeCommand, ProcessGroup};
use crate::execution::profile::Profile;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;
pub(crate) async fn execute(
    root: PathBuf,
    script: String,
    profile: Profile,
    environment: std::collections::BTreeMap<String, String>,
) -> Result<Value, String> {
    let execution = NativeCommand::new(&root)?;
    let mut command = execution.command(Path::new("/bin/sh"), &root)?;
    command.arg("-c").arg(script);
    command.envs(environment);
    for (key, value) in profile.secrets()? {
        command.env(key, value);
    }
    #[cfg(unix)]
    unsafe {
        let seconds = profile.timeout_seconds;
        command.pre_exec(move || {
            for (resource, limit) in [(libc::RLIMIT_CPU, seconds), (libc::RLIMIT_NOFILE, 256)] {
                let limit = libc::rlimit {
                    rlim_cur: limit as libc::rlim_t,
                    rlim_max: limit as libc::rlim_t,
                };
                if libc::setrlimit(resource, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("command launch failed: {e}"))?;
    let _group = ProcessGroup::new(child.id().ok_or("missing process ID")?);
    let stdout = child.stdout.take().ok_or("missing stdout")?;
    let stderr = child.stderr.take().ok_or("missing stderr")?;
    let (status, stdout, stderr) = tokio::try_join!(
        async { child.wait().await.map_err(|e| e.to_string()) },
        read(stdout),
        read(stderr)
    )?;
    Ok(
        json!({"success":status.success(),"exit_code":status.code(),"stdout":stdout,"stderr":stderr,"workdir":root,"execution":"host","network":"host"}),
    )
}
async fn read(mut reader: impl tokio::io::AsyncRead + Unpin) -> Result<String, String> {
    let mut bytes = vec![];
    reader
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
