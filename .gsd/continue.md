# Handoff — 2026-08-11 (M380 unified read-only contract delivered)

## Delivered milestone

**M380-ttiau7 Unified Read Only Query Contract** — product commit `821bbf7a`.

## Fixed security and semantic gap

Before M380:
- RemoteGraphStore query_readonly validated Cypher locally.
- Embedded SamyamaGraphStore query_readonly called unrestricted execute_cypher directly.
- `da graph-query --mode embedded` could therefore mutate despite its read-only product contract.

After M380:
- one adapter-neutral `kg_storage::validate_readonly_cypher` implementation;
- embedded and remote query_readonly both validate before engine/network;
- trusted full GraphStore query is unchanged;
- common adapter-neutral rejection messages.

## Policy contract

Accepted corpus:
- MATCH, RETURN, WITH, UNWIND;
- mutation words inside quoted strings, comments and backtick identifiers.

Rejected:
- CREATE, MERGE, DELETE, DETACH, SET, REMOVE, DROP, CALL, LOAD, FOREACH, USE;
- unterminated quotes/block comments;
- input without explicit read clause.

## Verification

- universal policy tests: 2;
- remote adapter tests: 8;
- embedded Samyama no-write: 1 with `--features samyama`;
- da-cli graph-task tests: 31;
- real revision preparation tasks: 4/4 PASS;
- graph-query tests: 2;
- kg-storage all-target clippy: zero project errors;
- full architecture guardrail: PASS;
- governor: zero violations;
- application logs: no ERROR/panic; recent GSD failures zero.

Embedded no-write proof:
- node/edge totals unchanged;
- Paper identity unchanged;
- before/after exports both reload into fresh stores with same totals/identity;
- Samyama export bytes are nondeterministic across unchanged exports, so raw byte equality is not claimed.

CLI parity proof:
- embedded and remote read result identical: revisions [1,5];
- CREATE rejected in both modes with same policy text;
- both write attempts exit 3;
- source snapshot file SHA unchanged;
- live daemon gracefully stopped and bg_shell reports `exited`, not crashed.

## Runtime UAT

- S01 `d269d243-45a2-486f-8987-3e179787c7f4`
- S02 `c48e1647-952b-40f8-a4fb-d5055a1727b8`
- S03 `43402161-fca8-4669-a977-2447eeecc58f`

## GitNexus and Codebase Memory

- Exact embedded query_readonly impact was CRITICAL: 13 symbols, 11 flows, including graph-task/finalize/ontology/readiness canaries.
- Expanded regression/full architecture matrix passed.
- GitNexus reindexed after structural change; final detect/reindex used.
- Codebase Memory MCP timed out; supplemental unavailable.

## Governor assessment

No new governor regex. The strongest guard is now one universal policy plus adapter contract tests and physical no-write evidence. Consider a DATA-backed governor check only if a future adapter implements query_readonly without calling the universal policy; first add a negative fixture.

## Next product gaps

1. Isolate vendor Samyama working tree before server-side protocol changes.
2. Add server-enforced RO_QUERY and typed response envelope under explicit vendor ownership.
3. Address nondeterministic snapshot export if byte-reproducible exports become a product requirement; currently semantic reload equivalence is the honest contract.
4. Do not expand HOT remote writes until atomic mutation/temporal/transaction semantics exist.

## Locks

D127 false; Samyama sole store; no hidden fallback/auto-start; remote scalar reads only; ADR-057 Proposed; ADR-058 Accepted; DSPy disabled.
