# daily-archive ADR Memory — M034 Universal Knowledge Base Decision Package

Source documents: `doc/adr/m034/` and supporting M034 docs under `doc/architecture/m034-universal-kb/`, `doc/contracts/m034-universal-kb/`, `doc/product/`, `doc/requirements/m034-universal-kb/`, and `doc/validation/m034-universal-kb/`.

## PURPOSE

daily-archive is a local-first universal knowledge base. Scientific papers/articles are the primary first domain and proving ground, but the architecture must not overfit to arXiv/PDF/paper-only assumptions.

The system should build durable, traceable evidence chains before graph promotion. Parser, sidecar, adapter, and LLM outputs are candidate evidence only until deterministic validation and review boundaries mark them eligible. Graph import and production writes remain explicitly unauthorized unless a future milestone changes that with evidence.

Binding safety defaults from M034:

```text
graph_import_allowed=false
graphdb_written=false
ladybugdb_written=false
production_import_attempted=false
import_eligible=false
```

## STACK

Current project stack is Python-first with local fixtures, verifiers, and generated artifacts. Scientific KG work currently uses deterministic tests, local artifacts, explicit review/readiness gates, and no-write boundaries.

Knowledge substrate choice is intentionally deferred. LadybugDB, FalkorDB, HelixDB, and other candidates remain open for comparison. LadybugDB must not be treated as the final production GraphDB until a dedicated evaluation milestone compares license, locality, graph-vector needs, performance, portability, operational burden, and safety boundary integration.

External parser/sidecar candidates from M033 are not production dependencies:

- GROBID: scholarly sidecar candidate for TEI, metadata, references, citations.
- OpenDataLoader PDF: hybrid sidecar candidate for layout/OCR/tables/coordinates with backend/cache lifecycle concerns.
- Adaptix: adapter candidate for typed structural adaptation, not semantic validation.
- quant-mind: pattern source only, not runtime dependency.

## ARCHITECTURE

Accepted M034 ADRs:

- ADR-000: Universal KB north star. The core architecture is a local-first universal KB; scientific articles are first domain adapters, not the whole product.
- ADR-003: Durable lazy async evidence pipeline. Build durable job/artifact state, lazy recompute, retry/resume, and observable status before scale or agents.
- ADR-004: Sidecars as candidate evidence producers. Sidecars produce evidence packets, never graph truth.
- ADR-005: No direct extractor/parser/sidecar/LLM to GraphDB path. Promotion must pass through validation, review, readiness, and explicit authorization.
- ADR-006: Agent boundary. Agents are optional future bounded helpers/reviewers/summarizers/triage workers, not current core orchestrators or graph-promotion authorities.
- ADR-007: quant-mind pattern source, not runtime dependency.

Deferred ADR:

- ADR-002: Final GraphDB selection remains deferred pending comparison of LadybugDB, FalkorDB, HelixDB, and other candidates.

Core architecture contracts from M034:

- Separate generic universal-KB primitives from paper-domain adapters.
- Persist state for jobs, artifacts, evidence, review packets, and readiness handoff.
- Model artifact dependencies so stale downstream outputs can be detected lazily.
- Use explicit status transitions and failure taxonomy.
- Keep graph-readiness handoff no-write unless future authorization changes safety flags.
- Introduce KnowledgeSubstratePort or equivalent abstraction before binding to any GraphDB.

## PATTERNS

M034 establishes these durable patterns:

1. Audit first: before drafting new architecture ADRs, audit existing Rxxx/Dxxx for conflicts, historical scope, clarification needs, and supersession routes.
2. Decision package as artifact set: ADRs, PRD, requirements, contracts, invariants, status matrix, failure taxonomy, dependency model, roadmap gates, open questions, and verifier must travel together.
3. Mermaid-assisted ADRs: prose and tables are authoritative; Mermaid is optional, bounded, and only for readability around context maps, safety gates, status transitions, option comparisons, and contract relationships. Every ADR should include LLM Reading Notes.
4. Fail-closed sidecars: sidecar/parser success creates candidate evidence, not import eligibility.
5. Verifier-first docs: every decision package should have a one-command verifier and explicit reader/UAT checks.
6. Open questions are not accepted decisions and do not authorize implementation.

## TRADEOFFS

M034 chose documentation hardening and decision clarity before implementation. This delays coding the durable sidecar pipeline but prevents architecture drift, paper-only overfitting, accidental GraphDB lock-in, parser-as-truth shortcuts, and premature agent orchestration.

GraphDB deferral preserves optionality but requires a future comparison gate before graph substrate work. The no-direct-write rule adds ceremony but protects against unsafe import paths and keeps evidence/review/provenance inspectable.

Agents are deferred as orchestrators because deterministic queue/status/tool-chain contracts are not yet in place. They may become bounded helpers only after contracts, tools, observability, and failure modes exist.

## PHILOSOPHY

Prefer local-first, inspectable, deterministic, fail-closed architecture. Evidence must be traceable before it becomes knowledge. Scientific papers are the first domain because they stress citations, provenance, extraction quality, and review boundaries, but the system should generalize to other knowledge domains.

Do not infer authorization from successful extraction, non-empty parser output, passing tests, or available GraphDB tooling. Authorization must be explicit, documented, and verified.

Recommended next milestone from M034: Durable Evidence Pipeline Prototype Planning. Start with state model, queue semantics, artifact dependency/stale detection, no-write sidecar worker simulation, retry/resume/failure verification, and review packet/readiness handoff in no-write mode.