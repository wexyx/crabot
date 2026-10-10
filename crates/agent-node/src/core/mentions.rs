use super::policies::{Member, Mode, Policy};

/// One `@mention` resolved against the group's own member list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mention {
    /// The member path exactly as the policy stores it.
    pub path: Vec<String>,
    /// The token as the human typed it, kept for diagnostics.
    pub token: String,
}

/// The longest `@token` this module will read. A longer run is treated as prose,
/// so an email address or a log line cannot be mistaken for a mention.
const MAX_TOKEN: usize = 255;
const MAX_MENTIONS: usize = 8;

/// Find the `@` tokens in `content` that name a member of this group.
///
/// A mention only counts when it matches a member exactly. Free text such as
/// `@here`, an email address or a decorator is left alone rather than resolved
/// to a member the writer did not name, and a run longer than an Agent id is not
/// read as a mention at all.
pub fn find(content: &str, members: &[Member]) -> Vec<Mention> {
    let mut found: Vec<Mention> = Vec::new();
    for token in tokens(content) {
        if found.len() == MAX_MENTIONS {
            break;
        }
        let wanted = token.to_ascii_lowercase();
        // Both the bare leaf id and the full path resolve, because a member may be
        // mounted at `node/agent` while the writer usually knows only the leaf.
        let exact = members
            .iter()
            .find(|m| m.path.join("/").eq_ignore_ascii_case(&wanted));
        let mut aliases = members.iter().filter(|m| {
            m.path
                .last()
                .is_some_and(|leaf| leaf.eq_ignore_ascii_case(&wanted))
        });
        let first = aliases.next();
        let Some(member) = exact.or_else(|| {
            if aliases.next().is_none() {
                first
            } else {
                None
            }
        }) else {
            continue;
        };
        if found.iter().any(|m| m.path == member.path) {
            continue;
        }
        found.push(Mention {
            path: member.path.clone(),
            token: token.to_string(),
        });
    }
    found
}
/// Every `@`-prefixed token in the text, bounded by the surrounding characters.
fn tokens(content: &str) -> Vec<&str> {
    let bytes = content.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while let Some(offset) = content[index..].find('@') {
        let start = index + offset;
        let rest = &content[start + 1..];
        let end = rest
            .find(|c: char| {
                !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '.' && c != '/'
            })
            .unwrap_or(rest.len());
        let token = &rest[..end];
        // `@` needs a left boundary or an email address reads as a mention.
        let bounded = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        if bounded && !token.is_empty() && token.len() <= MAX_TOKEN {
            tokens.push(token);
        }
        index = start + 1 + token.len();
        if index >= content.len() {
            break;
        }
    }
    tokens
}

/// Strip the mention tokens, leaving prose the addressed Agent can read.
///
/// The text stays otherwise intact: `@` marks an address, not a formatting mark,
/// so removing a token must not join two words or drop a sentence. Returns an
/// empty string when the message was nothing but mentions, because the caller
/// then has a request with no content to send.
pub fn strip(content: &str, mentions: &[Mention]) -> String {
    let mut text = content.to_string();
    for mention in mentions.iter().rev() {
        // Remove the token and its `@`, plus one following space if present.
        if let Some(at) = text.rfind(&format!("@{}", mention.token)) {
            let end = at + mention.token.len() + 1;
            let rest = &text[end..];
            let trimmed = rest.strip_prefix(' ').unwrap_or(rest);
            text = format!("{}{}", &text[..at], trimmed);
        }
    }
    text.trim().to_string()
}
/// Restrict one turn to the members the writer addressed.
///
/// Returns `None` when nothing was addressed, so the caller keeps the policy it
/// already has. A mention that names no member is an error rather than a silent
/// full-group dispatch: the writer asked someone specific, and answering with
/// everybody else would look like the request was understood.
pub fn narrow(policy: &Policy, paths: &[Vec<String>]) -> Option<Result<Policy, String>> {
    if paths.is_empty() {
        return None;
    }
    Some((|| {
        let mut narrowed = policy.clone();
        narrowed.members = policy
            .members
            .iter()
            .filter(|member| paths.iter().any(|path| *path == member.path))
            .cloned()
            .collect();
        if narrowed.members.is_empty() {
            return Err(format!(
                "@{} 不是当前项目成员；用 /members 查看成员",
                paths
                    .iter()
                    .map(|p| p.join("/"))
                    .collect::<Vec<_>>()
                    .join("、@")
            ));
        }
        // A discussion mention selects owners, not an isolated miniature group.
        // Keep the roster so an owner can consult peers during the same task.
        if policy.mode == Mode::A2a {
            return Ok(policy.clone());
        }
        if narrowed.members.len() == 1 {
            // Relay needs more than one participant to negotiate over, and PMO
            // needs a leader; a single addressed member is a direct conversation.
            narrowed.mode = Mode::Chat;
            narrowed.leader = None;
            narrowed.rounds = 1;
        } else if narrowed.mode == Mode::Pmo
            && !narrowed
                .members
                .iter()
                .any(|m| Some(&m.path) == policy.leader.as_ref())
        {
            // The configured leader was not addressed, so the addressed members plan
            // among themselves rather than reporting to someone who is not listening.
            narrowed.leader = Some(narrowed.members[0].path.clone());
        }
        narrowed.validate()?;
        Ok(narrowed)
    })())
}
/// Tell an addressed Agent that it was chosen, so an `@name` in the transcript is
/// not mistaken for someone else's text or for a quoted message.
pub fn note(paths: &[Vec<String>]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    format!(
        "The human assigned this turn to @{}. These members own the task. In discussion mode they may consult other roster members; consultation does not transfer ownership or grant permissions.",
        paths
            .iter()
            .map(|p| p.join("/"))
            .collect::<Vec<_>>()
            .join(", @")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn member(path: &str) -> Member {
        Member {
            path: path.split('/').map(str::to_owned).collect(),
            role: "worker".into(),
        }
    }
    fn roster() -> Vec<Member> {
        vec![member("alice"), member("node/bob"), member("carol")]
    }
    fn policy(members: Vec<Member>, mode: Mode) -> Policy {
        Policy {
            relay_strategy: Default::default(),
            mode,
            members,
            leader: None,
            rounds: 1,
            instructions: String::new(),
        }
    }
    #[test]
    fn a_bare_id_and_a_full_path_both_resolve() {
        let roster = roster();
        assert_eq!(
            find("@alice hi", &roster)[0].path,
            vec!["alice".to_string()]
        );
        assert_eq!(
            find("@node/bob hi", &roster)[0].path,
            vec!["node".to_string(), "bob".to_string()]
        );
        // Case must not decide who is addressed.
        assert_eq!(find("@Alice hi", &roster)[0].token, "Alice");
        assert_eq!(find("@NODE/BOB hi", &roster)[0].path.len(), 2);
    }
    #[test]
    fn prose_and_unknown_names_are_not_mentions() {
        let roster = roster();
        assert!(find("mail me at bob@example.com", &roster).is_empty());
        assert!(find("@here standup", &roster).is_empty());
        assert!(find("@nobody hello", &roster).is_empty());
        assert!(find("no mention here", &roster).is_empty());
        // A leaf name must not match a stranger who shares it.
        assert!(find("@alice2 hello", &roster).is_empty());
        // Repeating a member addresses it once.
        let repeated = find("@alice @alice @alice", &roster);
        assert_eq!(repeated.len(), 1);
        assert_eq!(repeated[0].token, "alice");
    }
    #[test]
    fn an_over_long_run_is_not_read_as_a_mention() {
        let roster = roster();
        let long = format!("@{} tail", "a".repeat(MAX_TOKEN + 1));
        assert!(find(&long, &roster).is_empty());
        // Even at the limit it only matches an actual member, never prose.
        assert!(find(&format!("@{} tail", "a".repeat(MAX_TOKEN)), &roster).is_empty());
    }
    #[test]
    fn mention_count_is_bounded_by_the_group_size_limit() {
        let roster: Vec<Member> = (0..12).map(|i| member(&format!("m{i}"))).collect();
        let content = (0..12)
            .map(|i| format!("@m{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(find(&content, &roster).len(), MAX_MENTIONS);
    }
    #[test]
    fn stripping_keeps_the_sentence_readable() {
        let roster = roster();
        let content = "@alice please review the diff";
        let mentions = find(content, &roster);
        assert_eq!(strip(content, &mentions), "please review the diff");
        let middle = "hey @alice and @carol look";
        let found = find(middle, &roster);
        assert_eq!(strip(middle, &found), "hey and look");
        // Mentions only, with no prose left, must not become an empty request.
        let only = "@alice @carol";
        assert!(strip(only, &find(only, &roster)).is_empty());
        assert_eq!(strip("no mentions", &[]), "no mentions");
    }
    #[test]
    fn addressing_one_member_is_a_direct_conversation() {
        let roster = roster();
        let relay = policy(roster.clone(), Mode::Relay);
        let narrowed = narrow(&relay, &[vec!["alice".into()]]).unwrap().unwrap();
        // Relay and PMO cannot run on one member, so the turn becomes a direct chat.
        assert_eq!(narrowed.mode, Mode::Chat);
        assert_eq!(narrowed.members.len(), 1);
        assert!(narrowed.leader.is_none());
        assert_eq!(narrowed.rounds, 1);
    }
    #[test]
    fn addressing_several_keeps_the_configured_mode() {
        let roster = roster();
        let a2a = policy(roster.clone(), Mode::A2a);
        let narrowed = narrow(&a2a, &[vec!["alice".into()], vec!["carol".into()]])
            .unwrap()
            .unwrap();
        assert_eq!(narrowed.mode, Mode::A2a);
        // Policy order is preserved, not the order the names were typed.
        assert_eq!(
            narrowed
                .members
                .iter()
                .map(|m| m.path.join("/"))
                .collect::<Vec<_>>(),
            vec![
                "alice".to_string(),
                "node/bob".to_string(),
                "carol".to_string()
            ]
        );
    }
    #[test]
    fn an_unaddressed_pmo_leader_hands_the_plan_to_an_addressed_member() {
        let mut roster = roster();
        roster.insert(
            0,
            Member {
                path: vec!["lead".into()],
                role: "lead".into(),
            },
        );
        let mut pmo = policy(roster.clone(), Mode::Pmo);
        pmo.leader = Some(vec!["lead".into()]);
        let narrowed = narrow(&pmo, &[vec!["alice".into()], vec!["carol".into()]])
            .unwrap()
            .unwrap();
        assert_eq!(narrowed.mode, Mode::Pmo);
        assert_eq!(narrowed.leader, Some(vec!["alice".into()]));
        // Addressing the leader itself keeps the configured leader.
        let kept = narrow(&pmo, &[vec!["lead".into()], vec!["alice".into()]])
            .unwrap()
            .unwrap();
        assert_eq!(kept.leader, Some(vec!["lead".into()]));
    }
    #[test]
    fn a_name_outside_the_roster_is_an_error_not_a_full_group_dispatch() {
        let roster = roster();
        let relay = policy(roster.clone(), Mode::Relay);
        let error = narrow(&relay, &[vec!["dave".into()]]).unwrap().unwrap_err();
        assert!(error.contains("dave"), "{error}");
        // No mentions at all keeps the policy the caller already has.
        assert!(narrow(&relay, &[]).is_none());
    }
    #[test]
    fn the_note_names_the_addressed_agents_only_when_addressed() {
        assert_eq!(note(&[]), "");
        assert!(note(&[vec!["alice".into()]]).contains("@alice"));
        assert!(note(&[vec!["a".into()], vec!["b".into()]]).contains("@a, @b"));
    }
}
