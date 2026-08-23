# M174 Repair Benchmark Category Closeout

## Verdict

**M174 status: PASS.**

M174 added a precise repair benchmark output category without generic repair matching and without moving the existing `caller-owned-index` exception.

## Category added

| Category | Count | Scope |
|---|---:|---|
| `repair-benchmark-output` | 5 | Exact files `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` and `src/research_graph/infrastructure/repair/chunking_benchmark.py`, excluding `index_path` |

## Preserved exception

```text
caller-owned-index=1
src/research_graph/infrastructure/repair/chunk_baseline_measurement.py + index_path
```

## Final inventory counts

```text
total_records=340
unknown=0
repair-benchmark-output=5
caller-owned-index=1
caller-owned=20
run-scoped=11
append-log=1
shared-state=0
```

## Delta from M174 baseline

```text
repair-benchmark-output +5
append-log -2
run-scoped -2
caller-owned -1
caller-owned-index 0
```

No `shared-state` records were reclassified.

## Code and tests

Changed:

- `scripts/inventory_write_paths.py`
- `tests/test_inventory_write_paths.py`

Focused tests:

```text
uv run pytest tests/test_inventory_write_paths.py -q
9 passed
```

Tests cover:

- repair benchmark positive classification;
- caller-owned-index preservation for `index_path`;
- unapproved repair-like diagnostics fallback;
- prior category and fallback tests from M172/M173.

## Verification

Integrated verification:

```text
focused inventory tests=9 passed
test architecture guard=dynamic=0, legacy=0, violations=0
onion guard=violation_count=0, allowed_violation_count=0
final inventory assertions=PASS
```

Quality stack:

```text
scoped ruff=PASS
pyrefly=0 errors
pre-commit=PASS
GitNexus detect_changes=LOW risk, affected_processes=0
scope hygiene=expected M174 files only
```

## Decisions

- D096: M174 adds `repair-benchmark-output` only for exact repair benchmark source paths and preserves `caller-owned-index` for `index_path`.

## Residual risks

1. The inventory scanner remains static and conservative, not data-flow analysis.
2. Pre-edit GitNexus impact could not resolve scanner targets, so final safety relies on focused tests plus final `detect_changes`.
3. Remaining broad groups still need separate review before future category movement.

## Follow-ups

Possible next category targets:

1. command-specific CLI outputs;
2. remaining mixed broad outputs after individual path review;
3. optional CI/report rendering for inventory deltas.

Each follow-up should repeat the rule: exact reviewed source scope, positive test, fallback test, regenerated inventory, delta artifact.
