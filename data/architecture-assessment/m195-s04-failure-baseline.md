# M195 S04 Failure Baseline

## Verdict

**PASS: failure taxonomy work can proceed in Universal KB contracts with LOW impact and without touching live network or graph backend code.**

## GitNexus impact

| Target | Result |
|---|---|
| `Class:src/research_graph/domain/universal_kb/contracts.py:FailureRecord` | LOW, impactedCount=26, direct=3, processes_affected=0 |
| `UniversalKBQueue.fail_retryable` | LOW, impactedCount=0, processes_affected=0 |
| `UniversalKBQueue.fail_terminal` | LOW, impactedCount=0, processes_affected=0 |
| `UniversalKBQueue.update_payload_diagnostics` | LOW, impactedCount=0, processes_affected=0 |

## Existing active failure surfaces

- `tests/test_ingestion_loader.py` covers `no_substantive_body`, `unsupported_type`, `decode_failed`, `source_empty`, and `source_missing` as typed loader failures.
- `tests/test_full_text_ingestion.py` covers `source_missing`, `source_empty`, and `low_quality_source` with `fallback_reason=no_substantive_body`.
- `src/research_graph/infrastructure/corpus/ingestion/loader.py` already models local source quality outcomes and loader failure reasons.
- `src/research_graph/workflows/universal_kb/queue.py` already rejects secret-shaped diagnostics and metadata payloads.
- `src/research_graph/domain/universal_kb/contracts.py` already has `FailureRecord` with `failure_class`, `error_code`, `retryable`, `redacted_message`, and `occurred_at`.

## Gap for M195 S04

Failure codes are present but not centrally enumerated for architecture-level pipeline categories. Later slices need stable metadata-only values for:

- network unavailable
- arXiv unavailable
- rate limited
- resource limit
- LLM limit
- stale hash
- low quality source
- partial artifact
- schema validation
- missing or incomplete review

## Minimal edit plan

Use Ponytail minimalism:

1. Add central frozenset constants in `src/research_graph/domain/universal_kb/contracts.py`:
   - `PIPELINE_FAILURE_CLASSES`
   - `PIPELINE_FAILURE_CODES`
   - `RETRYABLE_FAILURE_CODES`
2. Validate `FailureRecord.failure_class` and `FailureRecord.error_code` against metadata-code shape and the central sets.
3. Keep `redacted_message` required but do not log payload values.
4. Do not touch network clients, live arXiv access, LLM clients, queue schema, or graph code in S04.
5. Add tests proving representative failure classes/codes are accepted, unknown codes are rejected, retryability matches the retryable set, and forbidden payload keys or secret-shaped text remain rejected where queue diagnostics are persisted.

## Disallowed in S04

- No live network probes.
- No arXiv client changes.
- No LLM provider calls.
- No graph adapter or graph write.
- No queue schema migration.
- No import eligibility promotion.
