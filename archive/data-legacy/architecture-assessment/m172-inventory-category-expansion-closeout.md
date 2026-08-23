# M172 Inventory Category Expansion Closeout

## Verdict

**M172 status: PASS.**

M172 expanded write-path inventory categories beyond M171 without broad reclassification or hidden risk.

## Categories added

| Category | Count | Scope |
|---|---:|---|
| `graph-readiness-evidence` | 14 | Exact path prefix `src/research_graph/infrastructure/graph/readiness/` |
| `source-asset-package` | 4 | Exact file `src/research_graph/infrastructure/papers/source_assets/registry.py` |
| `article-artifact-package` | 7 | Exact paths `src/research_graph/cli/commands/article_artifacts.py` and `src/research_graph/infrastructure/papers/artifacts/` |

## Final inventory counts

```text
total_records=340
unknown=0
graph-readiness-evidence=14
source-asset-package=4
article-artifact-package=7
caller-owned=28
run-scoped=14
append-log=3
shared-state=0
```

## Delta from M172 baseline

```text
graph-readiness-evidence +14
source-asset-package +4
article-artifact-package +7
caller-owned -10
run-scoped -11
append-log -4
```

No `shared-state` records were reclassified.

## Code and tests

Changed:

- `scripts/inventory_write_paths.py`
- `tests/test_inventory_write_paths.py`

Focused tests:

```text
uv run pytest tests/test_inventory_write_paths.py -q
5 passed
```

Tests cover:

- graph-readiness positive classification;
- source-asset positive classification;
- article-artifact positive classification;
- unapproved summary/artifact target fallback;
- unreviewed state/index/catalog fallback to `shared-state`.

## Verification

Integrated verification:

```text
focused inventory tests=5 passed
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
scope hygiene=expected M172 files only
```

## Decisions

- D094: Inventory category expansion uses exact path-family categories only; generic target-name reclassification is rejected.

## Residual risks

1. The inventory scanner remains static and conservative; it is not data-flow analysis.
2. Pre-edit GitNexus impact could not resolve scanner targets, so final safety relies on focused tests plus final `detect_changes`.
3. Mixed groups such as parser replay, source scans, graph probes, repair benchmarks, and generic CLI outputs remain broad until individually reviewed.

## Follow-ups

Possible future category expansion targets:

1. parser replay outputs;
2. source scan outputs;
3. graph probe outputs;
4. repair benchmark outputs;
5. command-specific CLI output categories.

Each follow-up should repeat the same rule: exact reviewed path family, positive test, fallback test, regenerated inventory, delta artifact.
