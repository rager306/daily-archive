# M195 Active Boundary Inventory

## Verdict

**PASS: active implementation planning scope is separated from historical archive/runtime artifacts.**

## Counts

- Matching active files: 693
- Included roots: `src`, `scripts`, `tests`, `doc`
- Excluded parts: `.git`, `.gsd`, `.mypy_cache`, `.pytest_cache`, `.ruff_cache`, `__pycache__`, `archive`, `artifacts`, `mutants`

## By layer

- `application`: 18
- `docs`: 90
- `domain`: 9
- `infrastructure`: 51
- `other`: 5
- `scripts`: 212
- `tests`: 295
- `workflows`: 13

## By category

- `continuity_audit`: 317 files
- `failure_handling`: 484 files
- `graph_projection`: 283 files
- `pipeline_core`: 160 files
- `queue_dispatch`: 367 files
- `script_wrappers`: 260 files

## Active anchor files

- `src/research_graph/domain/ports.py`
- `src/research_graph/domain/universal_kb/contracts.py`
- `src/research_graph/application/orchestrator.py`
- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/workflows/universal_kb/rehearsal.py`
- `src/research_graph/workflows/universal_kb/smoke_runner.py`
- `src/research_graph/workflows/universal_kb/smoke_audit.py`
- `src/research_graph/infrastructure/graph/ladybug_adapter.py`
- `tests/test_universal_kb_queue.py`
- `tests/test_networkx_graph_probe_adapter.py`
- `tests/test_ladybug_adapter_port.py`

## Historical archive policy

archive, artifacts, mutants, and .gsd are excluded from active implementation inventory unless a slice explicitly documents historical context.

## Production graph policy

No production graph write or import eligibility promotion in M195.
