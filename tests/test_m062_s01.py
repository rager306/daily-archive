from __future__ import annotations

import inspect
import json
from collections.abc import Awaitable, Callable

import httpx
import pytest

from arxiv_archive import embedder as embedder_module
from arxiv_archive.embedder import (
    CIRCUIT_CLOSED,
    CIRCUIT_HALF_OPEN,
    CIRCUIT_OPEN,
    DEFAULT_DIMENSIONS,
    DEFAULT_ENDPOINT,
    Embedder,
)


def _openai_embedding_response(request: httpx.Request, *, dimensions: int = DEFAULT_DIMENSIONS) -> httpx.Response:
    payload = json.loads(request.content.decode("utf-8"))
    data = [
        {
            "object": "embedding",
            "embedding": [float(index)] * dimensions,
            "index": index,
            "dimensions": dimensions,
        }
        for index, _text in enumerate(payload["input"])
    ]
    return httpx.Response(
        200,
        json={
            "object": "list",
            "data": data,
            "model": "deepvk/USER-bge-m3",
            "usage": {"prompt_tokens": len(payload["input"]), "total_tokens": len(payload["input"])},
        },
        headers={"X-Cache": "HIT"},
        request=request,
    )


def _client(handler: Callable[[httpx.Request], httpx.Response]) -> httpx.AsyncClient:
    return httpx.AsyncClient(transport=httpx.MockTransport(handler))


def _no_sleep_factory(delays: list[float]) -> Callable[[float], Awaitable[None]]:
    async def _sleep(delay: float) -> None:
        delays.append(delay)

    return _sleep


@pytest.mark.asyncio
async def test_embedder_default_endpoint_is_127_8000() -> None:
    embedder = Embedder(client=_client(_openai_embedding_response))

    assert embedder.endpoint == "http://127.0.0.1:8000/v1/embeddings"
    assert DEFAULT_ENDPOINT == embedder.endpoint

    await embedder.close()


@pytest.mark.asyncio
async def test_embedder_uses_openai_shape() -> None:
    observed_payloads: list[dict] = []

    def handler(request: httpx.Request) -> httpx.Response:
        observed_payloads.append(json.loads(request.content.decode("utf-8")))
        return _openai_embedding_response(request)

    embedder = Embedder(client=_client(handler))

    await embedder.embed_batch(["alpha", "beta"])

    assert observed_payloads == [{"input": ["alpha", "beta"], "dimensions": DEFAULT_DIMENSIONS}]
    assert "inputs" not in observed_payloads[0]
    assert "truncate" not in observed_payloads[0]

    await embedder.close()


def test_embedder_dimensions_default_1024() -> None:
    embedder = Embedder()

    assert embedder.dimensions == 1024
    assert DEFAULT_DIMENSIONS == 1024


@pytest.mark.asyncio
async def test_retry_on_5xx() -> None:
    calls = 0
    delays: list[float] = []

    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal calls
        calls += 1
        if calls < 3:
            return httpx.Response(503, json={"error": "overload"}, headers={"Retry-After": "0"}, request=request)
        return _openai_embedding_response(request)

    embedder = Embedder(client=_client(handler), sleep=_no_sleep_factory(delays))

    embeddings = await embedder.embed_batch(["retry me"])

    assert calls == 3
    assert delays == [0.0, 0.0]
    assert embeddings == [[0.0] * DEFAULT_DIMENSIONS]
    assert embedder.export_metrics()["error_count"] == 2
    assert embedder.circuit_state == CIRCUIT_CLOSED

    await embedder.close()


@pytest.mark.asyncio
async def test_circuit_breaker_opens_after_3_fails() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(503, json={"error": "down"}, request=request)

    embedder = Embedder(client=_client(handler), max_attempts=1, sleep=_no_sleep_factory([]))

    with pytest.raises(httpx.HTTPStatusError):
        await embedder.embed_batch(["first"])
    with pytest.raises(httpx.HTTPStatusError):
        await embedder.embed_batch(["second"])

    degraded = await embedder.embed_batch(["third"])

    assert embedder.circuit_state == CIRCUIT_OPEN
    assert degraded == [[0.0] * DEFAULT_DIMENSIONS]
    assert embedder.export_metrics()["circuit_state_gauge"] == 2

    await embedder.close()


@pytest.mark.asyncio
async def test_circuit_breaker_half_open_after_60s() -> None:
    now = 100.0
    calls = 0

    def time_fn() -> float:
        return now

    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal calls
        calls += 1
        if calls == 1:
            return httpx.Response(503, json={"error": "down"}, request=request)
        return _openai_embedding_response(request)

    embedder = Embedder(
        client=_client(handler),
        max_attempts=1,
        circuit_failure_threshold=1,
        circuit_open_seconds=0.01,
        sleep=_no_sleep_factory([]),
        time_fn=time_fn,
    )

    degraded = await embedder.embed_batch(["open circuit"])
    assert degraded == [[0.0] * DEFAULT_DIMENSIONS]
    assert embedder.circuit_state == CIRCUIT_OPEN

    now += 0.02
    assert embedder.circuit_state == CIRCUIT_HALF_OPEN

    recovered = await embedder.embed_batch(["probe"])

    assert recovered == [[0.0] * DEFAULT_DIMENSIONS]
    assert embedder.circuit_state == CIRCUIT_CLOSED
    assert calls == 2

    await embedder.close()


@pytest.mark.asyncio
async def test_graceful_degradation_returns_zero_embedding() -> None:
    calls = 0

    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal calls
        calls += 1
        return httpx.Response(503, json={"error": "down"}, request=request)

    embedder = Embedder(
        client=_client(handler),
        max_attempts=1,
        circuit_failure_threshold=1,
        sleep=_no_sleep_factory([]),
    )

    first = await embedder.embed_batch(["opens"])
    second = await embedder.embed_batch(["skips http"])

    assert first == [[0.0] * DEFAULT_DIMENSIONS]
    assert second == [[0.0] * DEFAULT_DIMENSIONS]
    assert calls == 1
    assert embedder.circuit_state == CIRCUIT_OPEN

    await embedder.close()


@pytest.mark.asyncio
async def test_metrics_export() -> None:
    embedder = Embedder(client=_client(_openai_embedding_response))

    await embedder.embed_batch(["metrics"])
    metrics = embedder.export_metrics()

    assert metrics["request_count"] == 1
    assert metrics["error_count"] == 0
    assert metrics["cache_hit_rate"] == 1.0
    assert metrics["circuit_state"] == CIRCUIT_CLOSED
    assert metrics["circuit_state_gauge"] == 0
    assert set(metrics["latency"]) == {"count", "p50", "p95", "p99"}
    assert metrics["latency"]["count"] == 1

    await embedder.close()


def test_5_safety_defaults_explicit() -> None:
    assert embedder_module.SAFETY_DEFAULTS == {
        "graph_writes_authorized": False,
        "production_import_authorized": False,
        "fact_promotion_authorized": False,
        "external_network_authorized": False,
        "llm_calls_authorized": False,
    }
    assert len(embedder_module.SAFETY_DEFAULTS) == 5
    assert all(value is False for value in embedder_module.SAFETY_DEFAULTS.values())


def test_127_not_localhost() -> None:
    source = inspect.getsource(embedder_module)

    assert "127.0.0.1" in DEFAULT_ENDPOINT
    assert "localhost" not in DEFAULT_ENDPOINT
    assert "localhost" not in source
