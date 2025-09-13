use crate::core::{ClientError, KvClient, ProtocolKind, TargetSpec};
use async_trait::async_trait;
use std::time::Duration;
use std::sync::Arc;
use tracing::info;

pub struct NoopClientFactory;

#[async_trait]
impl crate::core::ClientFactory for NoopClientFactory {
    async fn build(
        &self,
        _protocol: ProtocolKind,
        target: TargetSpec,
    ) -> Result<Arc<dyn KvClient>, ClientError> {
        // Reference fields to avoid dead-code warnings and to demonstrate target binding
        let _name = target.name;
        let _addr = target.address;
        Ok(Arc::new(NoopClient {
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
        op_id: u64,
        _key: &str,
        _value: &str,
        _ttl: Option<Duration>,
    ) -> Result<(), ClientError> {
        info!(op_id, target = "noop", bound_to = %self._bound_to, at = %self._at, key = %_key, value = %_value, ?_ttl, "SET");
        Ok(())
    }

    async fn get(&self, op_id: u64, _key: &str) -> Result<Option<String>, ClientError> {
        info!(op_id, target = "noop", bound_to = %self._bound_to, at = %self._at, key = %_key, "GET");
        Ok(Some("noop".to_string()))
    }

    async fn mget(&self, op_id: u64, keys: &[String]) -> Result<Vec<Option<String>>, ClientError> {
        info!(op_id, target = "noop", bound_to = %self._bound_to, at = %self._at, keys = ?keys, "MGET");
        Ok(keys.iter().map(|_| Some("noop".to_string())).collect())
    }

    async fn delete(&self, op_id: u64, _key: &str) -> Result<(), ClientError> {
        info!(op_id, target = "noop", bound_to = %self._bound_to, at = %self._at, key = %_key, "DELETE");
        Ok(())
    }
}
