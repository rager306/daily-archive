# Onion / Hexagonal Layer Mapping (D086)

> **Status:** living document, companion to `doc/adr-v2.md` §3 and decision D086.
> Defines which `research_graph` package belongs to which onion layer, and the
> import-direction rule that keeps the layers honest.

## The rule

Onion dependency direction points **inward**:

```
domain  ←  application  ←  infrastructure
```

- **`domain`** depends only on the stdlib and on its own typed models. It NEVER
  imports `application` or `infrastructure`. This is the invariant the
  [`verify_onion_layering.py`](../scripts/verify_onion_layering.py) guard enforces.
- **`application`** depends on `domain` (cross-cutting Ports, schema), never on
  `infrastructure` directly. Application may declare use-case-local Protocol
  ports beside the use case when the seam is not yet cross-cutting.
- **`infrastructure`** may import `application` + `domain`; it implements domain
  Ports and application use-case Ports as concrete Adapters.

A reverse import (e.g. `domain/ports.py` doing `from research_graph.infrastructure
...`) is a layering violation and fails the guard.

## Package → layer mapping

> The onion layers are now **physical packages** (D086, M104 S03). Pure typed
> models moved into `domain/`; the pipeline moved into `application/`;
> `evaluation/` and `papers/indexing/` keep thin back-compat shims that
> re-export from the canonical domain home.

| Layer | Packages (physical) | Role |
|---|---|---|
| **domain (Core)** | `domain/` (`ports`, `schema`, `relation_types`, `statistical_context`, `extraction_signatures`, `semantic_chunks`, `navigation`) | Typed models + cross-cutting Port interfaces. Pure: no I/O, no drivers, no SDKs. AST-guarded. |
| **application** | `application/` (`types`, `primitives`, `orchestrator`, `profiles`, `corpus`, `graph`) | Use cases: the typed extraction pipeline plus local use-case Protocol ports. Imports only domain + stdlib (+ Adaptix at the LLM boundary). Infrastructure reaches it only by implementing injected Ports/callables. |
| **infrastructure** | `graph/` (ladybug_client), `corpus/` (sources, ingestion, parsing), `llm/`, `retrieval/`, `identity/`, `papers/` (indexing/chunking logic), `quality/`, `repair/`, `staging/`, `ops/`, `infrastructure/` (Adapters: `ladybug_adapter`, `md_converter_adapter`) | Concrete Adapters + driver-bearing code. Implement domain Ports and application use-case Ports. May contain infra-local Protocols for adapter internals only. |
| **entry / wiring** | `cli/`, `workflows/` (universal_kb, rlm, validation), `scripts/` (prototype), `application/profiles` (composition root) | Composition roots & runtime entry points. The ONE place Adapters/infra callables are injected into the application via Ports. |

### Strict boundary classification policy

M164 makes the outer-layer rules explicit while existing debt is ratcheted down:

* Reusable contracts imported by infrastructure must live inward in `domain/` or `application/`; `workflows/` is orchestration, not a contract home.
* CLI DTOs must not be imported by infrastructure. Shared analysis result shapes belong in `application/`; `cli/` remains an entrypoint.
* Scripts are process-boundary wrappers. Reusable logic belongs in package modules, and scripts delegate to it.
* Existing violations may be temporarily reported as bounded guard debt, but the guard must expose the count and exact imports.

### Back-compat shims (canonical home moved, not duplicated)

* `evaluation/__init__.py` re-exports `schema`/`relation_types`/`statistical_context`/`extraction_signatures` canonical types now in `domain/`.
* `papers/semantic_chunks.py` keeps `build_*` logic and re-exports `EvidencePath`/`SemanticChunk` from `domain/`.
* `papers/indexing/navigation.py` re-exports `PageIndexDocument`/`PageIndexNode`/`NavigationAnchor` from `domain/`.

New code imports cross-cutting models and Ports from the domain; the shims keep the 37+ legacy import sites green.

### Port taxonomy

ADR-034 uses "Port" in three explicit scopes:

1. **Domain cross-cutting Ports** live in `domain/ports.py`. Examples: `LLMClientPort`, `GraphDBPort`, and `FullTextProviderPort`.
2. **Application use-case Ports** live beside a use case in `application/**` when the seam is local to that use case. Examples: catalog ingest repository/source ports, parser replay ports, graph probe execution port, and `DispatchProtocol`.
3. **Infrastructure-local Protocols** live in `infrastructure/**` only for adapter internals such as transports or event loggers. They are not imported inward.

This keeps local seams local until there is evidence that a seam should become a cross-cutting domain Port. The Ponytail Port rule still applies: no Protocol for symmetry, no interface without a real seam.

### Why `evaluation/` is domain, not application

`evaluation/schema.py` defines the typed knowledge models (`TypedEntity`,
`TypedRelation`, `ExtractionPatch`, ...) that both the application pipeline and
the domain Ports reference. They are pure data contracts with fail-closed
`safety_flags` — no behaviour, no I/O. That makes them Core.

### Why `pipeline/` is application, not domain

`pipeline/` orchestrates stages and threads a `PipelineContext`. It depends on
`evaluation/schema` (domain) and on the Port-style `llm_client` callable, but
it does not own the typed contracts. It is a use case.

## Composition root

`pipeline/profiles/paper.py::build_wired_paper_pipeline(llm_provider=...)` is the
single wiring point for the paper extraction use case: it takes an
`LLMClientPort` (a domain Port) and injects it into the application pipeline's
LLM stages. Infrastructure code (the prototype script, future CLI) calls this
function with a concrete adapter (`MDConverterAdapter`, a MiniMax client, ...).

### Async-first entry point policy

Async hosts call async APIs directly: `run_analysis_async`, `run_pipeline_async`,
and `run_command_async`. Sync wrappers such as `run_analysis`, `run_pipeline`,
and `acquire_sources_for_manifest_sync` are compatibility surfaces for process
boundaries and synchronous scripts only. They must fail explicitly inside an
active event loop and point the caller to the async API instead of nesting event
loop runners.

### Artifact write-safety policy

Future multi-worker code must treat shared artifact paths as contested state:

* Small JSON/text state files use write-to-temp plus atomic `Path.replace()`.
* Larger run artifacts prefer run-scoped directories over shared filenames.
* Cross-process append logs or mutable databases need an explicit lock or a single-writer owner.
* Callers must not treat this as fully migrated until each write path is converted or explicitly scoped.

### Adapter ownership and lifecycle policy

Async adapters are owned by the run/task that creates them unless a module says otherwise:

* Do not share mutable adapter instances across threads or worker processes by default.
* Injected clients are caller-owned; adapter `close()` methods close only resources they created.
* Sync wrappers are process-boundary compatibility surfaces only and must fail inside active event loops.
* If future code needs shared adapters, it must add an explicit ownership, locking, and close-order contract with tests.

```
infrastructure (concrete adapter) ──calls──> build_wired_paper_pipeline(llm_provider=Adapter)
                                                      │ injects Port into
                                                      ▼
                                  application (pipeline stages depend on Port, not adapter)
                                                      │ use
                                                      ▼
                                            domain (Ports + schema)
```

## What the guard enforces

`scripts/verify_onion_layering.py` AST-scans `src/research_graph/domain/` and
`src/research_graph/application/` and fails on forbidden inward imports:

- domain must not import `research_graph.application`, infrastructure packages,
  workflows, cli, or local `scripts`.
- application must not import `research_graph.infrastructure`, workflows, cli, or
  local `scripts`.

Allowed domain imports: stdlib and `research_graph.domain.*` itself. Application
may import stdlib, domain, and its own application modules.

`evaluation/` and `papers/` are permitted in the domain because they hold the
typed models the Core references; if a future change makes them carry driver
code, they must be split (pure model stays in domain, driver moves to
infrastructure) before this mapping is updated.

## M105 S06 — evaluation package dissolved (2026-06-22)

The `evaluation/` package was classified and dissolved across onion layers:

- **domain/**: `extraction_signatures.py` (typed signatures)
- **application/**: `extraction_benchmark.py` (benchmark orchestrator)
- **infrastructure/evaluation/**: `scoring.py` (arxiv/semantic_scholar adapter), `analytics.py` (ladybug aggregation), `evaluation_metrics.py` (uses infrastructure.graph/retrieval), `scientific_extraction.py` (uses infrastructure.corpus.parsing), `dspy_extraction.py` (orchestrates LLM infra)

The `evaluation/` package marker (`__init__.py`) is preserved for historical reference (S01 lesson) with a docstring describing the dissolution. All callers updated to canonical paths. Onion guard confirmed clean (domain=8, application=7 files, 0 forbidden imports).

## M105 — Infrastructure packages physically moved to infrastructure/ (2026-06-22)

All 10 infrastructure packages moved into `research_graph/infrastructure/` over 4 waves:
- **W1**: llm, identity, quality (leaves)
- **W2**: ops, staging (depend on W1 leaves)
- **W3**: corpus, papers, repair (coordinated, cycle)
- **W4**: graph, retrieval (depend on cycle), graph merged with ladybug_adapter

All manifests (wave-03..wave-18) updated. Multi-layer AST guard enforces domain/application cannot import infrastructure.
