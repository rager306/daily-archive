"""Bounded source acquisition helpers for M006 thirty-paper deviation scan.

The helpers in this module acquire or audit Markdown availability for selected
papers. They intentionally emit redacted metadata only: no raw paper text,
chunk text, embeddings, vectors, or production write signals are serialized.


Formerly: src/arxiv_archive/thirty_paper_source_scan.py"""

from __future__ import annotations

import asyncio
import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

from research_graph.infrastructure.corpus.ingestion import assess_full_text_quality
from research_graph.infrastructure.corpus.sources.markdown_converter import (
    ConversionResult,
    MDConverter,
)


@dataclass(frozen=True)
class AcquisitionPaths:
    """Local roots used by a source acquisition run."""

    research_papers_dir: Path
    arxiv_cache_dir: Path


class MarkdownConverter(Protocol):
    async def convert(self, arxiv_id: str) -> ConversionResult: ...

    async def close(self) -> None: ...


SAFETY_FLAGS: dict[str, bool] = {
    "raw_text_included": False,
    "chunk_text_included": False,
    "raw_binary_included": False,
    "base64_included": False,
    "embeddings_included": False,
    "vectors_included": False,
    "secrets_included": False,
    "optimizer_traces_included": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
}


def missing_markdown_paper_ids(manifest: dict[str, Any]) -> list[str]:
    """Return selected paper ids that the manifest marks as missing Markdown."""
    ids: list[str] = []
    for paper in manifest.get("papers", []):
        if not isinstance(paper, dict):
            continue
        availability = paper.get("availability")
        if not isinstance(availability, dict):
            continue
        if not availability.get("available_markdown"):
            ids.append(str(paper["paper_id"]))
    return ids


async def acquire_sources_for_manifest(
    *,
    manifest_path: str | Path,
    output_dir: str | Path,
    paths: AcquisitionPaths | None = None,
    converter: MarkdownConverter | None = None,
    fast_only: bool = False,
) -> dict[str, Path]:
    """Attempt bounded Markdown acquisition for papers missing Markdown.

    The run writes a summary JSON and diagnostics JSONL. Successful conversion
    writes Markdown to the local research paper workspace and relies on the
    converter cache for cache Markdown/PDF artifacts.
    """
    manifest_file = Path(manifest_path)
    manifest = _read_json_object(manifest_file)
    paths = paths or AcquisitionPaths(
        research_papers_dir=Path.home() / ".research" / "papers",
        arxiv_cache_dir=Path.home() / ".arxiv_cache",
    )
    output = Path(output_dir)
    output.mkdir(parents=True, exist_ok=True)
    owns_converter = converter is None
    converter = converter or MDConverter()
    try:
        missing_ids = set(missing_markdown_paper_ids(manifest))
        records: list[dict[str, Any]] = []
        for paper in manifest.get("papers", []):
            if not isinstance(paper, dict):
                continue
            paper_id = str(paper["paper_id"])
            before = _availability_for(paper_id, paths)
            attempted = paper_id in missing_ids and not before["available_markdown"]
            conversion_method: str | None = None
            conversion_error: str | None = None
            quality: dict[str, Any] | None = None
            output_markdown_path: str | None = None
            if attempted:
                result = await _convert_bounded(converter, paper_id, fast_only=fast_only)
                conversion_method = result.method
                if result.markdown is not None:
                    quality_report = assess_full_text_quality(result.markdown)
                    quality = _quality_metadata(quality_report)
                    if quality_report.status == "ok":
                        destination = paths.research_papers_dir / paper_id / "full_text.md"
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        destination.write_text(result.markdown, encoding="utf-8")
                        output_markdown_path = str(destination)
                    else:
                        conversion_error = f"low_quality_markdown:{quality_report.fallback_reason}"
                else:
                    conversion_error = _redact_error(result.error)
            after = _availability_for(paper_id, paths)
            records.append(
                {
                    "paper_id": paper_id,
                    "rank": paper.get("rank"),
                    "selection_role": paper.get("selection_role"),
                    "attempted": attempted,
                    "before": before,
                    "after": after,
                    "outcome": _outcome(
                        attempted=attempted, before=before, after=after, error=conversion_error
                    ),
                    "conversion_method": conversion_method,
                    "conversion_error": conversion_error,
                    "quality": quality,
                    "output_markdown_path": output_markdown_path,
                    "cache_pdf_path": str(paths.arxiv_cache_dir / f"{paper_id}.pdf")
                    if (paths.arxiv_cache_dir / f"{paper_id}.pdf").exists()
                    else None,
                    **SAFETY_FLAGS,
                }
            )
    finally:
        if owns_converter:
            await converter.close()

    summary = _summary_from_records(manifest=manifest, records=records)
    summary_path = output / "source-acquisition-summary.json"
    diagnostics_path = output / "source-acquisition-diagnostics.jsonl"
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    with diagnostics_path.open("w", encoding="utf-8") as handle:
        for record in records:
            handle.write(json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n")
    return {"summary_path": summary_path, "diagnostics_path": diagnostics_path}


def acquire_sources_for_manifest_sync(**kwargs: Any) -> dict[str, Path]:
    """Synchronous wrapper for command-line/script use."""
    try:
        asyncio.get_running_loop()
    except RuntimeError:
        return asyncio.run(acquire_sources_for_manifest(**kwargs))
    raise RuntimeError(
        "acquire_sources_for_manifest_sync() cannot run inside an active event loop; "
        "await acquire_sources_for_manifest() instead"
    )


async def _convert_bounded(
    converter: MarkdownConverter, paper_id: str, *, fast_only: bool
) -> ConversionResult:
    """Run a bounded conversion attempt.

    `MDConverter.convert()` may fall through to PDF/Docling conversion, which can
    be too slow for a 20-paper discovery batch. In fast-only mode, use the
    converter's arxiv2md path when available and record failures as diagnostics
    instead of invoking PDF fallback.
    """
    if fast_only and hasattr(converter, "_try_arxiv2md"):
        return await converter._try_arxiv2md(paper_id)  # type: ignore[attr-defined]  # ty:ignore[call-non-callable]
    return await converter.convert(paper_id)


def _summary_from_records(
    *, manifest: dict[str, Any], records: list[dict[str, Any]]
) -> dict[str, Any]:
    attempted = [record for record in records if record["attempted"]]
    originally_missing = [
        record
        for record in records
        if _manifest_marked_missing_markdown(manifest=manifest, paper_id=str(record["paper_id"]))
    ]
    preexisting_ready = [
        record
        for record in originally_missing
        if record["before"]["available_markdown"] and not record["attempted"]
    ]
    succeeded = [record for record in attempted if record["outcome"] == "acquired_markdown"]
    still_missing = [record for record in records if not record["after"]["available_markdown"]]
    method_counts: dict[str, int] = {}
    outcome_counts: dict[str, int] = {}
    for record in records:
        outcome_counts[record["outcome"]] = outcome_counts.get(record["outcome"], 0) + 1
        method = record.get("conversion_method")
        if method:
            method_counts[str(method)] = method_counts.get(str(method), 0) + 1
    return {
        "schema_version": "m006-source-acquisition-summary.v1",
        "milestone": "M006-638rza",
        "slice": "S02",
        "paper_count": len(records),
        "m005_overlap_count": manifest.get("m005_overlap_count"),
        "expansion_count": manifest.get("expansion_count"),
        "attempted_missing_markdown_count": len(attempted),
        "originally_missing_markdown_count": len(originally_missing),
        "preexisting_markdown_ready_from_original_missing_count": len(preexisting_ready),
        "acquired_markdown_count": len(succeeded),
        "ready_for_markdown_scan_count": sum(
            1 for record in records if record["after"]["available_markdown"]
        ),
        "still_missing_markdown_count": len(still_missing),
        "available_pdf_count": sum(1 for record in records if record["after"]["available_pdf"]),
        "method_counts": dict(sorted(method_counts.items())),
        "outcome_counts": dict(sorted(outcome_counts.items())),
        **SAFETY_FLAGS,
    }


def _manifest_marked_missing_markdown(*, manifest: dict[str, Any], paper_id: str) -> bool:
    for paper in manifest.get("papers", []):
        if not isinstance(paper, dict) or str(paper.get("paper_id")) != paper_id:
            continue
        availability = paper.get("availability")
        return isinstance(availability, dict) and not bool(availability.get("available_markdown"))
    return False


def _availability_for(paper_id: str, paths: AcquisitionPaths) -> dict[str, Any]:
    research_dir = paths.research_papers_dir / paper_id
    full_text = research_dir / "full_text.md"
    cache_md = paths.arxiv_cache_dir / f"{paper_id}.md"
    cache_pdf = paths.arxiv_cache_dir / f"{paper_id}.pdf"
    paper_json = research_dir / "paper.json"
    return {
        "research_workspace": research_dir.exists(),
        "research_full_text_md": full_text.exists(),
        "cache_markdown": cache_md.exists(),
        "cache_pdf": cache_pdf.exists(),
        "paper_json": paper_json.exists(),
        "available_markdown": full_text.exists() or cache_md.exists(),
        "available_pdf": cache_pdf.exists(),
        "full_text_md": _file_metadata(full_text),
        "cache_markdown_file": _file_metadata(cache_md),
        "cache_pdf_file": _file_metadata(cache_pdf),
    }


def _file_metadata(path: Path) -> dict[str, Any] | None:
    if not path.exists() or not path.is_file():
        return None
    content = path.read_bytes()
    return {
        "path": str(path),
        "byte_size": len(content),
        "sha256": hashlib.sha256(content).hexdigest(),
    }


def _quality_metadata(quality_report: Any) -> dict[str, Any]:
    return {
        "status": quality_report.status,
        "char_count": quality_report.char_count,
        "line_count": quality_report.line_count,
        "heading_count": quality_report.heading_count,
        "non_heading_nonempty_line_count": quality_report.non_heading_nonempty_line_count,
        "fallback_reason": quality_report.fallback_reason,
        "warning_count": len(quality_report.warnings),
    }


def _outcome(
    *, attempted: bool, before: dict[str, Any], after: dict[str, Any], error: str | None
) -> str:
    if before["available_markdown"]:
        return "already_markdown_ready"
    if not attempted:
        return "not_attempted"
    if after["available_markdown"]:
        return "acquired_markdown"
    if error:
        return "conversion_failed"
    return "still_missing_markdown"


def _redact_error(error: str | None) -> str | None:
    if error is None:
        return None
    collapsed = " ".join(error.split())
    return collapsed[:240]


def _read_json_object(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object at {path}")
    return value
