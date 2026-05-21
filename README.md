# arxiv-daily-archive

Daily digest of top-10 research papers from arXiv cs.* categories.

Pipeline: **Record → Reduce → Score → Summarize → Deliver**

## Setup

```bash
uv sync --all-extras
```

Requires environment variables:
- `MINIMAX_API_KEY` — MiniMax API key for summarization
- `TELEGRAM_BOT_TOKEN` — Telegram bot token (optional, for delivery)
- `TELEGRAM_CHAT_ID` — Telegram chat ID (optional, for delivery)

## Run

```bash
# Process papers for a specific date
uv run python -m arxiv_archive --date 2026-05-15

# Or with explicit options
MINIMAX_API_KEY=your-key uv run python -m arxiv_archive --date 2026-05-15 --json
```

## Architecture

```
Record    → arXiv API (feedparser) — fetch papers for date/categories
Reduce    → Semantic Scholar (citations) + YAKE (keywords)
Score     → Weighted scoring: citations, recency, novelty, preference
Summarize → MiniMax LLM: HEADLINE / WHAT_IT_DOES / WHY_IT_MATTERS / ANALOGY
Deliver   → Telegram channel + local session log
```

### Paper Conversion

Papers are converted to Markdown using:

1. **arxiv2md** (primary, <1 sec) — parses ar5iv HTML via REST API
2. **Marker** (fallback, 10 min timeout) — PDF OCR, only for pre-2020 papers

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

## Project Structure

```
src/arxiv_archive/
├── __init__.py
├── __main__.py      # CLI entry point
├── arxiv_client.py  # Record: fetch from arXiv API
├── semantic_scholar.py  # Reduce: enrich with citations
├── keyword_extractor.py # Reduce: extract keywords (YAKE)
├── scoring.py       # Score: rank papers
├── summarizer.py    # Summarize: MiniMax LLM
├── md_converter.py  # Convert: arxiv2md + Marker fallback
├── pdf_downloader.py # Download PDFs to cache
└── telegram_sender.py # Deliver: send to Telegram
```

## Research Directory

Pipeline reads preferences from `~/.research/self/preferences.json`:

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

Output sessions saved to `~/.research/ops/sessions/{date}.md`.
