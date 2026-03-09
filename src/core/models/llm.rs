use serde::{Deserialize, Serialize};
use rbatis::{crud, rbdc::datetime::DateTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: u64,
    pub creator: String,
    pub modifier: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub is_deleted: bool,

    pub name: String,
    pub config: String,
    pub cost: i64,
    pub budget: i64,
}

// Automatically generate CRUD methods
crud!(Agent{});
