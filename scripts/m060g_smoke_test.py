#!/usr/bin/env python3
"""M060-gakmo0 S01 MiniMax judge model smoke test.

Diagnostic-only QA smoke test for MiniMax-M2.7-highspeed text and MiniMax-M3
text/multimodal calls. This script does not write graph data, does not import
production facts, and does not promote model output to fact.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import json
import os
import time
import urllib.error
import urllib.request
import zlib
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import yaml

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MODELS_YAML = ROOT / "models.yaml"
DEFAULT_OUTPUT_PATH = ROOT / "artifacts" / "m060g-judge" / "smoke-test.json"
ANTHROPIC_VERSION = "2023-06-01"
PROMPT = 'Reply with JSON: {"ok": true}'

SAFETY_DEFAULTS: dict[str, bool] = {
    "external_network_authorized": False,
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "llm_calls_authorized": False,
}

DIAGNOSTIC_LLM_CALLS_OVERRIDE: dict[str, Any] = {
    "llm_calls_authorized": True,
    "scope": "M060-gakmo0 S01 MiniMax figure QA diagnostic smoke test only",
    "reason": "Live calls are authorized only for this QA diagnostic; graph writes are not authorized, production import is not authorized, and fact promotion is not authorized.",
}

TEXT_TESTS: tuple[tuple[str, str], ...] = (
    ("m27_text", "figure-qa-judge-fast"),
    ("m3_text", "figure-qa-judge-quality"),
)
IMAGE_TEST = ("m3_multimodal_image", "figure-qa-judge-quality")


@dataclass(frozen=True)
class ModelBinding:
    """Resolved model binding from models.yaml."""

    binding_id: str
    model_id: str
    endpoint: str
    model_name: str


def load_dotenv(path: Path = ROOT / ".env") -> None:
    """Load simple KEY=VALUE pairs from .env without overriding process env."""

    if not path.exists():
        return
    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip().strip('"').strip("'")
        if key and key not in os.environ:
            os.environ[key] = value


def load_bindings(models_yaml: Path = DEFAULT_MODELS_YAML) -> dict[str, ModelBinding]:
    payload = yaml.safe_load(models_yaml.read_text())
    models = {model["id"]: model for model in payload["models"]}
    resolved: dict[str, ModelBinding] = {}
    for binding in payload["bindings"]:
        model = models[binding["model_id"]]
        resolved[binding["binding_id"]] = ModelBinding(
            binding_id=binding["binding_id"],
            model_id=binding["model_id"],
            endpoint=str(model["endpoint"]),
            model_name=str(model["model_name"]),
        )
    return resolved


def make_png_base64(width: int = 64, height: int = 64) -> tuple[str, int]:
    """Create a deterministic valid PNG around 10-13KB when base64 encoded."""

    def chunk(kind: bytes, data: bytes) -> bytes:
        crc = binascii.crc32(kind + data) & 0xFFFFFFFF
        return len(data).to_bytes(4, "big") + kind + data + crc.to_bytes(4, "big")

    rows = bytearray()
    for y in range(height):
        rows.append(0)  # no filter
        for x in range(width):
            rows.extend(((x * 4) % 256, (y * 4) % 256, ((x + y) * 2) % 256))
    ihdr = width.to_bytes(4, "big") + height.to_bytes(4, "big") + bytes([8, 2, 0, 0, 0])
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(bytes(rows), level=0))
        + chunk(b"IEND", b"")
    )
    return base64.b64encode(png).decode("ascii"), len(png)


def build_text_body(model_name: str) -> dict[str, Any]:
    return {
        "model": model_name,
        "max_tokens": 256,
        "temperature": 0.1,
        "messages": [{"role": "user", "content": PROMPT}],
    }


def build_image_body(model_name: str) -> tuple[dict[str, Any], int]:
    encoded_png, png_bytes = make_png_base64()
    return {
        "model": model_name,
        "max_tokens": 256,
        "temperature": 0.1,
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": PROMPT},
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": encoded_png,
                        },
                    },
                ],
            }
        ],
    }, png_bytes


def extract_text(payload: dict[str, Any]) -> str:
    parts: list[str] = []
    for block in payload.get("content", []):
        if isinstance(block, dict) and block.get("type") == "text":
            parts.append(str(block.get("text", "")))
    return "\n".join(part for part in parts if part).strip()


def parse_response_json(text: str) -> dict[str, Any] | None:
    if not text:
        return None
    stripped = text.strip()
    if stripped.startswith("```"):
        stripped = stripped.strip("`").strip()
        if stripped.startswith("json"):
            stripped = stripped[4:].strip()
    try:
        parsed = json.loads(stripped)
    except json.JSONDecodeError:
        return None
    return parsed if isinstance(parsed, dict) else None


def estimate_cost_usd(_model_name: str, _usage: dict[str, Any] | None) -> float | None:
    """Return None because MiniMax responses do not include billable price data."""

    return None


def call_minimax(
    binding: ModelBinding, body: dict[str, Any], *, timeout_seconds: int
) -> dict[str, Any]:
    api_key = os.environ.get("ANTHROPIC_API_KEY") or os.environ.get("MINIMAX_API_KEY")
    if not api_key:
        return {
            "status": "skipped",
            "status_code": None,
            "model_used": binding.model_name,
            "latency_ms": None,
            "response": None,
            "usage": None,
            "cost_estimate_usd": None,
            "cost_estimate_note": "Skipped because ANTHROPIC_API_KEY/MINIMAX_API_KEY is not set.",
        }

    request = urllib.request.Request(
        binding.endpoint,
        data=json.dumps(body).encode("utf-8"),
        headers={
            "Content-Type": "application/json",
            "x-api-key": api_key,
            "anthropic-version": ANTHROPIC_VERSION,
        },
        method="POST",
    )
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
            status_code = response.status
            payload = json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        latency_ms = round((time.perf_counter() - started) * 1000, 2)
        error_body = exc.read().decode("utf-8", errors="replace")[:1000]
        return {
            "status": "failed",
            "status_code": exc.code,
            "model_used": binding.model_name,
            "latency_ms": latency_ms,
            "response": None,
            "usage": None,
            "cost_estimate_usd": None,
            "cost_estimate_note": "Not measurable from failed response.",
            "error": error_body,
        }
    except (OSError, TimeoutError, json.JSONDecodeError) as exc:
        latency_ms = round((time.perf_counter() - started) * 1000, 2)
        return {
            "status": "failed",
            "status_code": None,
            "model_used": binding.model_name,
            "latency_ms": latency_ms,
            "response": None,
            "usage": None,
            "cost_estimate_usd": None,
            "cost_estimate_note": "Not measurable from failed response.",
            "error": f"{type(exc).__name__}: {exc}",
        }

    latency_ms = round((time.perf_counter() - started) * 1000, 2)
    response_text = extract_text(payload)
    usage = payload.get("usage") if isinstance(payload.get("usage"), dict) else None
    return {
        "status": "passed" if status_code == 200 and response_text else "failed",
        "status_code": status_code,
        "model_used": str(payload.get("model") or binding.model_name),
        "latency_ms": latency_ms,
        "response": response_text,
        "response_json": parse_response_json(response_text),
        "usage": usage,
        "cost_estimate_usd": estimate_cost_usd(binding.model_name, usage),
        "cost_estimate_note": "Usage tokens captured; cost is not measurable from MiniMax Anthropic-compatible response without an external pricing table.",
    }


def percentile(values: list[float], pct: float) -> float | None:
    if not values:
        return None
    if len(values) == 1:
        return round(values[0], 2)
    ordered = sorted(values)
    rank = (len(ordered) - 1) * pct
    low = int(rank)
    high = min(low + 1, len(ordered) - 1)
    weight = rank - low
    return round(ordered[low] * (1 - weight) + ordered[high] * weight, 2)


def summarize_latency(results: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    grouped: dict[str, list[float]] = {}
    for result in results:
        if result.get("status") != "passed" or result.get("latency_ms") is None:
            continue
        grouped.setdefault(str(result["model_used"]), []).append(float(result["latency_ms"]))
    return {
        model: {
            "count": len(values),
            "p50_ms": percentile(values, 0.50),
            "p95_ms": percentile(values, 0.95),
        }
        for model, values in grouped.items()
    }


def run_smoke(
    *,
    output_path: Path = DEFAULT_OUTPUT_PATH,
    models_yaml: Path = DEFAULT_MODELS_YAML,
    timeout_seconds: int = 60,
    live: bool = True,
) -> dict[str, Any]:
    load_dotenv()
    bindings = load_bindings(models_yaml)
    results: list[dict[str, Any]] = []

    for test_id, binding_id in TEXT_TESTS:
        binding = bindings[binding_id]
        body = build_text_body(binding.model_name)
        result = (
            call_minimax(binding, body, timeout_seconds=timeout_seconds)
            if live
            else {
                "status": "skipped",
                "status_code": None,
                "model_used": binding.model_name,
                "latency_ms": None,
                "response": None,
                "usage": None,
                "cost_estimate_usd": None,
                "cost_estimate_note": "Skipped because live calls are disabled.",
            }
        )
        result.update({"test_id": test_id, "binding_id": binding_id, "model_id": binding.model_id})
        results.append(result)

    image_test_id, image_binding_id = IMAGE_TEST
    image_binding = bindings[image_binding_id]
    image_body, png_bytes = build_image_body(image_binding.model_name)
    image_result = (
        call_minimax(image_binding, image_body, timeout_seconds=timeout_seconds)
        if live
        else {
            "status": "skipped",
            "status_code": None,
            "model_used": image_binding.model_name,
            "latency_ms": None,
            "response": None,
            "usage": None,
            "cost_estimate_usd": None,
            "cost_estimate_note": "Skipped because live calls are disabled.",
        }
    )
    image_result.update(
        {
            "test_id": image_test_id,
            "binding_id": image_binding_id,
            "model_id": image_binding.model_id,
            "image_media_type": "image/png",
            "image_bytes": png_bytes,
        }
    )
    results.append(image_result)

    report = {
        "schema_version": "m060g-smoke-v0.1",
        "generated_at": datetime.now(UTC).isoformat(),
        "safety_defaults": dict(SAFETY_DEFAULTS),
        "diagnostic_llm_calls_override": dict(DIAGNOSTIC_LLM_CALLS_OVERRIDE),
        "models_yaml": str(models_yaml),
        "results": results,
        "latency_summary": summarize_latency(results),
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return report


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--models-yaml", type=Path, default=DEFAULT_MODELS_YAML)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT_PATH)
    parser.add_argument("--timeout-seconds", type=int, default=60)
    parser.add_argument(
        "--no-live", action="store_true", help="Write a skipped report without network calls."
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    report = run_smoke(
        output_path=args.output,
        models_yaml=args.models_yaml,
        timeout_seconds=args.timeout_seconds,
        live=not args.no_live,
    )
    failed = [result for result in report["results"] if result["status"] == "failed"]
    print(
        json.dumps(
            {"output": str(args.output), "failed": len(failed), "results": report["results"]},
            indent=2,
        )
    )
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
