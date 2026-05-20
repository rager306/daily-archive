# M012 integration boundary matrix

## Summary

Both DSPy and MiniMax are conditionally compatible only for future bounded probes. Neither is ready for production process activation.

| Technology | Safe role | Current status | Next safe probe | Production activation |
|---|---|---|---|---|
| DSPy | Optional/dev extraction-program boundary over `ExtractionPatch` | `blocked_missing_dependencies` | `optional_dev_dependency_probe_no_lm` | blocked |
| MiniMax | Optional bounded helper/reviewer over redacted metadata | no-call payload dry run complete | `explicitly_approved_synthetic_auth_smoke_test` | blocked |

## DSPy

- Go for optional/dev prototype: `True`
- Go for production runtime: `False`
- Optimizer enabled: `False`
- Import available now: `False`
- Main blocker: missing dependency / no completed no-LM runtime probe.

## MiniMax

- Go for optional helper probe: `True`
- Live call attempted: `False`
- Key present: `True`
- Orchestrator allowed: `False`
- Main blocker: live auth/header + schema reliability not yet proven by explicit approved call.

## Shared constraints

- chunk_span_provenance_and_candidate_locators_required_before_positive_import
- no_raw_paper_or_chunk_text_in_machine_artifacts
- no_embeddings_or_vectors_in_artifacts
- no_secrets_in_logs_or_artifacts
- production_import_and_ladybugdb_writes_blocked
- bounded_probe_before_pipeline_activation
- explicit_go_no_go_decision_required

## Post-M011 connection

M011 proved that semantic import remains blocked until chunk-level span provenance and candidate locators exist. DSPy and MiniMax can prepare future helpers around that evidence, but neither removes the need for span/candidate-locator packets.

## Interpretation

The infrastructure principle is upheld: research and dry-run probes before activation. The next implementation milestone should not enable either tool in the production pipeline. It should either run a DSPy optional dependency no-LM probe, a MiniMax synthetic auth/header smoke test, or build the chunk-span provenance packet required by M011.
