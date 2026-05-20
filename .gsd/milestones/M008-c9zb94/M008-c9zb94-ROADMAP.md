# M008-c9zb94: First New Plus Ten Validation Batch

**Vision:** Use the M007 deterministic validation-batch workflow for its first real expansion: one new +10-paper batch, fully artifacted and independently reviewed before any further scaling.

## Success Criteria

- A new deterministic +10 corpus is selected with no overlap against M006 30 papers.
- The new batch runs through validation-batch init and preflight.
- Source gaps are resolved boundedly or block scan explicitly.
- The batch is scanned and reviewed if source-ready.
- Final recommendation decides whether to continue another +10.
- No positive KG import or production writes occur.

## Slices

- [ ] **S01: Select first new plus ten corpus** `risk:medium` `depends:[]`
  > After this: After this slice, there is a deterministic next-10 manifest that excludes all M006 papers and explains why each paper was selected.

- [ ] **S02: Initialize and preflight new plus ten batch** `risk:high` `depends:[S01]`
  > After this: After this slice, the new +10 batch has M007 batch-state/source-preflight artifacts and any missing Markdown blockers are either repaired with bounded steps or explicitly block scan.

- [ ] **S03: Scan new plus ten batch** `risk:high` `depends:[S02]`
  > After this: After this slice, the source-ready new +10 batch has scan, delta, outlier, and import-gate artifacts, or a clear blocker if scan is unsafe.

- [ ] **S04: Review first new plus ten batch** `risk:medium` `depends:[S03]`
  > After this: After this slice, independent review says whether the first new +10 batch is good enough to continue another +10, needs fixes, or blocks progression.

## Boundary Map

| Boundary | In scope | Out of scope |
|---|---|---|
| Selection | Deterministic next 10 paper IDs not already in M006 30-paper corpus | Broad crawling or arbitrary manual picks |
| Source work | M007 init/preflight and bounded source acquisition/repair only if needed | Unbounded conversion loops |
| Scan | M007 validation-batch scan/delta/outlier artifacts | Semantic correctness claims |
| Review | Independent review of new +10 evidence | Trusted KG promotion |
| Storage | Local redacted artifacts | Production LadybugDB writes |
| Scaling | One reviewed +10 batch | Unattended run to 100 |
