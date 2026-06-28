# M192 Graph Readiness Command Map

## Ordering rule

**Review post-check must run before any import-boundary rehearsal or manifest synthesis.**

If review post-check cannot run or cannot prove completed-review artifacts, M192 must preserve fail-closed labels and stop short of import eligibility promotion.

## Commands

### 1. Review post-check probe

Historical required convention:

```bash
uv run python -m arxiv_archive.graph_readiness_review \
  --review-dir <review-dir> \
  --events <events.jsonl> \
  --validate-only \
  --require-completed-review
```

Current S01 help probe result:

- `gsd_exec[34f72ee6-b71a-4e6d-a55a-2c9250963f18]`
- Result: module unavailable in current package layout: `ModuleNotFoundError: No module named 'arxiv_archive'`.

S03 must record this as fail-closed unless a canonical current-layout equivalent and local completed-review inputs are found before execution.

### 2. Import-boundary rehearsal help and candidate command

Help probe:

- `gsd_exec[bd33e123-68e1-4d4e-9f27-056031372c14]`

Available CLI arguments:

```bash
uv run python scripts/replay_m031_import_boundary_rehearsal.py \
  --closeout-summary <closeout-summary.json> \
  --summary <summary.json> \
  --structure-aware-package <structure-aware-package.json> \
  --graph-readiness-package <graph-readiness-package.json> \
  --independent-review-events <events.jsonl> \
  --output-dir <output-dir>
```

S04 may run this only after S03 records review-post-check state. If S03 is fail-closed due unavailable module or absent completed-review inputs, S04 may run only tests and safe output inspection, not eligibility promotion.

### 3. Targeted tests

```bash
uv run pytest tests/test_graph_readiness_review.py tests/test_graph_readiness_contract.py -q
uv run pytest tests/test_m031_import_boundary_rehearsal.py tests/test_import_boundary_rehearsal.py -q
uv run pytest tests/test_graph_readiness_manifest.py tests/test_graph_readiness_extraction_gate.py tests/test_graph_readiness_persistence.py tests/test_graph_readiness_retrieval_validation.py tests/test_graph_readiness_export.py -q
```

### 4. Output inspection

Inspect any generated M192 outputs for unsafe flags:

- `import_eligible=true`
- `promoted_to_fact_count > 0`
- `production_import_attempted=true`
- `ladybugdb_written=true`
- `graph_ready=true`
- `optimizer_enabled=true`

Expected M192 default is fail-closed:

- `import_eligible=false`
- `production_import_attempted=false`
- `ladybugdb_written=false`
- `graph_ready=false`
- `optimizer_enabled=false`

## Stop conditions

Stop before import eligibility promotion if any of the following hold:

- graph-readiness review post-check module is unavailable;
- review directory or event JSONL is missing;
- completed-review verdict event is missing;
- `output_contract_completed=true` is missing;
- any reviewer placeholder remains;
- metadata-only M031 evidence is the only available evidence;
- generated output lacks explicit false safety flags.
