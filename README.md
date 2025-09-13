# Memsanity

Memsanity is a tool to provide sanity/functional tests for simple to complex behaviours
for in-memory storages (Memcached, Redis).

## Design

Goals:

- Correctness-first sanity/functional checks for Memcached/Redis-like stores
- Small, deterministic test plans with light pressure (optionally configurable)
- All behavior defined in YAML suites; reproducible, versionable
- Async engine (Rust + Tokio)

Core concepts:

- Suite: A collection of scenarios against one or more targets
- Target: A thing to talk to (e.g., memcached TCP endpoint, redis)
- Client: Protocol driver bound to a target (memcached, redis)
- Scenario: Steps + assertions executed with a schedule (once, duration, iterations)
- Step: An operation (set/get/mget/delete, etc.) with inputs
- Assertion: Expected outcome (value equals, key miss, latency bound, ttl expiry)
- Hooks (optional): External commands you can run before/after a scenario to orchestrate env (e.g., pause a backend)

Minimal YAML shape: see `examples/concept/minimal.yaml`.

Example suites:

- Simple standalone Memcached → `examples/concept/memcached-core.yaml`
- Proxy/load balancer in front of Memcached → `examples/concept/proxy-memcached-smoke.yaml`

See `examples/concept/README.md` for the intents behind each example.

MVP roadmap (where to start):

1. CLI skeleton (Rust): `memsanity run <suite.yaml>`
2. YAML loader + schema (serde + serde_yaml)
3. Protocol driver (Memcached first): minimal text protocol client (set/get/delete/mget)
4. Execution engine: schedules (once/iterations/duration), concurrency (Tokio tasks), timeouts
5. Assertions + reporting: per-step result, summary, non-zero exit on failure
6. Hooks runner (optional): `exec` with timeout for external orchestration
7. Examples: see `examples/concept/` (including the two referenced above)

Notes:

- Not performance-first, but allow configurable concurrency to apply gentle pressure
- Deterministic defaults (fixed keys unless templated), optional key templating later
- Start with Memcached; Redis driver can follow the same abstractions
