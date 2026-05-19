# Validation batch CLI contract

## Purpose

M007 introduces a deterministic, resumable validation-batch CLI for iterative `+10 papers -> analyze -> review -> improve automation -> repeat` workflows toward a 100-paper diagnostic corpus.

This contract is operational only. It automates batch state, source readiness accounting, redacted scan artifacts, route/refusal deltas, outlier gates, and review handoff. It does not promote trusted KG facts.

## Safety boundary

No production KG import is allowed in M007.

The CLI and all machine artifacts must preserve these boundaries:

```text
raw_text_included=false
chunk_text_included=false
raw_binary_included=false
base64_included=false
embeddings_included=false
vectors_included=false
secrets_included=false
optimizer_traces_included=false
production_import_attempted=false
ladybugdb_written=false
```

Out of scope for M007:

- positive trusted KG import;
- production LadybugDB writes;
- semantic correctness claims for claim/method/table candidates;
- vector or embedding retrieval claims;
- unbounded PDF repair or bulk slow conversion;
- MiniMax as orchestrator or source of truth.

MiniMax may be considered later only as a bounded optional review/repair adapter after deterministic CLI artifacts exist.

## Command namespace

Target namespace:

```text
uv run python -m arxiv_archive validation-batch contract
uv run python -m arxiv_archive validation-batch init
uv run python -m arxiv_archive validation-batch preflight
uv run python -m arxiv_archive validation-batch scan
uv run python -m arxiv_archive validation-batch review
uv run python -m arxiv_archive validation-batch resume
```

S01 behavior:

- `contract` is informational and may exit successfully.
- `init`, `preflight`, `scan`, `review`, and `resume` are contract-only stubs.
- Stub workflow commands must not perform source acquisition, conversion, scanning, review mutation, KG import, or writes.
- Stub workflow commands should return non-zero so automation cannot mistake them for completed work.

## Future artifact layout

Future slices should write batch artifacts under:

```text
.gsd/milestones/M007-opaont/batches/{batch_id}/
  batch-state.json
  selection-manifest.json
  source-preflight-summary.json
  source-preflight-diagnostics.jsonl
  source-acquisition-summary.json
  validation-scan-summary.json
  validation-scan-diagnostics.jsonl
  delta-report.json
  outlier-report.json
  review-summary.md
```

S01 may define this layout but should not create real batch directories from workflow stubs.

## Phase model

A validation batch has one phase at a time:

| Phase | Meaning |
|---|---|
| `planned` | Batch is described but not initialized. |
| `initialized` | Paper IDs and selection roles are persisted. |
| `source_preflighted` | Source readiness was checked. |
| `source_ready` | Markdown-scan readiness is sufficient for scan. |
| `source_blocked` | Source gaps or contradictions block scan. |
| `scan_ready` | Inputs are ready for scan execution. |
| `scanned` | Redacted scan artifacts exist. |
| `review_required` | Automated gates require review. |
| `reviewed` | Review verdict exists. |
| `complete` | Batch is closed and next action is recorded. |

## State schema

A batch state JSON object uses schema version:

```text
m007-validation-batch-state.v1
```

Required top-level fields:

- `schema_version`
- `batch_id`
- `phase`
- `selected_papers`
- `input_manifests`
- `artifact_paths`
- `source_readiness_by_paper`
- `review`
- `recommendation`
- `safety`
- `diagnostics`

### Selected paper record

Each paper record includes:

- `paper_id`
- `rank`
- `selection_role`
- `risk_tags`
- `source_paths`
- `notes`

Allowed selection roles:

- `baseline_overlap`
- `deterministic_expansion`
- `retry`
- `repaired`
- `excluded`
- `manual_review_target`

### Source readiness record

Each source readiness record includes:

- `markdown_present`
- `markdown_quality_accepted`
- `pdf_present`
- `pdf_missing`
- `conversion_repaired`
- `conversion_failed`
- `unavailable_source`
- `ready_for_markdown_scan`

Important distinction:

- `ready_for_markdown_scan=true` means the Markdown scan can run.
- It does not mean PDF completeness, multimodal readiness, or KG import readiness.

## Diagnostics

Diagnostic records should include:

- `severity`: `info`, `warning`, or `blocker`
- `code`
- `paper_id` when applicable
- `message`
- `recommended_action`

Required contradiction diagnostics:

- `ready_for_markdown_scan=true` while `markdown_present=false`
- `ready_for_markdown_scan=true` while `markdown_quality_accepted=false`
- `ready_for_markdown_scan=true` while risk tags include `missing_markdown`
- `pdf_present=true` and `pdf_missing=true`
- `conversion_repaired=true` and `conversion_failed=true`
- `unavailable_source=true` while `ready_for_markdown_scan=true`

Unsafe safety flags are blockers.

## Delta and gate contract for later slices

Later slices should compare each batch against:

1. the previous batch;
2. cumulative corpus state;
3. M005/S03 structure-aware baseline;
4. M005/S06 mixed benchmark only as separate import-boundary context.

Gate conditions must flag or block:

- unresolved Markdown gaps;
- contradictory source readiness state;
- unexpected non-zero import eligibility;
- raw/chunk text leakage in machine artifacts;
- embeddings/vectors in machine artifacts;
- production KG import/write attempts;
- outlier or route-share spikes above documented thresholds.

## S01 acceptance

S01 is complete when:

- this contract exists;
- state helpers serialize/deserialize the schema;
- contradiction diagnostics are tested;
- the CLI exposes the namespace and safe contract-only stubs;
- focused tests and ruff pass.
