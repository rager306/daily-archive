# M131 M061 S02 Fixture Diagnosis

Schema: `daily-archive-fixture-diagnosis.v1`

## Reproduced failure

```bash
uv run pytest tests/test_m061_s02.py
```

Observed: `1 failed, 5 passed`.

Failing test:

```text
tests/test_m061_s02.py::test_m050_m064_s01_regression
```

The first failing assertion compares `S01_DECISION_SHA256` with the current git blob SHA for `artifacts/m061-2hop/s01-decision.md`.

## Hashes

| Artifact | Expected in test | Current tracked blob |
|---|---|---|
| `artifacts/m061-2hop/s01-decision.md` | `7ed6c71c955c8a9de1e52e143b7a62c9c79ae26d` | `9e6280aee19244251e6fd195c07ae07e5d9fec80` |
| `artifacts/m061-2hop/anchor-2605.18747/pipeline-summary.json` | `965a8f7103699f3c95e8e5cf179d75e82994fa1b` | `bcacdae1c0c4da78a7f2c071c94c9d6403006274` |

`git ls-files --stage` reports the same current blob IDs for both artifacts, and `git status` shows no local artifact modifications. The stale values are in the test constants.

## Content review

- `s01-decision.md` still says `**GO to S02.**`.
- The decision remains scoped to M061 S01 / anchor `2605.18747`.
- Safety defaults remain false, with scoped external-network override text preserved.
- The pipeline summary still reports:
  - `anchor_arxiv_id=2605.18747`
  - `generated_by=scripts/m061_anchor_pilot.py`
  - `real_arxiv_downloaded_pdf_count=30`
  - `real_arxiv_downloaded_eprint_count=30`
  - `fully_processed_real_paper_count=30`
  - `http_429_count=0`

## Recommendation

Update only the expected SHA constants in `tests/test_m061_s02.py` to the current tracked artifact blob IDs. Do not modify generated M061 artifacts in this milestone, and do not combine this repair with dynamic-import ratcheting.
