from __future__ import annotations

from pathlib import Path
from typing import Any

import httpx
import pytest

import arxiv_archive.embedder as embedder_module
from arxiv_archive.embedder import DEFAULT_EMBEDDING_ENDPOINT, SAFETY_DEFAULTS, Embedder


async def async_client_factory(client: "FakeAsyncClient") -> "FakeAsyncClient":
    return client


class FakeAsyncClient:
    def __init__(self, responses: list[httpx.Response]) -> None:
        self.responses = responses
        self.calls: list[dict[str, Any]] = []

    async def post(self, url: str, json: dict[str, Any]) -> httpx.Response:
        self.calls.append({"url": url, "json": json})
        if len(self.calls) <= len(self.responses):
            return self.responses[len(self.calls) - 1]
        return self.responses[-1]

    async def aclose(self) -> None:
        return None


def response(status_code: int, payload: dict[str, Any] | None = None, **headers: str) -> httpx.Response:
    return httpx.Response(
        status_code,
        json=payload if payload is not None else {"error": "forced"},
        headers=headers,
        request=httpx.Request("POST", DEFAULT_EMBEDDING_ENDPOINT),
    )


def success_payload(count: int, dimensions: int = 1024) -> dict[str, Any]:
    return {
        "object": "list",
        "data": [
            {
                "object": "embedding",
                "index": index,
                "embedding": [float(index)] * dimensions,
            }
            for index in range(count)
        ],
    }


@pytest.mark.asyncio
async def test_embedder_default_endpoint_is_127_8000() -> None:
    embedder = Embedder()

    assert embedder.endpoint == "http://127.0.0.1:8000/v1/embeddings"


@pytest.mark.asyncio
async def test_embedder_uses_openai_shape(monkeypatch: pytest.MonkeyPatch) -> None:
    fake_client = FakeAsyncClient([response(200, success_payload(2))])
    embedder = Embedder(retry_sleep=False)
    monkeypatch.setattr(embedder, "_get_client", lambda: async_client_factory(fake_client))

    await embedder.embed_batch(["alpha", "beta"])

    payload = fake_client.calls[0]["json"]
    assert payload == {"input": ["alpha", "beta"], "dimensions": 1024}
    assert "inputs" not in payload
    assert "truncate" not in payload


def test_embedder_dimensions_default_1024() -> None:
    assert Embedder().dimensions == 1024


@pytest.mark.asyncio
async def test_retry_on_5xx(monkeypatch: pytest.MonkeyPatch) -> None:
    fake_client = FakeAsyncClient(
        [
            response(503, **{"Retry-After": "0"}),
            response(503),
            response(503),
        ]
    )
    embedder = Embedder(retry_sleep=False)
    monkeypatch.setattr(embedder, "_get_client", lambda: async_client_factory(fake_client))

    with pytest.raises(httpx.HTTPStatusError):
        await embedder.embed_batch(["retry me"])

    assert len(fake_client.calls) == 3
    assert embedder.metrics.error_count["503"] == 3


@pytest.mark.asyncio
async def test_circuit_breaker_opens_after_3_fails(monkeypatch: pytest.MonkeyPatch) -> None:
    fake_client = FakeAsyncClient([response(500), response(500), response(500)])
    embedder = Embedder(max_attempts=1, retry_sleep=False)
    monkeypatch.setattr(embedder, "_get_client", lambda: async_client_factory(fake_client))

    for _ in range(3):
        with pytest.raises(httpx.HTTPStatusError):
            await embedder.embed_batch(["fail"])

    assert embedder.circuit_state == Embedder.CIRCUIT_OPEN
    degraded = await embedder.embed_batch(["degrade"])
    assert degraded == [[0.0] * 1024]
    assert embedder.was_degraded() is True


@pytest.mark.asyncio
async def test_circuit_breaker_half_open_after_60s(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    fake_client = FakeAsyncClient(
        [response(500), response(500), response(500), response(200, success_payload(1))]
    )
    now = 100.0
    monkeypatch.setattr(embedder_module.time, "monotonic", lambda: now)
    embedder = Embedder(max_attempts=1, retry_sleep=False, circuit_open_seconds=60.0)
    monkeypatch.setattr(embedder, "_get_client", lambda: async_client_factory(fake_client))

    for _ in range(3):
        with pytest.raises(httpx.HTTPStatusError):
            await embedder.embed_batch(["fail"])
    assert embedder.circuit_state == Embedder.CIRCUIT_OPEN

    now = 159.9
    assert await embedder.embed_batch(["still open"]) == [[0.0] * 1024]
    assert embedder.circuit_state == Embedder.CIRCUIT_OPEN

    now = 160.1
    recovered = await embedder.embed_batch(["probe"])
    assert recovered == [[0.0] * 1024]
    assert embedder.circuit_state == Embedder.CIRCUIT_CLOSED
    assert len(fake_client.calls) == 4


@pytest.mark.asyncio
async def test_graceful_degradation_returns_zero_embedding(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    fake_client = FakeAsyncClient([response(500), response(500), response(500)])
    embedder = Embedder(max_attempts=1, retry_sleep=False)
    monkeypatch.setattr(embedder, "_get_client", lambda: async_client_factory(fake_client))

    for _ in range(3):
        with pytest.raises(httpx.HTTPStatusError):
            await embedder.embed_batch(["open circuit"])

    degraded = await embedder.embed_batch(["a", "b"])
    assert degraded == [[0.0] * 1024, [0.0] * 1024]
    assert embedder.was_degraded() is True
    assert embedder.metrics.request_count["degraded"] == 1


@pytest.mark.asyncio
async def test_metrics_export(monkeypatch: pytest.MonkeyPatch) -> None:
    fake_client = FakeAsyncClient([response(200, success_payload(1), **{"X-Cache": "HIT"})])
    embedder = Embedder(retry_sleep=False)
    monkeypatch.setattr(embedder, "_get_client", lambda: async_client_factory(fake_client))

    await embedder.embed_batch(["metrics"])
    metrics = embedder.export_metrics()

    assert 'request_count{status="success"} 1' in metrics
    assert 'request_count{status="error"} 0' in metrics
    assert 'request_duration_seconds_bucket{le="+Inf"} 1' in metrics
    assert "request_duration_seconds_count 1" in metrics
    assert "cache_hit_rate 1.000000000" in metrics
    assert "circuit_state 0" in metrics


def test_5_safety_defaults_explicit() -> None:
    assert SAFETY_DEFAULTS == {
        "external_network_authorized": False,
        "fact_promotion_authorized": False,
        "graph_writes_authorized": False,
        "llm_calls_authorized": False,
        "production_import_authorized": False,
    }
    assert Embedder().safety_defaults == SAFETY_DEFAULTS


def test_127_not_loopback_hostname() -> None:
    forbidden = "local" + "host"
    source = Path("src/arxiv_archive/embedder.py").read_text(encoding="utf-8")

    assert "127.0.0.1" in DEFAULT_EMBEDDING_ENDPOINT
    assert forbidden not in DEFAULT_EMBEDDING_ENDPOINT
    assert forbidden not in source
