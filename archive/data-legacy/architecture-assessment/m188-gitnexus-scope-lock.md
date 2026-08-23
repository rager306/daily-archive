# M188 GitNexus Scope Lock

## Verdict

**M188 scope is real-corpus validation readiness, not new source movement.**

## GitNexus planning basis

Two GitNexus queries were used to identify the next post-M187 architecture crystallization work:

1. `post transition ratchet real corpus validation architecture crystallization manifest residuals write path inventory canonical baseline`
2. `real corpus validation M029 M030 M061 article catalog parser chunk graph readiness smoke selection validation evidence`

The strongest planning signal is that the remaining useful work is not another manifest residual refactor. It is validation readiness around existing real-corpus and smoke validation flows.

## Candidate flows

| Flow | GitNexus symbol | Use in M188 | Boundary |
|---|---|---|---|
| M029 unified replay | `Function:scripts/run_m029_unified_replay.py:run` | Candidate existing replay surface for S02/S03 evidence mapping. | Generates replay artifacts; do not run in a mode that overwrites canonical baselines unless explicitly planned. |
| M027 source acquisition boundary | `Function:scripts/verify_m027_source_acquisition_boundary.py:main` | Candidate source-quality and fail-closed validation surface. | Treat low-quality source as fail-closed; do not infer success from non-empty markdown or HTTP 200. |
| Universal KB smoke runner | `Function:src/research_graph/workflows/universal_kb/smoke_runner.py:run_article` | Candidate future smoke/readiness surface. | No queue production claims; no graph/import readiness unless proven. |
| M030 requested-ref intake | GitNexus process around M030 refs and selection identity | Existing current validator for catalog and intake consistency. | Validation-only in M188 unless a blocker is found. |

## Candidate tests and checks

Initial S02/S03 command candidates:

- `uv run python scripts/verify_article_catalog.py`
- `uv run python scripts/verify_m030_requested_ref_intake.py ... --validate-only`
- `uv run pytest tests/test_m029_post_validation_remediation.py -q`
- `uv run pytest tests/test_m029_loader_runtime_smoke.py -q`
- `uv run pytest tests/test_m031_chunk_evidence_replay.py -q`
- `uv run pytest tests/test_m036_real_corpus_no_write_smoke.py tests/test_m036_real_corpus_smoke_audit.py -q`

These are command candidates only; S02 will narrow them by current file availability and mutation risk before running.

## Non goals

M188 will not:

- introduce DSPy, RLM, optimizer, or ablation claims;
- promote parser evidence to graph readiness;
- introduce direct extractor to graph writes;
- mutate production corpus or canonical baselines without an explicit later plan;
- claim source quality from HTTP 200 or non-empty markdown alone.

## Scope decision

Proceed with evidence-only validation waves first:

1. S02: current validator and real-corpus gate baseline.
2. S03: parser/chunk readiness assessment using existing evidence.
3. S04: validation closeout and next milestone recommendation.
