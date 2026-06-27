# M184 Render Report Result

## Verdict

**Render-report exact wave: PASS.**

## Movement

```text
script-only: 55 -> 47
render-report-contract-output: 0 -> 8
audit-analysis-output=24
source-acquisition-evidence-output=10
unknown=0
shared-state=0
total_records=341
```

## Category added

`render-report-contract-output` covers exact reviewed source paths only:

- `scripts/render_bounded_repair_prototype.py`
- `scripts/render_chunk_repair_contract.py`
- `scripts/render_m055_report.py`
- `scripts/render_m055deep_report.py`
- `scripts/render_m056_report.py`
- `scripts/render_reviewer_packet_prototype.py`

## Verification

| Check | Result | Evidence |
|---|---|---|
| Fresh baseline | PASS | `gsd_exec[37d9b1dd-f9d4-40fd-9524-cc94fd42fe1b]` |
| Current record extraction | PASS | `gsd_exec[9eb6f59e-5dec-4c36-848f-5455cb0dfb5c]` |
| Candidate file record check | PASS | `gsd_exec[dacc02fd-fc14-456e-89d3-18b1a259d70b]` |
| Focused tests after scanner movement | PASS: 35 passed | `gsd_exec[5ebc1859-228d-4ba6-bae2-073e5a3ba471]` |
| Ruff scanner and tests | PASS | `gsd_exec[d55c127d-1c04-4894-a7d7-9c3c6133b68b]` |
| Generated delta before canonical refresh | PASS | `gsd_exec[c485504c-45c3-4d67-958e-8725d23abb3d]` |
| Canonical refresh, lowered ratchet, strict drift | PASS | `gsd_exec[0c709a3e-2fe2-4250-b670-dc2a318d27ac]` |

## Guardrails

- No broad render/report/contract/output/path/temp rule.
- No runtime code movement.
- Ratchet lowered to `script-only <= 47`.
- Canonical baseline refreshed.
