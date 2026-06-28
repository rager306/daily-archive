# M191 Expected Parser Outputs

## Verdict

**Expected parser readiness outputs are locked before execution. S03 must compare observed results to this contract and must not broaden parser readiness beyond the selected M029/M031 surfaces.**

## Contract inputs

- Parser command map: `data/architecture-assessment/m191-parser-command-map.md`
- M190 bounded execution summary: `data/architecture-assessment/m190-bounded-metrics-execution-summary.md`

## Selected bounded parser expansion surfaces

- M029 unified readiness verifier.
- M031 catalog-backed replay verifier.
- Parser replay use case and adapter tests.
- M031 catalog-backed acquisition loader tests.
- Focused low-quality source criteria tests.

## Expected command outcomes

| Gate | Expected outcome | Fail condition |
|---|---|---|
| M029 unified readiness verify | Command exits 0 and writes `m191-m029-readiness-verify-summary.json` | Network allowed, import flags true, missing provenance/dedupe checks, malformed readiness summary |
| M031 catalog-backed replay verify | Command exits 0 and writes summary, diagnostics, and report under `data/architecture-assessment/` | Parser/conversion/chunk readiness claimed, graph/import/production flag true, metadata leakage, stale hash/path escape |
| Parser replay tests | Tests pass | Low-quality source parsed as success, source diagnostics missing, adapter contract failure |
| M031 catalog-backed loader tests | Tests pass | Missing blocked row, unsafe true flags, stale hash, path escape, metadata leakage |
| Low-quality source criteria tests | Tests pass | Low-quality source accepted as parser success without typed fallback reason |
| GitNexus detect_changes | LOW or expected artifact-only changes | Source symbol changes without exact impact analysis, HIGH/CRITICAL risk |

## Required observed labels

S03 execution summary must report:

- `m029_readiness_verified`
- `m031_catalog_backed_replay_verified`
- `parser_low_quality_fail_closed`
- `parser_source_diagnostics_present`
- `parser_ready_scope`
- `chunk_ready_scope`
- `metadata_only_contract_preserved`
- `graph_import_ready=false`
- `production_persistence_ready=false`
- `optimizer_enabled=false`
- `direct_extractor_to_graph_write=false`

## Allowed claims

M191 may claim only:

- parser readiness evidence expanded from M190 M027 local scope to bounded M029 readiness artifacts and M031 catalog-backed replay metadata contracts;
- low-quality source handling remains fail-closed;
- parser replay contracts pass in existing tests;
- graph/import/production/optimizer remain false.

## Disallowed claims

M191 must not claim:

- broad production parser readiness;
- semantic KG readiness;
- graph import readiness;
- production persistence readiness;
- production retrieval quality;
- DSPy/RLM optimizer readiness;
- import eligibility from M031 metadata-only review contracts.

## Stop conditions

Stop and mark needs-attention if any execution result shows:

- low-quality source parsed as success;
- non-substantive arXiv navigation markdown treated as substantive body text;
- parser-ready article without source diagnostics;
- graph/import flag true;
- production persistence flag true;
- optimizer invocation;
- metadata leakage or raw source payload in diagnostics;
- path escape or stale hash in catalog-backed replay;
- GitNexus HIGH or CRITICAL risk.
