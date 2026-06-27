# M184 Replay Conversion Result

## Verdict

**Replay-conversion exact wave: PASS.**

## Movement

```text
script-only: 47 -> 45
replay-conversion-output: 0 -> 2
render-report-contract-output=8
audit-analysis-output=24
source-acquisition-evidence-output=10
unknown=0
shared-state=0
total_records=341
```

## Category added

`replay-conversion-output` covers exact reviewed source paths only:

- `scripts/replay_m025_article_loader.py`
- `scripts/run_m029_unified_replay.py`

## Verification

| Check | Result | Evidence |
|---|---|---|
| Fresh baseline | PASS | `gsd_exec[b8c6793d-e7d1-475b-ab0b-13beb7ae86f8]` |
| Current record extraction | PASS | `gsd_exec[1ebaf578-323b-40b8-96a0-ac95bb56d719]` |
| Candidate file record check | PASS | `gsd_exec[93b9a889-1bd2-47f3-87f6-ff1080797d1c]` |
| Focused tests after scanner movement | PASS: 36 passed | `gsd_exec[026d9573-d9dc-4726-b474-2894148f01c5]` |
| Ruff scanner and tests | PASS | `gsd_exec[36ed2d3e-bb35-4ca1-86a1-f4ae634b9f11]` |
| Generated delta before canonical refresh | PASS | `gsd_exec[d14af911-d271-456e-ad42-e0ed0b688ad3]` |
| Canonical refresh, lowered ratchet, strict drift | PASS | `gsd_exec[3497b929-31f1-4470-83c1-5ea4221461f2]` |

## Guardrails

- No broad replay/convert/path/fd rule.
- No runtime code movement.
- Ratchet lowered to `script-only <= 45`.
- Canonical baseline refreshed.
