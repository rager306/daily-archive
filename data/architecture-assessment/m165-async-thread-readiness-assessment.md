# M165 Async and Thread Readiness Assessment

## Verdict

**Async/thread readiness verdict: PARTIAL COMPLIANCE.**

The repository has good async-first direction and several important M164 hardening wins, but it is not yet strictly ready for future high-concurrency or multithreaded execution. The main blockers are import-time environment mutation in `research_graph.cli`, representative-only atomic write migration, limited adapter lifecycle coverage, and incomplete ownership/locking contracts for shared state and worker execution.

## Evidence

- Async/thread scan evidence: `.gsd/exec/32a68241-ce79-42dd-aee5-fd873f0bd0c1.stdout`
- Reviewed files:
  - `src/research_graph/cli/__init__.py`
  - `src/research_graph/infrastructure/retrieval/embedder.py`
  - `src/research_graph/infrastructure/corpus/sources/markdown_converter.py`
  - `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py`
  - `src/research_graph/workflows/universal_kb/queue.py`
  - `tests/test_analysis.py`
  - `tests/test_embedder.py`
  - `doc/onion-layers.md`

Scan summary:

```text
asyncio_run: 6 files
get_running_loop: 4 files
to_thread: 0 files
run_in_executor: 1 file
thread_pool: 6 files
semaphore: 2 files
lock: 2 files
gather/create_task: 3 files
os_environ_mutation pattern: 5 files
close methods: 8 files
many direct write_text/open-write paths remain
```

## Strengths

### S1 — Async-first public surfaces exist

Current policy and tests support:

- `run_analysis_async`,
- `run_pipeline_async`,
- `run_command_async`,
- sync wrappers for process-boundary compatibility only.

Tests confirm sync wrappers fail inside active event loops and point callers to async APIs.

### S2 — CPU-ish scoring work is offloaded and bounded

`run_analysis_async()` uses `_score_papers_bounded()` with an `asyncio.Semaphore`, and `_process_paper_async()` uses `loop.run_in_executor(None, ...)` for extraction/scoring. Test coverage verifies the concurrency cap and result order preservation.

Assessment: good local pattern, but executor ownership is still implicit because the default executor is used.

### S3 — Representative lifecycle ownership exists

`Embedder` tracks whether it owns the injected HTTP client:

```text
self._owns_client = client is None
close() closes only owned clients
```

`tests/test_embedder.py::test_embedder_close_does_not_close_injected_client` proves this.

### S4 — Sync wrappers guard active event loops

Reviewed examples:

- `MDConverter.convert_sync()` checks `asyncio.get_running_loop()` and fails inside an active loop.
- `acquire_sources_for_manifest_sync()` does the same.
- Embedder sync wrappers are tested.

This matches the documented async-first policy.

### S5 — Queue state write has atomic representative path

`write_state_json()` uses a same-directory temp file and atomic replacement; the test asserts no temp file remains.

## Findings and gaps

### G1 — Import-time environment mutation remains in `research_graph.cli`

`src/research_graph/cli/__init__.py` loads `.env` and calls `os.environ.setdefault(...)` at module import time.

This is acceptable for a pure script process boundary, but `research_graph.cli` is also imported by tests and other Python code. For future async hosts and multithreaded processes, import-time environment mutation is a hidden global side effect.

Severity: **P1 / medium-high**.

Recommended direction: move `.env` application into the Typer/process entrypoint path or an explicit `apply_cli_env_config()` function. Keep library imports non-mutating.

### G2 — Atomic/run-scoped writes are not systematic

The scan found many `write_text()` and open-write paths in `src/` and `scripts/`. Some may be safe because they write run-scoped artifacts; some may not. M164 migrated `write_state_json()` only as a representative shared-state path.

Severity: **P1 / medium-high** for future multi-worker mode.

Recommended direction: inventory write paths into categories:

1. shared mutable state: atomic/locked/single-writer required,
2. run-scoped artifacts: unique output directory required,
3. append logs/databases: explicit lock or single writer required,
4. scripts/prototypes: process-boundary only.

### G3 — Default executor ownership is implicit

`run_analysis_async()` offloads scoring through the event loop default executor. The semaphore caps task fanout, but the actual threadpool size and lifecycle remain owned by the event loop.

Severity: **P2 / medium**.

Recommended direction: before scaling, make the executor policy explicit or document why the default executor is acceptable for the expected workload.

### G4 — Shared adapter policy is documented but not broadly tested

Embedder ownership is tested. Other close-capable resources exist, including validation logging, markdown converter, queue, and scripts. They do not all have explicit shared-instance/close-order tests.

Severity: **P1 / medium-high** if adapters become shared across concurrent workers.

Recommended direction: add lifecycle tests for any adapter intended to be injected or reused across async tasks/threads.

### G5 — Queue/storage concurrency contract is not fully reviewed here

`workflows/universal_kb/queue.py` has a close method and uses SQLite-backed state. This assessment did not prove multi-process lease safety or write contention behavior; it only notes that future queue activation must be reviewed as a concurrency milestone.

Severity: **P1 before queue activation**.

Recommended direction: separate queue concurrency assessment before enabling high-concurrency workers.

### G6 — Script threadpools and integration scripts are outside strict package runtime guarantees

ThreadPoolExecutor usage appears in package worker code and several scripts. Scripts may be process-boundary tools, but package worker threadpool behavior needs explicit resource budgets if used in production workflows.

Severity: **P2**.

## Backlog

| Priority | Item | Rationale |
|---|---|---|
| P1 | Remove import-time `.env` mutation from `research_graph.cli` library import path | Hidden global mutation is unsafe for async hosts and test isolation. |
| P1 | Classify all production write paths as atomic, run-scoped, append/locked, or script-only | Required before multithreaded/multiprocess execution. |
| P1 | Add lifecycle/ownership tests for shared-capable adapters beyond `Embedder` | Prevents close-order races and cross-task resource leaks. |
| P1 | Run a dedicated UniversalKBQueue concurrency review before queue activation | SQLite lease semantics and multi-worker contention need explicit proof. |
| P2 | Make executor ownership/resource policy explicit for analysis scoring and worker pools | Semaphore caps fanout but not executor lifecycle or global threadpool sizing. |
| P2 | Add cancellation/timeout behavior tests for long-running async workflows | Future async hosts need predictable cancellation and cleanup. |

## Final async/thread conclusion

The architecture is ready for controlled async use and has a much stronger async-first posture than M163. It is **not yet strictly multithread/multi-worker ready**. Before high-concurrency operation, the project needs systematic write-path classification, environment mutation removal from import paths, broader adapter lifecycle contracts, and queue concurrency proof.
