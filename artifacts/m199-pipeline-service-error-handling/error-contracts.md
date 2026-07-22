# M199 Pipeline Service Error Contracts (operator reference)

Single place for fail-closed I/O error handling after M199-4rex3i S01–S03.
All diagnostics are **redacted** (no raw HTTP bodies, no secrets).

## Summary matrix

| Service | Module | Transient retry | Fail-closed surface | Codes / markers |
|---------|--------|-----------------|---------------------|-----------------|
| ArxivClient | `infrastructure/corpus/sources/arxiv_client.py` | yes (429/5xx/timeout/connect) | raises `ArxivFetchError` | `ARXIV_TIMEOUT`, `ARXIV_CONNECT`, `ARXIV_429`, `ARXIV_5XX`, `ARXIV_4XX`, `ARXIV_PARSE` |
| TEI / fd Embedder | `infrastructure/retrieval/embedder.py` | yes (existing circuit + backoff) | preflight `FdAuthError`; zeros marked `last_degraded`; CLI raises `FdDegradedEmbeddingsError` | `FD_AUTH_MISSING`, `FD_AUTH_INVALID`, `FD_DEGRADED_ZERO_VECTORS` |
| Markdown Converter | `infrastructure/corpus/sources/markdown_converter.py` | yes (arxiv2md only) | `ConversionResult.error` then optional Marker | exhaustion message includes retry count |
| ScoringEngine | `infrastructure/evaluation/scoring.py` | n/a (pure) | recency uses explicit `run_date` | same-day contract |
| Semantic Scholar | `infrastructure/corpus/sources/semantic_scholar.py` | not wired in CLI | weight `citations=0.0` | `SEMANTIC_SCHOLAR_INTEGRATION=disabled_not_wired_in_cli` |

## 1. ArxivClient (S01)

- **Retry:** `ARXIV_MAX_RETRY_ATTEMPTS=3`, `ARXIV_BACKOFF_SECONDS=(1,5,15,60,300)`, honors `Retry-After` on 429.
- **Error type:** `ArxivFetchError(RuntimeError)` with fields `code`, `service="arxiv_api"`, `message`, `retry_count`, `outcome`, `category`.
- **Diagnostic:** `error.diagnostic` → e.g. `arxiv_api:ARXIV_5XX exhausted after 3 retries category=cs.AI: HTTP 503`.
- **CLI:** `run_analysis_async` / `run_command_async` write `state.json` with typed diagnostic on fetch failure.
- **Non-transient:** HTTP 4xx (except 429) fail-fast, `retry_count=0`.

## 2. TEI Embedder / fd (S02)

- **Preflight (D113):** `validate_fd_api_key` / `Embedder.preflight_auth()` — missing/empty/short/placeholder key → `FdAuthError` **before** HTTP.
- **Zero-vector mark:** `_zero_embeddings` and all-zero HTTP responses set `Embedder.last_degraded: DegradedEmbeddingSignal`.
- **Predicates:** `is_zero_vector`, `is_zero_embedding_batch` for defense-in-depth.
- **CLI fail-closed:** after `embed_all`, if `last_degraded` or all-zero batch → `FdDegradedEmbeddingsError`, `state.json` stage=`embed`; zeros **not** assigned to `ScoredPaper`.
- **Happy path:** return type remains `list[list[float]]` (no breaking API change).

## 3. Markdown Converter (S03)

- **Retry (arxiv2md only):** `ARXIV2MD_MAX_RETRY_ATTEMPTS=3`, `ARXIV2MD_BACKOFF_SECONDS=(1,5,15)` for timeout / connect / HTTPError / 429 / 5xx.
- **404:** fail-fast, no retry.
- **Exhaustion:** `ConversionResult(markdown=None, method="arxiv2md", error="... after N retries")`; `convert()` may still attempt Marker fallback.
- **Marker / Docling:** unchanged timeouts; out of M199 retry scope for quality.

## 4. ScoringEngine recency (S03)

- **Contract:** `score(..., run_date=)` compares `paper.published` to `run_date` (not silent wall-clock-only retrospective).
- **CLI:** `_process_paper_async` / `_score_papers_bounded` require `run_date` from analysis day.
- **Buckets:** same day=10, 1d=8, ≤3d=5, ≤7d=2, older=0.5 (relative to `as_of`).

## 5. Semantic Scholar (S03 decision)

- **Status:** `SEMANTIC_SCHOLAR_INTEGRATION = "disabled_not_wired_in_cli"`.
- **CLI:** always scores with `semschol=None` (no live fetch).
- **Weights:** `DEFAULT_WEIGHTS["citations"] = 0.0`; remaining mass on recency/novelty/preference/graph_bridge (sum=1.0).
- **Client module:** remains for future wiring; do not re-enable weight without CLI fetch + retry.

## 6. Operator verification

```bash
uv run pytest tests/test_m199_error_contracts.py tests/test_arxiv_client.py \
  tests/test_embedder.py tests/test_md_converter_isolated.py \
  tests/test_scoring.py tests/test_analysis.py -q
```

Live TEI healthy path was verified separately (fd_api :8000, non-zero 1024-d vectors).
Negative live key smoke was **not** required for M199 closeout.
