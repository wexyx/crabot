use serde::{Deserialize, Serialize};
use rbatis::{crud, rbdc::datetime::DateTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Employee {
    pub id: u64,
    pub creator: String,
    pub modifier: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub is_deleted: bool,

    pub name: String,
    pub department_id: u64,
}

// Automatically generate CRUD methods
crud!(Employee{});