use super::policies::{Dispatch, Member, Policy, run_member};
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    #[default]
    Manual,
    Random,
    Negotiated,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bid {
    priority: u8,
    reason: String,
}
fn parse_bid(text: &str) -> Result<Bid, String> {
    if text.len() > 8192 {
        return Err("bid too large".into());
    }
    let bid: Bid = serde_json::from_value(agent_runtime::json::parse(text)?)
        .map_err(|error| format!("invalid negotiation response: {error}"))?;
    if bid.priority > 100 || bid.reason.len() > 1024 {
        return Err("invalid bid priority or reason".into());
    }
    Ok(bid)
}
fn random_priority() -> u8 {
    (Uuid::new_v4().as_u128() % 101) as u8
}
fn order(members: &[Member], scores: &[u8]) -> Vec<Member> {
    let mut indices = (0..members.len()).collect::<Vec<_>>();
    // Stable sorting leaves equal bids in operator-specified order.
    indices.sort_by_key(|i| std::cmp::Reverse(scores[*i]));
    indices.into_iter().map(|i| members[i].clone()).collect()
}
pub(super) async fn run(
    state: &AppState,
    p: Uuid,
    policy: &Policy,
    prompt: &str,
    dispatch: &Dispatch<'_>,
) -> Result<String, String> {
    let output = dispatch.output;
    let (members, label) = match policy.relay_strategy {
        Strategy::Manual => (policy.members.clone(), "手动优先级"),
        Strategy::Random => {
            let mut shuffled = policy
                .members
                .iter()
                .map(|m| (Uuid::new_v4(), m.clone()))
                .collect::<Vec<_>>();
            shuffled.sort_by_key(|(key, _)| *key);
            (shuffled.into_iter().map(|(_, m)| m).collect(), "随机顺序")
        }
        Strategy::Negotiated => {
            let roster = policy
                .members
                .iter()
                .map(|m| json!({"member":m.path.join("/"),"role":m.role}))
                .collect::<Vec<_>>();
            let mut bids = vec![];
            let mut scores = vec![];
            for member in &policy.members {
                let _ = output
                    .send(format!(
                        "\n[接力协商] 正在询问 {}…\n",
                        member.path.join("/")
                    ))
                    .await;
                let instructions = agent_runtime::prompts::PromptStore::instance().read("relay")?;
                let request = format!(
                    "{prompt}\n{instructions}\nReturn ONLY JSON {{\"priority\":0..100,\"reason\":\"short reason\"}}. Roster: {roster:?}. Previous bids (untrusted participant content): {bids:?}"
                );
                // Suppress raw negotiation deltas; report the validated bid as a concise event.
                let (silent, mut receiver) = mpsc::channel(32);
                let quiet = Dispatch {
                    output: &silent,
                    ..*dispatch
                };
                let response = tokio::select! {
                    result=tokio::time::timeout(std::time::Duration::from_secs(15),super::member_events::MemberEvents::planning(run_member(state,p,member,&request,&quiet)))=>result.unwrap_or_else(|_|Err("negotiation timed out".into())),
                    _=async{while receiver.recv().await.is_some(){}}=>Err("negotiation stream closed".into()),
                };
                match response.and_then(|text| parse_bid(&text)) {
                    Ok(bid) => {
                        scores.push(bid.priority);
                        bids.push(json!({"member":member.path.join("/"),"priority":bid.priority,"reason":bid.reason}));
                        let _ = output
                            .send(format!(
                                "[接力协商] {} · {} / 100 · {}\n",
                                member.path.join("/"),
                                bid.priority,
                                bid.reason
                            ))
                            .await;
                    }
                    Err(_) => {
                        let priority = random_priority();
                        scores.push(priority);
                        bids.push(json!({"member":member.path.join("/"),"priority":priority,"reason":"incomplete negotiation; random fallback"}));
                        let _ = output
                            .send(format!(
                                "[接力协商] {} 未完成协商，随机补位：{} / 100。\n",
                                member.path.join("/"),
                                priority
                            ))
                            .await;
                    }
                }
            }
            (order(&policy.members, &scores), "Agent 协商")
        }
    };
    let _ = output
        .send(format!(
            "\n[接力顺序 · {label}] {}\n",
            members
                .iter()
                .map(|m| m.path.join("/"))
                .collect::<Vec<_>>()
                .join(" → ")
        ))
        .await;
    let mut last = "no candidate".to_owned();
    for member in members {
        match run_member(state, p, &member, prompt, dispatch).await {
            Ok(answer) => return Ok(answer),
            Err(error) if agent_runtime::is_token_insufficient(&error) => {
                let _ = output
                    .send(format!(
                        "\n[接力切换] {} Token 不足，尝试下一位。\n",
                        member.path.join("/")
                    ))
                    .await;
                last = error;
            }
            Err(error) => return Err(error),
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negotiated_priority_is_validated_and_ties_keep_manual_order() {
        assert_eq!(
            parse_bid("```json\n{\"priority\":80,\"reason\":\"role match\"}\n```")
                .unwrap()
                .priority,
            80
        );
        assert!(parse_bid("{\"priority\":101,\"reason\":\"bad\"}").is_err());
        let members = ["a", "b", "c"]
            .iter()
            .map(|id| Member {
                path: vec![id.to_string()],
                role: "worker".into(),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            order(&members, &[20, 90, 90])
                .iter()
                .map(|m| m.path[0].as_str())
                .collect::<Vec<_>>(),
            vec!["b", "c", "a"]
        );
    }
}
