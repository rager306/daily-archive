# M173 Inventory Category Expansion Batch Two Closeout

## Verdict

**M173 status: PASS.**

M173 expanded the next inventory category batch for parser replay, source scan, and graph probe outputs without generic target-name matching or shared-state reclassification.

## Categories added

| Category | Count | Scope |
|---|---:|---|
| `parser-replay-output` | 3 | Exact file `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` |
| `source-scan-output` | 3 | Exact files `src/research_graph/infrastructure/corpus/sources/thirty_paper_deviation_scan.py` and `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py` |
| `graph-probe-output` | 2 | Exact file `src/research_graph/infrastructure/graph/r024_networkx_probe.py` |

## Final inventory counts

```text
total_records=340
unknown=0
parser-replay-output=3
source-scan-output=3
graph-probe-output=2
caller-owned=21
run-scoped=13
append-log=3
shared-state=0
```

## Delta from M173 baseline

```text
parser-replay-output +3
source-scan-output +3
graph-probe-output +2
caller-owned -7
run-scoped -1
```

No `shared-state` records were reclassified.

## Code and tests

Changed:

- `scripts/inventory_write_paths.py`
- `tests/test_inventory_write_paths.py`

Focused tests:

```text
uv run pytest tests/test_inventory_write_paths.py -q
8 passed
```

Tests cover:

- parser replay positive classification;
- source scan positive classification;
- graph probe positive classification;
- unapproved summary/artifact/cache/destination/graph-summary fallback;
- unreviewed state/index/catalog fallback to `shared-state`.

## Verification

Integrated verification:

```text
focused inventory tests=8 passed
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
scope hygiene=expected M173 files only
```

## Decisions

- D095: M173 expands inventory categories using exact path-family categories only; generic target-name or broad module reclassification is rejected.

## Residual risks

1. The inventory scanner remains static and conservative, not data-flow analysis.
2. Pre-edit GitNexus impact could not resolve scanner targets, so final safety relies on focused tests plus final `detect_changes`.
3. Remaining broad groups still need separate review before any future category movement.

## Follow-ups

Possible next category targets:

1. repair benchmark outputs;
2. command-specific CLI outputs;
3. remaining mixed broad outputs after individual path review.

Each follow-up should repeat the M172/M173 rule: exact reviewed source scope, positive test, fallback test, regenerated inventory, delta artifact.
