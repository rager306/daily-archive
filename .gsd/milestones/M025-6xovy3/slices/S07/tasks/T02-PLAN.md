---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T02: Replay separated evidence artifacts

Implement or adapt a local evidence replay command that reads the fixed corpus outputs from S06 and writes separate assets, tables, links, and identity artifacts per article. Unsupported evidence types must produce diagnostics rather than silent empty outputs. The command must read the catalog index and corpus selection at runtime and fail clearly if expected S06 chunking artifacts are absent.

## Inputs

- None specified.

## Expected Output

- `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence/`
- `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-events.jsonl`

## Verification

uv run python scripts/verify_m025_evidence_boundaries.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --chunks data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/chunking --evidence data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence --write-events data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-events.jsonl

## Observability Impact

Writes per-article extraction outcomes, evidence counts, unsupported-type diagnostics, provenance pointers, and redaction status.
