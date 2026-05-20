# M007-opaont: Iterative Validation CLI Automation

**Vision:** Turn the M006 manual 30-paper diagnostic workflow into a deterministic, resumable CLI that can safely repeat +10-paper validation batches toward 100 papers while preserving redaction, review gates, and no KG import/write boundaries.

## Success Criteria

- A deterministic CLI-first workflow exists for validation batches.
- Batch state is resumable and artifact-driven.
- Source readiness, scan metrics, deltas, outliers, and review gates are automated.
- Safety boundaries remain explicit and enforced.
- Independent review approves the workflow for continuing toward 100 papers.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: After this slice, the project has a documented CLI contract, command names, state schema, fixture shape, and safety gates for iterative validation batches.

- [x] **S02: S02** `risk:high` `depends:[]`
  > After this: After this slice, a batch can be initialized and source-preflighted with redacted readiness/contradiction artifacts, without production writes or unbounded repair.

- [x] **S03: S03** `risk:high` `depends:[]`
  > After this: After this slice, a batch can run the existing deviation scanner and produce redacted delta/outlier reports against previous, cumulative, and M005 baselines.

- [x] **S04: S04** `risk:medium` `depends:[]`
  > After this: After this slice, independent review confirms whether the CLI workflow is useful and M007 closes with a tested recommendation for continuing toward 100 papers.

## Boundary Map

| Boundary | In scope | Out of scope |
|---|---|---|
| Batch selection | Deterministic +10 selection and persisted roles | Broad/week-scale crawling |
| Source handling | Preflight, fast acquisition, targeted repair state | Unbounded slow conversion jobs |
| Scan | Existing redacted structure-aware/deviation metrics | Semantic candidate validation |
| Review gate | Deterministic blockers/flags and report inputs | Trusted KG promotion |
| Storage | Local artifacts and resumable state | Production LadybugDB writes |
| MiniMax | Future optional adapter note only | M007 implementation dependency |
