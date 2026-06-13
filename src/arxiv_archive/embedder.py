from __future__ import annotations

import asyncio
import email.utils
import logging
import time
from collections import defaultdict
from dataclasses import dataclass, field
from datetime import UTC, datetime
from typing import Any

import httpx

SAFETY_DEFAULTS = {
    "external_network_authorized": False,
    "fact_promotion_authorized": False,
    "graph_writes_authorized": False,
    "llm_calls_authorized": False,
    "production_import_authorized": False,
}

DEFAULT_EMBEDDING_ENDPOINT = "http://127.0.0.1:8000/v1/embeddings"
DEFAULT_DIMENSIONS = 1024
DEFAULT_BATCH_SIZE = 32
DEFAULT_RETRY_DELAYS_SECONDS = (1.0, 5.0, 15.0, 60.0, 300.0)
DEFAULT_MAX_ATTEMPTS = 3
DEFAULT_CIRCUIT_FAILURE_THRESHOLD = 3
DEFAULT_CIRCUIT_OPEN_SECONDS = 60.0
REQUEST_DURATION_BUCKETS = (0.05, 0.1, 0.5, 1.0, 5.0, float("inf"))

logger = logging.getLogger(__name__)


class EmbedderError(RuntimeError):
    """Raised when the fd embedding response cannot be used."""


@dataclass(slots=True)
class EmbedderMetrics:
    """Small in-process metrics registry for fd embedding calls."""

    request_count: dict[str, int] = field(default_factory=lambda: defaultdict(int))
    error_count: dict[str, int] = field(default_factory=lambda: defaultdict(int))
    request_duration_seconds: list[float] = field(default_factory=list)
    cache_hits: int = 0
    cache_misses: int = 0

    def record_request(self, status: str) -> None:
        self.request_count[status] += 1

    def record_error(self, code: str) -> None:
        self.error_count[code] += 1

    def record_latency(self, duration_seconds: float) -> None:
        self.request_duration_seconds.append(duration_seconds)

    def record_cache_header(self, cache_header: str | None) -> None:
        if cache_header is None:
            return
        normalized = cache_header.strip().lower()
        if normalized == "hit":
            self.cache_hits += 1
        elif normalized == "miss":
            self.cache_misses += 1

    @property
    def cache_hit_rate(self) -> float:
        total = self.cache_hits + self.cache_misses
        if total == 0:
            return 0.0
        return self.cache_hits / total

    def histogram_counts(self) -> dict[str, int]:
        counts: dict[str, int] = {}
        for bucket in REQUEST_DURATION_BUCKETS:
            label = "+Inf" if bucket == float("inf") else str(bucket)
            counts[label] = sum(1 for value in self.request_duration_seconds if value <= bucket)
        return counts

    def percentile(self, quantile: float) -> float:
        if not self.request_duration_seconds:
            return 0.0
        ordered = sorted(self.request_duration_seconds)
        index = min(len(ordered) - 1, max(0, round((len(ordered) - 1) * quantile)))
        return ordered[index]


class Embedder:
    """Async fd/OpenAI-compatible HTTP client for generating text embeddings."""

    CIRCUIT_CLOSED = "closed"
    CIRCUIT_HALF_OPEN = "half_open"
    CIRCUIT_OPEN = "open"

    _CIRCUIT_GAUGE = {
        CIRCUIT_CLOSED: 0,
        CIRCUIT_HALF_OPEN: 1,
        CIRCUIT_OPEN: 2,
    }

    def __init__(
        self,
        endpoint: str = DEFAULT_EMBEDDING_ENDPOINT,
        dimensions: int = DEFAULT_DIMENSIONS,
        batch_size: int = DEFAULT_BATCH_SIZE,
        *,
        timeout_seconds: float = 30.0,
        max_attempts: int = DEFAULT_MAX_ATTEMPTS,
        retry_delays_seconds: tuple[float, ...] = DEFAULT_RETRY_DELAYS_SECONDS,
        retry_sleep: bool = True,
        circuit_failure_threshold: int = DEFAULT_CIRCUIT_FAILURE_THRESHOLD,
        circuit_open_seconds: float = DEFAULT_CIRCUIT_OPEN_SECONDS,
    ) -> None:
        """Initialize the hardened fd embedder wrapper.

        Args:
            endpoint: OpenAI-compatible fd embeddings URL.
            dimensions: Matryoshka dimension limit requested from fd.
            batch_size: Max number of texts to send in one request.
            timeout_seconds: Per-request timeout for the shared async client.
            max_attempts: Total HTTP attempts before surfacing a request failure.
            retry_delays_seconds: Backoff schedule used between retryable failures.
            retry_sleep: Disable in tests to avoid waiting through the production schedule.
            circuit_failure_threshold: Consecutive failed calls before opening the circuit.
            circuit_open_seconds: Cooldown before the circuit transitions to half-open.
        """
        self.endpoint = endpoint
        self.dimensions = dimensions
        self.batch_size = batch_size
        self.timeout_seconds = timeout_seconds
        self.max_attempts = max_attempts
        self.retry_delays_seconds = retry_delays_seconds
        self.retry_sleep = retry_sleep
        self.circuit_failure_threshold = circuit_failure_threshold
        self.circuit_open_seconds = circuit_open_seconds
        self.safety_defaults = dict(SAFETY_DEFAULTS)

        self.metrics = EmbedderMetrics()
        self._client: httpx.AsyncClient | None = None
        self._circuit_state = self.CIRCUIT_CLOSED
        self._consecutive_failures = 0
        self._opened_at: float | None = None
        self._half_open_in_flight = False
        self._last_call_degraded = False
        self._lock = asyncio.Lock()

    async def __aenter__(self) -> Embedder:
        return self

    async def __aexit__(self, exc_type: object, exc: object, tb: object) -> None:
        await self.close()

    async def _get_client(self) -> httpx.AsyncClient:
        if self._client is None:
            self._client = httpx.AsyncClient(timeout=self.timeout_seconds)
        return self._client

    async def close(self) -> None:
        if self._client is not None:
            await self._client.aclose()
            self._client = None

    def close_sync(self) -> None:
        """Synchronously close the underlying async client."""
        asyncio.run(self.close())

    def was_degraded(self) -> bool:
        """Return whether the most recent embedding call used zero-vector degradation."""
        return self._last_call_degraded

    @property
    def circuit_state(self) -> str:
        return self._circuit_state

    def circuit_state_value(self) -> int:
        return self._CIRCUIT_GAUGE[self._circuit_state]

    def zero_embedding(self) -> list[float]:
        return [0.0] * self.dimensions

    def zero_embeddings(self, count: int) -> list[list[float]]:
        return [self.zero_embedding() for _ in range(count)]

    async def _before_request(self) -> bool:
        """Return True when the request may hit fd, False when it must degrade."""
        async with self._lock:
            if self._circuit_state != self.CIRCUIT_OPEN:
                if self._circuit_state == self.CIRCUIT_HALF_OPEN:
                    if self._half_open_in_flight:
                        return False
                    self._half_open_in_flight = True
                return True

            opened_at = self._opened_at or 0.0
            elapsed = time.monotonic() - opened_at
            if elapsed < self.circuit_open_seconds:
                return False

            self._circuit_state = self.CIRCUIT_HALF_OPEN
            self._half_open_in_flight = True
            logger.info(
                "fd_embedder_circuit_half_open",
                extra={"endpoint": self.endpoint, "elapsed_seconds": elapsed},
            )
            return True

    async def _record_success(self) -> None:
        async with self._lock:
            self._consecutive_failures = 0
            self._opened_at = None
            self._half_open_in_flight = False
            if self._circuit_state != self.CIRCUIT_CLOSED:
                logger.info("fd_embedder_circuit_closed", extra={"endpoint": self.endpoint})
            self._circuit_state = self.CIRCUIT_CLOSED

    async def _record_failure(self, code: str) -> None:
        async with self._lock:
            self._half_open_in_flight = False
            if self._circuit_state == self.CIRCUIT_HALF_OPEN:
                self._open_circuit_locked(code)
                return

            self._consecutive_failures += 1
            if self._consecutive_failures >= self.circuit_failure_threshold:
                self._open_circuit_locked(code)

    def _open_circuit_locked(self, code: str) -> None:
        self._circuit_state = self.CIRCUIT_OPEN
        self._opened_at = time.monotonic()
        logger.warning(
            "fd_embedder_circuit_open",
            extra={
                "endpoint": self.endpoint,
                "code": code,
                "consecutive_failures": self._consecutive_failures,
                "open_seconds": self.circuit_open_seconds,
            },
        )

    def _retry_delay_for(self, response: httpx.Response | None, attempt_index: int) -> float:
        retry_after = response.headers.get("Retry-After") if response is not None else None
        if retry_after:
            try:
                return max(0.0, float(retry_after))
            except ValueError:
                parsed = email.utils.parsedate_to_datetime(retry_after)
                if parsed.tzinfo is None:
                    parsed = parsed.replace(tzinfo=UTC)
                return max(0.0, (parsed - datetime.now(UTC)).total_seconds())
        return self.retry_delays_seconds[min(attempt_index, len(self.retry_delays_seconds) - 1)]

    async def _sleep_before_retry(self, delay_seconds: float) -> None:
        if self.retry_sleep and delay_seconds > 0:
            await asyncio.sleep(delay_seconds)

    @staticmethod
    def _is_retryable_status(status_code: int) -> bool:
        return status_code == 429 or status_code == 503 or 500 <= status_code <= 599

    async def _post_with_retry(self, payload: dict[str, Any]) -> httpx.Response:
        client = await self._get_client()
        last_error: BaseException | None = None
        last_code = "unknown"

        for attempt_index in range(self.max_attempts):
            started_at = time.perf_counter()
            response: httpx.Response | None = None
            try:
                response = await client.post(self.endpoint, json=payload)
                self.metrics.record_cache_header(response.headers.get("X-Cache"))
                if 200 <= response.status_code < 300:
                    self.metrics.record_latency(time.perf_counter() - started_at)
                    return response

                last_code = str(response.status_code)
                self.metrics.record_error(last_code)
                last_error = httpx.HTTPStatusError(
                    f"fd embedding request failed with HTTP {response.status_code}",
                    request=response.request,
                    response=response,
                )
                if not self._is_retryable_status(response.status_code):
                    raise last_error
            except httpx.TimeoutException as exc:
                last_code = "timeout"
                last_error = exc
                self.metrics.record_error(last_code)
            except httpx.HTTPError as exc:
                last_error = exc
                response = getattr(exc, "response", None)
                if response is not None:
                    last_code = str(response.status_code)
                    self.metrics.record_error(last_code)
                    if not self._is_retryable_status(response.status_code):
                        raise
                else:
                    last_code = exc.__class__.__name__
                    self.metrics.record_error(last_code)

            if attempt_index >= self.max_attempts - 1:
                break

            delay_seconds = self._retry_delay_for(response, attempt_index)
            logger.warning(
                "fd_embedder_retry_scheduled",
                extra={
                    "endpoint": self.endpoint,
                    "attempt": attempt_index + 1,
                    "max_attempts": self.max_attempts,
                    "delay_seconds": delay_seconds,
                    "code": last_code,
                },
            )
            await self._sleep_before_retry(delay_seconds)

        if last_error is None:
            raise EmbedderError("fd embedding request failed without an exception")
        raise last_error

    def _parse_embeddings(self, data: Any, expected_count: int) -> list[list[float]]:
        if not isinstance(data, dict):
            raise EmbedderError(f"fd response must be an object, got {type(data).__name__}")
        rows = data.get("data")
        if not isinstance(rows, list):
            raise EmbedderError("fd response is missing data[]")

        ordered_rows = sorted(rows, key=lambda item: int(item.get("index", 0)))
        embeddings: list[list[float]] = []
        for row in ordered_rows:
            if not isinstance(row, dict):
                raise EmbedderError("fd response data[] row must be an object")
            embedding = row.get("embedding")
            if not isinstance(embedding, list):
                raise EmbedderError("fd response row is missing embedding[]")
            if len(embedding) != self.dimensions:
                raise EmbedderError(
                    f"fd returned embedding dimension {len(embedding)}, expected {self.dimensions}"
                )
            embeddings.append([float(value) for value in embedding])

        if len(embeddings) != expected_count:
            raise EmbedderError("fd response length did not match request length")
        return embeddings

    async def embed_batch(self, texts: list[str]) -> list[list[float]]:
        """Generate embeddings for a batch of texts."""
        if not texts:
            self._last_call_degraded = False
            return []

        may_request = await self._before_request()
        if not may_request:
            self._last_call_degraded = True
            self.metrics.record_request("degraded")
            logger.warning(
                "fd_embedder_degraded_zero_embedding",
                extra={
                    "endpoint": self.endpoint,
                    "batch_size": len(texts),
                    "dimensions": self.dimensions,
                    "circuit_state": self._circuit_state,
                },
            )
            return self.zero_embeddings(len(texts))

        payload = {"input": texts, "dimensions": self.dimensions}
        try:
            response = await self._post_with_retry(payload)
            embeddings = self._parse_embeddings(response.json(), len(texts))
        except httpx.TimeoutException as exc:
            self._last_call_degraded = False
            self.metrics.record_request("timeout")
            await self._record_failure("timeout")
            logger.error(
                "fd_embedder_request_timeout",
                extra={"endpoint": self.endpoint, "batch_size": len(texts)},
            )
            raise EmbedderError("fd embedding request timed out") from exc
        except Exception as exc:
            self._last_call_degraded = False
            self.metrics.record_request("error")
            code = self._error_code(exc)
            await self._record_failure(code)
            logger.error(
                "fd_embedder_request_error",
                extra={"endpoint": self.endpoint, "batch_size": len(texts), "code": code},
            )
            raise

        self._last_call_degraded = False
        self.metrics.record_request("success")
        await self._record_success()
        logger.info(
            "fd_embedder_request_success",
            extra={
                "endpoint": self.endpoint,
                "batch_size": len(texts),
                "dimensions": self.dimensions,
                "circuit_state": self._circuit_state,
            },
        )
        return embeddings

    async def embed_all(self, texts: list[str]) -> list[list[float]]:
        """Embed all texts by splitting them into batches."""
        all_embeddings: list[list[float]] = []
        for index in range(0, len(texts), self.batch_size):
            batch = texts[index : index + self.batch_size]
            all_embeddings.extend(await self.embed_batch(batch))
        return all_embeddings

    def embed_batch_sync(self, texts: list[str]) -> list[list[float]]:
        """Synchronous wrapper around embed_batch for scripts that are not async."""
        return asyncio.run(self.embed_batch(texts))

    def embed_all_sync(self, texts: list[str]) -> list[list[float]]:
        """Synchronous wrapper around embed_all for scripts that are not async."""
        return asyncio.run(self.embed_all(texts))

    def _error_code(self, exc: BaseException) -> str:
        if isinstance(exc, httpx.HTTPStatusError):
            return str(exc.response.status_code)
        return exc.__class__.__name__

    def export_metrics(self) -> str:
        """Export current wrapper metrics in Prometheus-style text format."""
        lines = [
            "# HELP request_count Total fd embedding wrapper requests",
            "# TYPE request_count counter",
        ]
        for status in ("success", "error", "timeout", "degraded"):
            lines.append(f'request_count{{status="{status}"}} {self.metrics.request_count[status]}')

        lines.extend(
            [
                "# HELP error_count Total fd embedding wrapper errors",
                "# TYPE error_count counter",
            ]
        )
        for code, count in sorted(self.metrics.error_count.items()):
            lines.append(f'error_count{{code="{code}"}} {count}')

        lines.extend(
            [
                "# HELP request_duration_seconds fd embedding wrapper request latency",
                "# TYPE request_duration_seconds histogram",
            ]
        )
        for label, count in self.metrics.histogram_counts().items():
            lines.append(f'request_duration_seconds_bucket{{le="{label}"}} {count}')
        lines.append(f"request_duration_seconds_count {len(self.metrics.request_duration_seconds)}")
        lines.append(
            "request_duration_seconds_sum "
            f"{sum(self.metrics.request_duration_seconds):.9f}"
        )
        lines.append(f"request_duration_seconds_p50 {self.metrics.percentile(0.50):.9f}")
        lines.append(f"request_duration_seconds_p95 {self.metrics.percentile(0.95):.9f}")
        lines.append(f"request_duration_seconds_p99 {self.metrics.percentile(0.99):.9f}")

        lines.extend(
            [
                "# HELP cache_hit_rate fd embedding wrapper cache hit ratio from fd headers",
                "# TYPE cache_hit_rate gauge",
                f"cache_hit_rate {self.metrics.cache_hit_rate:.9f}",
                "# HELP circuit_state fd embedding wrapper circuit state 0=closed 1=half_open 2=open",
                "# TYPE circuit_state gauge",
                f"circuit_state {self.circuit_state_value()}",
            ]
        )
        return "\n".join(lines) + "\n"
