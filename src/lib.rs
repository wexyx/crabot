// OpenClaw 组织管理系统 - 基于字节跳动架构
pub mod models;
pub mod services;
pub mod database;
pub mod middleware;
pub mod error;

pub use error::{Result, CrabotError};
