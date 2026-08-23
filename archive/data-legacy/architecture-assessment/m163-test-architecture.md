# M163 Test Architecture Audit

## Verdict

**Test architecture verdict: PARTIAL PASS.**

The automated test-architecture guard passes with zero violations, and M162 reduced dynamic script imports substantially. However, strict hexagonal/onion conformance is not complete: 3 dynamic candidates and 18 legacy-mixed tests remain allowlisted, and 77 tests are still `unknown` bucket. These are tracked debt, not new failures.

## Evidence

Command:

```bash
uv run python scripts/audit_test_architecture.py --output-dir <tmp> --json
uv run python scripts/verify_test_architecture.py --json
```

Results:

| Metric | Value |
|---|---:|
| Total test files | 269 |
| Guardrail violations | 0 |
| Acceptance bucket | 6 |
| Application bucket | 10 |
| Infrastructure bucket | 87 |
| Script-wrapper bucket | 71 |
| Legacy-mixed bucket | 18 |
| Unknown bucket | 77 |
| Dynamic script import signal | 3 |
| Strict application allowlist | 6 |
| Strict infrastructure allowlist | 6 |
| Strict script-wrapper allowlist | 54 |

Evidence command id: `gsd_exec` `705846d1-182f-4cf2-82e1-3e10d00d0e01`.

## Remaining dynamic candidates

These are still allowed but not strict:

- `tests/test_m060d_s01.py`
- `tests/test_m061_s03.py`
- `tests/test_m062_s03.py`

## Remaining legacy-mixed candidates

Examples include workflow and validation tests that mix layers or historical script patterns:

- `tests/test_import_boundary_rehearsal.py`
- `tests/test_m052_rlm_workflow.py`
- `tests/test_m060d_s01.py`
- `tests/test_m061_s03.py`
- `tests/test_m062_s03.py`
- `tests/test_universal_kb_queue.py`
- `tests/test_universal_kb_review_assistance.py`
- `tests/test_universal_kb_sidecar_boundary.py`
- `tests/test_universal_kb_smoke_cli.py`
- `tests/test_validation_batch_cli_article_report.py`
- `tests/test_validation_batch_cli_freshness.py`
- `tests/test_validation_batch_provenance.py`
- `tests/test_validation_batch_quota_fill.py`
- `tests/test_validation_batch_scan_workflow.py`
- `tests/test_validation_batch_state.py`
- `tests/test_validation_batch_top_up.py`
- `tests/test_validation_batch_workflow.py`
- `tests/test_validation_logging.py`

## Positive signals

- Tests now have a visible strict-script-wrapper path: 54 allowlisted strict script-wrapper files and 71 script-wrapper bucket files.
- `tests/test_analysis.py` covers async CLI entrypoints and active-event-loop failures for sync wrappers.
- `tests/test_thirty_paper_source_scan.py` covers `acquire_sources_for_manifest_sync()` active-loop failure and async source acquisition behavior.
- `tests/test_md_converter_isolated.py` includes many async backend failure paths and explicit sync-in-async-loop failure checks.
- M162 removed dynamic imports from four candidates without hiding runtime caveats.

## Strict concerns

| Severity | Finding | Why it matters | Recommendation |
|---|---|---|---|
| CONCERN | `unknown` bucket remains large at 77 files. | Guardrail cannot yet assert many tests are aligned to domain/application/infrastructure/script-wrapper intent. | Add bucket classification or targeted strict allowlist as tests are touched. |
| CONCERN | 18 `legacy_mixed` files remain. | Mixed tests can normalize architecture seams that production code should not rely on. | Convert by domain/use-case/infrastructure slices; avoid large batch rewrites. |
| CONCERN | 3 dynamic candidates remain. | Dynamic script import bypasses normal package/module boundaries. | Continue M162-style bounded repairs: reproduce, normal import, update stale expectations only with evidence. |
| CONCERN | Async and multithread coverage is uneven. | There are strong tests for CLI/source conversion async paths, but future worker/adapter concurrency is not systematically covered. | Add concurrency stress/contract tests only after S04 risks are prioritized. |

## Not violations

- Script-wrapper tests are acceptable when they intentionally validate historical scripts or process-boundary behavior, as long as they import scripts normally or use subprocess explicitly.
- Collection-only evidence for known slow tests is acceptable when the milestone is import architecture assessment, not full runtime validation.

## Recommendations

1. **P1:** Continue dynamic debt ratchet for the remaining 3 files.
2. **P1:** Add stricter guard checks for tests that import workflows/scripts once production layering rules are extended.
3. **P2:** Reduce `legacy_mixed` by converting validation/workflow tests into clearer entrypoint vs application/infrastructure tests.
4. **P2:** Add targeted async/concurrency tests for shared filesystem outputs, adapter close/lifecycle, and parallel worker execution after the code audit in S04.
