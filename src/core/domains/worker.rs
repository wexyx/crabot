use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    pub id: u64,

    pub name: String,
    pub summary: String,
    pub dag: String,
    pub input: String,
    pub output: String,
}
