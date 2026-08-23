# M180 Three Direction Feasibility

## Verdict

**Combined scope is feasible.**

M180 can include all three requested directions in one bounded milestone because the fresh baseline is stable, canonical baseline artifacts now exist, and cache lifecycle review can complete as exact movement or no-move without weakening scanner rules.

## Baseline

```text
total_records=341
script-only=142
unknown=0
shared-state=0
dynamic=0
legacy=0
```

## Direction fit

| Direction | Feasible in M180 | Boundary |
|---|---:|---|
| Next exact verify-family script-only wave | Yes | Exact source-path rules only. |
| Canonical baseline CI soak and cleanup | Yes | Prefer committed canonical baseline; current outputs stay temporary. |
| Cache lifecycle review | Yes | Movement only with exact lifecycle and concurrency proof; otherwise no-move review is valid completion. |

## Execution rule

S02 and S03 select and freeze the exact verify wave before scanner edits. S06 through S08 harden canonical-baseline CI behavior and refresh canonical artifacts. S09 performs cache lifecycle review without broad target-name or cache-prefix classification.

## Not doing

- No broad `verify_m031*` or `verify_m033*` prefix rule.
- No generic target-name rules such as `cache_path`, `output_path`, `summary_path`, `markdown_path`, or `index_path`.
- No CI current inventory files written to tracked paths.
