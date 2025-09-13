use crate::core::{ClientError, KvClient, ProtocolKind, TargetSpec};
use async_trait::async_trait;
use std::time::Duration;

pub struct NoopClientFactory;

#[async_trait]
impl crate::core::ClientFactory for NoopClientFactory {
    async fn build(
        &self,
        _protocol: ProtocolKind,
        target: TargetSpec,
    ) -> Result<Box<dyn KvClient>, ClientError> {
        // Reference fields to avoid dead-code warnings and to demonstrate target binding
        let _name = target.name;
        let _addr = target.address;
        Ok(Box::new(NoopClient {
            _bound_to: _name,
            _at: _addr,
        }))
    }
}

struct NoopClient {
    _bound_to: String,
    _at: String,
}

#[async_trait]
impl KvClient for NoopClient {
    async fn set(
        &self,
        _key: &str,
        _value: &str,
        _ttl: Option<Duration>,
    ) -> Result<(), ClientError> {
        Ok(())
    }

    async fn get(&self, _key: &str) -> Result<Option<String>, ClientError> {
        Ok(Some("noop".to_string()))
    }

    async fn mget(&self, keys: &[String]) -> Result<Vec<Option<String>>, ClientError> {
        Ok(keys.iter().map(|_| Some("noop".to_string())).collect())
    }

    async fn delete(&self, _key: &str) -> Result<(), ClientError> {
        Ok(())
    }
}
