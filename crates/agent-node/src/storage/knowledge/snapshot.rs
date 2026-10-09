use super::{
    graph::Knowledge,
    recall::{self, Hit},
};
use lbug::Value;

/// Immutable input boundary: later arrivals cannot be covered by this summary.
pub(crate) struct ContextSnapshot {
    pub rows: Vec<Hit>,
    pub through: u64,
    pub reset: u64,
}
pub(crate) fn context_snapshot(
    db: &Knowledge,
    chat: &str,
    agent: &str,
    before_message: Option<&str>,
) -> Result<ContextSnapshot, String> {
    let mut through = recall::latest_seq(db, chat)?.unwrap_or(0);
    if let Some(message) = before_message {
        let rows = db.rows("MATCH (t:Turn {chat: $chat}) WHERE t.message_id = $message AND t.kind = 'message.created' RETURN min(t.seq)", vec![("chat", Value::String(chat.into())), ("message", Value::String(message.into()))])?;
        if let Some(Value::Int64(seq)) = rows.first().and_then(|r| r.first()) {
            through = through.min((*seq).max(1) as u64 - 1);
        }
    }
    let reset = reset_seq(db, chat, through)?;
    let summary = recall::latest_summary(db, chat, agent)?
        .filter(|s| s.seq_end > reset && s.seq_end <= through);
    let mut cursor = summary.as_ref().map_or(reset, |s| s.seq_end);
    let mut rows = Vec::new();
    if let Some(summary) = summary {
        rows.push(recall::summary_hit(chat, summary));
    }
    loop {
        let page = recall::range(db, chat, cursor, through.saturating_add(1), 1000, false)?;
        let Some(last) = page.last() else { break };
        cursor = last.seq;
        rows.extend(page);
        if cursor >= through {
            break;
        }
    }
    Ok(ContextSnapshot {
        rows,
        through,
        reset,
    })
}
pub(super) fn reset_seq(db: &Knowledge, chat: &str, through: u64) -> Result<u64, String> {
    let rows = db.rows("MATCH (t:Turn {chat: $chat}) WHERE t.kind = 'context.reset' AND t.seq <= $through RETURN max(t.seq)", vec![("chat", Value::String(chat.into())), ("through", Value::Int64(through.min(i64::MAX as u64) as i64))])?;
    Ok(match rows.first().and_then(|r| r.first()) {
        Some(Value::Int64(v)) => (*v).max(0) as u64,
        _ => 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::knowledge::ingest::{persist_at, write_summary};
    use serde_json::json;

    #[test]
    fn each_agent_restores_its_own_latest_summary_and_complete_tail() {
        let dir = tempfile::tempdir().unwrap();
        let db = Knowledge::at(&dir.path().join("index")).unwrap();
        let rows = (1..=6)
            .map(|n| json!({"type":"user","content":format!("message {n}")}))
            .collect::<Vec<_>>();
        persist_at(&db, "chat", &rows).unwrap();
        write_summary(&db, "chat", "small", 1, 2, "small summary").unwrap();
        write_summary(&db, "chat", "large", 1, 4, "large summary").unwrap();
        let small = context_snapshot(&db, "chat", "small", None).unwrap();
        let large = context_snapshot(&db, "chat", "large", None).unwrap();
        assert_eq!(
            small.rows.iter().map(|h| h.seq).collect::<Vec<_>>(),
            vec![2, 3, 4, 5, 6]
        );
        assert_eq!(
            large.rows.iter().map(|h| h.seq).collect::<Vec<_>>(),
            vec![4, 5, 6]
        );
        assert_eq!(small.rows[0].excerpt, "small summary");
        assert_eq!(large.rows[0].excerpt, "large summary");
        assert_eq!(
            context_snapshot(&db, "chat", "new", None)
                .unwrap()
                .rows
                .len(),
            6
        );
        assert!(
            context_snapshot(&db, "another", "small", None)
                .unwrap()
                .rows
                .is_empty()
        );
        write_summary(&db, "chat", "small", 1, 5, "new small summary").unwrap();
        let again = context_snapshot(&db, "chat", "small", None).unwrap();
        assert_eq!(again.rows.len(), 2);
        assert_eq!(again.rows[0].excerpt, "new small summary");
        assert_eq!(
            recall::recall_range(&db, "chat", None, 0, u64::MAX, 100)
                .unwrap()
                .0
                .len(),
            6,
            "summaries never hide display history"
        );
    }

    #[test]
    fn summary_never_covers_new_arrivals_and_reset_survives_without_text() {
        let dir = tempfile::tempdir().unwrap();
        let db = Knowledge::at(&dir.path().join("index")).unwrap();
        persist_at(&db, "chat", &[json!({"type":"user","content":"old"})]).unwrap();
        let snapshot = context_snapshot(&db, "chat", "a", None).unwrap();
        persist_at(
            &db,
            "chat",
            &[json!({"type":"user","content":"arrived while summarizing"})],
        )
        .unwrap();
        write_summary(&db, "chat", "a", 1, snapshot.through, "summary").unwrap();
        assert_eq!(
            context_snapshot(&db, "chat", "a", None).unwrap().rows[1].seq,
            2
        );
        persist_at(
            &db,
            "chat",
            &[
                json!({"type":"context.reset"}),
                json!({"type":"user","content":"new context"}),
                json!({"type":"agent.done"}),
            ],
        )
        .unwrap();
        let reset = context_snapshot(&db, "chat", "a", None).unwrap();
        assert_eq!(reset.reset, 3);
        assert_eq!(
            reset.rows.iter().map(|h| h.seq).collect::<Vec<_>>(),
            vec![4, 5]
        );
        assert!(!reset.rows.iter().any(|h| h.kind == "summary"));
    }

    #[test]
    fn context_reads_every_page_but_display_pages_from_the_end() {
        let dir = tempfile::tempdir().unwrap();
        let db = Knowledge::at(&dir.path().join("index")).unwrap();
        let rows = (0..1005).map(|n|json!({"type":"message.created","payload":{"message_id":format!("m{n}"),"content":format!("message {n}")}})).collect::<Vec<_>>();
        persist_at(&db, "chat", &rows).unwrap();
        let snapshot = context_snapshot(&db, "chat", "a", Some("m1004")).unwrap();
        assert_eq!(snapshot.through, 1004);
        assert_eq!(snapshot.rows.len(), 1004);
        assert_eq!(snapshot.rows.last().unwrap().seq, 1004);
        let (page, more) = recall::recall_range(&db, "chat", None, 0, u64::MAX, 2).unwrap();
        assert!(more);
        assert_eq!(
            page.iter().map(|h| h.seq).collect::<Vec<_>>(),
            vec![1004, 1005]
        );
        let (older, _) = recall::recall_range(&db, "chat", None, 0, 1004, 2).unwrap();
        assert_eq!(
            older.iter().map(|h| h.seq).collect::<Vec<_>>(),
            vec![1002, 1003]
        );
    }
}
