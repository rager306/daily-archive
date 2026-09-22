<!-- project-current:start -->
# daily-archive — agent map (what / where)

Runtime: **Rust workspace** `crates/da-{domain,ports,application,parse,adapters,cli,mcp}` (hexagonal / onion).  
Frozen evidence: Python stack cold-archived under `archive/python-era-cold-*/` (see `archive/PYTHON-ERA-COLD.md`); `legacy/` is a restore stub — not on the runtime path.

## Hard locks (do not violate)

| Lock | Rule |
|------|------|
| Import / graph write | `import_eligible=false` until **explicit user yes** (D127) |
| Free invent | constrained select = `candidate_id` only |
| GEPA/LLM deploy | no auto-promote; dual F1 + same-n + val_gap required |
| Sidecar TEI/md | candidate evidence only, not graph truth |
| Onion | no da-application → da-adapters inversion for path defaults |
| DSPy | not enabled until eval metrics + benchmark fixtures verified (D136) |
| Samyama | sole KG+vector+persist store (ADR-040); no JSONL, no external DB for state |

## GSD workflow & process conventions (read first)

This project uses **GSD (Git Ship Done)** for milestone/slice/task tracking.
Work through GSD tools, do not hand-edit `.gsd/` planning artifacts.

### Lifecycle (GSD tools, in order)

1. `gsd_milestone_generate_id` → get milestone ID (M###-xxxxx).
2. `gsd_plan_milestone` → roadmap with slices (each needs sliceId, title, risk, depends, demo, goal, successCriteria, proofLevel, integrationClosure, observabilityImpact).
3. `gsd_plan_slice` → per-slice PLAN.md with tasks (each needs taskId, title, description, estimate, files, verify, inputs, expectedOutput).
4. `gsd_task_complete` → per-task (oneLiner + narrative + verification + verificationEvidence).
5. `gsd_slice_complete` → per-slice (sliceTitle + oneLiner + narrative + uatContent).
6. `gsd_validate_milestone` → when all slices done.
7. `gsd_complete_milestone` → closeout.

**Forbidden:** hand-editing STATE.md, PLAN.md, SUMMARY.md, ROADMAP.md, gsd.db.
Use the engine tools. `.gsd/continue.md` and `CHANGELOG.md` are hand-writable.

### Compile & test rules (avoid the rocksdb rebuild trap)

**One-time setup (do this first):** install system RocksDB 8.10.0 and create
`.cargo/config.toml` from `.cargo/config.example.toml`. This eliminates
`cc1plus` (C++ RocksDB recompilation, 5–10 min) on every `cargo` invocation.
See `.cargo/config.example.toml` for full instructions.

**sccache (cross-workspace cache):** `~/.cargo/config.toml` sets
`build.rustc-wrapper = "sccache"` globally. Combined with
`[profile.dev.package."*"] incremental = false` in workspace `Cargo.toml`,
this caches samyama/ruvector compilation across workspaces and `cargo clean`.
`SCCACHE_BASEDIRS` in `~/.bashrc` normalizes paths (`/root/vendor-source`,
`/root/daily-archive`). Workspace members (da-*, kg-*) keep `incremental=true`
for fast iteration. Vendor workspaces (`samyama-graph`, `ruvector`) have their
own `.cargo/config.toml` with `ROCKSDB_LIB_DIR`.

`cargo test --workspace` / `cargo check --workspace` triggers
`librocksdb-sys` (Samyama RocksDB FFI) rebuild for the test profile —
**5–10 min each** (without the system-library setup above). Per-crate commands
skip it:

```bash
# RIGHT: per-crate checks during iterative dev
 cargo check  -p <changed-crate>
 cargo clippy -p <changed-crate> --all-targets --no-deps -- -D warnings
 cargo test   -p <changed-crate> --lib

# WRONG: full workspace (rocksdb rebuild, 5-10 min)
cargo test --workspace      # DON'T
```

`PYTHONPATH=scripts python3 -m governor architecture` is the **full gate** — run it
only as the final check at slice end, not on every edit. It runs
fmt + clippy + hexagonal invariant + per-crate tests for all our crates.

**Concurrent cargo processes block on `target/` file lock.** Cancel
background `async_bash` cargo jobs (via `cancel_job`) before re-running.
Stale `async_bash` results surface as delayed "background job done"
notices — their content is often already superseded.

### Browser evidence gate (GSD validation gotcha)

GSD `gsd_validate_milestone` has a browser-evidence gate that downgrades
`verdict: pass` → `needs-attention` when slice goal/successCriteria
text contains trigger words from `BROWSER_REQUIREMENT_RE`. The regex
matches: `file://`, `localhost`, `playwright`, `chrome`, `screenshot`,
**`snapshot`**, `browser_*` tool names.

**`snapshot` is the hidden trap for backend milestones.** Domain terms
like `PropertySnapshot`, `export_snapshot`, `import_snapshot`, or
"post-M284 snapshot" in slice goal/successCriteria trigger the gate.

**Fix:** for each triggering slice, write `285-SS-ASSESSMENT.md` with
runtime-executable evidence:

```yaml
---
uatType: runtime-executable
verdict: PASS
---

| check_name | type | verdict | tool | duration |
|------------|------|---------|------|----------|
| check-id | runtime | PASS | gsd_uat_exec <id> | 99ms |
```

Get real evidence IDs via `gsd_uat_exec` with `intent: "uat-runtime-check"`
or `intent: "uat-artifact-check"`. The `hasRuntimeExecutableUatEvidenceText`
bypass then activates and `verdict: pass` is preserved.

### Manual closeout when Attempt is unavailable

Free-form sessions cannot mint a running Attempt, so `gsd_task_complete` fails there. Use engine calls, never a hand-written row: `testedSourceRevision` is the engine aggregate `sha256:…` from `captureMilestoneVerificationSourceRevision`, not a git SHA; `gsd_save_gate_result` after `gsd_skip_slice` finds no tasks; Q5/Q6/Q7 need a real taskId; never write `.gsd/gsd.db` with sqlite3 or python (engine owns the WAL). The worked sequence that sealed M306-417kcl (MEM567 / D144) is in `git log -p -- AGENTS.md`.

### Governor CLI (trajectory enforcement)

`scripts/governor/` is the **governor CLI** — hexagonal Python package that
derives project trajectory report (verdict: on_track / drift_risk / blocked)
from ADRs, dimensions, reverse-ADR audit, drift flags. Rules are DATA
(YAML), not hardcoded Python.

```bash
PYTHONPATH=scripts python3 -m governor --phase preflight   # default
python3 scripts/check_project_trajectory.py --phase active
PYTHONPATH=scripts python3 -m governor --phase closeout
```

Reverse-ADR audit enforces 7 rules: 4 artifacts-gate (D127/ADR-005/ADR-040)
+ 3 Rust-specific (kg-* must not depend on da-*, da-domain must not depend
on infra, da-application must not depend on da-adapters in prod deps).
Negative test: inject `da-domain` into a kg-* Cargo.toml → governor flips
to `verdict=blocked`.

**Do NOT write new bash scripts to enforce invariants** the governor
already checks. Extend governor rules or `scripts/verify_rust_architecture.sh`
instead. (M285-z09px0/S08 extracted governor out of `legacy/` and
adapted it for Rust — it is the authoritative trajectory checker.)

### Pre-commit hooks (`.pre-commit-config.yaml`)

First line of defense before CI. Covers kg-* + da-* in cargo-fmt and
cargo-clippy-lib (no `--all-targets`, avoids rocksdb rebuild), plus
cargo-clippy-tests (per-crate `--lib --tests`, M430: clippy --lib does
NOT compile test modules — a test-only break passed the lib hook and
failed under `cargo test`, the M429 lesson), governor-trajectory
(ADR invariants + artifact gates) and docs-drift-check.
Run automatically on `git commit`.

### GitNexus (code intelligence)

Reindex after structural changes (new crates, renamed symbols):
`node .gitnexus/run.cjs analyze`. Until then, new symbols stay unresolved.
Use `impact({target, direction:"upstream"})` before editing any symbol —
warn the user on HIGH/CRITICAL risk. Use `detect_changes()` before commit.

### Testing strategy (ADR-051)

**Two axes:** hexagonal layers (7 contours) × test types (8 categories).

#### Key rules

- MockGraphStore MUST pass the same contract suite as SamyamaGraphStore
- Use `DeterministicEmbedder` (LCG seed), never real network in tests
- Use `#[tokio::test(start_paused = true)]` for timeout/backoff tests
- Flaky test → fix determinism, NEVER add retry
- `roundtrip<T>()` macro for every Serialize type
- Do NOT build oracle through same code being tested (common-mode failure)
- State-based fakes > choreography mocks — verify result, not call sequence
- Contour 6 disk→query: **same-process** `--snapshot` only (process-local store dies on exit)
- Task Cypher uses ontology wire strings (`hasPart`, `CITES`) from `data/ontology/edge_contracts.yaml`
- Extraction F1 alone ≠ scientific KG readiness — pair with graph task suite

#### CI tiers

- **PR** (<30s): Unit + Property + Serde + Contract (Mock) + fmt + clippy + arch
- **Merge** (<2min): + Integration (Samyama) + Business logic
- **Nightly** (<15min): + Mutation (domain) + E2E + Flaky(5) + Fuzz + Shuttle/Turmoil
- **Release** (<30min): + Mutation (app) + Crash recovery + Migration upgrade + semver

#### Governor commands

```bash
PYTHONPATH=scripts python3 -m governor --check-contracts
PYTHONPATH=scripts python3 -m governor --check-serde-roundtrip
PYTHONPATH=scripts python3 -m governor --check-flaky 5
PYTHONPATH=scripts python3 -m governor --test-matrix
```

See `doc/adr/ADR-051-testing-strategy-overlapping-verification-contours.md` for full details.

### Data-driven ontology (ADR-050 Phase D+F)

Schema definitions are DATA, not hardcoded Rust. Two modes:

```bash
# Export all 31 node schemas to YAML
da ontology-export --output data/ontology/

# List schemas from YAML (data-driven mode)
da schema-list --from-yaml

# List schemas from hardcoded Rust impls (default)
da schema-list
```

- `YamlSchemaAdapter` (da-domain): impl `NodeSchemaDef` via `Box::leak`
  pattern — wraps YAML data, leaks strings to `&'static str`.
- `OntologyRegistry` (kg-ontology): loads `node_schemas.yaml`, provides
  `get_schema(label)`, `labels()`, `len()`.
- `validate_node_properties_or_yaml()`: opt-in YAML-driven validator.
  Existing `validate_node_properties()` stays hardcoded (backward compat).
- `schema_for_label_or_yaml()`: tries YAML first, falls back to hardcoded.

Round-trip: `da ontology-export` → `data/ontology/node_schemas.yaml` →
`OntologyRegistry::load_from_dir()` → `YamlSchemaAdapter` →
`NodeSchemaDef` trait → validator.

### Handoff between sessions

At session end (or before context exhaustion): update `.gsd/continue.md`
with active milestone, what shipped, next action, and operators. Next
session reads `.gsd/continue.md` + `.gsd/STATE.md` + this AGENTS.md to
re-orient in <1 min.

## Where to look first

| Question | Open this |
|----------|-----------|
| Human overview + architecture | `README.md` |
| Project state (GSD) | `.gsd/PROJECT.md`, `.gsd/STATE.md` |
| Binding ADRs | `doc/adr/ADR-INDEX.md` |
| Documented solutions (module / problem area) | `.compound-engineering/artifacts/solutions/` — YAML frontmatter (`module`, `tags`, `problem_type`); relevant when implementing or debugging in an area that already has an entry |
| Graph schema | `doc/GRAPH-SCHEMA.md` |
| Ontology alignment | `doc/ONTOLOGY-ALIGNMENT.md` |
| Healing scenarios | `doc/GRAPH-HEALING-SCENARIOS.md` |
| Persistence model | `doc/PERSISTENCE-ANALYSIS.md` |
| Extraction evaluation | `data/gold_standard/`, `crates/da-cli/examples/eval_batch.rs` |
| Corpus metrics (corpus) | `crates/da-cli/examples/eval_batch.rs` output |
| GROBID semantic correctness (active) | `doc/adr/ADR-076-grobid-cascade-three-automata-graph-freeze.md`, `doc/review/grobid-absorption-followup.json`, `.gsd/STATE.md` |
| GSD state (local) | `.gsd/continue.md`, `.gsd/STATE.md` |

After a solved, verified problem, offer once to invoke the `ce-compound` skill at the completion checkpoint only when the work produced durable project reasoning that is not readily recoverable from the final code, tests, types, comments, or existing documentation, and losing it would plausibly cause recurrence, material risk, or substantial rediscovery. Apply this counterfactual: if the learning document disappeared, would a future engineer reading the final implementation still be likely to repeat the mistake or redo substantial investigation? If not, do not offer. Completion, effort, and diff size alone are not enough. Offer at the checkpoint so a qualifying learning can ship in the PR that produced it, and only where the repository treats captured learnings as tracked, committed knowledge.

Write every report, summary, or handoff to the user through the `ce-noslop` skill. This applies when you are the top-level agent writing to the user, not when you are a subagent reporting to its caller. Do not apply it to code, config, verbatim quotes, or text the user asked to post as written.

## Package map (Rust workspace)

| Crate | Path | Role |
|-------|------|------|
| Domain | `crates/da-domain/` | Paper/Entity/Evidence/Relation/Healing/Scheduler/Eval types |
| Ports | `crates/da-ports/` | GraphStore/DirectGraphStore (HOT path), ParserPort, Embedder, Extractor, OpenAlexClient |
| Application | `crates/da-application/` | Ingest/BatchIngest/Extraction/Healing/Enrich/Scheduler use-cases |
| Parse | `crates/da-parse/` | Scholarly PDF fulltext: Java GROBID HTTP (production route), native cascade shadow (M506–M511, ADR-076), TEI IR, ParserPort projector |
| Adapters | `crates/da-adapters/` | HTML parse, pdf-classifier, XY-Cut, fd_api embedder (bge-m3), SamyamaGraphStore, RuleBasedExtractor, OpenAlexHttpAdapter, GrobidParseStage |
| CLI | `crates/da-cli/` | `da` binary: health/version/ingest/batch-ingest/extract/schema-init/heal/enrich/scheduler-run |
| MCP | `crates/da-mcp/` | MCP server for knowledge-base external access (ADR-054) |
| **kg-ontology** | `crates/kg-ontology/` | Universal types: Violation/Severity/PropertySnapshot/TemporalEdge (ADR-050, no da-* deps) |
| **kg-storage** | `crates/kg-storage/` | Universal storage traits: GraphStore/DirectGraphStore/MockGraphStore (ADR-050) |
| **kg-embeddings** | `crates/kg-embeddings/` | Universal embeddings: Embedder/FdApiEmbedder (ADR-050) |
| **kg-algorithms** | `crates/kg-algorithms/` | Universal graph algorithms: resolve_temporal_edges (ADR-050) |
| **kg-pipeline** | `crates/kg-pipeline/` | Universal pipeline DSL: PipelineBuilder/ExecutionEngine (ADR-050) |

Dependency direction (enforced by `scripts/verify_rust_architecture.sh` + CI):
`da-domain ← da-ports ← da-application ← da-cli`; `da-adapters` is infrastructure (depends on ports, not vice versa).
`da-domain` also depends on `kg-ontology` (re-export, ADR-050 Phase A).
**kg-* crates are universal — must NOT depend on da-*** (ADR-050 invariant, enforced by governor + guardrail).

## Operators (run these)

```bash
# Architecture guardrail — FULL gate, run at slice END only (slow)
# (fmt + clippy + hexagonal invariant + per-crate tests for kg-* + da-*)
PYTHONPATH=scripts python3 -m governor architecture

# Governor CLI — trajectory report (fast, ~1s)
PYTHONPATH=scripts python3 -m governor --phase preflight

# Per-crate checks during iterative dev (FAST, avoids rocksdb rebuild)
cargo check  -p <changed-crate>
cargo clippy -p <changed-crate> --all-targets --no-deps -- -D warnings
cargo test   -p <changed-crate> --lib

# Common per-crate test targets (AVOIDS `cargo test --workspace`)
cargo test -p da-domain --lib
cargo test -p da-parse --lib
cargo test -p da-adapters --lib
cargo test -p da-application --tests
cargo test -p kg-ontology --lib
cargo test -p kg-storage --lib
cargo test -p kg-algorithms --lib

# ADR-051 P1 property-based tests (M344/M353–M355)
cargo test -p kg-ontology --lib proptest   # TemporalEdge intervals (8)
cargo test -p da-domain --lib proptest     # EntityType/RelationType/confidence/VID (19)
cargo test -p da-cli --lib proptest        # graph_tasks parser (6)

# ADR-051 §8.1 semantic graph task suite (M336–M338)
cargo test -p da-cli --lib graph_tasks                        # parser unit
cargo run -p da-cli --example graph_task_suite -- \
  --snapshot artifacts/gepa_hub_production/hub.sgsnap \
  --tasks data/graph_tasks/gepa_hub.tasks.cypher \
  --report artifacts/graph_tasks/gepa_hub_report.json

# PDF OCR triage via pdf-inspector (M346–M351; optional feature)
cargo test -p da-adapters --features pdf-classifier --lib pdf_classifier
cargo test -p da-adapters --features pdf-classifier --test hub_ocr_probe -- --nocapture
# CLI build with pre-GROBID triage (off by default):
#   cargo build -p da-cli --features pdf-classifier

# Feature-gated shadow/contract tests (pdf-layout, off by default).
# Nine da-cli test files are `#![cfg(feature = "pdf-layout")]` because the
# examples they drive carry `required-features = ["pdf-layout"]`. A plain
# `cargo test -p da-cli --tests` compiles them to EMPTY targets, so their
# coverage is nominal unless the feature is enabled. `cargo test --test NAME`
# does NOT build examples, so provision the runners first:
cargo build -p da-cli --features pdf-layout --examples  # example runners
cargo test  -p da-cli --features pdf-layout --tests     # 447 tests (vs 418 default)
# `governor architecture` runs both as its last steps
# (da-cli feature-gated example runners / contracts).

# Offline stage-tape recorder for the Java GROBID cascade (M506/S01).
# Captures the model debug dump (GROBID `debugMode`) and derives per-model
# coverage: sections are invocations, not distinct models. Offline evidence
# only — it never writes a graph and is not part of `ParserPort`.
# Raw TEI is captured by the compatibility tooling, not here (one source of truth).
cargo run -p da-cli --example grobid_stage_tape -- \
  --pdf data/sources/arxiv/<cat>/<id>/source/<id>.pdf \
  --out /tmp/tape --endpoint fulltext --models segmentation
# `--models` filters the RESPONSE only; it does not reduce Java computation,
# and a filtered tape is marked `filtered` rather than `captured`.
# Verify a stored tape: re-hashes the source PDF and the stored dump, probes the
# live release, and exits non-zero on any finding (substitution, absence, drift,
# recorded failure, no sections). A tape nothing verifies is not evidence.
cargo run -p da-cli --example grobid_stage_tape -- --verify /tmp/tape/tape.json

# Extraction evaluation on gold-standard corpus (12 modern papers)
cargo run -p da-cli --example eval_batch

# Single-paper extraction debug
cargo run -p da-cli --example eval_extract -- 2507.19457

# End-to-end (requires GROBID :8070 + FD_API :8000)
da schema init
da batch-ingest --pdf-dir data/article_catalog/article_catalog/arxiv/cs-cl/<id>/source
da batch-enrich
da extract --id <paper_id>
# Grounded extractive summaries for PaperRevision nodes already in-process (no LLM):
#   da summarize-revisions [--graph file.sgsnap]

# Wave 3 multi-process RESP health (in-memory, no RocksDB; M307–M312)
# Port 16379 — not Redis 6379, not vendor SAMYAMA_RESP_PORT 6380
da health                              # reports DA_GRAPH_MODE; remote is a RESP read plane
da serve-graph --port 16379            # terminal A (optional: --snapshot file.sgsnap)
da graph-ping --port 16379             # terminal B → PONG
# DA_GRAPH_MODE=remote opens a read plane (RemoteGraphStore); Samyama HOT writes return GraphModeError::RemoteWritesUnsupported
```

Historical `legacy/scripts/verify_m0xx_*.py` — frozen Python milestone pins, not runtime ops. `legacy/` is gitignored (untracked frozen Python stack).

## Current snapshot (extraction eval)

Latest committed complete-denominator numbers are **2026-08-09** (README; the 2026-08-05 measurement below is historical): **P=0.766, R=1.000, micro-F1=0.8677** on 200/200 fixtures, 1,439 gold entities (`archive/artifacts-legacy/gold_corpus/eval_aggregate_all_sources.json`). No newer committed corpus F1 exists as of 2026-09-22.

Historical measurement **2026-08-05** after M315/M316 gold promote (`cargo run -p da-cli --example eval_batch`): 193 papers (200 fixtures, 7 missing PDF/HTML skipped), 1,399 gold entities, Micro P/R/F1 0.769/1.000/0.869, Macro F1 0.859, TP/FP/FN 1399/421/0.

## Problem backlog (open)

| ID | Problem | Where |
|----|---------|--------|
| G1 | GROBID semantic correctness under graph freeze: bibliography id/target integrity (S02/H15), then H16–H21 | **Active M511-j7wz2f**, slice S02, phase refining (`.gsd/STATE.md`); ADR-076 §14. Owner stop 2026-09-22 — execution only on explicit permission |
| E1 | Multi-word method extraction | **Closed M313** |
| E2 | Incomplete gold → FP inflation | **Closed M315 + M316** — remaining FP is noise/singleton |
| E3 | Weak-paper precision | **Closed as a corpus problem** — 2026-08-09 numbers above supersede the per-paper F1 chase |
| P1 | Remote graph is a read plane, not a full store: `DA_GRAPH_MODE=remote` opens `RemoteGraphStore`; Samyama HOT writes return `GraphModeError::RemoteWritesUnsupported` | `crates/kg-storage/src/graph_mode.rs`; health path is serve-graph + graph-ping (M307–M312) |
| L1 | Import locked by policy | D127 — need user go. ADR-076 §14 adds `deployed=false`, `auto_promote=forbidden`, `graph_write=forbidden`; authorizing M511 does not lift D127 |

## Code navigation

1. Prefer **GitNexus** `query` / `context` / `impact` (rules below).
2. Before edit: `impact({target, direction:"upstream"})` — warn on HIGH/CRITICAL.
3. Before commit: `detect_changes({scope:"compare", base_ref:"main"})`.
4. Onion: `PYTHONPATH=scripts python3 -m governor architecture`.
5. Do not invent free labels; do not open import without user yes.

<!-- project-current:end -->

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **daily-archive** (81541 nodes, 130372 edges, 639 execution flows — .gitnexus/meta.json, indexed 2026-09-22).

> Index stale? Run `node .gitnexus/run.cjs analyze --index-only` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? Bootstrap with `npx`, `bunx`, or `pnpm dlx` — e.g. `bunx gitnexus@latest analyze` (npm 11 npx crash; #1939).

## Always Do

- **MUST run impact before editing.** Use `impact({target: "symbolName", direction: "upstream"})` or `node .gitnexus/run.cjs impact "symbolName" --direction upstream --repo .`; report callers, processes, and risk. Never substitute grep for graph analysis.
- **MUST analyze graph changes before committing.** Use `detect_changes({scope: "all"})` (MCP) or `node .gitnexus/run.cjs detect-changes --scope all --repo .` (CLI fallback). `partial: true` or `truncated: true` is not a clean check — a zero means unseen, not unaffected; re-run it. For regression review: `detect_changes({scope: "compare", base_ref: "main"})` or `node .gitnexus/run.cjs detect-changes --scope compare --base-ref "main" --repo .`.
- MUST warn on HIGH/CRITICAL `risk` pre-edit; never use `riskSharedAxes` to waive a HIGH/CRITICAL `risk` warning. Compare File/symbol: MCP File omits axes; Graph-RAG expands File.
- **MUST treat `risk: UNKNOWN` as unresolved, not as low.** An empty caller set is not evidence the symbol is unused — it can also mean the callers are not resolvable by the index (plain-object property access, dynamic dispatch, cross-language calls). `impact` pairs `UNKNOWN` with a `riskNote` saying so. Confirm with a text search before treating the symbol as safe to change or delete; do not proceed on the strength of a zero.
- **MUST use `query({search_query: "concept"})` for concepts/flows, `context({name: "symbolName"})` for a named symbol, or `impact` for blast radius, on read-only callers, dependencies, imports, or execution flow.** Graph first; text search only for empty/`UNKNOWN`/literals.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method before MCP/CLI impact analysis.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis, and never read `UNKNOWN` as an all-clear — it means the walk could not answer, which is the one verdict that requires confirming by other means.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit before MCP/CLI graph change analysis.

## Resources

| Resource | Use for |
| --- | --- |
| `gitnexus://repo/daily-archive/context` | Codebase overview, check index freshness |
| `gitnexus://repo/daily-archive/clusters` | All functional areas |
| `gitnexus://repo/daily-archive/processes` | All execution flows |
| `gitnexus://repo/daily-archive/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
| --- | --- |
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->

<!-- parser-sidecars:start -->
# Parser sidecars (GROBID)

Hybrid PDF path expects **GROBID** HTTP service (default `http://127.0.0.1:8070`).
GROBID TEI output is consumed by `crates/da-parse` (Rust `GrobidParser`),
not by Python code anymore.

## Start / stop

```bash
# GROBID CRF (recommended for daily-archive pilots)
docker compose -f .docker/docker-compose.yml --env-file .env up -d grobid
curl -sS http://127.0.0.1:8070/api/isalive   # true

docker compose -f .docker/docker-compose.yml --env-file .env down
```

Env keys: see `.env.example` (`GROBID_*`, `HYBRID_AUTO_START_CONTAINERS`). GROBID CRF has **no API key**.

**GROBID REST profiles** (`da_parse::GrobidProfile`, D204):
- `rich` (backward-compatible default): `consolidateCitations=1`,
  `consolidateHeader=1`, `teiCoordinates=["figure","table","note"]`.
- `structural` (gold/bounded pipeline): both consolidation flags are `0`, while
  coordinates remain enabled. This keeps local TEI generation independent of
  Crossref 429/backoff; bibliography/header enrichment runs later through the
  separately cached OpenAlex/Crossref stage.

Never mix profiles under one cache key. Profile and consolidation flags must be
part of the parse stage configuration fingerprint. Rich mode may take minutes
under Crossref rate limits; structural mode is the managed scalable default for
corpus parsing.

## Auto ensure from code (Rust)

```rust
use da_parse::GrobidParser;
use da_ports::parser::ParserPort;

let parser = GrobidParser::from_env();
assert!(parser.is_alive().await);
let parsed = parser.parse_pdf(pdf_path, paper_id).await?;
```

Rules:

- Probe/start is infrastructure; **import_eligible stays false**.
- Do not treat sidecar TEI/markdown as graph truth — GROBID output feeds the
  Rust RuleBasedExtractor, which produces canonical Entity nodes in Samyama.

Details: `.docker/README.md`
<!-- parser-sidecars:end -->

<!-- logging:start -->
# Structured Logging (ADR-052)

Dual-layer tracing: stderr (compact) + rolling JSON file under one `RUST_LOG`/`LOG_LEVEL` filter. INFO lifecycle events carry semantic identity and write deltas by default.

## Configuration

```bash
# .env
LOG_DIR=logs          # directory for rolling files (default: logs/)
LOG_LEVEL=info        # filter level (RUST_LOG overrides)
LOG_SUPPRESS=lopdf::object=error   # extra target=level directives (default shown)
```

`LOG_SUPPRESS` is appended after the base filter and exists because a
third-party PDF parser (`lopdf`) emits one `WARN` per undecodable glyph or
font-encoding difference in real-world PDFs — observed ~2.5k lines/day across 19
distinct messages, which drowned our own signal and defeated the
query-the-log-by-level workflow below. It names an explicit third-party target
so no `da-*`/`kg-*` target is ever silenced, and uses `error` rather than `off`
so genuine PDF parse failures stay visible. Set it to an empty value to disable
suppression entirely.

## Log files

Daily rotation: `logs/da.log.YYYY-MM-DD`
JSON format with `flatten_event`, span hierarchy, `FmtSpan::CLOSE` timing.

## Querying (jq)

```bash
jq 'select(.paper_id == "2507.19457")' logs/da.log.2026-08-05
jq 'select(.level == "ERROR")' logs/da.log.2026-08-05
jq 'select(.span.name == "ingest_pdf")' logs/da.log.2026-08-05
jq 'select(.fields.duration_ms > 1000)' logs/da.log.2026-08-05
```

## Architecture (hexagonal)

- **da-cli** (infrastructure): owns subscriber, WorkerGuard, `logging::init_logging()`
- **da-application** (use cases): `#[instrument]` on `ingest_pdf`, `batch_ingest_pdfs`, `extract_from_parsed`
- **da-parse** (parse plane): `#[instrument]` on `GrobidParser::parse_pdf`
- **da-adapters** (adapters): `tracing::info!` on remaining port impls
- **da-domain, da-ports, kg-***: **zero tracing dependency** (pure types/traits)

## WorkerGuard

`init_logging()` returns `Option<WorkerGuard>`. Must be held in `main()` scope.
Dropping it early silently stops file logging — #1 tracing footgun.
<!-- logging:end -->
