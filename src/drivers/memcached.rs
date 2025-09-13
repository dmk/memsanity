use crate::core::{ClientError, ClientFactory, KvClient, ProtocolKind, TargetSpec};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::info;

pub struct MemcachedClientFactory;

#[async_trait]
impl ClientFactory for MemcachedClientFactory {
    async fn build(
        &self,
        protocol: ProtocolKind,
        target: TargetSpec,
    ) -> Result<Arc<dyn KvClient>, ClientError> {
        match protocol {
            ProtocolKind::Memcached => Ok(Arc::new(MemcachedClient { at: target.address })),
            _ => Err(anyhow::anyhow!("unsupported protocol for MemcachedClientFactory")),
        }
    }
}

struct MemcachedClient {
    at: String,
}

#[async_trait]
impl KvClient for MemcachedClient {
    async fn set(
        &self,
        op_id: u64,
        key: &str,
        value: &str,
        ttl: Option<Duration>,
    ) -> Result<(), ClientError> {
        let mut stream = TcpStream::connect(&self.at).await?;
        let exptime = ttl.map(|d| d.as_secs() as u32).unwrap_or(0);
        let cmd = format!(
            "set {} 0 {} {}\r\n{}\r\n",
            key,
            exptime,
            value.as_bytes().len(),
            value
        );
        stream.write_all(cmd.as_bytes()).await?;
        stream.flush().await?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        info!(op_id, target = "memcached", at = %self.at, key = %key, value = %value, exptime, response = %line.trim_end(), "SET");
        if line.starts_with("STORED") {
            Ok(())
        } else {
            Err(anyhow::anyhow!(format!("unexpected response: {}", line.trim())))
        }
    }

    async fn get(&self, op_id: u64, key: &str) -> Result<Option<String>, ClientError> {
        let mut stream = TcpStream::connect(&self.at).await?;
        let cmd = format!("get {}\r\n", key);
        stream.write_all(cmd.as_bytes()).await?;
        stream.flush().await?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        let mut out: Option<String> = None;
        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 { break; }
            if line.starts_with("END") { break; }
            if line.starts_with("VALUE ") {
                // VALUE <key> <flags> <bytes>\r\n
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let _flags: u32 = parts[2].parse().unwrap_or(0);
                    let num_bytes: usize = parts[3].parse().unwrap_or(0);
                    let mut buf = vec![0u8; num_bytes];
                    reader.read_exact(&mut buf).await?;
                    let _ = reader.read_exact(&mut [0u8; 2]).await; // read trailing \r\n
                    out = Some(String::from_utf8_lossy(&buf).to_string());
                }
            }
        }
        let hit = out.is_some();
        let value_str: &str = out.as_deref().unwrap_or("");
        info!(op_id, target = "memcached", at = %self.at, key = %key, hit, value = %value_str, "GET");
        Ok(out)
    }

    async fn mget(&self, op_id: u64, keys: &[String]) -> Result<Vec<Option<String>>, ClientError> {
        let mut stream = TcpStream::connect(&self.at).await?;
        let cmd = format!("get {}\r\n", keys.join(" "));
        stream.write_all(cmd.as_bytes()).await?;
        stream.flush().await?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        let mut values: HashMap<String, String> = HashMap::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 { break; }
            if line.starts_with("END") { break; }
            if line.starts_with("VALUE ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let k = parts[1].to_string();
                    let _flags: u32 = parts[2].parse().unwrap_or(0);
                    let num_bytes: usize = parts[3].parse().unwrap_or(0);
                    let mut buf = vec![0u8; num_bytes];
                    reader.read_exact(&mut buf).await?;
                    let _ = reader.read_exact(&mut [0u8; 2]).await; // \r\n
                    let v = String::from_utf8_lossy(&buf).to_string();
                    values.insert(k, v);
                }
            }
        }
        let result = keys
            .iter()
            .map(|k| values.get(k).cloned())
            .map(|opt| opt.map(|s| s.to_string()))
            .collect();
        info!(op_id, target = "memcached", at = %self.at, keys = ?keys, "MGET");
        Ok(result)
    }

    async fn delete(&self, op_id: u64, key: &str) -> Result<(), ClientError> {
        let mut stream = TcpStream::connect(&self.at).await?;
        let cmd = format!("delete {}\r\n", key);
        stream.write_all(cmd.as_bytes()).await?;
        stream.flush().await?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        info!(op_id, target = "memcached", at = %self.at, key = %key, response = %line.trim_end(), "DELETE");
        if line.starts_with("DELETED") || line.starts_with("NOT_FOUND") {
            Ok(())
        } else {
            Err(anyhow::anyhow!(format!("unexpected response: {}", line.trim())))
        }
    }
}
