# Architecture map — daily-archive (M001 orientation/assessment)

Snapshot at base `3031bf3ec` (branch `gsd-path/M001`; worktree `gsd-path-task/T002`). Method: static reading + codebase-memory-mcp + recorded logs only (`/tmp/arch-20261123-pass2.log`, `/tmp/preflight-20261122.log`); no new cargo/test runs were performed for this report. Assessment only: no gate flip, no pin, no threshold, no status value changes (N1 — diff is .md-only) — see `scripts/governor/__main__.py` for the only recorded execution plane. Verdict vocabulary: работает / кандидат / заглушка / unverified, each with a proof-weight tag (ran-in-gate / static-only / unverified).

## 1. Workspace composition

The workspace is a 12-crate Cargo workspace declared in `Cargo.toml` (members list, `Cargo.toml` lines 1–17), edition 2024, rust-version 1.85 (`Cargo.toml` `[workspace.package]`). The crate set at HEAD:

| Crate | Layer role | Source files at HEAD | Manifest |
|---|---|---|---|
| `kg-ontology` | universal ontology/schema | 11 (`crates/kg-ontology/src/lib.rs` et al.) | `crates/kg-ontology/Cargo.toml` |
| `kg-storage` | universal persistence (Samyama-backed) | 14 (`crates/kg-storage/src/lib.rs` et al.) | `crates/kg-storage/Cargo.toml` |
| `kg-embeddings` | universal embedding plane | 7 (`crates/kg-embeddings/src/lib.rs` et al.) | `crates/kg-embeddings/Cargo.toml` |
| `kg-algorithms` | universal graph algorithms (PPR etc.) | 6 (`crates/kg-algorithms/src/lib.rs` et al.) | `crates/kg-algorithms/Cargo.toml` |
| `kg-pipeline` | universal stage pipeline | 4 (`crates/kg-pipeline/src/lib.rs` et al.) | `crates/kg-pipeline/Cargo.toml` |
| `da-domain` | inner hexagon: entities, no infra deps | 85 (`crates/da-domain/src/lib.rs` et al.) | `crates/da-domain/Cargo.toml` |
| `da-ports` | port traits over kg-* + da-domain | 38 (`crates/da-ports/src/lib.rs` et al.) | `crates/da-ports/Cargo.toml` |
| `da-application` | use-case/application services | 104 (`crates/da-application/src/lib.rs` et al.) | `crates/da-application/Cargo.toml` |
| `da-adapters` | infrastructure adapters (GROBID, storage, LLM) | 51 (`crates/da-adapters/src/lib.rs` et al.) | `crates/da-adapters/Cargo.toml` |
| `da-parse` | parse plane (GROBID client + native ladder) | 75 (`crates/da-parse/src/lib.rs` et al.) | `crates/da-parse/Cargo.toml` |
| `da-cli` | composition root / CLI binary | 61 (`crates/da-cli/src/main.rs` et al.) | `crates/da-cli/Cargo.toml` |
| `da-mcp` | MCP server surface | 5 (`crates/da-mcp/src/main.rs` et al.) | `crates/da-mcp/Cargo.toml` |

Source-file counts are `git ls-tree -r` over each crate's `src` directory at base `3031bf3ec`; each row cites one concrete file from that directory plus the manifest [static-only]. The historical `da-graph` crate is removed — it appears nowhere in the `Cargo.toml` members list nor under `crates/` at HEAD (checked via `git ls-tree 3031bf3ec crates/`; the tree contains only the 12 member directories above plus `crates/OKF.md` and `crates/SCHEMA.yaml`) [removed, not on the runtime path]. The `legacy/` tree referenced by older docs is also removed and absent at HEAD: `git ls-tree 3031bf3ec --name-only` at the repo root finds no `legacy/` entry [removed; more no longer exists in the tree]. ADR-042–049 are proposed-documentary status, not implemented fact — cited in docs, not in any manifest under `crates/` at HEAD [proposed].

## 2. Layering and dependency direction

The hexagonal rule is enforced in CI, not just documented: the "Hexagonal dependency-direction check" step of `.github/workflows/architecture-guardrail.yml` (lines 104–145) asserts (a) `crates/da-domain/Cargo.toml` has no infrastructure deps (regex `reqwest|tokio|samyama|ruvector|hyper|axum|actix` over `[dependencies]`), (b) the same for `crates/da-ports/Cargo.toml`, and (c) no kg-* crate depends on any da-* crate (ADR-050 universality) [static-only; the check runs in GitHub CI].

Observed prod-dependency edges at HEAD (from each manifest's `[dependencies]`):

- `da-domain` → `kg-ontology` only (`crates/da-domain/Cargo.toml`) — inner layer touches one universal crate, no infra.
- `da-ports` → `da-domain`, `kg-storage`, `kg-embeddings`, `kg-algorithms` (`crates/da-ports/Cargo.toml`) — port traits typed against kg-* implementations.
- `da-application` → `da-domain`, `da-ports`, `kg-algorithms`, `kg-ontology`, `kg-pipeline`, `kg-storage` + `ruvector-sona` (`crates/da-application/Cargo.toml`).
- `da-adapters` → `da-domain`, `da-ports`, `da-parse`, `kg-storage`, `kg-embeddings` + infra (`reqwest`, `samyama`, `samyama-sdk`, `ruvector-cognitive-container`, `wasmi`, `jsonschema`; optional `pdf-inspector` behind the `pdf-classifier` feature, D295-gated `redb` cache-only) (`crates/da-adapters/Cargo.toml`).
- `da-parse` → `da-ports` only + `reqwest`/`tokio` (`crates/da-parse/Cargo.toml`) — the parse plane is deliberately thin in dependencies; its bulk is its own code under `crates/da-parse/src/` (75 files).
- `da-cli` → `da-domain`, `da-ports`, `da-adapters`, `da-parse`, `da-application`, `kg-ontology`, `kg-storage`, `kg-pipeline`, `kg-algorithms` + `samyama-graph-algorithms` (`crates/da-cli/Cargo.toml`) — the composition root wires every layer.
- `da-mcp` → `kg-storage`, `kg-ontology`, `kg-embeddings`, `da-domain`, `da-ports` + `rmcp`, `ruvector-hybrid` (`crates/da-mcp/Cargo.toml`).
- kg-* internal edges: `kg-pipeline` → `kg-ontology`; `kg-algorithms` → `kg-ontology` + `samyama-graph-algorithms` + `ruvector-solver`; `kg-storage` → `samyama` + `samyama-graph-algorithms`; `kg-embeddings` → `ruvector-hybrid` + `ruvector-solver` (respective manifests: `crates/kg-ontology/Cargo.toml`, `crates/kg-storage/Cargo.toml`, `crates/kg-embeddings/Cargo.toml`, `crates/kg-algorithms/Cargo.toml`, `crates/kg-pipeline/Cargo.toml`).

No reverse edge exists (no kg-* manifest names a da-* dependency — enumerated over the five manifests listed above at HEAD, e.g. `crates/kg-storage/Cargo.toml`) [static-only].

## 3. Vendor path dependencies

Builds are bound to local vendor checkouts via absolute path dependencies declared in `Cargo.toml` `[workspace.dependencies]`: `samyama` → `/root/vendor-source/samyama-graph`, `samyama-sdk` → `/root/vendor-source/samyama-graph/crates/samyama-sdk`, `samyama-graph-algorithms` → `/root/vendor-source/samyama-graph/crates/samyama-graph-algorithms`, and eight RuVector crates → `/root/vendor-source/ruvector/crates/...` (`ruvector-sona`, `ruvector-gnn-rerank`, `ruvector-hybrid`, `ruvector-cognitive-container`, `ruvector-agent-memory`, `ruvector-solver`, `rvf-runtime`, `rvf-types`) [static-only]. Consequence: any machine without `/root/vendor-source` cannot build the workspace — this is a structural property of `Cargo.toml`, not a CI claim.

## 4. Parse plane and the GROBID boundary

The production parse route is the GROBID Java service reached over HTTP: the adapter entry point is `crates/da-adapters/src/grobid_parser.rs`, the transport/client layer lives in `crates/da-parse/src/http.rs` and `crates/da-parse/src/transport.rs`, and Java process invocation in `crates/da-parse/src/java_invocation.rs` [static-only; per-family verdicts belong to the grobid-state report, `doc/review/grobid-state.md`, not this map].

The native replacement route exists in-tree as a large candidate surface: ~60 `native_*.rs` modules under `crates/da-parse/src/` (enumerated by `git ls-tree -r 3031bf3ec --name-only crates/da-parse/src`; e.g. `crates/da-parse/src/native_header_sequence.rs`, `crates/da-parse/src/native_bibliography_sequence.rs`, `crates/da-parse/src/native_body_text.rs`, `crates/da-parse/src/native_visual.rs`, `crates/da-parse/src/native_segmentation.rs`) plus the shadow-join comparator `crates/da-parse/src/p5_shadow_join.rs` that keeps native output comparable against GROBID production bytes [static-only]. Authorization state machinery: `crates/da-parse/src/native_retirement_authorization.rs`.

## 5. Live-test boundary and execution planes (U1)

The live-service-dependent test boundary sits in the da-cli example suites, provisioned by the governor gate script: `scripts/governor/__main__.py` (provisioning list around lines 784–800) names the seven da-cli examples executed by ungated test files (`acceptance_evidence_test`, `cross_stage_semantic_invariants_example`, `derived_maturity_rehearsal_example`, `external_holdout_capture_test`, `native_family_gate_rehearsal_test`, `parse_batch_budget_shadow_test`, `rest_batch_asset_shadow_test`) — verbatim from the script comment block [static-only]. The real-model suites requiring a live GROBID/model endpoint live in `crates/da-adapters/tests/`: `crates/da-adapters/tests/native_bibliography_real_model.rs`, `crates/da-adapters/tests/native_fulltext_real_model.rs`, `crates/da-adapters/tests/native_header_real_model.rs`, `crates/da-adapters/tests/native_visual_family_real_model.rs`, `crates/da-adapters/tests/scalar_crf_real_model.rs` [static-only].

Execution planes, stated exactly (U1):

- GitHub CI runs per-crate `--lib` tests for the kg-*, `da-domain`, `da-parse`, `da-mcp` crates and `--tests` for `da-application`, `da-adapters`, `da-cli` — including `cargo test -p da-parse --lib` (`.github/workflows/architecture-guardrail.yml` lines 95–106). No da-parse integration suite executes in GitHub CI; the workflow line is `cargo test -p da-parse --lib` (`.github/workflows/architecture-guardrail.yml` line 101). Nothing in the 146-suite da-parse integration plane is CI-protected.
- The only recorded execution plane for the full architecture gate is the local governor gate: `/tmp/arch-20261123-pass2.log` records `architecture gate steps: 23/23 passed, 0 timed out` and `ARCHITECTURE GATE: PASS` (2026-11-23, first complete pass since October) [ran-in-gate].
- The 146 da-parse integration suite count is rendered from the tree at this base: `git ls-tree -r 3031bf3ec --name-only crates/da-parse/tests` enumerates the suite files it is derived from — from code, not from a ledger [static-only].
- Preflight snapshot axes are dated and stale relative to this report: `/tmp/preflight-20261122.log` records `axis measurability: 5/21 (23.8%)` (2026-11-22, before the M001 doc edits) — a snapshot, not a current measurement [ran-in-gate, dated].

Absence claims in this map, each citing the checked location (E2): the removed `da-graph` crate — absent (checked via the `Cargo.toml` members list and `git ls-tree 3031bf3ec crates/`, neither contains it; removed name, not on the runtime path); the removed `legacy/` tree — absent (checked via `git ls-tree 3031bf3ec --name-only` at repo root, no entry; removed names survive only in documentary prose) [removed; more no longer exists in the tree].

## 6. CLI and surfaces

The CLI command surface is defined by the clap dispatch in `crates/da-cli/src/main.rs` (module list under `crates/da-cli/src/`, e.g. `crates/da-cli/src/load.rs`, `crates/da-cli/src/native_acceptance.rs`, `crates/da-cli/src/graph_query.rs`); example runners live under `crates/da-cli/examples/` (122 tree entries at base `3031bf3ec`, `git ls-tree -r`). The command×use-case ledger with per-command verdicts is T003's deliverable at doc/review/functionality-ledger.md — not yet in tree at this base (parallel wave-1 task; unverified until it lands). The MCP tool surface is `crates/da-mcp/src/tools.rs` with server entry `crates/da-mcp/src/main.rs` and hybrid search wiring `crates/da-mcp/src/hybrid.rs` [static-only].

## 7. Known weak spots (code-verifiable)

- Vendor path deps pin builds to `/root/vendor-source` (see §3, `Cargo.toml`).
- The da-parse integration plane executes only under the local governor gate (`scripts/governor/__main__.py`, provisioning list lines 784–800); CI runs only `cargo test -p da-parse --lib` (`.github/workflows/architecture-guardrail.yml` line 101) — the CI `--lib`-only gap (U1).
- Mixed ADR statuses: ADR-042–049 are Proposed, a documentary state of the ADR set, not implemented code fact; no manifest under `crates/` implements them at HEAD [proposed].
- The learning-critic crate lives outside the workspace (`feat/learning-critic-e1` worktree, not a `Cargo.toml` member); its port surface inside the workspace is `crates/da-ports/src/learning_critic.rs` [static-only].
- `CHANGELOG.md` still describes gsd-pi-era hooks — documentary staleness, scheduled for the T004 fix-doc wave; no code claim is derived from it here [documentary].

## 8. Method and evidence statement

All path citations in this file were enumerated with `git cat-file -e` at base `3031bf3ec` before writing (E1 pattern proven in T001's Log); the Verify command for this task re-checks every cited path against HEAD. Verdicts: this map makes no per-family verdict claims — those live in `doc/review/grobid-state.md` and in the T003 functionality ledger. Every "runs/passes" claim above cites its recorded log; everything else is tagged static-only or unverified. No CI file, no code, no config was touched (N1/N3 — this task edits only its own deliverable, doc/review/architecture-map.md, this file).
