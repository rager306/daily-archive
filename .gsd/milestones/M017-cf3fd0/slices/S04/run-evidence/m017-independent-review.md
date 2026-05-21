# M017 independent review

## Review scope

Files reviewed:

```text
src/arxiv_archive/minimax_usage.py
src/arxiv_archive/minimax_structured.py
tests/test_minimax_usage.py
tests/test_minimax_structured.py
.gsd/milestones/M017-cf3fd0/slices/S02/run-evidence/minimax-usage-helper-guard.json
.gsd/milestones/M017-cf3fd0/slices/S03/run-evidence/minimax-structured-helper-guard.json
```

## Reviewer result

Reviewer verdict: `PASS`.

Reviewer focus:

- endpoint/auth/key alias semantics;
- usage/remains count semantics;
- forced tool-call schema validation;
- prompt-only JSON rejection;
- non-authoritative MiniMax boundaries.

No blocking correctness findings were reported.

## Security review result

Initial security verdict: `FLAG`.

Findings:

1. Request dataclass `repr` could leak raw API keys or prompts if helper objects are logged.
2. Raw corpus blocking relied only on caller-supplied `payload_class`, so mislabeled raw paper/chunk text could still be sent.
3. A dependency audit reported vulnerabilities in unrelated transitive ML packages (`torch`, `transformers`) outside the MiniMax helper changes.

## Remediation applied

Findings 1 and 2 were fixed before final guard:

- `MiniMaxUsageRequest` now uses `repr=False` so raw Bearer headers are not displayed by object repr.
- `MiniMaxStructuredRequest` now uses `repr=False` so raw prompt text is not displayed by object repr.
- `build_minimax_structured_request` now rejects obvious raw corpus markers even when `payload_class="redacted"` is supplied.
- Regression tests assert that secret/prompt sentinels do not appear in `repr(...)` or sanitized dicts.

Finding 3 is recorded as broader dependency debt, not introduced by M017 MiniMax helper. It should be handled in a separate dependency/security milestone if those packages are used in active runtime paths.

## Final review verdict

`PASS WITH NOTED DEPENDENCY DEBT`

M017 helper boundaries are acceptable for dev-only bounded use after remediation. The helper still does not perform live calls, production KG import, LadybugDB writes, or MiniMax source-of-truth behavior.
