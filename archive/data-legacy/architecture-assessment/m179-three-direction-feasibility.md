# M179 Three Direction Feasibility

## Verdict

**Combined scope is feasible.**

M179 can include all three requested directions in one bounded milestone because the fresh baseline is stable and the non-scanner work can be closed with policy or no-move artifacts if exact movement is unsafe.

## Baseline

```text
total_records=341
script-only=170
unknown=0
shared-state=0
dynamic=0
legacy=0
```

## Direction fit

| Direction | Feasible in M179 | Boundary |
|---|---:|---|
| Next exact script-only family wave | Yes | Exact source-path rules only. |
| Canonical baseline CI drift policy | Yes | Use temp current outputs and a committed canonical baseline path. |
| Cache lifecycle review | Yes | Movement only with exact lifecycle proof; otherwise no-move review is valid completion. |

## Execution rule

S02 and S03 select and freeze the exact script wave before scanner edits. S06 through S08 make CI canonical-baseline based. S09 performs cache lifecycle review without broad target-name or cache-prefix classification.

## Not doing

- No broad path prefix classification.
- No generic target-name rules such as `cache_path`, `output_path`, or `summary_path`.
- No strict CI rewrite that generates tracked current-inventory files during CI.
