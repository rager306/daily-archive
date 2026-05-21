# S02: MiniMax usage limit helper — UAT

**Milestone:** M017-cf3fd0
**Written:** 2026-05-21T06:12:39.514Z

# S02: MiniMax usage limit helper — UAT

## Result

- Added pure helper module: `arxiv_archive.minimax_usage`.
- Endpoint order follows M016/9router.
- Canonical key is `MINIMAX_API_KEY`; `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` are aliases.
- Usage/remains auth uses `Authorization: Bearer <MINIMAX_API_KEY>`.
- Success requires provider `base_resp.status_code == 0` plus quota rows.
- `token_plan/remains` count means used.
- `coding_plan/remains` count means remaining.
- Sanitized outputs exclude raw response bodies, exact quota values, and credential values.

## Verification

```text
5 passed
All checks passed!
minimax-usage-helper-guard-ok
```

## Safety

No live call was performed, no KG import/write path was enabled, and MiniMax remains a bounded helper only.
