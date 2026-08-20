# STUB-AUDIT — Rust-era stub and deferral inventory

schema_version: rust-stub-audit.v1
generated: 2026-08-20 (M424 S02, session scan of crates/*/src)
supersedes: the Python-era STUB-AUDIT.md (vanished with the cold archive;
only a dangling reference in ingest.rs survived — fixed in M424)

## Purpose

One honest home for every stub, fake, and deferral in the Rust workspace.
Each entry carries file:line evidence, a class, and a disposition. Tests
must never pin a fake (MEM910): when an entry moves to `fake-fixed`, the
pinning test flips in the same commit.

## Classes

- **fake-fixed** — was a fake implementation; replaced by honest behavior.
- **honest-deferral** — typed fail-closed behavior with a documented reason;
  no fake success is pretended.
- **external-limitation** — blocked on an external crate/upstream capability.
- **test-double** — intentional test scaffolding with documented drift.

## Inventory

| file:line | symbol | class | disposition | evidence |
|---|---|---|---|---|
| crates/kg-storage/src/mock.rs | `MockGraphStore::vector_search_direct` | fake-fixed | M424 S01: brute-force cosine, score desc + node-id tiebreak, k-truncated; was constant 0.0 in HashMap order, no k bound | commit 56577b0a→M424; tests `mock_vector_search_ranks_aligned_above_orthogonal`, `mock_vector_search_truncates_filters_and_tiebreaks` |
| crates/kg-storage/src/mock.rs | `MockGraphStore::vector_search` (GraphStore) | fake-fixed | M424 S01: delegates to the direct path, returns seeded node with vid + props; was always empty | test `mock_graph_store_vector_search_returns_seeded_node_with_vid`; da-mcp flip `test_vector_search_seeded_returns_the_seeded_node` |
| crates/kg-storage/src/mock.rs | `MockGraphStore::query` / `query_readonly` | test-double | Returns an empty QueryResult **by design**: Mock has no Cypher engine. Query-path tool coverage lives in da-mcp tests against the real in-process SamyamaGraphStore (M425); Mock remains the write-path double. A mock Cypher engine stays out of scope until a contract suite requires it | header contract list in mock.rs; `test_get_paper_samyama_finds_seeded_paper` |
| crates/da-mcp/src/tools.rs | `search_papers` case-insensitive claim | fake-fixed | M425: engine CONTAINS is case-sensitive and the Rust post-filter only narrowed engine hits — a lowercase query returned zero. Fixed with a case-normalized `toLower` retry (fallback only; exact-case path unchanged). Found by moving the test from Mock to the real engine | `test_search_papers_case_insensitive_finds_seeded_on_real_engine`; `test_search_papers_samyama_exact_case_finds_seeded` |
| crates/da-mcp/src/tools.rs | `get_references` error swallowing + row-shape mismatch | fake-fixed | M426: `if let Ok` swallowed engine errors (a query failure looked like no data) and both-direction concatenated backward/forward rows with different widths into one schema. Fixed: errors propagate as Err; unified 6-column schema with null padding; fallback columns match query aliases | `test_get_references_samyama_backward_and_both_schema` |
| crates/da-mcp/src/tools.rs | `get_method_lineage` error swallowing | fake-fixed | M428: same class as M426 — two `if let Ok` blocks turned engine failures into silent empty lineage. Fixed: errors propagate as Err. Variable-length `predecessor_of*1..N` paths proven supported on the real engine | `test_get_method_lineage_samyama_seeded_chain` |
| crates/da-application/src/embedding_write.rs | `write_node_embedding` fake success | fake-fixed | M429: every store error was swallowed (`let _ =`); when both the primary write and its fallback failed the helper still returned `Replaced` (callers emitted `node_embedding_replaced` for a write that never happened), and a failed sha stamp returned `Added` with `embedding_sha256` silently drifting. Fixed: `Result`-returning; dual-write and sha-stamp failures propagate; ingest fails the materialization, the entity loop warns visibly per entity | `dual_write_failure_is_err_not_fake_replaced`; `failed_sha_stamp_is_err_not_fake_added` |
| .pre-commit-config.yaml | lib-only clippy | test-double | M430: the pre-commit clippy hook compiled only lib targets, so a test-only import break (M429) passed the hook and failed later under `cargo test`. Fixed: a per-crate `cargo-clippy-tests` (`--lib --tests`) hook added; both hooks avoid the workspace rocksdb test-profile rebuild | split documented in AGENTS.md pre-commit section |
| crates/kg-storage/src/graph_mode.rs | RocksDB persistence | honest-deferral | `DA_GRAPH_MODE` env contract refuses `DA_GRAPH_DATA_PATH` with `PersistenceNotConfigured` (M332); durability path is `batch-ingest --output .sgsnap` | `refuse_persistence_path_from_env` |
| crates/kg-storage/src/graph_mode.rs | remote HOT writes | honest-deferral | `RemoteWritesUnsupported` (M421): RESP read plane exists; no DirectGraphStore over RESP; D278/D134 | test `remote_writes_unsupported_message_is_actionable` |
| crates/da-application/src/ingest.rs | Summary node creation | honest-deferral | ADR-053 Layer B: deferred until real summarization; schema ready, no `abstract_copy` fake | comment at the creation site references ADR-053 Layer B (re-pointed in M424) |
| crates/da-domain/src/validator.rs | `Decision.policy_id` cross-ref | honest-deferral | Phase 2: Policy node does not exist; registry row targets Decision as a documented self-reference placeholder with `required: false` (ADR-048) | `cross_reference_fields()` Decision plane block |
| crates/da-domain/src/relation.rs | 7 causal relation types | honest-deferral | Need GRPO/causal models (no GPU); declared in the module header, absent from RELATION_TYPES | module doc lines 3–11 |
| crates/da-adapters (pdf-inspector) | TSR second table source | external-limitation | `extract_tables_with_structure_*` needs independent cell bboxes + layout model; feeding GROBID TEI is tautological (M419 followUp, rejected) | M419 SUMMARY followUps |
| crates/da-adapters (pdf-inspector) | ToUnicode/markdown repair | external-limitation | pdf-inspector 0.1.x exposes no repair path; only detection. Garbled pages surface as typed `page_text_missingness` (M422) | MEM916; pdf_triage_canary measurement |
| crates/da-adapters (pdf-inspector) | `PdfOptions::detect_only()` for triage | honest-deferral | Measured blind to `suspected_garbled_text` (hub PDF: Full sees 69+84, detect-only sees none, 17x faster); product stays Full (MEM916) | `pdf_triage_canary` example, M423 |
| crates/da-adapters/src/candidate_materialization_stage.rs | tables/figures rows in linear topology | honest-deferral | Typed `deferred: … dependency not wired in this topology` reasons — rows keep typed missingness, nothing is invented | lines 180–242 |
| crates/da-application/src/scheduler_runner.rs | GraphWrite task type | honest-deferral | D278/ADR-063: no honest producer; governor forbids fake revival | `scheduler_no_graph_write_executor` pin |

## Scan method (reproduce)

```
rg -n 'todo!|unimplemented!' crates/*/src            # zero hits in src
rg -n -i 'stub|placeholder|deferred|not wired' crates/*/src
rg -n 'TODO|FIXME|XXX' crates/*/src
```

Zero `todo!`/`unimplemented!` in workspace src as of 2026-08-20.
