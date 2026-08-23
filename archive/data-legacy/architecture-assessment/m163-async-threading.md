# M163 Async and Multithread Readiness Audit

## Verdict

**Future async and multithread readiness verdict: PARTIAL.**

The repository is moving in the right direction: key public surfaces now have async APIs, sync wrappers fail explicitly inside active event loops, HTTP clients expose async close paths, and some thread-based scripts use bounded worker counts. Strict future readiness is not complete: concurrency contracts are uneven, some async paths use unbounded default executors or gather all tasks at once, several mutable metrics/caches are only safe by convention, and filesystem outputs lack explicit multi-process locking/idempotency contracts.

## Evidence

Evidence command id: `gsd_exec` `8d2c7c22-fd21-45dd-8072-818dfecd7771`.

### Async-first strengths

| Surface | Evidence | Assessment |
|---|---|---|
| CLI async entrypoint | `src/research_graph/cli/__init__.py:433 run_analysis_async` | Good async host surface. |
| CLI sync wrapper guard | `src/research_graph/cli/__init__.py:487 run_analysis`; `:494 active event loop` | Good: sync wrapper fails in active loop. |
| Pipeline async entrypoint | `src/research_graph/cli/__init__.py:498 run_pipeline_async`; `:514 run_pipeline`; `:522 active event loop` | Good. |
| Command async entrypoint | `src/research_graph/cli/__init__.py:526 run_command_async` | Good orchestration surface. |
| Source acquisition sync guard | `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py:156`; `:163 active event loop` | Good, added in M161. |
| Embedder sync guard | `src/research_graph/infrastructure/retrieval/embedder.py:160 _raise_if_running_loop`; `:363 embed_batch_sync`; `:368 embed_all_sync` | Good active-loop policy. |
| MD converter sync guard | `src/research_graph/infrastructure/corpus/sources/markdown_converter.py:365-368` | Good active-loop policy. |

### Lifecycle strengths

| Surface | Evidence | Assessment |
|---|---|---|
| Embedder async client ownership | `src/research_graph/infrastructure/retrieval/embedder.py:217 _get_client`; `:222 close` | Good owner/injected-client distinction. |
| CLI embedder lifecycle | `src/research_graph/cli/__init__.py:461-470` uses `try/finally await embedder.close()` | Good cleanup in main async path. |
| Models registry cache | `src/research_graph/infrastructure/llm/models_registry.py:82-84` `_CACHE` plus `_CACHE_LOCK` | Good explicit thread-safety for module cache. |

## Risks

| Severity | Risk | Evidence | Why it matters | Recommendation |
|---|---|---|---|---|
| HIGH | CLI analysis fans out all papers into default thread executor with no explicit worker budget. | `src/research_graph/cli/__init__.py:421-423` uses `run_in_executor(None, ...)`; `:459-460` gathers all tasks. | Future larger batches can exhaust default executor, CPU, or memory; scoring/extraction may not be thread-safe. | Add explicit bounded executor or semaphore and document load budget. |
| HIGH | Package workflows and infrastructure share filesystem output paths without explicit multi-process locking. | CLI writes session/queue/day artifacts in `src/research_graph/cli/__init__.py:205-414`; many scripts write fixed artifact paths. | Concurrent runs can overwrite artifacts or interleave writes. | Introduce run-scoped output dirs or atomic write+lock conventions for shared paths. |
| HIGH | Infrastructure imports workflow/CLI contracts, making concurrent worker boundaries harder to reason about. | S01 infra→workflow/CLI findings. | Worker threads/processes should depend on stable inner DTOs, not entry-layer objects. | Move DTO/contracts inward before real worker pool expansion. |
| MEDIUM | `Embedder` metrics and circuit state are mutable and not lock-protected. | `src/research_graph/infrastructure/retrieval/embedder.py:197-204` mutable counters/lists; `:389+` mutates circuit state. | Safe for one event loop and one instance by convention; unsafe if shared across threads/tasks without policy. | Document per-run/per-task ownership or add async lock if shared instances become supported. |
| MEDIUM | Script thread pools are bounded but not generalized into reusable concurrency policy. | `scripts/acquire_linked_target_pdfs.py:242-253`; `scripts/m060g_figure_judge.py:817-829`. | Bounded scripts are acceptable, but future package workers need shared cancellation, retries, rate-limit, and cleanup contracts. | Keep scripts as process-boundary tools; move reusable worker semantics into application/infrastructure modules with tests. |
| MEDIUM | Async clients and sync wrappers are inconsistent across older modules. | Good examples exist in CLI/embedder/MDConverter/source acquisition, but many scripts still use sync network/file APIs. | Future async hosts can accidentally call blocking scripts. | Require new async package code to expose async API first; keep sync wrappers at entry/script boundaries only. |
| LOW | Models registry cache is thread-safe but has no invalidation beyond reset. | `models_registry.py:82-84`, `reset_cache()`. | OK for current process; long-lived workers need explicit reload policy. | Keep lock; add documented reload/invalidation if model registry becomes hot-reloaded. |

## Readiness scores

| Area | Score | Rationale |
|---|---:|---|
| Event-loop safety | 7/10 | Key sync wrappers now fail in active loops; older scripts still need boundary discipline. |
| Async API availability | 7/10 | Main CLI/embedder/source conversion surfaces have async APIs. |
| Thread safety | 5/10 | Some locks and bounded pools exist; mutable per-instance state and default executors lack contracts. |
| Multi-process file safety | 4/10 | Many artifact writes are deterministic paths without locking/idempotent write protocol. |
| Resource lifecycle | 6/10 | Embedder/MDConverter close patterns are good; scripts and workflows vary. |
| Queue/worker compatibility | 5/10 | Queue concepts exist, but strict onion violations and shared artifact paths should be fixed before scaling workers. |

## Recommendations

1. **P1: Define a concurrency budget contract for analysis.** Bound `run_analysis_async` scoring/extraction fanout with a semaphore or explicit executor and test at a representative batch size.
2. **P1: Establish artifact write safety.** Use run-scoped directories or atomic write conventions for session/day/queue artifacts before enabling parallel runs.
3. **P1: Move shared DTO/contracts inward.** This is both architecture and concurrency work because worker boundaries need stable inner contracts.
4. **P2: Document object ownership.** State whether `Embedder`, converters, queues, and adapters are per-task, per-run, per-thread, or shared; enforce with tests.
5. **P2: Add cancellation/timeout tests.** Especially for async HTTP clients, thread pools, and queue workers.
6. **P3: Keep script thread pools bounded and isolated.** Do not promote script concurrency patterns directly into package code without lifecycle and rate-limit contracts.

## Bottom line

The code is suitable for controlled async entrypoints today. It is **not yet strict-ready for broad multithreaded or multi-worker execution** without additional contracts around bounded concurrency, artifact isolation, and DTO ownership.
