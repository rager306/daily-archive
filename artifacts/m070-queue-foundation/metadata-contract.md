# M070 S01 T01 Queue Metadata Contract

## Decision

Extend `UniversalKBQueue` with a backward-compatible JSON metadata column on `jobs`, exposed as `job["payload_metadata"]`.

This is safer than adding many nullable columns because existing queue rows and tests can keep their current lifecycle shape, while future M069-derived metadata can evolve behind a versioned JSON contract.

## Required keys

The default payload metadata must include:

- `schema_version`
- `stable_id_version`
- `metric_bundle_id`
- `extractor_version`
- `prompt_program_hash`
- `source_artifact_refs`
- `evidence_path_refs`
- `cost_estimate`
- `latency_ms`
- `retry_count`
- `diagnostics`
- `write_eligibility`
- `promotion_eligibility`

## Safety defaults

- `write_eligibility` defaults to `false`.
- `promotion_eligibility` defaults to `false`.
- `source_artifact_refs` and `evidence_path_refs` are metadata refs only, not raw text.
- Secret-shaped values and forbidden diagnostic keys are rejected.
- The metadata contract never changes `SafetyFlags`; graph writes and promotion remain disabled.

## Backward compatibility

- Existing callers of `enqueue` do not need to pass metadata.
- Existing rows without metadata should return the same default payload metadata shape.
- Existing lease, retry, dependency, and failure APIs should not change status semantics.
- SQLite migration should add a JSON text column only if it is missing.

## Initial API shape

`enqueue(..., payload_metadata: Mapping[str, Any] | None = None)` stores a sanitized payload metadata document.

The stored metadata is merged with safe defaults, so callers may pass only the fields they know.

## Future diagnostics API

S02 may add `update_payload_diagnostics(...)` or similar to update metadata-only diagnostics such as parse validity, schema validity, evidence-path validity, cost, latency, retry count, and low-quality output status without changing job lifecycle state.

