use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
use tokio::{process::Command, sync::watch};

fn state() -> &'static watch::Sender<String> {
    static STATE: OnceLock<watch::Sender<String>> = OnceLock::new();
    STATE.get_or_init(|| watch::channel(String::new()).0)
}
pub(super) fn subscribe() -> watch::Receiver<String> {
    state().subscribe()
}
pub(super) async fn check() {
    let text = match super::update_check::check().await {
        Ok(Some(tag)) => format!("有更新 {tag} · /update"),
        Ok(None) => "已是最新版本".into(),
        Err(_) => return,
    };
    state().send_if_modified(|current| {
        if current.is_empty() {
            *current = text;
            true
        } else {
            false
        }
    });
}

fn installation(exe: &Path) -> Result<(PathBuf, Option<PathBuf>), String> {
    let parent = exe.parent().ok_or("cannot locate current executable")?;
    let script = parent.join("install.sh");
    if script.is_file() {
        let bundle = parent.parent().ok_or("cannot locate bundle")?;
        let releases = bundle.parent().ok_or("cannot locate release directory")?;
        let prefix = (releases.file_name().is_some_and(|v| v == "releases")
            && releases
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|v| v == "crabot")
            && releases
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .is_some_and(|v| v == "share"))
        .then(|| {
            releases
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .to_path_buf()
        });
        return Ok((script, prefix));
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install.sh");
    if source.is_file() {
        return Ok((source, None));
    }
    Err("此安装包缺少更新器，请重新运行安装命令。".into())
}

struct ProcessGroup(u32);
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let _ = std::process::Command::new("/bin/kill")
                .args(["-KILL", "--", &format!("-{}", self.0)])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }
}
pub(super) async fn install() -> Result<String, String> {
    state().send_replace("正在下载更新…".into());
    let result = install_inner().await;
    state().send_replace(if result.is_ok() {
        "更新已安装 · 请重启 Crabot".into()
    } else {
        "更新失败 · /update 重试".into()
    });
    result
}
async fn install_inner() -> Result<String, String> {
    let (script, prefix) = installation(&std::env::current_exe().map_err(|e| e.to_string())?)?;
    run_installer(script, prefix).await
}
async fn run_installer(script: PathBuf, prefix: Option<PathBuf>) -> Result<String, String> {
    let mut command = Command::new("bash");
    command
        .arg(script)
        .env("CRABOT_VERSION", "latest")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    if let Some(prefix) = prefix {
        command.env("CRABOT_INSTALL_PREFIX", prefix);
    }
    #[cfg(unix)]
    command.process_group(0);
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let child = command.spawn().map_err(|e| e.to_string())?;
    let _group = ProcessGroup(child.id().ok_or("missing updater process ID")?);
    let output = child.wait_with_output().await.map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(
            "更新失败，当前版本与会话保持不变；请检查网络或 Release 安装包后 /update 重试。".into(),
        );
    }
    Ok("更新已安装。配置和聊天记录已保留；请退出后重启 Crabot 使用新版本。".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn installed_updater_uses_its_own_prefix_and_latest_without_exiting() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("custom prefix");
        let libexec = prefix.join("share/crabot/releases/v1.0.0-test/libexec");
        std::fs::create_dir_all(&libexec).unwrap();
        let script = libexec.join("install.sh");
        std::fs::write(&script, "test \"$CRABOT_VERSION\" = latest && test -d \"$CRABOT_INSTALL_PREFIX/share/crabot/releases\"").unwrap();
        let (found, target) = installation(&libexec.join("agent-node")).unwrap();
        assert_eq!(target.as_deref(), Some(prefix.as_path()));
        assert!(run_installer(found, target).await.unwrap().contains("重启"));
        std::fs::write(&script, "exit 7").unwrap();
        assert!(run_installer(script, Some(prefix)).await.is_err());
    }
}
