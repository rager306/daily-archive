#!/usr/bin/env python3
"""Extract M058 S01 figure captions from arXiv TeX source with plotextractor.

The artifact writer is intentionally idempotent: each run rewrites only
artifacts/m058-plotextractor outputs and /tmp/m058_tex extraction folders.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import tarfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

from plotextractor import process_tarball

ROOT = Path(__file__).resolve().parents[1]
ARTIFACT_ROOT = ROOT / "artifacts" / "m058-plotextractor"
DEFAULT_PER_PDF_DIR = ARTIFACT_ROOT / "per-pdf"
DEFAULT_SUMMARY = ARTIFACT_ROOT / "summary.json"
DEFAULT_TMP_ROOT = Path("/tmp/m058_tex")
DEFAULT_SAMPLE: tuple[dict[str, str], ...] = (
    {
        "arxiv_id": "2605.18747",
        "category": "cs-cl",
        "role": "anchor",
        "source_pdf": "data/article_catalog/article_catalog/arxiv/cs-cl/2605.18747/source/2605.18747.pdf",
    },
    {
        "arxiv_id": "2601.05808",
        "category": "cs-cl",
        "role": "diverse-category",
        "source_pdf": "data/article_catalog/article_catalog/arxiv/cs-cl/2601.05808/source/2601.05808.pdf",
    },
    {
        "arxiv_id": "2602.10090",
        "category": "cs-ai",
        "role": "diverse-category",
        "source_pdf": "data/article_catalog/article_catalog/arxiv/cs-ai/2602.10090/source/2602.10090.pdf",
    },
    {
        "arxiv_id": "2507.19457",
        "category": "cs-lg",
        "role": "diverse-category",
        "source_pdf": "data/article_catalog/article_catalog/arxiv/cs-lg/2507.19457/source/2507.19457.pdf",
    },
    {
        "arxiv_id": "1804.02767",
        "category": "cs-cv",
        "role": "diverse-category",
        "source_pdf": "data/article_catalog/article_catalog/arxiv/cs-cv/1804.02767/source/1804.02767.pdf",
    },
)

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}


class TexDownloadError(RuntimeError):
    """Raised when an arXiv TeX tarball cannot be downloaded."""


class TexExtractionError(RuntimeError):
    """Raised when a TeX source archive cannot be safely extracted."""


def _safe_member_path(base: Path, member_name: str) -> Path:
    target = (base / member_name).resolve()
    base_resolved = base.resolve()
    if base_resolved not in target.parents and target != base_resolved:
        raise TexExtractionError(f"tar member escapes extraction directory: {member_name}")
    return target


def safe_extract_tarball(tarball_path: Path, destination: Path) -> None:
    """Extract tarball_path into destination after path traversal checks."""

    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True, exist_ok=True)
    try:
        with tarfile.open(tarball_path) as archive:
            for member in archive.getmembers():
                _safe_member_path(destination, member.name)
            archive.extractall(destination, filter="data")
    except tarfile.TarError as exc:
        raise TexExtractionError(f"failed to extract {tarball_path}: {exc}") from exc


def download_tex_tarball(
    arxiv_id: str,
    *,
    tmp_root: Path = DEFAULT_TMP_ROOT,
    timeout_seconds: float = 30.0,
    retries: int = 3,
) -> Path:
    """Download https://arxiv.org/e-print/{arxiv_id} into tmp_root."""

    tmp_root.mkdir(parents=True, exist_ok=True)
    tarball_path = tmp_root / f"{arxiv_id}.tar"
    url = f"https://arxiv.org/e-print/{arxiv_id}"
    last_error: BaseException | None = None
    for attempt in range(1, retries + 1):
        request = urllib.request.Request(url, headers={"User-Agent": "daily-archive-m058/1.0"})
        try:
            with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
                data = response.read()
            if not data:
                raise TexDownloadError(f"empty TeX tarball response for {arxiv_id}")
            tarball_path.write_bytes(data)
            return tarball_path
        except (
            TimeoutError,
            urllib.error.URLError,
            urllib.error.HTTPError,
            TexDownloadError,
        ) as exc:
            last_error = exc
            if attempt < retries:
                time.sleep(min(2**attempt, 5))
    raise TexDownloadError(f"failed to download TeX tarball for {arxiv_id}: {last_error}")


def _caption_text(raw: Any) -> str:
    if isinstance(raw, list):
        return "\n".join(str(item).strip() for item in raw if str(item).strip()).strip()
    if raw is None:
        return ""
    return str(raw).strip()


def normalize_plotextractor_figures(
    arxiv_id: str, records: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    """Normalize plotextractor output into the S01 artifact schema."""

    figures: list[dict[str, Any]] = []
    for idx, record in enumerate(records, start=1):
        caption = _caption_text(record.get("captions"))
        image_path = str(record.get("url") or record.get("image_path") or "")
        label = str(record.get("label") or "").strip()
        name = str(record.get("name") or Path(image_path).stem or f"figure-{idx}").strip()
        figures.append(
            {
                "figure_id": f"{arxiv_id}::{idx}",
                "figure_idx": idx,
                "name": name,
                "label": label,
                "caption_text": caption,
                "image_path": image_path,
                "extraction_source": "plotextractor",
            }
        )
    return figures


def _strip_tex_comments(text: str) -> str:
    lines: list[str] = []
    for line in text.splitlines():
        cut_at: int | None = None
        for index, char in enumerate(line):
            if char == "%" and (index == 0 or line[index - 1] != "\\"):
                cut_at = index
                break
        lines.append(line if cut_at is None else line[:cut_at])
    return "\n".join(lines)


def _balanced_brace_argument(text: str, open_index: int) -> str | None:
    if open_index >= len(text) or text[open_index] != "{":
        return None
    depth = 0
    start = open_index + 1
    index = open_index
    while index < len(text):
        char = text[index]
        escaped = index > 0 and text[index - 1] == "\\"
        if char == "{" and not escaped:
            depth += 1
        elif char == "}" and not escaped:
            depth -= 1
            if depth == 0:
                return text[start:index].strip()
        index += 1
    return None


def _command_argument(text: str, command: str) -> str:
    match = re.search(rf"\\{re.escape(command)}\b", text)
    if not match:
        return ""
    index = match.end()
    while index < len(text) and text[index].isspace():
        index += 1
    if index < len(text) and text[index] == "[":
        depth = 1
        index += 1
        while index < len(text) and depth:
            if text[index] == "[" and text[index - 1] != "\\":
                depth += 1
            elif text[index] == "]" and text[index - 1] != "\\":
                depth -= 1
            index += 1
        while index < len(text) and text[index].isspace():
            index += 1
    if index < len(text) and text[index] == "{":
        return _balanced_brace_argument(text, index) or ""
    return ""


def _all_command_arguments(text: str, command: str) -> list[str]:
    values: list[str] = []
    for match in re.finditer(rf"\\{re.escape(command)}(?:\[[^\]]*\])?\s*{{", text):
        open_index = match.end() - 1
        value = _balanced_brace_argument(text, open_index)
        if value:
            values.append(value)
    return values


def _clean_latex_text(text: str) -> str:
    text = re.sub(r"\s+", " ", text).strip()
    text = text.replace("\\(", "").replace("\\)", "")
    return text


def _resolve_image_path(tex_file: Path, image_ref: str, tex_root: Path) -> str:
    if not image_ref:
        return ""
    raw = image_ref.strip()
    candidate = (tex_file.parent / raw).resolve()
    suffixes = ["", ".pdf", ".png", ".jpg", ".jpeg", ".eps"]
    for suffix in suffixes:
        path = candidate if suffix == "" else candidate.with_suffix(suffix)
        if path.exists():
            return str(path)
    matches = list(tex_root.rglob(Path(raw).name))
    if matches:
        return str(matches[0])
    return raw


def extract_figures_from_tex_source(arxiv_id: str, tex_root: Path) -> list[dict[str, Any]]:
    """Fallback TeX parser for figure environments missed by plotextractor."""

    figures: list[dict[str, Any]] = []
    figure_pattern = re.compile(r"\\begin\{figure\*?\}(.*?)\\end\{figure\*?\}", re.DOTALL)
    for tex_file in sorted(tex_root.rglob("*.tex")):
        try:
            text = _strip_tex_comments(tex_file.read_text(encoding="utf-8", errors="ignore"))
        except OSError:
            continue
        for env in figure_pattern.findall(text):
            caption = _clean_latex_text(_command_argument(env, "caption"))
            if not caption:
                continue
            label = _command_argument(env, "label")
            image_refs = _all_command_arguments(env, "includegraphics")
            image_path = (
                _resolve_image_path(tex_file, image_refs[0], tex_root) if image_refs else ""
            )
            idx = len(figures) + 1
            figures.append(
                {
                    "figure_id": f"{arxiv_id}::{idx}",
                    "figure_idx": idx,
                    "name": label or Path(image_path).stem or f"figure-{idx}",
                    "label": label,
                    "caption_text": caption,
                    "image_path": image_path,
                    "tex_file": str(tex_file),
                    "extraction_source": "tex-fallback",
                }
            )
    return figures


def merge_figure_records(
    arxiv_id: str, records: list[dict[str, Any]], tex_root: Path
) -> list[dict[str, Any]]:
    """Merge plotextractor records with TeX fallback captions and reindex."""

    merged: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for figure in [
        *normalize_plotextractor_figures(arxiv_id, records),
        *extract_figures_from_tex_source(arxiv_id, tex_root),
    ]:
        caption = re.sub(r"\s+", " ", str(figure.get("caption_text") or "")).strip().lower()
        label = str(figure.get("label") or "").strip().lower()
        key = (label, caption[:160])
        if not caption or key in seen:
            continue
        seen.add(key)
        idx = len(merged) + 1
        figure["figure_idx"] = idx
        figure["figure_id"] = f"{arxiv_id}::{idx}"
        merged.append(figure)
    return merged


def extract_one(
    sample: dict[str, str],
    *,
    per_pdf_dir: Path = DEFAULT_PER_PDF_DIR,
    tmp_root: Path = DEFAULT_TMP_ROOT,
    timeout_seconds: float = 30.0,
    retries: int = 3,
) -> dict[str, Any]:
    """Download, extract, run plotextractor, and persist one per-PDF packet."""

    arxiv_id = sample["arxiv_id"]
    tarball_path = download_tex_tarball(
        arxiv_id,
        tmp_root=tmp_root,
        timeout_seconds=timeout_seconds,
        retries=retries,
    )
    tex_extract_path = tmp_root / arxiv_id
    plotextractor_output = tex_extract_path / "plotextractor-output"
    safe_extract_tarball(tarball_path, tex_extract_path)
    if plotextractor_output.exists():
        shutil.rmtree(plotextractor_output)
    plotextractor_output.mkdir(parents=True, exist_ok=True)
    records = process_tarball(str(tarball_path), str(plotextractor_output), context=True)
    if not isinstance(records, list):
        raise TexExtractionError(f"plotextractor returned {type(records).__name__}, expected list")
    fallback_figures = extract_figures_from_tex_source(arxiv_id, tex_extract_path)
    figures = merge_figure_records(arxiv_id, records, tex_extract_path)
    packet: dict[str, Any] = {
        "schema_version": "m058.plotextractor.per-pdf.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "arxiv_id": arxiv_id,
        "category": sample.get("category"),
        "role": sample.get("role"),
        "source_pdf": sample.get("source_pdf"),
        "tex_status": "ok",
        "tex_tarball_url": f"https://arxiv.org/e-print/{arxiv_id}",
        "tex_tarball_path": str(tarball_path),
        "tex_tarball_size": tarball_path.stat().st_size,
        "tex_extracted_path": str(tex_extract_path),
        "plotextractor_output_path": str(plotextractor_output),
        "plotextractor_figure_count": len(records),
        "tex_fallback_figure_count": len(fallback_figures),
        "figure_count": len(figures),
        "caption_count": sum(1 for figure in figures if figure["caption_text"]),
        "figures": figures,
    }
    per_pdf_dir.mkdir(parents=True, exist_ok=True)
    (per_pdf_dir / f"{arxiv_id}.json").write_text(
        json.dumps(packet, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return packet


def _flatten_corpus(packets: list[dict[str, Any]]) -> list[dict[str, Any]]:
    figures: list[dict[str, Any]] = []
    for packet in packets:
        for figure in packet.get("figures", []):
            figures.append(
                {
                    "arxiv_id": packet["arxiv_id"],
                    "category": packet.get("category"),
                    "source_pdf": packet.get("source_pdf"),
                    "figure_id": figure["figure_id"],
                    "figure_idx": figure["figure_idx"],
                    "name": figure.get("name", ""),
                    "label": figure.get("label", ""),
                    "caption": figure.get("caption_text", ""),
                    "image_path": figure.get("image_path", ""),
                    "text_repr": f"Figure from {packet['arxiv_id']}: {figure.get('caption_text', '')}",
                }
            )
    return figures


def write_summary(
    packets: list[dict[str, Any]], *, summary_path: Path = DEFAULT_SUMMARY
) -> dict[str, Any]:
    """Write the aggregate extraction summary and v2 caption corpus."""

    figures = _flatten_corpus(packets)
    by_pdf = [
        {
            "arxiv_id": packet["arxiv_id"],
            "category": packet.get("category"),
            "tex_status": packet.get("tex_status"),
            "tex_tarball_size": packet.get("tex_tarball_size"),
            "figure_count": packet.get("figure_count", 0),
            "caption_count": packet.get("caption_count", 0),
        }
        for packet in packets
    ]
    summary: dict[str, Any] = {
        "schema_version": "m058.plotextractor.summary.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "sample_size": len(packets),
        "sample_arxiv_ids": [packet["arxiv_id"] for packet in packets],
        "tex_ok_count": sum(1 for packet in packets if packet.get("tex_status") == "ok"),
        "total_figures": len(figures),
        "total_captions": sum(1 for figure in figures if figure.get("caption")),
        "per_pdf": by_pdf,
    }
    corpus = {
        "schema_version": "m058.plotextractor.figure-caption-corpus.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "figure_count": len(figures),
        "figures": figures,
    }
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    (summary_path.parent / "figure-caption-corpus.json").write_text(
        json.dumps(corpus, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return summary


def run_extraction(
    *,
    samples: tuple[dict[str, str], ...] = DEFAULT_SAMPLE,
    per_pdf_dir: Path = DEFAULT_PER_PDF_DIR,
    summary_path: Path = DEFAULT_SUMMARY,
    tmp_root: Path = DEFAULT_TMP_ROOT,
    timeout_seconds: float = 30.0,
    retries: int = 3,
) -> dict[str, Any]:
    packets = [
        extract_one(
            sample,
            per_pdf_dir=per_pdf_dir,
            tmp_root=tmp_root,
            timeout_seconds=timeout_seconds,
            retries=retries,
        )
        for sample in samples
    ]
    return write_summary(packets, summary_path=summary_path)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--per-pdf-dir", type=Path, default=DEFAULT_PER_PDF_DIR)
    parser.add_argument("--summary", type=Path, default=DEFAULT_SUMMARY)
    parser.add_argument("--tmp-root", type=Path, default=DEFAULT_TMP_ROOT)
    parser.add_argument("--timeout-seconds", type=float, default=30.0)
    parser.add_argument("--retries", type=int, default=3)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    summary = run_extraction(
        per_pdf_dir=args.per_pdf_dir,
        summary_path=args.summary,
        tmp_root=args.tmp_root,
        timeout_seconds=args.timeout_seconds,
        retries=args.retries,
    )
    print(
        json.dumps(
            {
                "summary": str(args.summary),
                "sample_size": summary["sample_size"],
                "total_figures": summary["total_figures"],
                "total_captions": summary["total_captions"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
