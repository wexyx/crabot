use serde::{Deserialize, Serialize};
use rbatis::rbdc::datetime::DateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,

    pub session_id: u64,
    pub message_id: u64,
    pub dag: String, // 是skill的执行逻辑，其本质就是skill的dag

    pub last_at: DateTime,
    pub next_at: DateTime,
    pub finish: bool,
}
