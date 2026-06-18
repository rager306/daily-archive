from __future__ import annotations

import httpx
import pytest

from research_graph.retrieval.embedder import Embedder

FD_EMBEDDINGS_ENDPOINT = "http://127.0.0.1:8000/v1/embeddings"
FD_BAD_ENDPOINT = "http://127.0.0.1:8000/v1/not-found-for-m062-s01"


@pytest.mark.asyncio
async def test_live_fd_embed_1_input() -> None:
    embedder = Embedder(endpoint=FD_EMBEDDINGS_ENDPOINT, timeout_seconds=120.0, retry_sleep=False)
    try:
        embeddings = await embedder.embed_batch(["M062 S01 live fd smoke input"])
    finally:
        await embedder.close()

    assert len(embeddings) == 1
    assert len(embeddings[0]) == 1024
    assert all(isinstance(value, float) for value in embeddings[0])
    assert embedder.was_degraded() is False


@pytest.mark.asyncio
async def test_live_fd_batch_10_inputs() -> None:
    embedder = Embedder(endpoint=FD_EMBEDDINGS_ENDPOINT, timeout_seconds=120.0, retry_sleep=False)
    try:
        embeddings = await embedder.embed_batch([f"M062 S01 batch input {index}" for index in range(10)])
    finally:
        await embedder.close()

    assert len(embeddings) == 10
    assert all(len(embedding) == 1024 for embedding in embeddings)
    assert all(isinstance(value, float) for embedding in embeddings for value in embedding)
    assert embedder.was_degraded() is False


@pytest.mark.asyncio
async def test_live_fd_graceful_degradation_on_404() -> None:
    embedder = Embedder(
        endpoint=FD_BAD_ENDPOINT,
        timeout_seconds=10.0,
        max_attempts=1,
        retry_sleep=False,
    )
    try:
        for _ in range(3):
            with pytest.raises(httpx.HTTPStatusError):
                await embedder.embed_batch(["force circuit failure"])
        degraded = await embedder.embed_batch(["degrade after open circuit"])
    finally:
        await embedder.close()

    assert degraded == [[0.0] * 1024]
    assert embedder.was_degraded() is True
    assert embedder.circuit_state == Embedder.CIRCUIT_OPEN


@pytest.mark.asyncio
async def test_live_fd_metrics_after_5_calls() -> None:
    embedder = Embedder(endpoint=FD_EMBEDDINGS_ENDPOINT, timeout_seconds=120.0, retry_sleep=False)
    try:
        for index in range(5):
            embeddings = await embedder.embed_batch([f"M062 S01 metrics input {index}"])
            assert len(embeddings) == 1
        metrics = embedder.export_metrics()
    finally:
        await embedder.close()

    assert 'request_count{status="success"} 5' in metrics
    assert 'request_count{status="error"} 0' in metrics
    assert "request_duration_seconds_count 5" in metrics
    assert "circuit_state 0" in metrics
