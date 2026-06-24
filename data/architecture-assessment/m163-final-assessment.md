# M163 Final Assessment: Hexagonal and Onion Architecture Strictness

## Overall verdict

**Overall strict verdict: PARTIAL COMPLIANCE, NOT STRICT COMPLIANCE.**

The repository has a strong and improving hexagonal/onion foundation: domain and application boundaries are guarded and currently clean, the Port taxonomy mostly matches ADR-034, async-first entrypoints are now documented and tested, and test architecture debt has been reduced. However, under a strict interpretation that includes entry/wiring and future multithreaded execution, the codebase is not fully compliant yet.

The main blockers to strict compliance are:

1. **Infrastructure imports entry/workflow contracts** even though docs classify workflows/CLI/scripts as entry/wiring.
2. **Workflows import scripts**, making scripts act as reusable package dependencies rather than process-boundary wrappers.
3. **Current guardrails enforce only domain/application**, not the full documented onion dependency matrix.
4. **Future async/multithread readiness is partial**, because concurrency budgets, artifact write isolation, DTO ownership, and adapter lifecycle contracts are not yet systematic.

## Verdict matrix

| Area | Verdict | Evidence | Main gap |
|---|---|---|---|
| Repository structure | Partial pass | `doc/onion-layers.md`, `ADR-034`, S01 import inventory | Entry/workflow/script boundary is not enforced. |
| Domain layer | Pass | `verify_onion_layering.py --json`: `violation_count=0`; `domain/ports.py` Ports | No strict issue found in assessed scope. |
| Application layer | Pass | `verify_onion_layering.py --json`: `violation_count=0`; application-local Ports | No strict issue found in assessed scope. |
| Infrastructure layer | Concern / violation | S01 found infra imports workflow/CLI modules | Infrastructure depends outward on entry/workflow contracts. |
| Entry and scripts | Concern | S01 found workflows importing `scripts.*` | Scripts are not consistently thin process-boundary wrappers. |
| Ports and adapters | Mostly pass | Domain/application/infra Protocol placement from S03 | One ambiguous infra logging Protocol tied to workflow logging. |
| Tests | Partial pass | `verify_test_architecture.py`: `violations=0`, dynamic=3, legacy=18 | 3 dynamic, 18 legacy-mixed, 77 unknown tests remain. |
| ADRs and decisions | Partial alignment | ADR-034 accepted and mostly implemented | Guardrail scope and workflow classification lag strict policy. |
| Async-first policy | Pass | `run_analysis_async`, `run_pipeline_async`, `run_command_async`; active-loop guards | Keep policy enforced for new APIs. |
| Multithread readiness | Partial | S04 inventory | Missing concurrency budgets, file write isolation, ownership contracts. |

## Evidence summary

### Commands and evidence artifacts

- S01 code layering: `data/architecture-assessment/m163-code-layering.md`
- S02 test architecture: `data/architecture-assessment/m163-test-architecture.md`
- S03 decision consistency: `data/architecture-assessment/m163-decisions.md`
- S04 async/threading: `data/architecture-assessment/m163-async-threading.md`
- Onion guard: `uv run python scripts/verify_onion_layering.py --json` → pass, `violation_count=0`.
- Test architecture guard: `uv run python scripts/verify_test_architecture.py --json` → pass, `violations=0`, `allowlisted_dynamic_script_import=3`.
- Supplemental S01 AST import inventory → 11 strict-boundary findings outside current guard scope.

## Strict architecture findings

### Passes

- `domain/` is guarded and currently clean against outward imports.
- `application/` is guarded and currently clean against infrastructure/workflow/CLI/script imports.
- Domain cross-cutting Ports exist in `domain/ports.py` and match ADR-034 categories.
- Application-local Ports exist beside use cases and avoid speculative domain abstractions.
- Infrastructure-local Protocols are mostly adapter-internal, which ADR-034 allows.
- CLI is correctly acting as a composition/entry surface for current async command flows.

### Violations under strict onion interpretation

| Priority | Finding | Evidence | Required direction |
|---|---|---|---|
| P1 | Infrastructure imports workflow contracts. | `infrastructure/repair/chunk_import_contract.py`, `infrastructure/papers/artifacts/models.py`, `batch_validation.py`, ingestion logging/loader. | Move pure workflow contracts inward to domain/application. |
| P1 | Infrastructure imports CLI DTO. | `infrastructure/graph/ladybug_client.py` references `research_graph.cli.DailyAnalysis`. | Move DTO to application/domain or define Protocol. |
| P1 | Workflows import scripts. | `workflows/validation/batch_workflow.py`; `workflows/universal_kb/smoke.py`. | Move reusable logic into package modules; keep scripts thin. |
| P1 | Guardrail misses above violations. | `verify_onion_layering.py` currently covers domain/application only. | Extend guard matrix to infra and workflows. |

## Test architecture findings

Current test guard state is healthy but not fully strict:

- Total test files: 269.
- Guardrail violations: 0.
- Dynamic script import debt: 3 files.
- Legacy mixed debt: 18 files.
- Unknown bucket: 77 files.
- Strict script-wrapper allowlist: 54 files.

Remaining dynamic candidates:

- `tests/test_m060d_s01.py`
- `tests/test_m061_s03.py`
- `tests/test_m062_s03.py`

Assessment: tests are good enough to protect current boundaries, but not strict enough to prove full onion conformance or future concurrency safety.

## ADR and decision findings

ADR-034 remains valid and should not be discarded. The issue is incomplete enforcement and some implementation drift:

- **Aligned:** core domain/application layering, Port taxonomy, adapter intent, async-first sync-wrapper policy.
- **Partial:** guardrail only enforces inner layers.
- **Drift:** docs classify workflows/scripts as entry/wiring, but live code treats them as reusable package dependencies.

Recommended decision follow-up: add an ADR-034 addendum or follow-up decision that explicitly classifies `workflows/` either as entry-only or as application orchestration package. Then enforce the chosen classification in code and guardrails.

## Async and multithread readiness findings

The repository is suitable for controlled async entrypoints today. It is not yet strict-ready for broad multithreaded or multi-worker execution.

Strengths:

- Async-first public APIs exist for core CLI flow.
- Sync wrappers fail inside active event loops.
- Embedder owns/cleans its async HTTP client correctly in main flow.
- Models registry cache uses a lock.
- Some scripts use bounded ThreadPoolExecutor.

Gaps:

- `run_analysis_async` gathers all paper tasks and uses the default executor without an explicit worker budget.
- Deterministic artifact paths are written without a repository-wide atomic write or lock convention.
- `Embedder` mutable metrics/circuit state are safe by per-instance convention, not by thread-safety contract.
- Worker boundaries depend on DTOs/contracts that are not fully inward-layered.
- Cancellation/timeout tests are uneven outside selected async HTTP paths.

## Priority remediation plan

### P1 — Required for strict compliance

1. **Extend onion guardrails.** Add checks that infrastructure cannot import `research_graph.cli`, `research_graph.workflows`, or `scripts`, and that workflows cannot import `scripts` unless explicitly allowlisted.
2. **Move workflow contracts inward.** Split pure DTOs/contracts from workflow orchestration modules into domain or application-local packages.
3. **Move CLI DTOs inward.** Move `DailyAnalysis` and related data shapes out of CLI if infrastructure or application needs to reference them.
4. **Convert scripts used by workflows into package modules.** Keep `scripts/` as process-boundary wrappers.
5. **Define concurrency budgets.** Bound `run_analysis_async` fanout with an explicit semaphore/executor and tests.

### P2 — Required before broad async or multithread expansion

6. **Artifact write safety contract.** Define run-scoped directories or atomic write/lock conventions for shared outputs.
7. **Object ownership contract.** State whether adapters, embedders, queues, and converters are per-run, per-task, per-thread, or shared.
8. **Concurrency tests.** Add cancellation, timeout, and parallel-worker tests for key adapters and filesystem outputs.
9. **Ratchet test debt.** Continue reducing 3 dynamic candidates, 18 legacy-mixed tests, and 77 unknown tests.

### P3 — Maintenance hardening

10. **Document guardrail scope in contributor docs.** Avoid treating current guard green as full strict compliance until P1 is done.
11. **Keep Ponytail Port rule.** Do not introduce broad Ports just to satisfy symmetry; move only real seams inward.

## Final answer to the user's question

- **Repository architecture:** directionally correct, not strictly compliant.
- **Code:** inner layers are clean; infrastructure and entry boundaries have strict violations.
- **Tests:** guardrails pass; architecture debt remains tracked and meaningful.
- **Architectural decisions:** ADR-034 remains sound but only partially enforced.
- **Future async readiness:** acceptable for controlled async entrypoints.
- **Future multithreading readiness:** not yet acceptable for broad worker/multi-thread execution without P1/P2 remediation.
