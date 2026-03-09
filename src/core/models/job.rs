use serde::{Deserialize, Serialize};
use rbatis::{crud, rbdc::datetime::DateTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub creator: String,
    pub modifier: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub is_deleted: bool,

    pub session_id: u64,
    pub message_id: u64,
    pub dag: String,

    pub last_at: DateTime,
    pub next_at: DateTime,
    pub finish: bool,
}

// Automatically generate CRUD methods
crud!(Job{});
