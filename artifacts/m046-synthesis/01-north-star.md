# 01 — North Star and Idea Synthesis

> **Source ADRs:** [ADR-000](../doc/adr/m034/ADR-000-universal-kb-north-star.md) (Universal KB North Star, binding)
> **Source milestones:** M033 (research), M034 (ADR package), M035-M045 (delivery)
> **Synthesis layer:** 1 of 7

## 0. One-line Decision (from ADR-000)

> We will frame daily-archive as a **local-first universal knowledge base** built from durable, traceable evidence chains, with **scientific articles as the primary first domain and proving ground**.
> We will not frame the project as only a PDF parser, only a scientific-paper KG, only a RAG app, or a direct parser-to-GraphDB pipeline.

## 1. The Idea

daily-archive began as a cron-safe arXiv article analysis CLI (M001). It accumulated two layered capabilities:

1. **Local deterministic ingestion** — catalog records, source acquisition, loader evidence, parser/conversion diagnostics, chunks, evidence packages, review packets, no-write import boundaries.
2. **Architecture decision-making** — explicit ADRs (M034), evidence pipeline prototypes (M035), no-write smoke on real corpus (M036), control surface (M037), governance memory (M038-M039), connectivity validation (M040-M042), sidecar probes (M043-M044), trajectory checking (M045).

The synthesis is that these two layers describe **one project** with a clean separation:

- **generic evidence primitives** that could one day power non-paper domains;
- **paper-domain adapters** that prove the primitives against the hardest current case (citations, figures, tables, equations, sections, source spans, review burden).

Future agents must be able to ingest scientific papers locally, inspect durable artifacts, compare parser/chunker quality, and only advance toward Scientific KG / GraphDB import when evidence is traceable, reviewed, and explicitly authorized.

## 2. Primary First Domain — Scientific Articles

Why scientific articles first:

- They stress the system with citations, references, figures, tables, equations, sections, source spans, and review burden.
- Existing parsers (GROBID, OpenDataLoader, Adaptix) are most mature in this domain.
- quant-mind, the pattern source, focuses on scientific paper knowledge structures.
- Historical work in this project (M001-M032) already established the loader, conversion, and graph-readiness boundaries on arXiv data.

Non-paper domains are **deliberately deferred** to a future decision point (ADR-000, Open Question #3). They are not blocked, only sequenced behind paper-domain validation.

## 3. Generic Knowledge-Base Primitives (from ADR-000)

| Primitive | Purpose |
|---|---|
| `KnowledgeSourceRecord` | any source of knowledge, paper or otherwise |
| `DomainAdapterRecord` | adapter from generic to domain-specific |
| `EvidenceArtifactRecord` | durable output of a processing step |
| `ProcessingJob` | a unit of work tracked in the durable queue |
| `DependencyRecord` | explicit dependency between jobs/artifacts |
| `FailureRecord` | typed failure with diagnostic code |
| `CandidatePacket` | candidate evidence awaiting review |
| `ReviewPacket` | review bundle of one or more candidate packets |
| `GraphReadinessHandoff` | handoff to (future) graph-readiness review |
| `KnowledgeSubstratePort` | substrate-agnostic interface (LadybugDB / FalkorDB / HelixDB / other) |
| `SafetyFlags` | safety defaults (always false until explicitly authorized) |

## 4. Paper-Domain Specializations (from ADR-000)

- `ArticleRecord`
- `PaperSourceRecord`
- `ArticleJob`
- `SidecarJob`
- GROBID sidecar output contract (TEI summary, hash, element counts only)
- OpenDataLoader sidecar output contract (JSON, layout/OCR/table/coordinates)
- Adaptix typed adapter output contract (mapped to review-only candidate summaries)
- `PaperCandidatePacket`
- `PaperReviewPacket`

These specializations **must not bypass the generic primitives**. Any future addition (multimodal, code, web pages) follows the same generic-first / domain-adapter pattern.

## 5. Value for Future Agents

A new agent opening this project for the first time should be able to:

- **Ingest** scientific papers locally (catalog, source, loader).
- **Inspect** durable evidence artifacts under `artifacts/m*/`.
- **Compare** parser / chunker quality via sidecar probes (`scripts/probe_m043_*`).
- **Trace** every decision back to an ADR or a GSD decision.
- **Verify** safety boundaries via the trajectory check (`scripts/check_project_trajectory.py`).
- **Plan** the next milestone with a clear, artifact-backed picture of current state.

The value is **trajectory preservation, not feature velocity**. A wrong graph import is more expensive than a slow one.

## 6. Capability Contract Mapping (R001–R065 → layers)

This table maps each active or validated requirement to the synthesis layer where it lives. Used for traceability and gap detection.

| Requirement | Class | Status | Synthesis Layer | Anchor |
|---|---|---|---|---|
| R001-R013 | core (cron CLI) | validated | 04 (module map) | M001 |
| R014 | core (full-text ingestion) | validated | 04 | M003 S01 |
| R015-R018 | core (PageIndex, SemanticChunk, SCI KG schema) | validated | 04 | M003 |
| R019 | core (hybrid retrieval) | active | 05 (evidence layer) | M003 S06 |
| R020-R021 | quality (eval fixtures, DSPy gates) | validated | 07 (assessment) | M003 S07 |
| R022 | core (RLM doc/workflow) | active | 05 | M003 S09 |
| R023 | differentiator (RLM benchmark) | active | 07 | M003 S10 |
| R024 | quality (staged KG validation) | active | 06 (next gate) | post-M003 |
| R025 | operability (Loguru logs) | validated | 04 | M003 |
| R026 | quality (real-data pipeline debug) | validated | 05 | M003 |
| R027 | quality (graph-readiness contract) | active | 05 | M004 |
| R028 | quality (independent review) | validated | 05 | M005 |
| R029 | quality (typed chunk package) | active | 05 | M005 |
| R030 | quality (preserve source artifacts) | validated | 04 | M005 |
| R031 | quality (30-paper deviation scan) | active | 04 | M006 |
| R032 | operability (100-paper loop) | active | 06 | M006 |
| R033 | operability (deterministic +10 CLI) | active | 04 | M007 |
| R034 | operability (first new +10 batch) | validated | 04 | M008 |
| R035 | operability (deterministic replacement) | active | 04 | M008 |
| R036 | operability (replay/audit provenance) | active | 04 | M009 |
| R037 | operability (next reviewed +10) | active | 04 | M010 |
| R038 | failure-visibility (semantic evidence gate) | active | 05 | M011 |
| R039 | operability (DSPy/MiniMax compat) | validated | 04 | M012 |
| R040 | operability (new infra safety) | active | 05 | M012 |
| R041 | operability (optimizer applicability) | validated | 05 | M013 |
| R042-R045 | operability (MiniMax Token Plan) | validated | 04 | M014-M017 |
| R046 | compliance/security (ML vuln triage) | validated | 05 | M018 |
| R047 | differentiator (open research agent compare) | validated | 04 | M019 |
| R048-R050 | core (candidate locators, article CLI) | active | 04 | M020-M021 |
| R051-R052 | operability (MiniMax helper, DSPy gate) | validated | 05 | M023 |
| R053 | quality (external PDF tool compare) | validated | 04 | M033 |
| R054 | core (durable lazy sidecar pipeline) | validated | 04 | M035 |
| R055 | failure-visibility (sidecar lifecycle) | validated | 05 | M035 |
| R056 | core (sidecar candidate-only) | validated | 05 | M035 |
| R057 | quality (architecture gates) | validated | 02 (architecture) | M034 |
| R058 | core (local-first mission) | validated | 01 (north star) | M034 |
| R059 | quality (GraphDB selection defer) | validated | 02 | M034 |
| R060 | core (universal KB frame) | validated | 01 | M034 |
| R061 | quality (R/D consistency audit) | validated | 03 (decisions) | M034 |
| R062 | operability (governance memory mirror) | validated | 06 (trajectory) | M038 |
| R063 | operability (typed graph projection) | validated | 06 | M039 |
| R064 | operability (mixed 20-30 article) | validated | 04 | M041 |
| R065 | quality (unified trajectory check) | validated | 06 | M045 |

**Active requirements count:** 17. **Validated:** 48. **Total:** 65.

## 7. Safety Defaults (5× false)

These defaults are **always false** unless a future explicit graph promotion / import milestone authorizes otherwise.

```text
graph_import_allowed=false
graphdb_written=false
ladybugdb_written=false
production_import_attempted=false
import_eligible=false
```

These five flags are the single safety contract. Every artifact in the synthesis package must respect them.

## 8. Non-Authorization (binding, from ADR-000)

This synthesis package does **not** authorize:

- production graph import;
- final GraphDB selection;
- LadybugDB / FalkorDB / HelixDB writes;
- parser output as graph-ready truth;
- agentic orchestration;
- bypassing validators, review packets, or the trajectory check.

## 9. LLM Reading Notes (binding)

- **Treat this layer as north star, not as policy.** Policy is in `doc/adr/m034/`. North star is the framing for future work.
- **Do not infer** final GraphDB selection, LadybugDB production adoption, or parser output as truth from any layer of this package.
- **Safe next action** is to read `02-architecture-layers.md` to understand how the system is structured, then `04-module-map.md` to find the actual code.
- **Blocked** until a future graph promotion / import milestone is explicitly accepted, with all five safety defaults flipped to true via that milestone.

## 10. Cross-References

- Architecture: `02-architecture-layers.md`
- Decisions: `03-adr-decisions.md`
- Modules: `04-module-map.md`
- Evidence/Safety: `05-evidence-safety.md`
- Trajectory/Ops: `06-trajectory-ops.md`
- Assessment: `07-2026-assessment.md`
