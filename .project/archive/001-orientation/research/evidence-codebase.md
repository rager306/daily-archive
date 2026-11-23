# Evidence — codebase

<!-- Written by the codebase mapper during /skill:gsd-path-inspect. Read by define
     (brownfield mode), researchers (fifth standard dimension), the planner
     (conventions are binding), and reviewers. -->

Repo root: /root/daily-archive
Scanned: 2026-11-22
Checks run: static inspection only — the inspect brief forbids builds/test suites/cargo, so no `cargo check`/`cargo test` was executed. Read-only commands actually run (all in the verify sidecar at 0f06b764c912f9c2d6025ff8427646c0cbaa7363, clean tree): `git rev-parse HEAD` → `0f06b764c…` (= recorded Audited HEAD); `git status --porcelain` → empty; `git log --oneline -25`, `git log --format='%ad %h %s' --date=short -18`; `git ls-files` counts (crates `*.rs` = 909 files; `crates/*/tests/*.rs` = 323 integration-test files; `archive/**` = 12,622; `data/**` = 13,276; total = 34,084 tracked files); `git ls-files 'crates/*.rs' | xargs wc -l` → 532,770 total; `git ls-files 'crates/*/src/*.rs' | xargs wc -l` → 273,658 total (da-parse 65,395; da-adapters 21,564); `git grep -c '#\[test\]' -- 'crates/**/*.rs'` → 5,012; `'#\[tokio::test\]'` → 791; `git grep -c 'name = ' Cargo.lock` → 638 packages; `git ls-files '.project/**' | wc -l` → 0 and `git check-ignore -v .project` → not ignored; `git ls-files 'legacy/**' | wc -l` → 0; `git ls-files 'crates/*/examples/*.rs' | wc -l` → 125; `git branch -a`, `git tag` (no tags).

## Map

- **Stack**: Rust workspace, edition 2024, `rust-version = "1.85"`, pinned toolchain `1.99.0` with rustfmt+clippy (`rust-toolchain.toml`); 12 workspace crates (`Cargo.toml` members: kg-{ontology,storage,algorithms,embeddings,pipeline}, da-{domain,ports,application,adapters,parse,cli,mcp}); 638 locked packages (`Cargo.lock`). Storage = Samyama graph (path dep `/root/vendor-source/samyama-graph`, locked `1.16.0` in Cargo.lock) + RuVector crates (path deps under `/root/vendor-source/ruvector`); `redb 2.6.3` optional/feature-gated as rebuildable cache accelerator only (D295, `crates/da-adapters/Cargo.toml:53-65`). Python ≥3.11 via uv exists for the governor CLI and its tests only (`pyproject.toml`: PyYAML, jsonschema, pytest dev-group). Services: GROBID HTTP 0.9.1-crf pinned by digest (`.env.example` GROBID_IMAGE), TEI/fd_api embeddings on :8000, 9router LLM proxy :20128, optional MinerU/GLiNER2 Python subprocesses. Rust not the only "language in use" at runtime: GROBID (Java, containerized) is a hard parse dependency today.
- **Entry points**: (1) `da` CLI — `crates/da-cli/src/main.rs` (3,215 lines, clap derive; `enum Commands` at main.rs:130) with ~60 subcommand modules (source FSM, policy, ingest/batch-ingest, load-snapshot, graph-stats/query/graph-query, serve-graph, agent-run, parse-batch/parse-job/config/profile, schema-check/validate-objects, health…); (2) `da-mcp` binary — read-only MCP server over stdio (`crates/da-mcp/src/main.rs`: MockGraphStore unless `--snapshot`; `crates/da-mcp/src/tools.rs` 2,055 lines: get_schema, get_graph_stats, read_cypher with destructive-keyword blocklist, get_paper/get_sections/get_references…); (3) Python governor CLI — `PYTHONPATH=scripts python3 -m governor` (`scripts/governor/__main__.py`; checks are DATA in `scripts/governor/checks/*.yaml`, 41 rule/check files); (4) 125 example runners under `crates/{da-adapters,da-cli,kg-storage}/examples/`.
- **Architecture**: Hexagonal/onion. `da-domain` is pure (Cargo.toml deps: serde/chrono/thiserror/kg-ontology only — spot-checked, no tokio/reqwest/samyama); `da-ports` defines 38 port-trait modules (graph_store, parser, llm_client, embedder, acquisition, parse_job, pipeline_cache, …); `da-application` holds 104 use-case/stage modules; `da-adapters` (51 modules) implements infra: SamyamaGraphStore, GrobidParser, native PDF visual, LLM clients (anthropic, openai-compatible), arxiv/openalex acquisition, wasm policy host; `da-parse` (75 modules, 65k LOC) is the parse plane with the native Rust route alongside GROBID; `da-cli`/`da-mcp` are composition roots. `kg-*` are universal crates that must not depend on `da-*` (ADR-050, enforced in CI). Flow: `da` command → use case (da-application) → port traits (da-ports) → adapters (Samyama embedded graph or RESP read-only remote plane, GROBID/native parsers, LLMs) → data-driven registries (data/path_config.yaml, data/ontology/*.yaml, models.yaml). Governance: `scripts/governor/` enforces ADR invariants + artifact gates as YAML DATA.
- **Conventions** (binding on the planner): conventional commits with milestone scopes — `feat(m519):`, `fix(build):`, `docs(adr):`, `test(policy):`, `chore(cutover):` (git log); error handling: thiserror typed errors in domain/ports, anyhow at CLI edges; fail-closed env parsing is a stated norm ("blank/non-numeric/zero/overflow are rejected with a typed error — never a silent default", `.env.example`); honesty conventions are pervasive: NON-PERSISTENT warnings on snapshot import, D127 import/write gates (`DA_IMPORT_ELIGIBLE=false`, `DA_GRAPH_WRITES_ALLOWED=false`, `DA_USER_GO=false`), MCP server read-only with keyword blocklist (`da-mcp/src/tools.rs:50-52`); test style: heavy use of suffix families `*_contract.rs`, `*_data_contract.rs`, `*_oracle.rs`, `*_properties.rs`, `*_same_n.rs` (323 integration files, proptest in da-domain dev-deps), MockGraphStore must pass the same contract suite as Samyama (AGENTS.md §testing, ADR-051); DATA-first: ontology/rules/paths in YAML under data/ and scripts/governor/checks/, model roles in models.yaml; logging: tracing dual-layer stderr+rotating JSON (ADR-052, LOG_* keys); docs gates: `.pre-commit-config.yaml` (cargo-fmt, clippy-lib+tests, governor-trajectory, docs-drift-check) and advisory `scripts/docs_drift_check.sh`; two-tier docs gates mirrored in `.github/workflows/architecture-guardrail.yml`.
- **Maturity**: Large and heavily tested on paper: 273,658 src LOC + 5,012 `#[test]` + 791 `#[tokio::test]` in crates/. da-parse is the most mature/active plane (864 rs files, 146 integration test files, dense oracle/property coverage); kg-* are small stable crates (8–27 files each). CI is one workflow (`.github/workflows/architecture-guardrail.yml`: fmt, check --workspace, clippy -D warnings, per-crate tests, hexagonal dep-direction grep, governor preflight) but is explicitly self-hosted-only because of vendor path deps (workflow header comment). Scaffolding/cold: `archive/` holds 12,622 tracked files (10,611 artifacts-legacy + 1,854 data-legacy) per `archive/PYTHON-ERA-COLD.md`; `tools/learning-critic-ruvllm` is a separate Cargo project outside the workspace; `governor/fixtures/` at repo root holds check fixtures; many Proposed ADRs describe designed-but-unbuilt planes.
- **Recent activity**: Dense sprint 2026-10-06→10-13 (parse-policy learning, ADR-080 cycles, feat/learning-critic-e1 merged 10-13), single commits 10-14 (vendor pin), 10-24 (M451 textbook import), 10-31 (samyama 8b94c081 pin), then 2026-11-22: `chore(cutover): install gsd-path as the governing engine, freeze gsd-pi` (34d0718a5) and HEAD `feat(m519): complete native-route wave — vendor samyama-1.16 rebuild, smoke coverage, gate 23/23` (0f06b764c). The center of work is the native PDF parse route replacing GROBID (ADR-075..080, M519), plus the governance-engine migration. Branches: master, feat/da-parse (HEAD lineage), feat/learning-critic-e1, gsd-path/M001, wip/t01-prompt-registry; no git tags.

## Finding: builds are host-bound to absolute `/root/vendor-source` paths

- **Claim**: Every build of the workspace requires vendor checkouts at exact absolute paths outside the repo: `samyama = { path = "/root/vendor-source/samyama-graph" }` and 8 RuVector path deps (`Cargo.toml` [workspace.dependencies]), and CI therefore runs only on self-hosted runners ("on CI they are separate repos. For now this guardrail runs only on self-hosted (vendor paths)" — `.github/workflows/architecture-guardrail.yml:52-56`).
- **Source**: Cargo.toml (workspace.dependencies samyama/ruvector block); Cargo.lock `samyama 1.16.0`; .env.example `SAMYAMA_VENDOR_PATH=/root/vendor-source/samyama-graph`, `RUVECTOR_VENDOR_PATH=/root/vendor-source/ruvector`.
- **Confidence**: high
- **Why it matters here**: any verification, replication, or planning that assumes `cargo check` works from a clean checkout is wrong; environment setup (vendor clones at fixed paths) is a hard precondition the docs under-emphasize. A planner must not schedule generic CI or fresh-clone builds.

## Finding: the largest integration-test surface (da-parse, 146 files) is not run in CI

- **Claim**: CI runs `cargo test -p da-parse --lib` only; none of the 146 files in `crates/da-parse/tests/` (contract/oracle/properties suites for the native route) execute in the guardrail workflow, though da-application/-adapters/-cli run `--tests`.
- **Source**: `.github/workflows/architecture-guardrail.yml` "Unit + integration tests" step (explicit per-crate list); `git ls-files 'crates/da-parse/tests/*.rs' | wc -l` → 146.
- **Confidence**: high
- **Why it matters here**: "the tests exist" and "the tests gate merges" are different claims; green CI does not certify the parse-plane contracts. Inspection and any verify planning must know the biggest test mass runs only locally/manually.

## Finding: AGENTS.md and ADR-INDEX reference a `legacy/` tree that no longer exists

- **Claim**: `legacy/` has zero tracked files at HEAD and is absent from the checkout, while AGENTS.md says "`legacy/` is a restore stub — not on the runtime path" and ADR-INDEX.md says Python-era ADRs "remain valid for the frozen `legacy/` codebase … see `legacy/doc/adr/`".
- **Source**: AGENTS.md (project-current section); `doc/adr/ADR-INDEX.md:3-6`; `git ls-files 'legacy/**' | wc -l` → 0; top-level ls has no `legacy/`. Python era actually lives in `archive/` (`archive/PYTHON-ERA-COLD.md`).
- **Confidence**: high
- **Why it matters here**: two authoritative navigation docs point to a missing directory; an inspector or planner following them hits dead paths, and the Python-era history location is mis-stated (it is under archive/, not legacy/).

## Finding: AGENTS.md claims ".project/ is committed" but .project is not in the repo at the audited HEAD

- **Claim**: `git ls-files '.project/**'` = 0, `.project` matches no .gitignore rule, and the directory is absent from this sidecar checkout of 0f06b764c — yet AGENTS.md records the 2026-11-22 ruling "`.project/` is committed".
- **Source**: AGENTS.md (project-current, Rulings 2026-11-22); commands listed in Checks run.
- **Confidence**: high (for the audited HEAD in this worktree); the ruling may refer to state created after this commit in the primary working tree.
- **Why it matters here**: GSD Path reads its own state from `.project/`; if the binding claim and the tree disagree, re-anchors and resume flows will silently find no planning state in the repo. Define should settle where `.project/` really lives (tracked vs host-only).

## Finding: the parse plane dominates the codebase

- **Claim**: `crates/da-parse` holds 864 of 909 crate `.rs` files tracked under `crates/` and 65,395 of 273,658 src LOC (≈24% of src, ≈50% of all crate files), with 146 of the 323 integration-test files; da-cli (266 files, 74 test files) is second.
- **Source**: `git ls-files 'crates/*'` per-crate counts; `git ls-files 'crates/da-parse/src/*.rs' | xargs wc -l` → 65,395.
- **Confidence**: high
- **Why it matters here**: effort, risk, and review weight concentrate in da-parse; a planner who spreads work evenly across the 12 crates misreads the project. It also means cargo commands touching da-parse are the expensive ones.

## Finding: test style is contract/oracle/property-heavy with honesty suffix families

- **Claim**: 5,012 `#[test]` + 791 `#[tokio::test]` functions across crates, organized as `*_contract.rs` / `*_data_contract.rs` / `*_oracle.rs` / `*_properties.rs` / `*_same_n.rs` / `*_metamorphic.rs` files (323 integration files), plus several live/real-model suites (`native_*_real_model.rs`, `parse_batch_budget_live_test.rs`) that require running services.
- **Source**: `git grep -c '#\[test\]'`/`#[tokio::test]`; `git ls-files 'crates/*/tests/*.rs'` listing (e.g. `crates/da-adapters/tests/native_fulltext_real_model.rs`, `crates/da-cli/tests/parse_batch_budget_live_test.rs`).
- **Confidence**: high
- **Why it matters here**: new tests must follow the existing suffix taxonomy (a second convention would violate repo norms), and "run the tests" needs qualification: live/real-model tests need GROBID, LLM endpoints, or model files and cannot run in a bare sandbox.

## Finding: Python is still 67.6k LOC, and part of it sits outside the stated allowlist

- **Claim**: `scripts/*.py` totals 67,631 lines: the governor package (`scripts/governor/`, hexagonal: ports/domain/application/adapters) and its tests (`scripts/tests/`, 38 files), plus milestone scripts `m450_build_processed.py`, `m451_*.py` (7 files) directly under `scripts/` — while AGENTS.md records D237 `no_new_product_python_scripts`: "Two allowlisted roots exist: `scripts/governor/` and `scripts/tests/`. A new tool elsewhere under `scripts/` is a hard finding."
- **Source**: `git ls-files 'scripts/*.py' | xargs wc -l` → 67,631; AGENTS.md (D237 rule); `scripts/` listing.
- **Confidence**: high on the counts and the rule text; **unknown** whether m450/m451 predate the rule and are grandfathered.
- **Why it matters here**: the "Rust-only product" framing in pyproject.toml is true only with "governor is Python" attached; a planner adding automation under `scripts/` outside the two roots would trip the governor gate. The m451 scripts' status should be settled, not guessed.

## Finding: D127-style safety gates and fail-closed honesty are binding runtime conventions

- **Claim**: Import/Graph-write/User-go default false and must be flipped per-session by explicit human action (`DA_IMPORT_ELIGIBLE/DA_GRAPH_WRITES_ALLOWED/DA_USER_GO=false`, `.env.example` SAFETY block citing ADR-039/D127); snapshot loads print NON-PERSISTENT warnings (`da-cli/src/main.rs:66-73`); the MCP server rejects destructive Cypher keywords client-side (`da-mcp/src/tools.rs:50-52`) and the remote RESP plane rejects mutations server-side (ADR-061); ambiguous env values "fail closed — no silent pick" (`.env.example` DA_GROBID_URL/DA_GRAPH_ENDPOINT blocks).
- **Source**: `.env.example`; `crates/da-cli/src/main.rs:28-73`; `crates/da-mcp/src/tools.rs:50-52`.
- **Confidence**: high
- **Why it matters here**: any planned feature that "conveniently" defaults a gate to true, auto-starts a daemon, or silently picks between conflicting env values violates a binding project convention, not just style.

## Finding: the repo carries its data and cold archive in git

- **Claim**: `data/` has 13,276 tracked files (ontology YAML, prompts, policies, gold_standard, graphs, processed), `archive/` 12,622 (mostly artifacts-legacy/data-legacy), `artifacts/` 5,969 — 31,867 of 34,084 tracked files are data/archives, and the MANIFEST snapshot records `data` at 3,924.2 MB (MANIFEST.yaml, generated 2026-08-23). Runtime policy journals were recently excluded (`0a5c4522a chore: exclude runtime policy journals from git`).
- **Source**: `git ls-files | cut -d/ -f1 | sort | uniq -c`; MANIFEST.yaml directories block.
- **Confidence**: high (counts); medium on actual current size (MANIFEST is a stale snapshot by its own note "FS is truth… this is a snapshot").
- **Why it matters here**: clone/fork cost, git operations, and any "regenerate data" plan must account for a multi-GB tracked tree; history rewrites or data migrations are expensive here.

## Finding: designed-but-unbuilt and stale areas exist and are marked unevenly

- **Claim**: ADR statuses are mixed and some code/docs are stale: ADR-042/043/044/046/047/048/049 are "Proposed"; ADR-060 is "Partially implemented (M388)"; `tools/learning-critic-ruvllm` is a standalone Cargo project outside the workspace (own Cargo.lock); `crates/da-cli/src/main.rs:263,1837` still says "via da-graph builders" for a `da-graph` crate the workspace removed (CI comment: "M432: da-graph removed"); the workspace Cargo.toml comment claims "~86 heavyweight example runners" while 125 exist.
- **Source**: `doc/adr/ADR-INDEX.md` status column; `tools/learning-critic-ruvllm/Cargo.lock`; `crates/da-cli/src/main.rs:263,1837`; `.github/workflows/architecture-guardrail.yml` M432 comment; `Cargo.toml` dev-profile comment vs `git ls-files 'crates/*/examples/*.rs' | wc -l` → 125.
- **Confidence**: high on each observation; low on intent behind them (user may know exactly why).
- **Why it matters here**: ADR status is the map of what may be built next vs what is frozen; stale references (da-graph, ~86) will mislead impact analysis and effort guesses.

## Finding: ADR-080 policy-learning concluded "measured_no_gain" and the corpus cycles are frozen

- **Claim**: The learned parse-policy track (ADR-078 candidate tables, ADR-080 two-arm paired observations) ended with `measured_no_gain confirmed and stable` (commit 0fcd1546a), TOC arm winning 74% on one metric but native best on validation (548cbaff6), post-M516 correction retracting arm-agreement gain claims (ADR-INDEX row 080), and `eda16b4a4 fix(policy): bind paired evaluation to outcomes and freeze corpus cycles`.
- **Source**: git log subjects 0fcd1546a/548cbaff6/eda16b4a4; `doc/adr/ADR-080-…md`; `data/policies/parse_pair_metric.yaml` (recently touched).
- **Confidence**: high on the recorded verdicts; the code (`crates/da-application/src/parse_policy_*.rs`, `da policy` CLI) remains present and candidate-only.
- **Why it matters here**: a planner might assume the learning track is an active growth direction; the repo's own evidence says it measured no gain and is frozen — new policy work would contradict recorded findings unless the user reopens it.

## Finding: governance engine cutover is brand new and half-visible in the tree

- **Claim**: The 2026-11-22 cutover commit freezes gsd-pi (`.gsd/` "no longer governs"; `AGENTS.pre-path.md` keeps historical detail) and installs gsd-path, but at the audited HEAD `.project/` is absent from the tree (see finding above) and CHANGELOG's newest entries still describe the gsd-pi hook mechanism (`.gsd/PREFERENCES.md` pre-dispatch hooks, CHANGELOG.md:1-60).
- **Source**: AGENTS.md (workflow-engine cutover section); `34d0718a5` commit; CHANGELOG.md top entries; `.gsd-path/` dir present with 2 tracked files.
- **Confidence**: high
- **Why it matters here**: docs, changelog, and tree state disagree about which engine governs; inspection, define, and planning must treat gsd-path as authoritative per AGENTS.md and not resurrect gsd-pi machinery.

## Apparent intent

<!-- Inference, clearly flagged — what the project seems to be becoming. -->

- **Becoming a native-Rust scholarly parse platform that retires the GROBID/Java dependency** — based on ADR-075/076/077/079 (scholarly parse plane, grobid cascade freeze, independent PDF layout input plane, queen faculties), the M519 "native-route wave … gate 23/23" HEAD commit, and ~150 `native_*` tests/files in da-parse/da-cli; the retirement packages (`native_retirement_readiness*`, `grobid_retirement_package_contract`) exist as contracts awaiting execution.
- **Keeping the graph/agent planes mostly frozen while the parse plane churns** — based on kg-* stability (8–27 files), Proposed-status ADRs for research/healing planes, and the last 200 commits clustering almost entirely in parse/policy/docs.
- **Governance is a first-class product surface, not tooling** — based on the 67.6k-LOC hexagonal governor with YAML rule DATA, trajectory/closeout/absorption services, pre-commit and CI gates, and its own ADRs; GSD Path pipeline state (`.gsd-path/`, AGENTS.md gsd-path sections) is being wired in as of the cutover.
- **The project is managed agent-first (machine-readable truth over human docs)** — based on MANIFEST.yaml "FS is truth, SCHEMA is norm, this is a snapshot", OKF.md/SCHEMA.yaml pairs at root/governor/scripts, governed_process_record contracts, and honesty/receipt conventions everywhere (`.agents/`, `.compound-engineering/` state committed).

## Open questions for define

<!-- What only the user can settle about this codebase. -->

- Is the native route now authoritative for PDF parsing (GROBID retirement executable), or does GROBID remain the production default until the retirement packages pass? ADR-076 says "graph freeze" — what remains before flipping?
- After ADR-080's measured_no_gain, is the `da policy` learning surface frozen permanently, kept for future data, or scheduled for removal?
- Should the stale `legacy/` references (AGENTS.md, ADR-INDEX.md) be rewritten to point at `archive/python-era-cold-*`, and are the m450/m451 scripts grandfathered against D237 or to be moved/deleted?
- Where must `.project/` (gsd-path state) live — committed in-repo as AGENTS.md's ruling says, or host-only? The audited HEAD contains neither the dir nor an ignore rule.
- Are the Proposed ADRs (044 schema healing, 049 pipeline DSL, 042/043 research planes) intended near-term work, or parking lot? ADR-060 "Partially implemented" — resume or retire the figure pipeline remainder?
- Which machine(s) are the supported build hosts, given the absolute `/root/vendor-source` path deps and self-hosted-only CI — and is there a plan to make vendor paths relocatable?

## Blocked areas

<!-- What could not be inspected and why. "none" if clean. -->

- Runtime behavior/build health — the inspect brief forbids cargo/build/test execution; all "what runs" claims are static (manifests, CI config, test listings). The role's verify-sidecar carve-out for commands was overridden by the explicit "static inspection only" constraint.
- `/root/vendor-source/{samyama-graph,ruvector}` vendor trees — outside this worktree; their state/versions could not be verified (only the pins in Cargo.toml/Cargo.lock are evidence).
- Live services (GROBID, 9router, embeddings, RESP plane) and `data/` payload contents (multi-GB) — not started/opened per constraints; capacities and health unknown.
- `logs/` is empty in the checkout and `MANIFEST.yaml` snapshots are dated 2026-08-23, so current data size/health is `unverifiable` from this tree.
