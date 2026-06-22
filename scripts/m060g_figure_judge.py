#!/usr/bin/env python3
"""M060-gakmo0 S02 figure QA judge pilot.

Diagnostic-only pilot for 30 M058 figures: 15 data plots and 15 schema/diagram
figures. The script calls MiniMax-M2.7-highspeed as a text-only judge and
MiniMax-M3 as a multimodal judge, then emits per-figure and aggregate comparison
artifacts.

Safety posture: graph writes are not authorized, production import is not
authorized, fact promotion is not authorized, and these model scores are only a
QA diagnostic signal. LLM calls are enabled by a scoped diagnostic-only override
for this M060-gakmo0 S02 pilot.
"""

from __future__ import annotations

import argparse
import base64
import concurrent.futures
import json
import mimetypes
import os
import statistics
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import yaml

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MODELS_YAML = ROOT / "models.yaml"
DEFAULT_FIGURES_DIR = ROOT / "artifacts" / "m058-plotextractor" / "per-pdf"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m060g-judge"
DEFAULT_PAGE_CONTEXT_ROOT = ROOT / "artifacts" / "m056-bfs-graph"
ANTHROPIC_VERSION = "2023-06-01"
FAST_BINDING_ID = "figure-qa-judge-fast"
QUALITY_BINDING_ID = "figure-qa-judge-quality"
DIMENSIONS = ("caption_accuracy", "figure_completeness", "structural_fidelity")
REQUESTED_SCORE_KEYS = (*DIMENSIONS, "missing_elements")
NETWORK_HOST = "127.0.0.1"
MAX_IMAGE_BYTES = 4_500_000

SAFETY_DEFAULTS: dict[str, bool] = {
    "external_network_authorized": False,
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "llm_calls_authorized": False,
}

DIAGNOSTIC_LLM_CALLS_OVERRIDE: dict[str, Any] = {
    "llm_calls_authorized": True,
    "scope": "M060-gakmo0 S02 MiniMax figure QA diagnostic pilot only",
    "reason": (
        "Live LLM calls are enabled only for this QA diagnostic; graph writes are not "
        "authorized, production import is not authorized, and fact promotion is not authorized."
    ),
}


@dataclass(frozen=True)
class ModelBinding:
    binding_id: str
    model_id: str
    endpoint: str
    model_name: str


@dataclass(frozen=True)
class FigureCandidate:
    arxiv_id: str
    figure_id: str
    figure_idx: int
    category: str
    caption: str
    image_path: Path
    source_json: Path
    page_context: str
    label: str
    name: str

    @property
    def safe_id(self) -> str:
        return self.figure_id.replace("::", "__").replace("/", "_").replace(":", "_")


def load_dotenv(path: Path = ROOT / ".env") -> None:
    """Load simple KEY=VALUE pairs without overriding process env or echoing secrets."""

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
    bindings: dict[str, ModelBinding] = {}
    for binding in payload["bindings"]:
        model = models[binding["model_id"]]
        bindings[binding["binding_id"]] = ModelBinding(
            binding_id=binding["binding_id"],
            model_id=binding["model_id"],
            endpoint=str(model["endpoint"]),
            model_name=str(model["model_name"]),
        )
    return bindings


def classify_figure(raw: dict[str, Any]) -> str:
    text = " ".join(
        str(raw.get(key, "")) for key in ("label", "name", "caption_text", "extraction_source")
    ).lower()
    schema_terms = (
        "schema",
        "diagram",
        "architecture",
        "workflow",
        "pipeline",
        "framework",
        "tree",
        "dag",
        "prompt",
        "algorithm",
        "database",
        "infographic",
        "overview",
        "process",
        "system",
        "interface",
        "state",
    )
    plot_terms = (
        "plot",
        "chart",
        "curve",
        "bar",
        "score",
        "accuracy",
        "ratio",
        "latency",
        "reward",
        "ablation",
        "performance",
        "comparison",
        "histogram",
        "scatter",
        "map50",
        "fps",
        "time",
    )
    schema_hits = sum(term in text for term in schema_terms)
    plot_hits = sum(term in text for term in plot_terms)
    return "data_plot" if plot_hits > schema_hits else "schema_diagram"


def markdown_context_index(root: Path = DEFAULT_PAGE_CONTEXT_ROOT) -> dict[str, Path]:
    return {
        path.stem: path
        for path in sorted(root.glob("wave-*/opendataloader/markdown/*.md"))
        if path.is_file()
    }


def load_page_context(
    arxiv_id: str, markdown_index: dict[str, Path], *, max_chars: int = 1800
) -> str:
    path = markdown_index.get(arxiv_id)
    if path is None:
        return "No M056 OpenDataLoader markdown context was found for this arXiv id."
    text = path.read_text(errors="replace")
    compact = "\n".join(line.strip() for line in text.splitlines() if line.strip())
    return compact[:max_chars]


def load_figure_candidates(
    figures_dir: Path = DEFAULT_FIGURES_DIR,
    *,
    per_category: int = 15,
    page_context_root: Path = DEFAULT_PAGE_CONTEXT_ROOT,
) -> list[FigureCandidate]:
    markdown_index = markdown_context_index(page_context_root)
    selected: dict[str, list[FigureCandidate]] = {"data_plot": [], "schema_diagram": []}

    for source_json in sorted(figures_dir.glob("*.json")):
        payload = json.loads(source_json.read_text())
        arxiv_id = str(payload.get("arxiv_id") or source_json.stem)
        page_context = load_page_context(arxiv_id, markdown_index)
        for raw in payload.get("figures", []):
            caption = str(raw.get("caption_text") or "").strip()
            image_path_text = str(raw.get("image_path") or "").strip()
            if not caption or not image_path_text:
                continue
            image_path = Path(image_path_text)
            category = classify_figure(raw)
            if len(selected[category]) >= per_category:
                continue
            selected[category].append(
                FigureCandidate(
                    arxiv_id=arxiv_id,
                    figure_id=str(raw.get("figure_id") or f"{arxiv_id}::{raw.get('figure_idx')}"),
                    figure_idx=int(raw.get("figure_idx") or len(selected[category]) + 1),
                    category=category,
                    caption=caption,
                    image_path=image_path,
                    source_json=source_json,
                    page_context=page_context,
                    label=str(raw.get("label") or ""),
                    name=str(raw.get("name") or ""),
                )
            )
            if all(len(items) >= per_category for items in selected.values()):
                return selected["data_plot"] + selected["schema_diagram"]

    missing = {category: per_category - len(items) for category, items in selected.items()}
    if any(count > 0 for count in missing.values()):
        raise RuntimeError(f"Unable to select balanced 30-figure set from M058 output: {missing}")
    return selected["data_plot"] + selected["schema_diagram"]


def render_pdf_first_page_to_png(pdf_path: Path, output_path: Path) -> Path:
    """Render the first PDF page to PNG using available project/runtime libraries."""

    try:
        import fitz  # type: ignore[import-not-found]  # ty:ignore[unresolved-import]
    except Exception:
        fitz = None  # type: ignore[assignment]

    output_path.parent.mkdir(parents=True, exist_ok=True)
    if fitz is not None:
        doc = fitz.open(pdf_path)
        try:
            page = doc.load_page(0)
            pixmap = page.get_pixmap(matrix=fitz.Matrix(1.5, 1.5), alpha=False)
            pixmap.save(output_path)
        finally:
            doc.close()
        return output_path

    try:
        import pypdfium2 as pdfium  # type: ignore[import-not-found]
    except Exception as exc:  # pragma: no cover - depends on runtime libraries
        raise RuntimeError(
            "PDF rendering is disabled because neither PyMuPDF nor pypdfium2 is available"
        ) from exc

    pdf = pdfium.PdfDocument(str(pdf_path))
    page = pdf[0]
    bitmap = page.render(scale=1.5)
    image = bitmap.to_pil()
    image.save(output_path)
    pdf.close()
    return output_path


def ensure_png_image(figure: FigureCandidate, image_cache_dir: Path) -> Path:
    source = figure.image_path
    if not source.exists():
        raise FileNotFoundError(f"Figure image path is missing: {source}")
    suffix = source.suffix.lower()
    if suffix == ".png":
        return source
    output_path = image_cache_dir / f"{figure.safe_id}.png"
    if output_path.exists() and output_path.stat().st_size > 0:
        return output_path
    if suffix == ".pdf":
        return render_pdf_first_page_to_png(source, output_path)
    if suffix in {".jpg", ".jpeg", ".webp"}:
        from PIL import Image

        with Image.open(source) as image:
            image.convert("RGB").save(output_path)
        return output_path
    raise ValueError(f"Unsupported image format for multimodal judge: {source}")


def image_payload(image_path: Path) -> dict[str, Any]:
    data = image_path.read_bytes()
    if len(data) > MAX_IMAGE_BYTES:
        raise ValueError(
            f"Image payload is too large for diagnostic judge: {image_path} ({len(data)} bytes)"
        )
    media_type = mimetypes.guess_type(str(image_path))[0] or "image/png"
    if media_type not in {"image/png", "image/jpeg", "image/webp", "image/gif"}:
        media_type = "image/png"
    return {
        "type": "image",
        "source": {
            "type": "base64",
            "media_type": media_type,
            "data": base64.b64encode(data).decode("ascii"),
        },
    }


def build_prompt(figure: FigureCandidate) -> str:
    return f"""You are a diagnostic figure QA judge for an arXiv extraction pipeline.
Return only valid JSON with exactly these keys:
- caption_accuracy: number from 0 to 1
- figure_completeness: number from 0 to 1
- structural_fidelity: number from 0 to 1
- missing_elements: array of short strings

You must always return numeric scores. Do not return null. Do not refuse. If you are the text-only judge and cannot inspect pixels, estimate diagnostic scores from the caption, figure metadata, and page context, and list uncertainty or unavailable visual evidence in missing_elements.

Scoring guidance:
- caption_accuracy: how well the caption matches the visible or described figure.
- figure_completeness: whether the extracted figure appears complete, not cropped, and contains expected elements.
- structural_fidelity: whether axes, legends, nodes, arrows, labels, tables, or layout are preserved.
- missing_elements: list concrete missing/cropped/unclear elements, or [] if none are evident.

Safety: graph writes are not authorized; production import is not authorized; fact promotion is not authorized. Treat this as diagnostic QA only.

Figure metadata:
- figure_id: {figure.figure_id}
- arxiv_id: {figure.arxiv_id}
- category: {figure.category}
- label: {figure.label}
- name: {figure.name}

Caption:
{figure.caption}

Page context excerpt:
{figure.page_context}
""".strip()


def build_request_body(
    binding: ModelBinding, figure: FigureCandidate, *, image: Path | None
) -> dict[str, Any]:
    content: list[dict[str, Any]] = [{"type": "text", "text": build_prompt(figure)}]
    if image is not None:
        content.append(image_payload(image))
    return {
        "model": binding.model_name,
        "max_tokens": 4096,
        "temperature": 0,
        "messages": [{"role": "user", "content": content}],
    }


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
        if stripped.lower().startswith("json"):
            stripped = stripped[4:].strip()
    first = stripped.find("{")
    last = stripped.rfind("}")
    if first >= 0 and last > first:
        stripped = stripped[first : last + 1]
    try:
        parsed = json.loads(stripped)
    except json.JSONDecodeError:
        return None
    return parsed if isinstance(parsed, dict) else None


def validate_score_payload(payload: dict[str, Any] | None) -> dict[str, Any]:
    if payload is None:
        raise ValueError("response did not contain parseable JSON")
    normalized: dict[str, Any] = {}
    for key in DIMENSIONS:
        value = payload.get(key)
        if not isinstance(value, int | float) or isinstance(value, bool):
            raise ValueError(f"{key} is missing or not numeric")
        if not 0 <= float(value) <= 1:
            raise ValueError(f"{key} is outside 0..1")
        normalized[key] = round(float(value), 4)
    missing = payload.get("missing_elements")
    if not isinstance(missing, list) or not all(isinstance(item, str) for item in missing):
        raise ValueError("missing_elements is missing or not a list of strings")
    normalized["missing_elements"] = missing
    return normalized


def response_cost_note(_usage: dict[str, Any] | None) -> tuple[float | None, str]:
    return (
        None,
        "Usage tokens are captured when returned; cost is not measurable without an external MiniMax pricing table.",
    )


def resolve_messages_endpoint(binding: ModelBinding, base_url: str | None = None) -> str:
    """Resolve Anthropic-compatible Messages endpoint from registry + optional env base URL."""

    if not base_url:
        return binding.endpoint
    normalized = base_url.rstrip("/")
    if normalized.endswith("/v1/messages") or normalized.endswith("/messages"):
        return normalized
    if normalized.endswith("/anthropic"):
        return f"{normalized}/v1/messages"
    if normalized.endswith("/anthropic/v1"):
        return f"{normalized}/messages"
    return f"{normalized}/messages"


def call_anthropic_messages(
    binding: ModelBinding,
    body: dict[str, Any],
    *,
    timeout_seconds: int,
    max_retries: int,
    backoff_seconds: float,
) -> dict[str, Any]:
    auth = os.environ.get("ANTHROPIC_API_KEY") or os.environ.get("MINIMAX_API_KEY")
    endpoint = resolve_messages_endpoint(binding, os.environ.get("ANTHROPIC_BASE_URL"))
    if not auth:
        raise RuntimeError("ANTHROPIC_API_KEY is not set; live MiniMax judge is disabled")

    encoded = json.dumps(body).encode("utf-8")
    last_error: str | None = None
    for attempt in range(max_retries + 1):
        started = time.perf_counter()
        request = urllib.request.Request(
            endpoint,
            data=encoded,
            headers={
                "content-type": "application/json",
                "x-api-key": auth,
                "anthropic-version": ANTHROPIC_VERSION,
            },
            method="POST",
        )
        try:
            with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
                status_code = response.status
                payload = json.loads(response.read().decode("utf-8"))
                latency_ms = round((time.perf_counter() - started) * 1000, 2)
                return {
                    "status": "passed" if status_code == 200 else "failed",
                    "status_code": status_code,
                    "payload": payload,
                    "latency_ms": latency_ms,
                    "usage": payload.get("usage")
                    if isinstance(payload.get("usage"), dict)
                    else None,
                    "error": None,
                    "attempts": attempt + 1,
                }
        except urllib.error.HTTPError as exc:
            latency_ms = round((time.perf_counter() - started) * 1000, 2)
            body_text = exc.read().decode("utf-8", errors="replace")[:2000]
            retryable = exc.code in {408, 409, 425, 429, 500, 502, 503, 504}
            last_error = f"HTTPError {exc.code}: {body_text}"
            if not retryable or attempt >= max_retries:
                return {
                    "status": "failed",
                    "status_code": exc.code,
                    "payload": None,
                    "latency_ms": latency_ms,
                    "usage": None,
                    "error": last_error,
                    "attempts": attempt + 1,
                }
        except (OSError, TimeoutError, json.JSONDecodeError) as exc:
            latency_ms = round((time.perf_counter() - started) * 1000, 2)
            last_error = f"{type(exc).__name__}: {exc}"
            if attempt >= max_retries:
                return {
                    "status": "failed",
                    "status_code": None,
                    "payload": None,
                    "latency_ms": latency_ms,
                    "usage": None,
                    "error": last_error,
                    "attempts": attempt + 1,
                }
        time.sleep(backoff_seconds * (2**attempt))

    return {
        "status": "failed",
        "status_code": None,
        "payload": None,
        "latency_ms": 0,
        "usage": None,
        "error": last_error or "unknown retry failure",
        "attempts": max_retries + 1,
    }


def judge_one_model(
    figure: FigureCandidate,
    binding: ModelBinding,
    *,
    image: Path | None,
    timeout_seconds: int,
    max_retries: int,
    backoff_seconds: float,
) -> dict[str, Any]:
    body = build_request_body(binding, figure, image=image)
    result = call_anthropic_messages(
        binding,
        body,
        timeout_seconds=timeout_seconds,
        max_retries=max_retries,
        backoff_seconds=backoff_seconds,
    )
    response_text = (
        extract_text(result["payload"]) if isinstance(result.get("payload"), dict) else ""
    )
    score_payload: dict[str, Any] | None = None
    validation_error: str | None = None
    status = result["status"]
    try:
        score_payload = validate_score_payload(parse_response_json(response_text))
    except ValueError as exc:
        validation_error = str(exc)
        status = "failed"
    combined_error = result.get("error")
    if validation_error:
        combined_error = (
            f"{combined_error}; {validation_error}" if combined_error else validation_error
        )
    cost_usd, cost_note = response_cost_note(result.get("usage"))
    return {
        "binding_id": binding.binding_id,
        "model_id": binding.model_id,
        "model_used": binding.model_name,
        "modality": "multimodal" if image is not None else "text-only",
        "status": status,
        "status_code": result.get("status_code"),
        "latency_ms": result.get("latency_ms"),
        "attempts": result.get("attempts"),
        "scores": score_payload,
        "response_text": response_text,
        "usage": result.get("usage"),
        "cost_estimate_usd": cost_usd,
        "cost_estimate_note": cost_note,
        "error": combined_error,
    }


def compare_scores(fast: dict[str, Any], quality: dict[str, Any]) -> dict[str, Any]:
    fast_scores = fast.get("scores") or {}
    quality_scores = quality.get("scores") or {}
    deltas = {
        key: round(float(quality_scores.get(key, 0)) - float(fast_scores.get(key, 0)), 4)
        for key in DIMENSIONS
    }
    total_delta = round(sum(deltas.values()), 4)
    if total_delta > 0:
        winner = QUALITY_BINDING_ID
    elif total_delta < 0:
        winner = FAST_BINDING_ID
    else:
        winner = "tie"
    return {"dimension_deltas_m3_minus_m27": deltas, "total_delta": total_delta, "winner": winner}


def judge_figure(
    figure: FigureCandidate,
    bindings: dict[str, ModelBinding],
    output_dir: Path,
    *,
    timeout_seconds: int,
    max_retries: int,
    backoff_seconds: float,
    force: bool,
) -> dict[str, Any]:
    per_figure_dir = output_dir / "per-figure"
    image_cache_dir = output_dir / "image-cache"
    output_path = per_figure_dir / f"{figure.safe_id}.json"
    if output_path.exists() and not force:
        return json.loads(output_path.read_text())

    per_figure_dir.mkdir(parents=True, exist_ok=True)
    png_path = ensure_png_image(figure, image_cache_dir)
    fast = judge_one_model(
        figure,
        bindings[FAST_BINDING_ID],
        image=None,
        timeout_seconds=timeout_seconds,
        max_retries=max_retries,
        backoff_seconds=backoff_seconds,
    )
    quality = judge_one_model(
        figure,
        bindings[QUALITY_BINDING_ID],
        image=png_path,
        timeout_seconds=timeout_seconds,
        max_retries=max_retries,
        backoff_seconds=backoff_seconds,
    )
    record = {
        "figure": {
            "figure_id": figure.figure_id,
            "safe_id": figure.safe_id,
            "arxiv_id": figure.arxiv_id,
            "figure_idx": figure.figure_idx,
            "category": figure.category,
            "caption": figure.caption,
            "page_context_excerpt": figure.page_context,
            "source_json": str(figure.source_json.relative_to(ROOT)),
            "source_image_path": str(figure.image_path),
            "judge_png_path": str(png_path),
            "label": figure.label,
            "name": figure.name,
        },
        "safety_defaults": SAFETY_DEFAULTS,
        "diagnostic_llm_calls_override": DIAGNOSTIC_LLM_CALLS_OVERRIDE,
        "models": {
            FAST_BINDING_ID: fast,
            QUALITY_BINDING_ID: quality,
        },
        "comparison": compare_scores(fast, quality),
    }
    tmp_fd, tmp_name = tempfile.mkstemp(prefix=output_path.name, suffix=".tmp", dir=per_figure_dir)
    with os.fdopen(tmp_fd, "w") as handle:
        json.dump(record, handle, indent=2, sort_keys=True)
        handle.write("\n")
    Path(tmp_name).replace(output_path)
    return record


def aggregate(records: list[dict[str, Any]]) -> dict[str, Any]:
    model_stats: dict[str, Any] = {}
    for binding_id in (FAST_BINDING_ID, QUALITY_BINDING_ID):
        passed = [
            record["models"][binding_id]
            for record in records
            if record["models"][binding_id]["status"] == "passed"
        ]
        means = {}
        for key in DIMENSIONS:
            values = [float(item["scores"][key]) for item in passed if item.get("scores")]
            # pyrefly: ignore [unsupported-operation]
            means[key] = round(statistics.mean(values), 4) if values else None
        latencies = [
            float(item["latency_ms"]) for item in passed if item.get("latency_ms") is not None
        ]
        outlier_records = []
        for record in records:
            scores = record["models"][binding_id].get("scores") or {}
            low_dims = [
                key
                for key in DIMENSIONS
                if isinstance(scores.get(key), int | float) and float(scores[key]) < 0.5
            ]
            if low_dims:
                outlier_records.append(
                    {"figure_id": record["figure"]["figure_id"], "dimensions": low_dims}
                )
        model_stats[binding_id] = {
            "model_used": records[0]["models"][binding_id]["model_used"] if records else None,
            "passed_count": len(passed),
            "failed_count": len(records) - len(passed),
            "mean_scores": means,
            "latency_avg_ms": round(statistics.mean(latencies), 2) if latencies else None,
            "outlier_count": len(outlier_records),
            "outliers": outlier_records,
            "cost_estimate_usd": None,
            "cost_estimate_note": "Not measurable without an external MiniMax pricing table.",
        }
    winners = [record["comparison"]["winner"] for record in records]
    return {
        "model_stats": model_stats,
        "winner_counts": {winner: winners.count(winner) for winner in sorted(set(winners))},
        "figure_count": len(records),
        "category_counts": {
            category: sum(1 for record in records if record["figure"]["category"] == category)
            for category in ("data_plot", "schema_diagram")
        },
    }


def write_reports(records: list[dict[str, Any]], output_dir: Path) -> dict[str, Any]:
    output_dir.mkdir(parents=True, exist_ok=True)
    generated_at = datetime.now(UTC).isoformat()
    aggregate_stats = aggregate(records)
    side_by_side = [
        {
            "figure_id": record["figure"]["figure_id"],
            "category": record["figure"]["category"],
            "caption": record["figure"]["caption"],
            "m27_scores": record["models"][FAST_BINDING_ID].get("scores"),
            "m3_scores": record["models"][QUALITY_BINDING_ID].get("scores"),
            "m27_latency_ms": record["models"][FAST_BINDING_ID].get("latency_ms"),
            "m3_latency_ms": record["models"][QUALITY_BINDING_ID].get("latency_ms"),
            "winner": record["comparison"]["winner"],
            "deltas_m3_minus_m27": record["comparison"]["dimension_deltas_m3_minus_m27"],
        }
        for record in records
    ]
    comparison = {
        "generated_at": generated_at,
        "safety_defaults": SAFETY_DEFAULTS,
        "diagnostic_llm_calls_override": DIAGNOSTIC_LLM_CALLS_OVERRIDE,
        "network_host_reference": NETWORK_HOST,
        "selection": {
            "source": "M058 plotextractor per-pdf JSON with M056 page-context lookup",
            "strategy": "first 15 data_plot and first 15 schema_diagram figures with caption and image_path",
        },
        "aggregate": aggregate_stats,
        "side_by_side": side_by_side,
    }
    (output_dir / "comparison.json").write_text(
        json.dumps(comparison, indent=2, sort_keys=True) + "\n"
    )
    (output_dir / "judge-summary.json").write_text(
        json.dumps(
            {
                "generated_at": generated_at,
                "figure_count": aggregate_stats["figure_count"],
                "category_counts": aggregate_stats["category_counts"],
                "model_stats": aggregate_stats["model_stats"],
                "winner_counts": aggregate_stats["winner_counts"],
                "cost_estimate_total_usd": None,
                "cost_estimate_note": "Not measurable without an external MiniMax pricing table.",
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    write_markdown_report(comparison, output_dir / "comparison.md")
    return comparison


def write_markdown_report(comparison: dict[str, Any], path: Path) -> None:
    stats = comparison["aggregate"]["model_stats"]
    lines = [
        "# M060-gakmo0 S02 Figure QA Judge Comparison",
        "",
        f"Generated: {comparison['generated_at']}",
        "",
        "## Safety",
        "",
        "- Graph writes are not authorized.",
        "- Production import is not authorized.",
        "- Fact promotion is not authorized.",
        "- External network default is disabled; live LLM calls use a diagnostic-only override.",
        "- LLM calls default is disabled; override scope is M060-gakmo0 S02 only.",
        "- Local diagnostic host reference: 127.0.0.1.",
        "",
        "## Aggregate",
        "",
        f"- Figures judged: {comparison['aggregate']['figure_count']}",
        f"- Category counts: {comparison['aggregate']['category_counts']}",
        f"- Winner counts: {comparison['aggregate']['winner_counts']}",
        "- Cost estimate: not measurable without an external MiniMax pricing table.",
        "",
        "| Model | Caption accuracy | Figure completeness | Structural fidelity | Avg latency ms | Outliers | Failed |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]
    for binding_id in (FAST_BINDING_ID, QUALITY_BINDING_ID):
        model = stats[binding_id]
        means = model["mean_scores"]
        lines.append(
            f"| {binding_id} ({model['model_used']}) | {means['caption_accuracy']} | "
            f"{means['figure_completeness']} | {means['structural_fidelity']} | "
            f"{model['latency_avg_ms']} | {model['outlier_count']} | {model['failed_count']} |"
        )
    lines.extend(
        [
            "",
            "## Side-by-side",
            "",
            "| Figure | Category | Winner | Δ caption | Δ completeness | Δ structural |",
            "|---|---|---|---:|---:|---:|",
        ]
    )
    for row in comparison["side_by_side"]:
        deltas = row["deltas_m3_minus_m27"]
        lines.append(
            f"| {row['figure_id']} | {row['category']} | {row['winner']} | "
            f"{deltas['caption_accuracy']} | {deltas['figure_completeness']} | {deltas['structural_fidelity']} |"
        )
    path.write_text("\n".join(lines) + "\n")


def run_judge(
    *,
    output_dir: Path = DEFAULT_OUTPUT_DIR,
    figures_dir: Path = DEFAULT_FIGURES_DIR,
    models_yaml: Path = DEFAULT_MODELS_YAML,
    concurrency: int = 3,
    timeout_seconds: int = 90,
    max_retries: int = 2,
    backoff_seconds: float = 2.0,
    force: bool = False,
) -> dict[str, Any]:
    if not 2 <= concurrency <= 4:
        raise ValueError("concurrency must be between 2 and 4 for rate-limit safety")
    load_dotenv()
    bindings = load_bindings(models_yaml)
    for required in (FAST_BINDING_ID, QUALITY_BINDING_ID):
        if required not in bindings:
            raise RuntimeError(f"Required binding {required} is missing from models.yaml")
    figures = load_figure_candidates(figures_dir)
    records: list[dict[str, Any]] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as executor:
        future_to_figure = {
            executor.submit(
                judge_figure,
                figure,
                bindings,
                output_dir,
                timeout_seconds=timeout_seconds,
                max_retries=max_retries,
                backoff_seconds=backoff_seconds,
                force=force,
            ): figure
            for figure in figures
        }
        for future in concurrent.futures.as_completed(future_to_figure):
            records.append(future.result())
    records.sort(
        key=lambda record: (
            record["figure"]["category"],
            record["figure"]["arxiv_id"],
            record["figure"]["figure_idx"],
        )
    )
    return write_reports(records, output_dir)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--figures-dir", type=Path, default=DEFAULT_FIGURES_DIR)
    parser.add_argument("--models-yaml", type=Path, default=DEFAULT_MODELS_YAML)
    parser.add_argument("--concurrency", type=int, default=3)
    parser.add_argument("--timeout-seconds", type=int, default=90)
    parser.add_argument("--max-retries", type=int, default=2)
    parser.add_argument("--backoff-seconds", type=float, default=2.0)
    parser.add_argument("--force", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    comparison = run_judge(
        output_dir=args.output_dir,
        figures_dir=args.figures_dir,
        models_yaml=args.models_yaml,
        concurrency=args.concurrency,
        timeout_seconds=args.timeout_seconds,
        max_retries=args.max_retries,
        backoff_seconds=args.backoff_seconds,
        force=args.force,
    )
    print(
        json.dumps(
            {
                "figure_count": comparison["aggregate"]["figure_count"],
                "category_counts": comparison["aggregate"]["category_counts"],
                "winner_counts": comparison["aggregate"]["winner_counts"],
                "comparison_json": str((args.output_dir / "comparison.json").resolve()),
                "comparison_md": str((args.output_dir / "comparison.md").resolve()),
            },
            indent=2,
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
