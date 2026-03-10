use std::collections::{HashMap, VecDeque};
use crate::core::dag::node::{Node, ExecutableTask};
use anyhow::Result;

#[derive(Debug)]
pub struct Dag<T: ExecutableTask> {
    nodes: HashMap<String, Node<T>>,
}

impl<T: ExecutableTask> Dag<T> {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    pub fn add_task(&mut self, task: T, deps: Vec<&str>) {
        let id = task.id().to_string();
        let dependencies = deps.iter().map(|s| s.to_string()).collect();
        let node = Node::new(task, dependencies);
        self.nodes.insert(id, node);
    }

    pub fn compute_execution_order(&self) -> Result<Vec<Vec<String>>> {
        let mut indegree = HashMap::new();
        let mut graph = HashMap::new();

        // Initialize indegree and graph
        for (id, _node) in &self.nodes {
            indegree.insert(id.clone(), 0);
            graph.insert(id.clone(), Vec::new());
        }

        // Build graph and indegree
        for (id, node) in &self.nodes {
            for dep in &node.dependencies {
                if let Some(neighbors) = graph.get_mut(dep) {
                    neighbors.push(id.clone());
                }
                if let Some(deg) = indegree.get_mut(id) {
                    *deg += 1;
                }
            }
        }

        // Queue for nodes with indegree 0
        let mut queue = VecDeque::new();
        for (id, &deg) in &indegree {
            if deg == 0 {
                queue.push_back(id.clone());
            }
        }

        let mut result = Vec::new();

        while !queue.is_empty() {
            let mut level = Vec::new();
            let level_size = queue.len();

            for _ in 0..level_size {
                if let Some(node) = queue.pop_front() {
                    level.push(node.clone());

                    if let Some(neighbors) = graph.get(&node) {
                        for neighbor in neighbors {
                            if let Some(deg) = indegree.get_mut(neighbor) {
                                *deg -= 1;
                                if *deg == 0 {
                                    queue.push_back(neighbor.clone());
                                }
                            }
                        }
                    }
                }
            }

            if !level.is_empty() {
                result.push(level);
            }
        }

        // Check for cycles (if not all nodes are processed)
        if result.iter().map(|v| v.len()).sum::<usize>() != self.nodes.len() {
            return Err(anyhow::anyhow!("Cycle detected in DAG"));
        }

        Ok(result)
    }

    pub fn execute(&self, ctx: &mut T::Context) -> Result<()> {
        let order = self.compute_execution_order()?;
        for level in order {
            for task_id in level {
                if let Some(node) = self.nodes.get(&task_id) {
                    node.task.execute(ctx)?;
                }
            }
        }
        Ok(())
    }
}