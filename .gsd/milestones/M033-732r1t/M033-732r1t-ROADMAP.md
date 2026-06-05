# M033-732r1t: External Parser and Paper Knowledge Architecture Research

**Vision:** Evaluate whether daily-archive should improve scientific article parsing by combining GROBID for deep scholarly and bibliography parsing with OpenDataLoader-style layout/table/OCR extraction, while also extracting reusable architecture patterns from quant-mind. The milestone must produce bounded evidence and an integration recommendation, not production parser adoption, graph readiness, LadybugDB writes, or positive import eligibility.

## Success Criteria

- Current daily-archive parser/conversion/refusal baseline is mapped clearly enough to compare external tools against existing contracts.
- GROBID capabilities, output contracts, runtime complexity, and bibliography/TEI strengths are understood against daily-archive needs.
- OpenDataLoader PDF is tested hands-on on three local PDF articles with OCR/layout backend investigation, backend health or typed blocker evidence, output artifacts, quality review, and contract mapping.
- quant-mind is evaluated as architecture-pattern reference, separating implemented code from README vision and identifying reusable patterns without adopting it as production dependency.
- A combined architecture recommendation explains whether GROBID plus OpenDataLoader-style tooling is feasible, how responsibilities should be split, what complexity it adds, and what remains blocked.
- A bounded follow-up quality/integration plan exists for any recommended parser combination, preserving no graph import, no LadybugDB write, and no graph-readiness claims.

## Slices

- [x] **S01: Current Parser Baseline Map** `risk:high` `depends:[]`
  > After this: After this: daily-archive's current parser/conversion/refusal contracts are mapped as the comparison baseline for external tools.

- [x] **S02: GROBID Scholarly Parsing Study** `risk:high` `depends:[S01]`
  > After this: After this: GROBID's TEI, bibliography, citation, metadata, runtime, and service complexity are understood against daily-archive contracts.

- [x] **S03: OpenDataLoader OCR Layout Table Probe** `risk:high` `depends:[S01]`
  > After this: After this: OpenDataLoader PDF has been tested or blocked on three local PDFs with backend health, outputs, quality review, and contract mapping evidence.

- [x] **S04: QuantMind Architecture Pattern Study** `risk:medium` `depends:[S01]`
  > After this: After this: quant-mind is classified as pattern source versus dependency, with reusable paper-knowledge ideas mapped to daily-archive.

- [x] **S05: Combined Parser Architecture Recommendation** `risk:high` `depends:[S02,S03,S04,S07]`
  > After this: After this: there is a recommended or rejected architecture for combining GROBID, OpenDataLoader-style extraction, Adaptix typed adapter evidence, daily-archive contracts, and quant-mind-inspired patterns.

- [x] **S06: Bounded External Parser Quality Plan** `risk:medium` `depends:[S05]`
  > After this: After this: a future implementation/probe milestone can run a bounded parser quality evaluation without weakening no-import safety boundaries.

- [x] **S07: Adaptix OpenDataLoader Adapter Probe** `risk:medium` `depends:[S03]`
  > After this: After this: OpenDataLoader fixed JSON has been tested through an Adaptix typed adapter into review-only daily-archive candidate summaries, with safety flags fail-closed.

## Boundary Map

### S01 → S02/S03/S05
Produces:
- Current daily-archive parser/conversion/refusal baseline matrix with input/output/diagnostic/safety contracts.
Consumes:
- M031 parser/conversion/chunk/no-write evidence and current codebase contracts.

### S02 → S05/S06
Produces:
- GROBID capability, output, runtime, and TEI/bibliography mapping assessment.
Consumes:
- S01 baseline for comparison.

### S03 → S05/S06
Produces:
- OpenDataLoader three-PDF probe artifacts, backend health/blocker evidence, output quality review, and daily-archive contract mapping.
Consumes:
- S01 baseline and local PDF manifest.

### S04 → S05/S06
Produces:
- quant-mind architecture pattern assessment with implemented-vs-aspirational separation.
Consumes:
- GitNexus-indexed quant-mind source.

### S05 → S06
Produces:
- Combined architecture recommendation and complexity assessment.
Consumes:
- S01-S04 findings.

### S06 → future implementation milestone
Produces:
- Bounded quality/integration evaluation plan for any recommended parser combination.
Consumes:
- S05 recommendation and S03 quality evidence.
