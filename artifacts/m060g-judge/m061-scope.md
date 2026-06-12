# M061 Scope: 2-hop BFS with M3 Judge Integration

## Decision

M061 should proceed with 2-hop BFS acquisition, parsing, and figure QA diagnostics using the M3 multimodal judge selected in M060-gakmo0 S03.

Selected binding:

- Binding: `figure-qa-judge-quality`
- Model: `minimax-m3-multimodal-anthropic`
- Role: production choice for diagnostic figure QA judging, not production import authorization

## Why this scope

M060g S02 showed that M3 multimodal is the best current judge for figure-layer diagnostics:

- 8549 ms average latency versus 23846 ms for M2.7-highspeed.
- Better `figure_completeness`: 0.8757 versus 0.7823.
- Better `structural_fidelity`: 0.8603 versus 0.7467.
- 23 side-by-side wins out of 30, with 1 tie.
- 30/30 runs passed.

M2.7-highspeed remains useful for caption-heavy audit cases because it scored better on `caption_accuracy`, but it should not be the default M061 judge.

## Proposed M061 work package

Estimated engineering time: 8–10 hours.

Scope:

1. Acquire 2-hop BFS article/figure candidates.
2. Parse candidate PDFs through the manifest-driven ingest path from ADR-013.
3. Extract or reference figure images, captions, and structural metadata.
4. Run M3 multimodal judge on figure candidates.
5. Persist diagnostic artifacts: raw response, parsed scores, latency, prompt version, binding id, outlier flags, and safety override audit.
6. Produce review queues for outliers and caption/visual disagreements.

## Runtime and cost planning

S02 observed M3 multimodal at roughly 8.5 seconds per figure.

Full diagnostic run estimate:

- 2000 figures × 8.5 s = ~4.7 hours model wall time.
- 5000 figures × 8.5 s = ~11.8 hours model wall time.
- Practical planning range: 5–12 hours wall time, before orchestration overhead and retries.

Cost estimate remains not measurable from current artifacts without an external MiniMax pricing table. M061 should record token usage and response metadata, but should not invent USD costs.

## Lower-risk alternative

If full 2000–5000 figure execution is too expensive for first M061 pass, use a 10% sample:

- 200 figures × 8.5 s = ~28 minutes model wall time.
- 500 figures × 8.5 s = ~71 minutes model wall time.

This sample should preserve category diversity and include known hard cases from M058/M060g where possible.

## Safety defaults

The five defaults remain unchanged and must appear in M061 artifacts:

1. Graph writes are not authorized.
2. Production import is not authorized.
3. Fact promotion is not authorized.
4. External network default is disabled.
5. LLM calls default is disabled.

diagnostic-only override:

- `llm_calls_authorized` may be `true` only for a named M061 figure QA diagnostic run.
- The override must record reason, scope, generated timestamp, binding id, and non-authorization statements.
- The override does not authorize graph writes, production import, or fact promotion.

Local host references must use `127.0.0.1`, not the loopback hostname.

## Deliverables expected from M061

- Acquisition manifest for 2-hop BFS candidates.
- Parsing outputs linked to ADR-013 manifest contracts.
- M3 judge output directory with per-figure JSON artifacts.
- Aggregate comparison/report with latency distribution and outlier rate.
- Review queue for low-confidence or structurally suspicious figures.
- Explicit closeout statement on whether M062 should calibrate thresholds, sample more figures, or prepare a production-readiness gate for M063.

## Non-goals

- No production graph import.
- No fact promotion from judge output.
- No silent retries that hide model failures.
- No loopback hostname references in source or markdown; use `127.0.0.1`.
- No ensemble by default; M2.7 comparison should be scoped to audit cases only if needed.
