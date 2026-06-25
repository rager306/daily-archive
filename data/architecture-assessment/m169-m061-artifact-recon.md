# M169 M061 Artifact Authority Recon

## Verdict

**M061 blocker is artifact authority drift, not import mechanics.**

`tests/test_m061_s03.py` is currently baseline-red even before normal-import migration. Normal import of `scripts.m061_synthesis` succeeds, but two protected hashes in the test are stale relative to tracked repository artifacts, and the written `m061-summary.json` aggregate is stale relative to `scripts.m061_synthesis.collect_summary(...)` over the current tracked anchor artifacts.

## Evidence

| Probe | Result | Evidence |
|---|---|---|
| Current focused test | FAIL: 5 passed, 2 failed | `gsd_exec[916fe1d1-a145-48f4-9073-cb6cb153dd26]` |
| Normal import and drift probe | PASS probe: `normal_import=ok`, drift found | `gsd_exec[8e3f85f8-72d6-41ba-b2ee-db75a921336a]` |
| Artifact status | Clean tracked files | `git status --short` returned no M061 artifact changes |
| Artifact summary probe | Current summary values captured | `gsd_exec[55f867bb-e343-479d-a40f-922a5e8af670]` |

## Failing tests today

```text
FAILED tests/test_m061_s03.py::test_m050_m064_s01_s02_regression
FAILED tests/test_m061_s03.py::test_synthesis_collect_summary_matches_written_artifact
```

The dynamic loader is not the cause of these failures.

## Normal import result

```text
from scripts import m061_synthesis
normal_import=ok
```

This means S04 can remove the dynamic import mechanics once artifact authority is reconciled or the blocker is explicitly preserved.

## Protected hash drift

```text
artifacts/m061-2hop/s01-decision.md
  expected=231cb251d89c5b77a68007ebf93efbde20be3ad97b32829500ca1b5e663a51e0
  actual=2fe79a7a0129f2971b9b99896c339902abc92e1e9e5a04a96e6878415ff2a561
  match=False

artifacts/m061-2hop/s02-decision.md
  match=True

artifacts/m061-2hop/anchor-2605.18747/pipeline-summary.json
  expected=28398554a4e6470956ed58cda6c0ec879ff509fb7eb49be6c81b1690d45544db
  actual=5214a274545547f7175d9444419e7a49e46f3e46f5c0251dc3c3cc4b0bd6869f
  match=False

artifacts/m061-2hop/5-anchor-5-layer-graph-manifest.json
  match=True
```

Because `git status --short` reports these files as clean, the current tracked repo state disagrees with the test's frozen expected hashes.

## Summary aggregate drift

`collect_summary("2026-06-13T00:00:00Z")` over current tracked artifacts differs from `artifacts/m061-2hop/m061-summary.json` only in these aggregate values:

```text
aggregate.average_pacing_delay_seconds
  collected=2.8385302972259456
  written=2.882633399566519

aggregate.cumulative_real_paper_throughput_per_min
  collected=6.929166867747222
  written=7.1128780948369705
```

Graph data matches, and anchor ordering matches.

## Current summary facts

Current tracked `artifacts/m061-2hop/m061-summary.json` reports:

```text
fully_processed_real_paper_count=150
total_arxiv_requests=323
average_pacing_delay_seconds=2.882633399566519
cumulative_real_paper_throughput_per_min=7.1128780948369705
anchors=['2605.18747', '2401.04016', '2207.05608', '2505.19443', '2510.12157']
```

## Authority recommendation for S03

Use the **current tracked source artifacts plus deterministic `scripts.m061_synthesis.collect_summary(...)`** as the reconciliation candidate, but do not rewrite `m061-summary.json` or protected expected hashes until S03 proves the update is bounded and preserves the safety invariants.

Rationale:

- The source artifacts are clean tracked files in the repository.
- The script imports normally and deterministically recomputes summary values from those artifacts.
- The existing test is already red, so leaving it unchanged preserves neither a passing baseline nor a useful ratchet.
- Blindly replacing hashes or summary files would still be unsafe without a focused harness explaining exactly which values changed and why.

## Stop condition

If S03 cannot prove that the only changes are the two protected-hash expectations and the two aggregate values above, S04 should not rewrite artifacts. It should instead keep `tests/test_m061_s03.py` allowlisted and record the blocker as requiring human artifact authority review.
