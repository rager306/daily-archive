# M134 M052 Repair

Schema: `daily-archive-m134-m052-repair.v1`

Final status: `baseline-green`

## Repair summary

Updated M052 fixture construction to current typed extraction schema fields and removed an obsolete normal scripts import missing-import suppression from the test.

## Field mapping

| Legacy field | Current field |
|---|---|
| `Claim.id` | `Claim.claim_id` |
| `Claim.paper_id` | `Claim.source_id` |
| `ScientificEntity.id` | `ScientificEntity.entity_id` |
| `ScientificEntity.paper_id` | `ScientificEntity.source_id` |
| `ScientificEntity.label` | `ScientificEntity.canonical_name` |
| `ScientificRelation.id` | `ScientificRelation.relation_id` |
| `ScientificRelation.paper_id` | `ScientificRelation.source_id` |
| `ScientificRelation.source_id` | `ScientificRelation.from_entity_id` |
| `ScientificRelation.target_id` | `ScientificRelation.to_entity_id` |
| `ExtractionPatch.paper_id` | `ExtractionPatch.source_id` |
| `relation_type=supports` | `relation_type=SUPPORTS` |

## Verification

- `uv run pytest tests/test_m052_s02_e2e.py` -> `7 passed`
- Ruff -> passed
- Pyrefly -> `0 errors`

## Deferred
- No test architecture allowlist ratchet in M134. M052 can be considered in a future ratchet now that it is baseline-green.
- Two existing pyrefly bad-assignment suppressions remain in scripts/m052_rlm_e2e.py and were not part of missing-import/schema repair.
