# M181 Combined Feasibility

## Verdict

**Combined scope is feasible.**

M181 can include all three requested directions if the scanner movement remains an exact source-path wave, docs/CI cleanup is limited to exact obsolete active references, and cache lifecycle review is allowed to close as explicit no-move when exact ownership proof is absent.

## Baseline

```text
total_records=341
script-only=122
unknown=0
shared-state=0
verify-m031-output=10
verify-m033-output=10
```

## Direction 1: exact residual script-only wave

Feasible. Quick candidate grouping shows `verify_m029` as the largest obvious residual verify family:

```text
verify_m029: 8
verify_m027: 4
replay_m031: 3
```

S02 will choose exact source paths and expected movement counts. No broad `verify_m029*`, `verify_*`, target-name, markdown, manifest, converter, index, or cache rule is allowed.

## Direction 2: canonical docs and CI cleanup

Feasible as a bounded cleanup/recon. The workflow is already canonical-only after M180, so M181 should not rework baseline semantics. Cleanup is limited to exact stale active references that can confuse maintainers; durable historical M17x/M18x evidence artifacts should remain historical unless they directly instruct active CI usage.

## Direction 3: cache lifecycle review

Feasible as review-first and likely no-move. Quick cache-like residual scan found manifest/index flavored script-only paths, not proven shared cache lifecycle ownership:

```text
scripts/benchmark_m055_corpus_manifest.py
scripts/build_m055deep_corpus_manifest_20.py
scripts/m058_build_graph_manifest.py
scripts/m059_build_manifest.py
```

No scanner movement will happen unless S08 proves exact source ownership, lifecycle, invalidation, and concurrency behavior. No-move remains a valid close.

## Decision

Proceed with all three directions in M181. If later recon invalidates docs cleanup or cache movement, those directions close as exact no-op/no-move artifacts while direction 1 continues with enough slices.
