use super::graph::Knowledge;
use serde_json::{Value, json};

/// One search result, already reduced to what a model is shown.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Hit {
    pub chat: String,
    pub seq: u64,
    pub kind: String,
    pub agent: String,
    pub excerpt: String,
    pub score: Option<f64>,
    /// The original event's message id, when the record carried one. Used by the
    /// host for resume-point truncation, not shown to the model.
    pub message_id: Option<String>,
    /// The complete event row as JSON. Display paths rebuild full payloads from it.
    pub raw: Option<String>,
}

/// How much of a matching record to return.
///
/// A search is a pointer to the exact place, not a substitute for it: returning whole
/// records would put the transcript back into the context that searching was meant to
/// keep small.
const EXCERPT: usize = 240;

/// Search the index for one project's records.
///
/// The project is not a parameter. It is bound by the caller when the index is opened,
/// so a model cannot aim the search at another project, and the query carries no path
/// to redirect it.
///
/// Returns the hits plus whether more exist: one extra row is fetched so truncation is
/// observed rather than guessed.
pub(crate) fn recall(
    knowledge: &Knowledge,
    query: &str,
    chat: Option<&str>,
    limit: usize,
) -> Result<(Vec<Hit>, bool), String> {
    let needle = query.trim();
    if needle.is_empty() {
        return Err("检索词不能为空".into());
    }
    let limit = limit.clamp(1, 100);
    // Streaming fragments and progress markers live in the index (the only store,
    // and live feeds replay them in order) but are never recall hits: a model
    // searching history wants statements, not a token-by-token replay.
    let recallable = |hit: &Hit| {
        !matches!(
            hit.kind.as_str(),
            "agent.delta" | "text_delta" | "agent.progress"
        )
    };
    // Full-text first when the extension loaded, because BM25 ranks; the literal
    // predicate is the fallback and also the only option when FTS is unavailable.
    // A query failure is not fatal either: the literal predicate below still finds
    // the record, and search must never hard-fail a run because the index grew an
    // extra column the engine cannot project.
    if knowledge.has_fts() {
        let rows = knowledge
            .rows(
                "CALL QUERY_FTS_INDEX('Turn', 'turn_text', $query) \
                 RETURN node.chat AS chat, node.seq AS seq, node.kind AS kind, \
                        node.agent AS agent, node.text AS text, \
                        node.message_id AS message_id, node.raw AS raw, score AS score \
                 ORDER BY score DESC",
                vec![("query", lbug::Value::String(needle.to_string()))],
            )
            .unwrap_or_default();
        let mut hits = rows
            .iter()
            .filter_map(|row| hit(row, needle))
            // The table name and index name are fixed by the schema, but the chat
            // filter is a model-supplied value and stays a Rust-side predicate on
            // bound results rather than query text.
            .filter(|hit| chat.is_none_or(|name| hit.chat == name))
            .filter(recallable)
            // One past the limit: truncation is observed, not guessed. Excerpts are
            // built per hit, so rows beyond this are dropped before that cost.
            .take(limit + 1)
            .collect::<Vec<_>>();
        if !hits.is_empty() {
            let truncated = hits.len() > limit;
            hits.truncate(limit);
            return Ok((hits, truncated));
        }
    }
    // Two shapes instead of a NULL trick: the chat restriction is either present as
    // a bound parameter or absent entirely, so there is no three-valued-logic edge.
    let (shape, params): (&str, Vec<(&str, lbug::Value)>) = match chat {
        Some(name) => (
            "MATCH (t:Turn) WHERE CONTAINS(LOWER(t.text), LOWER($query)) AND t.chat = $chat \
             RETURN t.chat, t.seq, t.kind, t.agent, t.text, t.message_id, t.raw \
             ORDER BY t.seq DESC",
            vec![
                ("query", lbug::Value::String(needle.to_string())),
                ("chat", lbug::Value::String(name.to_string())),
            ],
        ),
        None => (
            "MATCH (t:Turn) WHERE CONTAINS(LOWER(t.text), LOWER($query)) \
             RETURN t.chat, t.seq, t.kind, t.agent, t.text, t.message_id, t.raw \
             ORDER BY t.seq DESC",
            vec![("query", lbug::Value::String(needle.to_string()))],
        ),
    };
    let rows = knowledge.rows(shape, params)?;
    let mut hits = rows
        .iter()
        .filter_map(|row| hit(row, needle))
        .filter(recallable)
        .take(limit + 1)
        .collect::<Vec<_>>();
    let truncated = hits.len() > limit;
    hits.truncate(limit);
    Ok((hits, truncated))
}

/// Read history by sequence range, in sequential order.
///
/// Display reads (`agent: None`) preserve every original event, including records
/// covered by summaries. An Agent-specific read anchors its first page with that
/// Agent\'s latest summary. Context construction uses `context_snapshot` to freeze
/// the boundary and apply reset semantics across all pages.
pub(crate) fn recall_range(
    knowledge: &Knowledge,
    chat: &str,
    agent: Option<&str>,
    after_seq: u64,
    before_seq: u64,
    limit: usize,
) -> Result<(Vec<Hit>, bool), String> {
    let limit = limit.clamp(1, 1000);
    let anchor = match agent {
        Some(agent) if after_seq == 0 => latest_summary(knowledge, chat, agent)?,
        _ => None,
    };
    let floor = anchor
        .as_ref()
        .map_or(after_seq, |s| after_seq.max(s.seq_end));
    let descending = agent.is_none() && after_seq == 0;
    let mut hits = range(knowledge, chat, floor, before_seq, limit + 1, descending)?;
    let truncated = hits.len() > limit;
    hits.truncate(limit);
    if descending {
        hits.reverse();
    }
    if let Some(summary) = anchor {
        hits.insert(0, summary_hit(chat, summary));
    }
    Ok((hits, truncated))
}

pub(super) fn range(
    knowledge: &Knowledge,
    chat: &str,
    after: u64,
    before: u64,
    limit: usize,
    descending: bool,
) -> Result<Vec<Hit>, String> {
    let order = if descending { "DESC" } else { "ASC" };
    let rows = knowledge.rows(
        &format!("MATCH (t:Turn {{chat: $chat}}) WHERE t.seq > $after AND t.seq < $before RETURN t.chat, t.seq, t.kind, t.agent, t.text, t.message_id, t.raw ORDER BY t.seq {order} LIMIT {limit}"),
        vec![("chat", lbug::Value::String(chat.into())), ("after", lbug::Value::Int64(after.min(i64::MAX as u64) as i64)), ("before", lbug::Value::Int64(before.min(i64::MAX as u64) as i64))],
    )?;
    Ok(rows.iter().filter_map(|r| hit(r, "")).collect())
}
pub(super) fn summary_hit(chat: &str, summary: Summary) -> Hit {
    Hit {
        chat: chat.into(),
        seq: summary.seq_end,
        kind: "summary".into(),
        agent: summary.agent,
        excerpt: summary.text,
        score: None,
        message_id: None,
        raw: None,
    }
}

/// The most recent summary of one chat and agent.
///
/// Used to anchor a freshly built context window and to compute where the next
/// summary's coverage starts. `seq_end` is the coverage frontier: everything
/// written after it is still uncovered for this agent.
pub(crate) fn latest_summary(
    knowledge: &Knowledge,
    chat: &str,
    agent: &str,
) -> Result<Option<Summary>, String> {
    let rows = knowledge.rows(
        "MATCH (s:Summary {chat: $chat, agent: $agent}) \
         RETURN s.seq_start, s.seq_end, s.text, s.agent \
         ORDER BY s.seq_end DESC LIMIT 1",
        vec![
            ("chat", lbug::Value::String(chat.into())),
            ("agent", lbug::Value::String(agent.into())),
        ],
    )?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(summary_of(row)))
}

fn summary_of(row: &[lbug::Value]) -> Summary {
    let seq_end = match row.get(1) {
        Some(lbug::Value::Int64(v)) => (*v).max(0) as u64,
        _ => 0,
    };
    Summary {
        seq_end,
        text: text_of(row, 2).unwrap_or_default(),
        agent: text_of(row, 3).unwrap_or_default(),
    }
}

/// The highest stored sequence number of one chat, for attach/replay baselines.
pub(crate) fn latest_seq(knowledge: &Knowledge, chat: &str) -> Result<Option<u64>, String> {
    let rows = knowledge.rows(
        "MATCH (t:Turn {chat: $chat}) RETURN max(t.seq)",
        vec![("chat", lbug::Value::String(chat.into()))],
    )?;
    let Some(value) = rows.first().and_then(|row| row.first()) else {
        return Ok(None);
    };
    match value {
        lbug::Value::Int64(v) => Ok(Some((*v).max(0) as u64)),
        _ => Ok(None),
    }
}

/// A stored summary, with the sequence range it covers and the agent that wrote it.
pub(crate) struct Summary {
    pub seq_end: u64,
    pub text: String,
    pub agent: String,
}

fn hit(row: &[lbug::Value], needle: &str) -> Option<Hit> {
    let text = text_of(row, 4)?;
    // Integers come back in whatever width the engine used; accept them all rather
    // than failing on a width the schema did not promise.
    let seq = match row.get(1)? {
        lbug::Value::Int64(v) => *v,
        lbug::Value::Int32(v) => *v as i64,
        lbug::Value::Int16(v) => *v as i64,
        lbug::Value::Int8(v) => *v as i64,
        lbug::Value::UInt64(v) => *v as i64,
        lbug::Value::UInt32(v) => *v as i64,
        lbug::Value::UInt16(v) => *v as i64,
        lbug::Value::UInt8(v) => *v as i64,
        _ => return None,
    };
    let score = match row.get(7) {
        Some(lbug::Value::Double(v)) => Some(*v),
        Some(lbug::Value::Float(v)) => Some(*v as f64),
        _ => None,
    };
    Some(Hit {
        chat: text_of(row, 0)?,
        seq: seq.max(0) as u64,
        kind: text_of(row, 2).unwrap_or_default(),
        agent: text_of(row, 3).unwrap_or_default(),
        excerpt: excerpt(&text, needle),
        score,
        message_id: text_of(row, 5).filter(|s| !s.is_empty()),
        raw: text_of(row, 6).filter(|s| !s.is_empty()),
    })
}

fn text_of(row: &[lbug::Value], index: usize) -> Option<String> {
    match row.get(index)? {
        lbug::Value::String(value) => Some(value.clone()),
        _ => None,
    }
}

/// Centre the excerpt on the match so the useful part survives the cut.
pub(super) fn excerpt(text: &str, needle: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= EXCERPT {
        return flat;
    }
    let at = flat
        .to_lowercase()
        .find(&needle.to_lowercase())
        .map(|at| flat.to_lowercase()[..at].chars().count())
        .unwrap_or(0);
    // Pull the window back so the match is preceded by context, then skip by chars: an
    // index derived from a byte offset would split a codepoint.
    let start = at.saturating_sub(EXCERPT / 3);
    let body = flat.chars().skip(start).take(EXCERPT).collect::<String>();
    if start == 0 {
        body
    } else {
        format!("…{body}")
    }
}

/// Shape returned to the model, mirroring the tool-result contract.
pub(crate) fn render(hits: &[Hit], truncated: bool) -> Value {
    json!({
        "matches":hits
            .iter()
            .map(|hit| {
                let mut match_ = json!({
                    "chat":hit.chat,
                    "seq":hit.seq,
                    "kind":hit.kind,
                    "agent":hit.agent,
                    "excerpt":hit.excerpt,
                });
                if let Some(message_id) = hit.message_id.as_deref().filter(|s| !s.is_empty()) {
                    match_["message_id"] = json!(message_id);
                }
                match_
            })
            .collect::<Vec<_>>(),
        "matched":hits.len(),
        "truncated":truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::super::ingest::{from_row, ingest, persist_at, write_summary};
    use super::*;
    use serde_json::json;

    fn seeded() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("kb");
        let knowledge = Knowledge::at(&root).unwrap();
        let rows = [
            json!({"seq":1,"type":"user","content":"we picked axum for the router"}),
            json!({"seq":2,"type":"user","content":"unrelated deploy note"}),
            json!({"seq":3,"type":"agent.done","payload":{"content":"axum was chosen"}}),
        ];
        let batch: Vec<_> = rows.iter().filter_map(|r| from_row("design", r)).collect();
        ingest(&knowledge, &batch).unwrap();
        (dir, root)
    }

    #[test]
    fn finds_a_record_and_reports_its_coordinate() {
        let (_guard, root) = seeded();
        let knowledge = Knowledge::at(&root).unwrap();
        let (hits, truncated) = recall(&knowledge, "axum", None, 20).unwrap();
        assert!(!truncated);
        assert!(!hits.is_empty());
        // Whatever BM25 ranks first, every hit carries its project coordinate.
        assert!(hits.iter().all(|h| h.chat == "design"));
        assert!(hits.iter().any(|h| h.seq == 1 && h.kind == "user"));
        assert!(hits.iter().any(|h| h.seq == 3));
    }

    #[test]
    fn a_chat_filter_narrows_the_search() {
        let (_guard, root) = seeded();
        let knowledge = Knowledge::at(&root).unwrap();
        assert!(
            recall(&knowledge, "axum", Some("ops"), 20)
                .unwrap()
                .0
                .is_empty()
        );
        assert!(
            !recall(&knowledge, "axum", Some("design"), 20)
                .unwrap()
                .0
                .is_empty()
        );
    }

    #[test]
    fn an_empty_query_is_refused() {
        let (_guard, root) = seeded();
        let knowledge = Knowledge::at(&root).unwrap();
        assert!(recall(&knowledge, "   ", None, 20).is_err());
    }

    #[test]
    fn a_chat_name_with_a_quote_cannot_redirect_the_query() {
        let dir = tempfile::tempdir().unwrap();
        let knowledge = Knowledge::at(&dir.path().join("kb")).unwrap();
        let rows = [
            json!({"seq":1,"type":"user","content":"axum"}),
            json!({"seq":1,"type":"user","content":"axum"}),
        ];
        let batch: Vec<_> = ["design", "other' OR '1'='1"]
            .iter()
            .zip(rows.iter())
            .filter_map(|(chat, row)| from_row(chat, row))
            .collect();
        ingest(&knowledge, &batch).unwrap();
        // The injected name matches nothing, so only the real chat is returned.
        let (hits, truncated) = recall(&knowledge, "axum", Some("other' OR '1'='1"), 20).unwrap();
        assert!(!truncated);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chat, "other' OR '1'='1");
    }

    #[test]
    fn the_excerpt_keeps_the_match_and_stays_within_the_limit() {
        let text = format!("{} axum {}", "prefix ".repeat(200), "suffix ".repeat(200));
        let cut = excerpt(&text, "axum");
        assert!(cut.contains("axum"));
        assert!(cut.chars().count() <= EXCERPT + 1, "ellipsis aside");
    }

    #[test]
    fn a_short_record_is_returned_whole() {
        assert_eq!(excerpt("short note", "note"), "short note");
    }

    /// The write path end to end: persisted rows land in the index, sequential
    /// reads return them with their seqs, and a summary written over the range
    /// anchors the first page while its covered records disappear from it.
    #[test]
    fn probe_two_rows() {
        let dir = tempfile::tempdir().unwrap();
        let knowledge = Knowledge::at(&dir.path().join("kb")).unwrap();
        persist_at(
            &knowledge,
            "design",
            &[
                json!({"type":"user","content":"we chose axum for the router"}),
                json!({"type":"agent.done","payload":{"content":"agreed","agent":"first"}}),
            ],
        )
        .unwrap();
        let rows = knowledge
            .rows(
                "MATCH (t:Turn {chat:'design'}) \
                 RETURN t.chat, t.seq, t.kind, t.agent, t.text, t.message_id, t.raw \
                 ORDER BY t.seq ASC",
                vec![],
            )
            .unwrap();
        let via_range = recall_range(&knowledge, "design", None, 0, u64::MAX, 20).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(via_range.0.len(), 2);
    }

    #[test]
    fn persist_then_read_then_compact_chains() {
        let dir = tempfile::tempdir().unwrap();
        let knowledge = Knowledge::at(&dir.path().join("kb")).unwrap();
        persist_at(
            &knowledge,
            "design",
            &[
                json!({"type":"user","content":"we chose axum for the router"}),
                json!({"type":"agent.done","payload":{"content":"agreed","agent":"first"}}),
            ],
        )
        .unwrap();
        let (hits, truncated) = recall_range(&knowledge, "design", None, 0, u64::MAX, 20).unwrap();
        assert!(!truncated);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].seq, 1);
        assert_eq!(hits[1].seq, 2);
        // A second persist continues the sequence.
        persist_at(
            &knowledge,
            "design",
            &[json!({"type":"user","content":"now add tests"})],
        )
        .unwrap();
        assert_eq!(latest_seq(&knowledge, "design").unwrap(), Some(3));
        // The first summary covers the whole sequence written so far; the first
        // page then leads with the summary and drops the covered records.
        write_summary(
            &knowledge,
            "design",
            "first",
            0,
            3,
            r#"{"goals":["router"]}"#,
        )
        .unwrap();
        let (hits, _) = recall_range(&knowledge, "design", Some("first"), 0, u64::MAX, 20).unwrap();
        assert_eq!(hits.len(), 1, "only the summary remains for agent one");
        assert_eq!(hits[0].kind, "summary");
        assert_eq!(hits[0].seq, 3);
        // Records written after the compaction get higher seqs and stay visible
        // even when the wall clock has not moved: coverage is by sequence.
        persist_at(
            &knowledge,
            "design",
            &[json!({"type":"user","content":"still here"})],
        )
        .unwrap();
        let (hits, _) = recall_range(&knowledge, "design", Some("first"), 0, u64::MAX, 20).unwrap();
        assert_eq!(hits.len(), 2, "summary plus the post-compaction record");
        assert_eq!(hits[0].kind, "summary");
        assert_eq!(hits[1].seq, 4);
        // Another agent's view keeps the originals: coverage is per agent.
        let (hits, _) =
            recall_range(&knowledge, "design", Some("second"), 0, u64::MAX, 20).unwrap();
        assert_eq!(hits.len(), 4, "agent two sees every original record");
        // The display view shows the summary and everything uncovered by anyone.
        let (hits, _) = recall_range(&knowledge, "design", None, 0, u64::MAX, 20).unwrap();
        assert_eq!(hits.len(), 4);
        assert_eq!(hits[0].kind, "user");
    }

    #[test]
    fn a_multi_byte_match_does_not_split_a_codepoint() {
        let text = format!("{} 知识图谱 {}", "前".repeat(200), "后".repeat(200));
        let cut = excerpt(&text, "知识图谱");
        assert!(cut.contains("知识图谱"));
    }
}
