# M169 M061 Reconciliation Harness

## Verdict

**Bounded reconciliation is feasible.**

S03 generated a machine-readable probe showing that the M061 blocker is narrow enough for S04 to attempt a safe fix. Normal import works; safety and graph invariants remain intact. The allowed S04 edit scope is limited to:

1. migrate `tests/test_m061_s03.py` from dynamic loader to normal `scripts.m061_synthesis` import;
2. update stale protected hash expectations for two current tracked artifacts;
3. update `artifacts/m061-2hop/m061-summary.json` to match deterministic `scripts.m061_synthesis.collect_summary(...)` for the current tracked source artifacts.

## Probe evidence

JSON artifact:

```text
data/architecture-assessment/m169-m061-reconciliation-probe.json
```

Command evidence:

```text
gsd_exec[294d1240-fba6-4941-8b27-24d75c5e45ee]
```

Probe summary:

```text
normal_import_ok=true
protected_hash_mismatches=2
aggregate_diffs=[
  average_pacing_delay_seconds,
  cumulative_real_paper_throughput_per_min,
]
anchor_diff_count=1
graph_diffs=[]
decision_diffs=[]
```

## Invariant checks

| Invariant | Result |
|---|---|
| Safety defaults preserved | PASS |
| Network references remain `127.0.0.1` | PASS |
| Graph summary matches | PASS |
| Decision summary matches | PASS |
| Anchor IDs and order match | PASS |
| Anchor count remains 5 | PASS |
| Total arXiv requests remains 323 | PASS |
| HTTP 429 count remains 0 | PASS |
| Fully processed real paper count remains 150 | PASS |

## Exact drift fields

### Protected hashes

The current tracked repo state differs from test constants for these two files:

```text
artifacts/m061-2hop/s01-decision.md
  old expected: 231cb251d89c5b77a68007ebf93efbde20be3ad97b32829500ca1b5e663a51e0
  current hash: 2fe79a7a0129f2971b9b99896c339902abc92e1e9e5a04a96e6878415ff2a561

artifacts/m061-2hop/anchor-2605.18747/pipeline-summary.json
  old expected: 28398554a4e6470956ed58cda6c0ec879ff509fb7eb49be6c81b1690d45544db
  current hash: 5214a274545547f7175d9444419e7a49e46f3e46f5c0251dc3c3cc4b0bd6869f
```

### Written summary artifact

`artifacts/m061-2hop/m061-summary.json` differs from recomputed `collect_summary(...)` in one anchor payload and aggregate values derived from that payload:

```text
anchors[0].average_pacing_delay_seconds
  collected=2.6414815602045447
  written=2.864064404829627

anchors[0].real_paper_throughput_per_min
  collected=6.3918567952554275
  written=7.256266372653047

aggregate.average_pacing_delay_seconds
  collected=2.8385302972259456
  written=2.882633399566519

aggregate.cumulative_real_paper_throughput_per_min
  collected=6.929166867747222
  written=7.1128780948369705
```

## Authority decision

Treat current tracked source artifacts plus deterministic `scripts.m061_synthesis.collect_summary(...)` as authoritative for M061 S03 test reconciliation.

Rationale:

- The files with current hashes are clean in git; the test constants are stale relative to the repository's current tracked state.
- `scripts.m061_synthesis` imports normally and recomputes summary values deterministically from those tracked source artifacts.
- Safety defaults, graph data, decision data, anchor ordering, request totals, and HTTP 429 counts are preserved.
- The current test is already red, so keeping stale constants does not preserve a useful passing ratchet.

## S04 edit contract

S04 may edit only these files unless a new blocker appears:

1. `tests/test_m061_s03.py`
   - replace the dynamic loader with a normal `from scripts import m061_synthesis` import;
   - remove now-unused dynamic import imports and helper;
   - update the two stale protected hash constants above.
2. `artifacts/m061-2hop/m061-summary.json`
   - update only `anchors[0].average_pacing_delay_seconds`;
   - update only `anchors[0].real_paper_throughput_per_min`;
   - update only `aggregate.average_pacing_delay_seconds`;
   - update only `aggregate.cumulative_real_paper_throughput_per_min`.
3. `data/test-architecture-alignment/test-architecture-allowlist.json`
   - remove `tests/test_m061_s03.py` from `dynamic_script_import` and `legacy_mixed` if the focused test passes after migration.
4. Generated guardrail outputs may update after running `scripts/verify_test_architecture.py --json`.

## S04 stop condition

Stop and preserve the allowlist if any of these happen:

- more summary fields change than the four listed above;
- safety defaults, graph data, decision data, anchor IDs, request totals, or HTTP 429 count change;
- normal import causes side effects or import failure;
- focused M061 test still fails after the bounded edits.
