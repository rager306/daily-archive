# M006-638rza: Thirty Paper Deviation Scan

**Vision:** Expand M005's 10-paper import-model evidence to a 30-paper deviation scan so the project can discover outliers, recurring failure modes, and new corpus-level patterns without confusing broader measurement with production KG readiness.

## Success Criteria

- 30-paper corpus selected with documented rationale and availability diagnostics.
- Current no-import/no-write evidence path is exercised or blocked per paper with explicit diagnostics.
- Deviation analysis identifies new outliers/patterns compared with the M005 10-paper baseline.
- Independent review confirms semantic usefulness of the scan and blocks over-claims.
- Positive KG import and production writes remain blocked.

## Slices

- [ ] **S01: Thirty paper corpus selection and availability audit** `risk:high` `depends:[]`
  > After this: After this slice, there is a 30-paper manifest with selection rationale, local source availability, and known risk tags.

- [ ] **S02: Thirty paper dry run evidence** `risk:high` `depends:[S01]`
  > After this: After this slice, current M005-style dry-run evidence exists for the 30-paper corpus with redacted per-paper diagnostics.

- [ ] **S03: Deviation and pattern analysis** `risk:medium` `depends:[S02]`
  > After this: After this slice, there is a deviation report comparing 30-paper behavior against M005 baseline and identifying new patterns/outliers.

- [ ] **S04: Review and recommendation** `risk:medium` `depends:[S03]`
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
