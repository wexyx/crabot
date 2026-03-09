use serde::{Deserialize, Serialize};
use rbatis::{crud, rbdc::datetime::DateTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: u64,
    pub creator: String,
    pub modifier: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub is_deleted: bool,

    pub name: String,
    pub summary: String,
    pub input: String,
    pub output: String,
    pub skill: String,
}

// Automatically generate CRUD methods
crud!(Skill{});