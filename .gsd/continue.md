# continue — M465 S03 W2 shipped; S01–S02 skip-delivered; next S04 bibliography

## Active state (updated 2026-09-04 after M465/S03 W2)

Parse-plane: S01 fc9dfae6a, S02 8d28e01fc, S03 header sequence implemented.
Ledger header atoms are shadow with crate surfaces. run_header_sequence is
date then name then affiliation then compose; empty date or empty name cannot
invent a HeaderCandidate. Graph frozen (R118). Next is S04 W3 bibliography
(segment then fields then compose_references; citation names still absent).
Do not treat 9-lane segmentation report as 202-page pass.
Product: `M446-rw9a2p` S03 still 23/25, pending T24/T25.
Branch `feat/da-parse`.

```text
branch: feat/da-parse
HEAD:   6b941d02b (doctor snapshot of continue.md only; parent 549e333e1)
GROBID: alive (http://127.0.0.1:8070/api/isalive → true)
```

## Running processes (as of health repair)

- Locked auto `bg_908cbcb4` / PID 893498 is **dead**. Doctor could not see it
  as a crash lock (`workers.status=stopping` with no in-flight dispatch).
  Engine `forceReleaseLeasesForWorker` released the expired M446 lease
  (fencing_token 41). A new locked auto is allowed.
- Supervisor `eval_batch` PID 863349/863363 was also dead at health time.
  Check `/tmp/m446_eval_batch.log` before starting another eval_batch.

## What shipped this session

1. **mixed-source liquidated (user order)**: 75 unique articles moved to
   typed arXiv categories (primary_category via live arXiv API, network
   authorized by user — D320), 13 byte-identical dups deleted, stale
   `2501.07811` mixed copy deleted (typed cs-se canonical per arXiv
   primary + sweep). 4 PDF-less articles fetched live from arXiv.
   Backup manifest: `artifacts/mixed_source_backup_manifest.json`.
   Commits: `abb8aa6c4` (consolidation) + `33d9bbc1b` (gold dedup fix:
   byte-identical → deterministic path choice, real conflicts stay
   fail-closed) + `b23b7f829` (recovery-merge bare-ID normalization).
2. **Honest denominators proven**: inventory 807 − 200 gold (excluded by
   design, go through eval_batch) = 607 non-gold; report closed 612/612;
   run **594 completed / 0 failed / 0 replay_noop**; drift 0 alarms.
   `papers_removed` vs the 589-baseline is expected for gold IDs.
3. **Unicode span panic fixed** (`ee65b756d`): `RuleBasedExtractor`
   clamps all projected spans to UTF-8 boundaries (was panic on `•`
   in 2005.14165). Suite 51 passed; single-paper probe 1/0.
4. **Sweep resume made cumulative** (`d8c011a97`, `5729c78ba`): runner
   preserves prior typed rows; Verify is read-only (mutating sweep in
   Verify trips gsd-source-integrity — proven twice).
5. T04–T22 sealed except T22-light-core; T22 split into T23/T24/T25
   (monolithic Verify exceeds 600s exec cap — proven exit 124).

## Next action

1. Wait for `bg_2894ad34` eval_batch → collect micro/macro F1 from
   `/tmp/m446_eval_batch.log` (need micro ≥0.869, macro ≥0.859; do NOT
   edit gold on failure).
2. T23 consumes F1 evidence → T24 (index bench on sealed snapshot) →
   T25 (governor architecture) → validate → complete M446.
3. If headless stalls on closeout-backstop again (recurring lifecycle
   bug: `Host verification requires the latest succeeded canonical
   Attempt`), use sanctioned recovery: `gsd_task_recovery_resume` +
   clear paused KV via `db/runtime-kv.js` + locked headless restart.
   Never hand-edit `gsd.db`; never run two locked autos.

## Locks

- `import_eligible=false`; D127 unchanged. No archived extraction/
  embedding payloads as current evidence (baseline-only).
- D320: user authorized live arXiv network (API + PDF fetch).
- Single cargo writer; per-crate commands only (rocksdb trap).
- `data/sources/arxiv` is git-ignored except MANIFEST/SCHEMA;
  `artifacts/catalog_sweep/pipeline_cache/` local-only (git info/exclude).
