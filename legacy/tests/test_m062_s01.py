from __future__ import annotations

import importlib
import inspect
import json
import os
import tempfile
from collections.abc import Awaitable, Callable
from contextlib import contextmanager
from pathlib import Path
from typing import Any

import httpx
import pytest

from research_graph.infrastructure.retrieval import embedder as embedder_module
from research_graph.infrastructure.retrieval.embedder import (
    CIRCUIT_CLOSED,
    CIRCUIT_HALF_OPEN,
    CIRCUIT_OPEN,
    DEFAULT_DIMENSIONS,
    DEFAULT_ENDPOINT,
    Embedder,
)

FD_ENV_KEYS = (
    "FD_EMBEDDINGS_ENDPOINT",
    "TEI_URL",
    "FD_API_KEY",
    "MODEL_ID",
    "REDIS_HOST",
    "REDIS_PORT",
    "FD_MODEL_NAME",
    "FD_EMBEDDINGS_ENDPOINT_BASE",
    "FD_DIMENSIONS",
    "FD_BATCH_SIZE",
    "FD_REQUEST_TIMEOUT_SECONDS",
    "FD_MAX_RETRIES",
    "FD_RETRY_BACKOFF_SECONDS",
    "FD_CIRCUIT_FAILURE_THRESHOLD",
    "FD_CIRCUIT_OPEN_SECONDS",
    "FD_GRACEFUL_DEGRADATION_ENABLED",
)


@contextmanager
def _embedder_env(dotenv_dir: Path | None = None, **overrides: str):
    original = {key: os.environ.get(key) for key in FD_ENV_KEYS}
    original_cwd = os.getcwd()
    temp_dir = None
    try:
        for key in FD_ENV_KEYS:
            os.environ.pop(key, None)
        os.environ.update(overrides)
        if dotenv_dir is None:
            temp_dir = tempfile.TemporaryDirectory()
            os.chdir(temp_dir.name)
        else:
            os.chdir(dotenv_dir)
        yield importlib.reload(embedder_module)
    finally:
        os.chdir(original_cwd)
        if temp_dir is not None:
            temp_dir.cleanup()
        for key, value in original.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
        importlib.reload(embedder_module)


def _openai_embedding_response(
    request: httpx.Request, *, dimensions: int = DEFAULT_DIMENSIONS
) -> httpx.Response:
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
            "usage": {
                "prompt_tokens": len(payload["input"]),
                "total_tokens": len(payload["input"]),
            },
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

    assert observed_payloads == [
        {
            "input": ["alpha", "beta"],
            "model": "deepvk/USER-bge-m3",
            "dimensions": DEFAULT_DIMENSIONS,
        }
    ]
    assert "inputs" not in observed_payloads[0]
    assert "truncate" not in observed_payloads[0]

    await embedder.close()


def test_embedder_dimensions_default_1024() -> None:
    embedder = Embedder()

    assert embedder.dimensions == 1024
    assert DEFAULT_DIMENSIONS == 1024


def test_env_override_endpoint() -> None:
    endpoint = "http://127.0.0.1:9000/v1/embeddings"

    with _embedder_env(FD_EMBEDDINGS_ENDPOINT=endpoint) as module:
        assert module.DEFAULT_ENDPOINT == endpoint
        assert module.Embedder().endpoint == endpoint


async def test_fd_api_key_in_authorization_header() -> None:
    observed_headers: dict[str, str | None] = {}

    def handler(request: httpx.Request) -> httpx.Response:
        observed_headers["authorization"] = request.headers.get("Authorization")
        return _openai_embedding_response(request)

    with _embedder_env(FD_API_KEY="test-key-12345") as module:
        async with _client(handler) as client:
            await module.Embedder(client=client).embed_batch(["hello"])

    assert observed_headers["authorization"] == "Bearer test-key-12345"


async def test_model_id_in_x_model_id_header() -> None:
    observed: dict[str, Any] = {}

    def handler(request: httpx.Request) -> httpx.Response:
        observed["x_model_id"] = request.headers.get("X-Model-Id")
        observed["payload"] = json.loads(request.content.decode("utf-8"))
        return _openai_embedding_response(request)

    with _embedder_env(MODEL_ID="deepvk/test-bge-m3") as module:
        async with _client(handler) as client:
            await module.Embedder(client=client).embed_batch(["hello"])

    assert observed["x_model_id"] == "deepvk/test-bge-m3"
    assert observed["payload"]["model"] == "deepvk/test-bge-m3"


def test_tei_url_override() -> None:
    with _embedder_env(TEI_URL="http://127.0.0.1:19000") as module:
        assert module.DEFAULT_TEI_URL == "http://127.0.0.1:19000"
        assert module.DEFAULT_ENDPOINT == "http://127.0.0.1:19000/v1/embeddings"
        assert module.Embedder().endpoint == "http://127.0.0.1:19000/v1/embeddings"


def test_redis_host_env() -> None:
    with _embedder_env(REDIS_HOST="redis.internal", REDIS_PORT="6380") as module:
        assert module.DEFAULT_REDIS_HOST == "redis.internal"
        assert module.DEFAULT_REDIS_PORT == 6380


def test_backward_compat_fd_embeddings_endpoint() -> None:
    endpoint = "http://127.0.0.1:18000/custom/embeddings"

    with _embedder_env(FD_EMBEDDINGS_ENDPOINT=endpoint, TEI_URL="http://127.0.0.1:19000") as module:
        assert module.DEFAULT_ENDPOINT == endpoint
        assert module.Embedder().endpoint == endpoint


def test_env_override_dimensions() -> None:
    with _embedder_env(FD_DIMENSIONS="512") as module:
        assert module.DEFAULT_DIMENSIONS == 512
        assert module.Embedder().dimensions == 512


def test_dotenv_fallback_does_not_mutate_process_env(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    (tmp_path / ".env").write_text(
        "FD_EMBEDDINGS_ENDPOINT=http://127.0.0.1:19000/v1/embeddings\n"
        "FD_API_KEY=dotenv-key\n"
        "FD_DIMENSIONS=512\n"
    )

    with _embedder_env(dotenv_dir=tmp_path) as module:
        assert module.DEFAULT_ENDPOINT == "http://127.0.0.1:19000/v1/embeddings"
        assert module.DEFAULT_API_KEY == "dotenv-key"
        assert module.DEFAULT_DIMENSIONS == 512
        assert "FD_EMBEDDINGS_ENDPOINT" not in os.environ
        assert "FD_API_KEY" not in os.environ
        assert "FD_DIMENSIONS" not in os.environ


def test_process_env_overrides_dotenv(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    (tmp_path / ".env").write_text(
        "FD_EMBEDDINGS_ENDPOINT=http://127.0.0.1:19000/v1/embeddings\nFD_DIMENSIONS=512\n"
    )

    endpoint = "http://127.0.0.1:20000/v1/embeddings"
    with _embedder_env(
        dotenv_dir=tmp_path, FD_EMBEDDINGS_ENDPOINT=endpoint, FD_DIMENSIONS="768"
    ) as module:
        assert module.DEFAULT_ENDPOINT == endpoint
        assert module.DEFAULT_DIMENSIONS == 768
        assert module.Embedder().endpoint == endpoint
        assert module.Embedder().dimensions == 768


def test_public_env_config_can_apply_dotenv_explicitly(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    (tmp_path / ".env").write_text("FD_API_KEY=dotenv-key\nFD_DIMENSIONS=512\n")

    with _embedder_env(dotenv_dir=tmp_path) as module:
        config = module.load_embedder_env_config()
        assert config.get("FD_API_KEY") == "dotenv-key"
        assert "FD_API_KEY" not in os.environ

        config.apply_to_environ()

        assert os.environ["FD_API_KEY"] == "dotenv-key"
        assert os.environ["FD_DIMENSIONS"] == "512"


def test_live_code_uses_public_embedder_env_config() -> None:
    embedder_source = Path("src/research_graph/infrastructure/retrieval/embedder.py").read_text()
    m103_source = Path("scripts/m103_extraction_prototype.py").read_text()
    live_sources = list(Path("src").rglob("*.py")) + list(Path("scripts").glob("*.py"))

    offenders = [
        str(path)
        for path in live_sources
        if "_load_dotenv_if_present" in path.read_text()
        or "from research_graph.infrastructure.retrieval.embedder import _" in path.read_text()
    ]

    assert offenders == []
    assert "_load_dotenv_if_present" not in embedder_source
    assert "load_embedder_env_config" in m103_source


def test_env_default_values() -> None:
    with _embedder_env() as module:
        assert module.DEFAULT_TEI_URL == "http://127.0.0.1:8000"
        assert module.DEFAULT_ENDPOINT == "http://127.0.0.1:8000/v1/embeddings"
        assert module.DEFAULT_API_KEY is None
        assert module.DEFAULT_MODEL_ID == "deepvk/USER-bge-m3"
        assert module.DEFAULT_MODEL_NAME == "deepvk/USER-bge-m3"
        assert module.DEFAULT_REDIS_HOST == "127.0.0.1"
        assert module.DEFAULT_REDIS_PORT == 6379
        assert module.DEFAULT_DIMENSIONS == 1024
        assert module.DEFAULT_BATCH_SIZE == 32
        assert module.DEFAULT_TIMEOUT_SECONDS == 120.0
        assert module.DEFAULT_MAX_ATTEMPTS == 3
        assert module.DEFAULT_RETRY_SCHEDULE_SECONDS == (1.0, 5.0, 15.0, 60.0, 300.0)
        assert module.DEFAULT_CIRCUIT_FAILURE_THRESHOLD == 3
        assert module.DEFAULT_CIRCUIT_OPEN_SECONDS == 60.0
        assert module.DEFAULT_GRACEFUL_DEGRADATION_ENABLED is True


def test_env_invalid_value_falls_back() -> None:
    with _embedder_env(FD_DIMENSIONS="invalid") as module:
        assert module.DEFAULT_DIMENSIONS == 1024
        assert module.Embedder().dimensions == 1024


@pytest.mark.asyncio
async def test_retry_on_5xx() -> None:
    calls = 0
    delays: list[float] = []

    def handler(request: httpx.Request) -> httpx.Response:
        nonlocal calls
        calls += 1
        if calls < 3:
            return httpx.Response(
                503, json={"error": "overload"}, headers={"Retry-After": "0"}, request=request
            )
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


@pytest.mark.asyncio
async def test_m050_m062_s01_s02_regression_openai_request_and_safety_defaults() -> None:
    observed_payloads: list[dict[str, Any]] = []

    def handler(request: httpx.Request) -> httpx.Response:
        observed_payloads.append(json.loads(request.content.decode("utf-8")))
        return _openai_embedding_response(request)

    with _embedder_env() as module:
        async with _client(handler) as client:
            await module.Embedder(client=client).embed_batch(["regression"])

        assert observed_payloads == [
            {"input": ["regression"], "model": "deepvk/USER-bge-m3", "dimensions": 1024}
        ]
        assert "inputs" not in observed_payloads[0]
        assert "truncate" not in observed_payloads[0]
        assert all(value is False for value in module.SAFETY_DEFAULTS.values())


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
