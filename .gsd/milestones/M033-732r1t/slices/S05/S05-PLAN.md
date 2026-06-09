# S05: Combined Parser Architecture Recommendation

**Goal:** Synthesize S01/S02/S03/S04/S07 findings into a combined parser architecture recommendation that assigns responsibilities across GROBID, OpenDataLoader-style extraction, Adaptix adapter mapping, quant-mind-inspired patterns, and daily-archive-owned validators while preserving fail-closed graph/import boundaries.
**Demo:** After this: there is a recommended or rejected architecture for combining GROBID, OpenDataLoader-style extraction, Adaptix typed adapter evidence, daily-archive contracts, and quant-mind-inspired patterns.

## Must-Haves

- S05 consumes and cross-references S01 baseline, S02 GROBID verdict, S03 OpenDataLoader verdict, S07 Adaptix verdict, and S04 quant-mind pattern verdict.
- The recommendation explicitly chooses or rejects a combined architecture. Expected recommendation direction: `recommended-bounded-combined-sidecar-architecture`, not production adoption.
- Responsibility boundaries are clear: GROBID for scholarly TEI/metadata/references/citations, OpenDataLoader for layout/OCR/table/coordinate candidates, Adaptix for typed adapter mapping over fixed JSON, quant-mind for tree/card/provenance patterns, daily-archive for contracts/validators/review/graph readiness.
- Complexity and operational costs are documented: GROBID Docker/JDK21/full-DL future option, OpenDataLoader hybrid backend/model cache/lifecycle, Adaptix dependency scope, and quant-mind no-runtime decision.
- All outputs keep graph/import/write safety flags false and explicitly state parser output is candidate evidence only.

## Proof Level

- This slice proves: Artifact synthesis and validate-only closeout over completed prior slice evidence; no new external parser runtime is required.

## Integration Closure

S05 must produce repo-local recommendation artifacts under `data/article_corpora/m033-combined-parser-architecture-v1/`. It must not modify production parser code, add dependencies, run graph imports, write LadybugDB, or claim import eligibility.

## Verification

- Produces machine-readable recommendation, decision matrix, boundary map, risks/unknowns, and closeout summary for S06 planning and milestone validation.

## Tasks

- [x] **T01: Compiled completed S01/S02/S03/S04/S07 evidence into the S05 synthesis matrix.** `est:small`
  Read completed S01/S02/S03/S04/S07 artifacts and create a machine-readable evidence matrix summarizing verdicts, strengths, gaps, safety flags, and downstream implications. This is synthesis only; do not rerun external tools.
  - Files: `data/article_corpora/m033-combined-parser-architecture-v1/synthesis-evidence-matrix.json`, `data/article_corpora/m033-combined-parser-architecture-v1/synthesis-evidence-matrix.md`, `data/article_corpora/m033-combined-parser-architecture-v1/synthesis-events.jsonl`
  - Verify: Fresh command validates the evidence matrix includes S01, S02, S03, S04, and S07 entries, all expected verdict labels, and false graph/import/write safety flags.

- [x] **T02: Wrote the combined sidecar architecture recommendation for GROBID, OpenDataLoader, Adaptix, quant-mind patterns, and daily-archive validators.** `est:medium`
  Create the recommended architecture artifact describing the combined sidecar flow: source acquisition -> GROBID TEI sidecar -> OpenDataLoader layout/OCR/table sidecar -> Adaptix typed adapter -> daily-archive candidate contracts -> validators/review gates -> graph-readiness review. Include alternatives rejected and why.
  - Files: `data/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.json`, `data/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.md`
  - Verify: Fresh command validates recommendation verdict is `recommended-bounded-combined-sidecar-architecture`, includes component responsibilities for GROBID/OpenDataLoader/Adaptix/quant-mind/daily-archive, includes rejected alternatives, and keeps safety flags false.

- [x] **T03: Documented combined-architecture complexity risks and unresolved validation gates for S06.** `est:medium`
  Write the risk/unknowns and validation-gate artifact: runtime burdens, model cache risks, full-DL GROBID future accuracy option, layout/table fidelity gaps, source-span anchoring, bibliography/citation quality, reading order/OCR quality, review packet requirements, and no-write import boundary. This feeds S06 directly.
  - Files: `data/article_corpora/m033-combined-parser-architecture-v1/complexity-and-validation-gates.json`, `data/article_corpora/m033-combined-parser-architecture-v1/complexity-and-validation-gates.md`
  - Verify: Fresh command validates all expected risk categories and validation gates are present, including graph-readiness review and no-write/import flags false.

- [x] **T04: Added and passed the S05 validate-only closeout checker.** `est:small`
  Add a validate-only closeout checker for S05 artifacts and run the acceptance gate. It must reject missing slice evidence, unsafe flags, production adoption wording, graph-readiness claims, or missing responsibility boundaries.
  - Files: `scripts/verify_m033_combined_parser_architecture.py`, `data/article_corpora/m033-combined-parser-architecture-v1/combined-architecture-closeout-summary.json`, `data/article_corpora/m033-combined-parser-architecture-v1/combined-architecture-closeout-report.md`
  - Verify: `uv run python scripts/verify_m033_combined_parser_architecture.py --architecture-dir data/article_corpora/m033-combined-parser-architecture-v1 && uv run ruff check scripts/verify_m033_combined_parser_architecture.py` exits 0.

## Files Likely Touched

- data/article_corpora/m033-combined-parser-architecture-v1/synthesis-evidence-matrix.json
- data/article_corpora/m033-combined-parser-architecture-v1/synthesis-evidence-matrix.md
- data/article_corpora/m033-combined-parser-architecture-v1/synthesis-events.jsonl
- data/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.json
- data/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.md
- data/article_corpora/m033-combined-parser-architecture-v1/complexity-and-validation-gates.json
- data/article_corpora/m033-combined-parser-architecture-v1/complexity-and-validation-gates.md
- scripts/verify_m033_combined_parser_architecture.py
- data/article_corpora/m033-combined-parser-architecture-v1/combined-architecture-closeout-summary.json
- data/article_corpora/m033-combined-parser-architecture-v1/combined-architecture-closeout-report.md
