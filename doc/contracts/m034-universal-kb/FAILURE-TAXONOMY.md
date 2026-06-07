# M034 Failure Taxonomy

## Failure Classes

| Class | Retry? | Examples |
|---|---|---|
| `retryable` | yes | network_unstable, backend_temporarily_unhealthy, rate_limited |
| `terminal` | no | unsupported_source_type, unsafe_path, malformed_contract |
| `blocked` | no until external action | model_cache_missing_no_network, missing_local_source, needs_user_decision |
| `stale` | rerun after refresh | stale_source_hash, stale_tool_version, stale_review_contract |
| `needs_review` | human/reviewer action | review_packet_incomplete, semantic_review_required |

## Error Codes

| Code | Class | Notes |
|---|---|---|
| `missing_local_source` | blocked | Required source artifact absent. |
| `unsafe_path` | terminal | Path outside allowed corpus/project boundary. |
| `low_quality_source` | terminal/needs_review | Non-substantive content or failed conversion quality. |
| `backend_unhealthy` | retryable/blocked | Sidecar backend unavailable depending on duration. |
| `model_cache_missing_no_network` | blocked | Hybrid/model backend cannot start without cache. |
| `network_unstable` | retryable | Bounded network mode only. |
| `tei_parse_failed` | retryable/terminal | GROBID output malformed or unsupported. |
| `layout_quality_below_gate` | needs_review | OpenDataLoader/layout output below quality threshold. |
| `table_fidelity_below_gate` | needs_review | Table reconstruction suspect. |
| `adaptix_mapping_failed` | terminal/needs_review | Structural mapping failed. |
| `invalid_evidence_path` | terminal | Evidence reference cannot resolve. |
| `review_packet_incomplete` | needs_review | Review packet lacks required fields. |
| `graph_readiness_postcheck_failed` | blocked | Review post-check failed. |

## Diagnostic Redaction

Failure records expose codes and JSON-pointer-style paths only, never payload values or raw corpus text.
