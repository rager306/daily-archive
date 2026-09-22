---
name: daily-archive
last_updated: 2026-09-22
---
<!-- Initial draft 2026-09-22: authored autonomously at the owner's explicit no-questions instruction, grounded in repo evidence (AGENTS.md, ADR-074/075/076, absorption registries) and owner decisions taken this session (D127 locks, human gates, anti-fabrication). NOT owner-interviewed: treat as a draft needing owner review, not settled direction. -->

# daily-archive Strategy

See PRODUCT.md for the product's current shape and VISION.md for the destination; this document carries direction.

## Purpose

A researcher's arXiv corpus grows faster than any human can read it, and the failure mode of every automated extractor is silent: facts are dropped or invented without a trace. daily-archive exists to turn that corpus into a knowledge graph where every step proves what it kept, what it lost, and what it does not know.

## Positioning

Local-first and honesty-gated: every parse and extraction claim carries machine-verifiable conservation evidence and typed absence, and nothing reaches the graph without an explicit human release — a commitment no hosted parse-API pipeline can truthfully make.

## Users

**Primary:** The owning researcher — they hire daily-archive to convert their paper corpus into a citable, queryable scientific knowledge graph without having to trust black-box extraction.

## Boundaries

- No auto-promotion of candidates; promotion requires frozen evidence and an explicit decision.
- No graph writes or import before honest parity and a human yes (D127).
- Extraction F1 is never the parse acceptance oracle; percentages of "migration" are not a metric.
- No big-bang GROBID rewrite and no extra parse sidecars (MinerU, Docling, pdffigures2, Python OCR); the cascade is replaced one family at a time.
- Human release decisions are never fabricated or auto-recorded.

_Resist a change when:_ it would mark work done without machine-verifiable evidence at a pinned source revision, or route around a human gate.

## Key metrics

- **Parse conservation** — `no_silent_loss` and family-receipt accounting on tracked runs; governor `scholarly_parse_plane` checks.
- **Composition correctness** — independent-parser contract and property suites over preserved TEI; measured in the `native_fulltext_composition_contract` / `_properties` suites.
- **Extraction quality** — micro P/R/F1 on the pinned gold corpus via `eval_batch`.
- **Corpus regression numbers** — precision/recall/F1 on the fixed real corpus with frozen denominators (H18 target).
- **Trajectory verdict** — governor report (`on_track` / `drift_risk` / `blocked`) per phase.

## Tracks

### Honest GROBID absorption

Replace the Java parse cascade one family at a time with conservation-proven native stages.

_Why it serves the approach:_ production parse honesty is the precondition for every downstream claim the graph makes.

### KG materialization readiness

Extraction quality, semantic graph task suite, and catalog-scale materialization that make the locked graph trustworthy enough to unlock.

_Why it serves the approach:_ import should flip only when the content behind it is already verified end-to-end.

### Delivery reliability

GSD lifecycle friction reduction, governor gates, and the ADR-051 verification contours that keep long autonomous waves from lying about progress.

_Why it serves the approach:_ a program measured in months dies if its own automation stalls silently or reports invented success.
