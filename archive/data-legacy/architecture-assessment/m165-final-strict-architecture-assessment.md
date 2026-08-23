# M165 Final Strict Architecture Assessment

## Final verdict

**Overall verdict: PARTIAL STRICT COMPLIANCE, NOT FULL STRICT COMPLIANCE.**

The repository now satisfies the **strict onion import dependency matrix** after M164: live guardrails report zero blocked imports and zero bounded-debt imports across `domain`, `application`, `infrastructure`, and `workflows`.

However, strict compliance for the whole repository also requires governance, tests, and future concurrency readiness to be strict. Those are not fully strict yet:

- tests still have allowlisted dynamic/legacy categories,
- docs and ADR text still contain stale pre-M164 guard-scope language,
- `research_graph.cli` mutates `os.environ` at import time,
- artifact write safety and adapter lifecycle contracts are representative, not systematic,
- queue/multi-worker concurrency has not been proven.

So the correct assessment is:

```text
Code import architecture: STRICT PASS
Hexagonal/onion architecture as implemented: HIGH COMPLIANCE WITH TRANSITIONAL SHIMS
Test architecture strictness: STRONG PARTIAL
Decision/doc governance: SUBSTANTIALLY ALIGNED WITH STALE TEXT
Async/thread readiness: PARTIAL
Overall strict architecture: PARTIAL STRICT COMPLIANCE
```

## Compliance matrix

| Area | Verdict | Evidence | Blocking gap for full strict claim |
|---|---|---|---|
| Repository structure | High compliance | Physical `domain/`, `application/`, `infrastructure/`, `workflows`, `scripts` entry surfaces | Compatibility shims remain and need lifecycle policy. |
| Domain layer | Strict pass | Onion guard: 11 domain files, 0 violations | None found. |
| Application layer | Strict pass | Onion guard: 18 application files, 0 violations | None found for imports; runtime purity still broader than import guard. |
| Infrastructure boundary | Strict pass | Targeted scan: 0 `infrastructure -> cli/workflows/scripts` imports | Runtime/write/lifecycle behavior still needs broader proof. |
| Workflow/script boundary | Strict pass for imports | Targeted scan: 0 `workflows -> scripts`; 4 M164 scripts are thin wrappers | Historical scripts are not globally wrappers; acceptable while not imported by package/workflow code. |
| Ports/adapters | Mostly pass | D086/D088 taxonomy, `domain/ports.py`, app-local Protocols, infra-local Protocols | Need shim lifecycle and more adapter lifecycle tests before broad shared use. |
| Tests | Strong partial | Guard status passed, 0 violations; synthetic failure tests exist | 3 dynamic script-import and 18 legacy-mixed allowlisted tests remain. |
| Decisions/ADRs | Substantially aligned | D086/D087/D088 and ADR-034 align with implementation intent | ADR/doc enforcement text stale after M164. |
| Async-first APIs | Pass for current controlled use | `run_analysis_async`, `run_pipeline_async`, `run_command_async`; active-loop guard tests | CLI import-time env mutation and default executor policy remain. |
| Multithread/multi-worker readiness | Partial | Bounded fanout, atomic queue-state write, Embedder lifecycle ownership | Write paths, adapter sharing, queue concurrency, cancellation/timeouts not systematic. |

## Evidence table

| Evidence | Result |
|---|---|
| `.gsd/exec/3ee90692-73e7-4ea1-a359-e587109dea9f.stdout` | Onion guard `status=clear`, `violation_count=0`, `allowed_violation_count=0`. |
| `.gsd/exec/bb4b20fd-0df9-4c8c-b028-c821dff93cbc.stdout` | Test architecture guard `status=passed`, `violations=0`, but allowlisted dynamic=3 and legacy=18. |
| `.gsd/exec/f6f146b3-383c-478f-95ea-7adc37d3e20a.stdout` | Targeted code scan found zero forbidden strict dependency paths; 11 shims, 4 script wrappers. |
| `.gsd/exec/d4c2f5d4-11f6-475a-99cd-e58fe96f8d60.stdout` | Architecture test inventory found key guard/async/concurrency/lifecycle tests and known dynamic candidates. |
| `.gsd/exec/32a68241-ce79-42dd-aee5-fd873f0bd0c1.stdout` | Async/thread scan found async wrappers, executor/semaphore usage, write paths, env mutation patterns, close methods. |
| `data/architecture-assessment/m165-code-layering-assessment.md` | Code layering verdict: strict import compliance, high hexagonal compliance with observations. |
| `data/architecture-assessment/m165-test-architecture-assessment.md` | Test architecture verdict: strong partial strictness. |
| `data/architecture-assessment/m165-decision-doc-assessment.md` | Decision/doc verdict: substantially aligned with stale enforcement text. |
| `data/architecture-assessment/m165-async-thread-readiness-assessment.md` | Async/thread verdict: partial compliance. |

## Strict findings

### What is now strict

1. **Import direction is strict and machine-checked.**
   - Domain does not import outward.
   - Application does not import infrastructure/workflows/CLI/scripts.
   - Infrastructure does not import CLI/workflow/script entrypoints.
   - Workflows do not import scripts.

2. **M163 P1 architecture violations were remediated.**
   - Contracts and DTOs moved inward.
   - Workflow-consumed script logic moved into package modules.
   - Guard scope expanded to infrastructure/workflows.

3. **Guard tests include synthetic failures.**
   - The guard is not only green on current code; it is tested to fail on forbidden imports.

4. **Async-first entrypoint policy is implemented for key CLI paths.**
   - Async APIs exist.
   - Sync wrappers fail inside active event loops.
   - Analysis scoring has bounded fanout.

### What is not yet strict

1. **Governance docs lag implementation.**
   - `doc/onion-layers.md` and ADR-034 still include pre-M164 guard-scope descriptions.

2. **Compatibility shims have no explicit lifecycle.**
   - They are acceptable now, but need a policy preventing new production imports to deprecated homes.

3. **Tests still rely on allowlists.**
   - `allowlisted_dynamic_script_import=3`.
   - `allowlisted_legacy_mixed=18`.

4. **`research_graph.cli` mutates process environment at import time.**
   - This violates the stricter future async-host/library-import expectation.

5. **Artifact writes are not systematically classified.**
   - `write_state_json()` is fixed and tested, but many direct writes remain unclassified as atomic/run-scoped/locked/script-only.

6. **Multi-worker queue and adapter concurrency is not yet proven.**
   - Embedder lifecycle is covered; other adapters and UniversalKBQueue concurrency need dedicated proof before broad parallel execution.

## Risk register

| Risk | Severity | Current impact | Future impact |
|---|---|---|---|
| Stale ADR/doc guard-scope text | Medium | Misleads future planning | Agents may under/over-enforce architecture. |
| Shim regrowth | Medium | No current import violation | Deprecated homes can become canonical again. |
| Dynamic/legacy test allowlists | Medium | Guard is green but not fully strict | Hidden architecture exceptions persist. |
| CLI import-time env mutation | Medium-high | Hidden global side effect during imports | Async hosts/tests/workers can observe order-dependent config. |
| Unclassified writes | Medium-high | Current controlled writes pass | Multi-worker runs can race or partially overwrite artifacts. |
| Queue concurrency not proven | High before queue activation | No immediate issue in sync/controlled use | Lease/write contention bugs under real workers. |
| Default executor policy implicit | Medium | Bounded fanout controls submissions | Threadpool sizing/lifecycle may be opaque at scale. |

## Remediation backlog

### P0

No P0 issue was found that invalidates current controlled operation or current strict import guard status.

### P1

1. **Remove CLI import-time environment mutation.**
   - Move `.env` application out of `research_graph.cli` import path and into explicit process-boundary setup.

2. **Update governance docs after M164.**
   - Fix `doc/onion-layers.md` guard-scope text.
   - Amend ADR-034 or add an ADR-034 addendum for four-layer guard enforcement.

3. **Record shim lifecycle policy.**
   - Deprecated workflow/CLI/script compatibility homes should be compatibility-only.
   - New production imports should target canonical homes.
   - Add tests/ratchet to prevent regrowth.

4. **Classify production write paths.**
   - Shared state: atomic or locked.
   - Run artifacts: run-scoped directories.
   - Append logs/databases: explicit lock or single writer.
   - Scripts/prototypes: process-boundary only.

5. **Run a UniversalKBQueue concurrency review before queue activation.**
   - Prove lease semantics, transaction boundaries, close ownership, retry races, and multi-process behavior.

### P2

1. Reduce or retire the 3 dynamic script-import test exceptions.
2. Reduce or reclassify 18 legacy-mixed test exceptions.
3. Add lifecycle tests for shared-capable adapters beyond `Embedder`.
4. Make executor/threadpool policy explicit for analysis scoring and worker modules.
5. Add cancellation/timeout cleanup tests for long-running async workflows.
6. Continue converting reusable historical scripts only when a package/workflow dependency appears.

## Architecture answer by requested dimension

### Repository

The repository structure is now consistent with hexagonal/onion layering, with source packages physically organized around core/application/infrastructure and entry surfaces. It is not perfectly strict at repository level because historical scripts, archive shims, and compatibility modules remain.

### Code

Code import boundaries pass strict checks. Broader runtime purity and concurrency safety are not fully proven by import checks.

### Tests

Tests are strong and architecture-aware, but still not strict because allowlists and partial concurrency coverage remain.

### Architecture decisions

Decisions are mostly correct and aligned. Documentation must be synchronized with post-M164 enforcement, and async/thread/shim policies should become durable decisions or ADR addenda.

### Architecture

The implemented architecture is a strong hexagonal/onion architecture with strict import boundaries. The full system should still be called **partially strict** until docs, tests, environment mutation, write safety, and queue/adapters concurrency are hardened.

## Final conclusion

M165 should supersede the M163 verdict in one important way: **the code-layer strict-boundary violations are closed.**

But M165 should not claim full strict compliance yet. The accurate current state is:

> **Strict import architecture: compliant. Full repository/code/test/decision/concurrency architecture: partially strict, high confidence, with P1 remediation needed before claiming strict async/multithread-ready hexagonal/onion compliance.**
