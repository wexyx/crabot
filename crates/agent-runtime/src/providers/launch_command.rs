use std::path::{Path, PathBuf};

/// Parse a configured launcher without invoking a shell or expanding environment variables.
pub(crate) struct LaunchCommand {
    binary: PathBuf,
    arguments: Vec<String>,
}
impl LaunchCommand {
    pub(crate) fn parse(value: &Path) -> Result<Self, String> {
        // Preserve compatibility with an existing unquoted executable path containing spaces.
        if value.is_file() {
            return Ok(Self {
                binary: value.into(),
                arguments: vec![],
            });
        }
        let raw = value.to_str().ok_or("启动命令必须是 UTF-8")?;
        if raw.contains(['\n', '\r', '\0', '`']) || raw.contains("$(") {
            return Err("启动命令不支持换行、Shell 脚本或命令替换".into());
        }
        let mut words = shlex::split(raw).ok_or("启动命令引号未闭合")?.into_iter();
        let program = words
            .next()
            .filter(|s| !s.is_empty())
            .ok_or("启动命令不能为空")?;
        let arguments: Vec<_> = words.collect();
        if program.contains('=')
            || arguments
                .iter()
                .any(|s| matches!(s.as_str(), "|" | "||" | "&&" | ";" | ">" | ">>" | "<" | "&"))
        {
            return Err("启动命令不支持 Shell 管道、重定向或变量赋值；请使用可执行脚本封装".into());
        }
        Ok(Self {
            binary: program.into(),
            arguments,
        })
    }
    pub(crate) fn reject(&self, reserved: &[&str]) -> Result<(), String> {
        for arg in &self.arguments {
            let flag = arg.split('=').next().unwrap_or(arg);
            if reserved.contains(&flag) || flag == "--" {
                return Err(format!(
                    "启动命令参数 {flag} 由 Crabot 管理，请只填写程序及模型等启动选项"
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn binary(&self) -> &Path {
        &self.binary
    }
    pub(crate) fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_paths_quoted_arguments_and_invalid_shell_syntax() {
        let cmd = LaunchCommand::parse(Path::new("claude --model 'my model'")).unwrap();
        assert_eq!(cmd.binary(), Path::new("claude"));
        assert_eq!(cmd.arguments(), &["--model", "my model"]);
        let cmd = LaunchCommand::parse(Path::new("\"/a path/codex\" --model test")).unwrap();
        assert_eq!(cmd.binary(), Path::new("/a path/codex"));
        for bad in ["", "claude 'bad", "claude && echo hi", "claude $(echo hi)"] {
            assert!(LaunchCommand::parse(Path::new(bad)).is_err());
        }
        assert!(
            LaunchCommand::parse(Path::new("claude --permission-mode=bypassPermissions"))
                .unwrap()
                .reject(&["--permission-mode"])
                .is_err()
        );
    }
}
