# M184 Replay Conversion Audit

## Verdict

**Movement decision: move both replay/conversion residual records by exact source path.**

## Baseline

```text
script-only=47
unknown=0
shared-state=0
```

## GitNexus

- `_classify` impact: UNKNOWN, not safety proof.
- GitNexus surfaced active replay and conversion flows including `replay_end_to_end`, `replay_baseline`, and `run_m029_unified_replay` tests.
- S06 avoids runtime code movement. The extraction pilot remains deferred to S09 after more evidence.

## Candidate decisions

| Path | Records | Decision | Category |
|---|---:|---|---|
| `scripts/replay_m025_article_loader.py` | 1 | Move | `replay-conversion-output` |
| `scripts/run_m029_unified_replay.py` | 1 | Move | `replay-conversion-output` |

## Boundaries

- No broad `replay`, `convert`, `path`, or `fd` rule.
- No runtime code movement in S06.
- These are reviewed process-boundary replay/conversion outputs.
