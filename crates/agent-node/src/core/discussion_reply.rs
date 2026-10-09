use super::{discussion::YIELD, member_response::Participation};

/// Hold only a possible control reply so it never flashes as an assistant bubble.
#[derive(Default)]
pub(super) struct DiscussionReply {
    pending: String,
    released: bool,
}

impl DiscussionReply {
    pub(super) fn events(&mut self, kind: &str, content: &str) -> Vec<(String, String)> {
        if kind == "agent.delta" && !self.released {
            self.pending.push_str(content);
            let candidate = self.pending.trim_start();
            if "本轮让出。原因：".starts_with(candidate) || Participation::is_yield(candidate)
            {
                return vec![];
            }
            self.released = true;
            return vec![(kind.into(), std::mem::take(&mut self.pending))];
        }
        if kind == "agent.message" {
            self.pending.clear();
            if !self.released && content.trim() == YIELD {
                return vec![("agent.yield".into(), "已让出本轮".into())];
            }
        }
        let mut events = Vec::new();
        // Preserve partial output on an error or before a tool boundary.
        if matches!(kind, "agent.member.error" | "agent.tool.started") && !self.pending.is_empty() {
            self.released = true;
            events.push(("agent.delta".into(), std::mem::take(&mut self.pending)));
        }
        events.push((kind.into(), content.into()));
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_yield_becomes_one_notice_without_any_chat_fragments() {
        let mut reply = DiscussionReply::default();
        for fragment in [" \n", "本", "轮让", "出", "。", "\n"] {
            assert!(reply.events("agent.delta", fragment).is_empty());
        }
        assert_eq!(
            reply.events("agent.message", " 本轮让出。\n"),
            vec![("agent.yield".into(), "已让出本轮".into())]
        );
        assert_eq!(
            DiscussionReply::default().events("agent.message", YIELD)[0].0,
            "agent.yield"
        );
    }

    #[test]
    fn yield_reasons_are_withheld_until_the_scheduler_accepts_them() {
        let mut reply = DiscussionReply::default();
        for fragment in [
            "本轮",
            "让出。",
            "原",
            "因：",
            "当前是翻译任务，我负责部署。",
        ] {
            assert!(reply.events("agent.delta", fragment).is_empty());
        }
    }

    #[test]
    fn ordinary_replies_and_quotes_are_preserved() {
        for text in [
            "本轮让出。但我还有意见",
            "“本轮让出。”是什么意思？",
            "本轮让我来处理",
            "正常回答",
        ] {
            let mut reply = DiscussionReply::default();
            let streamed: String = text
                .chars()
                .flat_map(|c| reply.events("agent.delta", &c.to_string()))
                .map(|(_, text)| text)
                .collect();
            assert_eq!(streamed, text);
            assert_eq!(
                reply.events("agent.message", text),
                vec![("agent.message".into(), text.into())]
            );
        }
    }

    #[test]
    fn incomplete_prefix_and_errors_do_not_lose_text() {
        let mut reply = DiscussionReply::default();
        assert!(reply.events("agent.delta", "本轮").is_empty());
        assert_eq!(
            reply.events("agent.member.error", "interrupted"),
            vec![
                ("agent.delta".into(), "本轮".into()),
                ("agent.member.error".into(), "interrupted".into())
            ]
        );
        let mut reply = DiscussionReply::default();
        assert!(reply.events("agent.delta", "本轮").is_empty());
        assert_eq!(
            reply.events("agent.message", "本轮"),
            vec![("agent.message".into(), "本轮".into())]
        );
    }
}
