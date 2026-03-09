use std::sync::Arc;
use anyhow::Error;
use rbatis::RBatis;
use rbdc_mysql::driver::MysqlDriver;
use tokio::sync::OnceCell;

use crate::biz_err;

static DATABASE: OnceCell<Arc<Database>> = OnceCell::const_new();

/// 数据库中间件，负责数据库连接和初始化
pub struct Database {
    rb: Arc<RBatis>,
}

impl Database {
    /// 初始化数据库连接池
    pub async fn new(database_url: &str) -> Result<Self, Error> {
        let rb = rbatis::RBatis::new();
        rb.link(MysqlDriver{}, database_url).await.map_err(|e| biz_err!(e.to_string()))?;
        Ok(Self { rb: Arc::new(rb) })
    }

    /// 获取Rbatis实例
    pub fn rb(&self) -> &RBatis {
        &self.rb
    }
}

impl Database {
    pub async fn init(database_url: &str) -> Result<(), Error> {
        let database = Database::new(database_url).await?;
        DATABASE.set(Arc::new(database)).map_err(|e|biz_err!(e.to_string()))
    }

    pub async fn get() -> Option<Arc<Database>> {
        DATABASE.get().map(|d|d.clone())
    }
}