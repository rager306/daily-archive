# M183 Test Plan

## Focused tests

Run:

```bash
uv run pytest tests/test_inventory_write_paths.py -q
```

Required assertions:

- Exact selected M055 benchmark paths map to `benchmark-m055-output`.
- Exact selected M055deep benchmark paths map to `benchmark-m055deep-output`.
- Exact `scripts/m066_graphdb_full_benchmark.py` maps to `m066-graphdb-benchmark-output`.
- Exact `scripts/audit_test_architecture.py` maps to `test-architecture-audit-output`.
- Future unlisted paths remain `script-only`.
- `scripts/benchmark_m055_corpus_manifest.py` remains `script-only` unless cache proof changes S06.

## Delta checks

Expected movement:

```text
script-only: 103 -> 89
benchmark-m055-output: 0 -> 5
benchmark-m055deep-output: 0 -> 3
m066-graphdb-benchmark-output: 0 -> 3
test-architecture-audit-output: 0 -> 3
unknown=0
shared-state=0
total_delta=+0
```

Use generated deltas only from `scripts/inventory_write_paths.py`.

## Docs and ADR checks

- Read active docs/ADR files before editing.
- Do not rewrite historical GSD projection/history.
- Add/update active guidance for exact scanner policy, canonical baseline update protocol, script boundary contract, and cache proof gate.

## Cache checks

- Movement requires exact ownership/lifecycle/invalidation/concurrency proof.
- No broad cache/index/manifest/target-name rule.

## Final quality

Run ruff, pyrefly, pre-commit, GitNexus detect_changes, and filtered status hygiene before completion.
