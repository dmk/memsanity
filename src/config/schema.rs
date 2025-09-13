use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct SuiteConfig {
    #[serde(rename = "suite")]
    pub name: String,
    pub targets: Vec<TargetConfig>,
    pub scenarios: Vec<ScenarioConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TargetConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: TargetKind,
    pub address: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    Memcached,
    Redis,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScenarioConfig {
    pub name: String,
    pub schedule: ScheduleConfig,
    #[serde(default)]
    pub hooks: Option<HooksConfig>,
    pub steps: Vec<StepConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HooksConfig {
    #[serde(default)]
    pub before: Vec<ExecHook>,
    #[serde(default)]
    pub after: Vec<ExecHook>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExecHook {
    pub exec: String,
    #[serde(default)]
    pub timeout: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScheduleConfig {
    pub kind: ScheduleKind,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub iterations: Option<u64>,
    #[serde(default = "default_concurrency")]
    pub concurrency: u16,
}

fn default_concurrency() -> u16 {
    1
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleKind {
    Once,
    Duration,
    Iterations,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum StepConfig {
    Set {
        key: String,
        value: String,
        #[serde(default)]
        ttl: Option<String>,
    },
    Get {
        key: String,
        #[serde(default)]
        expect: Option<ExpectConfig>,
    },
    Mget {
        keys: Vec<String>,
        #[serde(default)]
        expect: Option<ExpectMultiConfig>,
    },
    Delete {
        key: String,
    },
    Sleep {
        duration: String,
    },
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExpectConfig {
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub miss: Option<bool>,
    #[serde(default)]
    pub latency_ms_lt: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExpectMultiConfig {
    #[serde(default)]
    pub values: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub misses: Option<Vec<String>>,
}
