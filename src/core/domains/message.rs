use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: u64,

    pub content: String,
    pub role: String,
    pub from_user: String,
}