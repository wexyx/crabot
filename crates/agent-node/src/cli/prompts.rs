use crate::core::Core;
use serde_json::json;

pub(super) async fn execute(core: &Core, args: &str) -> Result<String, String> {
    let data = core.system_prompts().await?;
    let rows = data["prompts"].as_array().ok_or("invalid prompt catalog")?;
    if args.trim().is_empty() || args.trim() == "list" {
        return Ok(format!(
            "系统提示词 · 覆盖目录 {}\n{}\n/prompts show ID · /prompts set ID 内容 · /prompts import ID 文件路径 · /prompts reset ID\n默认随版本更新，不复制到实例；保存才创建覆盖文件，reset 删除覆盖并恢复默认。",
            data["directory"].as_str().unwrap_or_default(),
            rows.iter()
                .map(|p| format!(
                    "  {} · {} · {}",
                    p["id"].as_str().unwrap_or_default(),
                    p["title"].as_str().unwrap_or_default(),
                    if p["overridden"] == true {
                        "实例覆盖"
                    } else {
                        "随版本默认"
                    }
                ))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    let (action, rest) = args
        .trim()
        .split_once(char::is_whitespace)
        .ok_or("用法：/prompts show|set|import|reset ID [内容/文件路径]")?;
    let (id, value) = rest
        .trim()
        .split_once(char::is_whitespace)
        .unwrap_or((rest.trim(), ""));
    let row = rows
        .iter()
        .find(|p| p["id"] == id)
        .ok_or("未知系统提示词；用 /prompts 查看名称")?;
    if action == "show" && value.is_empty() {
        return Ok(format!(
            "{} · 覆盖路径：{}\n{}",
            if row["overridden"] == true {
                "实例覆盖"
            } else {
                "随版本默认（尚无覆盖文件）"
            },
            row["path"].as_str().unwrap_or_default(),
            row["content"].as_str().unwrap_or_default()
        ));
    }
    if action == "reset" && value.is_empty() {
        core.save_system_prompt(json!({"id":id,"reset":true,"expected":row["content"]}))
            .await?;
        return Ok(format!(
            "已恢复 {id} 为随版本默认，实例覆盖文件已移除；下一次任务生效。"
        ));
    }
    let content = match action {
        "set" => value.to_owned(),
        "import" if !value.trim().is_empty() => {
            let path = value.trim();
            let meta = tokio::fs::metadata(path).await.map_err(|e| e.to_string())?;
            if !meta.is_file() || meta.len() > 128 * 1024 {
                return Err("请选择不超过 128 KiB 的 UTF-8 文本文件".into());
            }
            tokio::fs::read_to_string(path)
                .await
                .map_err(|e| e.to_string())?
        }
        _ => return Err("用法：/prompts show|set|import|reset ID [内容/文件路径]".into()),
    };
    core.save_system_prompt(json!({"id":id,"content":content,"expected":row["content"]}))
        .await?;
    Ok(format!(
        "已保存 {id}，下一次任务生效。文件：{}",
        row["path"].as_str().unwrap_or_default()
    ))
}
