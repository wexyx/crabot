use lbug::{Connection, Database, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

/// Own the database; short-lived connections borrow it only while access is locked.
/// This avoids leaking a whole buffer pool whenever an index is opened and dropped.
pub(crate) struct Knowledge {
    db: Database,
    /// Serialises writes and keeps each multi-statement transaction on one connection.
    access: Mutex<()>,
    fts: OnceLock<bool>,
}

/// One open database per project directory.
///
/// A single global would let the first project opened win for the whole process, and
/// every later project would then read and write the wrong graph. The map is keyed by
/// the canonicalised directory so two spellings of one project share one handle.
static OPEN: OnceLock<Mutex<HashMap<PathBuf, (Arc<Knowledge>, Instant)>>> = OnceLock::new();
const CACHED_PROJECTS: usize = 8;

/// Open the index for one project, or `None` when it is unavailable.
///
/// Every storage namespace has its own index; handles never cross project boundaries.
pub(crate) fn for_project(project: uuid::Uuid) -> Option<Arc<Knowledge>> {
    open(
        &agent_runtime::paths::data_dir()
            .join("knowledge")
            .join(project.to_string()),
    )
}

/// Open, creating the schema if this is a fresh directory.
///
/// This is the primary history store. Callers must report `None` as unavailable;
/// there is no file-log fallback.
fn open(root: &Path) -> Option<Arc<Knowledge>> {
    std::fs::create_dir_all(root).ok()?;
    let key = std::fs::canonicalize(root).ok()?;
    let map = OPEN.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = map.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((knowledge, touched)) = map.get_mut(&key) {
        *touched = Instant::now();
        return Some(knowledge.clone());
    }
    let knowledge = match Knowledge::at(&key) {
        Ok(db) => Arc::new(db),
        Err(error) => {
            eprintln!("history index open failed: {error}");
            return None;
        }
    };
    // Evict idle handles only. An in-flight reader/writer holds its own Arc, so
    // it is never closed or reopened underneath a query.
    let mut retired = Vec::new();
    while map.len() >= CACHED_PROJECTS {
        let oldest = map
            .iter()
            .filter(|(_, (db, _))| Arc::strong_count(db) == 1)
            .min_by_key(|(_, (_, touched))| *touched)
            .map(|(path, _)| path.clone());
        let Some(oldest) = oldest else { break };
        retired.push(map.remove(&oldest));
    }
    map.insert(key, (knowledge.clone(), Instant::now()));
    // Finish closing before another opener can reacquire this file's write lock.
    drop(retired);
    drop(map);
    Some(knowledge)
}

impl Knowledge {
    /// Open at one directory, for tests and for the per-project map.
    pub(crate) fn at(root: &Path) -> Result<Self, String> {
        // The engine takes a file path, not a directory: it creates the database
        // file itself, and handing it an existing directory fails. The directory
        // stays the unit the rest of the node reasons about, so the file lives in it.
        if let Err(error) = std::fs::create_dir_all(root) {
            return Err(format!("knowledge index directory: {error}"));
        }
        let db = Database::new(root.join("graph.db"), super::memory::configuration()?)
            .map_err(|e| e.to_string())?;
        let knowledge = Self {
            db,
            access: Mutex::new(()),
            fts: OnceLock::new(),
        };
        knowledge.ensure_schema()?;
        Ok(knowledge)
    }

    /// Create the graph if absent, and load FTS.
    ///
    /// Every statement here must tolerate a previous open, because the schema outlives the
    /// process. Table creation treats "already exists" as success; the FTS index is
    /// checked through `SHOW_INDEXES` because recreating it would discard every posting.
    fn ensure_schema(&self) -> Result<(), String> {
        // Identity is a single string: Ladybug supports one primary key column, so the
        // chat coordinate (chat, seq) is folded into `id`. `raw` carries the complete
        // event row (the index is now the only store, so display needs full payloads),
        // and `message_id` keeps resume-point truncation working.
        for statement in [
            "CREATE NODE TABLE Turn(id STRING PRIMARY KEY, chat STRING, seq INT64, kind STRING, agent STRING, text STRING, message_id STRING, raw STRING, created_at TIMESTAMP)",
            "CREATE NODE TABLE Chat(name STRING PRIMARY KEY, next_seq INT64)",
            "CREATE NODE TABLE Session(id STRING PRIMARY KEY)",
            "CREATE NODE TABLE Summary(id STRING PRIMARY KEY, chat STRING, agent STRING, seq_start INT64, seq_end INT64, text STRING)",
            "CREATE REL TABLE IN_CHAT(FROM Turn TO Chat)",
            "CREATE REL TABLE IN_SESSION(FROM Turn TO Session)",
            "CREATE REL TABLE COVERED_BY(FROM Turn TO Summary)",
        ] {
            // "already exists" is the expected case on every open after the first.
            let _ = self.run(statement);
        }
        // Indexes created before this branch gained the extra columns keep working:
        // apply missing columns, then verify the required schema below. An unavailable
        // column must fail opening the store rather than silently lose history.
        for statement in [
            "ALTER NODE TABLE Turn ADD message_id STRING",
            "ALTER NODE TABLE Turn ADD raw STRING",
            "ALTER NODE TABLE Chat ADD next_seq INT64",
            "ALTER NODE TABLE Summary ADD agent STRING",
            "ALTER NODE TABLE Summary ADD seq_start INT64",
            "ALTER NODE TABLE Summary ADD seq_end INT64",
        ] {
            let _ = self.run(statement);
        }
        // FTS is an extension rather than part of the engine. Release bundles ship it
        // beside the binary and `load_fts` tries that path before the user cache or the
        // network, so first run works offline.
        // Fail closed: this is the primary store, not a disposable cache.
        self.rows(
            "MATCH (t:Turn) RETURN t.raw, t.seq, t.message_id LIMIT 0",
            vec![],
        )?;
        self.rows(
            "MATCH (s:Summary) RETURN s.agent, s.seq_start, s.seq_end LIMIT 0",
            vec![],
        )?;
        if self.has_fts() {
            let indexed = self
                .rows("CALL SHOW_INDEXES() RETURN index_name", vec![])
                .map(|rows| {
                    rows.iter()
                        .any(|row| row.first().is_some_and(|v| v.to_string() == "turn_text"))
                })
                .unwrap_or(false);
            if !indexed {
                let _ = self.run("CALL CREATE_FTS_INDEX('Turn', 'turn_text', ['text'])");
            }
        }
        Ok(())
    }

    /// Whether full-text search is usable. Literal predicates still work without it.
    ///
    /// Loading the extension registers its functions for this connection; a repeat
    /// load is cheap and keeps `has_fts` a pure probe with no caller-side state.
    pub(crate) fn has_fts(&self) -> bool {
        *self.fts.get_or_init(|| self.load_fts().is_ok())
    }

    fn load_fts(&self) -> Result<(), String> {
        if let Some(path) = bundled_fts_extension() {
            if self
                .run(&format!("LOAD EXTENSION '{}'", cypher_string(&path)))
                .is_ok()
            {
                return Ok(());
            }
        }
        self.run("LOAD EXTENSION FTS")
    }

    pub(crate) fn run(&self, cypher: &str) -> Result<(), String> {
        self.batch(|conn| conn.query(cypher).map(|_| ()).map_err(|e| e.to_string()))
    }

    /// Rows of a read query with bound parameters.
    ///
    /// Values never enter the query text, so a chat name or a message containing a
    /// quote cannot change the statement.
    pub(crate) fn rows(
        &self,
        cypher: &str,
        params: Vec<(&str, Value)>,
    ) -> Result<Vec<Vec<Value>>, String> {
        self.batch(|conn| {
            let mut statement = conn.prepare(cypher).map_err(|e| e.to_string())?;
            let result = conn
                .execute(&mut statement, params)
                .map_err(|e| e.to_string())?;
            Ok(result.map(|tuple| tuple.into_iter().collect()).collect())
        })
    }

    pub(crate) fn transaction<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        self.batch(|conn| {
            conn.query("BEGIN TRANSACTION").map_err(|e| e.to_string())?;
            match f(conn) {
                Ok(value) => match conn.query("COMMIT") {
                    Ok(_) => Ok(value),
                    Err(error) if super::commit_status::is_durable(&error.to_string()) => {
                        // Ladybug explicitly reports that the WAL commit succeeded.
                        // Reporting this as a rejected append invites duplicate writes.
                        eprintln!("history checkpoint deferred (append is durable): {error}; consider increasing CRABOT_HISTORY_BUFFER_MIB and restarting");
                        Ok(value)
                    }
                    Err(error) => {
                        let _ = conn.query("ROLLBACK");
                        Err(error.to_string())
                    }
                },
                Err(error) => {
                    let _ = conn.query("ROLLBACK");
                    Err(error)
                }
            }
        })
    }

    /// Run several statements under one lock.
    ///
    /// Ingest writes a node plus its edges per record; holding the lock across the
    /// group keeps one batch from interleaving with another on the shared connection.
    /// The closure receives the connection directly because the lock is already held —
    /// calling `run` or `rows` from inside would deadlock on the non-reentrant mutex.
    pub(crate) fn batch<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let _guard = self.access.lock().unwrap_or_else(|e| e.into_inner());
        let conn = Connection::new(&self.db).map_err(|e| e.to_string())?;
        f(&conn)
    }
}

fn cypher_string(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
}

fn bundled_fts_extension() -> Option<PathBuf> {
    std::env::var_os("CRABOT_LBUG_FTS_EXTENSION")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|exe| {
                    exe.parent()?
                        .parent()
                        .map(|root| root.join("lib/lbug/fts/libfts.lbug_extension"))
                })
                .filter(|path| path.is_file())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_and_declares_its_schema() {
        let dir = tempfile::tempdir().unwrap();
        let knowledge = Knowledge::at(&dir.path().join("kb")).unwrap();
        // Re-opening the same directory must not fail on existing tables.
        assert!(knowledge.run("MATCH (t:Turn) RETURN count(t)").is_ok());
        assert!(knowledge.run("MATCH (c:Chat) RETURN count(c)").is_ok());
        assert!(
            knowledge
                .run("MATCH (t:Turn)-[:IN_CHAT]->(c:Chat) RETURN count(*)")
                .is_ok()
        );
        assert!(
            knowledge
                .run("MATCH (t:Turn)-[:IN_SESSION]->(s:Session) RETURN count(*)")
                .is_ok()
        );
    }

    /// Two spellings of one project share one handle, and two projects never share.
    #[test]
    fn opens_are_keyed_by_project_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("kb");
        let first = open(&root).unwrap();
        let second = open(&root.join(".")).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        let other = open(&dir.path().join("other")).unwrap();
        assert!(!Arc::ptr_eq(&first, &other));
    }

    #[test]
    fn idle_cached_databases_release_their_pool_but_active_handles_survive() {
        let dir = tempfile::tempdir().unwrap();
        let active = open(&dir.path().join("active")).unwrap();
        let idle = open(&dir.path().join("idle")).unwrap();
        let retired = Arc::downgrade(&idle);
        drop(idle);
        for i in 0..CACHED_PROJECTS + 1 {
            drop(open(&dir.path().join(format!("other-{i}"))).unwrap());
        }
        assert!(retired.upgrade().is_none());
        assert!(Arc::ptr_eq(
            &active,
            &open(&dir.path().join("active")).unwrap()
        ));
        assert!(
            open(&dir.path().join("idle"))
                .unwrap()
                .run("MATCH (t:Turn) RETURN count(t)")
                .is_ok()
        );
    }
}
