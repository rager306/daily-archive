# M011-2f8j8m: Semantic Import Readiness Gate

**Vision:** Move beyond operational +10 scan counts by creating a bounded, redacted semantic review gate that evaluates whether M010 chunks/outliers can support trusted scientific KG extraction, while preserving no-import/no-write boundaries.

## Success Criteria

- Bounded semantic review corpus selected from M010 without raw text leakage.
- Redacted rubric and judgments produced for every selected target.
- Independent review completed with PASS or FLAG verdict.
- Final recommendation preserves no-import/no-write unless a future rehearsal is explicitly scoped.
- R038 advanced or validated with artifact evidence.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: A redacted semantic review corpus manifest exists, pointing to source files by path/hash/span and M010 scan evidence by artifact path, with leakage guard passing.

- [x] **S02: S02** `risk:high` `depends:[]`
  > After this: A review rubric and redacted judgment packet are available for the selected targets, with decisions expressed as categories and source references rather than raw text.

- [x] **S03: S03** `risk:high` `depends:[]`
  > After this: An independent reviewer has evaluated the rubric and redacted judgments and produced a PASS or FLAG verdict with concrete blockers and limits.

- [x] **S04: S04** `risk:medium` `depends:[]`
  > After this: A final recommendation states whether positive import remains blocked and what exact next milestone is justified by the semantic evidence.

## Boundary Map

| Boundary | In scope | Out of scope |
|---|---|---|
| Source evidence | Paths, hashes, spans, structural metadata, redacted review judgments | Embedding raw paper text or chunk text into JSON/JSONL/GSD artifacts |
| Review | Bounded semantic read of selected chunks/outliers with redacted findings | Production extraction, trusted fact creation, broad corpus scaling |
| Import | Negative or blocked import-readiness recommendation only | Positive KG import or LadybugDB writes |
| Automation | Reusable runbook and optional helper scripts if needed | Unattended run-to-100 or optimizer-driven extraction |
