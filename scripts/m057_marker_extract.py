#!/usr/bin/env python3
"""Run M057 Marker/Nougat extraction over the 166-PDF M056 corpus.

The extractor is deliberately fail-closed. If Marker or the Nougat fallback is not
usable in this environment, every PDF still receives an explicit per-PDF packet
with status=marker_unavailable instead of a fabricated success.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import tempfile
import time
from collections import Counter
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "artifacts" / "m056-bfs-graph" / "cumulative-corpus.json"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m057-fd-marker" / "marker-extraction"

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}

_MARKDOWN_TABLE_RE = re.compile(r"^\s*\|.+\|\s*$", re.MULTILINE)
_FIGURE_RE = re.compile(r"\b(?:figure|fig\.)\s+\d+", re.IGNORECASE)
_EQUATION_RE = re.compile(r"(?:\$\$|\\\[|\\begin\{equation\}|\\begin\{align\})")
_WORD_RE = re.compile(r"\b\w+\b", re.UNICODE)


def run_command(command: list[str], *, timeout_seconds: int) -> dict[str, Any]:
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout_seconds,
            check=False,
        )
        return {
            "command": command,
            "exit_code": completed.returncode,
            "duration_ms": round((time.perf_counter() - started) * 1000, 3),
            "stdout_tail": completed.stdout[-1000:],
            "stderr_tail": completed.stderr[-1000:],
            "timed_out": False,
        }
    except subprocess.TimeoutExpired as exc:
        return {
            "command": command,
            "exit_code": 124,
            "duration_ms": round((time.perf_counter() - started) * 1000, 3),
            "stdout_tail": (exc.stdout or "")[-1000:] if isinstance(exc.stdout, str) else "",
            "stderr_tail": (exc.stderr or "")[-1000:] if isinstance(exc.stderr, str) else "",
            "timed_out": True,
        }


def load_manifest(path: Path = DEFAULT_MANIFEST) -> list[dict[str, Any]]:
    data = json.loads(path.read_text(encoding="utf-8"))
    pdfs = data.get("pdfs", [])
    seen: set[str] = set()
    unique: list[dict[str, Any]] = []
    for item in pdfs:
        arxiv_id = str(item.get("arxiv_id", "")).strip()
        pdf_path = str(item.get("path", "")).strip()
        if not arxiv_id or not pdf_path or arxiv_id in seen:
            continue
        seen.add(arxiv_id)
        unique.append({**item, "arxiv_id": arxiv_id, "path": pdf_path})
    return unique


def install_backend(*, skip_install: bool = False, timeout_seconds: int = 600) -> dict[str, Any]:
    marker_install = {"attempted": False, "ok": False, "result": None}
    nougat_install = {"attempted": False, "ok": False, "result": None}
    if not skip_install:
        marker_install["attempted"] = True
        marker_install["result"] = run_command(
            ["uv", "pip", "install", "marker-pdf"], timeout_seconds=timeout_seconds
        )
        marker_install["ok"] = marker_install["result"]["exit_code"] == 0
    marker_preflight = run_command(["uv", "run", "marker_single", "--help"], timeout_seconds=60)
    if marker_preflight["exit_code"] == 0:
        return {
            "backend": "marker",
            "marker_install": marker_install,
            "marker_preflight": marker_preflight,
            "nougat_install": nougat_install,
            "nougat_preflight": None,
        }

    if not skip_install:
        nougat_install["attempted"] = True
        nougat_install["result"] = run_command(
            ["uv", "pip", "install", "nougat-ocr"], timeout_seconds=timeout_seconds
        )
        nougat_install["ok"] = nougat_install["result"]["exit_code"] == 0
    nougat_preflight = run_command(["uv", "run", "nougat", "--help"], timeout_seconds=60)
    backend = "nougat" if nougat_preflight["exit_code"] == 0 else "none"
    return {
        "backend": backend,
        "marker_install": marker_install,
        "marker_preflight": marker_preflight,
        "nougat_install": nougat_install,
        "nougat_preflight": nougat_preflight,
    }


def extract_text_metrics(text: str) -> dict[str, Any]:
    table_lines = _MARKDOWN_TABLE_RE.findall(text)
    table_count = max(0, len(table_lines) // 2)
    if table_count:
        table_structure_quality_avg = min(
            1.0, 0.5 + min(0.5, len(table_lines) / max(1, table_count * 4))
        )
    else:
        table_structure_quality_avg = 0.0
    return {
        "table_count": table_count,
        "table_structure_quality_avg": round(table_structure_quality_avg, 3),
        "figure_count": len(_FIGURE_RE.findall(text)),
        "equation_count": len(_EQUATION_RE.findall(text)),
        "body_word_count": len(_WORD_RE.findall(text)),
    }


def find_text_output(output_dir: Path) -> str:
    chunks: list[str] = []
    for extension in ("*.md", "*.mmd", "*.txt", "*.html"):
        for path in sorted(output_dir.rglob(extension)):
            try:
                chunks.append(path.read_text(encoding="utf-8", errors="replace"))
            except OSError:
                continue
    return "\n\n".join(chunks)


def unavailable_packet(arxiv_id: str, pdf_path: str, *, error: str, backend: str) -> dict[str, Any]:
    return {
        "arxiv_id": arxiv_id,
        "pdf_path": pdf_path,
        "backend": backend,
        "table_count": 0,
        "table_structure_quality_avg": 0.0,
        "figure_count": 0,
        "equation_count": 0,
        "body_word_count": 0,
        "status": "marker_unavailable",
        "error": error,
    }


def extract_one(
    pdf: dict[str, Any], backend: str, per_pdf_root: Path, timeout_seconds: int
) -> dict[str, Any]:
    arxiv_id = pdf["arxiv_id"]
    pdf_path = str(pdf["path"])
    absolute_pdf_path = ROOT / pdf_path
    if not absolute_pdf_path.exists():
        return unavailable_packet(arxiv_id, pdf_path, error="pdf_missing", backend=backend)
    if backend not in {"marker", "nougat"}:
        return unavailable_packet(
            arxiv_id, pdf_path, error="marker and nougat are not usable", backend="none"
        )

    with tempfile.TemporaryDirectory(prefix=f"m057-{arxiv_id}-") as tmp_name:
        tmp_output = Path(tmp_name)
        if backend == "marker":
            command = [
                "uv",
                "run",
                "marker_single",
                str(absolute_pdf_path),
                "--output_dir",
                str(tmp_output),
            ]
        else:
            command = ["uv", "run", "nougat", str(absolute_pdf_path), "--out", str(tmp_output)]
        result = run_command(command, timeout_seconds=timeout_seconds)
        if result["exit_code"] != 0:
            return unavailable_packet(
                arxiv_id,
                pdf_path,
                error=f"{backend} extraction failed: {result['stderr_tail'] or result['stdout_tail']}",
                backend=backend,
            )
        text = find_text_output(tmp_output)
        if not text.strip():
            return unavailable_packet(
                arxiv_id, pdf_path, error=f"{backend} produced no text output", backend=backend
            )
        metrics = extract_text_metrics(text)
        destination = per_pdf_root / f"{arxiv_id}.md"
        destination.write_text(text, encoding="utf-8")
        return {
            "arxiv_id": arxiv_id,
            "pdf_path": pdf_path,
            "backend": backend,
            **metrics,
            "status": "success",
            "error": None,
        }


def run_extraction(
    manifest_path: Path = DEFAULT_MANIFEST,
    output_dir: Path = DEFAULT_OUTPUT_DIR,
    *,
    skip_install: bool = False,
    per_pdf_timeout_seconds: int = 180,
) -> dict[str, Any]:
    output_dir.mkdir(parents=True, exist_ok=True)
    per_pdf_dir = output_dir / "per-pdf"
    per_pdf_dir.mkdir(parents=True, exist_ok=True)
    text_dir = output_dir / "text"
    text_dir.mkdir(parents=True, exist_ok=True)

    pdfs = load_manifest(manifest_path)
    install = install_backend(skip_install=skip_install)
    backend = install["backend"]
    per_pdf: list[dict[str, Any]] = []
    for pdf in pdfs:
        packet = extract_one(pdf, backend, text_dir, per_pdf_timeout_seconds)
        per_pdf.append(packet)
        (per_pdf_dir / f"{packet['arxiv_id']}.json").write_text(
            json.dumps(packet, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    statuses = Counter(str(packet["status"]) for packet in per_pdf)
    successful = [packet for packet in per_pdf if packet["status"] == "success"]
    summary = {
        "schema_version": "m057-fd-marker.marker-extraction.v1",
        "manifest_path": str(
            manifest_path.relative_to(ROOT) if manifest_path.is_absolute() else manifest_path
        ),
        "output_dir": str(output_dir.relative_to(ROOT) if output_dir.is_absolute() else output_dir),
        "safety_defaults": SAFETY_DEFAULTS,
        "backend": backend,
        "install": install,
        "total_pdfs": len(pdfs),
        "success_count": len(successful),
        "status_counts": dict(sorted(statuses.items())),
        "total_table_count": sum(int(packet["table_count"]) for packet in per_pdf),
        "total_figure_count": sum(int(packet["figure_count"]) for packet in per_pdf),
        "total_equation_count": sum(int(packet["equation_count"]) for packet in per_pdf),
        "total_body_word_count": sum(int(packet["body_word_count"]) for packet in per_pdf),
        "table_structure_quality_avg": round(
            sum(float(packet["table_structure_quality_avg"]) for packet in per_pdf) / len(per_pdf),
            3,
        )
        if per_pdf
        else 0.0,
        "per_pdf": per_pdf,
    }
    (output_dir / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--skip-install", action="store_true")
    parser.add_argument("--per-pdf-timeout-seconds", type=int, default=180)
    args = parser.parse_args()
    summary = run_extraction(
        args.manifest,
        args.output_dir,
        skip_install=args.skip_install,
        per_pdf_timeout_seconds=args.per_pdf_timeout_seconds,
    )
    print(
        json.dumps(
            {
                "backend": summary["backend"],
                "total_pdfs": summary["total_pdfs"],
                "status_counts": summary["status_counts"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
