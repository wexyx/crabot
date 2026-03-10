use std::collections::HashMap;
use anyhow::Result;

pub trait Skill {
    fn name(&self) -> &str;
    fn execute(&self) -> Result<String>;
}