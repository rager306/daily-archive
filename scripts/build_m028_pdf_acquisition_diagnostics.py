#!/usr/bin/env python3
"""Build metadata-only PDF acquisition diagnostics for M028 S03.

This builder consumes the accepted source selection plus S01/S02 acquisition and
metadata adapter outputs.  It does not fetch URLs, parse article bodies, emit PDF
bytes, invoke graph/KG/model paths, or write production state.  The output is a
per-URL-ref diagnostic view of whether a PDF is already locally acquired and
checksum/signature-verifiable, not a PDF acquisition engine.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections import Counter, defaultdict
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

EVENT_SCHEMA_VERSION = "m028.pdf-acquisition-event.v1"
SUMMARY_SCHEMA_VERSION = "m028.pdf-acquisition-summary.v1"
EVENTS_FILENAME = "pdf-acquisition-events.jsonl"
SUMMARY_FILENAME = "pdf-acquisition-summary.json"
REPORT_FILENAME = "pdf-acquisition-report.md"

EXPECTED_REF_COUNT = 21
EXPECTED_IDENTITY_COUNT = 20
EXPECTED_SOURCE_KIND_COUNTS = {
    "arxiv_abs_url": 15,
    "arxiv_pdf_url": 4,
    "company_blog_url": 1,
    "nature_article_url": 1,
}

ARXIV_URL_RE = re.compile(r"arxiv\.org/(?:abs|pdf)/(\d{4}\.\d{4,5})(v\d+)?(?:\.pdf)?", re.I)

SAFETY_FLAGS = {
    "raw_article_text_embedded": False,
    "raw_pdf_bytes_embedded": False,
    "html_source_embedded": False,
    "chunk_content_embedded": False,
    "graph_write_attempted": False,
    "production_persistence_attempted": False,
    "parser_readiness_claimed": False,
    "kg_readiness_claimed": False,
    "dspy_attempted": False,
    "rlm_attempted": False,
    "minimax_attempted": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
}

UNSAFE_COUNTER_KEYS = (
    "raw_article_text_embedded",
    "raw_pdf_bytes_embedded",
    "html_source_embedded",
    "chunk_content_embedded",
    "graph_write_attempted",
    "production_persistence_attempted",
    "parser_readiness_claimed",
    "kg_readiness_claimed",
    "dspy_attempted",
    "rlm_attempted",
    "minimax_attempted",
    "production_import_attempted",
    "ladybugdb_written",
)


class PdfDiagnosticInputError(ValueError):
    """Raised when S03 PDF diagnostic inputs are malformed or inconsistent."""


def utc_now_iso() -> str:
    return datetime.now(UTC).isoformat()


def read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise PdfDiagnosticInputError(f"input_missing:{path}") from exc
    except json.JSONDecodeError as exc:
        raise PdfDiagnosticInputError(f"json_malformed:{path}:{exc.lineno}:{exc.colno}") from exc
    if not isinstance(payload, dict):
        raise PdfDiagnosticInputError(f"json_object_required:{path}")
    return payload


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise PdfDiagnosticInputError(f"input_missing:{path}") from exc

    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as exc:
            raise PdfDiagnosticInputError(f"jsonl_malformed:{path}:{line_number}:{exc.colno}") from exc
        if not isinstance(row, dict):
            raise PdfDiagnosticInputError(f"jsonl_object_required:{path}:{line_number}")
        rows.append(row)
    return rows


def validate_selection(selection: dict[str, Any]) -> list[dict[str, Any]]:
    refs = selection.get("refs")
    if not isinstance(refs, list) or not refs:
        raise PdfDiagnosticInputError("selection_refs_required")
    seen: set[str] = set()
    validated: list[dict[str, Any]] = []
    for index, ref in enumerate(refs):
        if not isinstance(ref, dict):
            raise PdfDiagnosticInputError(f"selection_ref_object_required:{index}")
        ref_id = ref.get("ref_id")
        url = ref.get("url")
        canonical_url = ref.get("canonical_url")
        source_kind = ref.get("source_kind")
        normalized_identity = ref.get("normalized_identity")
        if not all(isinstance(value, str) and value for value in (ref_id, url, canonical_url, source_kind, normalized_identity)):
            raise PdfDiagnosticInputError(f"selection_ref_required_fields:{index}")
        if ref_id in seen:
            raise PdfDiagnosticInputError(f"selection_ref_duplicate:{ref_id}")
        seen.add(ref_id)
        validated.append(ref)
    return validated


def validate_event_rows(rows: list[dict[str, Any]], *, label: str) -> dict[str, dict[str, Any]]:
    by_ref: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(rows):
        ref_id = row.get("ref_id")
        if not isinstance(ref_id, str) or not ref_id:
            raise PdfDiagnosticInputError(f"{label}_ref_id_required:{index}")
        if ref_id in by_ref:
            raise PdfDiagnosticInputError(f"{label}_ref_duplicate:{ref_id}")
        by_ref[ref_id] = row
    return by_ref


def source_family(source_kind: str) -> str:
    if source_kind.startswith("arxiv_"):
        return "arxiv"
    if source_kind == "nature_article_url":
        return "nature"
    if source_kind == "company_blog_url":
        return "company_blog"
    return "unknown"


def classify_variant(url: str, source_kind: str) -> str:
    if source_kind == "arxiv_pdf_url" or "/pdf/" in url:
        return "pdf_url"
    if source_kind == "arxiv_abs_url" or "/abs/" in url:
        return "abs_url"
    if source_kind == "nature_article_url":
        return "nature_article_url"
    if source_kind == "company_blog_url":
        return "company_blog_url"
    return "unknown_url"


def arxiv_id_from_url(url: str) -> str | None:
    match = ARXIV_URL_RE.search(url)
    if match is None:
        return None
    return match.group(1) + (match.group(2) or "")


def unversioned_arxiv_id(arxiv_id: str | None) -> str | None:
    if not arxiv_id:
        return None
    return re.sub(r"v\d+$", "", arxiv_id)


def arxiv_id_for_ref(ref: dict[str, Any]) -> str | None:
    return unversioned_arxiv_id(str(ref.get("arxiv_unversioned_id") or ref.get("arxiv_id") or "")) or unversioned_arxiv_id(
        arxiv_id_from_url(str(ref.get("url") or ""))
    )


def arxiv_pdf_url_for_ref(ref: dict[str, Any], metadata_event: dict[str, Any] | None) -> tuple[str | None, str | None, bool]:
    source_kind = str(ref["source_kind"])
    if source_kind == "arxiv_pdf_url":
        return str(ref["url"]), "selection.url", False

    metadata_pdf_url = None
    metadata_source = None
    if metadata_event is not None:
        optional_metadata = metadata_event.get("optional_metadata")
        if isinstance(optional_metadata, dict):
            pdf_url = optional_metadata.get("pdf_url")
            if isinstance(pdf_url, dict) and pdf_url.get("status") == "present" and isinstance(pdf_url.get("value"), str):
                metadata_pdf_url = str(pdf_url["value"])
                metadata_source = str(pdf_url.get("source") or "source_metadata.optional_metadata.pdf_url")
    if metadata_pdf_url:
        return metadata_pdf_url, metadata_source, True

    arxiv_id = arxiv_id_for_ref(ref)
    if arxiv_id:
        return f"https://arxiv.org/pdf/{arxiv_id}", "derived.arxiv_id", False
    return None, None, False


def build_identity_groups(refs: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for ref in refs:
        grouped[str(ref["normalized_identity"])].append(ref)
    groups: dict[str, dict[str, Any]] = {}
    for normalized_identity, group_refs in grouped.items():
        ref_ids = [str(ref["ref_id"]) for ref in group_refs]
        groups[normalized_identity] = {
            "group_id": f"identity:{normalized_identity}",
            "normalized_identity": normalized_identity,
            "ref_ids": ref_ids,
            "url_ref_count": len(ref_ids),
            "has_url_variants": len(ref_ids) > 1,
            "url_variants": [classify_variant(str(ref["url"]), str(ref["source_kind"])) for ref in group_refs],
        }
    return groups


def diagnostic(code: str, ref_id: str, severity: str, json_path: str, details: dict[str, Any] | None = None) -> dict[str, Any]:
    return {
        "code": code,
        "severity": severity,
        "ref_id": ref_id,
        "json_path": json_path,
        "message": "PDF acquisition diagnostic; inspect code, severity, and JSON path.",
        "details": details or {},
    }


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def pdf_signature_verified(path: Path) -> bool:
    with path.open("rb") as handle:
        return handle.read(5) == b"%PDF-"


def safe_repo_path(repo_root: Path, path_value: Any) -> tuple[Path | None, str | None]:
    if not isinstance(path_value, str) or not path_value.strip():
        return None, "artifact_path_missing"
    if "://" in path_value:
        return None, "artifact_path_is_url"
    normalized = PurePosixPath(path_value.replace("\\", "/"))
    if normalized.is_absolute() or ".." in normalized.parts or any(part == "" for part in normalized.parts):
        return None, "artifact_path_unsafe"
    repo_root_resolved = repo_root.resolve()
    resolved = (repo_root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(repo_root_resolved):
        return None, "artifact_path_escapes_repo"
    return resolved, None


def validate_scope(
    refs: list[dict[str, Any]],
    acquisition_by_ref: dict[str, dict[str, Any]],
    metadata_by_ref: dict[str, dict[str, Any]],
    metadata_summary: dict[str, Any],
    *,
    expected_ref_count: int,
    expected_identity_count: int,
    expected_source_kind_counts: dict[str, int],
) -> None:
    ref_ids = [str(ref["ref_id"]) for ref in refs]
    ref_set = set(ref_ids)
    if len(refs) != expected_ref_count:
        raise PdfDiagnosticInputError(f"selection_ref_count_mismatch:expected={expected_ref_count}:actual={len(refs)}")
    identity_count = len({str(ref["normalized_identity"]) for ref in refs})
    if identity_count != expected_identity_count:
        raise PdfDiagnosticInputError(f"selection_identity_count_mismatch:expected={expected_identity_count}:actual={identity_count}")
    source_kind_counts = Counter(str(ref["source_kind"]) for ref in refs)
    if dict(sorted(source_kind_counts.items())) != dict(sorted(expected_source_kind_counts.items())):
        raise PdfDiagnosticInputError("selection_source_kind_counts_mismatch")
    if set(acquisition_by_ref) != ref_set:
        missing = sorted(ref_set - set(acquisition_by_ref))
        extra = sorted(set(acquisition_by_ref) - ref_set)
        raise PdfDiagnosticInputError(f"acquisition_ref_set_mismatch:missing={missing}:extra={extra}")
    if set(metadata_by_ref) != ref_set:
        missing = sorted(ref_set - set(metadata_by_ref))
        extra = sorted(set(metadata_by_ref) - ref_set)
        raise PdfDiagnosticInputError(f"metadata_ref_set_mismatch:missing={missing}:extra={extra}")

    summary_ref_count = metadata_summary.get("url_ref_count", metadata_summary.get("ref_count"))
    if summary_ref_count != expected_ref_count:
        raise PdfDiagnosticInputError(f"metadata_summary_ref_count_mismatch:expected={expected_ref_count}:actual={summary_ref_count}")
    if metadata_summary.get("normalized_identity_count") != expected_identity_count:
        raise PdfDiagnosticInputError("metadata_summary_identity_count_mismatch")
    if dict(sorted((metadata_summary.get("source_kind_counts") or {}).items())) != dict(sorted(expected_source_kind_counts.items())):
        raise PdfDiagnosticInputError("metadata_summary_source_kind_counts_mismatch")

    for ref in refs:
        ref_id = str(ref["ref_id"])
        source_kind = ref["source_kind"]
        normalized_identity = ref["normalized_identity"]
        acquisition = acquisition_by_ref[ref_id]
        metadata = metadata_by_ref[ref_id]
        if acquisition.get("source_kind") != source_kind:
            raise PdfDiagnosticInputError(f"acquisition_source_kind_mismatch:{ref_id}")
        if acquisition.get("normalized_identity") != normalized_identity:
            raise PdfDiagnosticInputError(f"acquisition_identity_mismatch:{ref_id}")
        if acquisition.get("terminal") is not True:
            raise PdfDiagnosticInputError(f"acquisition_not_terminal:{ref_id}")
        if metadata.get("source_kind") != source_kind:
            raise PdfDiagnosticInputError(f"metadata_source_kind_mismatch:{ref_id}")
        if metadata.get("normalized_identity") != normalized_identity:
            raise PdfDiagnosticInputError(f"metadata_identity_mismatch:{ref_id}")


def inspect_existing_pdf_artifact(ref: dict[str, Any], acquisition: dict[str, Any], repo_root: Path) -> tuple[dict[str, Any], list[dict[str, Any]], str | None]:
    ref_id = str(ref["ref_id"])
    diagnostics: list[dict[str, Any]] = []
    artifact_path, path_error = safe_repo_path(repo_root, acquisition.get("artifact_path"))
    path_value = acquisition.get("artifact_path") if isinstance(acquisition.get("artifact_path"), str) else None
    artifact = {
        "path": path_value,
        "exists": False,
        "content_type": acquisition.get("content_type") if isinstance(acquisition.get("content_type"), str) else None,
        "byte_count": acquisition.get("byte_count") if isinstance(acquisition.get("byte_count"), int) else None,
        "sha256": acquisition.get("sha256") if isinstance(acquisition.get("sha256"), str) else None,
        "checksum_verified": False,
        "signature_verified": False,
        "bytes_embedded": False,
    }
    if path_error is not None:
        diagnostics.append(diagnostic(path_error, ref_id, "error", "pdf_artifact.path"))
        return artifact, diagnostics, path_error
    assert artifact_path is not None
    if not artifact_path.exists():
        diagnostics.append(diagnostic("artifact_file_missing", ref_id, "error", "pdf_artifact.path"))
        return artifact, diagnostics, "artifact_file_missing"

    artifact["exists"] = True
    actual_byte_count = artifact_path.stat().st_size
    artifact["byte_count"] = actual_byte_count
    expected_byte_count = acquisition.get("byte_count")
    if isinstance(expected_byte_count, int) and expected_byte_count != actual_byte_count:
        diagnostics.append(
            diagnostic(
                "artifact_byte_count_mismatch",
                ref_id,
                "warning",
                "pdf_artifact.byte_count",
                {"expected": expected_byte_count, "actual": actual_byte_count},
            )
        )

    expected_sha256 = acquisition.get("sha256")
    actual_sha256 = sha256_file(artifact_path)
    artifact["checksum_verified"] = isinstance(expected_sha256, str) and expected_sha256 == actual_sha256
    if not artifact["checksum_verified"]:
        diagnostics.append(diagnostic("artifact_checksum_mismatch", ref_id, "error", "pdf_artifact.sha256"))
        return artifact, diagnostics, "artifact_checksum_mismatch"

    content_type = str(acquisition.get("content_type") or "").lower()
    if "pdf" not in content_type and artifact_path.suffix.lower() != ".pdf":
        diagnostics.append(diagnostic("existing_artifact_not_pdf", ref_id, "error", "pdf_artifact.content_type"))
        return artifact, diagnostics, "existing_artifact_not_pdf"

    artifact["signature_verified"] = pdf_signature_verified(artifact_path)
    if not artifact["signature_verified"]:
        diagnostics.append(diagnostic("malformed_existing_pdf_signature", ref_id, "error", "pdf_artifact.signature_verified"))
        return artifact, diagnostics, "malformed_existing_pdf_signature"
    return artifact, diagnostics, None


def build_event(
    ref: dict[str, Any],
    acquisition: dict[str, Any],
    metadata_event: dict[str, Any],
    identity_groups: dict[str, dict[str, Any]],
    repo_root: Path,
) -> dict[str, Any]:
    ref_id = str(ref["ref_id"])
    source_kind = str(ref["source_kind"])
    family = source_family(source_kind)
    diagnostics: list[dict[str, Any]] = []
    candidate_url: str | None = None
    candidate_url_source: str | None = None
    metadata_pdf_url_present = False
    if family == "arxiv":
        candidate_url, candidate_url_source, metadata_pdf_url_present = arxiv_pdf_url_for_ref(ref, metadata_event)
        candidate_kind = "explicit_arxiv_pdf_url" if source_kind == "arxiv_pdf_url" else "arxiv_abs_pdf_candidate"
        candidate_reason = None
    else:
        optional_metadata = metadata_event.get("optional_metadata") if isinstance(metadata_event, dict) else None
        pdf_url = optional_metadata.get("pdf_url") if isinstance(optional_metadata, dict) else None
        metadata_pdf_url_present = bool(isinstance(pdf_url, dict) and pdf_url.get("status") == "present" and pdf_url.get("value"))
        candidate_kind = "not_applicable_non_arxiv"
        candidate_reason = "not_applicable_non_arxiv_pdf_source"

    candidate_pdf = {
        "is_candidate": family == "arxiv",
        "candidate_kind": candidate_kind,
        "url": candidate_url,
        "url_source": candidate_url_source,
        "metadata_pdf_url_present": metadata_pdf_url_present,
        "not_candidate_reason": candidate_reason,
    }
    empty_artifact = {
        "path": None,
        "exists": False,
        "content_type": None,
        "byte_count": None,
        "sha256": None,
        "checksum_verified": False,
        "signature_verified": False,
        "bytes_embedded": False,
    }

    if family != "arxiv":
        diagnostics.append(diagnostic("not_applicable_non_arxiv_pdf_source", ref_id, "info", "candidate_pdf"))
        pdf_acquisition = {"status": "not_applicable", "terminal": True, "reason": "not_applicable_non_arxiv_pdf_source"}
        pdf_artifact = empty_artifact
    elif source_kind == "arxiv_pdf_url":
        pdf_artifact, artifact_diagnostics, failure_reason = inspect_existing_pdf_artifact(ref, acquisition, repo_root)
        diagnostics.extend(artifact_diagnostics)
        if failure_reason is None:
            pdf_acquisition = {"status": "acquired_existing_pdf", "terminal": True, "reason": "existing_pdf_checksum_signature_verified"}
        else:
            pdf_acquisition = {"status": "not_acquired", "terminal": True, "reason": failure_reason}
    else:
        diagnostics.append(
            diagnostic(
                "arxiv_abs_no_local_pdf_artifact",
                ref_id,
                "info",
                "pdf_acquisition",
                {"candidate_url_source": candidate_url_source},
            )
        )
        pdf_acquisition = {"status": "not_acquired", "terminal": True, "reason": "arxiv_abs_no_local_pdf_artifact"}
        pdf_artifact = empty_artifact

    return {
        "schema_version": EVENT_SCHEMA_VERSION,
        "ref_id": ref_id,
        "url": ref["url"],
        "canonical_url": ref["canonical_url"],
        "url_variant": classify_variant(str(ref["url"]), source_kind),
        "source_kind": source_kind,
        "source_family": family,
        "normalized_identity": ref["normalized_identity"],
        "identity_group": identity_groups[str(ref["normalized_identity"])],
        "candidate_pdf": candidate_pdf,
        "source_acquisition": {
            "status": acquisition.get("status"),
            "terminal": acquisition.get("terminal"),
            "http_status": acquisition.get("http_status"),
            "failure_code": acquisition.get("failure_code"),
            "captured": acquisition.get("status") == "captured",
        },
        "pdf_acquisition": pdf_acquisition,
        "pdf_artifact": pdf_artifact,
        "safety_flags": dict(SAFETY_FLAGS),
        "diagnostics": diagnostics,
    }


def summarize(events: list[dict[str, Any]], identity_groups: dict[str, dict[str, Any]]) -> dict[str, Any]:
    source_kind_counts = Counter(str(event["source_kind"]) for event in events)
    source_family_counts = Counter(str(event["source_family"]) for event in events)
    candidate_kind_counts = Counter(str(event["candidate_pdf"]["candidate_kind"]) for event in events)
    pdf_status_counts = Counter(str(event["pdf_acquisition"]["status"]) for event in events)
    reason_counts = Counter(str(event["pdf_acquisition"]["reason"]) for event in events if event["pdf_acquisition"]["status"] != "acquired_existing_pdf")
    diagnostic_counts: Counter[str] = Counter()
    checksum_counts = Counter("verified" if event["pdf_artifact"]["checksum_verified"] else "not_verified" for event in events)
    signature_counts = Counter("verified" if event["pdf_artifact"]["signature_verified"] else "not_verified" for event in events)
    for event in events:
        for item in event["diagnostics"]:
            diagnostic_counts[str(item["code"])] += 1

    duplicate_groups = [group for group in identity_groups.values() if group["url_ref_count"] > 1]
    return {
        "schema_version": SUMMARY_SCHEMA_VERSION,
        "generated_at": utc_now_iso(),
        "url_ref_count": len(events),
        "ref_count": len(events),
        "normalized_identity_count": len(identity_groups),
        "duplicate_identity_group_count": len(duplicate_groups),
        "duplicate_identity_groups": duplicate_groups,
        "ref_ids": [str(event["ref_id"]) for event in events],
        "identity_groups": list(identity_groups.values()),
        "source_kind_counts": dict(sorted(source_kind_counts.items())),
        "source_family_counts": dict(sorted(source_family_counts.items())),
        "candidate_kind_counts": dict(sorted(candidate_kind_counts.items())),
        "pdf_status_counts": dict(sorted(pdf_status_counts.items())),
        "non_acquired_reason_counts": dict(sorted(reason_counts.items())),
        "diagnostic_counts": dict(sorted(diagnostic_counts.items())),
        "checksum_status_counts": dict(sorted(checksum_counts.items())),
        "signature_status_counts": dict(sorted(signature_counts.items())),
        "existing_pdf_artifact_count": sum(1 for event in events if event["pdf_artifact"]["exists"]),
        "candidate_ref_count": sum(1 for event in events if event["candidate_pdf"]["is_candidate"]),
        "safety_flags": dict(SAFETY_FLAGS),
        "unsafe_claim_counts": {key: 0 for key in UNSAFE_COUNTER_KEYS},
        "load_profile": {
            "expected_url_refs": EXPECTED_REF_COUNT,
            "ten_x_url_refs": EXPECTED_REF_COUNT * 10,
            "first_saturating_resource": "sequential filesystem reads and streaming checksum/signature probes for existing PDF artifacts",
            "protection": "no network calls, one-pass chunked SHA-256 hashing, five-byte signature probe, deterministic per-ref iteration, no parser/graph/KG/model production paths",
        },
        "failure_modes": [
            {
                "dependency": "selection JSON",
                "failure_path": "missing, malformed, stale-count, duplicate, or required-field errors raise PdfDiagnosticInputError before output write",
            },
            {
                "dependency": "source acquisition JSONL",
                "failure_path": "missing, malformed, duplicate, non-terminal, missing-linkage, source-kind drift, or identity drift raise stable input errors",
            },
            {
                "dependency": "source metadata events/summary",
                "failure_path": "missing, malformed, stale count, missing linkage, source-kind drift, or identity drift raise stable input errors",
            },
            {
                "dependency": "captured PDF artifact filesystem",
                "failure_path": "unsafe paths, missing files, checksum mismatches, non-PDF artifacts, and malformed signatures become typed per-ref non-acquired diagnostics",
            },
        ],
        "negative_tests": [
            "tests/test_m028_pdf_acquisition_diagnostics.py::test_malformed_existing_pdf_signature_becomes_typed_diagnostic",
            "tests/test_m028_pdf_acquisition_diagnostics.py::test_missing_acquisition_linkage_is_stable_input_error",
            "tests/test_m028_pdf_acquisition_diagnostics.py::test_checksum_mismatch_records_typed_non_acquired_reason",
            "tests/test_m028_pdf_acquisition_diagnostics.py::test_real_corpus_regeneration_contract",
        ],
    }


def render_report(summary: dict[str, Any]) -> str:
    lines = [
        "# M028 S03 PDF Acquisition Diagnostics",
        "",
        "Metadata-only diagnostics for the accepted URL refs. This report records existing local PDF verification and typed terminal non-acquired/not-applicable reasons; it does not fetch URLs or serialize article/PDF bytes.",
        "",
        "## Scope",
        f"- URL refs: {summary['url_ref_count']}",
        f"- Normalized identities: {summary['normalized_identity_count']}",
        f"- Duplicate identity groups: {summary['duplicate_identity_group_count']}",
        f"- Source kind counts: `{json.dumps(summary['source_kind_counts'], sort_keys=True)}`",
        "",
        "## PDF Acquisition Counts",
        f"- Status counts: `{json.dumps(summary['pdf_status_counts'], sort_keys=True)}`",
        f"- Non-acquired/not-applicable reasons: `{json.dumps(summary['non_acquired_reason_counts'], sort_keys=True)}`",
        f"- Existing PDF artifacts: {summary['existing_pdf_artifact_count']}",
        f"- Candidate refs: {summary['candidate_ref_count']}",
        "",
        "## Safety Flags",
        f"- All fail-closed flags false: `{json.dumps(summary['safety_flags'], sort_keys=True)}`",
        f"- Unsafe claim counts: `{json.dumps(summary['unsafe_claim_counts'], sort_keys=True)}`",
        "",
        "## Failure Modes",
    ]
    for item in summary["failure_modes"]:
        lines.append(f"- {item['dependency']}: {item['failure_path']}")
    lines.extend([
        "",
        "## Load Profile",
        f"- Expected refs: {summary['load_profile']['expected_url_refs']}; 10x refs: {summary['load_profile']['ten_x_url_refs']}",
        f"- First saturating resource: {summary['load_profile']['first_saturating_resource']}",
        f"- Protection: {summary['load_profile']['protection']}",
        "",
        "## Negative Tests",
    ])
    for item in summary["negative_tests"]:
        lines.append(f"- `{item}`")
    lines.extend([
        "",
        "## Observability Impact",
        "- Emits per-ref PDF candidate classification, terminal typed acquisition reason, checksum/signature status, duplicate identity membership, diagnostics, and fail-closed aggregate counters.",
        "",
    ])
    return "\n".join(lines)


def build_pdf_acquisition_outputs(
    selection_path: Path,
    acquisition_events_path: Path,
    metadata_events_path: Path,
    metadata_summary_path: Path,
    out_dir: Path,
    *,
    repo_root: Path | None = None,
    expected_ref_count: int = EXPECTED_REF_COUNT,
    expected_identity_count: int = EXPECTED_IDENTITY_COUNT,
    expected_source_kind_counts: dict[str, int] | None = None,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    repo_root = repo_root or Path.cwd()
    expected_source_kind_counts = expected_source_kind_counts or EXPECTED_SOURCE_KIND_COUNTS
    selection = read_json(selection_path)
    refs = validate_selection(selection)
    acquisition_by_ref = validate_event_rows(read_jsonl(acquisition_events_path), label="acquisition")
    metadata_by_ref = validate_event_rows(read_jsonl(metadata_events_path), label="metadata")
    metadata_summary = read_json(metadata_summary_path)
    validate_scope(
        refs,
        acquisition_by_ref,
        metadata_by_ref,
        metadata_summary,
        expected_ref_count=expected_ref_count,
        expected_identity_count=expected_identity_count,
        expected_source_kind_counts=expected_source_kind_counts,
    )
    identity_groups = build_identity_groups(refs)
    events = [
        build_event(ref, acquisition_by_ref[str(ref["ref_id"])], metadata_by_ref[str(ref["ref_id"])], identity_groups, repo_root)
        for ref in refs
    ]
    summary = summarize(events, identity_groups)

    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / EVENTS_FILENAME).write_text("\n".join(json.dumps(event, sort_keys=True) for event in events) + "\n", encoding="utf-8")
    (out_dir / SUMMARY_FILENAME).write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    (out_dir / REPORT_FILENAME).write_text(render_report(summary), encoding="utf-8")
    return events, summary


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--acquisition-events", required=True, type=Path)
    parser.add_argument("--metadata-events", required=True, type=Path)
    parser.add_argument("--metadata-summary", required=True, type=Path)
    parser.add_argument("--out-dir", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        _events, summary = build_pdf_acquisition_outputs(
            args.selection,
            args.acquisition_events,
            args.metadata_events,
            args.metadata_summary,
            args.out_dir,
        )
    except PdfDiagnosticInputError as exc:
        raise SystemExit(str(exc)) from exc
    sys.stdout.write(
        "wrote PDF acquisition diagnostics: "
        f"refs={summary['url_ref_count']} identities={summary['normalized_identity_count']} "
        f"events={args.out_dir / EVENTS_FILENAME} summary={args.out_dir / SUMMARY_FILENAME} report={args.out_dir / REPORT_FILENAME}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
