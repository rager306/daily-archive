#!/usr/bin/env python3
"""fd v2 contract test harness for M062 S03.

This script validates the daily-archive fd integration contract from ADR-019 and
/root/fd/docs/fd-v2.md against the currently running fd service. Contract
failures are reported as data, not as process failures, because fd v1 is known to
miss many v2 endpoints during this milestone.
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import contextlib
import json
import os
import statistics
import sys
import time
from collections import Counter, defaultdict
from collections.abc import Callable, Iterator
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import httpx

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT / "src") not in sys.path:
    sys.path.insert(0, str(ROOT / "src"))

from arxiv_archive.embedder import CIRCUIT_CLOSED, CIRCUIT_OPEN, Embedder  # noqa: E402

DEFAULT_TEI_URL = "http://127.0.0.1:8000"
DEFAULT_ENDPOINT = f"{DEFAULT_TEI_URL}/v1/embeddings"
DEFAULT_MODEL_ID = "deepvk/USER-bge-m3"
DEFAULT_REDIS_HOST = "127.0.0.1"
DEFAULT_REDIS_PORT = "6379"
ARTIFACT_DIR = Path(os.environ.get("FD_CONTRACT_REPORT_DIR", "artifacts/m062-fd-contract"))
RESULTS_JSON = "fd-contract-results-v2.json"
PRIOR_REPORT_MD = "fd-contract-report.md"
REPORT_MD = "fd-contract-report-v2.md"
GAP_MD = "fd-actual-vs-required-v2.md"
SNIPPET_LIMIT = 240


@dataclass(frozen=True)
class Evidence:
    status_code: int | None = None
    body_snippet: str = ""
    headers: dict[str, str] = field(default_factory=dict)
    latency_ms: float | None = None
    note: str = ""

    def compact(self) -> str:
        parts: list[str] = []
        if self.status_code is not None:
            parts.append(f"status={self.status_code}")
        if self.latency_ms is not None:
            parts.append(f"latency_ms={self.latency_ms:.1f}")
        if self.headers:
            header_bits = ", ".join(f"{key}={value}" for key, value in sorted(self.headers.items()))
            parts.append(f"headers={{{header_bits}}}")
        if self.body_snippet:
            parts.append(f"body={self.body_snippet!r}")
        if self.note:
            parts.append(f"note={self.note}")
        return "; ".join(parts) if parts else "no evidence"


@dataclass(frozen=True)
class TestResult:
    test_id: str
    category: str
    description: str
    expected: str
    status: str
    evidence: Evidence
    requirements: tuple[str, ...] = ()

    @property
    def observed(self) -> str:
        return self.evidence.compact()


@dataclass(frozen=True)
class HttpCase:
    test_id: str
    category: str
    description: str
    method: str
    path: str
    expected: str
    expected_status: int | None = None
    json_payload: Any | None = None
    raw_body: bytes | None = None
    headers: dict[str, str] = field(default_factory=dict)
    validator: Callable[[httpx.Response, float], tuple[bool, str]] | None = None
    requirements: tuple[str, ...] = ()
    skip_reason: str | None = None
    use_auth: bool = True


def get_base_url() -> str:
    """Return the fd base URL from the v2 TEI_URL env, with legacy fallback."""
    return os.environ.get("TEI_URL") or os.environ.get("FD_EMBEDDINGS_ENDPOINT_BASE") or DEFAULT_TEI_URL


def get_endpoint() -> str:
    """Return the embeddings endpoint from TEI_URL, allowing explicit legacy override."""
    explicit_endpoint = os.environ.get("FD_EMBEDDINGS_ENDPOINT")
    if explicit_endpoint:
        return explicit_endpoint
    base_url = get_base_url().rstrip("/")
    if base_url.endswith("/v1/embeddings"):
        return base_url
    return f"{base_url}/v1/embeddings"


def get_model_id() -> str:
    """Return the model id advertised by fd v2."""
    return os.environ.get("MODEL_ID") or os.environ.get("FD_MODEL_NAME") or DEFAULT_MODEL_ID


def get_redis_host() -> str:
    """Return Redis host env used by fd v2 cache wiring."""
    return os.environ.get("REDIS_HOST", DEFAULT_REDIS_HOST)


def get_redis_port() -> str:
    """Return Redis port env used by fd v2 cache wiring."""
    return os.environ.get("REDIS_PORT", DEFAULT_REDIS_PORT)


def get_auth_headers(api_key: str | None = None) -> dict[str, str]:
    """Return Authorization header from FD_API_KEY without exposing it in evidence."""
    key = os.environ.get("FD_API_KEY") if api_key is None else api_key
    if not key:
        return {}
    return {"Authorization": f"Bearer {key}"}


@contextlib.contextmanager
def temporary_env(values: dict[str, str | None]) -> Iterator[None]:
    previous = {key: os.environ.get(key) for key in values}
    try:
        for key, value in values.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
        yield
    finally:
        for key, value in previous.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value


def _snippet(text: str) -> str:
    squashed = " ".join(text.split())
    return squashed[:SNIPPET_LIMIT]


def _selected_headers(response: httpx.Response | None) -> dict[str, str]:
    if response is None:
        return {}
    interesting = [
        "Server",
        "X-Request-Id",
        "X-Model-Id",
        "X-Dimensions",
        "X-Cache",
        "Retry-After",
        "Connection",
        "ETag",
        "Content-Type",
        "Cache-Control",
    ]
    return {name: response.headers[name] for name in interesting if name in response.headers}


def _json(response: httpx.Response) -> Any:
    try:
        return response.json()
    except ValueError:
        return None


def _error_code(response: httpx.Response) -> str | None:
    data = _json(response)
    if isinstance(data, dict):
        error = data.get("error")
        if isinstance(error, dict):
            code = error.get("code")
            return str(code) if code is not None else None
        code = data.get("code")
        if code is not None:
            return str(code)
    return None


def _embedding_lengths(response: httpx.Response) -> list[int]:
    data = _json(response)
    if not isinstance(data, dict):
        return []
    items = data.get("data")
    if not isinstance(items, list):
        return []
    lengths: list[int] = []
    for item in items:
        if not isinstance(item, dict):
            continue
        embedding = item.get("embedding")
        if isinstance(embedding, list):
            lengths.append(len(embedding))
        elif isinstance(embedding, str):
            try:
                decoded = base64.b64decode(embedding, validate=True)
            except Exception:
                lengths.append(-1)
            else:
                lengths.append(len(decoded))
    return lengths


def _has_version_field(response: httpx.Response) -> bool:
    data = _json(response)
    return isinstance(data, dict) and "version" in data


def _body_contains_model_loaded_true(response: httpx.Response) -> bool:
    data = _json(response)
    if isinstance(data, dict):
        return data.get("model_loaded") is True or data.get("model_loaded") == "true"
    return "model_loaded" in response.text and "true" in response.text.lower()


def _code_is(expected_code: str) -> Callable[[httpx.Response, float], tuple[bool, str]]:
    def validate(response: httpx.Response, _latency: float) -> tuple[bool, str]:
        code = _error_code(response)
        if code == expected_code:
            return True, f"code={code}"
        return False, f"code={code!r}, expected={expected_code!r}"

    return validate


def _status_and_code(expected_status: int, expected_code: str) -> Callable[[httpx.Response, float], tuple[bool, str]]:
    def validate(response: httpx.Response, _latency: float) -> tuple[bool, str]:
        code = _error_code(response)
        ok = response.status_code == expected_status and code == expected_code
        return ok, f"status={response.status_code}, code={code!r}"

    return validate


def _embedding_count_and_dim(count: int, dim: int) -> Callable[[httpx.Response, float], tuple[bool, str]]:
    def validate(response: httpx.Response, _latency: float) -> tuple[bool, str]:
        lengths = _embedding_lengths(response)
        ok = response.status_code == 200 and len(lengths) == count and all(length == dim for length in lengths)
        return ok, f"embedding_count={len(lengths)}, dimensions={lengths[:5]}"

    return validate


def _base64_embedding(response: httpx.Response, _latency: float) -> tuple[bool, str]:
    data = _json(response)
    items = data.get("data") if isinstance(data, dict) else None
    embedding = items[0].get("embedding") if isinstance(items, list) and items and isinstance(items[0], dict) else None
    ok = response.status_code == 200 and isinstance(embedding, str)
    return ok, f"embedding_type={type(embedding).__name__}"


def _header_equals(name: str, expected: str) -> Callable[[httpx.Response, float], tuple[bool, str]]:
    def validate(response: httpx.Response, _latency: float) -> tuple[bool, str]:
        observed = response.headers.get(name)
        return observed == expected, f"{name}={observed!r}, expected={expected!r}"

    return validate


def _header_present(name: str) -> Callable[[httpx.Response, float], tuple[bool, str]]:
    def validate(response: httpx.Response, _latency: float) -> tuple[bool, str]:
        observed = response.headers.get(name)
        return bool(observed), f"{name}={observed!r}"

    return validate


def _content_type_contains(expected: str) -> Callable[[httpx.Response, float], tuple[bool, str]]:
    def validate(response: httpx.Response, _latency: float) -> tuple[bool, str]:
        observed = response.headers.get("Content-Type", "")
        return expected in observed, f"Content-Type={observed!r}, expected_contains={expected!r}"

    return validate


def _latency_under(target_ms: float) -> Callable[[httpx.Response, float], tuple[bool, str]]:
    def validate(response: httpx.Response, latency_ms: float) -> tuple[bool, str]:
        cache = response.headers.get("X-Cache")
        ok = response.status_code == 200 and latency_ms < target_ms and cache == "HIT"
        return ok, f"latency_ms={latency_ms:.1f}, target_ms={target_ms:.1f}, X-Cache={cache!r}"

    return validate


def _not_legacy_unmarshal(response: httpx.Response, _latency: float) -> tuple[bool, str]:
    code = _error_code(response)
    legacy = "json: cannot unmarshal" in response.text
    ok = response.status_code == 400 and code == "invalid_request_error" and not legacy
    return ok, f"status={response.status_code}, code={code!r}, legacy_unmarshal={legacy}"


def _url_for(case: HttpCase) -> str:
    if case.path == "$endpoint":
        return get_endpoint()
    return get_base_url().rstrip("/") + case.path


def _run_http_case(client: httpx.Client, case: HttpCase) -> TestResult:
    if case.skip_reason:
        return TestResult(
            case.test_id,
            case.category,
            case.description,
            case.expected,
            "SKIP",
            Evidence(note=case.skip_reason),
            case.requirements,
        )

    if case.use_auth and not os.environ.get("FD_API_KEY"):
        return TestResult(
            case.test_id,
            case.category,
            case.description,
            case.expected,
            "SKIP",
            Evidence(note="FD_API_KEY is not configured; protected fd v2 request is not authorized for verification"),
            case.requirements,
        )

    url = _url_for(case)
    headers = get_auth_headers() if case.use_auth else {}
    headers.update(case.headers)
    started = time.perf_counter()
    try:
        response = client.request(
            case.method,
            url,
            json=case.json_payload,
            content=case.raw_body,
            headers=headers,
        )
        latency_ms = (time.perf_counter() - started) * 1000
    except Exception as exc:
        return TestResult(
            case.test_id,
            case.category,
            case.description,
            case.expected,
            "FAIL",
            Evidence(note=f"request_error={type(exc).__name__}: {exc}"),
            case.requirements,
        )

    expected_status_ok = case.expected_status is None or response.status_code == case.expected_status
    validator_ok = True
    validator_note = ""
    if case.validator is not None:
        validator_ok, validator_note = case.validator(response, latency_ms)
    status = "PASS" if expected_status_ok and validator_ok else "FAIL"
    expected_note = f"expected_status={case.expected_status}" if case.expected_status is not None else ""
    note = ", ".join(part for part in [expected_note, validator_note] if part)
    return TestResult(
        case.test_id,
        case.category,
        case.description,
        case.expected,
        status,
        Evidence(
            status_code=response.status_code,
            body_snippet=_snippet(response.text),
            headers=_selected_headers(response),
            latency_ms=latency_ms,
            note=note,
        ),
        case.requirements,
    )


def build_http_cases() -> list[HttpCase]:
    return [
        HttpCase("T-H-1", "happy", "single 1024-d embedding", "POST", "$endpoint", "200, dimensions=1024 in response", 200, {"input": ["hello"]}, validator=_embedding_count_and_dim(1, 1024)),
        HttpCase("T-H-2", "happy", "single 512-d embedding", "POST", "$endpoint", "200, dimensions=512", 200, {"input": ["hello"], "dimensions": 512}, validator=_embedding_count_and_dim(1, 512)),
        HttpCase("T-H-3", "happy", "three embeddings", "POST", "$endpoint", "200, 3 embeddings", 200, {"input": ["a", "b", "c"]}, validator=_embedding_count_and_dim(3, 1024)),
        HttpCase("T-H-4", "happy", "max batch of 32 embeddings", "POST", "$endpoint", "200, 32 embeddings", 200, {"input": ["a"] * 32}, validator=_embedding_count_and_dim(32, 1024), requirements=("R-P0-2",)),
        HttpCase("T-H-5", "happy", "base64 encoding_format", "POST", "$endpoint", "200, base64 string", 200, {"input": ["a"], "encoding_format": "base64"}, validator=_base64_embedding, requirements=("R-P1-5",)),
        HttpCase("T-H-6", "happy", "priority option", "POST", "$endpoint", "200", 200, {"input": ["a"], "priority": "high"}, requirements=("R-P1-7",)),
        HttpCase("T-H-7", "happy", "health deep check", "GET", "/health", "200, body contains model_loaded: true", 200, validator=lambda r, _l: (_body_contains_model_loaded_true(r), f"model_loaded_true={_body_contains_model_loaded_true(r)}"), requirements=("R-P1-1",)),
        HttpCase("T-H-8", "happy", "liveness endpoint", "GET", "/live", "200", 200),
        HttpCase("T-H-9", "happy", "readiness endpoint", "GET", "/ready", "200 after warmup", 200, requirements=("R-P0-4",)),
        HttpCase("T-H-10", "happy", "version endpoint", "GET", "/version", "200, version field", 200, validator=lambda r, _l: (_has_version_field(r), f"has_version={_has_version_field(r)}"), requirements=("R-P0-7",)),
        HttpCase("T-E-1", "error", "missing input", "POST", "$endpoint", "400, code=input_required", 400, {}, validator=_code_is("input_required"), requirements=("R-P0-18", "R-P0-19")),
        HttpCase("T-E-2", "error", "empty input", "POST", "$endpoint", "400, code=input_required", 400, {"input": []}, validator=_code_is("input_required"), requirements=("R-P0-18", "R-P0-19")),
        HttpCase("T-E-3", "error", "invalid dimensions", "POST", "$endpoint", "400, code=dimensions_invalid", 400, {"input": ["a"], "dimensions": 99999}, validator=_code_is("dimensions_invalid"), requirements=("R-P0-18", "R-P0-19")),
        HttpCase("T-E-4", "error", "invalid non-string input", "POST", "$endpoint", "400, code=invalid_request_error, no legacy unmarshal", 400, {"input": [123]}, validator=_not_legacy_unmarshal, requirements=("R-P0-18", "R-P0-19")),
        HttpCase("T-E-5", "error", "malformed JSON", "POST", "$endpoint", "400, code=invalid_json", 400, raw_body=b"{malformed", headers={"Content-Type": "application/json"}, validator=_code_is("invalid_json"), requirements=("R-P0-18", "R-P0-19")),
        HttpCase("T-E-6", "error", "batch too large", "POST", "$endpoint", "413, code=batch_too_large", 413, {"input": ["a"] * 100}, validator=_code_is("batch_too_large"), requirements=("R-P0-2", "R-P0-18", "R-P0-19")),
        HttpCase("T-E-7", "error", "input too long", "POST", "$endpoint", "413, code=input_too_long", 413, {"input": ["x" * 10000]}, validator=_code_is("input_too_long"), requirements=("R-P0-1", "R-P0-18", "R-P0-19")),
        HttpCase("T-E-8", "error", "GET embeddings method not allowed", "GET", "/v1/embeddings", "405, not 404", 405, requirements=("R-P0-19",)),
        HttpCase("T-E-9", "error", "auth rejects invalid bearer token", "POST", "$endpoint", "401, code=unauthorized or is not authorized", 401, {"input": ["a"]}, headers=get_auth_headers("test-fd-api-key-12345"), validator=_code_is("unauthorized"), requirements=("R-P1-8",), use_auth=False),
        HttpCase("T-E-10", "error", "unknown route", "GET", "/v9999", "404, code=not_found", 404, validator=_code_is("not_found"), requirements=("R-P0-18", "R-P0-19")),
        HttpCase("T-E-11", "error", "during graceful shutdown", "POST", "$endpoint", "503, code=shutting_down, Retry-After: 30", skip_reason="shutdown mutation is disabled by default for this read-only daily-archive contract harness", requirements=("R-P0-5", "R-P0-16", "R-P0-19")),
        HttpCase("T-E-12", "error", "model not loaded", "POST", "$endpoint", "503, code=model_not_loaded, Retry-After: 5", skip_reason="model-unloaded fixture is disabled by default; actual fd is expected to be warm or externally managed", requirements=("R-P0-3", "R-P0-16", "R-P0-19")),
        HttpCase("T-E-13", "error", "rate limit hit", "POST", "$endpoint", "429, code=rate_limit_exceeded, Retry-After: 60", skip_reason="rate-limit hammering is disabled by default to avoid mutating local fd state", requirements=("R-P0-16", "R-P0-19", "R-P2-5")),
        HttpCase("T-E-14", "error", "oversized 50MB body", "POST", "$endpoint", "413, code=payload_too_large", skip_reason="50MB payload test is disabled by default to avoid excessive local resource usage", requirements=("R-P0-19",)),
        HttpCase("T-E-15", "error", "forced internal error", "POST", "$endpoint", "500, code=internal_error, X-Request-Id in body", skip_reason="forced internal-error injection is disabled by default; fd exposes no safe fixture endpoint", requirements=("R-P0-11", "R-P0-18", "R-P0-19")),
        HttpCase("T-HDR-1", "headers", "server version header", "GET", "/health", "Server: fd/2.0.0", validator=_header_equals("Server", "fd/2.0.0"), requirements=("R-P0-12",)),
        HttpCase("T-HDR-2", "headers", "request id echo", "POST", "$endpoint", "response echoes X-Request-Id: my-id", 200, {"input": ["a"]}, headers={"X-Request-Id": "my-id"}, validator=_header_equals("X-Request-Id", "my-id"), requirements=("R-P0-11",)),
        HttpCase("T-HDR-3", "headers", "generated request id", "POST", "$endpoint", "response has X-Request-Id", 200, {"input": ["a"]}, validator=_header_present("X-Request-Id"), requirements=("R-P0-11",)),
        HttpCase("T-HDR-4", "headers", "model id header", "POST", "$endpoint", "X-Model-Id matches MODEL_ID", 200, {"input": ["a"]}, validator=_header_equals("X-Model-Id", get_model_id()), requirements=("R-P0-13",)),
        HttpCase("T-HDR-5", "headers", "dimensions header", "POST", "$endpoint", "X-Dimensions: 1024", 200, {"input": ["a"]}, validator=_header_equals("X-Dimensions", "1024"), requirements=("R-P0-14",)),
        HttpCase("T-HDR-6", "headers", "cache hit header on repeat", "POST", "$endpoint", "X-Cache: HIT", 200, {"input": ["cache-hot"]}, validator=_header_equals("X-Cache", "HIT"), requirements=("R-P0-15", "R-P1-4")),
        HttpCase("T-HDR-7", "headers", "cache miss header on first request", "POST", "$endpoint", "X-Cache: MISS", 200, {"input": [f"cache-miss-{int(time.time())}"]}, validator=_header_equals("X-Cache", "MISS"), requirements=("R-P0-15", "R-P1-4")),
        HttpCase("T-HDR-8", "headers", "Retry-After on temporary failure", "POST", "$endpoint", "429/503 response has Retry-After", skip_reason="temporary-failure fixture is disabled by default; no safe fd trigger exists", requirements=("R-P0-16",)),
        HttpCase("T-HDR-9", "headers", "keep-alive connection", "GET", "/health", "Connection: keep-alive", validator=_header_equals("Connection", "keep-alive"), requirements=("R-P0-17",)),
        HttpCase("T-HDR-10", "headers", "cache hit ETag", "POST", "$endpoint", "ETag: <hash>", 200, {"input": ["cache-hot"]}, validator=_header_present("ETag"), requirements=("R-P2-2",)),
        HttpCase("T-P-1", "performance", "1 input cache-hot p95 target", "POST", "$endpoint", "p95 < 50ms and X-Cache: HIT", 200, {"input": ["perf-one"]}, validator=_latency_under(50), requirements=("R-P0-6", "R-P1-4")),
        HttpCase("T-P-2", "performance", "10 inputs cache-hot p95 target", "POST", "$endpoint", "p95 < 200ms and X-Cache: HIT", 200, {"input": ["perf-ten"] * 10}, validator=_latency_under(200), requirements=("R-P0-6", "R-P1-4")),
        HttpCase("T-P-3", "performance", "32 inputs cache-hot p95 target", "POST", "$endpoint", "p95 < 1000ms and X-Cache: HIT", 200, {"input": ["perf-thirty-two"] * 32}, validator=_latency_under(1000), requirements=("R-P0-6", "R-P1-4")),
        HttpCase("T-EX-1", "endpoints", "version endpoint exists", "GET", "/version", "200, not 404", 200, requirements=("R-P0-7",)),
        HttpCase("T-EX-2", "endpoints", "info endpoint exists", "GET", "/info", "200, not 404", 200, requirements=("R-P0-8",)),
        HttpCase("T-EX-3", "endpoints", "metrics endpoint exists", "GET", "/metrics", "200, Content-Type: text/plain", 200, validator=_content_type_contains("text/plain"), requirements=("R-P0-9",)),
        HttpCase("T-EX-4", "endpoints", "OpenAPI schema exists", "GET", "/openapi.json", "200, not 404", 200, requirements=("R-P2-1",)),
        HttpCase("T-EX-5", "endpoints", "Swagger UI exists", "GET", "/docs", "200, not 404", 200, requirements=("R-P2-1",)),
    ]


def _run_performance_sequence(client: httpx.Client) -> list[TestResult]:
    if not os.environ.get("FD_API_KEY"):
        note = "FD_API_KEY is not configured; protected fd v2 request is not authorized for verification"
        return [
            TestResult("T-P-4", "performance", "100 sequential cache-hot requests", "p95 < 50ms, all X-Cache=HIT", "SKIP", Evidence(note=note), ("R-P0-6",)),
            TestResult("T-P-5", "performance", "concurrency 32 cache-hot requests", "p95 < 50ms, all X-Cache=HIT", "SKIP", Evidence(note=note), ("R-P0-6",)),
        ]

    results: list[TestResult] = []
    endpoint = get_endpoint()

    def post(payload: dict[str, Any]) -> tuple[httpx.Response | None, float, str]:
        started = time.perf_counter()
        try:
            response = client.post(endpoint, json=payload, headers=get_auth_headers())
            return response, (time.perf_counter() - started) * 1000, ""
        except Exception as exc:
            return None, (time.perf_counter() - started) * 1000, f"request_error={type(exc).__name__}: {exc}"

    # T-P-4: contract says 100 sequential cache-hot requests. Abort early on first error
    # to keep fd-down runs quick while still recording the first failure evidence.
    latencies: list[float] = []
    cache_values: Counter[str | None] = Counter()
    errors: list[str] = []
    for index in range(100):
        response, latency_ms, error = post({"input": ["perf-sequential"]})
        latencies.append(latency_ms)
        if error:
            errors.append(f"{index + 1}/100 {error}")
            break
        assert response is not None
        cache_values[response.headers.get("X-Cache")] += 1
        if response.status_code != 200:
            errors.append(f"{index + 1}/100 status={response.status_code}")
            break
    p95 = statistics.quantiles(latencies, n=20)[18] if len(latencies) >= 20 else max(latencies or [0.0])
    sequential_ok = not errors and cache_values.get("HIT", 0) == 100
    results.append(
        TestResult(
            "T-P-4",
            "performance",
            "100 sequential cache-hot requests",
            "0 errors, 0 timeouts, X-Cache: HIT",
            "PASS" if sequential_ok else "FAIL",
            Evidence(
                latency_ms=p95,
                headers={"X-Cache-HIT-count": str(cache_values.get("HIT", 0))},
                note=f"completed={sum(cache_values.values())}/100, errors={errors[:1]}",
            ),
            ("R-P0-6", "R-P1-4"),
        )
    )

    # T-P-5: 4 concurrent callers, each with 8 inputs. Use threads via httpx sync client.
    import concurrent.futures

    started = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
        futures = [executor.submit(post, {"input": [f"perf-concurrent-{i}"] * 8}) for i in range(4)]
        concurrent_results = [future.result() for future in futures]
    total_ms = (time.perf_counter() - started) * 1000
    statuses = [response.status_code if response is not None else None for response, _lat, _err in concurrent_results]
    caches = [response.headers.get("X-Cache") if response is not None else None for response, _lat, _err in concurrent_results]
    errors = [error for _response, _lat, error in concurrent_results if error]
    concurrent_ok = not errors and statuses == [200, 200, 200, 200] and total_ms < 2000 and all(cache == "HIT" for cache in caches)
    results.append(
        TestResult(
            "T-P-5",
            "performance",
            "concurrent 4 callers x 8 cache-hot inputs",
            "all succeed, total time < 2s, X-Cache: HIT",
            "PASS" if concurrent_ok else "FAIL",
            Evidence(
                latency_ms=total_ms,
                headers={"X-Cache-values": ",".join(str(cache) for cache in caches)},
                note=f"statuses={statuses}, errors={errors}",
            ),
            ("R-P0-6", "R-P1-4"),
        )
    )
    return results


class SequenceTransport(httpx.AsyncBaseTransport):
    def __init__(self, statuses: list[int], dimensions: int = 4) -> None:
        self.statuses = statuses
        self.dimensions = dimensions
        self.calls = 0

    async def handle_async_request(self, request: httpx.Request) -> httpx.Response:
        status = self.statuses[min(self.calls, len(self.statuses) - 1)]
        self.calls += 1
        if status == 200:
            body = {
                "object": "list",
                "data": [{"object": "embedding", "index": 0, "embedding": [0.25] * self.dimensions}],
                "model": "deepvk/USER-bge-m3",
            }
        else:
            body = {"error": {"message": "forced failure", "code": "internal_error"}}
        return httpx.Response(status, json=body, request=request)


async def _wrapper_three_failures_then_zero() -> TestResult:
    transport = SequenceTransport([500, 500, 500], dimensions=4)
    client = httpx.AsyncClient(transport=transport)
    embedder = Embedder(
        endpoint=DEFAULT_ENDPOINT,
        dimensions=4,
        max_attempts=1,
        circuit_failure_threshold=3,
        circuit_open_seconds=60,
        graceful_degradation_enabled=True,
        client=client,
        sleep=lambda _seconds: asyncio.sleep(0),
    )
    try:
        observed_errors: list[str] = []
        for _ in range(3):
            try:
                await embedder.embed_batch(["a"])
            except httpx.HTTPStatusError as exc:
                observed_errors.append(str(exc.response.status_code))
        fourth = await embedder.embed_batch(["a"])
        ok = fourth == [[0.0] * 4] and embedder.circuit_state == CIRCUIT_OPEN and len(observed_errors) == 2
        return TestResult(
            "T-W-1",
            "wrapper",
            "force 3 failures, fourth call returns zero embedding",
            "circuit open and zero embedding",
            "PASS" if ok else "FAIL",
            Evidence(note=f"state={embedder.circuit_state}, first_errors={observed_errors}, fourth={fourth}, calls={transport.calls}"),
        )
    finally:
        await client.aclose()


async def _wrapper_circuit_recovers_after_cooldown() -> TestResult:
    now = 1000.0

    def time_fn() -> float:
        return now

    transport = SequenceTransport([500, 500, 500, 200], dimensions=4)
    client = httpx.AsyncClient(transport=transport)
    embedder = Embedder(
        endpoint=DEFAULT_ENDPOINT,
        dimensions=4,
        max_attempts=1,
        circuit_failure_threshold=3,
        circuit_open_seconds=60,
        graceful_degradation_enabled=True,
        client=client,
        sleep=lambda _seconds: asyncio.sleep(0),
        time_fn=time_fn,
    )
    try:
        observed_errors: list[str] = []
        for _ in range(3):
            try:
                await embedder.embed_batch(["a"])
            except httpx.HTTPStatusError as exc:
                observed_errors.append(str(exc.response.status_code))
        opened = embedder.circuit_state == CIRCUIT_OPEN and len(observed_errors) == 2
        now += 61.0
        recovered = await embedder.embed_batch(["a"])
        ok = opened and recovered == [[0.25] * 4] and embedder.circuit_state == CIRCUIT_CLOSED
        return TestResult(
            "T-W-2",
            "wrapper",
            "circuit opens then recovers after 60s cooldown",
            "half-open probe succeeds and circuit closes",
            "PASS" if ok else "FAIL",
            Evidence(note=f"opened={opened}, final_state={embedder.circuit_state}, recovered={recovered}"),
        )
    finally:
        await client.aclose()


async def _wrapper_5xx_returns_zero_and_warning() -> TestResult:
    transport = SequenceTransport([500], dimensions=4)
    client = httpx.AsyncClient(transport=transport)
    embedder = Embedder(
        endpoint=DEFAULT_ENDPOINT,
        dimensions=4,
        max_attempts=1,
        circuit_failure_threshold=1,
        graceful_degradation_enabled=True,
        client=client,
        sleep=lambda _seconds: asyncio.sleep(0),
    )
    try:
        embeddings = await embedder.embed_batch(["a"])
        metrics = embedder.export_metrics()
        ok = embeddings == [[0.0] * 4] and metrics["error_count"] == 1
        return TestResult(
            "T-W-3",
            "wrapper",
            "5xx response returns zero embedding and records warning path",
            "zero embedding plus error_count=1",
            "PASS" if ok else "FAIL",
            Evidence(note=f"embeddings={embeddings}, metrics={metrics}"),
        )
    finally:
        await client.aclose()


async def run_wrapper_tests() -> list[TestResult]:
    return [
        await _wrapper_three_failures_then_zero(),
        await _wrapper_circuit_recovers_after_cooldown(),
        await _wrapper_5xx_returns_zero_and_warning(),
    ]


def run_env_override_tests() -> list[TestResult]:
    results: list[TestResult] = []
    with temporary_env({"FD_API_KEY": "test-fd-api-key-12345"}):
        headers = get_auth_headers()
        observed = "Authorization" in headers and headers["Authorization"].startswith("Bearer ")
        results.append(
            TestResult(
                "T-ENV-1",
                "env",
                "FD_API_KEY supplies bearer auth header",
                "Authorization bearer header is set from FD_API_KEY without logging the key",
                "PASS" if observed else "FAIL",
                Evidence(note="authorization_header_present=True" if observed else "authorization_header_present=False"),
            )
        )
    with temporary_env({"TEI_URL": "http://fd-test.internal:18000", "FD_EMBEDDINGS_ENDPOINT": None, "FD_EMBEDDINGS_ENDPOINT_BASE": None}):
        observed_base = get_base_url()
        observed_endpoint = get_endpoint()
        expected_base = "http://fd-test.internal:18000"
        expected_endpoint = "http://fd-test.internal:18000/v1/embeddings"
        ok = observed_base == expected_base and observed_endpoint == expected_endpoint
        results.append(
            TestResult(
                "T-ENV-2",
                "env",
                "TEI_URL overrides fd base URL and derived endpoint",
                "TEI_URL base derives /v1/embeddings endpoint",
                "PASS" if ok else "FAIL",
                Evidence(note=f"base_host=fd-test.internal, endpoint_suffix=/v1/embeddings, ok={ok}"),
            )
        )
    with temporary_env({"MODEL_ID": "test/model-v2", "FD_MODEL_NAME": None}):
        observed = get_model_id()
        results.append(
            TestResult(
                "T-ENV-3",
                "env",
                "MODEL_ID overrides advertised model id",
                "get_model_id() returns test/model-v2",
                "PASS" if observed == "test/model-v2" else "FAIL",
                Evidence(note=f"observed_model_id={observed}"),
            )
        )
    with temporary_env({"REDIS_HOST": "fd-cache.internal", "REDIS_PORT": "6380"}):
        observed_host = get_redis_host()
        observed_port = get_redis_port()
        ok = observed_host == "fd-cache.internal" and observed_port == "6380"
        results.append(
            TestResult(
                "T-ENV-4",
                "env",
                "REDIS_HOST and REDIS_PORT override cache target",
                "Redis cache target env resolves to fd-cache.internal:6380",
                "PASS" if ok else "FAIL",
                Evidence(note=f"observed_host={observed_host}, observed_port={observed_port}"),
            )
        )
    return results


def build_tests() -> list[str]:
    """Return the canonical 52 test IDs for unit-test validation."""
    ids = [case.test_id for case in build_http_cases()]
    ids.extend(["T-P-4", "T-P-5", "T-W-1", "T-W-2", "T-W-3", "T-ENV-1", "T-ENV-2", "T-ENV-3", "T-ENV-4"])
    return ids


def run_contract_tests() -> list[TestResult]:
    timeout = httpx.Timeout(float(os.environ.get("FD_CONTRACT_TIMEOUT_SECONDS", "5.0")))
    results: list[TestResult] = []
    with httpx.Client(timeout=timeout) as client:
        for case in build_http_cases():
            results.append(_run_http_case(client, case))
        results.extend(_run_performance_sequence(client))
    results.extend(asyncio.run(run_wrapper_tests()))
    results.extend(run_env_override_tests())
    return results


REQUIREMENTS: dict[str, dict[str, str]] = {
    "R-P0-1": {"priority": "P0", "description": "Input length validation"},
    "R-P0-2": {"priority": "P0", "description": "Batch size validation"},
    "R-P0-3": {"priority": "P0", "description": "503 for model not loaded or overloaded"},
    "R-P0-4": {"priority": "P0", "description": "Warmup and readiness"},
    "R-P0-5": {"priority": "P0", "description": "Graceful shutdown"},
    "R-P0-6": {"priority": "P0", "description": "Performance baseline"},
    "R-P0-7": {"priority": "P0", "description": "GET /version"},
    "R-P0-8": {"priority": "P0", "description": "GET /info or GET /v1/models"},
    "R-P0-9": {"priority": "P0", "description": "GET /metrics Prometheus text"},
    "R-P0-10": {"priority": "P0", "description": "GET /v1/healthcheck alias"},
    "R-P0-11": {"priority": "P0", "description": "X-Request-Id"},
    "R-P0-12": {"priority": "P0", "description": "Server: fd/<version>"},
    "R-P0-13": {"priority": "P0", "description": "X-Model-Id header"},
    "R-P0-14": {"priority": "P0", "description": "X-Dimensions header"},
    "R-P0-15": {"priority": "P0", "description": "X-Cache header"},
    "R-P0-16": {"priority": "P0", "description": "Retry-After on 429/503"},
    "R-P0-17": {"priority": "P0", "description": "Connection keep-alive"},
    "R-P0-18": {"priority": "P0", "description": "OpenAI-style error envelope"},
    "R-P0-19": {"priority": "P0", "description": "HTTP status code mapping"},
    "R-P1-1": {"priority": "P1", "description": "GET /health deep check"},
    "R-P1-2": {"priority": "P1", "description": "GET /warmup status"},
    "R-P1-3": {"priority": "P1", "description": "POST /warmup trigger"},
    "R-P1-4": {"priority": "P1", "description": "Cache with X-Cache header"},
    "R-P1-5": {"priority": "P1", "description": "encoding_format option"},
    "R-P1-6": {"priority": "P1", "description": "user field"},
    "R-P1-7": {"priority": "P1", "description": "priority option"},
    "R-P1-8": {"priority": "P1", "description": "API key auth"},
    "R-P1-9": {"priority": "P1", "description": "CORS headers"},
    "R-P2-1": {"priority": "P2", "description": "OpenAPI schema and Swagger UI"},
    "R-P2-2": {"priority": "P2", "description": "ETag and Cache-Control"},
    "R-P2-3": {"priority": "P2", "description": "/v1/batch endpoint"},
    "R-P2-4": {"priority": "P2", "description": "Streaming response"},
    "R-P2-5": {"priority": "P2", "description": "Rate limiting"},
    "R-P2-6": {"priority": "P2", "description": "/v1/traces debugging"},
}


EXTRA_REQUIREMENT_EVIDENCE: dict[str, str] = {
    "R-P0-10": "No section 5 case exists for /v1/healthcheck; fd v1 observed missing in fd-v2.md section 1.3.",
    "R-P1-2": "No section 5 case exists for GET /warmup; feature is missing from fd v1.",
    "R-P1-3": "No section 5 case exists for POST /warmup; feature is missing from fd v1.",
    "R-P1-6": "No section 5 case exists for user field; feature is not evidenced.",
    "R-P1-9": "No section 5 case exists for CORS; feature is not evidenced.",
    "R-P2-3": "No section 5 case exists for /v1/batch; fd-v2.md section 1.3 says it is absent.",
    "R-P2-4": "No section 5 case exists for streaming response; feature is not evidenced.",
    "R-P2-6": "No section 5 case exists for /v1/traces; feature is not evidenced.",
}


def requirement_statuses(results: list[TestResult]) -> dict[str, dict[str, Any]]:
    by_req: dict[str, list[TestResult]] = defaultdict(list)
    for result in results:
        for requirement in result.requirements:
            by_req[requirement].append(result)

    statuses: dict[str, dict[str, Any]] = {}
    for req_id, meta in REQUIREMENTS.items():
        req_results = by_req.get(req_id, [])
        if any(result.status == "PASS" for result in req_results):
            status = "MET"
        elif any(result.status == "SKIP" for result in req_results):
            status = "UNKNOWN"
        elif req_results and all(result.evidence.status_code is None for result in req_results):
            status = "UNKNOWN"
        elif any(
            result.status == "FAIL"
            and result.evidence.status_code is not None
            and result.evidence.status_code != 404
            for result in req_results
        ):
            status = "PARTIAL"
        else:
            status = "UNKNOWN"
        evidence = "; ".join(
            f"{result.test_id}:{result.status}:{result.evidence.compact()}" for result in req_results[:3]
        )
        if not evidence:
            evidence = EXTRA_REQUIREMENT_EVIDENCE.get(req_id, "no mapped evidence")
        statuses[req_id] = {
            "priority": meta["priority"],
            "description": meta["description"],
            "status": status,
            "tests": [result.test_id for result in req_results],
            "evidence": evidence,
        }
    return statuses


def summarize(results: list[TestResult]) -> dict[str, Any]:
    total = len(results)
    counts = Counter(result.status for result in results)
    by_category: dict[str, dict[str, int]] = {}
    for category in sorted({result.category for result in results}):
        category_results = [result for result in results if result.category == category]
        category_counts = Counter(result.status for result in category_results)
        by_category[category] = {
            "total": len(category_results),
            "passed": category_counts.get("PASS", 0),
            "failed": category_counts.get("FAIL", 0),
            "skipped": category_counts.get("SKIP", 0),
        }
    reqs = requirement_statuses(results)
    requirement_summary: dict[str, dict[str, int]] = {}
    for priority in ("P0", "P1", "P2"):
        priority_reqs = [item for item in reqs.values() if item["priority"] == priority]
        requirement_summary[priority] = {
            "total": len(priority_reqs),
            "met": sum(1 for item in priority_reqs if item["status"] == "MET"),
            "partial": sum(1 for item in priority_reqs if item["status"] == "PARTIAL"),
            "unknown": sum(1 for item in priority_reqs if item["status"] == "UNKNOWN"),
        }
    return {
        "total": total,
        "passed": counts.get("PASS", 0),
        "failed": counts.get("FAIL", 0),
        "skipped": counts.get("SKIP", 0),
        "by_category": by_category,
        "requirements": reqs,
        "requirement_summary": requirement_summary,
    }


def _md_escape(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def _load_prior_report_statuses(artifact_dir: Path) -> dict[str, str]:
    prior_path = artifact_dir / PRIOR_REPORT_MD
    if not prior_path.exists():
        return {}
    statuses: dict[str, str] = {}
    for line in prior_path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("| T-"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) >= 6:
            statuses[cells[0]] = cells[5]
    return statuses


def write_reports(results: list[TestResult], artifact_dir: Path = ARTIFACT_DIR) -> None:
    artifact_dir.mkdir(parents=True, exist_ok=True)
    summary = summarize(results)
    prior_statuses = _load_prior_report_statuses(artifact_dir)
    results_payload = {
        "summary": summary,
        "v1_statuses_loaded": bool(prior_statuses),
        "tests": [
            {
                "test_id": result.test_id,
                "category": result.category,
                "description": result.description,
                "expected": result.expected,
                "observed": result.observed,
                "status": result.status,
                "v1_status": prior_statuses.get(result.test_id),
                "requirements": list(result.requirements),
            }
            for result in results
        ],
    }
    (artifact_dir / RESULTS_JSON).write_text(json.dumps(results_payload, indent=2, ensure_ascii=False), encoding="utf-8")

    lines = [
        "# M062 fd Contract Report v2",
        "",
        "Source contract: `/root/fd/docs/fd-v2.md` (the requested `/root/fd-v2.md` path was not present in this environment).",
        "Configuration: fd v2 env uses `FD_API_KEY`, `TEI_URL`, `MODEL_ID`, `REDIS_HOST`, and `REDIS_PORT`.",
        "",
        "## Summary",
        "",
        f"total={summary['total']}, passed={summary['passed']}, failed={summary['failed']}, skipped={summary['skipped']}",
        "",
        "## Per-category breakdown",
        "",
        "| Category | Total | Passed | Failed | Skipped | Pass rate |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for category, data in summary["by_category"].items():
        pass_rate = (data["passed"] / data["total"] * 100) if data["total"] else 0.0
        lines.append(
            f"| {category} | {data['total']} | {data['passed']} | {data['failed']} | {data['skipped']} | {pass_rate:.1f}% |"
        )
    lines.extend([
        "",
        "## Per-test detail",
        "",
        "| Test ID | Category | Description | Expected | Observed | Status | Evidence |",
        "|---|---|---|---|---|---|---|",
    ])
    for result in results:
        lines.append(
            "| "
            + " | ".join(
                [
                    result.test_id,
                    result.category,
                    _md_escape(result.description),
                    _md_escape(result.expected),
                    _md_escape(result.observed),
                    result.status,
                    _md_escape(result.evidence.compact()),
                ]
            )
            + " |"
        )

    current_by_id = {result.test_id: result for result in results}
    improved = [
        result for result in results
        if result.status == "PASS" and prior_statuses.get(result.test_id) not in (None, "PASS")
    ]
    regressions = [
        result for result in results
        if result.status != "PASS" and prior_statuses.get(result.test_id) == "PASS"
    ]
    unchanged_pass = [
        result for result in results
        if result.status == "PASS" and prior_statuses.get(result.test_id) == "PASS"
    ]
    lines.extend([
        "",
        "## v1 -> v2 comparison",
        "",
        f"Prior v1 statuses loaded: {'yes' if prior_statuses else 'no'}.",
        f"Now passing after v1 failure or skip: {len(improved)}.",
        f"Still passing from v1: {len(unchanged_pass)}.",
        f"Regressed from v1 PASS: {len(regressions)}.",
        "",
        "### Tests now passing",
        "",
    ])
    if improved:
        for result in improved:
            lines.append(f"- **{result.test_id}** — v1={prior_statuses.get(result.test_id)}, v2=PASS; {_md_escape(result.description)}")
    else:
        lines.append("- No v1 failed or skipped test passed in this v2 run; if fd v2 is not deployed this remains UNKNOWN rather than a contract regression.")
    lines.extend(["", "### Regressions from v1 PASS", ""])
    if regressions:
        for result in regressions:
            lines.append(f"- **{result.test_id}** — v1=PASS, v2={result.status}; {_md_escape(result.evidence.compact())}")
    else:
        lines.append("- None observed.")

    lines.extend(["", "## Gaps prioritized", ""])
    reqs = summary["requirements"]
    for priority in ("P0", "P1", "P2"):
        lines.append(f"### {priority}")
        lines.append("")
        gaps = [
            (req_id, item) for req_id, item in reqs.items()
            if item["priority"] == priority and item["status"] != "MET"
        ]
        if not gaps:
            lines.append("- None; all mapped requirements are MET.")
        for req_id, item in gaps:
            lines.append(f"- **{req_id} {item['status']}** — {item['description']}: {_md_escape(item['evidence'])}")
        lines.append("")
    (artifact_dir / REPORT_MD).write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")

    gap_lines = [
        "# M062 fd Actual vs Required v2",
        "",
        "Status meanings: MET = at least one mapped contract test passed; PARTIAL = endpoint responded but header/body/status contract is incomplete; UNKNOWN = endpoint unavailable, skipped fixture, or no live evidence.",
        "",
        "## Expected requirement coverage",
        "",
        "- P0: 19/19 requirements represented in the contract matrix.",
        "- P1: 9/9 requirements represented in the contract matrix.",
        "- P2: 6/6 requirements represented in the contract matrix.",
        "",
        "## Summary",
        "",
        "| Priority | Met | Partial | Unknown | Total |",
        "|---|---:|---:|---:|---:|",
    ]
    for priority in ("P0", "P1", "P2"):
        data = summary["requirement_summary"][priority]
        gap_lines.append(f"| {priority} | {data['met']} | {data['partial']} | {data['unknown']} | {data['total']} |")
    gap_lines.extend([
        "",
        "## Per-requirement detail",
        "",
        "| Requirement | Priority | Description | Status | Tests | Evidence |",
        "|---|---|---|---|---|---|",
    ])
    for req_id, item in reqs.items():
        gap_lines.append(
            f"| {req_id} | {item['priority']} | {_md_escape(item['description'])} | {item['status']} | {', '.join(item['tests']) or 'n/a'} | {_md_escape(item['evidence'])} |"
        )
    (artifact_dir / GAP_MD).write_text("\n".join(gap_lines).rstrip() + "\n", encoding="utf-8")


def print_results(results: list[TestResult]) -> None:
    for result in results:
        if result.status == "PASS":
            print(f"[PASS] {result.test_id} ({result.evidence.compact()})")
        elif result.status == "SKIP":
            print(f"[SKIP] {result.test_id} ({result.evidence.note})")
        else:
            expected_status = ""
            if result.evidence.status_code is not None:
                expected_status = f"status={result.evidence.status_code}"
            print(f"[FAIL] {result.test_id} ({expected_status}; expected={result.expected}; {result.evidence.compact()})")
    summary = summarize(results)
    print(f"Summary: total={summary['total']}, passed={summary['passed']}, failed={summary['failed']}, skipped={summary['skipped']}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Run fd v2 contract tests and write M062 S03 reports.")
    parser.add_argument("--no-write-report", action="store_true", help="Run tests without writing markdown/json artifacts.")
    args = parser.parse_args()
    results = run_contract_tests()
    print_results(results)
    if not args.no_write_report:
        write_reports(results)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
