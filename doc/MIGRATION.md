# Migration Notes

## M062 fd embedding wrapper

`arxiv_archive.embedder.Embedder` is now the canonical fd embedding client for daily-archive.

### What changed

- Default endpoint is `http://127.0.0.1:8000/v1/embeddings`.
- Default embedding dimensions are `1024`.
- Request payload uses OpenAI-compatible shape: `{"input": [...], "dimensions": 1024}`.
- The wrapper includes retry/backoff, circuit breaker, graceful degradation to zero embeddings, structured logs, and metrics export.
- `scripts/m057_table_embed.py` moved to `scripts/legacy/m057_table_embed.py` for historical reproduction only.

### New usage

```python
from arxiv_archive.embedder import Embedder

embedder = Embedder()
embeddings = await embedder.embed_batch(["example text"])
metrics = embedder.export_metrics()
await embedder.close()
```

For synchronous one-off scripts, use `embed_batch_sync()` or `embed_all_sync()`.
