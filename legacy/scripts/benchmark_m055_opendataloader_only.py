#!/usr/bin/env python3
"""OpenDataLoader-only baseline probe for M055 parser hybrid benchmark S03.

Runs a bounded, fail-closed OpenDataLoader probe over the five-PDF M055 corpus
manifest. The script emits per-PDF diagnostic packets, captured markdown/layout
artifacts, and a summary only; it never writes graph data, never attempts a
production import, and keeps all five safety defaults false.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import importlib
import json
import re
import subprocess
import sys
import tempfile
import time
import uuid
from collections.abc import Iterable
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m055-parser-benchmark.opendataloader-only.v1"
DEFAULT_CORPUS_MANIFEST = Path("artifacts/m055-parser-benchmark/corpus-manifest.json")
DEFAULT_OUTPUT_DIR = Path("artifacts/m055-parser-benchmark/opendataloader-only")
DEFAULT_MAX_RETRIES = 3
DEFAULT_THREADS = 4
DEFAULT_FORMAT = "md"
LOW_QUALITY_MIN_MARKDOWN_BYTES = 1024
AGGREGATE_STATUSES = ("success", "low_quality_source", "opendataloader_unavailable")
SAFETY_DEFAULTS = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}


class OpenDataLoaderProbeError(RuntimeError):
    """Raised when OpenDataLoader fails before producing readable artifacts."""


def _utc_now() -> str:
    return dt.datetime.now(tz=dt.UTC).isoformat()


def _safety_defaults() -> dict[str, bool]:
    return dict(SAFETY_DEFAULTS)


def _atomic_write_bytes(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(path.suffix + f".{uuid.uuid4().hex}.tmp")
    tmp_path.write_bytes(payload)
    tmp_path.replace(path)


def _atomic_write_text(path: Path, payload: str) -> None:
    _atomic_write_bytes(path, payload.encode("utf-8"))


def _atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    body = json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False).encode("utf-8")
    _atomic_write_bytes(path, body + b"\n")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _bounded_retries(max_retries: int) -> int:
    return max(1, min(DEFAULT_MAX_RETRIES, int(max_retries)))


def _normalize_format(format_name: str) -> str:
    normalized = format_name.strip().lower()
    if normalized in {"md", "markdown"}:
        return "markdown"
    return normalized


def _requested_formats(format_name: str) -> list[str]:
    normalized = _normalize_format(format_name)
    if normalized == "markdown":
        return ["markdown", "json"]
    if normalized == "json":
        return ["json"]
    return [normalized, "json"]


def _read_first_text_file(base_dir: Path, suffixes: Iterable[str]) -> str:
    for suffix in suffixes:
        matches = sorted(path for path in base_dir.rglob(f"*{suffix}") if path.is_file())
        if matches:
            return matches[0].read_text(encoding="utf-8", errors="replace")
    return ""


def _read_first_json_file(base_dir: Path) -> dict[str, Any] | list[Any] | None:
    matches = sorted(path for path in base_dir.rglob("*.json") if path.is_file())
    if not matches:
        return None
    return json.loads(matches[0].read_text(encoding="utf-8"))


def _run_via_import_api(pdf_path: Path, temp_output_dir: Path, format_name: str) -> str:
    module = importlib.import_module("opendataloader_pdf")
    normalized = _normalize_format(format_name)

    if hasattr(module, "run") and normalized == "markdown":
        module.run(str(pdf_path), output_folder=str(temp_output_dir), generate_markdown=True)
        return "import:run"

    if hasattr(module, "run_jar"):
        cli_format = ",".join(_requested_formats(format_name))
        module.run_jar(["-o", str(temp_output_dir), "-f", cli_format, str(pdf_path)], quiet=True)
        return "import:run_jar"

    if hasattr(module, "convert"):
        module.convert(
            str(pdf_path),
            output_dir=str(temp_output_dir),
            format=_requested_formats(format_name),
            quiet=True,
        )
        return "import:convert"

    raise OpenDataLoaderProbeError(
        "opendataloader_pdf has no supported run, run_jar, or convert API"
    )


def _run_via_subprocess(pdf_path: Path, temp_output_dir: Path, format_name: str) -> str:
    cli_format = ",".join(_requested_formats(format_name))
    command = [
        sys.executable,
        "-m",
        "opendataloader_pdf",
        "-o",
        str(temp_output_dir),
        "-f",
        cli_format,
        str(pdf_path),
    ]
    completed = subprocess.run(command, check=False, capture_output=True, text=True, timeout=300)
    if completed.returncode != 0:
        message = (
            completed.stderr or completed.stdout or "OpenDataLoader subprocess failed"
        ).strip()
        raise OpenDataLoaderProbeError(message)
    return "subprocess:python -m opendataloader_pdf"


def _persist_probe_artifacts(
    pdf_path: Path,
    output_dir: Path,
    markdown_text: str,
    json_layout: dict[str, Any] | list[Any] | None,
) -> tuple[str | None, str | None]:
    stem = pdf_path.stem
    markdown_path = output_dir / "markdown" / f"{stem}.md"
    layout_path = output_dir / "layout" / f"{stem}.json"

    markdown_rel: str | None = None
    layout_rel: str | None = None
    if markdown_text:
        _atomic_write_text(markdown_path, markdown_text)
        markdown_rel = str(markdown_path.relative_to(output_dir))
    if json_layout is not None:
        _atomic_write_json(layout_path, {"layout": json_layout})
        layout_rel = str(layout_path.relative_to(output_dir))
    return markdown_rel, layout_rel


def _probe_opendataloader_pdf(
    pdf_path: Path,
    output_dir: Path,
    *,
    threads: int,
    format: str,
) -> dict[str, Any]:
    """Run OpenDataLoader for one PDF and return captured markdown/layout data.

    The current OpenDataLoader 2.4.7 Python API does not expose a threads
    argument. The value is accepted for CLI compatibility and recorded by the
    caller, but not passed to the library.
    """

    del threads
    output_dir.mkdir(parents=True, exist_ok=True)
    start = time.monotonic()
    try:
        with tempfile.TemporaryDirectory(prefix=f"{pdf_path.stem}.", dir=output_dir) as tmp:
            temp_output_dir = Path(tmp)
            try:
                runner = _run_via_import_api(pdf_path, temp_output_dir, format)
            except (ImportError, ModuleNotFoundError):
                raise
            except Exception as import_error:
                try:
                    runner = _run_via_subprocess(pdf_path, temp_output_dir, format)
                except Exception as subprocess_error:
                    raise OpenDataLoaderProbeError(
                        f"import API failed: {import_error}; subprocess failed: {subprocess_error}"
                    ) from subprocess_error

            markdown_text = _read_first_text_file(temp_output_dir, (".md", ".markdown"))
            json_layout = _read_first_json_file(temp_output_dir)
            markdown_rel, layout_rel = _persist_probe_artifacts(
                pdf_path, output_dir, markdown_text, json_layout
            )
            duration_ms = int((time.monotonic() - start) * 1000)
            return {
                "markdown_text": markdown_text,
                "json_layout": json_layout,
                "format": format,
                "normalized_format": _normalize_format(format),
                "bytes": len(markdown_text.encode("utf-8")),
                "duration_ms": duration_ms,
                "error": None,
                "runner": runner,
                "markdown_path": markdown_rel,
                "layout_path": layout_rel,
            }
    except Exception as exc:  # fail closed; caller maps this to unavailable status
        duration_ms = int((time.monotonic() - start) * 1000)
        return {
            "markdown_text": "",
            "json_layout": None,
            "format": format,
            "normalized_format": _normalize_format(format),
            "bytes": 0,
            "duration_ms": duration_ms,
            "error": f"{type(exc).__name__}: {exc}",
            "runner": None,
            "markdown_path": None,
            "layout_path": None,
        }


def _walk_json_nodes(value: Any) -> Iterable[Any]:
    yield value
    if isinstance(value, dict):
        for child in value.values():
            yield from _walk_json_nodes(child)
    elif isinstance(value, list):
        for child in value:
            yield from _walk_json_nodes(child)


def _layout_page_count(json_layout: dict[str, Any] | list[Any] | None) -> int:
    if json_layout is None:
        return 0
    if isinstance(json_layout, dict) and isinstance(json_layout.get("number of pages"), int):
        return int(json_layout["number of pages"])
    page_numbers = []
    for node in _walk_json_nodes(json_layout):
        if isinstance(node, dict) and isinstance(node.get("page number"), int):
            page_numbers.append(int(node["page number"]))
    return max(page_numbers, default=0)


def _layout_bounding_box_count(json_layout: dict[str, Any] | list[Any] | None) -> int:
    if json_layout is None:
        return 0
    count = 0
    for node in _walk_json_nodes(json_layout):
        if isinstance(node, dict):
            box = node.get("bounding box") or node.get("bbox")
            if isinstance(box, list) and len(box) >= 4:
                count += 1
    return count


def _extract_markdown_metrics(
    markdown_text: str, json_layout: dict[str, Any] | list[Any] | None
) -> dict[str, Any]:
    markdown_size_bytes = len(markdown_text.encode("utf-8"))
    table_count = len(re.findall(r"\|\s*-{2,}\s*\|", markdown_text))
    table_count += markdown_text.lower().count("<table")
    image_count = markdown_text.count("![") + len(
        re.findall(r"<img\b|<image\b", markdown_text, re.I)
    )
    section_count = len(re.findall(r"(?m)^#{1,6}\s+\S", markdown_text))
    page_count = _layout_page_count(json_layout)
    bounding_box_count = _layout_bounding_box_count(json_layout)

    metrics = {
        "markdown_size_bytes": markdown_size_bytes,
        "table_count": table_count,
        "image_count": image_count,
        "section_count": section_count,
        "page_count": page_count,
        "bounding_box_count": bounding_box_count,
    }
    metrics["low_quality_source"] = _low_quality_source_criteria(metrics)
    return metrics


def _low_quality_source_criteria(md_metrics: dict[str, Any]) -> bool:
    markdown_size_bytes = int(md_metrics.get("markdown_size_bytes") or 0)
    table_count = int(md_metrics.get("table_count") or 0)
    image_count = int(md_metrics.get("image_count") or 0)
    section_count = int(md_metrics.get("section_count") or 0)
    return (
        markdown_size_bytes < LOW_QUALITY_MIN_MARKDOWN_BYTES
        or (table_count == 0 and image_count == 0)
        or section_count == 0
    )


def _load_manifest(corpus_manifest_path: Path) -> list[dict[str, Any]]:
    manifest = json.loads(corpus_manifest_path.read_text(encoding="utf-8"))
    pdfs = manifest.get("pdfs")
    if not isinstance(pdfs, list):
        raise ValueError(f"manifest {corpus_manifest_path} does not contain a pdfs list")
    return pdfs


def _packet_for_pdf(
    entry: dict[str, Any],
    probe: dict[str, Any],
    metrics: dict[str, Any],
    *,
    attempts: int,
    status: str,
    sha256_actual: str,
) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "article_key": entry.get("article_key"),
        "arxiv_id": entry.get("arxiv_id"),
        "category": entry.get("category"),
        "pdf_path": entry.get("path"),
        "manifest_sha256": entry.get("sha256"),
        "sha256_actual": sha256_actual,
        "sha256_matches_manifest": sha256_actual == entry.get("sha256"),
        "status": status,
        "http_status": None,
        "format": probe.get("format"),
        "normalized_format": probe.get("normalized_format"),
        "runner": probe.get("runner"),
        "bytes": probe.get("bytes", 0),
        "markdown_size_bytes": metrics["markdown_size_bytes"],
        "table_count": metrics["table_count"],
        "image_count": metrics["image_count"],
        "section_count": metrics["section_count"],
        "page_count": metrics["page_count"],
        "bounding_box_count": metrics["bounding_box_count"],
        "low_quality_source": metrics["low_quality_source"],
        "attempts": attempts,
        "duration_ms": probe.get("duration_ms", 0),
        "error": probe.get("error"),
        "markdown_path": probe.get("markdown_path"),
        "layout_path": probe.get("layout_path"),
        "safety_defaults": _safety_defaults(),
    }


def probe_opendataloader_only(
    corpus_manifest_path: Path,
    output_dir: Path,
    *,
    max_retries: int = DEFAULT_MAX_RETRIES,
    threads: int = DEFAULT_THREADS,
    format: str = DEFAULT_FORMAT,
    dry_run: bool = False,
) -> dict[str, Any]:
    pdfs = _load_manifest(corpus_manifest_path)
    output_dir.mkdir(parents=True, exist_ok=True)
    per_pdf_dir = output_dir / "per-pdf"
    aggregate_counts = dict.fromkeys(AGGREGATE_STATUSES, 0)
    packets: list[dict[str, Any]] = []
    retries = _bounded_retries(max_retries)

    if dry_run:
        summary = {
            "schema_version": SCHEMA_VERSION,
            "dry_run": True,
            "corpus_manifest_path": str(corpus_manifest_path),
            "output_dir": str(output_dir),
            "total_pdfs": len(pdfs),
            "aggregate_counts": aggregate_counts,
            "safety_defaults": _safety_defaults(),
        }
        _atomic_write_json(output_dir / "summary.json", summary)
        return summary

    for entry in pdfs:
        pdf_path = Path(str(entry["path"]))
        sha256_actual = _sha256(pdf_path) if pdf_path.exists() else ""
        last_probe: dict[str, Any] | None = None
        attempts = 0
        for attempt_index in range(1, retries + 1):
            attempts = attempt_index
            last_probe = _probe_opendataloader_pdf(
                pdf_path, output_dir, threads=threads, format=format
            )
            if last_probe.get("error") is None:
                break

        probe = last_probe or {
            "markdown_text": "",
            "json_layout": None,
            "format": format,
            "normalized_format": _normalize_format(format),
            "bytes": 0,
            "duration_ms": 0,
            "error": "OpenDataLoader did not run",
        }
        metrics = _extract_markdown_metrics(
            str(probe.get("markdown_text") or ""), probe.get("json_layout")
        )
        if probe.get("error") is not None:
            status = "opendataloader_unavailable"
            metrics["low_quality_source"] = True
        elif metrics["low_quality_source"]:
            status = "low_quality_source"
        else:
            status = "success"
        aggregate_counts[status] += 1

        packet = _packet_for_pdf(
            entry,
            probe,
            metrics,
            attempts=attempts,
            status=status,
            sha256_actual=sha256_actual,
        )
        packet_path = per_pdf_dir / f"{entry['arxiv_id']}.json"
        _atomic_write_json(packet_path, packet)
        packets.append(packet)

    summary = {
        "schema_version": SCHEMA_VERSION,
        "corpus_manifest_path": str(corpus_manifest_path),
        "output_dir": str(output_dir),
        "total_pdfs": len(pdfs),
        "aggregate_counts": aggregate_counts,
        "success_count": aggregate_counts["success"],
        "low_quality_source_count": aggregate_counts["low_quality_source"],
        "opendataloader_unavailable_count": aggregate_counts["opendataloader_unavailable"],
        "total_markdown_size_bytes": sum(packet["markdown_size_bytes"] for packet in packets),
        "total_table_count": sum(packet["table_count"] for packet in packets),
        "total_image_count": sum(packet["image_count"] for packet in packets),
        "total_section_count": sum(packet["section_count"] for packet in packets),
        "total_page_count": sum(packet["page_count"] for packet in packets),
        "total_bounding_box_count": sum(packet["bounding_box_count"] for packet in packets),
        "per_pdf_statuses": {packet["arxiv_id"]: packet["status"] for packet in packets},
        "safety_defaults": _safety_defaults(),
    }
    _atomic_write_json(output_dir / "summary.json", summary)
    return summary


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus-manifest", type=Path, default=DEFAULT_CORPUS_MANIFEST)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--max-retries", type=int, default=DEFAULT_MAX_RETRIES)
    parser.add_argument("--threads", type=int, default=DEFAULT_THREADS)
    parser.add_argument("--format", default=DEFAULT_FORMAT)
    parser.add_argument("--dry-run", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_arg_parser().parse_args(argv)
    summary = probe_opendataloader_only(
        args.corpus_manifest,
        args.output_dir,
        max_retries=args.max_retries,
        threads=args.threads,
        format=args.format,
        dry_run=args.dry_run,
    )
    print(json.dumps(summary, indent=2, sort_keys=True, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
