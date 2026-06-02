# M027 S06 Provenance And Riskratchet Gate Report

- status: `passed`
- verification_result: `passed`
- milestone_id: `M027-aakeky`
- slice_id: `S06`
- selection_id: `m027-mixed-source-corpus-v1`
- network_fetch_attempted: `False`
- production_import_attempted: `False`
- graph_import_allowed: `False`
- ladybugdb_written: `False`

## Provenance
- Command: `scripts/verify_m027_provenance_and_riskratchet_gate.py`
- CWD: `/root/daily-archive`
- Git commit: `0c13520055d444e42600a606b5fb7376640826cd`
- Self hash excluded: `True`
- Self hash reason: summary output provenance is self-referential; non-summary outputs are hash-enforced validate-only

## Riskratchet
- Diagnostic only: `True`
- Blocking: `False`
- Pass/fail affected: `False`
- Tool status: `ok`
- Max score: `73.66`

## Diagnostics
- None.

## Failure Modes
- Filesystem: missing/malformed S05 and S06 artifacts, unsafe paths, stale hashes, and unreadable reports emit diagnostics with artifact_path/json_path and fail closed.
- Network: intentionally absent; URL artifact paths are rejected and network safety flags remain false.
- Subprocess/API: riskratchet uses the local Python wrapper; unavailability is warning telemetry, but absent/malformed telemetry artifacts fail validation.
- Graph/database: intentionally absent; graph/import/LadybugDB flags remain false.

## Load Profile
At 10x the six-article corpus, local file hashing and report size saturate first and grow linearly; riskratchet remains a fixed explicit Python scope. Protection is safe relative path validation, local-only hashing, no network/database/graph writers, and diagnostic-only telemetry.

## Negative Tests
- `tests/test_m027_provenance_and_riskratchet_gate.py::test_gate_generates_happy_path_and_self_hash_exclusion` covers happy path generation, self-hash exclusion, provenance, safety flags, and maintainability outputs.
- `tests/test_m027_provenance_and_riskratchet_gate.py::test_validate_only_reads_existing_outputs_without_rerunning_riskratchet` covers validate-only readback without rewriting/rerunning riskratchet.
- `tests/test_m027_provenance_and_riskratchet_gate.py::test_validate_only_reports_missing_and_malformed_artifacts` covers missing/malformed JSON/JSONL artifacts.
- `tests/test_m027_provenance_and_riskratchet_gate.py::test_gate_rejects_unsafe_flags_redaction_riskratchet_and_paths` covers unsafe safety flags, raw payload sentinel leakage, blocking/pass-fail riskratchet telemetry, invalid artifact paths, and stale output hashes.
