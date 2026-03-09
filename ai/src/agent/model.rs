use openai_api_rs::v1::chat_completion::ChatCompletionMessage;
use serde::{Deserialize, Serialize};


#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AgentConfig {
    pub prompt: String,          // 系统提示词
    pub model: String,           // 指定具体的模型
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AgentContext {
    pub messages: Vec<ChatCompletionMessage>,
    pub status: AgentStatus,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AgentStatus {
    pub stop: bool,             // 是否结束
    pub stop_reason: Option<String>,    // 结束原因
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct NodeInfo {
    pub key: String,
    pub goal: String,
    pub result: Option<String>,
    pub requirements: Vec<Requirement>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PlanInfo {
    pub result: Option<String>,
    pub finish: bool,
    pub nodes: Option<Vec<NodeInfo>>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Requirement {
    pub key: String,
    pub reason: String,
    pub result: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct ToolCallInfo {
    pub result: Option<String>,
    pub params: Option<String>,
    pub requirements: Vec<Requirement>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema() {
        let plan = PlanInfo {
            result: Some("本轮调度结果，基于上下文直接给出结论，不再需要任务调度数据".to_string()),
            finish: false,
            nodes: Some(vec![NodeInfo{ 
                key: "节点的key，即传入的tools的nama数据".to_string(), 
                goal: "节点执行目的，即说明为什么要执行".to_string(), 
                result: Some("节点执行结果，基于上下文直接给出结论，不再需要任务调度数据".to_string()), 
                requirements: vec![Requirement { 
                    key: "依赖的参数key，全局唯一".to_string(), 
                    reason: "依赖的参数信息，即为什么需要这个参数".to_string(), 
                    result: None,
                }] 
            }]),
        };

        println!("{}", serde_json::to_string_pretty(&plan).unwrap_or_default());

        let info = ToolCallInfo {
            result: Some("本轮调度结果，基于上下文直接给出结论，不再需要任务调度数据".to_string()),
            params: Some("基于tool的scheme的请求入参数，json格式".to_string()),
            requirements: vec![Requirement { 
                key: "依赖的参数key，全局唯一".to_string(), 
                reason: "依赖的参数信息，即为什么需要这个参数".to_string(), 
                result: None,
            }] 
        };

        println!("{}", serde_json::to_string_pretty(&info).unwrap_or_default());
    }
}
