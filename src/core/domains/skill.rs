use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: u64,

    pub name: String,
    pub summary: String,
    pub input: String,  // 输入的schema
    pub output: String, // 输出的描述
    pub skill: String,  // 具体的skill实现，可能是一个函数调用，也可能是调用llm进行逻辑处理
}