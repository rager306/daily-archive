# M006-638rza: Thirty Paper Deviation Scan

**Vision:** Expand M005's 10-paper import-model evidence to a 30-paper deviation scan so the project can discover outliers, recurring failure modes, and new corpus-level patterns without confusing broader measurement with production KG readiness.

## Success Criteria

- 30-paper corpus selected with documented rationale and availability diagnostics.
- Current no-import/no-write evidence path is exercised or blocked per paper with explicit diagnostics.
- Deviation analysis identifies new outliers/patterns compared with the M005 10-paper baseline.
- Independent review confirms semantic usefulness of the scan and blocks over-claims.
- Positive KG import and production writes remain blocked.

## Slices

- [x] **S01: S01** `risk:high` `depends:[]`
  > After this: After this slice, there is a 30-paper manifest with selection rationale, local source availability, and known risk tags.

- [x] **S02: S02** `risk:high` `depends:[]`
  > After this: After this slice, the 20 missing-Markdown expansion papers have bounded acquisition/conversion attempts with redacted diagnostics, and the 30-paper corpus is either source-ready or explicitly blocked per paper.

- [x] **S03: S03** `risk:medium` `depends:[]`
  > After this: After this slice, there is a deviation report comparing 30-paper behavior against M005 baseline and identifying new patterns/outliers.

- [x] **S04: S04** `risk:medium` `depends:[]`
  > After this: After this slice, independent review confirms whether the 30-paper scan meaningfully identifies deviations and what next remediation should be.

## Boundary Map

```text
In M006:
  Select and document a 30-paper deviation-scan corpus
  Run current M005 measurement/structure/annotation/source/benchmark/negative-boundary evidence where feasible
  Compare 30-paper distributions against M005 10-paper baseline
  Identify new outliers, source-artifact gaps, route/refusal shifts, and repeated failure patterns
  Produce recommendations for remediation or further expansion

Out of M006:
  Production LadybugDB KG writes
  Positive trusted KG import
  Broad corpus or week-scale scaling
  Semantic/vector retrieval claims
  Entity/relation extraction claims
  Asset-to-fact promotion
  DSPy optimizer adoption
  Raw text/chunk text/embeddings/vectors in machine logs
```
