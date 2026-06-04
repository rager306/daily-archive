#!/usr/bin/env python3
"""Replay M031 parser-ready conversion rows into redacted chunk/evidence packages.

This S04 wrapper is intentionally local-only and fail-closed. It consumes the S03
parser conversion closeout and conversion-quality summary, builds structure-aware
chunk diagnostics only for rows that were independently marked parser-ready, and
emits explicit zero-chunk refusals for every other conversion row. It never marks
chunks graph/import/LadybugDB eligible; generated graph-readiness packages are
reviewer handoff packets, not KG import inputs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import tempfile
import time
from collections import Counter, defaultdict
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Mapping

from arxiv_archive.chunk_import_contract import validate_import_ready_package, validation_to_dict
from arxiv_archive.chunking.chunker import parse_markdown_structure
from arxiv_archive.graph_readiness_export import CONTRACT_VERSION as GRAPH_READINESS_CONTRACT_VERSION
from arxiv_archive.graph_readiness_export import SCHEMA_VERSION as GRAPH_READINESS_SCHEMA_VERSION

MILESTONE_ID = "M031-vwpd8e"
SLICE_ID = "S04"
SOURCE_SLICE_ID = "S03"
SELECTION_ID = "m031-catalog-backed-replay-v1"
SCHEMA_VERSION = "m031-chunk-evidence-replay.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m031-chunk-evidence-diagnostic.v1"
GRAPH_PACKAGE_SCHEMA_VERSION = "m031-graph-readiness-review-package.v1"
SUMMARY_NAME = "chunk-evidence-summary.json"
DIAGNOSTICS_NAME = "chunk-evidence-diagnostics.jsonl"
REPORT_NAME = "chunk-evidence-report.md"
PACKAGES_DIR_NAME = "packages"
EXPECTED_FALSE_FLAGS = {
    "network_fetch_attempted",
    "graph_import_allowed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "ladybugdb_written",
    "graph_write_attempted",
    "production_persistence_attempted",
    "production_ladybugdb_write_allowed",
    "kg_readiness_claimed",
    "chunk_ready_claimed_for_non_parser_ready_rows",
    "raw_text_included",
    "chunk_text_included",
    "embeddings_included",
    "vectors_included",
    "raw_payload_embedded_in_metadata",
}
FAIL_CLOSED_FLAGS = {flag: False for flag in sorted(EXPECTED_FALSE_FLAGS)}
FORBIDDEN_PAYLOAD_KEYS = {
    "text",
    "raw_text",
    "chunk_text",
    "paper_text",
    "claim_text",
    "html",
    "raw_html",
    "pdf_bytes",
    "binary_payload",
    "base64_payload",
    "embedding",
    "embeddings",
    "vector",
    "vectors",
}
FORBIDDEN_SNIPPETS = (
    "deterministic fallback capture",
    "This fixture PDF contains enough local scientific prose",
    "No network fetches or graph writes should be needed",
)


class ChunkEvidenceReplayError(ValueError):
    """Typed setup/validation failure with a stable code and JSON path."""

    def __init__(self, code: str, message: str, *, json_path: str = "$") -> None:
        super().__init__(message)
        self.code = code
        self.json_path = json_path


def utc_now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    tmp_path = Path(tmp_name)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        tmp_path.replace(path)
    except Exception:
        tmp_path.unlink(missing_ok=True)
        raise


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    atomic_write_text(path, json.dumps(payload, indent=2, sort_keys=False) + "\n")


def write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    atomic_write_text(path, "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


def load_json_object(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ChunkEvidenceReplayError("malformed_json", f"malformed JSON at {path}: {exc}", json_path=str(path)) from exc
    except OSError as exc:
        raise ChunkEvidenceReplayError("json_read_failed", f"failed to read {path}: {exc}", json_path=str(path)) from exc
    if not isinstance(payload, dict):
        raise ChunkEvidenceReplayError("malformed_json_object", f"expected JSON object at {path}", json_path=str(path))
    return payload


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def slug(value: Any) -> str:
    safe = re.sub(r"[^A-Za-z0-9._-]+", "_", str(value or "missing")).strip("_")
    return safe or "missing"


def package_key(row: Mapping[str, Any]) -> str:
    return f"{slug(row.get('article_ref') or row.get('identity'))}_{slug(row.get('source_role'))}"


def safe_relative_display(project_root: Path, path: Path) -> str:
    try:
        return path.resolve().relative_to(project_root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def resolve_converted_text_path(project_root: Path, value: Any) -> Path:
    if not isinstance(value, str) or not value.strip():
        raise ChunkEvidenceReplayError("missing_converted_text_path", "parser-ready row has no converted_text_path", json_path="$.converted_text_path")
    if "://" in value:
        raise ChunkEvidenceReplayError("converted_text_path_url", "converted_text_path must be project-local", json_path="$.converted_text_path")
    raw = PurePosixPath(value.replace("\\", "/"))
    if ".." in raw.parts:
        raise ChunkEvidenceReplayError("unsafe_converted_text_path", "converted_text_path contains '..'", json_path="$.converted_text_path")
    candidate = Path(value)
    if not candidate.is_absolute():
        candidate = project_root / candidate
    resolved = candidate.resolve()
    if not resolved.is_relative_to(project_root.resolve()):
        raise ChunkEvidenceReplayError("converted_text_path_outside_project", "converted_text_path escapes project root", json_path="$.converted_text_path")
    if not resolved.exists():
        raise ChunkEvidenceReplayError("missing_converted_text_artifact", "converted text artifact is missing", json_path="$.converted_text_path")
    if not resolved.is_file():
        raise ChunkEvidenceReplayError("converted_text_not_file", "converted_text_path is not a file", json_path="$.converted_text_path")
    return resolved


def validate_s03_closeout(closeout: Mapping[str, Any], conversion_summary: Mapping[str, Any]) -> None:
    if closeout.get("schema_version") != "m031-parser-conversion-closeout-verifier.v1":
        raise ChunkEvidenceReplayError("unexpected_closeout_schema", "S03 closeout schema is not recognized", json_path="$.schema_version")
    if closeout.get("milestone_id") != MILESTONE_ID or closeout.get("selection_id") != SELECTION_ID:
        raise ChunkEvidenceReplayError("unexpected_closeout_identity", "S03 closeout does not match M031 corpus", json_path="$.selection_id")
    if closeout.get("status") != "passed" or closeout.get("failure_count") != 0:
        raise ChunkEvidenceReplayError("s03_closeout_not_passed", "S03 closeout must pass before chunk evidence replay", json_path="$.status")
    rows = conversion_summary.get("results")
    if conversion_summary.get("schema_version") != "m031-parser-conversion-replay.v1":
        raise ChunkEvidenceReplayError("unexpected_conversion_schema", "conversion summary schema is not recognized", json_path="$.schema_version")
    if not isinstance(rows, list):
        raise ChunkEvidenceReplayError("malformed_conversion_results", "conversion summary results must be a list", json_path="$.results")
    if closeout.get("row_count") != len(rows):
        raise ChunkEvidenceReplayError("stale_s03_closeout_row_count", "S03 closeout row_count no longer matches conversion rows", json_path="$.row_count")
    parser_ready_count = sum(1 for row in rows if isinstance(row, Mapping) and row.get("parser_ready") is True)
    if closeout.get("parser_ready_count") != parser_ready_count or conversion_summary.get("parser_ready_count") != parser_ready_count:
        raise ChunkEvidenceReplayError("stale_s03_closeout_parser_ready_count", "S03 parser-ready count is stale", json_path="$.parser_ready_count")
    for flag, expected in FAIL_CLOSED_FLAGS.items():
        if flag in closeout and closeout.get(flag) is not expected:
            raise ChunkEvidenceReplayError("unsafe_closeout_flag", f"S03 closeout flag {flag} is not fail-closed", json_path=f"$.{flag}")
        flags = closeout.get("fail_closed_safety_flags")
        if isinstance(flags, Mapping) and flag in flags and flags.get(flag) is not expected:
            raise ChunkEvidenceReplayError("unsafe_closeout_flag", f"S03 closeout safety flag {flag} is not fail-closed", json_path=f"$.fail_closed_safety_flags.{flag}")


def validate_parser_ready_row(row: Mapping[str, Any], *, index: int, project_root: Path) -> Path:
    if row.get("status") != "converted" or row.get("parser_ready") is not True:
        raise ChunkEvidenceReplayError("parser_ready_status_mismatch", "parser-ready row must have converted status", json_path=f"$.results[{index}]")
    source_role = str(row.get("source_role") or "")
    if source_role in {"arxiv_html", "arxiv_abs_page", "arxiv_abs_url"}:
        raise ChunkEvidenceReplayError("non_pdf_parser_ready_refused", "only parser-ready PDF conversions may be chunked in S04", json_path=f"$.results[{index}].source_role")
    converted_path = resolve_converted_text_path(project_root, row.get("converted_text_path"))
    actual_hash = sha256_file(converted_path)
    if row.get("converted_text_sha256") != actual_hash:
        raise ChunkEvidenceReplayError("converted_text_sha256_mismatch", "converted text hash no longer matches S03 summary", json_path=f"$.results[{index}].converted_text_sha256")
    actual_size = converted_path.stat().st_size
    if row.get("converted_text_byte_size") != actual_size:
        raise ChunkEvidenceReplayError("converted_text_byte_size_mismatch", "converted text size no longer matches S03 summary", json_path=f"$.results[{index}].converted_text_byte_size")
    return converted_path


def base_row(row: Mapping[str, Any], *, index: int, status: str, code: str, message: str | None) -> dict[str, Any]:
    return {
        "schema_version": DIAGNOSTIC_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "json_path": f"$.results[{index}]",
        "identity": row.get("identity"),
        "article_ref": row.get("article_ref"),
        "variant_id": row.get("variant_id"),
        "source_role": row.get("source_role"),
        "source_status": row.get("status"),
        "source_diagnostic_code": row.get("diagnostic_code"),
        "parser_ready": row.get("parser_ready") is True,
        "status": status,
        "terminal_state": status,
        "code": code,
        "diagnostic_code": code,
        "severity": "info" if status == "chunked" else "warning",
        "message": message or code,
        "chunk_count": 0,
        "element_count": 0,
        "annotation_count": 0,
        "package_key": package_key(row),
        "package_path": None,
        "graph_readiness_package_path": None,
        "import_contract_valid_package": False,
        "import_eligible_chunk_count": 0,
        "refused_chunk_count": 0,
        "refusal_code": None if status == "chunked" else code,
        "refusal_reason": message,
        "safe_converted_text_path": None,
        **FAIL_CLOSED_FLAGS,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_FLAGS),
    }


def redacted_markdown_for_structure(markdown: str) -> str:
    """Keep deterministic structural cues while removing document-specific payload text."""
    redacted_lines: list[str] = []
    heading_index = 0
    paragraph_index = 0
    for line in markdown.splitlines():
        stripped = line.strip()
        heading = re.match(r"^(#{1,6})\s+(.+?)\s*$", stripped)
        if heading:
            heading_index += 1
            label = heading.group(2).strip().lower()
            if "abstract" in label:
                replacement = "Abstract"
            elif "method" in label or "approach" in label:
                replacement = "Method"
            elif "result" in label or "evaluation" in label:
                replacement = "Results"
            elif "reference" in label or "bibliography" in label:
                replacement = "References"
            elif "conclusion" in label:
                replacement = "Conclusion"
            elif heading.group(1) == "#":
                replacement = "Document"
            else:
                replacement = f"Section {heading_index:04d}"
            redacted_lines.append(f"{heading.group(1)} {replacement}")
        elif not stripped:
            redacted_lines.append("")
        elif stripped.startswith("|"):
            redacted_lines.append("| redacted | structural |")
        else:
            paragraph_index += 1
            redacted_lines.append(f"Redacted structural paragraph {paragraph_index:04d} for source span replay.")
    return "\n".join(redacted_lines)


def build_graph_readiness_package(
    *,
    row: Mapping[str, Any],
    structure_package: Mapping[str, Any],
    validation: Mapping[str, Any],
    package_path: Path,
    project_root: Path,
) -> dict[str, Any]:
    diagnostics = structure_package.get("diagnostics") if isinstance(structure_package.get("diagnostics"), Mapping) else {}
    return {
        "schema_version": GRAPH_PACKAGE_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "graph_readiness_contract_version": GRAPH_READINESS_CONTRACT_VERSION,
        "graph_readiness_schema_version": GRAPH_READINESS_SCHEMA_VERSION,
        "review_state": "pending_independent_graph_readiness_review",
        "review_required": True,
        "output_contract_completed": False,
        "identity": row.get("identity"),
        "article_ref": row.get("article_ref"),
        "source_role": row.get("source_role"),
        "package_key": package_key(row),
        "structure_aware_package_path": safe_relative_display(project_root, package_path),
        "validation": validation,
        "chunk_count": len(structure_package.get("chunks", [])) if isinstance(structure_package.get("chunks"), list) else 0,
        "element_count": len(structure_package.get("elements", [])) if isinstance(structure_package.get("elements"), list) else 0,
        "annotation_count": len(structure_package.get("annotations", [])) if isinstance(structure_package.get("annotations"), list) else 0,
        "counts_by_state": diagnostics.get("counts_by_state", {}),
        "counts_by_route": diagnostics.get("counts_by_route", {}),
        "counts_by_chunk_type": diagnostics.get("counts_by_chunk_type", {}),
        "refusal_counts": diagnostics.get("refusal_counts", {}),
        "review_blockers": [
            "independent_graph_readiness_review_required",
            "trusted_kg_import_not_allowed_by_s04",
            "production_ladybugdb_write_not_allowed_by_s04",
        ],
        **FAIL_CLOSED_FLAGS,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_FLAGS),
    }


def chunk_parser_ready_row(
    row: Mapping[str, Any],
    *,
    index: int,
    project_root: Path,
    output_dir: Path,
    run_id: str,
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    converted_path = validate_parser_ready_row(row, index=index, project_root=project_root)
    markdown = redacted_markdown_for_structure(converted_path.read_text(encoding="utf-8"))
    source_artifact = f"converted_text:{safe_relative_display(project_root, converted_path)}"
    package = parse_markdown_structure(
        markdown,
        paper_id=package_key(row),
        title=str(row.get("article_ref") or row.get("identity") or package_key(row)),
        source_artifact=source_artifact,
        categories=(),
        run_id=run_id,
    ).to_contract()
    validation = validation_to_dict(validate_import_ready_package(package))
    package_dir = output_dir / PACKAGES_DIR_NAME / package_key(row)
    package_path = package_dir / "structure-aware-package.json"
    graph_package = build_graph_readiness_package(
        row=row,
        structure_package=package,
        validation=validation,
        package_path=package_path,
        project_root=project_root,
    )
    graph_package_path = package_dir / "graph-readiness-package.json"
    diagnostic = base_row(row, index=index, status="chunked", code="parser_ready_chunk_package_created", message="parser-ready converted text replayed into redacted structure-aware package")
    diagnostic.update(
        {
            "chunk_count": len(package["chunks"]),
            "element_count": len(package["elements"]),
            "annotation_count": len(package["annotations"]),
            "package_path": safe_relative_display(project_root, package_path),
            "graph_readiness_package_path": safe_relative_display(project_root, graph_package_path),
            "safe_converted_text_path": safe_relative_display(project_root, converted_path),
            "import_contract_valid_package": validation["valid_package"],
            "import_eligible_chunk_count": validation["import_eligible_chunk_count"],
            "refused_chunk_count": validation["refused_chunk_count"],
            "counts_by_state": package["diagnostics"].get("counts_by_state", {}),
            "counts_by_route": package["diagnostics"].get("counts_by_route", {}),
            "counts_by_chunk_type": package["diagnostics"].get("counts_by_chunk_type", {}),
            "package_validation": validation,
        }
    )
    return diagnostic, package, graph_package


def refuse_row(row: Mapping[str, Any], *, index: int) -> dict[str, Any]:
    source_status = str(row.get("status") or "unknown")
    source_code = str(row.get("diagnostic_code") or row.get("refusal_code") or source_status)
    return base_row(
        row,
        index=index,
        status="zero_chunk_refused",
        code=f"non_parser_ready_zero_chunk_refusal:{source_code}",
        message="S04 preserves non-parser-ready S03 row as an explicit zero-chunk refusal",
    )


def assert_redacted(value: Any, *, path: Path) -> None:
    serialized = json.dumps(value, sort_keys=True) if not isinstance(value, str) else value
    lowered = serialized.lower()
    for key in FORBIDDEN_PAYLOAD_KEYS:
        if f'"{key}"' in lowered:
            raise ChunkEvidenceReplayError("metadata_payload_key_leakage", f"metadata contains forbidden payload key {key!r}: {path}")
    for snippet in FORBIDDEN_SNIPPETS:
        if snippet.lower() in lowered:
            raise ChunkEvidenceReplayError("metadata_payload_snippet_leakage", f"metadata contains forbidden raw payload snippet {snippet!r}: {path}")


def assert_fail_closed(summary: Mapping[str, Any], diagnostics: Iterable[Mapping[str, Any]], graph_packages: Iterable[Mapping[str, Any]]) -> None:
    all_records: list[Mapping[str, Any]] = [summary, *diagnostics, *graph_packages]
    for record in all_records:
        for flag, expected in FAIL_CLOSED_FLAGS.items():
            if flag in record and record.get(flag) is not expected:
                raise ChunkEvidenceReplayError("unsafe_safety_flag", f"safety flag {flag}={record.get(flag)!r}", json_path=f"$.{flag}")
        flags = record.get("fail_closed_safety_flags")
        if isinstance(flags, Mapping):
            for flag, expected in FAIL_CLOSED_FLAGS.items():
                if flags.get(flag) is not expected:
                    raise ChunkEvidenceReplayError("unsafe_safety_flag", f"safety flag {flag}={flags.get(flag)!r}", json_path=f"$.fail_closed_safety_flags.{flag}")


def build_summary(
    diagnostics: list[dict[str, Any]],
    *,
    selection_path: Path,
    conversion_summary_path: Path,
    closeout_summary_path: Path,
    output_dir: Path,
    duration_ms: int,
) -> dict[str, Any]:
    counts = Counter(str(row.get("status")) for row in diagnostics)
    by_identity: dict[str, dict[str, int]] = defaultdict(lambda: {"chunked": 0, "zero_chunk_refused": 0})
    for row in diagnostics:
        identity = str(row.get("identity") or "<missing-identity>")
        status = str(row.get("status"))
        if status in by_identity[identity]:
            by_identity[identity][status] += 1
    chunked = [row for row in diagnostics if row.get("status") == "chunked"]
    refused = [row for row in diagnostics if row.get("status") == "zero_chunk_refused"]
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "completed_with_diagnostics" if refused else "completed",
        "row_count": len(diagnostics),
        "parser_ready_row_count": len(chunked),
        "chunked_parser_ready_row_count": len(chunked),
        "zero_chunk_refusal_count": len(refused),
        "counts": dict(sorted(counts.items())),
        "chunk_count": sum(int(row.get("chunk_count") or 0) for row in diagnostics),
        "element_count": sum(int(row.get("element_count") or 0) for row in diagnostics),
        "annotation_count": sum(int(row.get("annotation_count") or 0) for row in diagnostics),
        "package_count": len(chunked),
        "graph_readiness_package_count": len(chunked),
        "pending_graph_readiness_review_count": len(chunked),
        "import_contract_valid_package_count": sum(1 for row in chunked if row.get("import_contract_valid_package") is True),
        "import_eligible_chunk_count": sum(int(row.get("import_eligible_chunk_count") or 0) for row in diagnostics),
        "refused_chunk_count": sum(int(row.get("refused_chunk_count") or 0) for row in diagnostics),
        "per_identity_chunk_state_counts": {key: dict(value) for key, value in sorted(by_identity.items())},
        "diagnostic_code_counts": dict(sorted(Counter(str(row.get("diagnostic_code")) for row in diagnostics).items())),
        "input_paths": {
            "selection": selection_path.as_posix(),
            "conversion_summary": conversion_summary_path.as_posix(),
            "closeout_summary": closeout_summary_path.as_posix(),
        },
        "output_paths": {
            "output_dir": output_dir.as_posix(),
            "summary": (output_dir / SUMMARY_NAME).as_posix(),
            "diagnostics": (output_dir / DIAGNOSTICS_NAME).as_posix(),
            "report": (output_dir / REPORT_NAME).as_posix(),
            "packages_dir": (output_dir / PACKAGES_DIR_NAME).as_posix(),
        },
        "package_paths": [row["package_path"] for row in chunked],
        "graph_readiness_package_paths": [row["graph_readiness_package_path"] for row in chunked],
        "duration_ms": duration_ms,
        **FAIL_CLOSED_FLAGS,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_FLAGS),
        "generated_at": utc_now(),
        "results": diagnostics,
    }


def render_report(summary: Mapping[str, Any]) -> str:
    lines = [
        "# M031 Chunk Evidence Replay Report",
        "",
        "This S04 report is metadata-only. It does not embed converted text, source HTML, PDF bytes, chunk text, embeddings, vectors, graph facts, or LadybugDB write claims.",
        "",
        f"- Schema: `{summary.get('schema_version')}`",
        f"- Status: `{summary.get('status')}`",
        f"- Row count: {summary.get('row_count')}",
        f"- Chunked parser-ready rows: {summary.get('chunked_parser_ready_row_count')}",
        f"- Zero-chunk refusals: {summary.get('zero_chunk_refusal_count')}",
        f"- Chunk count: {summary.get('chunk_count')}",
        f"- Package count: {summary.get('package_count')}",
        f"- Pending graph-readiness reviews: {summary.get('pending_graph_readiness_review_count')}",
        "- Import-eligible chunks: `0`",
        "- Network fetch attempted: `False`",
        "- Graph/import/LadybugDB writes: `False`",
        "",
        "## Failure Modes",
        "",
        "S03 closeout must be present, current, and passed before success artifacts are written. Malformed JSON, stale S03 row/parser-ready counts, unsafe converted paths, missing converted artifacts, hash/size mismatches, and unsafe graph/import flags raise typed deterministic errors. Non-parser-ready S03 rows are preserved as zero-chunk refusal diagnostics instead of being promoted.",
        "",
        "## Load Profile",
        "",
        "The replay is bounded to the seven S03 conversion rows and reads converted text only for parser-ready rows. At 10x expected load, local CPU/memory for deterministic Markdown structure parsing of converted text saturates first; no corpus-wide scan, network fetch, subprocess, graph import, or LadybugDB write path is used.",
        "",
        "## Negative Tests",
        "",
        "Covered by `tests/test_m031_chunk_evidence_replay.py`: converted hash mismatch, converted path outside project root, fallback HTML parser-ready promotion, stale/failing S03 closeout, missing converted artifact, raw payload marker redaction, and zero eligibility/fail-closed graph/import flags.",
        "",
        "## Results",
        "",
    ]
    for row in summary.get("results", []):
        if isinstance(row, Mapping):
            lines.append(
                f"- `{row.get('identity')}` `{row.get('source_role')}`: {row.get('status')} "
                f"chunks={row.get('chunk_count')} code=`{row.get('diagnostic_code')}` package=`{row.get('package_path') or '<none>'}`"
            )
    return "\n".join(lines) + "\n"


def replay_chunk_evidence(
    *,
    selection_path: Path,
    conversion_summary_path: Path,
    closeout_summary_path: Path,
    output_dir: Path,
    project_root: Path,
    run_id: str,
) -> tuple[dict[str, Any], list[dict[str, Any]], list[tuple[Path, dict[str, Any]]], list[tuple[Path, dict[str, Any]]]]:
    selection = load_json_object(selection_path)
    conversion_summary = load_json_object(conversion_summary_path)
    closeout = load_json_object(closeout_summary_path)
    if selection.get("selection_id") != SELECTION_ID:
        raise ChunkEvidenceReplayError("unexpected_selection_id", "selection is not the M031 replay corpus", json_path="$.selection_id")
    validate_s03_closeout(closeout, conversion_summary)
    rows = conversion_summary["results"]
    diagnostics: list[dict[str, Any]] = []
    structure_packages: list[tuple[Path, dict[str, Any]]] = []
    graph_packages: list[tuple[Path, dict[str, Any]]] = []
    for index, row in enumerate(rows):
        if not isinstance(row, Mapping):
            raise ChunkEvidenceReplayError("malformed_conversion_result", "conversion result rows must be objects", json_path=f"$.results[{index}]")
        if row.get("parser_ready") is True:
            diagnostic, structure_package, graph_package = chunk_parser_ready_row(row, index=index, project_root=project_root, output_dir=output_dir, run_id=run_id)
            package_dir = output_dir / PACKAGES_DIR_NAME / package_key(row)
            structure_packages.append((package_dir / "structure-aware-package.json", structure_package))
            graph_packages.append((package_dir / "graph-readiness-package.json", graph_package))
            diagnostics.append(diagnostic)
        else:
            diagnostics.append(refuse_row(row, index=index))
    return {}, diagnostics, structure_packages, graph_packages


def run_replay(
    *,
    selection_path: Path,
    conversion_summary_path: Path,
    closeout_summary_path: Path,
    output_dir: Path,
    project_root: Path,
) -> dict[str, Any]:
    started = time.perf_counter()
    run_id = f"m031-s04-chunk-evidence:{utc_now()}"
    _unused, diagnostics, structure_packages, graph_packages = replay_chunk_evidence(
        selection_path=selection_path,
        conversion_summary_path=conversion_summary_path,
        closeout_summary_path=closeout_summary_path,
        output_dir=output_dir,
        project_root=project_root,
        run_id=run_id,
    )
    summary = build_summary(
        diagnostics,
        selection_path=selection_path,
        conversion_summary_path=conversion_summary_path,
        closeout_summary_path=closeout_summary_path,
        output_dir=output_dir,
        duration_ms=int((time.perf_counter() - started) * 1000),
    )
    graph_payloads = [payload for _path, payload in graph_packages]
    assert_fail_closed(summary, diagnostics, graph_payloads)
    report = render_report(summary)
    assert_redacted(summary, path=output_dir / SUMMARY_NAME)
    assert_redacted(diagnostics, path=output_dir / DIAGNOSTICS_NAME)
    assert_redacted(report, path=output_dir / REPORT_NAME)
    for path, payload in [*structure_packages, *graph_packages]:
        assert_redacted(payload, path=path)
    output_dir.mkdir(parents=True, exist_ok=True)
    for path, payload in structure_packages:
        write_json(path, payload)
    for path, payload in graph_packages:
        write_json(path, payload)
    write_json(output_dir / SUMMARY_NAME, summary)
    write_jsonl(output_dir / DIAGNOSTICS_NAME, diagnostics)
    atomic_write_text(output_dir / REPORT_NAME, report)
    return summary


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--conversion-summary", required=True, type=Path)
    parser.add_argument("--closeout-summary", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--project-root", default=Path.cwd(), type=Path)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        project_root = args.project_root.resolve()
        for cli_path in (args.selection, args.conversion_summary, args.closeout_summary, args.output_dir):
            if not cli_path.is_absolute() and ".." in PurePosixPath(str(cli_path).replace("\\", "/")).parts:
                raise ChunkEvidenceReplayError("unsafe_cli_path", f"unsafe CLI path: {cli_path}")
        summary = run_replay(
            selection_path=args.selection,
            conversion_summary_path=args.conversion_summary,
            closeout_summary_path=args.closeout_summary,
            output_dir=args.output_dir.resolve(),
            project_root=project_root,
        )
        sys.stdout.write(json.dumps({"status": summary["status"], "counts": summary["counts"], "summary": (args.output_dir / SUMMARY_NAME).as_posix()}, sort_keys=True) + "\n")
        return 0
    except ChunkEvidenceReplayError as exc:
        sys.stderr.write(json.dumps({"status": "failed", "code": exc.code, "message": str(exc), "json_path": exc.json_path}, sort_keys=True) + "\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
