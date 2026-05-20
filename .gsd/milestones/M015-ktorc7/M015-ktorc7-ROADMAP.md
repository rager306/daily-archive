# M015-ktorc7: MiniMax Limits and Structured Output Remediation

**Vision:** Correct the under-debugged M014 results by thoroughly testing MiniMax limit visibility and structured output on the right surfaces before making any verdict.

## Success Criteria

- Token Plan limit-check path is debugged with a proper matrix, not one failed call.
- MiniMax structured-output behavior is debugged on recommended Anthropic-compatible and tool-call APIs.
- The final verdict is corrected and specific.
- No raw sensitive/project/model content is persisted.
- No production KG import, LadybugDB writes, or source-of-truth role is enabled.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: After S01, we know which key/header/method combinations work or fail for Token Plan remains and whether a Token Plan Key is needed.

- [x] **S02: S02** `risk:medium` `depends:[]`
  > After this: After S02, we know whether MiniMax supports structured output reliably via prompt JSON, reasoning split, response_format, or tool calls.

- [ ] **S03: Corrected MiniMax verdict** `risk:medium` `depends:[S01,S02]`
  > After this: After S03, M015 gives corrected recommendations and updates R043.

## Boundary Map

| Area | In scope | Out of scope |
|---|---|---|
| Token Plan limits | Endpoint/key/method/header matrix and secure Token Plan Key collection if needed | Manual secret paste or browser account mutation |
| Structured output | Anthropic-compatible recommended API, tool calls, OpenAI reasoning_split/response_format probes | Production extractor integration |
| Payloads | Synthetic/redacted metadata only | Raw paper/PDF/chunk text |
| Verdict | Corrected MiniMax readiness assessment | Source-of-truth or KG import approval |
