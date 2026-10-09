use super::{
    discussion::YIELD,
    member_events::MemberEvents,
    policies::{Dispatch, Member, run_member_inner},
};
use crate::AppState;
use uuid::Uuid;

const REASON: &str = "本轮让出。原因：";

/// Validate participation before publishing a yield or counting it toward consensus.
pub(super) struct Participation {
    required: bool,
}

impl Participation {
    pub(super) fn new(required: bool) -> Self {
        Self { required }
    }

    pub(super) fn is_yield(answer: &str) -> bool {
        let answer = answer.trim();
        answer == YIELD || answer.starts_with(REASON)
    }

    pub(super) fn reason(answer: &str) -> Option<&str> {
        answer
            .trim()
            .strip_prefix(REASON)
            .map(str::trim)
            .filter(|reason| {
                !reason.is_empty() && reason.chars().count() <= 160 && !reason.contains('\n')
            })
    }

    fn accept_yield(&self, answer: &str, rechecked: bool) -> bool {
        !self.required && rechecked && Self::reason(answer).is_some()
    }

    fn instructions(&self) -> Result<String, String> {
        agent_runtime::prompts::PromptStore::instance().read(if self.required {
            "addressed"
        } else {
            "participation"
        })
    }

    fn correction(&self) -> Result<String, String> {
        let action = if self.required {
            "No reason to yield will be accepted this turn. Answer directly, perform the task, or explain the specific blocker.".to_owned()
        } else {
            format!(
                "Only if you truly meet the conditions to yield, reply with exactly: {REASON}<one specific reason, at most 160 characters, on one line, in the user's language>. Otherwise, perform the task or explain the specific blocker."
            )
        };
        let recheck =
            agent_runtime::prompts::PromptStore::instance().read("participation-recheck")?;
        Ok(format!(
            "\n{recheck}\n{}\n{action} Do not reply with only the bare control signal: {YIELD}",
            self.instructions()?
        ))
    }
}

pub(super) async fn run(
    state: &AppState,
    project: Uuid,
    member: &Member,
    prompt: &str,
    dispatch: &Dispatch<'_>,
) -> Result<String, String> {
    let participation = Participation::new(dispatch.addressed.iter().any(|p| p == &member.path));
    MemberEvents::discussion(async {
        let events = MemberEvents::new(state, project, member);
        events.emit("agent.progress", "").await;
        let mut task = format!("{prompt}\n{}", participation.instructions()?);
        for attempt in 0..2 {
            let result = events
                .scope(run_member_inner(
                    state, project, member, &task, dispatch, &events,
                ))
                .await;
            let answer = match result {
                Ok(answer) => answer,
                Err(error) => {
                    events.emit("agent.member.error", &error).await;
                    return Err(error);
                }
            };
            if !Participation::is_yield(&answer) {
                events.emit("agent.message", &answer).await;
                return Ok(answer);
            }
            // Discard withheld control fragments before retrying; never flash an
            // unvalidated yield into the public stream or record it as consensus.
            events.reset_discussion_reply().await;
            if participation.accept_yield(&answer, attempt == 1) {
                events
                    .emit(
                        "agent.yield",
                        &format!("已让出本轮 · {}", Participation::reason(&answer).unwrap()),
                    )
                    .await;
                return Ok(answer);
            }
            if attempt == 0 {
                events
                    .emit(
                        "agent.activity",
                        &format!("{} 正在复核职责", member.path.join("/")),
                    )
                    .await;
                task.push_str(&participation.correction()?);
            }
        }
        let error = format!(
            "{} 未按要求回应：职责复核后仍然让出{}。本次任务未完成。",
            member.path.join("/"),
            if participation.required {
                "，但用户已明确指派"
            } else {
                "且未提供有效理由"
            }
        );
        events.emit("agent.member.error", &error).await;
        Err(error)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yields_need_rechecking_and_a_reason_but_addressed_members_cannot_yield() {
        let justified = "本轮让出。原因：当前任务是翻译，我负责部署且未被指派，无待处理事项。";
        let normal = Participation::new(false);
        assert!(!normal.accept_yield(justified, false));
        assert!(normal.accept_yield(justified, true));
        assert!(!normal.accept_yield(YIELD, true));
        assert!(!normal.accept_yield(REASON, true));
        assert!(!Participation::new(true).accept_yield(justified, true));
        assert!(!Participation::is_yield("本轮让出。但我有不同意见"));
    }
}
