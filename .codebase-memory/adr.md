# daily-archive ADR Memory — M034 Universal Knowledge Base Decision Package

Source documents: `doc/adr/m034/` and supporting M034 docs under `doc/architecture/m034-universal-kb/`, `doc/contracts/m034-universal-kb/`, `doc/product/`, `doc/requirements/m034-universal-kb/`, and `doc/validation/m034-universal-kb/`.

## Purpose

`daily-archive` is a local-first universal knowledge base. Scientific papers and arXiv-style article workflows are the primary first domain and proving ground, but the architecture must not overfit to arXiv, PDF, scientific-paper-only, RAG-only, or direct parser-to-GraphDB assumptions.

The current runnable product remains the research-paper daily archive pipeline. That runtime uses arXiv, Semantic Scholar, MiniMax, Telegram, arxiv2md, Marker/PyMuPDF, and local session artifacts as first-domain/domain-specific surfaces. These integrations are not the north-star architecture and do not authorize graph promotion.

The system should build durable, traceable evidence chains before graph promotion. Parser, sidecar, adapter, and LLM outputs are candidate evidence only until deterministic validation and review boundaries mark them eligible. Graph import and production writes remain explicitly unauthorized unless a future milestone changes that with evidence.

## Binding Safety Defaults

```text
graph_import_allowed=false
graphdb_written=false
ladybugdb_written=false
production_import_attempted=false
import_eligible=false
```

The authoritative safety baseline is `doc/contracts/m034-universal-kb/SAFETY-INVARIANTS.md`.

## Non-Authorization Rules

- Parser, sidecar, adapter, and LLM outputs are candidate evidence only.
- No direct extractor/parser/sidecar/LLM to GraphDB write path is allowed.
- No direct extractor/parser/sidecar/LLM to LadybugDB, FalkorDB, HelixDB, or any other GraphDB is allowed.
- GraphDB selection remains deferred until a dedicated comparison milestone or ADR closes the evidence requirements.
- Agentic orchestration remains deferred until deterministic contracts, durable queues, review gates, and safe traces exist.
- No review packet means no readiness handoff.
- No readiness handoff means no import recommendation.
- No explicit future graph-promotion milestone means no GraphDB write.

## ADR Decisions

- ADR-000: Universal KB North Star. daily-archive is a local-first Universal KB; scientific articles are the first domain and proving ground.
- ADR-002: Defer Final GraphDB Selection. LadybugDB, FalkorDB, HelixDB, and other candidates remain open until a comparison evaluates license, locality, graph/vector needs, performance, portability, operational burden, export/recovery, and safety-boundary integration.
- ADR-003: Durable Lazy Async Evidence Pipeline. Build persistent jobs, dependency/stale detection, retry/resume, typed failures, and artifact traces before scaling sidecar processing.
- ADR-004: Sidecars as Candidate Evidence Producers. GROBID, OpenDataLoader, Adaptix, and future extractors produce candidates, not semantic truth, graph readiness, or import eligibility.
- ADR-005: No direct extractor/parser/sidecar/LLM to GraphDB path. Promotion must pass through candidate, validation, review, readiness, and explicit authorization boundaries.
- ADR-006: Agent Boundary. LLM/agent helpers may provide bounded diagnostics later, but are not current orchestrators, reviewers with approval authority, or graph-promotion authorities.
- ADR-007: Quant-mind Pattern Source Not Runtime Dependency. Use quant-mind for architecture patterns only; do not adopt its paper_flow, OpenAI Agents runtime, GraphKnowledge, or in-memory batch model as production dependency now.

## M035 Executable Prototype Closure

M035 turns the M034 safety package into executable local contracts and a metadata-only rehearsal:

1. README, ADR support docs, and this memory summary now frame `daily-archive` as a local-first Universal KB with scientific articles as the first domain.
2. Core contracts live in `src/arxiv_archive/universal_kb_contracts.py` as frozen stdlib dataclasses with fail-closed `SafetyFlags`.
3. The durable prototype queue lives in `src/arxiv_archive/universal_kb_queue.py` as a local SQLite/WAL state machine with dependency gates, leases, heartbeats, retry/stale handling, and `job_events`.
4. Adaptix remains an anti-corruption boundary in `src/arxiv_archive/universal_kb_sidecar_boundary.py`; external sidecar JSON cannot widen authority.
5. Structured LLM help lives in `src/arxiv_archive/universal_kb_review_assistance.py` as diagnostics-only packets and sanitized `ToolInvocationRecord` traces.
6. No-write substrate rehearsal and ADR-005 static guards live in `src/arxiv_archive/universal_kb_substrate_rehearsal.py` and `tests/test_universal_kb_architecture_guards.py`.
7. The integrated metadata-only rehearsal lives in `src/arxiv_archive/universal_kb_rehearsal.py` and writes local inspection artifacts under `artifacts/m035-universal-kb-prototype/rehearsal/` when verified.

Run the current proof with:

```bash
python3 scripts/verify_m035_universal_kb_prototype.py
```

The verifier runs stable M034 ADR package checks, M035 Universal KB tests, ruff, and artifact inspection. The expected artifact proof is `graph_write_allowed=false`, `promotion_allowed=false`, and `production_import_attempted=false`.

M035 does not select a final GraphDB, authorize production graph import, authorize parser output as truth, or introduce agentic orchestration. MiniMax-M3-512k is helper/tool metadata only for Anthropic-compatible paths; it is not source-of-truth authority.

## M036/M037 Real-Corpus No-Write Smoke Command Surface

M036 starts the transition from fixture-only rehearsal to existing local real article artifacts. M037 consolidates that workflow behind one module command surface:

```bash
uv run python -m arxiv_archive.universal_kb_smoke all --limit 5 --profile fast
uv run python -m arxiv_archive.universal_kb_smoke verify --profile full
```

The legacy verifier remains available as a compatibility wrapper:

```bash
python3 scripts/verify_m036_real_corpus_no_write_smoke.py
```

The verifier selects 5 local article records, runs them through candidate, queue, diagnostic review, helper trace, and readiness handoff steps, and writes continuity artifacts under `artifacts/m036-real-corpus-no-write-smoke/`.

Expected safety result remains `graph_write_allowed=false`, `promotion_allowed=false`, `production_import_attempted=false`, and `import_eligible=false`. Current continuity blockers are diagnostic only: legacy or missing article safety flag shape and missing loader evidence for one selected record. These blockers must prevent GraphDB/import/promotion claims until resolved or explicitly addressed by a future ADR/milestone. M037 intentionally does not expand beyond 5 articles; 10-30 article expansion waits for the next milestone after this control surface is consolidated.

## Open Questions and Closure Evidence

Open questions are tracked in `doc/architecture/m034-universal-kb/OPEN-QUESTIONS.md`. They are not accepted decisions and must not be treated as authorization.

Current closure routes:

- GraphDB choice: requires comparison matrix over candidate graph stores and safety-boundary integration evidence.
- Durable state: M035 prototypes SQLite first with persisted job state, resume/retry, leases or equivalent claim semantics, stale detection, typed failure records, and deterministic tests.
- Worker shape: requires no-write sidecar worker simulation evidence before generic versus per-sidecar commitment.
- First non-paper domain: remains future planning after paper-domain stabilization.
- LLM/agent helper entry: requires structured-output helper contracts, redaction rules, ToolInvocationRecord traces, and tests proving no helper approval or graph import authority.
- Review approval: requires explicit review state machine and proof that no review packet means no readiness handoff and no import recommendation.

## LLM Reading Notes

- Binding decision: treat Universal KB and fail-closed evidence chains as the north-star architecture.
- Do not infer: final GraphDB choice, graph write authorization, parser-as-truth, LLM approval authority, or agentic orchestration.
- Safe next action: implement no-write executable contracts, queue state, boundary adapters, review diagnostics, guard tests, and metadata-only rehearsal.
- Blocked until future ADR/milestone: production graph import, final GraphDB selection, accepted semantic knowledge promotion, and autonomous agent orchestration.
