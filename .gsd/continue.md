# Handoff — 2026-08-12 (M386 Rust migration + M387 semantic pipeline active)

## Active milestone M386-57ae9l

Goal: Rust-only production pipeline (D237). Python remains only governor/tests/migration analysis/frozen evidence.

Delivered commit `9d57d53e`: typed batch acquisition report/use case and production `da acquire-arxiv`; real 580-ID no-network replay had 579 catalog hits and one withdrawn `2507.23261`; D127 false and zero Rust→Python subprocesses.

Remaining M386: typed article.v00.02 metadata builder; bounded network retry/backoff and partial/checksum conflict parity; archive Python downloader only after parity; governor ratchet; Rust-only end-to-end acceptance.

## Active milestone M387-jkpnfh

### S01 delivered `ccb27e08`

DATA-driven 14-stage acquisition→query ledger, strict DAG/cache/truth validation, runtime order/digest/failure audit. Durable `artifacts/runtime/pipeline_stage_ledger_m387.json`; UAT `6a431853-f9bf-4cd0-9e6a-5d71ea5eabe4`; full architecture PASS.

### S02 delivered `aebb7bce`

ACAP-aligned diagnostic context conservation:
- domain `StageContextSnapshot`, denominator+BoundaryMode `RatioObservation`, candidate/authoritative partitions and typed `no_claim`;
- no independent admission verdict; zero denominator is not perfect retention; tokens require tokenizer lineage; D127 fail-closed;
- pure parse, revision, extraction and summary/readiness observers; no graph writes;
- strict DATA `data/ontology/context_conservation_policy.yaml` with policy ID/digest; hard invariants cannot be weakened;
- real deterministic 30-article profile `artifacts/runtime/context_conservation_m387.json`: 30 parse + 30 extract payloads, 927 sections, 1,192 citations, 1,337,968 chars, 140 candidates, 117 valid ordinal/span groundings; 12 stages explicit no_claim; exact revision report D127 false;
- structured JSON lifecycle event verified in `/tmp/m387-logs/da.log.2026-08-12`;
- 11 focused tests, Clippy and full architecture PASS;
- UAT `17477aeb-a8f6-4340-b1bd-549059af799b` + `e947876c-cac9-49fa-abca-9af4c96ae428`;
- MEM752 records ACAP projection rule.

Reviewer blocked the first draft because it risked a second ACAP control plane/self-oracle. Reworked before commit. Do not reintroduce independent context PASS or tokenizer-free token counts.

### Exact next action: M387/S03 Cache Lineage Contracts and Logging Debt

Plan exists with four tasks:
1. shared PipelineCache contract suite against in-memory/filesystem implementations: cold/hit, key identity, payload hash, corruption, atomic publication, concurrent same-key and replay;
2. typed lineage report binds full key plus output SHA and every invalidation dimension; identical bytes under different keys remain distinct;
3. prove cache skips only compute, never revision/extraction graph materialization; Samyama replay applied=0/rejected=0/unobserved=0/unchanged>0;
4. close existing composition logging flag in `pipeline_ledger_audit.rs`, `materialization_profile.rs`, `materialization_hot_path_profile.rs`, then run 1/10/30 cache profile.

Then S04 bounded DAG scheduler/SMP, S05 profiles/ablations, S06 Samyama active path, S07 retrieval/RuVector applicability, S08 acceptance/governor.

## Session operational findings

- Governor initially BLOCKED the inline negative test literal `import_eligible=true`; moved it to `crates/da-domain/tests/` where explicit negative fixtures are allowed. Final active verdict drift_risk, 0 violations.
- Governor found `.env` rolling JSON logging missing in four profile roots. S02 root fixed; remaining three are explicit S03 debt.
- `target/` was 73.4GB. Ran `cargo clean`: removed 73.6GiB/142,634 files; post-clean target 132MB and `cargo check -p da-domain` passed. Preserve sccache.
- GSD journal query for M387 returned no stuck-loop/timeout events.
- Codebase Memory MCP search, architecture and fast reindex each timed out; GitNexus query/impact/detect_changes/reindex completed. Keep Codebase Memory supplemental, GitNexus authoritative.
- GitNexus HIGH impact on `ExtractedEntity`; S02 did not mutate it, observers are additive/read-only.
- Generated `artifacts/project-trajectory/trajectory-report.json` remains unstaged by policy.

## Locks

D127 false. D237 Rust-only product pipeline. No Rust→Python fallback. Samyama sole store. No implicit revision default. No legacy Paper summaries. Cache only pure compute; graph truth always materialized. Vendor edits only in isolated clean worktree. RocksDB/RuVector tuning only after active-path evidence. DSPy disabled.
