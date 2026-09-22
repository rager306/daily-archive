# daily-archive — Product

*Created: 2026-09-22. Current shape; see VISION.md for the destination, STRATEGY.md for direction.*

## What it is

**daily-archive** is a local-first scientific knowledge engine for a personal research corpus: it ingests PDFs and HTML from arXiv and similar sources, parses scholarly structure, extracts entities, embeds, and stages everything toward a durable knowledge graph (Samyama). Graph import is deliberately fail-closed (`import_eligible=false`, D127) until an explicit human release.

## What works today

- **Ingest and parse.** PDF fulltext via Java GROBID 0.9.0-crf (HTTP :8070) through `da-parse`; HTML parser side by side; profiles `structural` (default, Crossref-free) and `rich`; stage-tape recorder captures real cascade invocations as offline evidence.
- **Extraction.** Deterministic `RuleBasedExtractor` over parsed TEI; gold-corpus evaluation at **micro-F1 0.869** (P 0.769 / R 1.000; 193 papers evaluated of 200 fixtures, 7 skipped; 1,399 gold entities, TP/FP/FN 1,399/421/0 — AGENTS.md snapshot 2026-08-05).
- **Semantic materialization.** Catalog sweep materialized 589 papers into a committed `.sgsnap`; semantic graph task suite runs snapshot→tasks→verdict in one process; 14-stage pipeline ledger (`data/ontology/pipeline_stage_contracts.yaml`) records what each stage adds and loses.
- **Enrichment and summaries.** OpenAlex enrichment with a retry scheduler; article summaries and figure analysis run through `PromptExecutionPort` with versioned templates, JSON-schema validation and full lineage — outputs stay candidate-only, `eligible`/`deployed` remain false.
- **Query and inspection planes.** `da serve-graph` (read-only RESP identity plane, port 16379), graph-ping/graph-stats, schema registry and validators (26 indexes, edge contracts, node schemas), ontology export (31 schemas → YAML, data-driven validation).
- **Delivery and governance.** GSD milestone/slice/task lifecycle (M285…M511); governor CLI for trajectory verdicts, ADR reverse-audit and honesty reviews; pre-commit gates (fmt, clippy, architecture, docs drift).

## Program state: honest GROBID absorption

- Capability ledger: 38 capabilities — `proxy` 7, `shadow` 12, `candidate` 2, `fallback` 1, `absent` 16, `production` **0**. Native families (header atoms, bibliography, body accounting, visual, fulltext composition) exist and carry stage-contract closures (H04, H07–H13 complete) but remain shadow: not wired into production, guarded by quarantine scanners.
- **M511-j7wz2f active** — semantic correctness wave: S01 complete (composer XML honesty: RED→GREEN with an independent parser, 28 contract + 8 properties tests, 1374 suite total), S02–S09 pending (bibliography ID/target integrity, field fidelity, visual denominators, corpus regression, resource ceilings, parity ledger, frozen external holdout, document reconciliation).
- Locks: `deployed=false`, `import_eligible=false` (D127), `auto_promote=forbidden`, `graph_write=forbidden`, `this_wave_promotes=none`. GAR-015 (external holdout + human release decision) is the single open GAR finding.

## Where to look

| Question | Doc |
|---|---|
| Operations and commands | `README.md` |
| Project map, invariants, conventions | `AGENTS.md` |
| Direction / metrics / tracks | `STRATEGY.md` |
| Destination and principles | `VISION.md` |
| Domain vocabulary | `CONCEPTS.md` |
| Binding decisions | `doc/adr/ADR-INDEX.md` |
| Absorption registries | `doc/review/`, `data/parse/grobid_compatibility.yaml` |
