# arxiv-daily-archive

Daily arXiv paper archive with keyword extraction and Claude analysis.

## Setup

```bash
uv sync --all-extras
```

## Run

```bash
uv run python -m arxiv_daily_archive
```

## Development

```bash
# Run tests
uv run pytest

# Lint
uv run ruff check src/ tests/
```
