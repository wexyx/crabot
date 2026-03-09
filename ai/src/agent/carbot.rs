use std::{collections::HashMap, sync::Arc};
use anyhow::{Error, Ok};
use async_recursion::async_recursion;
use async_trait::async_trait;
use common::{biz_err, task};
use futures::lock::Mutex;

use crate::agent::{agent::Agent, model::Requirement};

#[async_trait]
pub trait CarbotDelegate: Send + Sync {
    // 向模型发送消息
    async fn run(&self, carbot: Arc<Carbot>) -> Result<String, Error>;
}

pub struct Carbot {
    pub(crate) parant: Option<Arc<Carbot>>,
    pub(crate) is_run: Mutex<bool>,

    pub(crate) requirements: Mutex<HashMap<String, String>>,
    pub(crate) delegate: Box<dyn CarbotDelegate>,
    pub(crate) root: bool,
}

impl Carbot {
    pub fn new(parent: Arc<Carbot>, delegate: Box<dyn CarbotDelegate>) -> Self {
        let carbot = Carbot {
            parant: Some(parent),
            requirements: Mutex::new(HashMap::new()),
            delegate,
            is_run: Mutex::new(false),
            root: false,
        };

        carbot
    }

    pub fn root(delegate: Box<dyn CarbotDelegate>) -> Self {
        let carbot = Carbot {
            parant: None,
            requirements: Mutex::new(HashMap::new()),
            delegate,
            is_run: Mutex::new(false),
            root: true,
        };

        carbot
    }
}

#[async_trait]
impl Agent for Carbot {
    async fn start(self: Arc<Self>) -> Result<String, Error> {
        if !self.check_can_run().await {
            return Err(biz_err!("carbot already running"))
        }

        let result = self.delegate.run(self.clone()).await?;
        Ok(result)
    }

    async fn find_requirements(&self, requirements: Vec<Requirement>)-> Result<HashMap<String, String>, Error> {
        let mut miss_requirements = vec![];
        let mut result = HashMap::new();
        for requirement in requirements {
            if let Some(value) = self.load_requirement_value(&requirement.key).await {
                result.insert(requirement.key, value);
            } else {
                miss_requirements.push(requirement);
            }
        }

        if let Some(parent) = self.parant.clone() {
            let miss_result = parent.find_requirements(miss_requirements).await?;
            for (k, v) in miss_result {
                result.insert(k.clone(), v.clone());
                self.set_requirement_value(&k, v).await;
            }
        }

        Ok(result)
    }
}

impl Carbot {
    #[async_recursion]
    pub async fn set_requirement_value(&self, requirement_key: &str, value: String) {
        if !self.root {
            if let Some(parent) = self.parant.clone() {
                parent.set_requirement_value(requirement_key, value).await;
            }

            return;
        }

        let mut data = self.requirements.lock().await;
        data.insert(requirement_key.to_string(), value);
    }

    #[async_recursion]
    pub async fn load_requirement_value(&self, requirement_key: &str) -> Option<String> {
        if !self.root {
            if let Some(parent) = self.parant.clone() {
                return parent.load_requirement_value(requirement_key).await;
            }

            return None;
        }

        let data = self.requirements.lock().await;
        data.get(requirement_key).cloned()
    }

    async fn check_can_run(&self) -> bool {
        let mut is_run = self.is_run.lock().await;
        if is_run.clone() {
            return false;
        }

        *is_run = true;
        return true;
    }
}