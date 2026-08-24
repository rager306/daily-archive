# ADR-072: XY-Cut Reading-Order Reconstruction for TOC-less PDF Pages

- Status: Accepted
- Date: 2026-08-24
- Deciders: repo owner + agent (M451)
- Tags: pdf-parsing, reading-order, layout-analysis, xy-cut, data-driven
- Sources: opendataloader-pdf `XYCutPlusPlusSorter` (Apache-2.0, arXiv:2504.10258),
  ispras/dedoc structure patterns (`HierarchyLevel`, patterns-as-DATA, `can_be_multiline`),
  our anchor-splitting (pdf_toc.rs) and margin classification (reference exporter)

## 0. One-line Decision

**Implement deterministic XY-Cut reading-order reconstruction as a pure function over neutral line boxes (da-domain/xy_cut.rs), ported from opendataloader's XYCutPlusPlusSorter with dedoc's block-boundary discipline for paragraph merging — used to classify pages of PDFs lacking authorial TOC bookmarks and as the figure-zone extractor for complex typesetting.**

## 1. Context

Our monograph pipeline depends on authorial TOC bookmarks (page_outline
grammar). Three gaps surfaced while processing Algorithms for Decision
Making (Tufte-style layout):

1. Pages where figure axis labels sit in the left body zone interleave into
   reading order (p61: ticks Z/Y/0.13/0.05/X mixed with prose).
2. PDFs without embedded TOC have no entry grammar at all.
3. Paragraph assembly needs block-boundary discipline: naive continuation
   merging glues text across columns/cuts.

Studied via codebase-memory indexes (opendataloader-pdf 5261 nodes,
dedoc 4683 nodes) plus the arXiv paper.

## 2. Decision

### 2.1 Port scope (da-domain/src/xy_cut.rs — no external deps)

Neutral `LineBox {text, x0..y1}` input; adapters convert pdf-inspector
`TextItem` or PyMuPDF dict lines.

Ported phases (each verified against the source):

| Phase | opendataloader | Our port |
|---|---|---|
| Pre-mask headers/footers | y-band furniture | same (header_y_max=50, footer_band=70) |
| Margin sidenotes | not handled | **ours**: x>0.65W zone; numeric ticks dropped, prose kept separately |
| Figure-zone pre-mask | PP-DocLayout model + masking | **reduced**: compact cluster (≥3 tick-ish lines) with zero prose inside bbox → masked; no ML model needed for our corpus |
| Recursive segmentation | largest-gap axis, MIN_GAP_THRESHOLD=5pt | same |
| Narrow-element retry | NARROW_ELEMENT_RATIO=0.1 | same (page numbers bridging column gaps) |
| No-valid-cut fallback | sort by Y then X | same |
| Merge-back cross-layout | by topY | margins sorted by center_y |

### 2.2 dedoc discipline adopted

- **Paragraph fusion never crosses cut boundaries**: merging is a separate
  pass over ordered boxes with geometry guards (same column: Δx0 ≤ 25%
  width; vertical gap ≤ 0.5×line-height; previous line ends mid-sentence).
  This mirrors dedoc's `can_be_multiline` being a per-node property.
- **Patterns stay DATA**: dedoc's RegexpPattern/tag_pattern hierarchy maps
  onto our existing structural_type_detectors.yaml — no new mechanism
  needed; detectors already carry regex pattern lists with notes.
- **HierarchyLevel(level_1, level_2)** lexicographic tree construction:
  noted for navigation depth >4; our path_id already encodes depth —
  adoption deferred until a document demands it.

### 2.3 What we deliberately did NOT adopt

- JVM runtime (opendataloader is Java) — violates single-runtime discipline;
  GROBID remains the only sanctioned sidecar.
- ML layout detector (PP-DocLayout) for pre-masking — our corpus is
  scientific monographs where geometric tick-clustering suffices (verified:
  p61 zone x163..306×y187..332 contains 0 prose lines).
- dedoc service/docker deployment — library functions only.

## 3. Consequences

- New fallback grammar path: page_outline (TOC present) → xy_cut pages →
  GROBID (articles). PDFs without bookmarks gain a deterministic first-class
  route instead of defaulting to GROBID.
- Figure zones are available as structured regions (masked set) — future
  wiring into figures/index.json provenance.
- 8 unit tests encode ported behaviour including two regression classes:
  cross-column paragraph fusion (caught during development), narrow-page-
  number column bridging.

## 4. Evidence

- Unit tests: margin separation, axis-label drop, two-column interleaving,
  full-width title precedence, narrow outlier retry, continuation merge
  within column only, wrapped-line across columns isolation, tick-cluster
  masking (8 tests).
- Live specimen checks (Python mirror of the Rust algorithm): p42 body
  order restored; p61 axes removed from body (4 clean paragraphs vs
  previously interleaved); tick zone contains zero prose.
- Suite green: domain 699, cli 176, pytest 145, validator 645×0, clippy clean.
