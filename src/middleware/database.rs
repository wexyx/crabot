use rbatis::rbatis::Rbatis;
use std::sync::Arc;
use crate::{Result, CrabotError};
use crate::database::schema;

/// 数据库中间件，负责数据库连接和初始化
pub struct DatabaseMiddleware {
    rb: Arc<Rbatis>,
}

impl DatabaseMiddleware {
    /// 初始化数据库连接池
    pub async fn new(database_url: &str) -> Result<Self> {
        let rb = Rbatis::new();
        rb.link(database_url).await.map_err(|e| CrabotError::DatabaseError(e.to_string()))?;

        Ok(Self { rb: Arc::new(rb) })
    }

    /// 初始化数据库表结构
    pub async fn init_schema(&self) -> Result<()> {
        let sql = schema::get_init_sql();
        // 分割SQL语句并执行
        for statement in sql.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() && !trimmed.starts_with("--") {
                self.rb.exec("", trimmed).await.map_err(|e| CrabotError::DatabaseError(format!("Failed to execute schema: {}", e)))?;
            }
        }
        Ok(())
    }

    /// 获取Rbatis实例
    pub fn rb(&self) -> &Rbatis {
        &self.rb
    }
}