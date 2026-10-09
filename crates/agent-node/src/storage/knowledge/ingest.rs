use super::graph::Knowledge;
use serde_json::Value;
use time::{OffsetDateTime, PlainDateTime, macros::format_description};

/// The canonical UTC civil shape timestamps are stored in (`now_datetime`).
///
/// Ladybug does not cast STRING to TIMESTAMP implicitly, so the write path parses
/// this shape and binds a real Timestamp; anything else falls back to the epoch.
const CREATED_AT: &[time::format_description::BorrowedFormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

/// How many records one test ingest call accepts.
#[cfg(test)]
const BATCH: usize = 512;

/// One indexed event, including its complete raw payload.
///
/// Two lifetimes because the chat name and the row usually come from different
/// owners: the caller holds the chat key while rows are borrowed out of an append
/// batch, and tests bind short-lived rows against a long-lived name.
pub(crate) struct Record<'c, 't> {
    pub chat: &'c str,
    pub seq: u64,
    pub kind: &'t str,
    pub agent: &'t str,
    pub text: &'t str,
    pub message_id: &'t str,
    /// The complete event row as JSON. The index is the only store now, so display
    /// and resume-point truncation rebuild from this rather than from reduced fields.
    pub raw: String,
    pub created_at: Option<String>,
}

/// What an ingest call did.
#[cfg(test)]
#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) struct Ingested {
    pub indexed: usize,
    /// Deferred records reported by the test ingest helper; write errors return Err.
    pub deferred: usize,
}

/// Index records that are not already present.
///
/// Idempotent by construction: the node merges on its id alone, so a replayed batch
/// sets the same properties again instead of duplicating. Production writes use
/// `persist_at` to allocate sequences and persist the batch in one transaction.
///
/// Every value is bound as a parameter, never interpolated: a chat name or a message
/// containing a quote must not be able to change the statement.
#[cfg(test)]
pub(crate) fn ingest(
    knowledge: &Knowledge,
    records: &[Record<'_, '_>],
) -> Result<Ingested, String> {
    if records.is_empty() {
        return Ok(Ingested::default());
    }
    let mut indexed = 0usize;
    let mut deferred = 0usize;
    for batch in records.chunks(BATCH) {
        let (batch_indexed, batch_deferred) = knowledge.batch(|conn| index_batch(conn, batch))?;
        indexed += batch_indexed;
        deferred += batch_deferred;
    }
    Ok(Ingested { indexed, deferred })
}

/// The statements one batch needs, under one lock.
///
/// Shared between `ingest` (which takes the lock per chunk) and `persist` (which
/// assigns sequence numbers under the same lock as the writes, so it calls this
/// directly instead of re-entering the non-reentrant mutex).
fn index_batch(
    conn: &lbug::Connection,
    batch: &[Record<'_, '_>],
) -> Result<(usize, usize), String> {
    let mut turn = conn
        .prepare(
            "MERGE (t:Turn {id: $id}) \
             SET t.chat = $chat, t.seq = $seq, t.kind = $kind, t.agent = $agent, \
                 t.text = $text, t.message_id = $message_id, t.raw = $raw, t.created_at = $created_at",
        )
        .map_err(|e| e.to_string())?;
    let mut chat = conn
        .prepare("MERGE (c:Chat {name: $chat})")
        .map_err(|e| e.to_string())?;
    let mut edge = conn
        .prepare(
            "MATCH (t:Turn {id: $id}), (c:Chat {name: $chat}) \
             MERGE (t)-[:IN_CHAT]->(c)",
        )
        .map_err(|e| e.to_string())?;
    let mut indexed = 0usize;
    let deferred = 0usize;
    for record in batch {
        // Raw events, including stream fragments, remain in the primary store.
        let id = identity(record.chat, record.seq);
        // The column is TIMESTAMP, which rejects STRING bindings, so the canonical
        // civil shape is parsed back into a Timestamp before it is bound.
        let created_at = record
            .created_at
            .as_deref()
            .unwrap_or("1970-01-01 00:00:00");
        let created_at = PlainDateTime::parse(created_at, CREATED_AT)
            .map(|dt| dt.assume_utc())
            .unwrap_or(OffsetDateTime::UNIX_EPOCH);
        let outcome = conn
            .execute(
                &mut turn,
                vec![
                    ("id", lbug::Value::String(id.clone())),
                    ("chat", lbug::Value::String(record.chat.into())),
                    ("seq", lbug::Value::Int64(record.seq as i64)),
                    ("kind", lbug::Value::String(record.kind.into())),
                    ("agent", lbug::Value::String(record.agent.into())),
                    ("text", lbug::Value::String(record.text.into())),
                    ("message_id", lbug::Value::String(record.message_id.into())),
                    ("raw", lbug::Value::String(record.raw.clone())),
                    ("created_at", lbug::Value::Timestamp(created_at)),
                ],
            )
            .and_then(|_| {
                conn.execute(
                    &mut chat,
                    vec![("chat", lbug::Value::String(record.chat.into()))],
                )
            })
            .and_then(|_| {
                conn.execute(
                    &mut edge,
                    vec![
                        ("id", lbug::Value::String(id)),
                        ("chat", lbug::Value::String(record.chat.into())),
                    ],
                )
            });
        match outcome {
            Ok(_) => indexed += 1,
            // The enclosing transaction must roll back, never acknowledge a gap.
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok((indexed, deferred))
}

/// Persist event rows as the only durable chat-record write, for one project.
///
/// Resolves the project's index handle, then delegates to `persist_at`. The index
/// is the only store, so an unavailable index is a hard error rather than
/// something to fall back from.
pub(crate) fn persist(
    project: uuid::Uuid,
    chat: &str,
    rows: &[serde_json::Value],
) -> Result<Vec<serde_json::Value>, String> {
    let knowledge = super::graph::for_project(project).ok_or("历史索引不可用")?;
    persist_at(&knowledge, chat, rows)
}

/// Persist event rows into one open index.
///
/// Sequence numbers are handed out under the same lock as the writes, so two
/// concurrent persists cannot allocate the same `(chat, seq)` coordinate. The
/// counter lives on the Chat node itself, so a restart continues where it stopped.
/// Returns the complete rows with their assigned `seq`.
pub(crate) fn persist_at(
    knowledge: &Knowledge,
    chat: &str,
    rows: &[serde_json::Value],
) -> Result<Vec<serde_json::Value>, String> {
    if rows.is_empty() {
        return Ok(vec![]);
    }
    knowledge.transaction(|conn| {
        let next = next_seq(conn, chat)?;
        let rows: Vec<serde_json::Value> = rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let mut row = row.clone();
                // `next_seq` is the highest seq already written, so the first record
                // of a fresh chat gets seq 1 and `after_seq = 0` ranges keep it.
                let seq = next
                    .checked_add(1 + i as u64)
                    .filter(|v| *v < i64::MAX as u64)
                    .ok_or("chat sequence overflow")?;
                if row.get("seq").is_some_and(|s| s.as_u64() != Some(seq)) {
                    return Err(format!("chat event sequence conflict"));
                }
                row["seq"] = serde_json::json!(seq);
                // The coverage frontier is compared as a TIMESTAMP, so the wall clock
                // is written in one canonical UTC shape instead of whatever producer
                // conventions (epoch numbers, "created_at") each caller uses.
                if row.get("logged_at").is_none() {
                    row["logged_at"] = serde_json::json!(now_datetime());
                }
                Ok(row)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let batch: Vec<Record<'_, '_>> =
            rows.iter().filter_map(|row| from_row(chat, row)).collect();
        let (_, deferred) = index_batch(conn, &batch)?;
        if deferred > 0 {
            return Err(format!("knowledge index deferred {deferred} records"));
        }
        let mut counter = conn
            .prepare("MERGE (c:Chat {name: $chat}) SET c.next_seq = $next")
            .map_err(|e| e.to_string())?;
        conn.execute(
            &mut counter,
            vec![
                ("chat", lbug::Value::String(chat.into())),
                (
                    "next",
                    lbug::Value::Int64((next + rows.len() as u64) as i64),
                ),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(rows)
    })
}

/// The current UTC wall clock in the canonical `%Y-%m-%d %H:%M:%S` shape.
///
/// No external time crate: this node already owns its formatting, and a hand-rolled
/// civil-from-epoch conversion is short and testable.
pub(crate) fn now_datetime() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let rem = secs % 86400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 to (year, month, day), UTC.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    // Howard Hinnant's civil-from-days algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// The next unused sequence number for one chat, under the caller's lock.
fn next_seq(conn: &lbug::Connection, chat: &str) -> Result<u64, String> {
    let mut stmt = conn
        .prepare("MATCH (c:Chat {name: $chat}) RETURN c.next_seq")
        .map_err(|e| e.to_string())?;
    let rows: Vec<Vec<lbug::Value>> = conn
        .execute(&mut stmt, vec![("chat", lbug::Value::String(chat.into()))])
        .map_err(|e| e.to_string())?
        .map(|tuple| tuple.into_iter().collect())
        .collect();
    match rows.first().and_then(|row| row.first()) {
        Some(lbug::Value::Int64(v)) => Ok((*v).max(0) as u64),
        None | Some(lbug::Value::Null(_)) => {
            let mut statement = conn
                .prepare("MATCH (t:Turn {chat: $chat}) RETURN max(t.seq)")
                .map_err(|e| e.to_string())?;
            let result = conn
                .execute(
                    &mut statement,
                    vec![("chat", lbug::Value::String(chat.into()))],
                )
                .map_err(|e| e.to_string())?
                .next();
            Ok(match result.and_then(|r| r.into_iter().next()) {
                Some(lbug::Value::Int64(v)) => v.max(0) as u64,
                _ => 0,
            })
        }
        _ => Err("knowledge index seq counter unreadable".into()),
    }
}

/// Write a summary to the index with the sequence range it covers.
///
/// The summary belongs to one agent: agents differ in context-length limits, so
/// each compresses with its own budget and its own coverage frontier. Records with
/// `seq_start..=seq_end` are marked covered *for this agent*, so that agent's
/// sequential reads skip them while other agents still see the originals. Coverage
/// is by sequence, not wall clock: a record persisted in the same second as the
/// compaction still gets a higher seq and stays visible.
pub(crate) fn write_summary(
    knowledge: &Knowledge,
    chat: &str,
    agent: &str,
    seq_start: u64,
    seq_end: u64,
    summary: &str,
) -> Result<(), String> {
    if summary.trim().is_empty() || seq_start > seq_end || seq_end >= i64::MAX as u64 {
        return Err("invalid summary coverage".into());
    }
    let id = serde_json::to_string(&(chat, agent, seq_end)).map_err(|e| e.to_string())?;
    knowledge.transaction(|conn| {
        let mut stmt = conn
            .prepare(
                "MERGE (s:Summary {id: $id}) \
                 SET s.chat = $chat, s.agent = $agent, s.seq_start = $seq_start, s.seq_end = $seq_end, s.text = $text",
            )
            .map_err(|e| e.to_string())?;
        conn.execute(
            &mut stmt,
            vec![
                ("id", lbug::Value::String(id.clone())),
                ("chat", lbug::Value::String(chat.into())),
                ("agent", lbug::Value::String(agent.into())),
                ("seq_start", lbug::Value::Int64(seq_start as i64)),
                ("seq_end", lbug::Value::Int64(seq_end as i64)),
                ("text", lbug::Value::String(summary.into())),
            ],
        )
        .map_err(|e| e.to_string())?;
        // Mark every record in the range as covered by this agent's summary
        let mut cover = conn
            .prepare(
                "MATCH (t:Turn {chat: $chat}) \
                 WHERE t.seq >= $seq_start AND t.seq <= $seq_end \
                 MATCH (s:Summary {id: $id}) \
                 MERGE (t)-[:COVERED_BY]->(s)",
            )
            .map_err(|e| e.to_string())?;
        conn.execute(
            &mut cover,
            vec![
                ("chat", lbug::Value::String(chat.into())),
                ("seq_start", lbug::Value::Int64(seq_start as i64)),
                ("seq_end", lbug::Value::Int64(seq_end as i64)),
                ("id", lbug::Value::String(id)),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// Stable identity from the chat sequence coordinate.
///
/// Ladybug supports a single primary-key column, so (chat, seq) folds into one string.
/// Built in Rust and bound as a parameter, never parsed as Cypher.
fn identity(chat: &str, seq: u64) -> String {
    format!("{chat}/{seq}")
}

/// Reduce a logged row to an indexable record.
///
/// Streaming fragments and progress markers ARE kept: the index is the only
/// store, and the web/CLI live feeds replay them in order from it. Search excludes
/// them (`recall` filters the kinds); they never reach a model as recall hits.
/// The full row is kept as `raw`, so display rebuilds whole payloads from it.
pub(crate) fn from_row<'c, 't>(chat: &'c str, row: &'t Value) -> Option<Record<'c, 't>> {
    let seq = row["seq"].as_u64()?;
    let kind = row["type"].as_str().unwrap_or_default();
    let payload = row.get("payload").unwrap_or(row);
    // Tool-started rows carry the tool name instead of prose; it is still worth
    // indexing so `find` can reach them and the timeline keeps its names.
    let text = payload["content"]
        .as_str()
        .or_else(|| payload["text"].as_str())
        .or_else(|| payload["message"].as_str())
        .or_else(|| payload["name"].as_str())
        .unwrap_or_default();
    let created_at = row["logged_at"]
        .as_str()
        .or_else(|| row["created_at"].as_str())
        .map(str::to_owned);
    let raw = serde_json::to_string(row).ok()?;
    Some(Record {
        chat,
        seq,
        kind,
        agent: payload["agent"].as_str().unwrap_or_default(),
        text,
        message_id: payload["message_id"].as_str().unwrap_or_default(),
        raw,
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sustained_history_writes_checkpoint_and_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("history");
        let knowledge = Knowledge::at(&root).unwrap();
        let setting = knowledge
            .rows(
                "CALL current_setting('checkpoint_threshold') RETURN *",
                vec![],
            )
            .unwrap();
        assert_eq!(setting[0][0].to_string(), (1024 * 1024).to_string());
        // Raw + searchable text exceed the old pool in total; individual writes
        // remain modest, as real streaming and tool-result events do.
        let payload = "history data with utf8 内容 ".repeat(4096);
        for i in 0..800 {
            persist_at(&knowledge, "chat", &[json!({"type":"agent.tool.finished","payload":{"content":format!("{i}:{payload}")}})])
                .unwrap_or_else(|error| panic!("write {i}: {error}"));
        }
        drop(knowledge);
        let reopened = Knowledge::at(&root).unwrap();
        assert_eq!(
            super::super::recall::latest_seq(&reopened, "chat").unwrap(),
            Some(800)
        );
        let tail = super::super::recall::range(&reopened, "chat", 799, 801, 1, false).unwrap();
        assert_eq!(tail.len(), 1);
        let row: Value = serde_json::from_str(tail[0].raw.as_deref().unwrap()).unwrap();
        assert_eq!(row["payload"]["content"], format!("799:{payload}"));
        persist_at(
            &reopened,
            "chat",
            &[json!({"type":"user","content":"continues"})],
        )
        .unwrap();
        assert_eq!(
            super::super::recall::latest_seq(&reopened, "chat").unwrap(),
            Some(801)
        );
    }

    #[test]
    fn identity_is_the_jsonl_coordinate() {
        assert_eq!(identity("design", 7), "design/7");
    }

    /// The canonical timestamp is a UTC civil shape, so TIMESTAMP comparisons and
    /// `ORDER BY end_time` are chronological and lexicographic at once.
    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(20_736), (2026, 10, 10));
        // Leap day: 2024-02-29 is 19782 days after the epoch.
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }

    #[test]
    fn now_datetime_is_a_utc_civil_shape() {
        let text = now_datetime();
        assert_eq!(text.len(), 19, "{text}");
        assert!(text.starts_with("20"), "{text}");
        assert_eq!(&text[4..5], "-", "{text}");
        assert_eq!(&text[10..11], " ", "{text}");
    }

    /// Values are bound as parameters, never interpolated, so hostile content cannot
    /// alter the statement. This is the parametrized equivalent of the old escaping
    /// test: content shaped like an injection must land as inert text.
    #[tokio::test]
    async fn hostile_content_is_stored_as_inert_text() {
        let dir = tempfile::tempdir().unwrap();
        let knowledge = Knowledge::at(&dir.path().join("kb")).unwrap();
        let rows =
            vec![json!({"seq":1,"type":"user","content":"x'}), (c:Chat {name:'pwned'}) //"})];
        let batch: Vec<Record<'_, '_>> = rows.iter().filter_map(|r| from_row("c", r)).collect();
        assert_eq!(ingest(&knowledge, &batch).unwrap().indexed, 1);
        let chats = knowledge
            .rows("MATCH (c:Chat) RETURN c.name", vec![])
            .unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0][0].to_string(), "c");
        let turns = knowledge
            .rows("MATCH (t:Turn) RETURN count(t)", vec![])
            .unwrap();
        assert_eq!(turns[0][0].to_string(), "1");
    }

    #[test]
    fn fragments_are_kept_for_display_and_textless_rows_are_empty() {
        // Streaming fragments stay in the index (the only store, and live feeds
        // replay them in order); search excludes them, display needs them.
        let delta_row = json!({"seq":1,"type":"text_delta","text":"hi"});
        let delta = from_row("c", &delta_row).unwrap();
        assert_eq!(delta.kind, "text_delta");
        assert_eq!(delta.text, "hi");
        // A row without any text is still recorded for display, but carries nothing
        // searchable; its raw payload still remains available for display.
        let bare_row = json!({"seq":2,"type":"tool_started","payload":{"name":"x"}});
        let bare = from_row("c", &bare_row).unwrap();
        assert_eq!(bare.text, "x");
        let row = json!({"seq":3,"type":"user","content":"do it"});
        let record = from_row("c", &row).unwrap();
        assert_eq!(record.text, "do it");
        assert_eq!(record.kind, "user");
    }

    #[test]
    fn reads_the_final_answer_out_of_a_payload() {
        let row = json!({"seq":4,"payload":{"type":"agent.done","content":"finished"}});
        let record = from_row("c", &row).unwrap();
        assert_eq!(record.text, "finished");
    }

    #[test]
    fn a_row_without_a_sequence_is_not_indexable() {
        assert!(from_row("c", &json!({"type":"user","content":"x"})).is_none());
    }

    /// Replay after a crash must not duplicate: the node id is the chat sequence coordinate.
    #[tokio::test]
    async fn replaying_the_same_records_indexes_each_once() {
        let dir = tempfile::tempdir().unwrap();
        let knowledge = Knowledge::at(&dir.path().join("kb")).unwrap();
        let rows = [
            json!({"seq":1,"type":"user","content":"we chose axum"}),
            json!({"seq":2,"type":"agent.done","payload":{"content":"agreed"}}),
        ];
        let batch: Vec<Record<'_, '_>> = rows
            .iter()
            .filter_map(|row| from_row("design", row))
            .collect();
        let first = ingest(&knowledge, &batch).unwrap();
        let second = ingest(&knowledge, &batch).unwrap();
        assert_eq!(first.indexed, 2);
        assert_eq!(second.indexed, 2);
        let counted = knowledge
            .rows("MATCH (t:Turn) RETURN count(t)", vec![])
            .unwrap();
        assert_eq!(counted[0][0].to_string(), "2");
    }

    #[tokio::test]
    async fn a_chat_with_a_quote_in_its_name_is_stored_and_reachable() {
        let dir = tempfile::tempdir().unwrap();
        let knowledge = Knowledge::at(&dir.path().join("kb")).unwrap();
        let chat = "it's a chat";
        let rows = vec![json!({"seq":1,"type":"user","content":"hello"})];
        let batch: Vec<Record<'_, '_>> = rows.iter().filter_map(|r| from_row(chat, r)).collect();
        assert_eq!(ingest(&knowledge, &batch).unwrap().indexed, 1);
        let found = knowledge
            .rows(
                "MATCH (t:Turn)-[:IN_CHAT]->(c:Chat) RETURN t.text, c.name",
                vec![],
            )
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0][1].to_string(), chat);
    }
}

#[cfg(test)]
mod probe {
    use super::*;
    use crate::storage::knowledge::graph::Knowledge;
    use serde_json::json;

    /// The exact Turn MERGE the write path runs, with a real Timestamp binding:
    /// Ladybug rejects STRING for the TIMESTAMP column, so this pins that the
    /// write path's conversion (in `index_batch`) is the one that works.
    #[test]
    fn turn_merge_accepts_a_timestamp_binding() {
        let dir = tempfile::tempdir().unwrap();
        let knowledge = Knowledge::at(&dir.path().join("kb")).unwrap();
        let row = json!({"seq":1,"type":"user","content":"hi"});
        let record = from_row("c", &row).unwrap();
        let raw = record.raw.clone();
        let created_at = time::OffsetDateTime::UNIX_EPOCH;
        let outcome = knowledge.batch(|conn| {
            let mut turn = conn
                .prepare(
                    "MERGE (t:Turn {id: $id}) \
                     SET t.chat = $chat, t.seq = $seq, t.kind = $kind, t.agent = $agent, \
                         t.text = $text, t.message_id = $message_id, t.raw = $raw, t.created_at = $created_at",
                )
                .map_err(|e| format!("prepare: {e}"))?;
            conn.execute(
                &mut turn,
                vec![
                    ("id", lbug::Value::String("c/1".into())),
                    ("chat", lbug::Value::String("c".into())),
                    ("seq", lbug::Value::Int64(1)),
                    ("kind", lbug::Value::String("user".into())),
                    ("agent", lbug::Value::String("".into())),
                    ("text", lbug::Value::String("hi".into())),
                    ("message_id", lbug::Value::String("".into())),
                    ("raw", lbug::Value::String(raw.clone())),
                    ("created_at", lbug::Value::Timestamp(created_at)),
                ],
            )
            .map_err(|e| format!("execute: {e}"))?;
            Ok(())
        });
        assert_eq!(outcome, Ok(()));
    }
}
