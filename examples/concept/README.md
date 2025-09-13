# Concept examples

This directory contains conceptual YAML suites that illustrate how to describe tests.

- proxy-memcached-smoke.yaml
  - Intent: Verify reads/writes flow correctly through a proxy/load balancer, and that brief backend disruption doesn’t break correctness.

- memcached-core.yaml
  - Intent: Validate core semantics: set/get/delete, TTL expiry, and multi-get behavior.

- minimal.yaml
  - Intent: A minimal shape to showcase the basic schema elements (targets, scenarios, schedule, steps, assertions).
