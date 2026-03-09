use serde::{Deserialize, Serialize};
use rbatis::{crud, rbdc::datetime::DateTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Department {
    pub id: u64,
    pub creator: String,
    pub modifier: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub is_deleted: bool,

    pub name: String,
    pub role: String,
    pub parant_department_id: u64,
}

// Automatically generate CRUD methods
crud!(Department{});