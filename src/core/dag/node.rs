use anyhow::Result;

pub trait ExecutableTask {
    type Context;
    fn id(&self) -> &str;
    fn execute(&self, ctx: &mut Self::Context) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct Node<T: ExecutableTask> {
    pub task: T,
    pub dependencies: Vec<String>,
}

impl<T: ExecutableTask> Node<T> {
    pub fn new(task: T, dependencies: Vec<String>) -> Self {
        Self { task, dependencies }
    }
}
