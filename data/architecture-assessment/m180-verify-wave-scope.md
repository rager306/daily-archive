# M180 Verify Wave Scope

## Decision

Select two exact residual verify families for M180: **M031 parser continuity verification outputs** and **M033 parser architecture verification outputs**.

## Expected movement

```text
script-only: 142 -> 122
verify-m031-output: 0 -> 10
verify-m033-output: 0 -> 10
total movement: 20
```

## Exact source paths

### M031 parser continuity verification outputs

- `scripts/verify_m031_s05_closeout.py`
- `scripts/verify_m031_validation_remediation.py`
- `scripts/verify_m031_process_continuity_audit.py`
- `scripts/verify_m031_chunk_evidence_replay.py`
- `scripts/verify_m031_parser_conversion_replay.py`

### M033 parser architecture verification outputs

- `scripts/verify_m033_combined_parser_architecture.py`
- `scripts/verify_m033_external_parser_quality_plan.py`
- `scripts/verify_m033_grobid_probe.py`
- `scripts/verify_m033_opendataloader_adaptix_adapter.py`
- `scripts/verify_m033_quantmind_pattern_study.py`

## Rejected alternatives

- `verify_m029` is a good future candidate but is not included in M180 to keep this wave bounded while CI and cache work are also included.
- Generic `verify_m*` classification remains rejected.
- Broad `verify_m031*` or `verify_m033*` prefix rules remain rejected; implementation must list exact paths.
