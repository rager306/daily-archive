# Continue — S05

## Last action

Completed and committed M003 S04: deterministic `Claim`, `ScientificEntity`, `ScientificRelation`, and `ExtractionPatch` contracts now exist in `src/arxiv_archive/scientific_extraction.py`; final verification passed with `50 passed`, Ruff clean, CLI help smoke exit 0, LSP clean, Pyrefly 0 errors, and Ty passed. Commit: `a6413f2 feat: add scientific extraction contracts`; GitNexus was refreshed with `gitnexus analyze . --name daily-archive` and now reports `1,329 nodes | 2,130 edges | 38 clusters | 26 flows`.

## Next action

Plan S05 through GSD before executing: create tasks for LadybugDB SCI KG schema expansion that consume S04 `ExtractionPatch` and validate/write Paper, PageIndexNode, SemanticChunk, Claim, ScientificEntity, EvidencePath, and relation edges idempotently and transaction-safely.

## Why

GSD state is at `M003-km5fty / S05: LadybugDB SCI KG schema expansion`, phase `planning`, and S05 currently has no DB tasks. S05 is the required persistence bridge before S06 hybrid retrieval; DSPy/RLM work remains out of scope until later slices.

## Open threads

- S05 should start with a compact schema mapping pass: node labels, edge names, uniqueness keys, write order, rollback behavior, invalid `ExtractionPatch` rejection, and EvidencePath graph representation.
- `R018` is the active requirement for S05; `R017` was validated by S04.
- `S05` should likely use test-first/red-green tasks over fixtures before touching runtime persistence paths.
- User asked whether `gsd auto` is OK; yes, but restart GSD first and let auto plan/execute S05 from this state.

## Do not

- Do NOT enable DSPy extraction, optimizers, or typed LM modules before S07 metrics/benchmark fixtures are verified.
- Do NOT start RLM workflows in S05.
- Do NOT add retrieval/fusion behavior in S05; that belongs to S06.
- Do NOT use `npx` for GitNexus; use `gitnexus analyze . --name daily-archive`.
- Do NOT reintroduce GSD worktree isolation; current project preference is normal git with `git.isolation: none`.
