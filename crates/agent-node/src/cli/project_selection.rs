use serde_json::Value;

#[derive(Clone)]
pub(super) struct ProjectPicker {
    rows: Vec<Value>,
    selected: usize,
}
impl ProjectPicker {
    pub(super) fn new(projects: &Value, current: Option<&str>) -> Option<Self> {
        let rows = projects.as_array()?.clone();
        let selected = rows
            .iter()
            .position(|row| row["key"].as_str() == current)
            .map(|i| i + 1)
            .unwrap_or(0);
        Some(Self { rows, selected })
    }
    pub(super) fn move_by(&mut self, forward: bool) {
        self.selected = if forward {
            (self.selected + 1) % (self.rows.len() + 1)
        } else {
            (self.selected + self.rows.len()) % (self.rows.len() + 1)
        };
    }
    pub(super) fn choose(&self, input: &str) -> Result<Option<String>, String> {
        let index = if input.trim().is_empty() {
            self.selected
        } else if let Ok(number) = input.trim().parse::<usize>() {
            number
                .checked_sub(1)
                .ok_or("聊天序号无效，请从列表选择。")?
        } else {
            return resolve(&Value::Array(self.rows.clone()), input.trim())
                .map(|row| Some(row["key"].as_str().unwrap_or_default().into()));
        };
        if index == 0 {
            return Ok(None);
        }
        let row = self
            .rows
            .get(index - 1)
            .ok_or("聊天序号无效，请从列表选择。")?;
        row["key"]
            .as_str()
            .map(|id| Some(id.to_owned()))
            .ok_or("项目缺少 ID".into())
    }
    fn label(row: &Value) -> String {
        let name = row["body"]["name"]
            .as_str()
            .filter(|v| !v.is_empty())
            .unwrap_or("未命名聊天");
        let id = row["key"].as_str().unwrap_or_default();
        format!("{name} · {}", id.chars().take(8).collect::<String>())
    }
    pub(super) fn menu(&self) -> String {
        let start = self
            .selected
            .saturating_sub(2)
            .min((self.rows.len() + 1).saturating_sub(5));
        let mut lines = vec!["选择聊天".into()];
        let items =
            std::iter::once("＋ 新建聊天".to_owned()).chain(self.rows.iter().map(Self::label));
        lines.extend(
            items.enumerate().skip(start).take(5).map(|(i, label)| {
                format!("{} {}", if i == self.selected { "›" } else { " " }, label)
            }),
        );
        lines.push(format!(
            "↑/↓ 选择 · Enter 进入 · Esc 取消 · {}/{}",
            self.selected + 1,
            self.rows.len() + 1
        ));
        lines.join("\n")
    }
    pub(super) fn numbered(&self) -> String {
        let mut lines = vec!["选择聊天 · 输入序号进入，回车进入选中项，/cancel 取消".into()];
        lines.push("  1) ＋ 新建聊天".into());
        lines.extend(self.rows.iter().enumerate().map(|(i, row)| {
            format!(
                "  {}) {} · {}",
                i + 2,
                Self::label(row),
                row["key"].as_str().unwrap_or_default()
            )
        }));
        lines.join("\n")
    }
}

pub(super) fn resolve<'a>(projects: &'a Value, query: &str) -> Result<&'a Value, String> {
    let rows = projects.as_array().ok_or("项目列表格式错误")?;
    if let Some(row) = rows.iter().find(|row| row["key"].as_str() == Some(query)) {
        return Ok(row);
    }
    let matches: Vec<_> = rows
        .iter()
        .filter(|row| {
            row["body"]["name"].as_str() == Some(query)
                || (!query.is_empty()
                    && row["key"]
                        .as_str()
                        .is_some_and(|key| key.starts_with(query)))
        })
        .collect();
    match matches.as_slice() {
        [row] => Ok(row),
        [] => Err(format!(
            "找不到项目「{query}」；用 /chat 查看项目 ID 或名称。"
        )),
        _ => Err(format!(
            "「{query}」匹配多个项目，请使用更完整的 ID；/chat 查看列表。"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn chat_picker_selects_without_needing_ids() {
        let rows = json!([{"key":"a","body":{"name":"One"}},{"key":"b","body":{"name":"Two"}}]);
        let mut picker = ProjectPicker::new(&rows, Some("b")).unwrap();
        assert_eq!(picker.choose("").unwrap(), Some("b".into()));
        picker.move_by(true);
        assert_eq!(picker.choose("").unwrap(), None);
        picker.move_by(true);
        assert_eq!(picker.choose("").unwrap(), Some("a".into()));
        assert_eq!(picker.choose("3").unwrap(), Some("b".into()));
        assert_eq!(picker.choose("Two").unwrap(), Some("b".into()));
        assert!(picker.choose("0").is_err());
        assert!(picker.menu().contains("› One"));
        assert_eq!(
            ProjectPicker::new(&json!([]), None)
                .unwrap()
                .choose("")
                .unwrap(),
            None
        );
    }
    #[test]
    fn selects_exact_id_name_or_unique_prefix_without_guessing() {
        let rows = json!([
            {"key":"abcd-1","body":{"name":"开发 项目"}},
            {"key":"abcd-2","body":{"name":"同名"}},
            {"key":"xyz","body":{"name":"同名"}}
        ]);
        for query in ["abcd-1", "开发 项目"] {
            assert_eq!(resolve(&rows, query).unwrap()["key"], "abcd-1");
        }
        assert_eq!(resolve(&rows, "xy").unwrap()["key"], "xyz");
        for query in ["", "abcd", "同名", "missing"] {
            assert!(resolve(&rows, query).is_err());
        }
    }
}
