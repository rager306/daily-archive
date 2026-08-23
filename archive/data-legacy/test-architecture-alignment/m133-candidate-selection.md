# M133 Candidate Selection

Schema: `daily-archive-m133-candidate-selection.v1`

## Selected dynamic ratchet

| Path | Target | Baseline | Rationale |
|---|---|---|---|
| `tests/test_m122_mutation_smoke.py` | `script-wrapper` | 1 passed | Shortest dynamic script import debt file; imports a single script runner; baseline-green; no network or artifact mutation in test body. |

## Selected unknown reduction

| Strategy | Baseline | Sample files |
|---|---|---|
| audit classifier enhancement | 12 passed | `tests/test_replay_m028_smoke_closeout.py`, `tests/test_m036_real_corpus_no_write_smoke.py`, `tests/test_m036_real_corpus_smoke_audit.py` |

## Blockers

| Path | Reason |
|---|---|
| `tests/test_m052_s02_e2e.py` | baseline-red before any M133 edit; failure is Claim.__init__ unexpected keyword argument id in scripts/m052_rlm_e2e.py |
