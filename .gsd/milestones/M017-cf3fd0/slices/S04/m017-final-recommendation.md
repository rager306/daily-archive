# M017 final recommendation

## Verdict

`GO for dev-only bounded MiniMax helper use`

M017 successfully turns the MiniMax research and remediation findings into reusable project code without enabling production KG behavior.

## What is now available

### Usage/remains helper

Module:

```text
arxiv_archive.minimax_usage
```

Provides:

- canonical MiniMax key alias resolver;
- 9router-compatible global usage/remains request builder;
- sanitized response parser;
- token-plan vs coding-plan count semantics;
- provider `base_resp.status_code == 0` success requirement;
- sanitized diagnostics that omit secrets, raw responses, and exact quota values.

### Structured-output helper boundary

Module:

```text
arxiv_archive.minimax_structured
```

Provides:

- Anthropic-compatible forced-tool request builder;
- local `input_schema` validation for returned `tool_use.input`;
- prompt-only JSON rejection;
- raw-corpus payload class rejection;
- raw-corpus marker rejection even when mislabeled as redacted;
- fail-closed temperature validation;
- non-authoritative helper-evidence-only results.

## Review outcome

Independent reviewer: `PASS`.

Security reviewer initially flagged two implementation risks and one broader dependency issue. The implementation risks were remediated:

- request dataclass repr no longer exposes raw Bearer headers or prompts;
- raw-corpus marker checks now block obvious mislabeled raw paper/chunk/PDF payloads.

The dependency audit concern is broader project debt, not introduced by the MiniMax helper. Track separately if those vulnerable packages are used on active runtime paths.

## Safety boundaries still in force

Still blocked:

```text
production KG import
LadybugDB writes
MiniMax as source of truth
MiniMax as orchestrator
raw paper/chunk/PDF text calls
raw response persistence
exact quota value persistence
embedding/vector persistence
unattended scaling
```

## Next recommendation

The MiniMax helper path is now cemented. The next useful work is one of:

1. Comparative research-agent spike over GPT Researcher, AI-Researcher, The AI Scientist, and prismAId.
2. KG candidate locators / chunk-span provenance milestone.

If the goal is immediate KG readiness, choose candidate locators. If the goal is learning from open-source research-agent systems first, choose the comparative spike.
