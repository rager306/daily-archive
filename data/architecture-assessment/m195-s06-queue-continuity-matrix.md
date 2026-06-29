# M195 S06 Queue Continuity Matrix

## Verdict

**PASS: queue continuity has explicit stage, status, failure, artifact, and event surfaces; remaining gaps are downstream orchestration choices, not hidden queue state.**

## Evidence

| Check | Result | Evidence |
|---|---|---|
| Constants and event extraction | PASS | `gsd_exec[1785f321-1324-497b-afbf-3d5cff739571]` |
| Derived active status import | PASS: 7 active statuses | `gsd_exec[ce0cd523-79ef-4b4b-b791-909acb80bc33]` |
| S05 final compatibility | PASS: 37 passed | `gsd_exec[4e4c062b-c48c-4365-b2c9-94aede1665be]` |

## Lifecycle states

| Category | Values |
|---|---|
| All statuses | `pending`, `ready`, `running`, `succeeded`, `failed_retryable`, `failed_terminal`, `blocked`, `stale`, `needs_review`, `skipped` |
| Terminal statuses | `succeeded`, `failed_terminal`, `skipped` |
| Active statuses | `pending`, `ready`, `running`, `failed_retryable`, `blocked`, `stale`, `needs_review` |

## Pipeline stages

- `intake`
- `acquisition`
- `parsing`
- `chunking`
- `evidence`
- `graph_candidate`
- `projection_rehearsal`

## Failure classes and retryable codes

| Surface | Values |
|---|---|
| Classes | `network`, `source`, `resource`, `llm`, `artifact`, `schema`, `review`, `validation`, `queue` |
| Retryable codes | `network_unavailable`, `arxiv_unavailable`, `rate_limited`, `resource_limit`, `llm_limit`, `queue_dispatch_error`, `stage_dispatch_error` |
| Non-retryable representative codes | `stale_hash`, `low_quality_source`, `source_missing`, `source_empty`, `no_substantive_body`, `partial_artifact`, `schema_validation_failed`, `missing_review_packet`, `incomplete_review_packet` |

## Event surfaces

- `enqueue`
- `payload_diagnostics_update`
- `block`
- `unblock`
- `claim`
- `heartbeat`
- `complete`
- `fail_retryable`
- `fail_terminal`
- `lease_expired`
- `reclaim`
- `stale_input`
- `stale_tool`
- `stale_contract`
- `artifact_registered`

## Continuity coverage matrix

| Continuity concern | Current mechanism | Test coverage |
|---|---|---|
| Idempotent enqueue | `job_id` primary key and single `enqueue` event | `test_enqueue_is_idempotent_and_records_event` |
| Dependency gating | `job_dependencies` plus `_dependencies_satisfied` | `test_artifact_dependency_unblocks_only_after_expected_hash_is_registered` |
| Artifact hash safety | `artifact_refs` exact ref/hash match, no payload reads | `test_artifact_dependency_unblocks_only_after_expected_hash_is_registered` |
| Missing artifact hash fail-closed | no `expected_hash` means blocked | `test_artifact_dependencies_without_hash_do_not_unblock_job` |
| Retry resume timing | `failed_retryable` plus `retry_after` | `test_retryable_failure_respects_retry_after_before_claim` |
| Lease recovery | expired running leases reclaim to `ready` or terminal after attempts | `test_expired_lease_reclaims_to_ready_until_attempts_exhausted` |
| Stale input/tool/contract | `mark_stale` emits `stale_*` events and status `stale` | `test_mark_stale_detects_input_tool_and_contract_drift` |
| Metadata-only diagnostics | sanitizer rejects raw refs, forbidden keys, secrets | queue diagnostics tests |
| No-write safety | `SafetyFlags` remain false in queue rows and rehearsal | rehearsal/substrate tests |

## Gaps intentionally deferred

- No distributed scheduler.
- No artifact payload hashing; callers provide metadata hashes.
- No live arXiv/network/LLM retry implementation.
- No graph backend adapter writes.
- No production import eligibility.

## Boundary statement

S06 T02 is artifact-only. It consolidates the continuity state created by S02-S05 and does not change source behavior.
