# S03: MiniMax structured helper boundary — UAT

**Milestone:** M017-cf3fd0
**Written:** 2026-05-21T06:28:32.512Z

# S03: MiniMax structured helper boundary — UAT

## Result

- Added pure helper module: `arxiv_archive.minimax_structured`.
- Uses Anthropic-compatible endpoint shape: `POST /anthropic/v1/messages`.
- Requires forced tool calls with `input_schema`.
- Validates `tool_use.input` locally.
- Rejects prompt-only JSON.
- Rejects `payload_class=raw_corpus`.
- Rejects `temperature=0`.
- Marks output as helper evidence only, not source-of-truth.

## Verification

```text
3 passed
All checks passed!
minimax-structured-helper-guard-ok
```

## Safety

No live call was performed, no raw model content was persisted, and no KG import/write path was enabled.
