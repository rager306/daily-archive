# M133 Unknown Bucket Reduction

Schema: `daily-archive-m133-unknown-reduction.v1`

Unknown bucket: `81` -> `77` (`-4`).

| Path | New bucket | Note |
|---|---|---|
| `tests/test_replay_m028_smoke_closeout.py` | `script-wrapper` | baseline-green focused sample |
| `tests/test_m036_real_corpus_no_write_smoke.py` | `script-wrapper` | baseline-green focused sample |
| `tests/test_m036_real_corpus_smoke_audit.py` | `script-wrapper` | baseline-green focused sample |
| `tests/test_m052_s02_e2e.py` | `script-wrapper` | baseline-red; no runtime edits in M133 |

## Blockers

`tests/test_m052_s02_e2e.py` is correctly reclassified as script-wrapper by import shape, but remains runtime baseline-red and must be fixed separately before any runtime ratchet work.
