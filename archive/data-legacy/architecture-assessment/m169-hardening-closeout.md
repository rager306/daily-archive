# M169 M061 Unknown Writes and Queue Soak Remediation Closeout

## Verdict

**M169 status: PASS.**

The milestone completed all three requested follow-up items together:

1. M061 dynamic import and artifact authority blocker resolved.
2. Remaining write-path unknown count reduced from 3 to 0 with atomic write hardening.
3. Multiprocess UniversalKBQueue soak added and verified.

## Item 1: M061 reconciliation and dynamic import closure

Status: **closed**.

Delivered:

- `tests/test_m061_s03.py` migrated from dynamic loader to normal `from scripts import m061_synthesis` import.
- Stale protected hash expectations updated to current tracked artifacts.
- `artifacts/m061-2hop/m061-summary.json` updated only in bounded deterministic fields approved by S03.
- `tests/test_m061_s03.py` moved from dynamic and legacy allowlists to strict script-wrapper coverage.
- Artifacts:
  - `data/architecture-assessment/m169-m061-artifact-recon.md`
  - `data/architecture-assessment/m169-m061-reconciliation-probe.json`
  - `data/architecture-assessment/m169-m061-reconciliation-harness.md`
  - `data/architecture-assessment/m169-m061-reconciliation-result.md`

Final counts:

```text
allowlisted_dynamic_script_import=0
allowlisted_legacy_mixed=0
strict_script_wrapper=57
violations=0
```

## Item 2: Remaining unknown write paths

Status: **closed**.

Delivered:

- CLI per-paper JSON writes now use atomic sibling-temp replacement:
  - `src/research_graph/cli/__init__.py::_atomic_write_text(...)`
  - `write_paper_artifacts(...)` routes `paper.json` and `scored.json` through it.
- PDF downloader cache writes now use atomic sibling-temp replacement:
  - `src/research_graph/infrastructure/corpus/ingestion/fetchers.py::_atomic_write_bytes(...)`
  - `PDFDownloader.download(...)` routes validated PDF bytes through it.
- Focused tests verify replacement behavior.
- Artifacts:
  - `data/architecture-assessment/m169-write-path-ownership-recon.md`
  - `data/architecture-assessment/m169-cli-write-resolution.md`
  - `data/architecture-assessment/m169-fetcher-write-resolution.md`
  - `data/architecture-assessment/m169-write-path-inventory.json`
  - `data/architecture-assessment/m169-write-path-inventory.md`
  - `data/architecture-assessment/m169-write-path-inventory-closeout.md`

Final counts:

```text
unknown=0
total_records=339
script-only=263
caller-owned=38
run-scoped=25
append-log=7
shared-state=4
temporary=1
database=1
```

## Item 3: Multiprocess UniversalKBQueue soak

Status: **closed for bounded pytest soak scope**.

Delivered:

- Added top-level process worker helper in `tests/test_universal_kb_queue.py`.
- Added `test_multiprocess_stress_claims_and_completes_each_job_once`.
- Soak bounds:
  - `job_count=16`
  - `process_count=4`
  - `join_timeout_seconds=15`
- Assertions verify no stuck processes, no worker errors, unique completed job ids, final `succeeded` status, and exactly one `claim` plus one `complete` event per job.
- Artifacts:
  - `data/architecture-assessment/m169-queue-soak-design.md`
  - `data/architecture-assessment/m169-queue-soak-result.md`

Verification:

```text
full queue suite: 25 passed
multiprocess stress repeated: 5/5 passed
```

## Integrated verification

Artifact:

```text
data/architecture-assessment/m169-integrated-verification.md
```

| Check | Result |
|---|---|
| Focused integrated pytest | PASS: 79 passed |
| Test architecture guard | PASS: dynamic=0, legacy=0, violations=0 |
| Write-path inventory | PASS: unknown=0 |
| Onion guard | PASS: zero violations |
| Scoped ruff | PASS after Python-only retry |
| Pyrefly | PASS: 0 errors |
| Pre-commit | PASS |
| GitNexus detect_changes | PASS: LOW risk, affected_processes=0 |
| Scope hygiene | PASS |

## Key files changed

- `tests/test_m061_s03.py`
- `artifacts/m061-2hop/m061-summary.json`
- `data/test-architecture-alignment/test-architecture-allowlist.json`
- `data/test-architecture-alignment/test-architecture-guardrail.json`
- `data/test-architecture-alignment/test-architecture-guardrail.md`
- `src/research_graph/cli/__init__.py`
- `tests/test_analysis.py`
- `src/research_graph/infrastructure/corpus/ingestion/fetchers.py`
- `tests/test_pdf_downloader.py`
- `tests/test_universal_kb_queue.py`
- `data/architecture-assessment/m169-*.md`
- `data/architecture-assessment/m169-m061-reconciliation-probe.json`
- `data/architecture-assessment/m169-write-path-inventory.json`

## Residual risks

1. `artifacts/m061-2hop/m061-summary.json` is historical evidence, but the update was bounded to deterministic values from current tracked source artifacts and preserved safety, graph, decision, anchor, request, and HTTP 429 invariants.
2. Atomic replacement prevents partial final cache files but does not provide lock-based same-key multi-writer coordination.
3. Queue soak is process-level bounded pytest evidence, not a long-duration production soak.
4. `shared-state=4` remains intentionally visible in the write-path inventory for future reviews.

## Follow-up backlog

1. If high-concurrency queue activation is planned, run a longer soak outside normal closeout bounds.
2. If same-key cache write contention becomes real, add explicit lock or compare-and-swap policy around CLI/PDF cache writes.
3. Consider extending atomic helpers to adjacent stable CLI daily/session artifacts only if a future inventory review marks them as shared-state risks.

## Conclusion

M169 closes the requested grouped remediation batch: dynamic import debt is zero, write-path unknowns are zero, and UniversalKBQueue now has process-level contention proof in the normal test suite.
