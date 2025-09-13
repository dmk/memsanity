use async_trait::async_trait;
use std::time::Duration;

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
    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), ClientError>;
    async fn get(&self, key: &str) -> Result<Option<String>, ClientError>;
    async fn mget(&self, keys: &[String]) -> Result<Vec<Option<String>>, ClientError>;
    async fn delete(&self, key: &str) -> Result<(), ClientError>;
}

#[async_trait]
pub trait ClientFactory: Send + Sync {
    async fn build(
        &self,
        protocol: ProtocolKind,
        target: TargetSpec,
    ) -> Result<Box<dyn KvClient>, ClientError>;
}
