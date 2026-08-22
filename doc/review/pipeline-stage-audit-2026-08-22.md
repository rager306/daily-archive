# Pipeline Stage Audit — ADR requirements vs implementation (2026-08-22, M445)

Scope: every pipeline stage from multi-source acquisition to the agent read
plane, checked against binding ADRs. Verdicts: READY / READY-WITH-GAPS /
GAP. Evidence paths are repo-relative. Companion decision trail: D276
(stage→layer binding), D279 (triage DATA), D284 (hot-path indexes).

## 1. Stage-by-stage

| Stage (ledger id) | ACAP layer | ADR | Logic / branching | Gaps → what to fix |
|---|---|---|---|---|
| acquisition | source_registered | ADR-056 W1–W5 | Catalog-hit + script ensure; SourceIdentity from assets (pdf/html sha, fingerprints); typed LoadReport | **GAP (multi-source)**: sources beyond arXiv+HTML-textbook not implemented: W10 rust-native fetch demoted to script; no DOI-first / PubMed / bioRxiv adapters; `source_codes.yaml` has vocabulary but only `arxiv` + textbook code exercised |
| catalog | source_verified | ADR-056, ADR-065 | Version binding: exact catalog revision → tei_declared fallback (v-bound, 589/604); versionless → typed skip; manifestation sha256 == parsed.pdf_hash enforced | 15 papers remain no_stamp (HTML idno / arXiv-API recovery — honest typed skips today) |
| parse | parsed | ADR-053, D204 | GROBID structural profile (DATA-fingerprinted); HtmlParser for HTML; pdf-inspector pre-triage (D279 DATA threshold); page_text_missingness stamped (M422) | READY. Aliases grobid_parse/html_parse/document_context/figure_detect all canonicalize here (documented gotcha: `document_context` step name hits parse gate — deferred ledger fix Q4#4) |
| document_context ("pageindex" AST) | structured | ADR-053 §layers, governor html_structure_honesty | Dotted `path_id`, per-parent `order`, `parent_section_id`, prev/next navigation chain; `~N` restart suffixes never become dotted parents; config fingerprint `navigation=source-order;summary_join=path_id` | READY-WITH-GAPS: page-level AST (page→column→block anchors) not modeled — sections only; figure_quote_policy gives bounded quote windows but no cross-page span continuity |
| figure_context / figure_analysis | structured / extracted | ADR-060, ADR-062 | Chart-native payloads, no synthetic components; quote policy windows (DATA) | READY |
| table_fidelity | evidence_linked | ADR-047 conflict rules | 18-cell fidelity, row/col/digest, typed conflicts never overwrite | READY |
| equation_extraction | embedded | equation_coverage_contract.yaml | MathML grounded, typed missingness (M414) | READY |
| revision_materialization | structured | ADR-058 | Revision-owned vids, [from,to), SUPERSEDES; replay-idempotent (M440: 589/589 zero violations) | READY |
| extraction_compute / _materialization | extracted / evidence_linked | ADR-045 patterns | Rule-based over DATA dictionaries (methods.phrases); entity vid dedup (shared nodes); MENTIONS/FOUND_IN per paper | 3 zero-mention papers (non-CS lexis) → dictionary extension backlog; citation-entity linking not extracted (authors/venues are not entities today) |
| embedding | embedded | ADR-040 | fd-api bge-m3 via env (FD_*), DeterministicEmbedder in tests; cached Vec<f32> replay | READY |
| summary | summarized | snapshot_derivation + section_summary_roles | Bounded→chained extractive, revision-grounded (M395/M396) | READY |
| readiness / cluster / resolution | (no layer — typed skip by design) | entity_resolution.yaml | Cluster detection, entity resolution DATA-driven | READY; `readiness`/`cluster` carry no acap_layer (honest no-claim) |
| assurance | — | ADR-055 | ACAP replay via InstrumentedGraphStore physical outcomes; hard invariants vs diagnostic scores | READY |
| checkpoint | accepted_checkpoint | snapshot lineage (M382) | .sgsnap export, SnapshotIdentity, chain continuity | READY |
| query | — | ADR-061 | RO RESP plane, DA.IDENTITY, `da agent-run --verify`; writes fail closed | READY; full remote GraphStore (P1) and RuVector agent brain (vendored, unwired) remain the two big agent-readiness gaps; ADR-039 REVIEW/OUTPUT FSM states not modeled |

## 2. Scheduling / FSM

- Scheduler TaskTypes: Ingest, Reparse, Extract, EmbedStale, OpenAlexEnrich
  are real tasks with executors/producers (M410–M412, M416–M418);
  **GraphWrite deferred-by-design** (D278, governor guard asserts nothing
  claims it).
- ACAP runtime admission: `StageIdRegistry::acap_layer` is the only
  lookup; orchestrator has **no hardcoded stage match** (D276 verified).
  Missing layer ⇒ typed skip. Governor pins `ledger_layers` — the one
  known wrinkle is the alias-canonicalization gotcha above.

## 3. Meta-prompt FSM readiness (hardcode hunt)

- Prompt surface is DATA: `data/ontology/prompt_tasks.yaml`; prompt/model
  fingerprints are part of cache StageCacheKey (ADR-055 cache contract) —
  prompt changes invalidate only prompt-dependent keys (tested).
- ADR-059 optimizer FSM exists; DSPy/GRPO admission locked until frozen
  eval (D136) — by design, not a gap.
- Hardcodes found (exhaustive scan of adapters/application for URLs):
  - `openalex_adapter.rs`: DOI filter `doi:10.48550/arxiv.{id}` is
    arXiv-specific and string-built → should come from a source-code→DOI-
    prefix map in `source_codes.yaml` when non-arXiv sources land.
  - All service endpoints/env: clean (GROBID_*, FD_*, DA_*).
  - Stage implementation/configuration fingerprints: identity strings by
    design (cache versioning), not config hardcodes.

## 4. Dictionaries / reference data (29 YAML)

Implemented: extraction_patterns (methods.phrases), node_schemas (31),
edge_contracts, node_vocabulary, source_codes, arxiv_categories,
extension_domains, failure_taxonomy, algorithm_params,
catalog_sweep_thresholds, pdf_triage_policy, figure_* (schemas, type
labels, panel markers, quote policy), equation_coverage_contract,
identity_indexes, property_indexes, revision_classification/readiness,
section_summary_roles, snapshot_derivation, context_conservation,
influence_flower_policy, entity_resolution, decision_categories,
acap_runtime_control_plane, prompt_tasks.

**To add for unification/links (priority order):**
1. `citation_styles.yaml` — arXiv/bibTeX/DOI/URL reference-shape parsing
   rules (today only GROBID biblStruct + arXiv-id heuristic).
2. `venue_registry.yaml` + `author_identity.yaml` — canonical venues/
   authors with aliases; needed before citation entities can materialize
   without name-collision chaos.
3. `doi_prefix_map.yaml` (source_code → DOI prefix) — kills the OpenAlex
   hardcode and generalizes enrichment.
4. `method_taxonomy.yaml` — method.phrases → taxonomy parents (lineage
   queries beyond flat strings).
5. Extend `methods.phrases` for math-oc / robotics lexes (3 zero-mention
   alarms: 1203.2295, 2302.06590, 2504.16925).

## 5. Scientific context (citations, references, links)

Present: Reference nodes per paper (81–84 avg) + CITES edges with
anchors (`citations_with_anchors`, revision_anchor_edges), Citation
arxiv_id resolution, OpenAlex Work enrichment (stub-with-pending honest),
DOI carried on Work, evidence chains (ADR-042).

Missing: DOI-first resolution path (doi.org fetch), Crossref fallback,
citation-entity extraction (authors/venues as first-class nodes),
co-citation / citation-context (WHERE in the text it cites — anchor exists
for GROBID markers only), non-arXiv external-id space.

## 6. Anomalies / irregularities register

1. `edge_contracts.yaml` exists in BOTH `data/` and `data/ontology/` —
   drift risk; unify on one path with the other as generated export.
2. `document_context` alias-canonicalization FSM wrinkle (deferred Q4#4).
3. 3 zero-mention extraction alarms (known, backlogged).
4. 15 versionless papers (typed skip; recovery path designed, not built).
5. GraphWrite executor absent by design — but producers that could fill
   it (corpus graph materializer) now exist; revisit D278 when durable
   Samyama server (P1) lands.
6. target/ bloat flag (housekeeping, non-blocking).

## 7. Verdict

Linear spine acquisition→checkpoint: **READY end-to-end on the arXiv
catalog corpus** (proven by the 589-paper zero-failure run + regression
harness). The honest frontier, in order: multi-source acquisition
(DOI/PubMed/bioRxiv), citation/venue/author dictionaries, page-level AST,
agent brain wiring. Nothing in the spine blocks agent read usage today.
