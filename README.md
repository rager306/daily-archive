# daily-archive

`daily-archive` is evolving into a **local-first Universal Knowledge Base** built from durable, traceable evidence chains. Scientific papers and arXiv-style article workflows are the first domain and proving ground, not the final boundary of the system.

The current runnable product is still the research-paper daily archive pipeline. M034 establishes the north-star architecture for moving from that first-domain pipeline toward a Universal KB without allowing parser, sidecar, adapter, or LLM output to become graph truth by accident.

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

M036 proved a 5-article real-corpus no-write smoke over existing article catalog artifacts. M037 consolidates the control surface so routine work uses one module command instead of separate selector, runner, audit, and verifier scripts.

Routine fast smoke:

```bash
uv run python -m arxiv_archive.universal_kb_smoke all --limit 5 --profile fast
```

Full pre-commit proof, including the M035 verifier:

```bash
uv run python -m arxiv_archive.universal_kb_smoke verify --profile full
```

The legacy command remains as a compatibility wrapper:

```bash
python3 scripts/verify_m036_real_corpus_no_write_smoke.py
```

The smoke writes:

```text
artifacts/m036-real-corpus-no-write-smoke/manifest.json
artifacts/m036-real-corpus-no-write-smoke/run/summary.json
artifacts/m036-real-corpus-no-write-smoke/audit.json
artifacts/m036-real-corpus-no-write-smoke/audit.md
```

Expected safety result:

```text
graph_write_allowed=false
promotion_allowed=false
production_import_attempted=false
import_eligible=false
```

Current continuity blockers are diagnostic only and block import/promotion claims: legacy or missing article safety flag shape, plus missing loader evidence for one selected record. M036/M037 do not authorize GraphDB selection, GraphDB writes, production import, fact promotion, agentic orchestration, or expanding beyond 5 articles. The 10-30 article expansion is intentionally deferred until after this control surface is consolidated.

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
