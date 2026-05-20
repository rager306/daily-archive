# S01: 9router MiniMax usage algorithm — UAT

**Milestone:** M016-9819d1
**Written:** 2026-05-20T12:38:28.201Z

# S01: 9router MiniMax usage algorithm — UAT

## Result

- 9router cloned under `/root/vendor-source/9router`.
- GitNexus indexed repo `9router`.
- Global MiniMax endpoint order documented.
- CN MiniMax endpoint order documented.
- M015 missed global fallback endpoint identified.
- Success criteria documented: `base_resp.status_code == 0` plus `model_remains` quota rows.
- Count semantics documented: token_plan means used counts; coding_plan means remaining counts.

## Meaning

S02 can now probe MiniMax limits using the same endpoint order and parser rules as 9router.
