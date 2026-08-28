# continue — session state

## Active direction (M464-z84in2): staged native scholarly parse platform

Branch `feat/da-parse` HEAD `174e1b7a7`. Canonical DB: M464 active, 19 slices.
S01 still **pending in GSD** (3/3 tasks pending) even though T01+T02 are
committed and green. S02-S19 remain sketches.

### Shipped this session (2026-08-28, code not GSD-sealed)

- T01 `54a470379`: `data/parse/api_compatibility_ledger.yaml` +
  `regression_cases.yaml` + `crates/da-parse/tests/data_contracts.rs`.
  Offline gate: **22 passed**. D296: no parallel `data/scholarly_parse`.
- T02 `174e1b7a7`: three preserved rich TEI fixtures under
  `crates/da-parse/tests/fixtures/preserved/` with `provenance.yaml` and
  sha256. `real_tei_tests` no longer silent-skips (9 passed). Structural
  companion stays **pending** — no structural-profile bytes exist; T03 owns
  that bind. Authors/idno/figures/coords pinned on fixture bytes; this-cut
  IR still omits those families.

### Lifecycle blocker (do not re-diagnose)

`gsd_task_complete` T01 → no running Attempt. Latest Attempt
`a1b5b96e-175a-45df-a2f0-5a3820bbd0f7` settled failed
(`failureClass=lifecycle-progression`). Recovery
`00a7d3ad-462a-43cc-837e-76f2272593f7` already resumed (guard rejects
second resume). 28 Aug 04:58 auto pause: SUMMARY-vs-pending drift; SUMMARY
now quarantined at
`.gsd/quarantine/projections/2026-08-28T05-03-35-672Z/`. Live tree has
`S01-T01-RECOVERY-BLOCKER.md` only.

**Unblock:** `/gsd auto M464-z84in2` after TUI restart (MEM953). If auto
still sees drift, `/gsd rebuild markdown` then auto. Do not invent Attempt
rows. Manual skip-delivered is for slice closeout, not while T03 is open.


**Architecture locked for execution:** ADR-075 accepted; D294 opens active
SONA/MicroLoRA/EWC++/ruVLLM training while auto-promote stays forbidden. D295:
Samyama=KG truth, RVF=canonical learning experience, pipeline cache=derived
parse artifacts, redb=rebuildable index/cursor only, CVF=trajectory interchange
schema. Requirements R105-R117 are in GSD. Final context is stored through GSD
at `phases/464-z84in2-staged-native-scholarly-parse-platform/464-CONTEXT.md`.

**GSD auto root cause fixed (MEM953):** GSD 1.16.2 parsed flat-phase directory
`464-z84in2-*` as bare `M464`, so `/gsd auto M464-z84in2` failed before any
journal flow. Both user and system `extensions/gsd/milestone-ids.js` now read
the canonical suffixed ID from `464-ROADMAP.md`. Fresh-process proof:
`findMilestoneIds -> M464-z84in2`; `gsd headless next M464-z84in2` reached
`Step-mode started` and `Session started`. The test process was intentionally
stopped before edits. **Current TUI has stale module cache: restart pi/GSD,
then run exactly `/gsd auto M464-z84in2` (never bare M464).** GSD upgrades may
wipe both hotfix copies.

**Repository state:** GSD created commit `9bbeb8027` (`gsd snapshot:
pre-dispatch`). Relevant research/ADR files remain untracked: the two
2026-08-27 CE research artifacts, ADR-075, and
`doc/research/m463-grobid-rust-ruvector-platform.md`. Unrelated untracked
`archive/m451-cleanup/data-corpora-*` and `artifacts/project-trajectory/` must
not be swept into a blanket add. Governor tests: 134 passed; preflight has 0
ADR violations and only pre-existing drift flags (M446 projection, target
bloat, graph Section raw_text gap).

Do not fork GROBID. Do not open D127. MinerU/Docling are donors, not TEI truth.

**Governor graph flags fixed (MEM949, commit this session):** old reader
reported paper_count=295 via count("Paper")//2 heuristic; truth = 589
(md5-тождественный с архивной копией snapshot в data/graphs/, эпоха
2026-08-22, совпадает с run-report completed=589). Reader теперь парсит
NDJSON структурно (sgsnap v2): paper_count=589, revision binding 589/589,
enrichment N/A по онтологии снапшота; честный флаг остался:
graph_section_text_missing — 28346 Section узлов БЕЗ raw_text в этом
сторе (текст жил в других узлах той эпохи). Выбор primary:
data/graphs приоритетнее artifacts/loads сессионных видов. Контракт
v2: graph-semantics.yaml. data/corpora дубликат карантинен →
archive/m451-cleanup/data-corpora-m033grobidprobe-dup-20260827.

## Previous: M462-ieh2tt load network is on by default

S01: allow_network default true; --disallow-network opt-out. Closed via
skip-delivered (`16c982357`, D290).

## Previous: M451 schema/manifest skeleton BEFORE code

ADR-068 accepted (revised after elegance review): four roles —
SCHEMA = norm (machine-readable), FS = truth, Governor = direct
validator (FS vs SCHEMA), MANIFEST = derived cache (~7 aggregate
levels only). Meta/prompt: no hardcoded structure anywhere; rules
live in schema data, checker is a generic engine.

Shipped (commit HEAD):
- 19 SCHEMA.yaml (structural dirs, v2, adr_ref: ADR-068)
- 7 MANIFEST.yaml caches (root, data, sources, arxiv, textbook, processed, governor)
- data/schemas/article_schema.yaml — SINGLE definition site: layers
  (A0-A7), lifecycle_order, invariants, forbidden edges, bi-temporal
  lifecycle_events, flat-default versioning
- Governor dimension schema_hierarchy: contract YAML + generic engine
  (valid_subdirs/valid_files/forbidden/ignore/tolerated/subdir_pattern)
- Gap closed: old_artifacts_graphs/ -> archive/m451-cleanup (byte-dupes);
  data/{index,catalog,index-rebuild-*} -> artifacts/m450-build-reports
- Pre-existing bugs fixed: doctor (ctor kwargs, report.flags, args.quick),
  test_prompt_eval path (M448 migration miss)
- Tests: 140 passed. Real repo: 0 schema_hierarchy flags.

## Next actions
1. Restart the current pi/GSD TUI so it loads the patched `milestone-ids.js`.
2. Run `/gsd auto M464-z84in2` (never bare `M464`) so auto mints the Attempt
   and can seal T01/T02. If SUMMARY-vs-pending drift returns: `/gsd rebuild markdown`.
3. Code next: T03 Java/direct TEI baseline reports (`parse_compat_baseline`,
   `artifacts/parse_compat/`). Optionally mint structural-profile TEI there.
4. Do not pull S02 contracts into S01.
3. Keep S01 small: first DATA compatibility/regression ledger, then fixtures,
   then baseline reports. Do not pull S02 contracts into T01.
4. Preserve existing cache identities. Batch/cache modes arrive in S06.
5. Keep learning persistent: RVF authority, redb rebuildable only. Training is
   active; promotion remains source-bound and explicit.

## Operators
`/gsd auto M464-z84in2`                              # after TUI restart
`gsd headless next M464-z84in2`                      # fresh-process diagnostic
`PYTHONPATH=scripts python3 -m pytest scripts/tests/test_governor.py -q`
`PYTHONPATH=scripts python3 -m governor --phase preflight --check-only`

## Do not forget
- D127: import_eligible stays false
- Destructive ops: only moves with preservation, never recursive deletion
- gsd-pi updates wipe hotfixes (markdown-renderer, doctor-engine-checks)
- Russian for user-facing replies

## M451 session (2026-08-24/25): ADR-069..074 — domain profiles, RLM, WASM policies

### Shipped (all committed, HEAD = 30ee100b + ADR-074 docs)
- **ADR-069**: ISC patterns — exact-quote evidence gate, genre detection,
  prose numbers, parse-integrity signals. Live: 2608.20845 4→22 entities.
- **ADR-070**: Domain knowledge profiles — data/domains/*/SCHEMA.yaml,
  multi-domain routing (coarse_topic_code→[domain,...]), live manifests.
- **ADR-071**: Survey spine (M-environments 39/39), table-cells surface,
  taxonomy nodes, EVALUATED_ON edges, Artifact type, hyperedge claims.
- **ADR-072**: XY-Cut reading order (opendataloader port) + boundary-tail
  recovery (103→0 lost boundaries) + margin_notes structured field.
- **ADR-073**: citracer resilience trio — batch DOI fetch, negative-cache
  purge, circuit breaker; all in Rust (`da source fetch-openalex`).
- **ADR-074**: WASM MicroLoRA swappable FSM policy modules (Proposed).
- cv_structure.rs: split_by_cv + find_toc_pages + assign_heading_levels.
- xy_cut.rs: recursive segmentation over LineBox (8 tests).
- boundary_agent.rs: FSM classifier for ambiguous anchors (6 tests).
- background_tasks.rs: Rust per-section summaries runner.
- Citation salvage: GROBID-mangled arXiv ids (+213 unique edges).
- Layout-aware PDF exporter: margin notes separated, dehyphenation.

### Key decisions
- Runtime path is Rust-only: python scripts removed from pipeline
  (m451_enrich_openalex deleted, replaced by fetch-openalex action)
- Toolchain registry is the single source for actions/pre-hooks/retry
- RLM projected onto FSM: stage = peek→plan(LLM)→delegate*→fold→emit;
  closed DSL operations in DATA, no eval; LLM proposes plans, Rust executes
- SONA three-loop learning maps to our CVF trajectories:
  Loop A=instant grounding_hint, Loop B=hourly pattern extraction,
  Loop C=EWC++ consolidation. D294 opens training; promotion stays explicit.

### Pending (next session)
1. ADR-074 Phase W2: ReasoningBank K-means over accumulated CVF trajectories
   (~1000 needed; currently ~19 summarizer + growing)
2. Train MicroLoRA candidates now; frozen eval is the promotion gate, not the training gate
3. Figure-repair loop (symmetric with critic repair)
4. Table-cells → extraction surface (ADR-071 §2.2 partially shipped for
   algorithms-for-decision-making; needs generalization)
5. Temporal filters on graph queries (ADR-073 §2.2 graphiti patterns)
6. RRF rerankers as ADR-071 ranking frame composition mechanism
7. content_ref indirection for future vector layer (edgequake pattern)
8. SCHEMA snapshot-diff (semantica pattern) for next schema bump
9. Remaining m451 python crutches: textbook_import/monograph_import are
   one-time tools (OK to keep); reference_layers/summary_eval are pilot/
   eval session scripts (OK to keep)
10. 22 residual fail-open boundaries (TOC page imprecision — acceptable)

### Architecture state
- FSM: source_import_tools.yaml = single truth for stages/tools/actions/
  retry/pre_hooks; rust_actions map added; retry_for() accessor
- Agents: summarizer+critic manifests DATA-driven; critic repair contract
  covers enriched/citations_resolved/summaries; figure-repair gap noted
- WASM: ADR-074 W3 infra SHIPPED (wasmi host, hot-swap, native fallback).
  W2 pattern extraction SHIPPED (sona-analyze). D294 opens MicroLoRA/EWC++
  training and ruVLLM evaluation; auto-promote remains forbidden.
  Next: train/evaluate the first source-bound policy candidate.
- Tests: domain 707(+boundary_agent 6), adapters 249, cli 176,
  pytest 142, validate 645×0, clippy clean on all touched crates
