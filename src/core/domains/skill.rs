use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: u64,
    pub name: String,
    pub summary: String,
    pub skill: String,
}
