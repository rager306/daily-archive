# M175 Baseline

## Verdict

**Baseline status: PASS.** No scanner edits have been made for M175.

## Baseline inventory

```text
total_records=340
unknown=0
by_root.scripts=264
by_root.src=76
```

Generated artifacts:

- `data/architecture-assessment/m175-write-path-inventory-baseline.json`
- `data/architecture-assessment/m175-write-path-inventory-baseline.md`

## Category counts

| Category | Count |
|---|---:|
| append-log | 1 |
| article-artifact-package | 7 |
| caller-owned | 20 |
| caller-owned-index | 1 |
| database | 1 |
| graph-probe-output | 2 |
| graph-readiness-evidence | 14 |
| legacy-evidence-regeneration | 2 |
| parser-replay-output | 3 |
| repair-benchmark-output | 5 |
| run-owned-state | 1 |
| run-scoped | 11 |
| script-only | 264 |
| source-asset-package | 4 |
| source-scan-output | 3 |
| temporary | 1 |

## Direction 1 candidate bucket: command-specific CLI outputs

Initial candidate family is `src/research_graph/cli/__init__.py`:

```text
caller-owned=2
run-scoped=3
temporary=1
```

The already-reviewed article artifact command stays out of scope:

```text
src/research_graph/cli/commands/article_artifacts.py -> article-artifact-package=7
```

## Direction 2 candidate bucket: remaining mixed broad outputs

Broad categories still visible after M174:

```text
caller-owned=20
run-scoped=11
append-log=1
temporary=1
script-only=264
```

These are not safe to move as a single group. S03 will review exact file families and may explicitly choose no-code decisions.

## Direction 3 candidate bucket: inventory delta reporting

M172-M174 used hand-written Python snippets to compare baseline and final inventory counts. M175 can replace that with minimal reproducible report rendering, provided it stays small and does not introduce a framework.

## Evidence

- Scanner run: `gsd_exec[50edc8cb-56d1-424e-a747-482e029589bd]`
- Candidate scan: `gsd_exec[93366e64-1e77-4a6c-8ade-ae17b92d3348]`
