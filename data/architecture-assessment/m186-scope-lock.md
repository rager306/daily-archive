# M186 Scope Lock

## Wave sequence

1. **S01 Scope baseline**: GitNexus and guardrail evidence.
2. **S02 to S07 Verifier wave**: contract-first M025/M031 movement or no-move proof.
3. **S08 to S14 Manifest lifecycle wave**: lifecycle contract, atomicity model, residual pilots, integration gate.
4. **S15 to S17 Ratchet and guard wave**: meaningful wrapper/lifecycle ratchets and integrated architecture guards.
5. **S18 to S19 Closeout wave**: quality stack, GitNexus, status hygiene, validation-ready summary.

## Dependencies

- Verifier movement depends on S02 contract tests.
- M025 movement depends on S04 catalog safety contract.
- M031 evidence builder movement depends on S03 path primitive pilot.
- Manifest pilots depend on S08 lifecycle contract and S09 atomic writer model.
- Ratchet expansion depends on completed verifier and manifest outcomes.

## Non-goals

- No production feature expansion.
- No DSPy/RLM/optimizer changes.
- No direct extractor-to-graph write.
- No broad write-path classification by generic names like `path`, `output_path`, `json_path`, `manifest`, or `cache`.
- No `.gsd/*` commits.

## Execution rule

Before editing any function, class, or method, run `gitnexus_impact` for the exact symbol. If impact is HIGH or CRITICAL, stop and ask before editing.
