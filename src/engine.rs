use crate::config::{
    ExpectConfig, ExpectMultiConfig, ScenarioConfig, ScheduleKind, StepConfig, SuiteConfig,
    TargetKind,
};
use crate::core::{ClientFactory, KvClient, ProtocolKind, TargetSpec};
use crate::drivers::noop::NoopClientFactory;
use anyhow::Result;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

pub fn print_plan(suite: &SuiteConfig) {
    println!("Suite: {}", suite.name);
    for t in &suite.targets {
        println!(
            "- Target {} {} {}",
            t.name,
            protocol_label(&t.kind),
            t.address
        );
    }
    for sc in &suite.scenarios {
        println!(
            "- Scenario: {} ({:?}) x{}",
            sc.name, sc.schedule.kind, sc.schedule.concurrency
        );
        for st in &sc.steps {
            println!("  - Step: {:?}", step_label(st));
        }
    }
}

fn protocol_label(kind: &TargetKind) -> &'static str {
    match kind {
        TargetKind::Memcached => "memcached",
        TargetKind::Redis => "redis",
    }
}

fn step_label(step: &StepConfig) -> &'static str {
    match step {
        StepConfig::Set { .. } => "set",
        StepConfig::Get { .. } => "get",
        StepConfig::Mget { .. } => "mget",
        StepConfig::Delete { .. } => "delete",
        StepConfig::Sleep { .. } => "sleep",
    }
}

pub async fn execute_suite(suite: &SuiteConfig) -> Result<()> {
    // For now use a noop factory. Later: build real memcached/redis drivers.
    let factory = NoopClientFactory;

    let mut clients: HashMap<String, Box<dyn KvClient>> = HashMap::new();
    for target in &suite.targets {
        let protocol = match target.kind {
            TargetKind::Memcached => ProtocolKind::Memcached,
            TargetKind::Redis => ProtocolKind::Redis,
        };
        let client = factory
            .build(
                protocol,
                TargetSpec {
                    name: target.name.clone(),
                    address: target.address.clone(),
                },
            )
            .await?;
        clients.insert(target.name.clone(), client);
    }

    // Naively pick the first client (if any) for now
    let default_client = clients.values().next();

    for sc in &suite.scenarios {
        if let Some(client) = default_client {
            run_scenario(sc, client.as_ref()).await?;
        } else {
            println!("No targets/clients available; scenario {} skipped", sc.name);
        }
    }
    Ok(())
}

async fn run_scenario(sc: &ScenarioConfig, client: &dyn KvClient) -> Result<()> {
    // Read hooks to avoid dead-code warnings and to sketch behavior
    if let Some(hooks) = &sc.hooks {
        for h in &hooks.before {
            println!("Hook(before): exec='{}' timeout={:?}", h.exec, h.timeout);
        }
    }
    match sc.schedule.kind {
        ScheduleKind::Once => run_once(sc, client).await,
        ScheduleKind::Iterations => run_iterations(sc, client).await,
        ScheduleKind::Duration => run_for_duration(sc, client).await,
    }
}

async fn run_once(sc: &ScenarioConfig, client: &dyn KvClient) -> Result<()> {
    execute_steps(&sc.steps, client).await?;
    if let Some(hooks) = &sc.hooks {
        for h in &hooks.after {
            println!("Hook(after): exec='{}' timeout={:?}", h.exec, h.timeout);
        }
    }
    Ok(())
}

async fn run_iterations(sc: &ScenarioConfig, client: &dyn KvClient) -> Result<()> {
    let n = sc.schedule.iterations.unwrap_or(1);
    for _ in 0..n {
        execute_steps(&sc.steps, client).await?;
    }
    Ok(())
}

async fn run_for_duration(sc: &ScenarioConfig, client: &dyn KvClient) -> Result<()> {
    // Minimal placeholder: run steps once; proper duration loop to come
    let _ = sc.schedule.duration.as_deref();
    execute_steps(&sc.steps, client).await
}

async fn execute_steps(steps: &[StepConfig], client: &dyn KvClient) -> Result<()> {
    for step in steps {
        match step {
            StepConfig::Set { key, value, ttl } => {
                let _ttl = ttl.as_deref().and_then(parse_duration);
                client
                    .set(key, value, _ttl)
                    .await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            }
            StepConfig::Get { key, expect } => {
                let _ = client
                    .get(key)
                    .await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                if let Some(e) = expect {
                    print_expect(e);
                }
            }
            StepConfig::Mget { keys, expect } => {
                let _ = client
                    .mget(keys)
                    .await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                if let Some(e) = expect {
                    print_expect_multi(e);
                }
            }
            StepConfig::Delete { key } => {
                client
                    .delete(key)
                    .await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            }
            StepConfig::Sleep { duration } => {
                if let Some(dur) = parse_duration(duration) {
                    sleep(dur).await;
                }
            }
        }
    }
    Ok(())
}

fn print_expect(e: &ExpectConfig) {
    if let Some(v) = &e.value {
        println!("  expect value={}", v);
    }
    if let Some(m) = e.miss {
        println!("  expect miss={}", m);
    }
    if let Some(l) = e.latency_ms_lt {
        println!("  expect latency_ms_lt={}", l);
    }
}

fn print_expect_multi(e: &ExpectMultiConfig) {
    if let Some(vs) = &e.values {
        println!("  expect values={:?}", vs);
    }
    if let Some(misses) = &e.misses {
        println!("  expect misses={:?}", misses);
    }
}

fn parse_duration(s: &str) -> Option<Duration> {
    // Very naive parser: supports suffixes s and ms
    if let Some(stripped) = s.strip_suffix("ms") {
        let v: u64 = stripped.parse().ok()?;
        return Some(Duration::from_millis(v));
    }
    if let Some(stripped) = s.strip_suffix('s') {
        let v: u64 = stripped.parse().ok()?;
        return Some(Duration::from_secs(v));
    }
    None
}
