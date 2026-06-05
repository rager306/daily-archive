# Project: daily-archive

## What This Is

daily-archive is a local-first arXiv research ingestion and scientific knowledge-graph project. It builds deterministic paper-processing evidence chains before any production graph import is allowed: catalog records, source acquisition, loader evidence, parser/conversion diagnostics, chunks, graph-readiness reviewer packets, and fail-closed no-write import boundaries.

## Core Value

A future agent should be able to ingest scientific papers locally, inspect durable artifacts, compare parser/chunker quality, and only advance toward Scientific KG / LadybugDB import when evidence is traceable, reviewed, and explicitly authorized.

## Project Shape

- **Complexity:** complex
- **Why:** The project spans local artifact persistence, source acquisition, parser/conversion quality, evidence paths, review packets, graph-readiness gates, and LadybugDB safety boundaries.

## Current State

- M031-vwpd8e is complete: **Catalog Backed Replay to Graph Readiness Refusal Boundary**.
- M031 established a bounded selected-ref chain: catalog/intake → acquisition → loader → parser/conversion → chunk/evidence → graph-readiness handoff → no-write import refusal.
- M031 validation round 1 is `pass` after S07 resolved round 0 needs-attention at the evaluation layer.
- S07 documented that the stale S02 direct `FAIL` assessment is historical artifact conflict superseded by S06/S07 scoped evidence for M031 validation.
- R024, R027, R029, and R050 remain active globally; M031 advanced them only within refusal-boundary scope and did not claim positive graph/import readiness.

## Current Research Direction

The next project question is whether article parsing/conversion quality and throughput can be improved by studying external PDF-processing and paper-knowledge frameworks:

- `opendataloader-pdf` from `https://github.com/opendataloader-project/opendataloader-pdf`
- GROBID from `https://github.com/kermitt2/grobid`
- `quant-mind` from `https://github.com/LLMQuant/quant-mind`

The core research goals are:

1. Compare opendataloader-pdf and GROBID with daily-archive's current parser/conversion/refusal pipeline.
2. Test the claim: for scientific-article RAG contexts, teams often combine GROBID for deep scholarly parsing and bibliography with OpenDataLoader-style tooling for layout and tables.
3. Assess whether that combination is feasible here and how much complexity it adds.
4. Evaluate processed article quality using current daily-archive outputs, opendataloader-pdf outputs, and GROBID outputs under a bounded, artifact-backed protocol.
5. Extract architecture patterns from quant-mind without adopting it wholesale: TreeKnowledge, PaperKnowledgeCard, provenance schemas, fetch/format separation, bounded batch flows, magic input resolution, and clear separation between realized code and aspirational README claims.

This is research/probe work only: no production graph import, no LadybugDB writes, no positive graph-readiness claims, and no adoption of external frameworks before compatibility, quality, and safety boundaries are proven.

## Safety Boundaries

- Do not treat parsed text existence as graph readiness.
- Do not infer quality from non-empty output alone; low-quality source diagnostics remain necessary.
- External parser probes must remain local, bounded, reproducible, and artifact-backed.
- Positive KG import remains blocked until independent completed review and an in-scope import authorization milestone.
- quant-mind should be treated as an architecture-pattern source and experimental reference, not as a production RAG/Knowledge Base dependency.

## Capability Contract

See `.gsd/REQUIREMENTS.md` for the explicit capability contract, active/validated status, and requirement coverage mapping.

## Milestone Sequence

- [x] M001: Cron-safe arXiv article analysis for Hermes — Stable daily CLI, JSON artifacts, queue state, and cron-safe behavior.
- [x] M002: LadybugDB Graph-Vector Scientific KG Foundation — Embedded graph-vector storage, embeddings, graph analytics, and paper-level hybrid recommendations.
- [x] M031-vwpd8e: Catalog Backed Replay to Graph Readiness Refusal Boundary — Bounded catalog-backed replay chain through no-write graph import refusal with validation round 1 pass.
- [ ] M032: External Parser and Paper-Knowledge Architecture Research — Vendor and index opendataloader-pdf, GROBID, and quant-mind; compare parser/output contracts, evaluate combination complexity, and identify reusable architecture patterns.

## Active Next Step

Use GitNexus to study `opendataloader-pdf`, GROBID, `quant-mind`, and `daily-archive` side by side. Produce a research artifact that answers: what each project actually implements, how it compares to current daily-archive boundaries, whether GROBID + OpenDataLoader-style combination is practical, what quality evaluation should be run, and which quant-mind architecture patterns are worth adapting.

## Important Artifacts

- M031 validation: `.gsd/milestones/M031-vwpd8e/M031-vwpd8e-VALIDATION.md`
- M031 summary: `.gsd/milestones/M031-vwpd8e/M031-vwpd8e-SUMMARY.md`
- S07 assessment reconciliation: `.gsd/milestones/M031-vwpd8e/slices/S07/S07-ASSESSMENT.md`
- Requirements contract: `.gsd/REQUIREMENTS.md`
- Vendor source roots: `/root/vendor-source/opendataloader-pdf`, `/root/vendor-source/grobid`, `/root/vendor-source/quant-mind`
