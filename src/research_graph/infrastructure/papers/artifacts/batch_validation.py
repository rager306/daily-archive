"""Deterministic metadata-only 10-document article batch validation contract.

S07 composes the S01-S06 article evidence surfaces into one local validation
report for ten real article inputs.  The report is deliberately review-only: it
summarizes identifiers, source provenance, subtree coverage, freshness, stable
path-addressed diagnostics, and fixed-zero graph/import/write counters.  It
never reads article payload files and never serializes article prose, chunks,
table/caption text, binary bytes/base64, embeddings, vectors, tokens, secrets,
model output, or production-write authorization.

Formerly: src/arxiv_archive/article_batch_validation.py
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from collections import Counter
from copy import deepcopy
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Literal

from research_graph.application.validation.batch_provenance import (
    append_validation_cli_provenance,
    build_artifact_freshness_report,
    build_validation_cli_provenance_entry,
    write_artifact_freshness_report,
)
from research_graph.application.validation.batch_state import ValidationBatchState, read_batch_state

ARTICLE_BATCH_VALIDATION_SCHEMA_VERSION = "article-batch-validation.v1"
ARTICLE_BATCH_VALIDATION_BUILDER = "metadata_only_article_batch_validation_v1"
EXPECTED_DOCUMENT_COUNT = 10

DocumentStatus = Literal["ready_review_only", "blocked_review_only"]
DiagnosticSeverity = Literal["info", "warning", "repair_required", "error"]
Recommendation = Literal[
    "proceed_to_20_document_scale_review_only",
    "repeat_10_document_batch_after_repairs",
    "collect_missing_local_sources",
    "stop_graph_import_unsafe_evidence",
]

SUBTREE_NAMES = (
    "loader",
    "bridge",
    "page_index",
    "assets",
    "links_dedup",
    "retrieval_tables",
)
ALLOWED_SUBTREE_STATUSES = frozenset(
    {
        "complete_review_only",
        "metadata_only",
        "review_only_not_import_eligible",
        "blocked",
        "repair_required",
        "not_attempted",
        "absent",
    }
)
BLOCKING_SUBTREE_STATUSES = frozenset({"blocked", "repair_required", "not_attempted", "absent"})

DIAGNOSTIC_COUNTER_KEYS = (
    "empty_batch_count",
    "batch_size_mismatch_count",
    "duplicate_document_id_count",
    "duplicate_source_id_count",
    "missing_source_path_count",
    "missing_source_hash_count",
    "malformed_document_count",
    "malformed_subtree_count",
    "blocked_subtree_count",
    "stale_artifact_count",
    "forbidden_payload_detection_count",
    "unsafe_authorization_count",
    "unsafe_readiness_count",
)

FORBIDDEN_PAYLOAD_KEYS = frozenset(
    {
        "text",
        "raw_text",
        "chunk",
        "chunks",
        "chunk_text",
        "paper_text",
        "claim_text",
        "section_text",
        "caption_text",
        "table_text",
        "equation_text",
        "model_output",
        "raw_model_output",
        "raw_minimax_response",
        "base64",
        "binary",
        "bytes",
        "image_bytes",
        "payload",
        "embedding",
        "embeddings",
        "vector",
        "vectors",
        "secret",
        "secrets",
        "token",
        "tokens",
        "api_key",
        "credentials",
        "optimizer_trace",
        "optimizer_traces",
        "source_of_truth",
    }
)
UNSAFE_AUTHORIZATION_FLAGS = frozenset(
    {
        "trusted_kg_import_allowed",
        "ladybugdb_written",
        "production_import_attempted",
        "graph_import_claim",
        "import_eligible",
        "promoted_to_fact",
        "trusted_kg_imported",
        "production_write_attempted",
    }
)
UNSAFE_PAYLOAD_FLAGS = frozenset(
    {
        "raw_payloads_included",
        "raw_text_embedded",
        "raw_binary_embedded",
        "raw_table_embedded",
        "caption_embedded",
        "table_text_included",
        "caption_text_included",
        "embeddings_included",
        "embedding_included",
        "vectors_included",
        "vector_included",
        "optimizer_traces_included",
        "dspy_used",
        "rlm_used",
        "optimizer_used",
    }
)
UNSAFE_FALSE_FLAGS = UNSAFE_AUTHORIZATION_FLAGS | UNSAFE_PAYLOAD_FLAGS
UNSAFE_READINESS_STATUSES = frozenset(
    {"ready_for_import", "import_ready", "trusted", "promoted", "fact", "accepted_for_import"}
)
_SAFE_ID_RE = re.compile(r"^[A-Za-z0-9_.:/-]+$")
_SHA256_RE = re.compile(r"^[a-fA-F0-9]{64}$")
_SECRET_LIKE_VALUE_RE = re.compile(
    r"(?i)(?:api[_-]?key|token|secret|password|credential)s?\s*=|[?&](?:api[_-]?key|token|secret|password|credential)s?="
)


@dataclass(frozen=True)
class ArticleBatchValidationDiagnostic:
    """One stable, redacted diagnostic for batch validation."""

    code: str
    json_path: str
    severity: DiagnosticSeverity = "repair_required"
    document_id: str | None = None
    message: str = (
        "Article batch validation diagnostic; inspect code and JSON path, not source content."
    )
    blocks_import: bool = True

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "json_path": self.json_path,
            "severity": self.severity,
            "document_id": self.document_id,
            "message": self.message,
            "blocks_import": self.blocks_import,
        }


def default_safety_counters() -> dict[str, int]:
    """Return fixed-zero graph/import/write counters for the review-only contract."""

    return {
        "graph_import_attempted_count": 0,
        "trusted_kg_import_allowed_count": 0,
        "ladybugdb_written_count": 0,
        "production_write_attempted_count": 0,
        "embedding_generation_attempted_count": 0,
        "vector_indexing_attempted_count": 0,
        "raw_payload_serialized_count": 0,
    }


def default_safety_flags() -> dict[str, bool]:
    """Return required false safety flags for S07 reports."""

    return {
        "metadata_only": True,
        "local_files_only": True,
        "review_only": True,
        "network_used": False,
        "graph_import_attempted": False,
        "trusted_kg_import_allowed": False,
        "ladybugdb_written": False,
        "production_write_attempted": False,
        "raw_payloads_included": False,
        "embeddings_included": False,
        "vectors_included": False,
    }


def build_article_batch_validation_report(manifest: dict[str, Any] | Any) -> dict[str, Any]:
    """Build a deterministic redacted batch report from metadata-only inputs.

    Malformed manifests and unsafe S01-S06 summaries fail closed with stable,
    path-addressed diagnostics.  The function never reads paths referenced by
    documents; path/hash checks are shape/provenance checks only.
    """

    root = manifest if isinstance(manifest, dict) else {}
    batch_id = _safe_string(root.get("batch_id")) or "unknown-batch"
    run_id = _safe_string(root.get("run_id")) or "unknown-run"
    selected = root.get("documents")
    documents = selected if isinstance(selected, list) else []

    diagnostics: list[ArticleBatchValidationDiagnostic] = []
    if not isinstance(manifest, dict):
        diagnostics.append(_diagnostic("malformed_manifest", "$", severity="error"))
    if selected is None:
        diagnostics.append(_diagnostic("missing_documents", "$.documents", severity="error"))
    elif not isinstance(selected, list):
        diagnostics.append(_diagnostic("malformed_documents", "$.documents", severity="error"))
    elif len(selected) == 0:
        diagnostics.append(_diagnostic("empty_batch", "$.documents", severity="error"))
    if isinstance(selected, list) and len(selected) != EXPECTED_DOCUMENT_COUNT:
        diagnostics.append(
            _diagnostic(
                "batch_size_mismatch",
                "$.documents",
                severity="error" if len(selected) == 0 else "repair_required",
                blocks_import=True,
            )
        )

    diagnostics.extend(_validate_forbidden_and_unsafe(root))

    document_rows: list[dict[str, Any]] = []
    seen_document_ids: dict[str, int] = {}
    seen_source_ids: dict[str, int] = {}
    for index, item in enumerate(documents):
        path = f"$.documents[{index}]"
        if not isinstance(item, dict):
            diagnostics.append(_diagnostic("malformed_document", path, severity="error"))
            document_rows.append(_blocked_placeholder(index, ["malformed_document"]))
            continue

        document_id = (
            _safe_string(item.get("document_id"))
            or _safe_string(item.get("paper_id"))
            or f"unknown-document-{index}"
        )
        source_id = _safe_string(item.get("source_id")) or f"{document_id}:source:unknown"
        document_codes: list[str] = []

        if not _safe_id(document_id):
            diagnostics.append(
                _diagnostic("malformed_document_id", f"{path}.document_id", document_id=document_id)
            )
            document_codes.append("malformed_document_id")
        if document_id in seen_document_ids:
            diagnostics.append(
                _diagnostic("duplicate_document_id", f"{path}.document_id", document_id=document_id)
            )
            document_codes.append("duplicate_document_id")
        seen_document_ids[document_id] = seen_document_ids.get(document_id, 0) + 1

        if not _safe_id(source_id):
            diagnostics.append(
                _diagnostic("malformed_source_id", f"{path}.source_id", document_id=document_id)
            )
            document_codes.append("malformed_source_id")
        if source_id in seen_source_ids:
            diagnostics.append(
                _diagnostic("duplicate_source_id", f"{path}.source_id", document_id=document_id)
            )
            document_codes.append("duplicate_source_id")
        seen_source_ids[source_id] = seen_source_ids.get(source_id, 0) + 1

        source_path = _safe_string(item.get("source_path"))
        source_sha256 = _safe_string(item.get("source_sha256") or item.get("sha256"))
        if not source_path:
            diagnostics.append(
                _diagnostic("missing_source_path", f"{path}.source_path", document_id=document_id)
            )
            document_codes.append("missing_source_path")
        if not source_sha256:
            diagnostics.append(
                _diagnostic("missing_source_hash", f"{path}.source_sha256", document_id=document_id)
            )
            document_codes.append("missing_source_hash")
        elif not _SHA256_RE.match(source_sha256):
            diagnostics.append(
                _diagnostic(
                    "malformed_source_hash", f"{path}.source_sha256", document_id=document_id
                )
            )
            document_codes.append("malformed_source_hash")

        subtree_result = _summarize_subtrees(
            item.get("subtrees"), path=f"{path}.subtrees", document_id=document_id
        )
        diagnostics.extend(subtree_result["diagnostics"])
        document_codes.extend(subtree_result["blocking_codes"])

        freshness = _summarize_freshness(
            item.get("freshness"), path=f"{path}.freshness", document_id=document_id
        )
        diagnostics.extend(freshness["diagnostics"])
        document_codes.extend(freshness["blocking_codes"])

        status: DocumentStatus = "blocked_review_only" if document_codes else "ready_review_only"
        document_rows.append(
            {
                "document_id": document_id,
                "paper_id": _safe_string(item.get("paper_id")) or document_id,
                "source_id": source_id,
                "source_path": source_path or None,
                "source_sha256": source_sha256
                if source_sha256 and _SHA256_RE.match(source_sha256)
                else None,
                "status": status,
                "diagnostic_codes": sorted(set(document_codes)),
                "subtree_statuses": subtree_result["statuses"],
                "coverage": subtree_result["coverage"],
                "freshness": freshness["summary"],
                "graph_import_attempted": False,
                "ladybugdb_written": False,
                "production_write_attempted": False,
                "import_eligible": False,
                "promoted_to_fact": False,
            }
        )

    diagnostic_dicts = _dedupe_diagnostics(diagnostics)
    summary = _summarize_batch(document_rows, diagnostic_dicts)
    recommendation = derive_recommendation(summary)
    report = {
        "schema_version": ARTICLE_BATCH_VALIDATION_SCHEMA_VERSION,
        "builder": ARTICLE_BATCH_VALIDATION_BUILDER,
        "batch_id": batch_id,
        "run_id": run_id,
        "expected_document_count": EXPECTED_DOCUMENT_COUNT,
        "document_status_rows": sorted(document_rows, key=lambda row: str(row["document_id"])),
        "aggregate_diagnostics": summary,
        "coverage_distributions": _coverage_distributions(document_rows),
        "provenance_freshness_summary": _freshness_distribution(document_rows),
        "safety_counters": default_safety_counters(),
        "safety_flags": default_safety_flags(),
        "recommendation": recommendation,
        "recommendation_rationale": _recommendation_rationale(recommendation, summary),
        "diagnostics": diagnostic_dicts,
    }
    return _redact(report)


def run_article_batch_validation_report(
    *,
    output_dir: str | Path,
    manifest_path: str | Path | None = None,
    state_path: str | Path | None = None,
    limit: int = EXPECTED_DOCUMENT_COUNT,
    provenance_log_path: str | Path | None = None,
    argv: list[str] | None = None,
) -> dict[str, Any]:
    """Write S07 metadata-only batch report, diagnostics, provenance, and freshness artifacts.

    Exactly one of ``manifest_path`` or ``state_path`` must be provided.  Manifest
    input is expected to already use the S07 ``documents`` contract.  State input
    is adapted from validation-batch selected-paper metadata only; article files
    referenced by source paths are not opened or hashed.
    """

    if (manifest_path is None) == (state_path is None):
        raise ValueError("provide exactly one of manifest_path or state_path")
    if limit <= 0:
        raise ValueError("limit must be positive")

    output = Path(output_dir)
    output.mkdir(parents=True, exist_ok=True)
    input_path = Path(manifest_path or state_path or "")
    report_path = output / "article-batch-validation-report.json"
    diagnostics_path = output / "article-batch-validation-diagnostics.jsonl"
    freshness_path = output / "article-batch-validation-freshness.json"
    provenance_path = (
        Path(provenance_log_path)
        if provenance_log_path is not None
        else output / "validation-cli-provenance.jsonl"
    )
    started_at = datetime.now(UTC)
    status = "article_report_written"
    exit_code = 0

    try:
        manifest = _load_runner_manifest(
            manifest_path=manifest_path, state_path=state_path, limit=limit
        )
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        manifest = _blocked_runner_manifest(input_path, reason=type(exc).__name__)
        status = "blocked_report_written"
        exit_code = 1

    report = build_article_batch_validation_report(manifest)
    report["runner"] = {
        "command": "validation-batch article-report",
        "source_kind": "manifest" if manifest_path is not None else "state",
        "input_path": str(input_path),
        "output_dir": str(output),
        "report_path": str(report_path),
        "diagnostics_path": str(diagnostics_path),
        "provenance_log_path": str(provenance_path),
        "freshness_report_path": str(freshness_path),
        "selected_document_limit": min(limit, EXPECTED_DOCUMENT_COUNT),
        "metadata_only": True,
        "real_source_acquisition_performed": False,
        "real_scan_performed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }
    _write_report_and_diagnostics(
        report, report_path=report_path, diagnostics_path=diagnostics_path
    )

    completed_at = datetime.now(UTC)
    provenance_entry = build_validation_cli_provenance_entry(
        command="validation-batch article-report",
        argv=argv or sys.argv[1:],
        batch_id=str(report.get("batch_id") or "unknown-batch"),
        input_paths=[input_path] if input_path.exists() else [],
        output_paths=[report_path, diagnostics_path],
        status=status,
        started_at=started_at,
        completed_at=completed_at,
        exit_code=0 if exit_code == 0 else 1,
        cwd=Path.cwd(),
        run_id=str(report.get("run_id") or "unknown-run"),
        real_source_acquisition_performed=False,
        real_scan_performed=False,
        expected_artifact_metadata={"schema_version": ARTICLE_BATCH_VALIDATION_SCHEMA_VERSION},
    )
    append_validation_cli_provenance(provenance_path, provenance_entry)
    freshness = build_artifact_freshness_report(provenance_entry)
    write_artifact_freshness_report(freshness, freshness_path)

    return {
        "status": status,
        "exit_code": exit_code,
        "batch_id": report.get("batch_id"),
        "run_id": report.get("run_id"),
        "recommendation": report.get("recommendation"),
        "ready_document_count": report.get("aggregate_diagnostics", {}).get(
            "ready_document_count", 0
        ),
        "blocked_document_count": report.get("aggregate_diagnostics", {}).get(
            "blocked_document_count", 0
        ),
        "diagnostic_count": report.get("aggregate_diagnostics", {}).get("diagnostic_count", 0),
        "freshness_verdict": freshness.get("verdict"),
        "report_path": str(report_path),
        "diagnostics_path": str(diagnostics_path),
        "freshness_report_path": str(freshness_path),
        "provenance_log_path": str(provenance_path),
        "real_source_acquisition_performed": False,
        "real_scan_performed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
    }


def _load_runner_manifest(
    *,
    manifest_path: str | Path | None,
    state_path: str | Path | None,
    limit: int,
) -> dict[str, Any]:
    if manifest_path is not None:
        payload = json.loads(Path(manifest_path).read_text(encoding="utf-8"))
        if not isinstance(payload, dict):
            raise ValueError("article batch manifest must be a JSON object")
        documents = payload.get("documents")
        if isinstance(documents, list):
            payload = dict(payload)
            payload["documents"] = documents[: min(limit, EXPECTED_DOCUMENT_COUNT)]
        return payload
    state = read_batch_state(Path(state_path or ""))
    return _manifest_from_batch_state(state, state_path=Path(state_path or ""), limit=limit)


def _manifest_from_batch_state(
    state: ValidationBatchState, *, state_path: Path, limit: int
) -> dict[str, Any]:
    documents: list[dict[str, Any]] = []
    selected = list(state.selected_papers)[: min(limit, EXPECTED_DOCUMENT_COUNT)]
    for paper in selected:
        source_path = _preferred_source_path(paper.source_paths)
        source_sha256 = _source_sha256_from_metadata(paper.source_paths)
        readiness = state.source_readiness_by_paper.get(paper.paper_id)
        freshness_status = (
            "fresh"
            if readiness is not None and readiness.ready_for_markdown_scan
            else "not_provided"
        )
        stale_count = 0 if freshness_status == "fresh" else 1 if readiness is not None else 0
        documents.append(
            {
                "document_id": paper.paper_id,
                "paper_id": paper.paper_id,
                "source_id": f"{paper.paper_id}:validation-batch-state",
                "source_path": source_path,
                "source_sha256": source_sha256,
                "subtrees": {
                    name: {"status": "metadata_only", "record_count": 0} for name in SUBTREE_NAMES
                },
                "freshness": {"status": freshness_status, "stale_artifact_count": stale_count},
                "selection_role": paper.selection_role,
                "rank": paper.rank,
                "risk_tag_count": len(paper.risk_tags),
            }
        )
    return {
        "batch_id": state.batch_id,
        "run_id": f"{state.batch_id}-article-report",
        "source_state_path": str(state_path),
        "documents": documents,
    }


def _preferred_source_path(source_paths: dict[str, str]) -> str | None:
    for key in (
        "research_full_text_md",
        "cache_markdown",
        "research_pdf",
        "cache_pdf",
        "source_path",
    ):
        value = source_paths.get(key)
        if value:
            return str(value)
    for _key, value in sorted(source_paths.items()):
        if value:
            return str(value)
    return None


def _source_sha256_from_metadata(source_paths: dict[str, str]) -> str | None:
    for key in ("source_sha256", "sha256", "research_full_text_sha256", "cache_markdown_sha256"):
        value = source_paths.get(key)
        if value:
            return str(value).removeprefix("sha256:")
    return None


def _blocked_runner_manifest(input_path: Path, *, reason: str) -> dict[str, Any]:
    return {
        "batch_id": input_path.stem or "blocked-input",
        "run_id": f"{input_path.stem or 'blocked-input'}-article-report-blocked",
        "documents": [],
        "runner_input_error": reason,
    }


def _write_report_and_diagnostics(
    report: dict[str, Any], *, report_path: Path, diagnostics_path: Path
) -> None:
    report_path.write_text(to_json(report), encoding="utf-8")
    with diagnostics_path.open("w", encoding="utf-8") as handle:
        for diagnostic in report.get("diagnostics", []):
            handle.write(json.dumps(diagnostic, sort_keys=True, separators=(",", ":")) + "\n")


def validate_article_batch_validation_report(report: dict[str, Any] | Any) -> list[dict[str, Any]]:
    """Validate an already-built S07 report and return redacted diagnostics."""

    root = report if isinstance(report, dict) else {}
    diagnostics: list[ArticleBatchValidationDiagnostic] = []
    if not isinstance(report, dict):
        diagnostics.append(_diagnostic("malformed_report", "$", severity="error"))
    if root.get("schema_version") != ARTICLE_BATCH_VALIDATION_SCHEMA_VERSION:
        diagnostics.append(_diagnostic("bad_schema_version", "$.schema_version", severity="error"))
    diagnostics.extend(_validate_forbidden_and_unsafe(root))
    if root.get("safety_counters") != default_safety_counters():
        diagnostics.append(
            _diagnostic("nonzero_safety_counters", "$.safety_counters", severity="error")
        )
    if root.get("safety_flags") != default_safety_flags():
        diagnostics.append(_diagnostic("unsafe_safety_flags", "$.safety_flags", severity="error"))
    rows = root.get("document_status_rows")
    if not isinstance(rows, list):
        diagnostics.append(
            _diagnostic("missing_document_status_rows", "$.document_status_rows", severity="error")
        )
    return _dedupe_diagnostics(diagnostics)


def derive_recommendation(summary: dict[str, Any]) -> Recommendation:
    """Return the stable S07 next-step recommendation vocabulary."""

    diagnostic_counts = (
        summary.get("diagnostic_counts")
        if isinstance(summary.get("diagnostic_counts"), dict)
        else {}
    )
    unsafe_count = int(diagnostic_counts.get("unsafe_authorization_count", 0) or 0) + int(  # ty:ignore[unresolved-attribute]
        diagnostic_counts.get("unsafe_readiness_count", 0) or 0  # ty:ignore[unresolved-attribute]
    )
    if unsafe_count:
        return "stop_graph_import_unsafe_evidence"
    if int(diagnostic_counts.get("missing_source_path_count", 0) or 0) or int(  # ty:ignore[unresolved-attribute]
        diagnostic_counts.get("missing_source_hash_count", 0) or 0  # ty:ignore[unresolved-attribute]
    ):
        return "collect_missing_local_sources"
    if int(summary.get("document_count", 0) or 0) != EXPECTED_DOCUMENT_COUNT or int(
        summary.get("blocked_document_count", 0) or 0
    ):
        return "repeat_10_document_batch_after_repairs"
    if int(diagnostic_counts.get("stale_artifact_count", 0) or 0) or int(  # ty:ignore[unresolved-attribute]
        diagnostic_counts.get("forbidden_payload_detection_count", 0) or 0  # ty:ignore[unresolved-attribute]
    ):
        return "repeat_10_document_batch_after_repairs"
    return "proceed_to_20_document_scale_review_only"


def to_json(report: dict[str, Any]) -> str:
    """Serialize a report deterministically for comparable artifact diffs."""

    return json.dumps(_redact(report), ensure_ascii=False, sort_keys=True, indent=2) + "\n"


def report_fingerprint(report: dict[str, Any]) -> str:
    """Return a deterministic SHA-256 fingerprint of the redacted report."""

    return hashlib.sha256(to_json(report).encode("utf-8")).hexdigest()


def _diagnostic(
    code: str,
    json_path: str,
    *,
    severity: DiagnosticSeverity = "repair_required",
    document_id: str | None = None,
    blocks_import: bool = True,
) -> ArticleBatchValidationDiagnostic:
    return ArticleBatchValidationDiagnostic(
        code=code,
        json_path=json_path,
        severity=severity,
        document_id=document_id,
        blocks_import=blocks_import,
    )


def _safe_string(value: Any) -> str:
    return value.strip() if isinstance(value, str) else ""


def _safe_id(value: str) -> bool:
    return bool(value and _SAFE_ID_RE.match(value))


def _json_child_path(parent: str, child: str | int) -> str:
    if isinstance(child, int):
        return f"{parent}[{child}]"
    return f"{parent}.{child}" if parent != "$" else f"$.{child}"


def _iter_payload_paths(value: Any, path: str = "$") -> list[tuple[str, str]]:
    findings: list[tuple[str, str]] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = _json_child_path(path, str(key))
            if str(key) in FORBIDDEN_PAYLOAD_KEYS:
                findings.append((str(key), child_path))
            findings.extend(_iter_payload_paths(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            findings.extend(_iter_payload_paths(child, _json_child_path(path, index)))
    return findings


def _iter_sensitive_value_paths(value: Any, path: str = "$") -> list[str]:
    findings: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            findings.extend(_iter_sensitive_value_paths(child, _json_child_path(path, str(key))))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            findings.extend(_iter_sensitive_value_paths(child, _json_child_path(path, index)))
    elif isinstance(value, str) and _SECRET_LIKE_VALUE_RE.search(value):
        findings.append(path)
    return findings


def _iter_unsafe_true_paths(value: Any, path: str = "$") -> list[tuple[str, str]]:
    findings: list[tuple[str, str]] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = _json_child_path(path, str(key))
            if str(key) in UNSAFE_FALSE_FLAGS and child is True:
                findings.append((str(key), child_path))
            findings.extend(_iter_unsafe_true_paths(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            findings.extend(_iter_unsafe_true_paths(child, _json_child_path(path, index)))
    return findings


def _iter_readiness_paths(value: Any, path: str = "$") -> list[str]:
    findings: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = _json_child_path(path, str(key))
            if str(key) in {
                "status",
                "benchmark_status",
                "review_state",
                "recommendation",
            } and isinstance(child, str):
                if child in UNSAFE_READINESS_STATUSES:
                    findings.append(child_path)
            findings.extend(_iter_readiness_paths(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            findings.extend(_iter_readiness_paths(child, _json_child_path(path, index)))
    return findings


def _validate_forbidden_and_unsafe(value: Any) -> list[ArticleBatchValidationDiagnostic]:
    diagnostics: list[ArticleBatchValidationDiagnostic] = []
    for key, path in _iter_payload_paths(value):
        diagnostics.append(_diagnostic(f"forbidden_payload_key:{key}", path, severity="error"))
    for path in _iter_sensitive_value_paths(value):
        diagnostics.append(
            _diagnostic("forbidden_payload_value:sensitive_token", path, severity="error")
        )
    for key, path in _iter_unsafe_true_paths(value):
        if key in UNSAFE_AUTHORIZATION_FLAGS:
            diagnostics.append(
                _diagnostic(f"unsafe_authorization_flag:{key}", path, severity="error")
            )
        else:
            diagnostics.append(_diagnostic(f"unsafe_payload_flag:{key}", path, severity="error"))
    for path in _iter_readiness_paths(value):
        diagnostics.append(_diagnostic("unsafe_readiness_status", path, severity="error"))
    return diagnostics


def _summarize_subtrees(value: Any, *, path: str, document_id: str) -> dict[str, Any]:
    diagnostics: list[ArticleBatchValidationDiagnostic] = []
    statuses: dict[str, str] = {}
    coverage: dict[str, int] = {}
    blocking_codes: list[str] = []
    if not isinstance(value, dict):
        diagnostics.append(
            _diagnostic("malformed_subtrees", path, severity="error", document_id=document_id)
        )
        blocking_codes.append("malformed_subtrees")
        value = {}

    for name in SUBTREE_NAMES:
        subtree = value.get(name) if isinstance(value, dict) else None
        subtree_path = f"{path}.{name}"
        if not isinstance(subtree, dict):
            diagnostics.append(
                _diagnostic("missing_subtree_summary", subtree_path, document_id=document_id)
            )
            statuses[name] = "absent"
            coverage[name] = 0
            blocking_codes.append(f"{name}:absent")
            continue
        status = _safe_string(subtree.get("status")) or "absent"
        if status not in ALLOWED_SUBTREE_STATUSES:
            diagnostics.append(
                _diagnostic(
                    "malformed_subtree_status", f"{subtree_path}.status", document_id=document_id
                )
            )
            blocking_codes.append(f"{name}:malformed_status")
            status = "blocked"
        if status in BLOCKING_SUBTREE_STATUSES:
            diagnostics.append(
                _diagnostic("blocked_subtree", f"{subtree_path}.status", document_id=document_id)
            )
            blocking_codes.append(f"{name}:{status}")
        statuses[name] = status
        coverage[name] = _non_negative_int(subtree.get("record_count"))
    return {
        "diagnostics": diagnostics,
        "statuses": statuses,
        "coverage": coverage,
        "blocking_codes": blocking_codes,
    }


def _summarize_freshness(value: Any, *, path: str, document_id: str) -> dict[str, Any]:
    diagnostics: list[ArticleBatchValidationDiagnostic] = []
    blocking_codes: list[str] = []
    if value is None:
        return {
            "diagnostics": diagnostics,
            "blocking_codes": blocking_codes,
            "summary": {"status": "not_provided", "stale_artifact_count": 0},
        }
    if not isinstance(value, dict):
        diagnostics.append(
            _diagnostic("malformed_freshness_summary", path, document_id=document_id)
        )
        blocking_codes.append("malformed_freshness_summary")
        return {
            "diagnostics": diagnostics,
            "blocking_codes": blocking_codes,
            "summary": {"status": "malformed", "stale_artifact_count": 1},
        }
    status = _safe_string(value.get("status")) or "not_provided"
    stale_count = _non_negative_int(value.get("stale_artifact_count"))
    if status in {"stale", "blocked"} or stale_count:
        diagnostics.append(_diagnostic("stale_artifact", path, document_id=document_id))
        blocking_codes.append("stale_artifact")
    return {
        "diagnostics": diagnostics,
        "blocking_codes": blocking_codes,
        "summary": {"status": status, "stale_artifact_count": stale_count},
    }


def _non_negative_int(value: Any) -> int:
    if isinstance(value, bool):
        return 0
    if isinstance(value, int) and value >= 0:
        return value
    return 0


def _blocked_placeholder(index: int, codes: list[str]) -> dict[str, Any]:
    document_id = f"unknown-document-{index}"
    return {
        "document_id": document_id,
        "paper_id": document_id,
        "source_id": f"{document_id}:source:unknown",
        "source_path": None,
        "source_sha256": None,
        "status": "blocked_review_only",
        "diagnostic_codes": sorted(set(codes)),
        "subtree_statuses": dict.fromkeys(SUBTREE_NAMES, "absent"),
        "coverage": dict.fromkeys(SUBTREE_NAMES, 0),
        "freshness": {"status": "not_provided", "stale_artifact_count": 0},
        "graph_import_attempted": False,
        "ladybugdb_written": False,
        "production_write_attempted": False,
        "import_eligible": False,
        "promoted_to_fact": False,
    }


def _summarize_batch(
    rows: list[dict[str, Any]], diagnostics: list[dict[str, Any]]
) -> dict[str, Any]:
    counts = dict.fromkeys(DIAGNOSTIC_COUNTER_KEYS, 0)
    for diagnostic in diagnostics:
        code = str(diagnostic.get("code", ""))
        if code == "empty_batch":
            counts["empty_batch_count"] += 1
        elif code == "batch_size_mismatch":
            counts["batch_size_mismatch_count"] += 1
        elif code == "duplicate_document_id":
            counts["duplicate_document_id_count"] += 1
        elif code in {"duplicate_source_id", "malformed_source_id"}:
            counts["duplicate_source_id_count"] += int(code == "duplicate_source_id")
        elif code == "missing_source_path":
            counts["missing_source_path_count"] += 1
        elif code in {"missing_source_hash", "malformed_source_hash"}:
            counts["missing_source_hash_count"] += 1
        elif code in {
            "malformed_manifest",
            "malformed_documents",
            "malformed_document",
            "malformed_document_id",
        }:
            counts["malformed_document_count"] += 1
        elif code in {"malformed_subtrees", "missing_subtree_summary", "malformed_subtree_status"}:
            counts["malformed_subtree_count"] += 1
        elif code == "blocked_subtree":
            counts["blocked_subtree_count"] += 1
        elif code in {"stale_artifact", "malformed_freshness_summary"}:
            counts["stale_artifact_count"] += 1
        elif code.startswith("forbidden_payload_key:") or code.startswith(
            "forbidden_payload_value:"
        ):
            counts["forbidden_payload_detection_count"] += 1
        elif code.startswith("unsafe_authorization_flag:"):
            counts["unsafe_authorization_count"] += 1
        elif code == "unsafe_readiness_status":
            counts["unsafe_readiness_count"] += 1
    ready = sum(1 for row in rows if row.get("status") == "ready_review_only")
    blocked = sum(1 for row in rows if row.get("status") == "blocked_review_only")
    return {
        "document_count": len(rows),
        "ready_document_count": ready,
        "blocked_document_count": blocked,
        "diagnostic_count": len(diagnostics),
        "diagnostic_counts": counts,
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
        "ladybugdb_written_count": 0,
        "production_write_attempted_count": 0,
        "graph_import_attempted_count": 0,
    }


def _coverage_distributions(rows: list[dict[str, Any]]) -> dict[str, dict[str, int]]:
    distributions: dict[str, dict[str, int]] = {}
    for name in SUBTREE_NAMES:
        status_counts = Counter(
            str(row.get("subtree_statuses", {}).get(name, "absent")) for row in rows
        )
        present_count = sum(1 for row in rows if int(row.get("coverage", {}).get(name, 0) or 0) > 0)
        distributions[name] = {
            "documents_with_records": present_count,
            **dict(sorted(status_counts.items())),
        }
    return distributions


def _freshness_distribution(rows: list[dict[str, Any]]) -> dict[str, Any]:
    statuses = Counter(str(row.get("freshness", {}).get("status", "not_provided")) for row in rows)
    stale = sum(int(row.get("freshness", {}).get("stale_artifact_count", 0) or 0) for row in rows)
    return {"status_counts": dict(sorted(statuses.items())), "stale_artifact_count": stale}


def _recommendation_rationale(recommendation: str, summary: dict[str, Any]) -> str:
    if recommendation == "proceed_to_20_document_scale_review_only":
        return "All ten metadata-only documents are review-ready with fixed-zero graph/import/write counters."
    if recommendation == "stop_graph_import_unsafe_evidence":
        return "Unsafe authorization or import-readiness claims were detected; do not scale or import until repaired."
    if recommendation == "collect_missing_local_sources":
        return "One or more documents lack local source path or checksum provenance; repair only those documents."
    return f"Batch has {summary.get('blocked_document_count', 0)} blocked document(s) or stale/incomplete coverage; rerun after repairs."


def _dedupe_diagnostics(
    diagnostics: list[ArticleBatchValidationDiagnostic],
) -> list[dict[str, Any]]:
    seen: set[tuple[str, str, str | None]] = set()
    output: list[dict[str, Any]] = []
    for diagnostic in diagnostics:
        item = diagnostic.to_redacted_dict()
        key = (str(item["code"]), str(item["json_path"]), item.get("document_id"))
        if key in seen:
            continue
        seen.add(key)
        output.append(item)
    return sorted(
        output,
        key=lambda item: (
            str(item["json_path"]),
            str(item["code"]),
            str(item.get("document_id") or ""),
        ),
    )


def _redact(value: Any) -> Any:
    if isinstance(value, dict):
        redacted: dict[str, Any] = {}
        for key in sorted(value):
            if key in FORBIDDEN_PAYLOAD_KEYS:
                continue
            child = value[key]
            if key in UNSAFE_FALSE_FLAGS:
                redacted[key] = (
                    False if isinstance(child, bool) else 0 if isinstance(child, int) else child
                )
            else:
                redacted[key] = _redact(child)
        return redacted
    if isinstance(value, list):
        return [_redact(child) for child in value]
    if isinstance(value, str):
        return (
            "<redacted-sensitive-value>" if _SECRET_LIKE_VALUE_RE.search(value) else deepcopy(value)
        )
    return deepcopy(value)
