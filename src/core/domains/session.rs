use serde::{Deserialize, Serialize};

use crate::core::domains;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: u64,
   
    pub name: String,
    pub session: String,
    pub extra: String,
    
    pub messages: Vec<domains::message::Message>,
}

impl Session {
}