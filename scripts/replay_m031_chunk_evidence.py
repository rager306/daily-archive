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
from collections.abc import Iterable, Mapping
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

from research_graph.infrastructure.graph.readiness.export import (
    CONTRACT_VERSION as GRAPH_READINESS_CONTRACT_VERSION,
)
from research_graph.infrastructure.graph.readiness.export import (
    SCHEMA_VERSION as GRAPH_READINESS_SCHEMA_VERSION,
)
from research_graph.infrastructure.graph.readiness.review import (
    generate_review_bundles,
    validate_review_artifacts,
)
from research_graph.infrastructure.papers.chunking.chunker import parse_markdown_structure
from research_graph.infrastructure.repair.chunk_import_contract import (
    validate_import_ready_package,
    validation_to_dict,
)

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
REVIEW_CORPUS_NAME = "review-corpus.json"
INDEPENDENT_REVIEW_EVENTS_NAME = "independent-review-events.jsonl"
GRAPH_READINESS_REVIEW_DIR_NAME = "graph-readiness-review"
INDEPENDENT_REVIEW_SUMMARY_NAME = "independent-review-summary.md"
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
FAIL_CLOSED_FLAGS = dict.fromkeys(sorted(EXPECTED_FALSE_FLAGS), False)
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
        raise ChunkEvidenceReplayError(
            "malformed_json", f"malformed JSON at {path}: {exc}", json_path=str(path)
        ) from exc
    except OSError as exc:
        raise ChunkEvidenceReplayError(
            "json_read_failed", f"failed to read {path}: {exc}", json_path=str(path)
        ) from exc
    if not isinstance(payload, dict):
        raise ChunkEvidenceReplayError(
            "malformed_json_object", f"expected JSON object at {path}", json_path=str(path)
        )
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
        raise ChunkEvidenceReplayError(
            "missing_converted_text_path",
            "parser-ready row has no converted_text_path",
            json_path="$.converted_text_path",
        )
    if "://" in value:
        raise ChunkEvidenceReplayError(
            "converted_text_path_url",
            "converted_text_path must be project-local",
            json_path="$.converted_text_path",
        )
    raw = PurePosixPath(value.replace("\\", "/"))
    if ".." in raw.parts:
        raise ChunkEvidenceReplayError(
            "unsafe_converted_text_path",
            "converted_text_path contains '..'",
            json_path="$.converted_text_path",
        )
    candidate = Path(value)
    if not candidate.is_absolute():
        candidate = project_root / candidate
    resolved = candidate.resolve()
    if not resolved.is_relative_to(project_root.resolve()):
        raise ChunkEvidenceReplayError(
            "converted_text_path_outside_project",
            "converted_text_path escapes project root",
            json_path="$.converted_text_path",
        )
    if not resolved.exists():
        raise ChunkEvidenceReplayError(
            "missing_converted_text_artifact",
            "converted text artifact is missing",
            json_path="$.converted_text_path",
        )
    if not resolved.is_file():
        raise ChunkEvidenceReplayError(
            "converted_text_not_file",
            "converted_text_path is not a file",
            json_path="$.converted_text_path",
        )
    return resolved


def validate_s03_closeout(
    closeout: Mapping[str, Any], conversion_summary: Mapping[str, Any]
) -> None:
    if closeout.get("schema_version") != "m031-parser-conversion-closeout-verifier.v1":
        raise ChunkEvidenceReplayError(
            "unexpected_closeout_schema",
            "S03 closeout schema is not recognized",
            json_path="$.schema_version",
        )
    if closeout.get("milestone_id") != MILESTONE_ID or closeout.get("selection_id") != SELECTION_ID:
        raise ChunkEvidenceReplayError(
            "unexpected_closeout_identity",
            "S03 closeout does not match M031 corpus",
            json_path="$.selection_id",
        )
    if closeout.get("status") != "passed" or closeout.get("failure_count") != 0:
        raise ChunkEvidenceReplayError(
            "s03_closeout_not_passed",
            "S03 closeout must pass before chunk evidence replay",
            json_path="$.status",
        )
    rows = conversion_summary.get("results")
    if conversion_summary.get("schema_version") != "m031-parser-conversion-replay.v1":
        raise ChunkEvidenceReplayError(
            "unexpected_conversion_schema",
            "conversion summary schema is not recognized",
            json_path="$.schema_version",
        )
    if not isinstance(rows, list):
        raise ChunkEvidenceReplayError(
            "malformed_conversion_results",
            "conversion summary results must be a list",
            json_path="$.results",
        )
    if closeout.get("row_count") != len(rows):
        raise ChunkEvidenceReplayError(
            "stale_s03_closeout_row_count",
            "S03 closeout row_count no longer matches conversion rows",
            json_path="$.row_count",
        )
    parser_ready_count = sum(
        1 for row in rows if isinstance(row, Mapping) and row.get("parser_ready") is True
    )
    if (
        closeout.get("parser_ready_count") != parser_ready_count
        or conversion_summary.get("parser_ready_count") != parser_ready_count
    ):
        raise ChunkEvidenceReplayError(
            "stale_s03_closeout_parser_ready_count",
            "S03 parser-ready count is stale",
            json_path="$.parser_ready_count",
        )
    for flag, expected in FAIL_CLOSED_FLAGS.items():
        if flag in closeout and closeout.get(flag) is not expected:
            raise ChunkEvidenceReplayError(
                "unsafe_closeout_flag",
                f"S03 closeout flag {flag} is not fail-closed",
                json_path=f"$.{flag}",
            )
        flags = closeout.get("fail_closed_safety_flags")
        if isinstance(flags, Mapping) and flag in flags and flags.get(flag) is not expected:
            raise ChunkEvidenceReplayError(
                "unsafe_closeout_flag",
                f"S03 closeout safety flag {flag} is not fail-closed",
                json_path=f"$.fail_closed_safety_flags.{flag}",
            )


def validate_parser_ready_row(row: Mapping[str, Any], *, index: int, project_root: Path) -> Path:
    if row.get("status") != "converted" or row.get("parser_ready") is not True:
        raise ChunkEvidenceReplayError(
            "parser_ready_status_mismatch",
            "parser-ready row must have converted status",
            json_path=f"$.results[{index}]",
        )
    source_role = str(row.get("source_role") or "")
    if source_role in {"arxiv_html", "arxiv_abs_page", "arxiv_abs_url"}:
        raise ChunkEvidenceReplayError(
            "non_pdf_parser_ready_refused",
            "only parser-ready PDF conversions may be chunked in S04",
            json_path=f"$.results[{index}].source_role",
        )
    converted_path = resolve_converted_text_path(project_root, row.get("converted_text_path"))
    actual_hash = sha256_file(converted_path)
    if row.get("converted_text_sha256") != actual_hash:
        raise ChunkEvidenceReplayError(
            "converted_text_sha256_mismatch",
            "converted text hash no longer matches S03 summary",
            json_path=f"$.results[{index}].converted_text_sha256",
        )
    actual_size = converted_path.stat().st_size
    if row.get("converted_text_byte_size") != actual_size:
        raise ChunkEvidenceReplayError(
            "converted_text_byte_size_mismatch",
            "converted text size no longer matches S03 summary",
            json_path=f"$.results[{index}].converted_text_byte_size",
        )
    return converted_path


def base_row(
    row: Mapping[str, Any], *, index: int, status: str, code: str, message: str | None
) -> dict[str, Any]:
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
        "review_status": "not_applicable_zero_chunk_refusal"
        if status != "chunked"
        else "pending_review",
        "independent_review_completed": False,
        "automated_state_is_structural_only": True,
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
            redacted_lines.append(
                f"Redacted structural paragraph {paragraph_index:04d} for source span replay."
            )
    return "\n".join(redacted_lines)


def build_graph_readiness_package(
    *,
    row: Mapping[str, Any],
    structure_package: Mapping[str, Any],
    validation: Mapping[str, Any],
    package_path: Path,
    project_root: Path,
) -> dict[str, Any]:
    diagnostics = (
        structure_package.get("diagnostics")
        if isinstance(structure_package.get("diagnostics"), Mapping)
        else {}
    )
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
        "chunk_count": len(structure_package.get("chunks", []))
        if isinstance(structure_package.get("chunks"), list)
        else 0,
        "element_count": len(structure_package.get("elements", []))
        if isinstance(structure_package.get("elements"), list)
        else 0,
        "annotation_count": len(structure_package.get("annotations", []))
        if isinstance(structure_package.get("annotations"), list)
        else 0,
        "counts_by_state": diagnostics.get("counts_by_state", {}),  # pyrefly: ignore[bad-assignment]
        "counts_by_route": diagnostics.get("counts_by_route", {}),  # pyrefly: ignore[bad-assignment]
        "counts_by_chunk_type": diagnostics.get("counts_by_chunk_type", {}),  # pyrefly: ignore[bad-assignment]
        "refusal_counts": diagnostics.get("refusal_counts", {}),  # pyrefly: ignore[bad-assignment]
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
    diagnostic = base_row(
        row,
        index=index,
        status="chunked",
        code="parser_ready_chunk_package_created",
        message="parser-ready converted text replayed into redacted structure-aware package",
    )
    diagnostic.update(
        {
            "chunk_count": len(package["chunks"]),
            "element_count": len(package["elements"]),
            "annotation_count": len(package["annotations"]),
            "package_path": safe_relative_display(project_root, package_path),
            "graph_readiness_package_path": safe_relative_display(project_root, graph_package_path),
            "safe_converted_text_path": safe_relative_display(project_root, converted_path),
            "review_status": "pending_review",
            "independent_review_completed": False,
            "automated_state_is_structural_only": True,
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
            raise ChunkEvidenceReplayError(
                "metadata_payload_key_leakage",
                f"metadata contains forbidden payload key {key!r}: {path}",
            )
    for snippet in FORBIDDEN_SNIPPETS:
        if snippet.lower() in lowered:
            raise ChunkEvidenceReplayError(
                "metadata_payload_snippet_leakage",
                f"metadata contains forbidden raw payload snippet {snippet!r}: {path}",
            )


def assert_fail_closed(
    summary: Mapping[str, Any],
    diagnostics: Iterable[Mapping[str, Any]],
    graph_packages: Iterable[Mapping[str, Any]],
) -> None:
    all_records: list[Mapping[str, Any]] = [summary, *diagnostics, *graph_packages]
    for record in all_records:
        for flag, expected in FAIL_CLOSED_FLAGS.items():
            if flag in record and record.get(flag) is not expected:
                raise ChunkEvidenceReplayError(
                    "unsafe_safety_flag",
                    f"safety flag {flag}={record.get(flag)!r}",
                    json_path=f"$.{flag}",
                )
        flags = record.get("fail_closed_safety_flags")
        if isinstance(flags, Mapping):
            for flag, expected in FAIL_CLOSED_FLAGS.items():
                if flags.get(flag) is not expected:
                    raise ChunkEvidenceReplayError(
                        "unsafe_safety_flag",
                        f"safety flag {flag}={flags.get(flag)!r}",
                        json_path=f"$.fail_closed_safety_flags.{flag}",
                    )


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
    by_identity: dict[str, dict[str, int]] = defaultdict(
        lambda: {"chunked": 0, "zero_chunk_refused": 0}
    )
    for row in diagnostics:
        identity = str(row.get("identity") or "<missing-identity>")
        status = str(row.get("status"))
        if status in by_identity[identity]:
            by_identity[identity][status] += 1
    chunked = [row for row in diagnostics if row.get("status") == "chunked"]
    refused = [row for row in diagnostics if row.get("status") == "zero_chunk_refused"]
    pending_review_count = sum(1 for row in chunked if row.get("review_status") == "pending_review")
    blocked_review_count = sum(
        1 for row in chunked if str(row.get("review_status", "")).startswith("review_blocked")
    )
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
        "pending_graph_readiness_review_count": pending_review_count,
        "graph_readiness_review_blocker_count": blocked_review_count,
        "independent_review_completed_count": sum(
            1 for row in diagnostics if row.get("independent_review_completed") is True
        ),
        "automated_state_is_structural_only": True,
        "import_contract_valid_package_count": sum(
            1 for row in chunked if row.get("import_contract_valid_package") is True
        ),
        "import_eligible_chunk_count": sum(
            int(row.get("import_eligible_chunk_count") or 0) for row in diagnostics
        ),
        "refused_chunk_count": sum(int(row.get("refused_chunk_count") or 0) for row in diagnostics),
        "per_identity_chunk_state_counts": {
            key: dict(value) for key, value in sorted(by_identity.items())
        },
        "diagnostic_code_counts": dict(
            sorted(Counter(str(row.get("diagnostic_code")) for row in diagnostics).items())
        ),
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
            "review_corpus": (output_dir / REVIEW_CORPUS_NAME).as_posix(),
            "independent_review_events": (output_dir / INDEPENDENT_REVIEW_EVENTS_NAME).as_posix(),
            "graph_readiness_review_dir": (
                output_dir.parent / GRAPH_READINESS_REVIEW_DIR_NAME
            ).as_posix(),
            "independent_review_summary": (
                output_dir.parent
                / GRAPH_READINESS_REVIEW_DIR_NAME
                / INDEPENDENT_REVIEW_SUMMARY_NAME
            ).as_posix(),
        },
        "package_paths": [row["package_path"] for row in chunked],
        "graph_readiness_package_paths": [row["graph_readiness_package_path"] for row in chunked],
        "review_corpus_path": None,
        "independent_review_events_path": None,
        "graph_readiness_review_paths": [],
        "independent_review_summary_path": None,
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
        "Covered by `tests/test_m031_chunk_evidence_replay.py`: converted hash mismatch, converted path outside project root, fallback HTML parser-ready promotion, stale/failing S03 closeout, missing converted artifact, missing graph-readiness package blocker event, deleted review Markdown verifier failure, stale placeholder/completed-verdict rejection, non-parser-ready review corpus refusal, raw payload marker redaction, and zero eligibility/fail-closed graph/import flags.",
        "",
        "## Graph-Readiness Review Handoff",
        "",
        f"- Review corpus: `{summary.get('review_corpus_path') or '<none>'}`",
        f"- Review events: `{summary.get('independent_review_events_path') or '<none>'}`",
        f"- Review summary: `{summary.get('independent_review_summary_path') or '<none>'}`",
        f"- Independent review completed: `{summary.get('independent_review_completed_count')}`",
        f"- Automated state is structural only: `{summary.get('automated_state_is_structural_only')}`",
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
) -> tuple[
    dict[str, Any],
    list[dict[str, Any]],
    list[tuple[Path, dict[str, Any]]],
    list[tuple[Path, dict[str, Any]]],
]:
    selection = load_json_object(selection_path)
    conversion_summary = load_json_object(conversion_summary_path)
    closeout = load_json_object(closeout_summary_path)
    if selection.get("selection_id") != SELECTION_ID:
        raise ChunkEvidenceReplayError(
            "unexpected_selection_id",
            "selection is not the M031 replay corpus",
            json_path="$.selection_id",
        )
    validate_s03_closeout(closeout, conversion_summary)
    rows = conversion_summary["results"]
    diagnostics: list[dict[str, Any]] = []
    structure_packages: list[tuple[Path, dict[str, Any]]] = []
    graph_packages: list[tuple[Path, dict[str, Any]]] = []
    for index, row in enumerate(rows):
        if not isinstance(row, Mapping):
            raise ChunkEvidenceReplayError(
                "malformed_conversion_result",
                "conversion result rows must be objects",
                json_path=f"$.results[{index}]",
            )
        if row.get("parser_ready") is True:
            diagnostic, structure_package, graph_package = chunk_parser_ready_row(
                row, index=index, project_root=project_root, output_dir=output_dir, run_id=run_id
            )
            package_dir = output_dir / PACKAGES_DIR_NAME / package_key(row)
            structure_packages.append(
                (package_dir / "structure-aware-package.json", structure_package)
            )
            graph_packages.append((package_dir / "graph-readiness-package.json", graph_package))
            diagnostics.append(diagnostic)
        else:
            diagnostics.append(refuse_row(row, index=index))
    return {}, diagnostics, structure_packages, graph_packages


def _sanitize_review_events(events_path: Path) -> None:
    sanitized: list[dict[str, Any]] = []
    for line_number, line in enumerate(
        events_path.read_text(encoding="utf-8").splitlines(), start=1
    ):
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as exc:
            raise ChunkEvidenceReplayError(
                "malformed_independent_review_event",
                f"malformed independent-review event at line {line_number}: {exc}",
                json_path=f"{events_path}:{line_number}",
            ) from exc
        if not isinstance(event, dict):
            raise ChunkEvidenceReplayError(
                "malformed_independent_review_event",
                f"independent-review event must be an object at line {line_number}",
                json_path=f"{events_path}:{line_number}",
            )
        if event.get("event") == "independent_review.verdict":
            raise ChunkEvidenceReplayError(
                "completed_review_event_fabricated",
                "S04 must not fabricate completed independent-review verdict events",
                json_path=f"{events_path}:{line_number}",
            )
        event["raw_text_included"] = False
        event["raw_text_scope"] = "not_in_json_events_review_markdown_only"
        event["output_contract_completed"] = False
        event["review_status"] = "pending_review"
        event["independent_review_completed"] = False
        event["automated_state_is_structural_only"] = True
        for flag in FAIL_CLOSED_FLAGS:
            event[flag] = False
        event["fail_closed_safety_flags"] = dict(FAIL_CLOSED_FLAGS)
        sanitized.append(event)
    write_jsonl(events_path, sanitized)


def _review_blocker_event(
    row: Mapping[str, Any], *, run_id: str, graph_package_path: Path, project_root: Path
) -> dict[str, Any]:
    return {
        "event": "independent_review.blocker",
        "run_id": run_id,
        "paper_id": row.get("package_key"),
        "package_key": row.get("package_key"),
        "review_status": "review_blocked_missing_graph_readiness_package",
        "diagnostic_code": "missing_graph_readiness_package",
        "message": "parser-ready chunk row did not have a graph-readiness package, so review generation was skipped for this row",
        "json_path": row.get("json_path"),
        "graph_readiness_package_path": safe_relative_display(project_root, graph_package_path),
        "raw_text_included": False,
        "output_contract_completed": False,
        "independent_review_completed": False,
        "automated_state_is_structural_only": True,
        **FAIL_CLOSED_FLAGS,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_FLAGS),
    }


def build_review_corpus(
    *,
    diagnostics: list[dict[str, Any]],
    output_dir: Path,
    project_root: Path,
    run_id: str,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    documents: list[dict[str, Any]] = []
    blocker_events: list[dict[str, Any]] = []
    for row in diagnostics:
        if row.get("status") != "chunked":
            continue
        graph_package_value = row.get("graph_readiness_package_path")
        converted_value = row.get("safe_converted_text_path")
        if not graph_package_value or not converted_value:
            row["review_status"] = "review_blocked_missing_package_evidence"
            blocker_events.append(
                _review_blocker_event(
                    row,
                    run_id=run_id,
                    graph_package_path=output_dir / "<missing>",
                    project_root=project_root,
                )
            )
            continue
        graph_package_path = project_root / str(graph_package_value)
        if not graph_package_path.exists():
            row["review_status"] = "review_blocked_missing_graph_readiness_package"
            row.pop("review_corpus_paper_id", None)
            row.pop("review_artifact_path", None)
            blocker_events.append(
                _review_blocker_event(
                    row,
                    run_id=run_id,
                    graph_package_path=graph_package_path,
                    project_root=project_root,
                )
            )
            continue
        graph_package = load_json_object(graph_package_path)
        if (
            graph_package.get("review_state") != "pending_independent_graph_readiness_review"
            or graph_package.get("output_contract_completed") is not False
        ):
            raise ChunkEvidenceReplayError(
                "unsafe_graph_readiness_package_review_state",
                "graph-readiness package is not a pending-review handoff",
                json_path=f"{graph_package_path}:$.review_state",
            )
        for flag, expected in FAIL_CLOSED_FLAGS.items():
            if graph_package.get(flag) is not expected:
                raise ChunkEvidenceReplayError(
                    "unsafe_graph_readiness_package_flag",
                    f"graph-readiness package flag {flag} is not fail-closed",
                    json_path=f"{graph_package_path}:$.{flag}",
                )
        converted_path = project_root / str(converted_value)
        if not converted_path.exists():
            raise ChunkEvidenceReplayError(
                "missing_review_source_artifact",
                "review source converted text is missing",
                json_path=str(converted_path),
            )
        paper_id = str(row.get("package_key"))
        row["review_status"] = "pending_review"
        row["independent_review_completed"] = False
        row["automated_state_is_structural_only"] = True
        row["review_corpus_paper_id"] = paper_id
        documents.append(
            {
                "rank": len(documents) + 1,
                "paper_id": paper_id,
                "title": str(row.get("article_ref") or row.get("identity") or paper_id),
                "paper_dir": str(converted_path.parent),
                "expected_full_text_path": str(converted_path),
                "source_slice_id": SOURCE_SLICE_ID,
                "graph_readiness_package_path": safe_relative_display(
                    project_root, graph_package_path
                ),
                "review_status": "pending_review",
                "independent_review_completed": False,
                "automated_state_is_structural_only": True,
                "import_eligible_count": 0,
                **FAIL_CLOSED_FLAGS,
                "fail_closed_safety_flags": dict(FAIL_CLOSED_FLAGS),
            }
        )
    corpus = {
        "schema_version": "m031-graph-readiness-review-corpus.v1",
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "run_id": run_id,
        "status": "pending_review" if documents else "no_review_documents",
        "document_count": len(documents),
        "parser_ready_document_count": len(documents),
        "review_status": "pending_review" if documents else "blocked_or_not_applicable",
        "independent_review_completed": False,
        "automated_state_is_structural_only": True,
        "import_eligible_count": 0,
        "documents": documents,
        **FAIL_CLOSED_FLAGS,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_FLAGS),
        "generated_at": utc_now(),
    }
    return corpus, blocker_events


def generate_review_handoff(
    *,
    summary: dict[str, Any],
    diagnostics: list[dict[str, Any]],
    output_dir: Path,
    review_dir: Path,
    project_root: Path,
    run_id: str,
) -> None:
    corpus, blocker_events = build_review_corpus(
        diagnostics=diagnostics, output_dir=output_dir, project_root=project_root, run_id=run_id
    )
    review_corpus_path = output_dir / REVIEW_CORPUS_NAME
    events_path = output_dir / INDEPENDENT_REVIEW_EVENTS_NAME
    write_json(review_corpus_path, corpus)
    if corpus["documents"]:
        result = generate_review_bundles(
            corpus_path=review_corpus_path,
            review_dir=review_dir,
            events_path=events_path,
            run_id=run_id,
            required_paper_ids=tuple(str(doc["paper_id"]) for doc in corpus["documents"]),
        )
        _sanitize_review_events(result.events_path)
        validation = validate_review_artifacts(
            review_dir=review_dir, events_path=events_path, require_completed_review=False
        )
        if not validation.ok:
            raise ChunkEvidenceReplayError(
                "generated_review_validation_failed",
                "; ".join(validation.diagnostics),
                json_path=str(review_dir),
            )
        summary["graph_readiness_review_paths"] = [
            safe_relative_display(project_root, path) for path in result.review_paths
        ]
        summary["independent_review_summary_path"] = safe_relative_display(
            project_root, result.summary_path
        )
        for row in diagnostics:
            if row.get("status") == "chunked" and row.get("review_status") == "pending_review":
                row["review_artifact_path"] = safe_relative_display(
                    project_root, review_dir / f"{row.get('package_key')}-review.md"
                )
    else:
        write_jsonl(events_path, blocker_events)
        summary["graph_readiness_review_paths"] = []
        summary["independent_review_summary_path"] = None
    if blocker_events:
        existing_events = (
            [
                json.loads(line)
                for line in events_path.read_text(encoding="utf-8").splitlines()
                if line.strip()
            ]
            if events_path.exists()
            else []
        )
        write_jsonl(events_path, [*existing_events, *blocker_events])
    summary["review_corpus_path"] = safe_relative_display(project_root, review_corpus_path)
    summary["independent_review_events_path"] = safe_relative_display(project_root, events_path)
    summary["pending_graph_readiness_review_count"] = sum(
        1 for row in diagnostics if row.get("review_status") == "pending_review"
    )
    summary["graph_readiness_review_blocker_count"] = sum(
        1 for row in diagnostics if str(row.get("review_status", "")).startswith("review_blocked")
    )
    summary["independent_review_completed_count"] = 0
    summary["automated_state_is_structural_only"] = True


def run_replay(
    *,
    selection_path: Path,
    conversion_summary_path: Path,
    closeout_summary_path: Path,
    output_dir: Path,
    project_root: Path,
    review_dir: Path | None = None,
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
    output_dir.mkdir(parents=True, exist_ok=True)
    for path, payload in [*structure_packages, *graph_packages]:
        assert_redacted(payload, path=path)
    for path, payload in structure_packages:
        write_json(path, payload)
    for path, payload in graph_packages:
        write_json(path, payload)
    generate_review_handoff(
        summary=summary,
        diagnostics=diagnostics,
        output_dir=output_dir,
        review_dir=review_dir or output_dir.parent / GRAPH_READINESS_REVIEW_DIR_NAME,
        project_root=project_root,
        run_id=run_id,
    )
    graph_payloads = [payload for _path, payload in graph_packages]
    assert_fail_closed(summary, diagnostics, graph_payloads)
    assert_redacted(summary, path=output_dir / SUMMARY_NAME)
    assert_redacted(diagnostics, path=output_dir / DIAGNOSTICS_NAME)
    assert_redacted(
        load_json_object(output_dir / REVIEW_CORPUS_NAME), path=output_dir / REVIEW_CORPUS_NAME
    )
    if (output_dir / INDEPENDENT_REVIEW_EVENTS_NAME).exists():
        assert_redacted(
            (output_dir / INDEPENDENT_REVIEW_EVENTS_NAME).read_text(encoding="utf-8"),
            path=output_dir / INDEPENDENT_REVIEW_EVENTS_NAME,
        )
    report = render_report(summary)
    assert_redacted(report, path=output_dir / REPORT_NAME)
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
    parser.add_argument("--review-dir", default=None, type=Path)
    parser.add_argument("--project-root", default=Path.cwd(), type=Path)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        project_root = args.project_root.resolve()
        cli_paths = [
            args.selection,
            args.conversion_summary,
            args.closeout_summary,
            args.output_dir,
        ]
        if args.review_dir is not None:
            cli_paths.append(args.review_dir)
        for cli_path in cli_paths:
            if (
                not cli_path.is_absolute()
                and ".." in PurePosixPath(str(cli_path).replace("\\", "/")).parts
            ):
                raise ChunkEvidenceReplayError("unsafe_cli_path", f"unsafe CLI path: {cli_path}")
        summary = run_replay(
            selection_path=args.selection,
            conversion_summary_path=args.conversion_summary,
            closeout_summary_path=args.closeout_summary,
            output_dir=args.output_dir.resolve(),
            project_root=project_root,
            review_dir=args.review_dir.resolve() if args.review_dir is not None else None,
        )
        sys.stdout.write(
            json.dumps(
                {
                    "status": summary["status"],
                    "counts": summary["counts"],
                    "summary": (args.output_dir / SUMMARY_NAME).as_posix(),
                    "review_corpus": summary.get("review_corpus_path"),
                    "review_events": summary.get("independent_review_events_path"),
                },
                sort_keys=True,
            )
            + "\n"
        )
        return 0
    except ChunkEvidenceReplayError as exc:
        sys.stderr.write(
            json.dumps(
                {
                    "status": "failed",
                    "code": exc.code,
                    "message": str(exc),
                    "json_path": exc.json_path,
                },
                sort_keys=True,
            )
            + "\n"
        )
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
