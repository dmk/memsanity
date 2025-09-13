use async_trait::async_trait;
use std::time::Duration;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TargetSpec {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ProtocolKind {
    Memcached,
    Redis,
}

pub type ClientError = anyhow::Error;

#[async_trait]
pub trait KvClient: Send + Sync {
    async fn set(&self, op_id: u64, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), ClientError>;
    async fn get(&self, op_id: u64, key: &str) -> Result<Option<String>, ClientError>;
    async fn mget(&self, op_id: u64, keys: &[String]) -> Result<Vec<Option<String>>, ClientError>;
    async fn delete(&self, op_id: u64, key: &str) -> Result<(), ClientError>;
}

#[async_trait]
pub trait ClientFactory: Send + Sync {
    async fn build(
        &self,
        protocol: ProtocolKind,
        target: TargetSpec,
    ) -> Result<Arc<dyn KvClient>, ClientError>;
}
