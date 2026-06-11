#!/usr/bin/env python3
"""Validate the local fd embedding service for M057 S01.

This script intentionally uses only stdlib HTTP clients and the loopback
address 127.0.0.1. It writes a machine-readable report with per-test verdicts,
latency statistics, cache timing, and explicit safety defaults.
"""

from __future__ import annotations

import argparse
import json
import statistics
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BASE_URL = "http://127.0.0.1:8000"
EXPECTED_MODEL = "deepvk/USER-bge-m3"
DEFAULT_OUTPUT = ROOT / "artifacts" / "m057-fd-marker" / "fd-validation.json"

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}


@dataclass(frozen=True)
class HttpResult:
    status_code: int
    payload: dict[str, Any]
    latency_ms: float


class FdClient:
    def __init__(self, base_url: str = DEFAULT_BASE_URL, timeout_seconds: float = 30.0) -> None:
        self.base_url = base_url.rstrip("/")
        self.timeout_seconds = timeout_seconds

    def get_health(self) -> HttpResult:
        return self._request("GET", "/health")

    def embed(self, text_or_texts: str | list[str], *, dimensions: int | None = None) -> HttpResult:
        payload: dict[str, Any] = {"input": text_or_texts, "model": EXPECTED_MODEL}
        if dimensions is not None:
            payload["dimensions"] = dimensions
        return self._request("POST", "/v1/embeddings", payload)

    def _request(self, method: str, path: str, payload: dict[str, Any] | None = None) -> HttpResult:
        data = json.dumps(payload).encode("utf-8") if payload is not None else None
        request = urllib.request.Request(
            f"{self.base_url}{path}",
            data=data,
            method=method,
            headers={"Content-Type": "application/json"},
        )
        started = time.perf_counter()
        try:
            with urllib.request.urlopen(request, timeout=self.timeout_seconds) as response:
                raw = response.read().decode("utf-8")
                decoded = json.loads(raw) if raw else {}
                return HttpResult(response.status, decoded, (time.perf_counter() - started) * 1000)
        except urllib.error.HTTPError as exc:
            raw = exc.read().decode("utf-8", errors="replace")
            try:
                decoded = json.loads(raw) if raw else {"error": raw}
            except json.JSONDecodeError:
                decoded = {"error": raw}
            return HttpResult(exc.code, decoded, (time.perf_counter() - started) * 1000)


def percentile(values: list[float], percentile_value: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = min(len(ordered) - 1, max(0, round((percentile_value / 100.0) * (len(ordered) - 1))))
    return ordered[index]


def verdict(name: str, passed: bool, *, details: dict[str, Any]) -> dict[str, Any]:
    return {"name": name, "passed": passed, "details": details}


def run_validation(base_url: str = DEFAULT_BASE_URL, latency_calls: int = 100) -> dict[str, Any]:
    client = FdClient(base_url)
    tests: list[dict[str, Any]] = []
    latencies: list[float] = []

    health = client.get_health()
    probe = client.embed("fd health model probe")
    health_passed = (
        health.status_code == 200
        and health.payload.get("status") == "ok"
        and probe.status_code == 200
        and probe.payload.get("model") == EXPECTED_MODEL
    )
    tests.append(
        verdict(
            "test_health",
            health_passed,
            details={
                "health_status_code": health.status_code,
                "health_status": health.payload.get("status"),
                "model_probe_status_code": probe.status_code,
                "model": probe.payload.get("model"),
                "latency_ms": round(health.latency_ms + probe.latency_ms, 3),
            },
        )
    )

    single = client.embed("M057 fd single embedding validation")
    single_data = single.payload.get("data", []) if isinstance(single.payload, dict) else []
    single_embedding = single_data[0].get("embedding") if single_data else None
    single_dim = len(single_embedding) if isinstance(single_embedding, list) else 0
    tests.append(
        verdict(
            "test_single_embedding_1024d",
            single.status_code == 200 and single_dim == 1024,
            details={"status_code": single.status_code, "dimension": single_dim, "latency_ms": round(single.latency_ms, 3)},
        )
    )

    batch_texts = [f"M057 fd batch validation item {index}" for index in range(32)]
    batch = client.embed(batch_texts)
    batch_data = batch.payload.get("data", []) if isinstance(batch.payload, dict) else []
    batch_dims = [len(item.get("embedding", [])) for item in batch_data if isinstance(item, dict)]
    tests.append(
        verdict(
            "test_batch_embedding",
            batch.status_code == 200 and len(batch_data) == 32 and all(dim == 1024 for dim in batch_dims),
            details={
                "status_code": batch.status_code,
                "returned": len(batch_data),
                "dimensions": sorted(set(batch_dims)),
                "latency_ms": round(batch.latency_ms, 3),
                "error": batch.payload.get("error"),
            },
        )
    )

    cache_text = f"M057 fd cache validation {time.time_ns()}"
    first = client.embed(cache_text)
    second = client.embed(cache_text)
    tests.append(
        verdict(
            "test_cache_behavior",
            first.status_code == 200 and second.status_code == 200 and second.latency_ms < first.latency_ms,
            details={
                "first_latency_ms": round(first.latency_ms, 3),
                "second_latency_ms": round(second.latency_ms, 3),
                "speedup_ratio": round(first.latency_ms / second.latency_ms, 3) if second.latency_ms else None,
            },
        )
    )

    for index in range(latency_calls):
        response = client.embed(f"M057 fd p95 latency sample {index}")
        latencies.append(response.latency_ms)
    p50 = percentile(latencies, 50)
    p95 = percentile(latencies, 95)
    tests.append(
        verdict(
            "test_latency_p95",
            len(latencies) == latency_calls and p95 < 500,
            details={
                "call_count": latency_calls,
                "p50_ms": round(p50, 3),
                "p95_ms": round(p95, 3),
                "max_ms": round(max(latencies), 3) if latencies else 0.0,
                "target_p95_ms": 500,
            },
        )
    )

    dim1024 = client.embed("M057 fd dimension 1024 validation", dimensions=1024)
    dim512 = client.embed("M057 fd dimension 512 validation", dimensions=512)
    dim1024_data = dim1024.payload.get("data", []) if isinstance(dim1024.payload, dict) else []
    dim512_data = dim512.payload.get("data", []) if isinstance(dim512.payload, dict) else []
    dim1024_len = len(dim1024_data[0].get("embedding", [])) if dim1024_data else 0
    dim512_len = len(dim512_data[0].get("embedding", [])) if dim512_data else 0
    tests.append(
        verdict(
            "test_dimensions_1024_512",
            dim1024.status_code == 200 and dim512.status_code == 200 and dim1024_len == 1024 and dim512_len == 512,
            details={
                "dimension_1024_status_code": dim1024.status_code,
                "dimension_1024_length": dim1024_len,
                "dimension_512_status_code": dim512.status_code,
                "dimension_512_length": dim512_len,
            },
        )
    )

    invalid_cases = {
        "empty_string": client.embed(""),
        "too_long": client.embed("x" * 200_000),
        "invalid_dimension": client.embed("M057 invalid dimension", dimensions=768),
    }
    tests.append(
        verdict(
            "test_error_handling",
            all(result.status_code >= 400 and result.payload.get("error") for result in invalid_cases.values()),
            details={
                name: {"status_code": result.status_code, "error": result.payload.get("error")}
                for name, result in invalid_cases.items()
            },
        )
    )

    passed = sum(1 for test in tests if test["passed"])
    total = len(tests)
    return {
        "schema_version": "m057-fd-marker.fd-validation.v1",
        "base_url": base_url,
        "expected_model": EXPECTED_MODEL,
        "safety_defaults": SAFETY_DEFAULTS,
        "summary": {
            "passed": passed,
            "failed": total - passed,
            "total": total,
            "all_passed": passed == total,
            "latency_p50_ms": round(p50, 3) if latencies else None,
            "latency_p95_ms": round(p95, 3) if latencies else None,
            "cache_hit_rate": 1.0 if second.latency_ms < first.latency_ms else 0.0,
        },
        "tests": tests,
    }


def write_report(report: dict[str, Any], output_path: Path = DEFAULT_OUTPUT) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--latency-calls", type=int, default=100)
    args = parser.parse_args()

    report = run_validation(args.base_url, args.latency_calls)
    write_report(report, args.output)
    print(json.dumps(report["summary"], sort_keys=True))
    return 0 if report["summary"]["all_passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
