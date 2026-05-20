# M012 final compatibility recommendation

## Verdict

**PASS as compatibility research. No production activation.**

M012 successfully retired the first layer of DSPy and MiniMax unknowns. Both technologies are plausible future helpers, but only behind explicit bounded probes and safety wrappers.

## DSPy verdict

- Verdict: `conditional_go_optional_dev_probe_only`
- Import available now: `False`
- Compatibility status: `blocked_missing_dependencies`
- Production runtime allowed: `False`
- Optimizer allowed: `False`

DSPy should proceed only to an optional/dev dependency no-LM probe. It must preserve `ExtractionPatch` as the authoritative schema and keep optimizers fail-closed.

## MiniMax verdict

- Verdict: `conditional_go_optional_helper_probe_only`
- Live call attempted: `False`
- Key present: `True`
- Optional helper probe allowed: `True`
- Orchestrator allowed: `False`

MiniMax should proceed only to an explicitly approved synthetic auth/header smoke test. It must remain an optional helper over redacted metadata, not source of truth.

## Still blocked

- Positive KG import
- Production LadybugDB writes
- DSPy optimizers
- DSPy production runtime activation
- MiniMax orchestration/source-of-truth behavior
- Direct PDF/raw paper ingestion through MiniMax
- Unattended scaling

## Next safe options

1. `dspy_optional_dev_dependency_no_lm_probe`
2. `minimax_explicit_synthetic_auth_smoke_test`
3. `chunk_span_provenance_candidate_locator_packet`

Recommended sequencing: if the goal is infrastructure readiness, run the DSPy no-LM dependency probe and MiniMax synthetic auth smoke test as separate small milestones/tasks. If the goal is KG import readiness, prioritize the chunk-span provenance and candidate-locator packet from M011.
