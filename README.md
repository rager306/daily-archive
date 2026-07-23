# daily-archive

`daily-archive` is a **local-first Universal Knowledge Base** with a **7-layer typed knowledge pipeline**: Source → Parser → Structure → Extraction → Graph → Review → Agents. Scientific papers are the first domain; textbooks, code repositories, and datasets are planned.

## Architecture (ADR-023)

```
Source → Parser → Structure → Extraction → Graph → Review → Agents
  (PDF/HTML/Code)  (Marker/GROBID)  (TreeKnowledge)  (Core-then-Modes)  (FalkorDB)  (fail-closed)  (SymFSM)
```

**Key principles:**
- **Statistical-first** (ADR-024): YAKE keywords, TF-IDF, embeddings before every LLM call
- **Multi-provider LLM** (ADR-025): MiniMax-M3 primary, GLM-5.2 secondary, per-provider rate limits
- **FalkorDB** (ADR-022): production GraphDB with typed relations (25 types in 5 groups)
- **Core-then-Modes** extraction (Agents-K1): typed entities + relations, ~50% fewer LLM calls
- **SymFSM agents** (ADR-026): FSM-controlled reasoning, LLM as interpreter not brain (deferred)
- **Fail-closed boundaries**: no graph writes without explicit authorization

## Current state

- **Package**: `research_graph/` — 110 modules in 12 packages (post M099-M100 migration)
- **Graph DB**: NetworkX intermediate (ADR-016), FalkorDB target (ADR-022), LadybugDB being retired
- **LLM**: MiniMax-M3-512k + GLM-5.2 via `provider_config.py` (hot-pluggable)
- **Embeddings**: BGE-M3 1024d via local fd/TEI service (ADR-019)
- **Corpus**: 220+ PDFs in canonical arXiv catalog
- **Tests**: 835+ passing

## Test architecture alignment

M128 tracks test-suite alignment with the hexagonal/onion architecture. The taxonomy, inventory, guardrail, and pilot commands live in:

```text
data/test-architecture-alignment/
```

Start with:

```bash
uv run python scripts/audit_test_architecture.py --output-dir data/test-architecture-alignment
uv run python scripts/verify_test_architecture.py --output-dir data/test-architecture-alignment
```

Use the guardrail as a ratchet: shrink legacy-mixed and dynamic script-import allowlists over time instead of broad rewrites.

## Local maintainability telemetry

`riskratchet` is integrated as **diagnostic-only maintainability telemetry**. It reports function scores, severity bands, and baseline deltas, but it is not a correctness, safety, replay, or release gate.

Run it directly with either command:

```bash
uv run python scripts/run_quality_gate.py --output-dir tmp/riskratchet-local
uv run python -m research_graph quality maintainability src/research_graph/infrastructure/quality/riskratchet_adapter.py --json
```

The local pre-commit config also includes `m127-riskratchet-maintainability`. It prints a compact summary during Python-file commits and writes JSON/Markdown reports to:

```text
/tmp/daily-archive-riskratchet/
```

Expected semantics in every report:

```text
blocking=false
pass_fail_affected=false
```

Use riskratchet output to spot maintainability pressure, not to claim feature correctness or readiness.

## Current first-domain runtime

The current CLI processes research papers from arXiv categories and produces local session artifacts and optional delivery output.

```text
Record    -> arXiv API (feedparser) — fetch papers for date/categories
Reduce    -> Semantic Scholar (citations) + YAKE (keywords)
Score     -> weighted scoring: citations, recency, novelty, preference
Summarize -> MiniMax LLM: HEADLINE / WHAT_IT_DOES / WHY_IT_MATTERS / ANALOGY
Deliver   -> Telegram channel + local session log
```

These integrations are **domain/runtime surfaces**, not authorization to treat external services or parser output as Universal KB truth.

## Universal KB safety boundaries

M034 defines the binding safety baseline for future implementation work:

- `graph_import_allowed=false`
- `graphdb_written=false`
- `ladybugdb_written=false`
- `production_import_attempted=false`
- `import_eligible=false`

Core rules:

- Scientific articles are the first domain; the architecture must not overfit to PDF/arXiv-only assumptions.
- Parser, sidecar, adapter, and LLM outputs are candidate evidence only.
- No direct extractor/parser/sidecar/LLM to GraphDB write path is allowed.
- GraphDB selection remains deferred.
- Agentic orchestration remains deferred until deterministic contracts, queues, and review gates exist.
- No review packet means no readiness handoff; no readiness handoff means no import recommendation.

Authoritative documents:

- ADRs: `doc/adr/ADR-TEMPLATE.md` (template), `doc/adr/ADR-INDEX.md` (index). All new ADRs MUST use this template.
- `doc/adr/m034/ADR-000-universal-kb-north-star.md`
- `doc/adr/m034/ADR-INDEX.md`
- `doc/contracts/m034-universal-kb/SAFETY-INVARIANTS.md`
- `doc/contracts/m034-universal-kb/CONTRACTS.md`
- `doc/architecture/m034-universal-kb/OPEN-QUESTIONS.md`
- `doc/architecture/m034-universal-kb/NEXT-MILESTONE-HANDOFF.md`

## M035 executable no-write prototype

M035 adds an executable local prototype for the M034 safety rules:

- frozen stdlib dataclass contracts in `src/arxiv_archive/universal_kb_contracts.py`;
- local SQLite durable queue in `src/arxiv_archive/universal_kb_queue.py`;
- Adaptix sidecar boundary mapping in `src/arxiv_archive/universal_kb_sidecar_boundary.py`;
- diagnostic-only review assistance in `src/arxiv_archive/universal_kb_review_assistance.py`;
- no-write readiness handoff in `src/arxiv_archive/universal_kb_substrate_rehearsal.py`;
- integrated metadata-only rehearsal in `src/arxiv_archive/universal_kb_rehearsal.py`.

The current MiniMax helper default is `MiniMax-M3-512k` for Anthropic-compatible helper/tool paths. Live S06 evidence showed that exact id works on the Anthropic-compatible endpoint and may return `MiniMax-M3` as the normalized model name; the tested OpenAI-compatible endpoint accepts `MiniMax-M3` and rejects exact `MiniMax-M3-512k`.

Run the full local M035 verification with:

```bash
python3 scripts/verify_m035_universal_kb_prototype.py
```

The verifier runs stable M034 ADR package checks, M035 Universal KB tests, ruff, and a fresh artifact inspection under:

```text
artifacts/m035-universal-kb-prototype/rehearsal/
```

Expected safety result:

```text
graph_write_allowed=false
promotion_allowed=false
production_import_attempted=false
```

These artifacts are rehearsal evidence only. They are not GraphDB writes, import recommendations, production queue state, or model approval authority.

## Universal KB smoke command surface

M036 proved a 5-article real-corpus no-write smoke over existing article catalog artifacts. M037 consolidated the control surface so routine work uses one module command instead of separate selector, runner, audit, and verifier scripts. M040 normalized per-article continuity metadata and expanded the no-write smoke to 10 articles. M041 adds a separate mixed 20-article connectivity smoke with 10 retained baseline articles, 5 articles linked from already loaded sources, and 5 Hermes review-section articles. M042 adds no-write linked metadata repair and connectivity group readiness reports over that M041 corpus. M043 applies the M033 combined sidecar architecture to the M042 connected component through candidate-only sidecar comparison packets.

Routine fast smoke for the normalized 10-article baseline:

```bash
uv run python -m arxiv_archive.universal_kb_smoke all --limit 10 --profile fast
```

Full pre-commit proof, including the M035 verifier:

```bash
uv run python -m arxiv_archive.universal_kb_smoke verify --profile full
```

The legacy command remains as a compatibility wrapper:

```bash
python3 scripts/verify_m036_real_corpus_no_write_smoke.py
```

The normalized baseline smoke writes:

```text
artifacts/m036-real-corpus-no-write-smoke/manifest.json
artifacts/m036-real-corpus-no-write-smoke/run/summary.json
artifacts/m036-real-corpus-no-write-smoke/audit.json
artifacts/m036-real-corpus-no-write-smoke/audit.md
```

The mixed connectivity smoke evidence lives separately:

```text
artifacts/m041-mixed-connectivity-smoke/manifest.json
artifacts/m041-mixed-connectivity-smoke/run/summary.json
artifacts/m041-mixed-connectivity-smoke/audit.json
artifacts/m041-mixed-connectivity-smoke/report.md
```

M042 readiness commands for the mixed corpus:

```bash
uv run python scripts/repair_m042_linked_metadata.py --no-network
uv run python scripts/audit_m042_connectivity_groups.py
```

M042 readiness evidence:

```text
artifacts/m042-linked-metadata-readiness/repair-report.json
artifacts/m042-linked-metadata-readiness/connectivity-audit.json
artifacts/m042-linked-metadata-readiness/readiness-report.md
```

No graph import is authorized by M042. The current M042 connectivity audit has one 6-article local-reference component, 14 isolated articles, and a 5-article Hermes co-selection group that does not count as reference edges.

M043 combined sidecar readiness commands:

```bash
uv run python scripts/probe_m043_sidecar_runtime_readiness.py
uv run python scripts/build_m043_sidecar_packets.py
```

M043 evidence:

```text
artifacts/m043-combined-sidecar-probe/target-subset.json
artifacts/m043-combined-sidecar-probe/runtime-readiness.json
artifacts/m043-combined-sidecar-probe/sidecar-packets.json
artifacts/m043-combined-sidecar-probe/architecture-fit-report.md
```

No graph import is authorized by M043. GROBID remains target-blocked without a configured live service URL; five linked target articles remain PDF-parser-blocked until bounded local PDF acquisition exists. OpenDataLoader PDF and Adaptix are locally available for the baseline path, and quant-mind remains a pattern source only, not a runtime dependency.

M044 architecture guardrail preflight for sidecar or graph-readiness work:

```bash
uv run python scripts/verify_m044_sidecar_architecture_guardrail.py
```

M044 guardrail evidence:

```text
artifacts/m044-grobid-architecture-guardrail/architecture-context-pack.json
artifacts/m044-grobid-architecture-guardrail/architecture-context-pack.md
```

Run this before GROBID/OpenDataLoader/Adaptix/quant-mind sidecar execution so M033/M034/M043 decisions are checked from artifacts, not remembered from chat context.

M044 also starts local GROBID CRF and leaves it running for follow-up sidecar work:

```text
service_url=http://127.0.0.1:8070
bg_shell_process_id=099f0de5
image=grobid/grobid:0.9.0-crf
```

M044 live GROBID evidence:

```text
artifacts/m044-grobid-architecture-guardrail/grobid-service-readiness.json
artifacts/m044-grobid-architecture-guardrail/live-grobid-candidate-packets.json
artifacts/m044-grobid-architecture-guardrail/final-report.md
```

## Parser sidecars (GROBID / OpenDataLoader)

Compose file: `.docker/docker-compose.yml`.

```bash
# Start GROBID (CRF image used in M033/M044 pilots)
docker compose -f .docker/docker-compose.yml --env-file .env up -d grobid
curl -sS "$GROBID_URL/api/isalive"   # default http://127.0.0.1:8070/api/isalive → true

# OpenDataLoader: prefer host library (not a long-lived HTTP API)
uv pip install opendataloader-pdf

# Optional workspace container
docker compose -f .docker/docker-compose.yml --env-file .env --profile odl up -d opendataloader
```

Env defaults live in `.env.example` (`GROBID_URL`, `GROBID_AUTO_START`, `HYBRID_AUTO_START_CONTAINERS`, …).  
Runtime probe/auto-start: `research_graph.infrastructure.corpus.parsing.sidecar_services`.  
Hybrid merge remains fail-closed (M212): container up ≠ graph import.

### Operator path (`article run` + live hybrid)

Architecture: **application** stays pure; live GROBID/ODL ports are built only in
`workflows/composition/hybrid_live_ports.py` and injected into the single-article pipeline.

```bash
# Install ODL library (optional extra)
uv sync --extra hybrid

# GROBID must be up (or HYBRID_AUTO_START_CONTAINERS=true)
docker compose -f .docker/docker-compose.yml --env-file .env up -d grobid

# Live hybrid: --mode hybrid enables live ports by default
uv run python -m research_graph article run path/to/paper.pdf --mode hybrid -o artifacts/single-article/demo

# Honest deferred (no sidecars)
uv run python -m research_graph article run path/to/paper.pdf --mode hybrid --no-live-hybrid
```

Flags:

| Flag | Meaning |
|------|---------|
| `--mode hybrid` | Prefer hybrid body route; **enables live ports by default** |
| `--live-hybrid` / `--no-live-hybrid` | Force live inject on/off (CLI source overrides hybrid default) |
| `--ensure-containers` / `--no-ensure-containers` | Allow/forbid `docker compose up grobid` on probe miss |

Success rules (unchanged):

- `hybrid_claimed_success` only when body markdown meets evidence threshold (~5000 chars)
- `import_eligible` / `graph_writes_allowed` always false on this path
- Without ports → `hybrid_deferred` (never fake hybrid completeness)

### M213/M214 hybrid batch gate (10 → 20 local PDFs)

Selections (offline, tracked):

- 10-rung: `artifacts/m213-hybrid-gate/selection.json`
- 20-rung: `artifacts/m213-hybrid-gate/selection-20.json` (first 10 identical to selection.json)

```bash
# Offline / CI (inject fakes in tests — unit suite)
uv run python -m pytest tests/test_m213_hybrid_batch_gate.py \
  tests/test_m213_hybrid_gate_selection.py \
  tests/test_m214_hybrid_gate_selection20.py -q

# Live batch (GROBID + ODL required; composition root only)
uv run python - <<'PY'
from pathlib import Path
from research_graph.workflows.composition.hybrid_batch_gate import (
    HybridBatchGateRequest,
    run_hybrid_batch_gate,
)
# 10-rung: selection.json + min_hybrid_success=7
# 20-rung: selection-20.json + min_hybrid_success=14
res = run_hybrid_batch_gate(HybridBatchGateRequest(
    selection_path=Path("artifacts/m213-hybrid-gate/selection-20.json"),
    work_dir=Path("artifacts/m213-hybrid-gate/runs-live-20"),
    enable_live_hybrid=True,
    ensure_hybrid_containers=True,
    min_hybrid_success=14,
    repo_root=Path("."),
))
print(res.to_dict()["gate_pass"], res.to_dict()["hybrid_success_count"], res.to_dict()["hybrid_deferred_count"])
PY
```

Output: `batch-summary.json` under the chosen `work_dir` (per-paper routes + aggregate).  
Runtime dirs `runs/`, `runs-live/`, `runs-live-20/` are gitignored.  
Import/writes remain fail-closed regardless of hybrid success count.

### M215 hybrid selection vs catalog coverage

After a hybrid gate selection, reconcile papers against the canonical catalog index
(composition root only; reuses M210 `reconcile_paths`):

```bash
uv run python - <<'PY'
from pathlib import Path
from research_graph.workflows.composition.hybrid_catalog_coverage import (
    HybridCatalogCoverageRequest,
    run_hybrid_catalog_coverage,
)
res = run_hybrid_catalog_coverage(HybridCatalogCoverageRequest(
    hybrid_selection_path=Path("artifacts/m213-hybrid-gate/selection-20.json"),
    catalog_index_path=Path("data/article_catalog/index.json"),
    catalog_root=Path("data/article_catalog"),
    output_path=Path("artifacts/m215-catalog-coverage/selection-20-coverage.json"),
    repo_root=Path("."),
))
print(res.package.verdict, res.cataloged_count, res.blocker_count)
PY
```

Missing selection refs become `typed_catalog_blocker` (never silently already_cataloged).

### M216 hybrid coverage + readiness handoff

Compose catalog coverage with no-write graph-data readiness over **precomputed** hybrid bodies
(e.g. `runs-live-20/{paper_id}/body/{paper_id}.hybrid.body.md`). Does **not** start GROBID/ODL.

```bash
uv run python - <<'PY'
from pathlib import Path
from research_graph.workflows.composition.hybrid_readiness_handoff import (
    HybridReadinessHandoffRequest,
    run_hybrid_readiness_handoff,
)
res = run_hybrid_readiness_handoff(HybridReadinessHandoffRequest(
    hybrid_selection_path=Path("artifacts/m213-hybrid-gate/selection-20.json"),
    body_root=Path("artifacts/m213-hybrid-gate/runs-live-20"),
    catalog_index_path=Path("data/article_catalog/index.json"),
    catalog_root=Path("data/article_catalog"),
    output_path=Path("artifacts/m216-hybrid-readiness-handoff/handoff.json"),
    repo_root=Path("."),
))
print(res.handoff_verdict, res.bodies_found, res.bodies_missing,
      res.coverage.package.verdict,
      res.readiness.package.verdict if res.readiness else None)
PY
```

`import_eligible` / `graph_writes_allowed` stay false even when coverage is `covered`.

### M217 GROBID TEI → header/citations ETL (candidate-only)

Hybrid path now persists structured GROBID artifacts next to ODL body:

- `{paper_id}.hybrid.body.md` — OpenDataLoader body (as before)
- `{paper_id}.hybrid.header.json` — title/authors/abstract/idnos from TEI
- `{paper_id}.hybrid.citations.jsonl` — one `biblStruct` row per line from `listBibl`

Pure parse: `research_graph.application.corpus.grobid_tei_parse.parse_grobid_tei`  
Live extract still uses `/api/processFulltextDocument` (Apache-2.0 GROBID 0.9.0-crf).  
**Not** graph-importable; raw TEI is not stored as graph truth.

### M218 scholarly wrapper in readiness handoff

`run_hybrid_readiness_handoff` attaches a **candidate-only** `scholarly_wrapper` section by scanning
precomputed `hybrid.header.json` / `hybrid.citations.jsonl` next to body markdown (no live GROBID):

```json
"scholarly_wrapper": {
  "headers_found": 1,
  "citation_total": 35,
  "complete_wrapper_count": 1,
  "import_eligible": false
}
```

Missing header/cites are reported as zeros — never invented. Still not graph import.

### M219 hybrid batch scholarly metrics + live reparse

`run_hybrid_batch_gate` scans each paper `body/` for M217 artifacts and reports:

- per-row: `header_found`, `citations_found`, `citation_count`, `header_title`
- batch: `headers_found`, `citations_files_found`, `scholarly_complete_count`, `citation_total`
- `scholarly_wrapper` in `batch-summary.json` (candidate-only)

Scholarly metrics are **additive observability** — `gate_pass` still depends on hybrid body success, not cites.

Live reparse (gitignored `runs-live-scholarly/`):

```bash
.venv/bin/python -c "from pathlib import Path; from research_graph.workflows.composition.hybrid_batch_gate import HybridBatchGateRequest, run_hybrid_batch_gate; r=run_hybrid_batch_gate(HybridBatchGateRequest(selection_path=Path('artifacts/m213-hybrid-gate/selection.json'), work_dir=Path('artifacts/m213-hybrid-gate/runs-live-scholarly'), enable_live_hybrid=True, repo_root=Path('.'), min_hybrid_success=10)); print(r.headers_found, r.citation_total, r.gate_pass)"
```

Live selection-10 proof (M219): 10/10 hybrid, 10/10 scholarly complete, 301 citations, import false.
Point handoff `body_root` at `runs-live-scholarly` for non-zero `scholarly_wrapper`.

### M220 selection-20 scholarly + citation candidate inventory

- Live rung: `selection-20.json` → gitignored `runs-live-scholarly-20/` (reuse `hybrid_batch_gate`).
- Live proof: **20/20** hybrid + scholarly complete, **878** citations, import false (~152s).
- Pure inventory: `research_graph.application.corpus.citation_candidate_inventory`
- Composition: `run_citation_candidate_inventory` scans `body_root` for header/cites and emits coverage rates (title/author/idno/date/venue). **Not** a review gate; import false.

```bash
.venv/bin/python -c "from pathlib import Path; from research_graph.workflows.composition.citation_candidate_inventory import CitationInventoryRequest, run_citation_candidate_inventory; r=run_citation_candidate_inventory(CitationInventoryRequest(hybrid_selection_path=Path('artifacts/m213-hybrid-gate/selection-20.json'), body_root=Path('artifacts/m213-hybrid-gate/runs-live-scholarly-20'), repo_root=Path('.'))); print(r.package.citation_total, r.package.to_dict()['title_coverage'], r.import_eligible)"
```

### M221 citation review policy (pre-import)

Pure `evaluate_citation_review_policy` maps M220 inventory coverages to:

- `ready_for_human_review` — hard thresholds met (title/author/file fraction)
- `repair` — coverage below thresholds
- `blocked` — no papers/cites/files

**Always** `import_eligible=false`, `review_required=true`.  
Default: **idno is advisory** (live TEI ~0.40); set `enforce_idno=True` only when DOI policy hardens.

```bash
.venv/bin/python -c "from pathlib import Path; from research_graph.workflows.composition.citation_review_policy import CitationReviewPolicyRequest, run_citation_review_policy; r=run_citation_review_policy(CitationReviewPolicyRequest(hybrid_selection_path=Path('artifacts/m213-hybrid-gate/selection-20.json'), body_root=Path('artifacts/m213-hybrid-gate/runs-live-scholarly-20'), repo_root=Path('.'))); print(r.policy.verdict, r.policy.citation_total, r.import_eligible)"
```

Live scholarly-20 smoke: `ready_for_human_review`, 878 cites, import false.


Current live probe result: 1 target article has `live_success` GROBID TEI summary evidence; 5 linked target articles remain `missing_pdf` blockers until bounded local PDF acquisition is performed. Raw TEI/full text is not persisted.

M049 models registry (canonical MiniMax model paths):

The `models.yaml` file at repo root is the single source of truth for MiniMax model paths and helper bindings. It is validated by `scripts/validate_models_yaml.py` (pre-commit mandatory on `models.yaml` changes).

```bash
# Validate models.yaml
uv run python scripts/validate_models_yaml.py
# Run registry tests
uv run pytest tests/test_models_registry.py -q
```

Each model entry includes `id`, `provider` (anthropic or openai), `endpoint` (https only), `model_name`, `tool_version`, `policy_version` — all required. Bindings map usage purposes (e.g., `article-artifact-classify`) to model ids. Per D074, MiniMax-M3-512k is canonical for Anthropic-compatible path; MiniMax-M3 is canonical for OpenAI-compatible path.

Schema versioning: bump `schema_version` in models.yaml on breaking changes; bump `policy_version` per model entry on calling-side behavior changes; bump `tool_version` on provider API contract changes.

M046 07-2026-assessment Recommendation 6 marked done with M049 evidence.

M050 LLM helper v2 worker pool (Track A) — requester + worker + reducer:

- `request_article_artifact_classification(structure, *, max_candidates, binding_id, run_id)`: emits an `ArticleArtifactWorkRequest` with deterministic `work_id` via M049 `compute_work_id`.
- `article_artifact_worker.run_worker_pool(work_requests, *, structures, transport, max_workers, storage_dir)`: bounded ProcessPoolExecutor (1-2 workers, NOT distributed), pluggable Transport (HttpTransport for real MiniMax, MockTransport for tests).
- `article_artifact_reducer.merge_article_artifact_results(results)`: idempotent dedup by work_id, sorted output (ActiveGraph pattern 3.6).
- `article_artifact_reducer.aggregate_article_artifact_log(results_dir)`: reads content-addressed dir, emits per-binding-id + validation_status counts.
- 5-flag safety block (graph_import_allowed, graphdb_written, ladybugdb_written, production_import_attempted, import_eligible) explicit on every output. All false per ADR-006.

```bash
# Run M050 tests
uv run pytest tests/test_m050_article_artifact_worker.py tests/test_m050_article_artifact_reducer.py tests/test_m050_e2e_pipeline.py -q
```

M051 bounded PDF acquisition (Track B) — 5 linked target records from M041:

- `scripts/acquire_linked_target_pdfs.py`: bounded download from arxiv.org (3 retries, 30s timeout, max-workers=1 sequential, atomic write).
- `scripts/audit_m054_pdf_acquisition.py`: deterministic markdown audit with 5-flag safety block.
- `scripts/update_m043_target_subset_post_m054.py`: idempotent M043 manifest update with per-record `local_pdf_present_post_m054` block.
- 5/5 PDFs acquired (9.0 MB), audit + M043 update both emitted.
- M045 next_gate closed: bounded local PDF acquisition for the 5 linked target records.

```bash
# Run M051 tests
uv run pytest tests/test_acquire_linked_target_pdfs.py -q
```

Pre-commit hooks (mandatory M044 architecture guardrail):

```bash
bash scripts/install-precommit.sh
```

This installs pre-commit into your uv environment and configures the git hook. From then on, every commit that touches `scripts/verify_*.py`, `src/arxiv_archive/universal_kb_*.py`, `src/arxiv_archive/graph_readiness_*.py`, `doc/adr/`, or `.gsd/{DECISIONS,REQUIREMENTS}.md` runs:

- **M044 architecture guardrail** (D079) — mandatory, blocks the commit if drift is detected.
- **M045 trajectory check** (D080) — advisory, prints drift report but does not fail the commit.

A GitHub Action (`.github/workflows/architecture-guardrail.yml`) runs the same checks on push and pull_request to master. M044 fails the merge; M045 surfaces drift as a workflow annotation.

Bypass only in emergency: `git commit --no-verify`.

M048 trajectory severity tuning per phase (QW-3 of M046 roadmap):

```bash
uv run python scripts/check_project_trajectory.py --output-dir artifacts/m046-synthesis/current --phase preflight   # default
uv run python scripts/check_project_trajectory.py --output-dir artifacts/m046-synthesis/current --phase active     # promotes uncommitted to medium
uv run python scripts/check_project_trajectory.py --output-dir artifacts/m046-synthesis/current --phase closeout   # demotes uncommitted to info
```

`PHASE_SEVERITY_OVERRIDES` dict in `scripts/check_project_trajectory.py` makes severity per phase explicit. `report['phase']` field added; render_markdown shows `Phase: \`{phase}\``. 5 new tests cover all phases + unknown + render. M046 07-2026-assessment Recommendation 5 marked done.

M047 architecture guardrail enforcement and reverse ADR audit (QW-2 of M046 roadmap):

- `.pre-commit-config.yaml` enforces M044 architecture guardrail (mandatory) and M045 trajectory check (advisory) on relevant file changes.
- `.github/workflows/architecture-guardrail.yml` runs the same checks on push and pull_request; M044 blocks merge, M045 surfaces drift as workflow annotation.
- `scripts/install-precommit.sh` is the one-step installer.
- `scripts/check_project_trajectory.py` extended with `reverse_adr_audit` dimension (8 rules anchored to ADR-002/005/007/R029).
- `tests/test_m045_project_trajectory.py` covers clear baseline + 2 violation cases.

M046 synthesis package over M033 to M045:

```text
artifacts/m046-synthesis/00-INDEX.md
artifacts/m046-synthesis/01-north-star.md
artifacts/m046-synthesis/02-architecture-layers.md
artifacts/m046-synthesis/03-adr-decisions.md
artifacts/m046-synthesis/04-module-map.md
artifacts/m046-synthesis/05-evidence-safety.md
artifacts/m046-synthesis/06-trajectory-ops.md
artifacts/m046-synthesis/07-2026-assessment.md
```

M046 is a synthesis-only milestone: it produces 8 self-contained artifacts over the M033-M045 evolution. It does not authorize graph import, GraphDB selection, parser adoption, or agentic orchestration. M046 added D081 (ADR-001 acceptance) and a new architecture layer: scientific articles as first domain (ADR-001, Accepted in M046 QW-1).

After M046:

- 8 ADRs all binding or deferred (zero Planned); ADR-001 (Scientific Papers as First Domain) joins the register.
- 2026 best-practices assessment covers 7 categories with 7 actionable recommendations (medium/low priority).
- Reverse ADR audit: 0 violations at code level.
- Next gate (per M045 follow-ups, M051 closed): live GROBID/OpenDataLoader/Adaptix pilot on the 5 acquired PDFs (M052 RLM S09 on Track A, M055 GROBID expansion on Track B).

Recent milestones (post M046 roadmap):
- M050-l8os7p closed: Bounded LLM Helper v2 Worker Pool.
- M051-aaw7j7 closed: Bounded PDF Acquisition for 5 linked target records.
- M052-xifwu6 closed: RLM S09 Document Workflow Harness on M050 worker pool, e2e + audit (Track A closure).
- M053-ool5c4 closed: Live GROBID Pilot on 5 acquired PDFs.
- M054-proc4f closed: Parser Hybrid Benchmark 5 PDF, ADR-008 binding.
- M055-kyxuqm closed: Hybrid Parser Deep-Dive 20 PDF, ADR-009 binding, fulltext uplift.
- M056-lchpnp closed: BFS 1-hop acquisition 166 PDF, 4454 citation edges, ADR-010 binding.
- M057-s70wkm closed: Graph-Readiness Gate v1 via fd Embeddings + Marker re-extraction, 4-layer diagnostic graph (9403 edges), ADR-011 binding.
- M058-cmjp1u closed: M059 Pilot Cycle (plotextractor v2 from TeX source + Marker iterative expansion, 3/5 slices), ADR-012 binding.
- M059-y6osma closed: M060 Manifest-Driven PDF Ingest Architecture (6 JSON schemas + 5 retroactive manifests + jsonschema validation + replay tooling), ADR-013 binding.

Next gate (per M059-y6osma decision):
- M061 2-hop BFS with manifest-first ingest (5 anchors × 2-hop → 2000-5000 PDF).
- M062 fd production hardening (persistent storage, monitoring, multi-worker). Closed M065-vq0do4: unified embedder + retry+circuit+graceful+metrics, env-driven config (10 FD_* env vars), ADR-019 binding + amended, 52 contract tests + gap report. M068 v2-verification: 5 new env vars (FD_API_KEY, MODEL_ID, TEI_URL, REDIS_HOST, REDIS_PORT), 52 contract tests re-run, 150 papers integration test, ADR-019 amended.
- M063 ADR-002 GraphDB selection (FalkorDB vs LadybugDB vs Neo4j). Closed M065-u29n4f: 5 candidates evaluated, LadybugDB chosen (39/45 score), ADR-020 binding, networkx->ladybug migration plan. SUPERSEDED by M066 (Neo4j 76/90, concurrent writes 5/5, GRAFBLAS 4/5, UDF 5/5). SUPERSEDED again by M067 (FalkorDB 70/90 for self-hosted, SSPLv1 license correction, supersedes ADR-021 Neo4j AGPLv3).
- M059b MiniMax-M3 figure QA judge pilot (LLM-as-judge on 30 figures, bounded).

M046 roadmap proposal: Quick wins (M044 pre-commit, trajectory severity tuning, M048 patterns review, M049 models registry) → parallel tracks A (M050 LLM helper worker pool, M052 RLM S09, M053 RLM S10) and B (M051 bounded PDF acquisition, M055 GROBID expansion, M056 GraphDB comparison, M057 hybrid pilot) → convergence at M058 (graph-readiness gate v1) or explicit deferral.

M045 unified trajectory preflight for planning and closeout:

```bash
uv run python scripts/check_project_trajectory.py --output-dir artifacts/m045-project-trajectory/current --codebase-memory-snapshot artifacts/m045-project-trajectory/codebase-memory-snapshot.json
```

M045 trajectory evidence:

```text
artifacts/m045-project-trajectory/current/trajectory-report.json
artifacts/m045-project-trajectory/current/trajectory-report.md
```

The trajectory report is derived, not canonical. It composes `.gsd/`, ADR docs, governance mirrors, recent milestone summaries, git state, and an optional codebase-memory MCP snapshot. Use it before planning and before closeout to check architecture, functionality, module/code movement, evidence, safety, operations, and next gate. codebase-memory is recall/navigation only; canonical facts remain in GSD, ADR docs, and GitNexus evidence.

Next gate: bounded local PDF acquisition for the five linked target records, then rerun live GROBID/OpenDataLoader/Adaptix candidate packets under the trajectory preflight.

Expected safety result:

```text
graph_write_allowed=false
promotion_allowed=false
production_import_attempted=false
import_eligible=false
```

Current continuity metadata is normalized: every selected article has metadata-only `continuity.json`, explicit false safety flags, source evidence status, and loader evidence status. Loader absence is represented as explicit diagnostic metadata for no-write smoke continuity; it does not authorize import. M036-M043 do not authorize GraphDB selection, GraphDB writes, production import, fact promotion, or agentic orchestration. The current baseline proof is a 10-article no-write batch with no continuity blockers. The current connectivity proof is a 20-article mixed no-write batch with reference-linked and Hermes review-section articles, no blockers, and all write/import/promotion flags false. M042 confirms the 5 reference-linked records have fetched identity metadata and linked-from evidence, but graph import remains blocked because connectivity is limited and ADR-005 still applies. M043 confirms the combined sidecar architecture fits as candidate evidence with explicit blockers, not as parser adoption or import readiness.

## Governance memory bridge

M038 uses a hybrid governance-memory workflow:

| Layer | Role |
|---|---|
| GSD `.gsd/REQUIREMENTS.md` and `.gsd/DECISIONS.md` | Canonical requirement and decision lifecycle. |
| ADR docs under `doc/adr/` | Canonical architecture decisions and binding notes. |
| GitNexus | Mandatory code-impact analysis before edits and change-scope checks before commits. |
| codebase-memory MCP | Fast semantic ADR/R/D recall mirror only; never canonical. |

Refresh the codebase-memory governance mirror after changing GSD requirements, GSD decisions, or ADR docs:

```bash
uv run python scripts/sync_codebase_memory_governance.py
uv run python scripts/sync_codebase_memory_governance.py --check
```

The generated mirror lives at `.codebase-memory/adr.md`. M039 also generates `.codebase-memory/governance-graph.json`, a typed governance graph projection with Requirement, Decision, ADR, Milestone, SafetyBoundary, and generated Artifact nodes plus explicit relationship edges. The graph projection is useful for agent navigation and codebase-memory-indexed search/readback, but it is still generated mirror state, not canonical state.

Current codebase-memory MCP `ingest_traces` reports that runtime edge creation is not implemented, so M039 does not claim native custom graph ingestion. Use the JSON projection until codebase-memory exposes a supported typed node/edge ingestion API. If generated governance files conflict with `.gsd/` or `doc/adr/`, treat them as stale and regenerate them. Do not use codebase-memory MCP as the source of truth for `R###`, `D###`, GraphDB authorization, import eligibility, or fact promotion.

## Setup

```bash
uv sync --all-extras
```

Environment variables used by the current first-domain runtime:

- `MINIMAX_API_KEY` — MiniMax API key for summarization and structured helper experiments.
- `TELEGRAM_BOT_TOKEN` — optional Telegram bot token for delivery.
- `TELEGRAM_CHAT_ID` — optional Telegram chat ID for delivery.

Never commit or log secret values.

## Run the current paper pipeline

```bash
# Process papers for a specific date
uv run python -m arxiv_archive --date 2026-05-15

# Or with explicit JSON output
uv run python -m arxiv_archive --date 2026-05-15 --json
```

Output sessions are saved to:

```text
~/.research/ops/sessions/{date}.md
```

## Paper conversion path

Papers are converted to Markdown using:

1. **arxiv2md** — primary fast path, parses ar5iv HTML via REST API.
2. **Marker** — fallback OCR/PDF path for cases where the primary conversion is missing or low quality.
3. **PyMuPDF repair paths** — used in later graph-readiness work when local PDFs are available and Marker is unavailable.

Do not infer conversion success from HTTP 200 or non-empty markdown alone. Real-corpus validation has shown that arxiv2md can return abstract-page navigation markdown without substantive body text.

## Development

MiniMax integration guidance is maintained as the global `minimax-safe-helper` skill in `~/.agents/skills/`. Use it before changing MiniMax helper behavior, structured output, Token Plan, or usage/remains checks.

```bash
# Run tests
uv run pytest tests/ -v

# Lint
uv run ruff check src/ tests/

# Type check
uv run pyrefly check src/
```

## Project structure

```text
src/arxiv_archive/
├── __init__.py
├── __main__.py              # CLI entry point
├── arxiv_client.py          # Record: fetch from arXiv API
├── semantic_scholar.py      # Reduce: enrich with citations
├── keyword_extractor.py     # Reduce: extract keywords with YAKE
├── scoring.py               # Score: rank papers
├── summarizer.py            # Summarize: MiniMax LLM
├── md_converter.py          # Convert: arxiv2md + fallback paths
├── pdf_downloader.py        # Download PDFs to cache
├── article_artifacts.py     # Fail-closed artifact manifest validation
├── chunk_import_contract.py # Import-readiness contract validation
└── minimax_structured.py    # Structured-output helper boundaries
```

## Preferences

Example topic-weight preference file:

```json
{
  "topic_weights": {
    "cs.SI": 1.5,
    "cs.KG": 1.5,
    "cs.IR": 1.3,
    "cs.CL": 1.3,
    "cs.AI": 1.2,
    "cs.LG": 1.0
  }
}
```

## Current implementation direction

The next implementation direction is the M035 durable evidence pipeline prototype:

1. executable Universal KB contracts and `SafetyFlags`;
2. local SQLite durable queue prototype;
3. Adaptix boundary mapping for sidecar JSON;
4. structured review assistance without approval authority;
5. no-write substrate rehearsal and architecture guards.

Until a future explicit graph-promotion milestone supersedes M034, all Universal KB prototype work must remain metadata-only and no-write with respect to production graph import.
