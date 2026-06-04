# M030 Recommended Next Implementation Roadmap

This brief turns the M030 architecture and continuity reports into a concrete next milestone proposal. It is a planning handoff only: it does **not** register articles, fetch sources, parse or chunk articles, write LadybugDB, or claim production ingestion.

## Recommendation

Plan the next milestone around the first executable continuity break: **materialize the M030 selected references from bounded intake into cataloged, locally acquired, parser-checked, chunk-reviewed evidence while preserving fail-closed graph/import boundaries**.

Recommended milestone title: **M031 Selected Reference Replay to Graph Readiness Boundary**.

Recommended milestone outcome: the four M030 requested refs have a continuous, validated evidence chain from catalog registration through graph-readiness review packets, with explicit blockers for any ref that cannot safely proceed. LadybugDB writes and production import should remain out of scope unless a later milestone explicitly authorizes them after independent review and import rehearsal pass.

## Why This Is the Next Milestone

M030 established that pipeline modules and verifiers exist, but the process continuity audit found the execution chain is still broken at the handoff from bounded URL intake to the article catalog.

Current state:

- Requested refs: `4`.
- Already cataloged: `2` — `arxiv:2507.19457`, `arxiv:2605.26099`.
- Missing from article catalog: `2` — `stanford:cs224n:gradient-notes`, `arxiv:2605.29548`.
- Source acquisition, parser readiness, chunk readiness, KG readiness, graph writes, and production persistence remain unsafe to claim.

The next milestone should therefore avoid graph or model-helper ambition and instead retire the earliest critical blockers with replay evidence.

## Recommended Milestone Scope

### In scope

- Register metadata-only catalog rows for the two missing selected refs.
- Revalidate article catalog/index linkage against the bounded M030 selection.
- Replay controlled local source acquisition for all four selected refs.
- Run loader evidence and parser/conversion quality checks from real local artifacts.
- Run PageIndex and semantic chunk evidence only from verified parser output.
- Generate graph-readiness reviewer packets from validated chunks.
- Require independent graph-readiness review artifacts before any import eligibility claim.
- Preserve fail-closed import rehearsal boundaries and refusal diagnostics.

### Out of scope

- LadybugDB writes.
- Production graph persistence.
- Trusted KG import.
- RLM activation or graph traversal adoption.
- MiniMax helper activation.
- DSPy optimizer activation.
- 30-paper or 100-paper scaling loops.
- Treating URL reachability as source acquisition evidence.
- Treating prior M028 inclusion as current M030 readiness evidence.

## Candidate Slices

### S01: Metadata-only catalog closure

Goal: turn the bounded selection into stable article catalog records without network fetches.

Work:

- Add metadata-only catalog rows for `stanford:cs224n:gradient-notes` and `arxiv:2605.29548`.
- Preserve existing rows for `arxiv:2507.19457` and `arxiv:2605.26099`.
- Keep `network_fetch_attempted=false` and `source_artifact_captured=false` during registration.
- Rebuild and validate `data/article_catalog/index.json` and catalog paths.

Acceptance criteria:

- All four selected refs resolve through the article catalog/index.
- Duplicate refs, unsafe paths, malformed arXiv keys, and missing titles are rejected by existing catalog tests or a new scoped verifier.
- A report lists the four refs, catalog paths, source variants, and fail-closed safety flags.

Verification candidates:

- `uv run pytest tests/test_m027_mixed_source_catalog.py tests/test_article_catalog_schema.py`
- `uv run python scripts/verify_m025_article_catalog.py`

### S02: Controlled source acquisition replay

Goal: capture real local source artifacts for all selected refs with provenance and failure diagnostics.

Work:

- Run controlled source acquisition for the four selected identities after catalog closure.
- Capture Stanford PDF and arXiv abs/PDF or article variants according to existing source variant contracts.
- Persist acquisition diagnostics, summary counts, hashes, byte sizes, media types, and safety flags.
- Refuse silent fallback if a source is missing, empty, blocked, or unsafe.

Acceptance criteria:

- Every selected ref has either captured local source artifacts or an explicit blocking diagnostic.
- Source artifact metadata is hash/size/path backed.
- No graph/import readiness flag changes during acquisition.

Verification candidates:

- `uv run pytest tests/test_m027_source_acquisition_boundary.py`
- `uv run python scripts/verify_m027_source_acquisition_boundary.py <acquisition-summary>`

### S03: Loader and parser/conversion quality replay

Goal: convert captured sources into validated parser inputs and record parser blockers before chunking.

Work:

- Run loader replay against all four selected identities from local artifacts.
- Classify the Stanford PDF through the local PDF parser path.
- Replay arXiv PDF/HTML conversion quality for the selected arXiv refs.
- Record source hashes, converted payload hashes, byte sizes, structure counts, diagnostic codes, and fallback reasons.

Acceptance criteria:

- Every successful load has provenance, source path, outcome, duration, warning count, and checksum.
- Parser/conversion outputs are bounded and hash-verified.
- Metadata-only, missing, stale hash, unsafe path, and low-quality source cases remain explicit blockers.

Verification candidates:

- `uv run pytest tests/test_full_text_ingestion.py tests/test_article_evidence_bridge.py tests/test_m027_conversion_quality_boundary.py tests/test_page_index.py`
- `uv run python scripts/verify_m027_conversion_quality_boundary.py <conversion-summary>`

### S04: PageIndex semantic chunk evidence

Goal: produce chunk/evidence-path artifacts only from parser-verified inputs.

Work:

- Build PageIndex documents from parser-ready converted payloads.
- Build semantic chunks and evidence paths with stable IDs, section anchors, bounds, and provenance.
- Preserve parser-ready zero-chunk diagnostics as blockers.
- Fail closed on empty sections, invalid evidence links, repair warnings, or missing provenance.

Acceptance criteria:

- Each parser-ready ref has chunk counts and evidence path counts in replay artifacts.
- Zero-chunk or invalid-link cases block graph-readiness promotion.
- Chunk artifacts do not include raw source payload leakage in metadata.

Verification candidates:

- `uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_m027_end_to_end_mixed_replay.py`
- `uv run python scripts/replay_m027_end_to_end_mixed_replay.py --no-network <scoped-options>`

### S05: Graph-readiness review packets and fail-closed import rehearsal

Goal: create bounded reviewer packets and prove import remains refused until review and contract gates pass.

Work:

- Export graph-readiness packages from validated chunk evidence.
- Generate bounded reviewer packets and independent review request/summary artifacts.
- Validate review artifacts with `--require-completed-review` only when completed review events exist.
- Run fail-closed import rehearsal after review status is known; preserve refusal diagnostics for non-eligible candidates.

Acceptance criteria:

- Graph-readiness packets exist only for refs with validated source, parser, chunk, and evidence-path chains.
- Any ref lacking review evidence has an unsafe-to-claim reason.
- Import rehearsal records `import_ready=false`, `production_import_attempted=false`, and `ladybugdb_written=false` unless future scope explicitly changes authorization.

Verification candidates:

- `uv run pytest tests/test_graph_readiness_export.py tests/test_graph_readiness_review.py tests/test_import_ready_contract.py tests/test_import_boundary_rehearsal.py`
- `uv run python -m arxiv_archive.graph_readiness_review --review-dir <review-dir> --events <events.jsonl> --validate-only --require-completed-review`

## Milestone Acceptance Criteria

The milestone is complete when:

1. All four M030 selected refs have catalog/index rows or an explicit fail-closed registration diagnostic.
2. Source acquisition replay has produced local artifact hashes or explicit blockers for each selected ref.
3. Loader and parser/conversion replay has produced provenance, quality diagnostics, and converted payload hashes for every successful source.
4. PageIndex/chunk/evidence-path replay has run only on parser-verified payloads and records zero-chunk or invalid-link blockers.
5. Graph-readiness reviewer packets are generated only from validated chunk evidence.
6. Independent review status is represented explicitly before any import eligibility claim.
7. Import rehearsal remains fail-closed unless every upstream gate passes and a future milestone authorizes import scope.
8. Reports preserve the safety claims from M030: no LadybugDB writes, no production persistence, no trusted KG import, and no model-helper activation.

## Required Observability

Each slice should leave artifacts that make the next agent's job mechanical:

- Per-ref status table with `ready`, `blocked`, or `unsafe-to-claim` state.
- Stable diagnostic codes for malformed inputs, unsafe paths, stale hashes, empty sources, zero chunks, missing review, and import refusal.
- Event JSONL and summary JSON for source acquisition, conversion, replay, graph-readiness export, and review stages.
- Input and output artifact hashes for every stage transition.
- Redacted reports that never include raw article body text, secrets, vectors, embeddings, or optimizer traces in metadata.

## Planning Notes for GSD

Suggested first GSD planning move: create M031 with five ordered slices matching the candidate slices above. S01 and S02 are the highest-risk slices because they retire the critical catalog/selection split and prove whether source acquisition can materialize the bounded selection. Later slices should depend on concrete artifacts from prior slices rather than on static M030 reports.

Do not collapse all work into one slice. The safe ordering is catalog closure, source acquisition, loader/parser replay, chunk evidence, then graph-readiness review/import rehearsal. That ordering keeps every positive claim tied to the evidence that authorizes it.
