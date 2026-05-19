# Thirty-paper deviation evidence review

## Verdict: FLAG

The evidence is useful enough to support planning a deterministic +10-to-100 CLI automation milestone, but final recommendations must tighten several claims before treating the report as planning-grade.

The strongest result is operational rather than semantic: S03 proves the pipeline can summarize 30 Markdown-ready packages, preserve refusal boundaries, expose route distributions, and identify review-priority outliers without leaking chunk text or attempting KG writes. It does not prove semantic correctness of extracted claims/methods/tables, nor does it establish import readiness.

## Findings

### 1. Zero import eligibility is handled correctly

Status: PASS

The structured evidence consistently reports:

- `import_eligible_chunk_count=0`
- `import_ready_count=0`
- `refused_chunk_count=4289`
- `production_import_attempted=false`
- `ladybugdb_written=false`

This is correctly framed as a safety boundary: broader Markdown readiness and more chunks do not imply trusted KG readiness.

### 2. Patterns are useful for automation routing, not semantic validation

Status: FLAG

Route shifts are meaningful beyond raw chunk counts because they expose recurring review categories: retrieval-only remains dominant, method candidates increase, table/citation/figure/claim routes become more visible, and refusal reasons remain route-specific.

However, because the evidence intentionally excludes raw chunk text, embeddings, optimizer traces, and raw Markdown, it cannot validate whether individual claim/method/table chunks are semantically useful or correctly classified.

Required wording for final recommendation:

> These patterns are useful for routing and review prioritization, but they do not validate the semantic correctness of extracted candidates.

### 3. Source-readiness language must be narrowed

Status: FLAG

Use `Markdown-scan readiness`, not full `source readiness`. The scanner considered all 30 papers ready for Markdown-based scanning, but source/PDF completeness remains partial and diagnostic risk tags still record missing Markdown/PDF history for many expansion papers. Treat this as Markdown-scan readiness, not full source readiness.

### 4. Baselines must stay separate

Status: FLAG

Use these labels consistently:

- **M005/S03 structure-aware baseline:** apples-to-apples route/chunk comparison.
- **M005/S06 mixed benchmark:** non-equivalent import-boundary context only.

Do not present the S06 2,471-candidate value as a generic baseline without qualification.

### 5. Outlier flags are useful but need method disclosure

Status: FLAG

The outlier list is useful for review prioritization. The final recommendation should define the current thresholds:

- `high_chunk_count`: chunk count at least max(2 × median chunk count, median + 25).
- `claim_candidate_heavy`: at least 25 claim-route chunks.
- `table_heavy`: at least 10 table-route chunks.

Future automation should also include normalized density such as chunks per 10k bytes.

## Required Corrections

Before using S03 evidence as final planning evidence:

1. Replace broad `source readiness complete` language with `Markdown-scan readiness according to the scanner`.
2. Note that missing Markdown/PDF risk tags may reflect historical acquisition state and need contradiction checks in automation.
3. Clearly separate M005/S03 structure-aware and M005/S06 mixed benchmark comparisons.
4. Soften causal/generalization language around route shifts.
5. Define outlier thresholds and include normalized density in future reports.
6. State explicitly that evidence supports automation routing and refusal-boundary planning, not semantic correctness or KG import readiness.

## Recommended automation requirements

For the +10-to-100 CLI milestone:

1. Batch preflight for paper IDs and source state, with Markdown/PDF/repair/unavailable tracked separately.
2. Deterministic +10 selection with selection role persisted.
3. Structured run summary with route counts, state counts, refusal counts, import eligibility, and safety flags.
4. Route-share delta reporting against previous batch, cumulative corpus, and M005/S03 baseline; keep S06 separate.
5. Documented outlier thresholds with absolute counts and normalized density.
6. Strict import gate: non-zero import eligibility must block for review unless produced by a separate reviewed promotion path.
7. No raw content in machine evidence.
8. Contradiction checks for states like Markdown-ready while risk tags still say missing Markdown.

## Bottom line

The evidence justifies planning the +10-to-100 deterministic CLI automation milestone if it is framed around repeatable scan orchestration, source-readiness accounting, route/refusal diagnostics, and review prioritization. It must not be framed as evidence that extracted candidates are semantically correct or KG-importable.
