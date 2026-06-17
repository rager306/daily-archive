# Formerly: src/arxiv_archive/embedder.py

"""Canonical fd embedding client for daily-archive."""

from __future__ import annotations

import asyncio
import logging
import os
import time
from collections.abc import Awaitable, Callable, Sequence
from email.utils import parsedate_to_datetime
from pathlib import Path
from statistics import median
from typing import Any

import httpx

logger = logging.getLogger(__name__)

def _env_int(name: str, default: int) -> int:
    try:
        return int(os.environ.get(name, str(default)))
    except ValueError:
        logger.warning(
            "invalid integer env value; using default",
            extra={"event": "fd_config_invalid_env", "env_var": name, "default": default},
        )
        return default


def _env_float(name: str, default: float) -> float:
    try:
        return float(os.environ.get(name, str(default)))
    except ValueError:
        logger.warning(
            "invalid float env value; using default",
            extra={"event": "fd_config_invalid_env", "env_var": name, "default": default},
        )
        return default


def _env_bool(name: str, default: bool) -> bool:
    raw = os.environ.get(name)
    if raw is None:
        return default
    return raw.lower() in ("true", "1", "yes")


def _env_str(name: str, default: str) -> str:
    return os.environ.get(name, default)


def _env_list(name: str, default: list[float]) -> list[float]:
    raw = os.environ.get(name)
    if not raw:
        return default
    try:
        return [float(x.strip()) for x in raw.split(",") if x.strip()]
    except ValueError:
        logger.warning(
            "invalid list env value; using default",
            extra={"event": "fd_config_invalid_env", "env_var": name, "default": default},
        )
        return default


DEFAULT_TEI_URL = _env_str("TEI_URL", _env_str("FD_EMBEDDINGS_ENDPOINT_BASE", "http://127.0.0.1:8000"))
DEFAULT_ENDPOINT = _env_str("FD_EMBEDDINGS_ENDPOINT", f"{DEFAULT_TEI_URL.rstrip('/')}/v1/embeddings")
DEFAULT_API_KEY = os.environ.get("FD_API_KEY")
DEFAULT_MODEL_ID = _env_str("MODEL_ID", _env_str("FD_MODEL_NAME", "deepvk/USER-bge-m3"))
DEFAULT_MODEL_NAME = DEFAULT_MODEL_ID
DEFAULT_REDIS_HOST = _env_str("REDIS_HOST", "127.0.0.1")
DEFAULT_REDIS_PORT = _env_int("REDIS_PORT", 6379)
DEFAULT_DIMENSIONS = _env_int("FD_DIMENSIONS", 1024)
DEFAULT_BATCH_SIZE = _env_int("FD_BATCH_SIZE", 32)
DEFAULT_TIMEOUT_SECONDS = _env_float("FD_REQUEST_TIMEOUT_SECONDS", 120.0)
DEFAULT_RETRY_SCHEDULE_SECONDS = tuple(_env_list("FD_RETRY_BACKOFF_SECONDS", [1.0, 5.0, 15.0, 60.0, 300.0]))
DEFAULT_MAX_ATTEMPTS = _env_int("FD_MAX_RETRIES", 3)
DEFAULT_CIRCUIT_FAILURE_THRESHOLD = _env_int("FD_CIRCUIT_FAILURE_THRESHOLD", 3)
DEFAULT_CIRCUIT_OPEN_SECONDS = _env_float("FD_CIRCUIT_OPEN_SECONDS", 60.0)
DEFAULT_GRACEFUL_DEGRADATION_ENABLED = _env_bool("FD_GRACEFUL_DEGRADATION_ENABLED", True)


def _load_dotenv_if_present(path: str | Path = ".env") -> None:
    """Load KEY=VALUE pairs from a local .env file without overriding process env.

    This is a minimal, dependency-free loader that mirrors the pattern used in
    ``scripts/m060g_smoke_test.py``. It runs once at module import so callers of
    ``Embedder`` see ``FD_API_KEY`` and related fd env vars even when the
    hosting process was started without ``source .env``.
    """
    try:
        env_path = Path(path)
    except OSError:
        return
    if not env_path.exists():
        return
    try:
        lines = env_path.read_text().splitlines()
    except OSError:
        return
    for raw_line in lines:
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip().strip('"').strip("'")
        if key and key not in os.environ:
            os.environ[key] = value


_load_dotenv_if_present()
# Re-evaluate fd env defaults now that .env has been loaded. Module-import
# ``DEFAULT_*`` constants must reflect the loaded ``FD_API_KEY`` and friends.
DEFAULT_TEI_URL = _env_str("TEI_URL", _env_str("FD_EMBEDDINGS_ENDPOINT_BASE", "http://127.0.0.1:8000"))
DEFAULT_ENDPOINT = _env_str("FD_EMBEDDINGS_ENDPOINT", f"{DEFAULT_TEI_URL.rstrip('/')}/v1/embeddings")
DEFAULT_API_KEY = os.environ.get("FD_API_KEY")
DEFAULT_MODEL_ID = _env_str("MODEL_ID", _env_str("FD_MODEL_NAME", "deepvk/USER-bge-m3"))
DEFAULT_MODEL_NAME = DEFAULT_MODEL_ID
DEFAULT_REDIS_HOST = _env_str("REDIS_HOST", "127.0.0.1")
DEFAULT_REDIS_PORT = _env_int("REDIS_PORT", 6379)
DEFAULT_DIMENSIONS = _env_int("FD_DIMENSIONS", 1024)
DEFAULT_BATCH_SIZE = _env_int("FD_BATCH_SIZE", 32)
DEFAULT_TIMEOUT_SECONDS = _env_float("FD_REQUEST_TIMEOUT_SECONDS", 120.0)
DEFAULT_RETRY_SCHEDULE_SECONDS = tuple(_env_list("FD_RETRY_BACKOFF_SECONDS", [1.0, 5.0, 15.0, 60.0, 300.0]))
DEFAULT_MAX_ATTEMPTS = _env_int("FD_MAX_RETRIES", 3)
DEFAULT_CIRCUIT_FAILURE_THRESHOLD = _env_int("FD_CIRCUIT_FAILURE_THRESHOLD", 3)
DEFAULT_CIRCUIT_OPEN_SECONDS = _env_float("FD_CIRCUIT_OPEN_SECONDS", 60.0)
DEFAULT_GRACEFUL_DEGRADATION_ENABLED = _env_bool("FD_GRACEFUL_DEGRADATION_ENABLED", True)


def _load_dotenv_if_present(path: str | Path = ".env") -> None:
    """Load KEY=VALUE pairs from a local .env file without overriding process env.

    This is a minimal, dependency-free loader that mirrors the pattern used in
    ``scripts/m060g_smoke_test.py``. It runs once at module import so callers of
    ``Embedder`` see ``FD_API_KEY`` and related fd env vars even when the
    hosting process was started without ``source .env``.
    """
    try:
        env_path = Path(path)
    except OSError:
        return
    if not env_path.exists():
        return
    try:
        lines = env_path.read_text().splitlines()
    except OSError:
        return
    for raw_line in lines:
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip().strip('"').strip("'")
        if key and key not in os.environ:
            os.environ[key] = value


_load_dotenv_if_present()

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}

CIRCUIT_CLOSED = "closed"
CIRCUIT_OPEN = "open"
CIRCUIT_HALF_OPEN = "half_open"


class FdEmbeddingError(RuntimeError):
    """Raised when fd returns an unusable embedding response."""


class Embedder:
    """Async HTTP client for generating embeddings through the local fd service."""

    def __init__(
        self,
        endpoint: str = DEFAULT_ENDPOINT,
        model_name: str = DEFAULT_MODEL_NAME,
        dimensions: int = DEFAULT_DIMENSIONS,
        batch_size: int = DEFAULT_BATCH_SIZE,
        *,
        timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
        max_attempts: int = DEFAULT_MAX_ATTEMPTS,
        retry_schedule_seconds: Sequence[float] = DEFAULT_RETRY_SCHEDULE_SECONDS,
        circuit_failure_threshold: int = DEFAULT_CIRCUIT_FAILURE_THRESHOLD,
        circuit_open_seconds: float = DEFAULT_CIRCUIT_OPEN_SECONDS,
        graceful_degradation_enabled: bool = DEFAULT_GRACEFUL_DEGRADATION_ENABLED,
        api_key: str | None = DEFAULT_API_KEY,
        client: httpx.AsyncClient | None = None,
        sleep: Callable[[float], Awaitable[None]] = asyncio.sleep,
        time_fn: Callable[[], float] = time.monotonic,
    ) -> None:
        """Initialize the canonical fd embedder.

        Args:
            endpoint: OpenAI-compatible fd embeddings endpoint.
            model_name: fd embedding model name.
            dimensions: Matryoshka dimension limit; fd supports 1024 and 512.
            batch_size: Max number of texts to send in one request.
            timeout_seconds: Per-request timeout for the fd HTTP call.
            max_attempts: Maximum attempts per batch, including the first request.
            retry_schedule_seconds: Backoff schedule used between retry attempts.
            circuit_failure_threshold: Consecutive failed attempts before opening the circuit.
            circuit_open_seconds: Cooldown before probing fd in half-open state.
            graceful_degradation_enabled: Return zero embeddings instead of raising when the circuit opens.
            api_key: Optional fd API key used only as a bearer token header.
            client: Optional injected AsyncClient for tests or shared lifecycle management.
            sleep: Async sleep function, injectable to keep retry tests fast.
            time_fn: Monotonic clock, injectable for circuit-breaker tests.
        """
        self.endpoint = endpoint
        self.model_name = model_name
        self.model_id = model_name
        self.api_key = api_key
        self.dimensions = dimensions
        self.batch_size = batch_size
        self.timeout_seconds = timeout_seconds
        self.max_attempts = max_attempts
        self.retry_schedule_seconds = tuple(retry_schedule_seconds)
        self.circuit_failure_threshold = circuit_failure_threshold
        self.circuit_open_seconds = circuit_open_seconds
        self.graceful_degradation_enabled = graceful_degradation_enabled
        self._client = client
        self._owns_client = client is None
        self._sleep = sleep
        self._time_fn = time_fn

        self.request_count = 0
        self.error_count = 0
        self._latencies: list[float] = []
        self._cache_hits = 0
        self._cache_observations = 0
        self._consecutive_failures = 0
        self._circuit_state = CIRCUIT_CLOSED
        self._circuit_opened_at: float | None = None

    @property
    def circuit_state(self) -> str:
        """Current circuit breaker state."""
        self._refresh_circuit_state()
        return self._circuit_state

    @property
    def request_headers(self) -> dict[str, str]:
        headers = {"X-Model-Id": self.model_id}
        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"
        return headers

    async def _get_client(self) -> httpx.AsyncClient:
        if self._client is None:
            self._client = httpx.AsyncClient(timeout=self.timeout_seconds)
            self._owns_client = True
        return self._client

    async def close(self) -> None:
        """Close the owned HTTP client, if this embedder created it."""
        if self._client is not None and self._owns_client:
            await self._client.aclose()
        self._client = None

    async def embed_batch(self, texts: list[str]) -> list[list[float]]:
        """Embed a single batch of text through fd.

        Returns zero embeddings when the circuit is open so callers can degrade
        gracefully without writing partial or inconsistent graph state.
        """
        if not texts:
            return []

        self._refresh_circuit_state()
        if self._circuit_state == CIRCUIT_OPEN:
            if self.graceful_degradation_enabled:
                return self._zero_embeddings(texts, reason="circuit_open")
            raise FdEmbeddingError("fd circuit is open")

        payload = {"input": texts, "model": self.model_name, "dimensions": self.dimensions}
        last_error: Exception | None = None

        for attempt in range(1, self.max_attempts + 1):
            started_at = self._time_fn()
            try:
                client = await self._get_client()
                self.request_count += 1
                logger.info(
                    "fd embedding request started",
                    extra={
                        "event": "fd_embedding_request_started",
                        "endpoint": self.endpoint,
                        "batch_size": len(texts),
                        "dimensions": self.dimensions,
                        "attempt": attempt,
                        "circuit_state": self._circuit_state,
                    },
                )
                response = await client.post(self.endpoint, json=payload, headers=self.request_headers)
                latency = self._time_fn() - started_at
                self._latencies.append(latency)

                if self._is_retriable_response(response):
                    response.raise_for_status()
                response.raise_for_status()

                self._observe_cache(response)
                embeddings = self._parse_embeddings_response(response.json(), expected_count=len(texts))
                self._record_success()
                logger.info(
                    "fd embedding request succeeded",
                    extra={
                        "event": "fd_embedding_request_succeeded",
                        "endpoint": self.endpoint,
                        "batch_size": len(texts),
                        "dimensions": self.dimensions,
                        "attempt": attempt,
                        "latency_seconds": latency,
                        "circuit_state": self._circuit_state,
                    },
                )
                return embeddings
            except (httpx.HTTPError, FdEmbeddingError, ValueError) as exc:
                latency = self._time_fn() - started_at
                if not isinstance(exc, httpx.HTTPStatusError):
                    self._latencies.append(latency)
                last_error = exc
                self._record_failure(exc)
                logger.warning(
                    "fd embedding request failed",
                    extra={
                        "event": "fd_embedding_request_failed",
                        "endpoint": self.endpoint,
                        "batch_size": len(texts),
                        "dimensions": self.dimensions,
                        "attempt": attempt,
                        "max_attempts": self.max_attempts,
                        "latency_seconds": latency,
                        "circuit_state": self._circuit_state,
                        "error_type": type(exc).__name__,
                    },
                )

                if self._circuit_state == CIRCUIT_OPEN:
                    if self.graceful_degradation_enabled:
                        return self._zero_embeddings(texts, reason="circuit_open_after_failure")
                    raise FdEmbeddingError("fd circuit opened after repeated failures")
                if attempt >= self.max_attempts or not self._is_retriable_exception(exc):
                    break

                await self._sleep(self._retry_delay(exc, attempt))

        assert last_error is not None
        raise last_error

    async def embed_all(self, texts: list[str]) -> list[list[float]]:
        """Embed all texts by splitting them into configured batches."""
        all_embeddings: list[list[float]] = []
        for index in range(0, len(texts), self.batch_size):
            batch = texts[index : index + self.batch_size]
            embeddings = await self.embed_batch(batch)
            all_embeddings.extend(embeddings)
        return all_embeddings

    def embed_batch_sync(self, texts: list[str]) -> list[list[float]]:
        """Synchronous wrapper for one-off scripts that cannot use async directly."""
        return asyncio.run(self.embed_batch(texts))

    def embed_all_sync(self, texts: list[str]) -> list[list[float]]:
        """Synchronous wrapper for embedding multiple batches."""
        return asyncio.run(self.embed_all(texts))

    def export_metrics(self) -> dict[str, Any]:
        """Return agent-readable metrics for wrapper health checks."""
        return {
            "request_count": self.request_count,
            "error_count": self.error_count,
            "latency": self._latency_summary(),
            "cache_hit_rate": self._cache_hit_rate(),
            "circuit_state": self.circuit_state,
            "circuit_state_gauge": self._circuit_state_gauge(),
        }

    def _record_success(self) -> None:
        if self._circuit_state == CIRCUIT_HALF_OPEN:
            logger.info(
                "fd embedding circuit closed",
                extra={"event": "fd_embedding_circuit_closed", "previous_state": CIRCUIT_HALF_OPEN},
            )
        self._consecutive_failures = 0
        self._circuit_state = CIRCUIT_CLOSED
        self._circuit_opened_at = None

    def _record_failure(self, exc: Exception) -> None:
        self.error_count += 1
        self._consecutive_failures += 1
        if self._consecutive_failures >= self.circuit_failure_threshold:
            if self._circuit_state != CIRCUIT_OPEN:
                logger.warning(
                    "fd embedding circuit opened",
                    extra={
                        "event": "fd_embedding_circuit_opened",
                        "failure_count": self._consecutive_failures,
                        "error_type": type(exc).__name__,
                    },
                )
            self._circuit_state = CIRCUIT_OPEN
            self._circuit_opened_at = self._time_fn()

    def _refresh_circuit_state(self) -> None:
        if self._circuit_state != CIRCUIT_OPEN or self._circuit_opened_at is None:
            return
        if self._time_fn() - self._circuit_opened_at >= self.circuit_open_seconds:
            self._circuit_state = CIRCUIT_HALF_OPEN
            logger.info(
                "fd embedding circuit half-open",
                extra={
                    "event": "fd_embedding_circuit_half_open",
                    "open_seconds": self.circuit_open_seconds,
                },
            )

    def _zero_embeddings(self, texts: list[str], *, reason: str) -> list[list[float]]:
        logger.warning(
            "fd embedding graceful degradation returned zero embeddings",
            extra={
                "event": "fd_embedding_graceful_degradation",
                "reason": reason,
                "batch_size": len(texts),
                "dimensions": self.dimensions,
                "circuit_state": self._circuit_state,
            },
        )
        return [[0.0] * self.dimensions for _ in texts]

    def _parse_embeddings_response(self, data: Any, *, expected_count: int) -> list[list[float]]:
        if not isinstance(data, dict):
            raise FdEmbeddingError(f"Expected OpenAI-style object response, got {type(data).__name__}")
        items = data.get("data")
        if not isinstance(items, list):
            raise FdEmbeddingError("Expected OpenAI-style response with list field 'data'")
        if len(items) != expected_count:
            raise FdEmbeddingError(
                f"Expected {expected_count} embeddings from fd, received {len(items)}"
            )

        embeddings_by_index: dict[int, list[float]] = {}
        for item in items:
            if not isinstance(item, dict):
                raise FdEmbeddingError("Expected each embedding item to be an object")
            index = item.get("index")
            embedding = item.get("embedding")
            if not isinstance(index, int):
                raise FdEmbeddingError("Expected embedding item to include integer index")
            if not isinstance(embedding, list):
                raise FdEmbeddingError("Expected embedding item to include list embedding")
            if len(embedding) != self.dimensions:
                raise FdEmbeddingError(
                    f"Expected embedding dimension {self.dimensions}, got {len(embedding)}"
                )
            embeddings_by_index[index] = [float(value) for value in embedding]

        try:
            return [embeddings_by_index[index] for index in range(expected_count)]
        except KeyError as exc:
            raise FdEmbeddingError(f"Missing embedding index {exc.args[0]}") from exc

    def _observe_cache(self, response: httpx.Response) -> None:
        cache_header = response.headers.get("X-Cache")
        if cache_header is None:
            return
        self._cache_observations += 1
        if cache_header.upper() == "HIT":
            self._cache_hits += 1

    def _is_retriable_response(self, response: httpx.Response) -> bool:
        return response.status_code == 429 or 500 <= response.status_code <= 599

    def _is_retriable_exception(self, exc: Exception) -> bool:
        if isinstance(exc, httpx.HTTPStatusError):
            return self._is_retriable_response(exc.response)
        return isinstance(exc, httpx.RequestError)

    def _retry_delay(self, exc: Exception, attempt: int) -> float:
        retry_after = self._retry_after_seconds(exc)
        if retry_after is not None:
            return retry_after
        schedule_index = min(attempt - 1, len(self.retry_schedule_seconds) - 1)
        return self.retry_schedule_seconds[schedule_index]

    def _retry_after_seconds(self, exc: Exception) -> float | None:
        if not isinstance(exc, httpx.HTTPStatusError):
            return None
        retry_after = exc.response.headers.get("Retry-After")
        if not retry_after:
            return None
        try:
            return max(0.0, float(retry_after))
        except ValueError:
            try:
                retry_at = parsedate_to_datetime(retry_after).timestamp()
            except (TypeError, ValueError):
                return None
            return max(0.0, retry_at - time.time())

    def _latency_summary(self) -> dict[str, float | int | None]:
        if not self._latencies:
            return {"count": 0, "p50": None, "p95": None, "p99": None}
        sorted_latencies = sorted(self._latencies)
        return {
            "count": len(sorted_latencies),
            "p50": median(sorted_latencies),
            "p95": self._percentile(sorted_latencies, 0.95),
            "p99": self._percentile(sorted_latencies, 0.99),
        }

    def _cache_hit_rate(self) -> float:
        if self._cache_observations == 0:
            return 0.0
        return self._cache_hits / self._cache_observations

    def _circuit_state_gauge(self) -> int:
        return {CIRCUIT_CLOSED: 0, CIRCUIT_HALF_OPEN: 1, CIRCUIT_OPEN: 2}[self.circuit_state]

    @staticmethod
    def _percentile(sorted_values: list[float], percentile: float) -> float:
        index = min(len(sorted_values) - 1, max(0, round((len(sorted_values) - 1) * percentile)))
        return sorted_values[index]
