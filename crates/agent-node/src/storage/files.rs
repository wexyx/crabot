//! Incremental state journal with legacy snapshot import. No SQL engine.
use crate::*;
use fs2::FileExt;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path as FilePath, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Data {
    format_version: u32,
    pub sequence: u64,
    pub(super) collections: HashMap<String, HashMap<String, Value>>,
    #[serde(skip)]
    undo: Option<HashMap<(String, String), Option<Value>>>,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            format_version: 1,
            sequence: 0,
            collections: HashMap::new(),
            undo: None,
        }
    }
}
impl Data {
    pub fn get(&self, collection: &str, key: &str) -> Option<&Value> {
        self.collections.get(collection)?.get(key)
    }
    pub fn list(&self, collection: &str) -> Vec<Value> {
        self.collections
            .get(collection)
            .map(|c| c.values().cloned().collect())
            .unwrap_or_default()
    }
    fn track(&mut self, collection: &str, key: &str) {
        if self.undo.is_some() {
            let identity = (collection.to_owned(), key.to_owned());
            if !self.undo.as_ref().unwrap().contains_key(&identity) {
                let previous = self.get(collection, key).cloned();
                self.undo.as_mut().unwrap().insert(identity, previous);
            }
        }
    }
    pub fn remove(&mut self, collection: &str, key: &str) {
        self.track(collection, key);
        if let Some(rows) = self.collections.get_mut(collection) {
            rows.remove(key);
        }
    }
    pub fn retain(&mut self, collection: &str, keep: impl Fn(&Value) -> bool) {
        let keys: Vec<_> = self
            .collections
            .get(collection)
            .into_iter()
            .flat_map(|rows| rows.iter())
            .filter(|(_, value)| !keep(value))
            .map(|(key, _)| key.clone())
            .collect();
        for key in keys {
            self.remove(collection, &key);
        }
    }
    fn rollback(&mut self) {
        for ((collection, key), previous) in self.undo.take().unwrap_or_default() {
            match previous {
                Some(value) => self.set(&collection, &key, value),
                None => self.remove(&collection, &key),
            }
        }
    }
    pub fn set(&mut self, collection: &str, key: &str, value: Value) {
        self.track(collection, key);
        self.collections
            .entry(collection.into())
            .or_default()
            .insert(key.into(), value);
    }
    pub fn insert(&mut self, collection: &str, key: &str, value: Value) -> Result<(), String> {
        if self.get(collection, key).is_some() {
            return Err("already_exists".into());
        }
        self.set(collection, key, value);
        Ok(())
    }
    pub fn credential(&mut self, value: Value) -> Result<(), String> {
        if self
            .list("credentials")
            .iter()
            .any(|c| c["project_id"] == value["project_id"] && c["client_id"] == value["client_id"])
        {
            return Err("already_exists".into());
        }
        let key = field(&value, "ak");
        self.insert("credentials", &key, value)
    }
}
struct Inner {
    data: std::sync::Mutex<Data>,
    /// Serializes durable commits so the journal sequence stays ordered and a rejected
    /// write can roll back without another transaction interleaving.
    commit: std::sync::Mutex<()>,
    file: Option<PathBuf>,
    // OS advisory lock is released even after a crash; never unlink the lock file.
    _lock: Option<File>,
}
/// The only parts of a run row that hot paths need. A `runs` row also carries the whole
/// prompt and the hydrated skill catalog, so cloning it per streamed token is pure cost.
#[derive(Clone, Debug, PartialEq)]
pub struct RunMeta {
    pub status: Option<String>,
    pub group_id: Option<String>,
}
#[derive(Clone)]
pub struct Store(Arc<Inner>);
impl Store {
    #[cfg(test)]
    pub fn memory() -> Self {
        Self(Arc::new(Inner {
            data: Default::default(),
            commit: Default::default(),
            file: None,
            _lock: None,
        }))
    }
    pub(crate) fn skill_row(&self, collection: &str, row: &Value) -> Result<Value, String> {
        match self.0.file.as_ref().and_then(|p| p.parent()) {
            Some(root) => super::skill_files::hydrate(root, collection, row),
            None => Ok(row.clone()),
        }
    }
    pub async fn get(&self, c: &str, k: &str) -> Option<Value> {
        self.0.data.lock().unwrap().get(c, k).cloned()
    }
    /// Read-only run metadata without materializing the row.
    pub async fn run_meta(&self, k: &str) -> Option<RunMeta> {
        let data = self.0.data.lock().unwrap();
        let row = data.get("runs", k)?;
        Some(RunMeta {
            status: row["status"].as_str().map(str::to_owned),
            group_id: row["group_id"].as_str().map(str::to_owned),
        })
    }
    pub async fn list(&self, c: &str) -> Vec<Value> {
        self.0.data.lock().unwrap().list(c)
    }
    pub async fn insert(&self, c: &str, k: &str, v: Value) -> Result<(), String> {
        self.transaction(|d| d.insert(c, k, v)).await
    }
    /// No await between durable commit and publishing memory: cancellation cannot split them.
    pub async fn transaction<T>(
        &self,
        change: impl FnOnce(&mut Data) -> Result<T, String>,
    ) -> Result<T, String> {
        self.transaction_sync(change)
    }
    fn transaction_sync<T>(
        &self,
        change: impl FnOnce(&mut Data) -> Result<T, String>,
    ) -> Result<T, String> {
        let _commit = self
            .0
            .commit
            .lock()
            .map_err(|_| "file store lock poisoned")?;
        // The state lock covers memory only. Holding it across the journal's fdatasync
        // stalled every concurrent reader on disk latency, which is the whole process.
        let (result, pending, bump) = {
            let mut data = self.0.data.lock().map_err(|_| "file store lock poisoned")?;
            data.undo = Some(HashMap::new());
            let result = match change(&mut data) {
                Ok(result) => result,
                Err(error) => {
                    data.rollback();
                    return Err(error);
                }
            };
            let mut changes: Vec<_> = data
                .undo
                .as_ref()
                .unwrap()
                .iter()
                .filter_map(|((collection, key), previous)| {
                    let value = data.get(collection, key).cloned();
                    (value != *previous).then(|| super::state_journal::Change {
                        collection: collection.clone(),
                        key: key.clone(),
                        deleted: value.is_none(),
                        value: value.unwrap_or(Value::Null),
                    })
                })
                .collect();
            let mut pending = None;
            let mut bump = None;
            if !changes.is_empty() {
                let Some(sequence) = data.sequence.checked_add(1) else {
                    data.rollback();
                    return Err("state sequence overflow".into());
                };
                if let Some(path) = &self.0.file {
                    for change in &mut changes {
                        if change.deleted {
                            continue;
                        }
                        match super::skill_files::persist(
                            path.parent().unwrap(),
                            &change.collection,
                            &change.key,
                            &change.value,
                        ) {
                            Ok(value) => {
                                // Keep definitions hydrated for transaction validation, but publish
                                // only references to their files in the durable JSONL journal.
                                if let Some(pointer) = value.get("skill_files_directory") {
                                    let mut hydrated = change.value.clone();
                                    hydrated["skill_files_directory"] = pointer.clone();
                                    data.set(&change.collection, &change.key, hydrated);
                                }
                                change.value = value;
                            }
                            Err(error) => {
                                data.rollback();
                                return Err(error);
                            }
                        }
                    }
                    pending = Some((path.clone(), sequence, changes));
                }
                bump = Some(sequence);
            }
            (result, pending, bump)
        };
        if let Some((path, sequence, changes)) = pending {
            if let Err(error) = super::state_journal::append(&path, sequence, changes) {
                // The commit lock keeps the undo log valid, so memory is restored.
                self.0
                    .data
                    .lock()
                    .map_err(|_| "file store lock poisoned")?
                    .rollback();
                return Err(error);
            }
        }
        let mut data = self.0.data.lock().map_err(|_| "file store lock poisoned")?;
        if let Some(sequence) = bump {
            data.sequence = sequence;
        }
        data.undo = None;
        Ok(result)
    }
}
fn private_file(path: &FilePath, exclusive: bool) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    if exclusive {
        options.create_new(true);
    } else {
        options.create(true).truncate(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
pub async fn open(dir: &FilePath) -> Result<Store, String> {
    if !dir.exists() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(dir).map_err(|e| e.to_string())?;
    }
    let mut lock = private_file(&dir.join("store.lock"), false).map_err(|e| e.to_string())?;
    lock.try_lock_exclusive().map_err(|_| {
        let owner=std::fs::read_to_string(dir.join("store.lock")).unwrap_or_default();
        format!("data directory {} is already in use by another Crabot ({owner}). Use another --name or close the existing instance.",dir.display())
    })?;
    lock.set_len(0).map_err(|e| e.to_string())?;
    write!(
        lock,
        "pid={} instance={}",
        std::process::id(),
        std::env::var("CRABOT_INSTANCE").unwrap_or_else(|_| "default".into())
    )
    .map_err(|e| e.to_string())?;
    lock.sync_all().map_err(|e| e.to_string())?;
    let path = dir.join("state.json");
    let mut data = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Data>(&bytes)
            .map_err(|e| format!("invalid state.json; refusing to overwrite: {e}"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Data::default(),
        Err(e) => return Err(e.to_string()),
    };
    if data.format_version != 1 {
        return Err("unsupported state.json format version".into());
    }
    // Chat records live only in the knowledge index now; the JSONL layer is gone.
    let journal = dir.join("state.jsonl");
    super::state_journal::replay(&journal, &mut data)?;
    for collection in ["skills", "management_skills", "capability_library"] {
        if let Some(rows) = data.collections.get_mut(collection) {
            for row in rows.values_mut() {
                *row = super::skill_files::hydrate(dir, collection, row)?;
            }
        }
    }
    Ok(Store(Arc::new(Inner {
        data: std::sync::Mutex::new(data),
        commit: Default::default(),
        file: Some(journal),
        _lock: Some(lock),
    })))
}
pub fn field(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().into()
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub async fn restore(state: &AppState) {
    for row in state.store.list("sessions").await {
        let id = Uuid::parse_str(&field(&row, "id")).expect("session ID");
        let (events, _) = broadcast::channel(256);
        state.sessions.lock().await.insert(
            id,
            Session {
                project_id: Uuid::parse_str(&field(&row, "project_id")).expect("project ID"),
                client_id: row["client_id"].as_str().map(str::to_owned),
                events,
                requests: HashMap::new(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("crabot-files-test-{}", Uuid::new_v4())))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[tokio::test]
    async fn json_reopens_and_exclusive_lock_releases() {
        let dir = Temp::new();
        let store = open(&dir.0).await.unwrap();
        assert!(open(&dir.0).await.is_err());
        store
            .insert("sessions", "a", json!({"content":"你好"}))
            .await
            .unwrap();
        let on_disk: Value =
            serde_json::from_slice(&std::fs::read(dir.0.join("state.jsonl")).unwrap()).unwrap();
        assert_eq!(on_disk["changes"][0]["value"]["content"], "你好");
        assert!(!dir.0.join("state.json").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(dir.0.join("state.jsonl"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        drop(store);
        let again = open(&dir.0).await.unwrap();
        assert_eq!(again.get("sessions", "a").await.unwrap()["content"], "你好");
    }
    #[tokio::test]
    async fn corrupt_file_is_not_silently_reset() {
        let dir = Temp::new();
        drop(open(&dir.0).await.unwrap());
        std::fs::write(dir.0.join("state.json"), b"invalid-json").unwrap();
        assert!(open(&dir.0).await.is_err());
        assert_eq!(
            std::fs::read(dir.0.join("state.json")).unwrap(),
            b"invalid-json"
        );
    }
    #[tokio::test]
    async fn run_meta_reports_status_and_group_without_materializing_the_row() {
        let dir = Temp::new();
        let store = open(&dir.0).await.unwrap();
        store
            .insert(
                "runs",
                "r1",
                json!({"status":"running","group_id":"g1","prompt":"x".repeat(200_000)}),
            )
            .await
            .unwrap();
        assert_eq!(
            store.run_meta("r1").await,
            Some(RunMeta {
                status: Some("running".into()),
                group_id: Some("g1".into())
            })
        );
        assert_eq!(store.run_meta("missing").await, None);
        // The bulky fields stay untouched on disk.
        let row = store.get("runs", "r1").await.unwrap();
        assert_eq!(row["prompt"].as_str().unwrap().len(), 200_000);
    }
    #[tokio::test]
    async fn failed_transaction_and_failed_disk_write_do_not_change_memory() {
        let dir = Temp::new();
        let store = open(&dir.0).await.unwrap();
        store.insert("test", "a", json!(1)).await.unwrap();
        let failed: Result<(), String> = store
            .transaction(|d| {
                d.set("test", "a", json!(2));
                Err("reject".into())
            })
            .await;
        assert!(failed.is_err());
        assert_eq!(store.get("test", "a").await, Some(json!(1)));
        std::fs::rename(dir.0.join("state.jsonl"), dir.0.join("saved.jsonl")).unwrap();
        std::fs::create_dir(dir.0.join("state.jsonl")).unwrap();
        assert!(store.insert("test", "b", json!(3)).await.is_err());
        assert!(store.get("test", "b").await.is_none());
        assert_eq!(store.get("test", "a").await, Some(json!(1)));
    }
    #[tokio::test]
    async fn file_backed_policy_cas_is_atomic_and_project_scoped() {
        let dir = Temp::new();
        let store = open(&dir.0).await.unwrap();
        let policy = policy_store::Store::files(store.clone());
        let p = Uuid::new_v4();
        policy
            .put(p, "subgroup", "g", 0, json!({"mode":"relay"}))
            .await
            .unwrap();
        let (a, b) = tokio::join!(
            policy.put(p, "subgroup", "g", 1, json!({"v":"a"})),
            policy.put(p, "subgroup", "g", 1, json!({"v":"b"}))
        );
        assert_ne!(a.is_ok(), b.is_ok());
        assert!(policy.get(Uuid::new_v4(), "subgroup", "g").await.is_err());
        drop(policy);
        drop(store);
        let reloaded = policy_store::Store::files(open(&dir.0).await.unwrap());
        assert_eq!(reloaded.get(p, "subgroup", "g").await.unwrap().version, 2);
    }
}
