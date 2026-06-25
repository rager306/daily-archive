# M165 Test Architecture Assessment

## Verdict

**Test architecture verdict: STRONG PARTIAL STRICTNESS.**

The test suite has explicit architecture guard tests and live guardrails are green, but it is not yet a fully strict architecture proof because the test architecture guard still relies on allowlisted dynamic and legacy-mixed buckets, and concurrency/write/lifecycle coverage is representative rather than systematic.

## Evidence

### Live guard evidence

- Test architecture guard: `.gsd/exec/bb4b20fd-0df9-4c8c-b028-c821dff93cbc.stdout`
- Architecture-relevant test inventory: `.gsd/exec/d4c2f5d4-11f6-475a-99cd-e58fe96f8d60.stdout`

Guard summary:

```text
status=passed
violations=0
total_test_files=269
strict_application=6
strict_infrastructure=6
strict_script_wrapper=54
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=18
```

### Directly reviewed tests

| Test file | Assessment |
|---|---|
| `tests/test_onion_layering.py` | Strong. It tests real clean layers and synthetic failure cases for domain, application, infrastructure, and workflow boundaries. |
| `tests/test_test_architecture_guardrail.py` | Strong as a guardrail unit test. It verifies allowlist pass/fail behavior and strict application/domain rejection rules. |
| `tests/test_analysis.py` | Strong for async-first CLI behavior, sync-wrapper active-loop failure, bounded analysis fanout, failure-state persistence, and representative atomic queue state write. |
| `tests/test_embedder.py` | Strong for async embedder behavior, sync-wrapper active-loop failure, and injected-client lifecycle ownership. |

## Strengths

### S1 — Guardrails have failure-case tests

`tests/test_onion_layering.py` does not only assert the real repo is clean; it constructs bad synthetic modules and verifies that forbidden imports fail. That reduces false confidence from a green current-state guard.

Examples covered:

- domain importing application/infrastructure,
- application importing infrastructure/scripts,
- infrastructure importing CLI/workflows/scripts,
- workflows importing scripts.

### S2 — Test architecture guard is itself tested

`tests/test_test_architecture_guardrail.py` verifies the guard distinguishes allowlisted and unallowlisted legacy/dynamic cases, and rejects strict application/domain tests with forbidden imports.

### S3 — Async-first entrypoints are protected

`tests/test_analysis.py` verifies:

- `run_analysis_async`, `run_pipeline_async`, and `run_command_async` are coroutine functions,
- sync wrappers delegate through `asyncio.run(...)`,
- sync wrappers fail explicitly inside an active event loop,
- `run_command_async` persists failed state without leaking traceback payloads.

### S4 — Representative concurrency and lifecycle contracts exist

- `_score_papers_bounded(..., concurrency=2)` is tested to cap max active scoring at 2 while preserving result order.
- `write_state_json()` is tested to preserve schema and leave no `.*.tmp` file behind.
- `Embedder.close()` is tested not to close caller-injected clients.

## Gaps

### G1 — Allowlisted dynamic and legacy buckets remain

The guard reports:

```text
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=18
```

Known dynamic candidates remain present:

- `tests/test_m060d_s01.py`
- `tests/test_m061_s03.py`
- `tests/test_m062_s03.py`

This does not mean current architecture is broken, but it means the test suite is not fully strict. It still has historical exceptions that must be audited before claiming strict test architecture compliance.

Risk: **medium**.

### G2 — Concurrency coverage is representative, not systematic

The suite covers analysis fanout and Embedder ownership, but it does not systematically prove thread/process safety across all artifact writers, all adapters, or all workflow runners.

Risk: **medium to high** for future multi-worker execution.

### G3 — Atomic write coverage is narrow

`write_state_json()` is covered, but M164 intentionally did not migrate or test every artifact write path. Many tests write artifacts through tmp paths, but that is not equivalent to proving atomic/run-scoped production writes.

Risk: **medium**.

### G4 — Tests still encode compatibility imports

Compatibility shims are tested/kept green for old import paths. This is useful for migration safety, but long-term strictness needs a ratchet preventing new code from depending on deprecated shim homes.

Risk: **medium**.

## Backlog

| Priority | Item | Rationale |
|---|---|---|
| P1 | Reduce or reclassify the 3 dynamic script-import tests | Dynamic script loading is hard to reason about under strict package boundaries. |
| P1 | Add shim-regrowth tests that fail if new production imports target deprecated workflow shims | Prevents transitional surfaces becoming canonical again. |
| P1 | Add broader write-path safety tests for shared artifact writers | Needed before multi-worker execution. |
| P2 | Add adapter lifecycle tests for other async adapters, not just `Embedder` | Future shared adapters need explicit ownership and close-order contracts. |
| P2 | Add thread/process contention tests for representative workflow runners | Required before claiming multithread readiness rather than async API readiness. |

## Final test-architecture conclusion

The test suite is strong enough to protect the current M164 architecture guardrails and key async-first contracts. It is not yet a strict proof for future async/multithread execution because exception buckets and representative-only concurrency coverage remain.
