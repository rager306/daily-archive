# M184 Audit Analysis Result

## Verdict

**Audit-analysis exact wave: PASS.**

## Movement

```text
script-only: 79 -> 55
audit-analysis-output: 0 -> 24
source-acquisition-evidence-output=10
unknown=0
shared-state=0
total_records=341
```

## Category added

`audit-analysis-output` covers exact reviewed source paths only:

- `scripts/analyze_m056_wave_1.py`
- `scripts/analyze_m056_wave_2.py`
- `scripts/analyze_m056_wave_3.py`
- `scripts/analyze_m056_wave_4.py`
- `scripts/analyze_m056_wave_5.py`
- `scripts/analyze_m056_wave_6.py`
- `scripts/audit_locator_evidence.py`
- `scripts/audit_m042_connectivity_groups.py`
- `scripts/audit_m053_grobid_pilot.py`
- `scripts/audit_pipeline_scripts.py`
- `scripts/check_project_trajectory.py`
- `scripts/test_fd_contract.py`
- `scripts/verify_test_architecture.py`

## Verification

| Check | Result | Evidence |
|---|---|---|
| Fresh baseline | PASS | `gsd_exec[adf2eea3-a0e6-4854-b261-d1c41872c1b2]` |
| Current record extraction | PASS | `gsd_exec[3a7c895f-a08a-4541-8c1d-a6952aaf95fc]` |
| Candidate file record check | PASS | `gsd_exec[24feb3ae-c164-4175-9b38-b0783446475f]` |
| Focused tests after scanner movement | PASS: 34 passed | `gsd_exec[8aee281e-eb21-4b3c-b26c-156fb12e69d0]` |
| Ruff scanner and tests | PASS | `gsd_exec[551377d5-658a-4cbe-8c35-e5b297650ce0]` |
| Generated delta before canonical refresh | PASS | `gsd_exec[9b65b606-17a8-40c3-a95f-4bbe1af1dc57]` |
| Canonical refresh, lowered ratchet, strict drift | PASS | `gsd_exec[13935a83-bc13-4dad-b55d-caa600694b6e]` |

## Guardrails

- No broad analyze/audit/verify/test/trajectory/path/json/markdown/artifact rule.
- No runtime code movement.
- Ratchet lowered to `script-only <= 55`.
- Canonical baseline refreshed.
