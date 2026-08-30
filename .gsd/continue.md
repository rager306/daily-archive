# continue — M464-z84in2 S14/T05: canonical envelope published, ready for auto closeout

## Active state (updated 2026-08-31)

Milestone `M464-z84in2` active. S01–S13 complete. S14: T01–T04 complete,
**T05 in_progress, canonical work DONE** — only Attempt-based DB completion
remains (via `/gsd auto`). S15–S19 unplanned (0 tasks) — auto plans them next.

```text
branch: feat/da-parse
HEAD:   f2a23c440 (canonical artifact commit)
fixes:  74ba16ecc (honest measurement surfaces) + 15f00e4d1 (D312 extras)
```

## What this session shipped

1. **Audit of the stale Luna attempt's WIP** (continue.md checklist, 7 points).
   Findings: F04 unfixed (matrix ran real pdfalto per worker count), fake
   `command_digest` (sha256 of a constant), self-declared `clean_tree_assertion`,
   `tested_source_revision` hashing manifest+version only, `execution_fingerprint`
   incomplete/misnamed, post-hoc fake "interrupt", vacuous `duplicate_tokens`/`extra_tokens`
   hard gates (hardcoded 0), `coordinate_coverage` always "complete", missing
   `#[serde(default)]` on `evidence_binding` (broke S13 prior deserialization →
   replay check would silently die), no runtime-config drift rejection on resume,
   `RuntimeMetrics` untested.
2. **All fixed** in `74ba16ecc` + `15f00e4d1` + `f2a23c440`:
   - CPU matrix (`1..=physical_core_cap`) runs in-process lanes only
     (`lane_scope: cpu_in_process`); real donor composition runs once at the
     representative count — F04 closed.
   - RuntimeMetrics observed (reserve-before-send, saturating rollback, peak
     recorded before blocking send) — F02 closed; regression test added
     (`runtime_metrics_are_observed_not_configured_echoes`).
   - Exact `tested_source_revision` = sha256(binary digest | manifest fp |
     pkg version | schema); `command_digest` = digest of real argv;
     full `runtime_config_fingerprint`; resume rejects drift (fail closed).
   - Incremental interrupt in the terminal loop (`report_state: partial`,
     gate `no_selection`).
   - Honest `extra_tokens` (D312 page-topology), `duplicate_tokens` (produced
     ordinals), `coordinate_valid` (finite non-degenerate bbox).
   - `WorkerRow` gained p50/p95/tokens/lane_scope; per-row `breakpoint`
     computed (none/first_mismatch/incomplete); REPORT_SCHEMA → v3.
3. **Canonical source-bound run executed** (D313's single authorized command,
   executed twice with identical source/config — second run solely binds
   `semantic_replays_equal=true`; exactly ONE acceptance denominator):
   `--manifest data/parse/layout_bakeoff.yaml --report
   artifacts/parse_compat/layout_bakeoff.json --pdfalto
   /root/vendor-source/grobid/grobid-home/pdfalto/lin-64/pdfalto
   --workers 12 --queue-capacity 24 --max-in-flight 12`
   Result: v3/final, 9/9 outcomes ok (semantic digests identical to S13),
   denominator 3/3/202/606 exactly once, matrix 1..=12 all-equal,
   gate `candidate_evidence_complete`, hard_vetoes 0, replays_equal=true,
   promotion_state=candidate, auto_promote=forbidden, route=fallback,
   redaction clean (no paths/token text), observed gauges in_q=8/out_q=1/in_flight=9.
4. **Verification**: fmt ok; `layout_runtime` 10 passed; `layout_smp_contract`
   6 passed; `layout_data_contract` 6 passed; `layout_bakeoff_test` 4 passed;
   clippy `-p da-cli --lib --tests -D warnings` clean.
   **Durable evidence: `gsd_exec f6e2aba0-3453-4ab2-8d0c-f64b9cfa7c79`
   (12/12 read-only assertions, artifact sha256 bce56fe5…e72e).**
   Decision **D314** records the measurement/binding semantics.
   Governor `architecture` gate: see `.gsd/exec/` journal entry for this
   session's run result before closeout (was green at last full run; rerun
   after final commits).

## Canonical artifact identity (preserve)

```text
artifacts/parse_compat/layout_bakeoff.json  (v3, committed at f2a23c440)
S13 predecessor: sha256 59377965ae2509fc6ed879baebe86ca5f55a21a4ab25d1f2d7b6a4500af29e81 (git history)
3 documents · 3 lanes · 202 pages/lane · 606 total lane-pages · 9 base outcomes
```

## Required next action (headless Luna auto)

The locked auto must NOT rerun measurements (D313 quota consumed; replays
bound). It must: claim T05 via execute-task (fresh Attempt; the orphaned
`d2846df3` attempt was settled as interrupted via `gsd_task_settle`), attach
evidence ref `gsd_exec f6e2aba0-3453-4ab2-8d0c-f64b9cfa7c79` to the task
summary WITHOUT rerunning the binary, run only the read-only verify
(`node .gsd/exec/t05-verify.mjs`) plus `PYTHONPATH=scripts python3 -m governor
architecture`, then complete T05/S14 through canonical gates (Q5–Q8) and
continue planning S15–S19.

Launch exactly one locked headless auto (only after confirming no other auto):

```text
GSD_MILESTONE_LOCK=M464-z84in2 \
  gsd headless --timeout 86400000 \
  --model 9router/cx/gpt-5.6-luna --thinking high \
  --output-format stream-json auto
```

Do not manually mark T05 complete and do not invent an Attempt.

## Locks

- `import_eligible=false`; D127 unchanged.
- Candidate evidence only; no donor selection or promotion
  (gate `candidate_evidence_complete` ≠ selection).
- `auto_promote=forbidden`; `stage.pdfalto_layout=fallback`; Java rollback retained.
- Optimization `not_applicable`, scalar oracle retained, `selection_forbidden=true`.
- No ParserPort, TeiLifter, cache, graph, import, endpoint, serving, or
  production route activation.
- S13 roadmap reassessment gap remains explicit; do not claim it was completed.
- Do not interfere with unrelated repositories/processes.
