use crate::{AppState, event};
use serde_json::json;
use uuid::Uuid;
tokio::task_local! { static NESTED: bool; static PLANNING: bool; static DISCUSSION: bool; }

pub(super) struct MemberEvents {
    state: AppState,
    project: Uuid,
    run: Option<Uuid>,
    invocation: Uuid,
    agent: String,
    role: String,
    planning: bool,
    discussion: Option<tokio::sync::Mutex<super::discussion_reply::DiscussionReply>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn discussion_yield_is_published_without_streaming_a_bubble() {
        let state = crate::tests::groups::state("discussion-stream").await;
        let project = Uuid::new_v4();
        let run = Uuid::new_v4();
        let (events, mut received) = tokio::sync::broadcast::channel(16);
        state.sessions.lock().await.insert(
            run,
            crate::Session {
                project_id: project,
                client_id: None,
                events,
                requests: Default::default(),
            },
        );
        let member = super::super::policies::Member {
            path: vec!["reviewer".into()],
            role: "review".into(),
        };
        super::super::project_execution::ProjectExecution::scope(
            "group".into(),
            run,
            serde_json::Value::Null,
            async {
                MemberEvents::discussion(async {
                    let output = MemberEvents::new(&state, project, &member);
                    for fragment in ["本轮", "让出", "。"] {
                        output.emit("agent.delta", fragment).await;
                        assert!(received.try_recv().is_err());
                    }
                    output.emit("agent.message", "本轮让出。").await;
                    let notice = received.try_recv().unwrap();
                    assert_eq!(notice.kind, "agent.yield");
                    assert_eq!(notice.data["agent"], "reviewer");
                    assert_eq!(notice.data["content"], "已让出本轮");
                    assert!(received.try_recv().is_err());
                })
                .await;
                // Other collaboration modes and normal chats retain ordinary text.
                MemberEvents::new(&state, project, &member)
                    .emit("agent.delta", "本轮让出。")
                    .await;
                assert_eq!(received.try_recv().unwrap().kind, "agent.delta");
            },
        )
        .await;
    }

    #[tokio::test]
    async fn reply_filter_only_applies_inside_discussion_scope() {
        let state = crate::tests::groups::state("discussion-notice").await;
        let project = Uuid::new_v4();
        let member = super::super::policies::Member {
            path: vec!["reviewer".into()],
            role: "review".into(),
        };
        assert!(
            MemberEvents::new(&state, project, &member)
                .discussion
                .is_none()
        );
        MemberEvents::discussion(async {
            assert!(
                MemberEvents::new(&state, project, &member)
                    .discussion
                    .is_some()
            );
        })
        .await;
        assert!(
            MemberEvents::new(&state, project, &member)
                .discussion
                .is_none()
        );
    }
}
impl MemberEvents {
    pub(super) fn new(state: &AppState, project: Uuid, member: &super::policies::Member) -> Self {
        Self {
            state: state.clone(),
            project,
            run: if NESTED.try_with(|v| *v).unwrap_or(false) {
                None
            } else {
                super::project_execution::ProjectExecution::run()
            },
            invocation: Uuid::new_v4(),
            agent: member.path.join("/"),
            role: member.role.clone(),
            planning: PLANNING.try_with(|v| *v).unwrap_or(false),
            discussion: DISCUSSION.try_with(|v| *v).unwrap_or(false).then(|| {
                tokio::sync::Mutex::new(super::discussion_reply::DiscussionReply::default())
            }),
        }
    }
    pub(super) async fn scope<F: std::future::Future>(&self, future: F) -> F::Output {
        NESTED.scope(true, future).await
    }
    pub(super) async fn planning<F: std::future::Future>(future: F) -> F::Output {
        PLANNING.scope(true, future).await
    }
    pub(super) async fn discussion<F: std::future::Future>(future: F) -> F::Output {
        DISCUSSION.scope(true, future).await
    }
    pub(super) async fn reset_discussion_reply(&self) {
        if let Some(reply) = &self.discussion {
            *reply.lock().await = super::discussion_reply::DiscussionReply::default();
        }
    }
    pub(super) async fn emit(&self, kind: &str, content: &str) {
        if self.run.is_none() {
            return;
        }
        if let Some(reply) = &self.discussion {
            let events = reply.lock().await.events(kind, content);
            for (kind, content) in events {
                self.publish(&kind, &content).await;
            }
        } else {
            self.publish(kind, content).await;
        }
    }
    async fn publish(&self, kind: &str, content: &str) {
        let Some(run) = self.run else { return };
        let kind = if self.planning && matches!(kind, "agent.delta" | "agent.message") {
            "agent.planning"
        } else {
            kind
        };
        let output = event(
            run,
            kind,
            json!({"message_id":run,"invocation_id":self.invocation,"agent":self.agent,"role":self.role,"content":content}),
        );
        if crate::persist_event(
            &self.state,
            self.project,
            &format!("session:{run}"),
            &output,
        )
        .await
        {
            if let Some(session) = self.state.sessions.lock().await.get(&run) {
                let _ = session.events.send(output);
            }
        }
    }
}
