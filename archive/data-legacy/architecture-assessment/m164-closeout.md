# M164 Closeout: Strict Onion Guardrails, Contracts, and Concurrency Hardening

## Verdict

**M164 remediation status: PASS.**

M164 implemented the requested combined remediation tracks from M163:

1. **Strict guardrails:** full-layer guard now scans `domain`, `application`, `infrastructure`, and `workflows`.
2. **Contracts and DTOs inward:** infrastructure no longer imports workflow contracts or CLI DTOs.
3. **Async/thread readiness:** analysis fanout is bounded, queue state writes are atomic, and adapter lifecycle ownership is documented/tested.

## Before and after boundary debt

| Category | M164 baseline | After S07 | Status |
|---|---:|---:|---|
| Infrastructure imports CLI | 1 | 0 | closed |
| Infrastructure imports workflows | 6 | 0 | closed |
| Workflow imports scripts | 4 | 0 | closed |
| Total strict-boundary findings | 11 | 0 | closed |
| Guard blocked violations | 0 | 0 | clean |
| Guard allowed debt | 11 during staged enforcement | 0 | closed |

Final guard result:

```text
status=clear
violation_count=0
allowed_violation_count=0
layers=['application', 'domain', 'infrastructure', 'workflows']
```

## What moved

### Canonical contract and DTO homes

- `research_graph.infrastructure.validation.logging`
  - canonical home for validation logging/event sink.
- `research_graph.application.validation.batch_state`
  - canonical home for validation batch DTO/state contracts.
- `research_graph.application.validation.batch_provenance`
  - canonical home for validation provenance helpers.
- `research_graph.domain.universal_kb.contracts`
  - canonical home for Universal KB fail-closed safety contracts.
- `research_graph.application.analysis`
  - canonical home for `DailyAnalysis` and `DailyAnalysisStatus`.

Workflow modules retain compatibility shims where needed, and CLI re-exports `DailyAnalysis` for old callers.

### Scripts converted to wrappers

- `scripts/select_m036_real_corpus_smoke_batch.py`
- `scripts/run_m036_real_corpus_no_write_smoke.py`
- `scripts/audit_m036_real_corpus_smoke.py`
- `scripts/run_quality_gate.py`

Reusable implementations now live in package modules:

- `research_graph.workflows.universal_kb.smoke_selection`
- `research_graph.workflows.universal_kb.smoke_runner`
- `research_graph.workflows.universal_kb.smoke_audit`
- `research_graph.infrastructure.quality.gate`

The scripts remain import-compatible wrappers that re-export implementation symbols and delegate `main()`.

## Async and multithread readiness improvements

- Added `ANALYSIS_SCORE_CONCURRENCY = 8`.
- Added `_score_papers_bounded()` using `asyncio.Semaphore`.
- Added async test proving concurrent paper scoring is bounded and result order is preserved.
- Added artifact write-safety policy to `doc/onion-layers.md`.
- Updated CLI queue state JSON writes to use same-directory temp file plus atomic `Path.replace()`.
- Added adapter ownership/lifecycle policy to `doc/onion-layers.md`.
- Added Embedder test proving injected clients are caller-owned and not closed by `Embedder.close()`.

## Verification summary

| Check | Result |
|---|---|
| Focused changed-area tests | PASS: `129 passed` |
| Onion guard | PASS: blocked=0, allowed=0 |
| Test architecture guard | PASS: `status=passed`, `violations=[]` |
| Scoped ruff on touched files | PASS |
| Pyrefly | PASS: 0 errors |
| GitNexus detect changes | PASS: LOW risk, affected_processes=0 |
| Full `uv run ruff check` | Known unrelated archive/package-layout shim failures; scoped touched-file ruff passes |
| Pre-commit | PASS |

## Remaining limitations

- Not every artifact write path was migrated to atomic writes; M164 establishes the contract and migrates queue state JSON as the representative shared state path.
- Adapter sharing remains intentionally unsupported unless a future requirement adds explicit locking and close-order contracts.
- Analysis scoring still uses the default executor inside each paper task, but fanout into that executor is now bounded.
- Compatibility shims remain for old workflow/CLI/script import paths; they can be removed in a future cleanup once downstream imports are migrated.

## Files to inspect first

- `scripts/verify_onion_layering.py`
- `tests/test_onion_layering.py`
- `doc/onion-layers.md`
- `src/research_graph/application/analysis.py`
- `src/research_graph/application/validation/`
- `src/research_graph/domain/universal_kb/contracts.py`
- `src/research_graph/infrastructure/validation/logging.py`
- `src/research_graph/infrastructure/quality/gate.py`
- `src/research_graph/cli/__init__.py`
- `tests/test_analysis.py`
- `tests/test_embedder.py`
