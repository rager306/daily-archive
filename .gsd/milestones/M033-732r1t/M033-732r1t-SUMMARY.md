---
id: M033-732r1t
title: "External Parser and Paper Knowledge Architecture Research"
status: complete
completed_at: 2026-06-05T12:00:42.483Z
key_decisions:
  - GROBID is a scholarly TEI/bibliography/citation sidecar candidate, not a standalone graph-ready parser or OCR/table replacement.
  - OpenDataLoader hybrid docling-fast is a layout/OCR/table/coordinate sidecar candidate, with OCR/table fidelity requiring future quality gates.
  - Adaptix is suitable as a post-processing typed adapter layer over fixed parser JSON, not semantic validation or graph readiness proof.
  - quant-mind is a pattern source only, not a production runtime dependency or OpenAI/API extraction proof.
  - The recommended architecture is a bounded combined sidecar architecture with daily-archive retaining validation, review, graph-readiness, and import ownership.
  - M033 authorizes no production parser integration, dependency adoption, graph import, LadybugDB write, or import eligibility.
key_files:
  - data/article_corpora/m033-current-parser-baseline-v1/
  - data/article_corpora/m033-grobid-probe-v1/
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/
  - data/article_corpora/m033-quantmind-pattern-study-v1/
  - data/article_corpora/m033-combined-parser-architecture-v1/
  - data/article_corpora/m033-external-parser-quality-plan-v1/
  - scripts/probe_m033_opendataloader_adaptix_adapter.py
  - scripts/verify_m033_grobid_probe.py
  - scripts/verify_m033_opendataloader_adaptix_adapter.py
  - scripts/verify_m033_quantmind_pattern_study.py
  - scripts/verify_m033_combined_parser_architecture.py
  - scripts/verify_m033_external_parser_quality_plan.py
  - tests/test_m033_opendataloader_adaptix_adapter.py
lessons_learned:
  - External parser success must stay separate from graph readiness and import eligibility.
  - Hybrid backend/model cache lifecycle is part of parser quality, not just setup detail.
  - A combined sidecar architecture is more defensible than adopting any one external parser as a replacement.
  - Static architecture studies should separate implemented code from README/roadmap claims before borrowing patterns.
  - GSD browser evidence validation can false-positive on artifact/research milestones when generic words are cached in the detector; local artifact assertions can satisfy the gate without fabricating external UI behavior.
---

# M033-732r1t: External Parser and Paper Knowledge Architecture Research

**Completed bounded external parser research and recommended a fail-closed combined sidecar architecture with a future quality plan.**

## What Happened

M033 evaluated daily-archive's current parser/conversion/refusal baseline against GROBID, OpenDataLoader PDF, Adaptix typed adapter mapping, and quant-mind architecture patterns. S01 established the current baseline and safety/refusal boundaries. S02 studied GROBID via Docker CRF probe and classified it as `grobid-scholarly-sidecar-candidate` for TEI metadata, sections, bibliography, and citation/ref markers. S03 ran OpenDataLoader hands-on against three local PDFs through the hybrid docling-fast backend and classified it as `hybrid-sidecar-candidate` for layout/OCR/table/page-coordinate sidecar evidence, with OCR/table fidelity still unproven. S07 mapped fixed OpenDataLoader JSON through Adaptix into typed review-only daily-archive candidate summaries and classified it as `adaptix-adapter-candidate`. S04 studied quant-mind statically, did not run live OpenAI/API/network flows, and classified it as `pattern-source-not-dependency` for TreeKnowledge/PageIndex, PaperKnowledgeCard, typed provenance, pipeline separation, bounded concurrency, and resolver guardrail patterns. S05 synthesized all evidence into `recommended-bounded-combined-sidecar-architecture`: GROBID for scholarly TEI, OpenDataLoader for layout/OCR/table/coordinates, Adaptix for typed mapping, quant-mind for patterns only, and daily-archive for contracts, validators, review gates, graph-readiness, and no-write import decisions. S06 produced a bounded future quality/integration plan with corpus classes, metrics, artifact contracts, diagnostics, rollback/no-adoption triggers, and closeout verification. No production parser integration, graph import, LadybugDB write, dependency adoption, or import eligibility was authorized.

## Success Criteria Results

- PASS: Current parser/conversion/refusal baseline was mapped in S01.
- PASS: GROBID capabilities, runtime complexity, TEI/bibliography/citation strengths, and limits were mapped in S02.
- PASS: OpenDataLoader PDF was tested hands-on on three local PDFs in S03 with backend/cache/run/quality/contract artifacts.
- PASS: quant-mind was evaluated as architecture pattern source in S04 without runtime adoption or OpenAI/API/network execution.
- PASS: Adaptix adapter evidence was added in S07 and consumed by S05.
- PASS: Combined architecture recommendation was produced in S05 with explicit component responsibilities and rejected alternatives.
- PASS: Bounded follow-up quality/integration plan was produced in S06 with quality dimensions, diagnostics, artifact contracts, and no-adoption boundaries.
- PASS: All milestone-level command-line verifiers, tests, Ruff checks, JSON invariants, and local artifact report assertions passed.
- PASS: Latest GSD milestone validation verdict is pass.

## Definition of Done Results

- PASS: All seven slices are complete: S01, S02, S03, S04, S05, S06, S07.
- PASS: All planned tasks are complete.
- PASS: Fresh verification was run before slice and milestone completion.
- PASS: GSD validation artifact verdict is pass.
- PASS: No graph import, LadybugDB write, production import, or positive import eligibility claim was introduced.
- PASS: Temporary localhost server used for artifact report assertions was stopped after validation.

## Requirement Outcomes

- R053: advanced/satisfied by bounded external parser evaluation, recommendation, and quality plan.
- R050: advanced by paper knowledge architecture sidecar/tree/card/provenance plan and future quality gates.
- R029: preserved by explicit graph-readiness/no-write/import boundaries, review post-check requirements, no-write rehearsal expectations, and false safety flags.
- No new active requirement requiring immediate scope change was discovered.

## Deviations

S05 and S06 were refined after S01-S04/S07 completed, because their original roadmap entries had zero tasks and needed evidence-specific planning. The first milestone validation attempts were downgraded by a GSD browser-evidence false positive over research terms; after local artifact report assertions and a pass validation, closeout proceeded.

## Follow-ups

If continuing beyond M033, create a new milestone for the S06 bounded quality plan: select concrete corpus papers, implement future artifact contracts/schemas, run no-network/cache preflight, evaluate GROBID and OpenDataLoader quality gates, verify Adaptix/daily-archive candidate mapping, run graph-readiness review post-check, and keep no-write import rehearsal until separately authorized.
