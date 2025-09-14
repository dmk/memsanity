use crate::config::{
    ExpectConfig, ExpectMultiConfig, ScenarioConfig, ScheduleKind, StepConfig, SuiteConfig,
    TargetKind,
};
use crate::core::{ClientFactory, KvClient, ProtocolKind, TargetSpec};
use crate::drivers::noop::NoopClientFactory;
use crate::drivers::memcached::MemcachedClientFactory;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

pub fn print_plan(suite: &SuiteConfig) {
    info!(suite = %suite.name, "suite_loaded");
    for t in &suite.targets {
        info!(target_name = %t.name, kind = %protocol_label(&t.kind), address = %t.address, "target");
    }
    for sc in &suite.scenarios {
        info!(scenario = %sc.name, schedule = ?sc.schedule.kind, concurrency = sc.schedule.concurrency, "scenario");
        for st in &sc.steps {
            info!(step = step_label(st), "step");
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

#[derive(Default)]
struct RunSummary {
    total_steps: AtomicU64,
    failed_steps: AtomicU64,
}

impl RunSummary {
    fn incr_total(&self) { self.total_steps.fetch_add(1, Ordering::Relaxed); }
    fn incr_failed(&self) { self.failed_steps.fetch_add(1, Ordering::Relaxed); }
    fn snapshot(&self) -> (u64, u64) {
        (
            self.total_steps.load(Ordering::Relaxed),
            self.failed_steps.load(Ordering::Relaxed),
        )
    }
}

static OP_ID: AtomicU64 = AtomicU64::new(1);
fn next_op_id() -> u64 { OP_ID.fetch_add(1, Ordering::Relaxed) }

pub async fn execute_suite(suite: &SuiteConfig) -> Result<()> {
    let mut clients: HashMap<String, Arc<dyn KvClient>> = HashMap::new();
    for target in &suite.targets {
        let protocol = match target.kind {
            TargetKind::Memcached => ProtocolKind::Memcached,
            TargetKind::Redis => ProtocolKind::Redis,
        };
        let client: Arc<dyn KvClient> = match protocol {
            ProtocolKind::Memcached => {
                MemcachedClientFactory
                    .build(
                        protocol,
                        TargetSpec { name: target.name.clone(), address: target.address.clone() },
                    )
                    .await?
            }
            _ => {
                NoopClientFactory
                    .build(
                        protocol,
                        TargetSpec { name: target.name.clone(), address: target.address.clone() },
                    )
                    .await?
            }
        };
        clients.insert(target.name.clone(), client);
    }

    // Naively pick the first client (if any) for now
    let default_client = clients.values().next().cloned();

    let summary = Arc::new(RunSummary::default());

    for sc in &suite.scenarios {
        if let Some(client) = default_client.clone() {
            run_scenario(sc, client.clone(), summary.clone()).await?;
        } else {
            info!(scenario = %sc.name, "scenario_skipped_no_targets");
        }
    }

    let (total, failed) = summary.snapshot();
    info!(total, failed, status = %if failed == 0 {"ok"} else {"fail"}, "summary");
    Ok(())
}

async fn run_scenario(sc: &ScenarioConfig, client: Arc<dyn KvClient>, summary: Arc<RunSummary>) -> Result<()> {
    // Read hooks to avoid dead-code warnings and to sketch behavior
    if let Some(hooks) = &sc.hooks {
        for h in &hooks.before {
            info!(hook = %h.exec, timeout = ?h.timeout, when = "before", "hook");
        }
    }
    match sc.schedule.kind {
        ScheduleKind::Once => run_once(sc, client.clone(), summary.clone()).await,
        ScheduleKind::Iterations => run_iterations(sc, client.clone(), summary.clone()).await,
        ScheduleKind::Duration => run_for_duration(sc, client.clone(), summary.clone()).await,
    }
}

async fn run_once(sc: &ScenarioConfig, client: Arc<dyn KvClient>, summary: Arc<RunSummary>) -> Result<()> {
    execute_steps(sc.name.clone(), 0, sc.steps.clone(), client.clone(), summary.clone()).await?;
    if let Some(hooks) = &sc.hooks {
        for h in &hooks.after {
            info!(hook = %h.exec, timeout = ?h.timeout, when = "after", "hook");
        }
    }
    Ok(())
}

async fn run_iterations(sc: &ScenarioConfig, client: Arc<dyn KvClient>, summary: Arc<RunSummary>) -> Result<()> {
    let n = sc.schedule.iterations.unwrap_or(1);
    let conc = sc.schedule.concurrency.max(1) as usize;
    let mut handles = Vec::with_capacity(conc);
    for worker_id in 0..conc {
        let steps = sc.steps.clone();
        let client = client.clone();
        let scenario_name = sc.name.clone();
        let summary = summary.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..n {
                let _ = execute_steps(scenario_name.clone(), worker_id, steps.clone(), client.clone(), summary.clone()).await;
            }
        }));
    }
    for h in handles { let _ = h.await; }
    Ok(())
}

async fn run_for_duration(sc: &ScenarioConfig, client: Arc<dyn KvClient>, summary: Arc<RunSummary>) -> Result<()> {
    let conc = sc.schedule.concurrency.max(1) as usize;
    let duration = sc.schedule.duration.as_deref().and_then(parse_duration).unwrap_or(Duration::from_secs(1));
    let mut handles = Vec::with_capacity(conc);
    for worker_id in 0..conc {
        let steps = sc.steps.clone();
        let client = client.clone();
        let scenario_name = sc.name.clone();
        let summary = summary.clone();
        handles.push(tokio::spawn(async move {
            let start = tokio::time::Instant::now();
            while start.elapsed() < duration {
                let _ = execute_steps(scenario_name.clone(), worker_id, steps.clone(), client.clone(), summary.clone()).await;
            }
        }));
    }
    for h in handles { let _ = h.await; }
    Ok(())
}

async fn execute_steps(scenario: String, worker_id: usize, steps: Vec<StepConfig>, client: Arc<dyn KvClient>, summary: Arc<RunSummary>) -> Result<()> {
    for (idx, step) in steps.iter().enumerate() {
        let op_id = next_op_id();
        summary.incr_total();
        match step {
            StepConfig::Set { key, value, ttl } => {
                let _ttl = ttl.as_deref().and_then(parse_duration);
                info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "set", key = %key, value = %value);
                match client
                    .set(op_id, key, value, _ttl)
                    .await
                {
                    Ok(_) => {
                        info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "set", result = "ok");
                    }
                    Err(e) => {
                        summary.incr_failed();
                        info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "set", result = "fail", error = %e);
                    }
                }
            }
            StepConfig::Get { key, expect } => {
                info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "get", key = %key);
                let start = tokio::time::Instant::now();
                match client
                    .get(op_id, key)
                    .await
                {
                    Ok(got) => {
                        let elapsed_ms = start.elapsed().as_millis() as u64;
                        let hit = got.is_some();
                        let value = got.as_deref().unwrap_or("");
                        let mut ok = true;
                        if let Some(e) = expect {
                            print_expect(op_id, e);
                            if let Some(expected_value) = &e.value {
                                if hit && value == expected_value {
                                    // ok
                                } else {
                                    ok = false;
                                    summary.incr_failed();
                                    info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "get", result = "fail", reason = "value_mismatch", expected = %expected_value, got = %value, hit);
                                }
                            }
                            if let Some(expect_miss) = e.miss {
                                if expect_miss && hit {
                                    ok = false;
                                    summary.incr_failed();
                                    info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "get", result = "fail", reason = "expected_miss_got_hit", got = %value);
                                }
                                if !expect_miss && !hit {
                                    ok = false;
                                    summary.incr_failed();
                                    info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "get", result = "fail", reason = "expected_hit_got_miss");
                                }
                            }
                            if let Some(limit_ms) = e.latency_ms_lt {
                                if elapsed_ms >= limit_ms {
                                    ok = false;
                                    summary.incr_failed();
                                    info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "get", result = "fail", reason = "latency_too_high", elapsed_ms, limit_ms);
                                }
                            }
                        }
                        if ok {
                            info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "get", result = "ok", hit, value = %value, elapsed_ms);
                        }
                    }
                    Err(e) => {
                        summary.incr_failed();
                        info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "get", result = "fail", error = %e);
                    }
                }
            }
            StepConfig::Mget { keys, expect } => {
                info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "mget", keys = ?keys);
                let start = tokio::time::Instant::now();
                match client
                    .mget(op_id, keys)
                    .await
                {
                    Ok(got) => {
                        let _elapsed_ms = start.elapsed().as_millis() as u64;
                        let mut ok = true;
                        if let Some(e) = expect {
                            print_expect_multi(op_id, e);
                            if let Some(expected_values) = &e.values {
                                for (k, expected_v) in expected_values {
                                    if let Some(pos) = keys.iter().position(|kk| kk == k) {
                                        match &got.get(pos) {
                                            Some(Some(actual_v)) if actual_v == expected_v => {}
                                            _ => {
                                                ok = false;
                                                summary.incr_failed();
                                                info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "mget", result = "fail", reason = "value_mismatch", key = %k, expected = %expected_v);
                                            }
                                        }
                                    } else {
                                        ok = false;
                                        summary.incr_failed();
                                        info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "mget", result = "fail", reason = "expected_key_not_requested", key = %k);
                                    }
                                }
                            }
                            if let Some(misses) = &e.misses {
                                for miss_key in misses {
                                    if let Some(pos) = keys.iter().position(|kk| kk == miss_key) {
                                        if got.get(pos).and_then(|v| v.clone()).is_some() {
                                            ok = false;
                                            summary.incr_failed();
                                            info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "mget", result = "fail", reason = "expected_miss_got_value", key = %miss_key);
                                        }
                                    } else {
                                        ok = false;
                                        summary.incr_failed();
                                        info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "mget", result = "fail", reason = "miss_key_not_in_requested", key = %miss_key);
                                    }
                                }
                            }
                        }
                        if ok {
                            info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "mget", result = "ok");
                        }
                    }
                    Err(e) => {
                        summary.incr_failed();
                        info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "mget", result = "fail", error = %e);
                    }
                }
            }
            StepConfig::Delete { key } => {
                info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "delete", key = %key);
                match client
                    .delete(op_id, key)
                    .await
                {
                    Ok(_) => {
                        info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "delete", result = "ok");
                    }
                    Err(e) => {
                        summary.incr_failed();
                        info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "delete", result = "fail", error = %e);
                    }
                }
            }
            StepConfig::Sleep { duration } => {
                info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "sleep", duration = %duration);
                if let Some(dur) = parse_duration(duration) {
                    sleep(dur).await;
                }
                info!(op_id, scenario = %scenario, worker_id, step_idx = idx, op = "sleep", result = "ok");
            }
        }
    }
    Ok(())
}

fn print_expect(op_id: u64, e: &ExpectConfig) {
    if let Some(v) = &e.value { info!(op_id, expect_value = %v); }
    if let Some(m) = e.miss { info!(op_id, expect_miss = m); }
    if let Some(l) = e.latency_ms_lt { info!(op_id, expect_latency_ms_lt = l); }
}

fn print_expect_multi(op_id: u64, e: &ExpectMultiConfig) {
    if let Some(vs) = &e.values { info!(op_id, expect_values = ?vs); }
    if let Some(misses) = &e.misses { info!(op_id, expect_misses = ?misses); }
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
