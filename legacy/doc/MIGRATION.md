# Migration Notes

## M062 fd embedding wrapper (updated M105)

`research_graph.infrastructure.retrieval.embedder.Embedder` is the canonical fd embedding client for daily-archive (M105 S05: moved from `arxiv_archive.embedder` → `research_graph.infrastructure.retrieval.embedder`).

### What changed

- Default endpoint is `http://127.0.0.1:8000/v1/embeddings`.
- Default embedding dimensions are `1024`.
- Request payload uses OpenAI-compatible shape: `{"input": [...], "dimensions": 1024}`.
- The wrapper includes retry/backoff, circuit breaker, graceful degradation to zero embeddings, structured logs, and metrics export.
- `scripts/m057_table_embed.py` moved to `scripts/legacy/m057_table_embed.py` for historical reproduction only.

### New usage

```python
from research_graph.infrastructure.retrieval.embedder import Embedder

embedder = Embedder()
embeddings = await embedder.embed_batch(["example text"])
metrics = embedder.export_metrics()
await embedder.close()
```

For synchronous one-off scripts, use `embed_batch_sync()` or `embed_all_sync()`.

## M105 migration summary (2026-06-22)

All 10 infrastructure packages moved into `research_graph/infrastructure/` over 4 waves:

- **W1**: llm, identity, quality (leaves)
- **W2**: ops, staging (depend on W1)
- **W3**: corpus, papers, repair (cycle, coordinated)
- **W4**: graph, retrieval (depend on cycle), graph merged with ladybug_adapter

`evaluation/` dissolved into onion layers (see `.gsd/milestones/M105-269bqo/M105-269bqo-SUMMARY.md`).

Update imports: `from arxiv_archive.X import Y` → `from research_graph.infrastructure.X import Y`.
