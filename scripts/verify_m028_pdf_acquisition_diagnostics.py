#!/usr/bin/env python3
"""Verify M028 S03 PDF acquisition diagnostic artifacts.

This verifier is metadata-only. It validates the accepted 21 URL refs / 20
normalized identities contract, PDF candidate classification, checksum/signature
claims for existing local PDFs, typed terminal non-acquired/not-applicable
reasons, duplicate identity preservation, and fail-closed safety flags. It does
not fetch URLs, parse source bodies, invoke graph/KG/model paths, or write
production state.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter, defaultdict
from collections.abc import Sequence
from pathlib import Path, PurePosixPath
from typing import Any

EXPECTED_EVENT_SCHEMA_VERSION = "m028.pdf-acquisition-event.v1"
EXPECTED_SUMMARY_SCHEMA_VERSION = "m028.pdf-acquisition-summary.v1"
EXPECTED_URL_REF_COUNT = 21
EXPECTED_NORMALIZED_IDENTITY_COUNT = 20
EXPECTED_NEW_REF_IDS = frozenset({"R15", "R16", "R17", "R18", "R19", "R20", "R21"})
EXPECTED_SOURCE_KIND_COUNTS = {
    "arxiv_abs_url": 15,
    "arxiv_pdf_url": 4,
    "company_blog_url": 1,
    "nature_article_url": 1,
}
EXPECTED_SOURCE_FAMILY_COUNTS = {"arxiv": 19, "company_blog": 1, "nature": 1}
EXPECTED_CANDIDATE_KIND_COUNTS = {
    "arxiv_abs_pdf_candidate": 15,
    "explicit_arxiv_pdf_url": 4,
    "not_applicable_non_arxiv": 2,
}
EXPECTED_PDF_STATUS_COUNTS = {"acquired_existing_pdf": 4, "not_acquired": 15, "not_applicable": 2}
EXPECTED_NON_ACQUIRED_REASON_COUNTS = {
    "arxiv_abs_no_local_pdf_artifact": 15,
    "not_applicable_non_arxiv_pdf_source": 2,
}
EXPECTED_CHECKSUM_STATUS_COUNTS = {"not_verified": 17, "verified": 4}
EXPECTED_SIGNATURE_STATUS_COUNTS = {"not_verified": 17, "verified": 4}
EXPECTED_EXISTING_PDF_REFS = frozenset({"R01", "R08", "R12", "R13"})
EXPECTED_DUPLICATE_IDENTITY = "arxiv:2605.20897"
EXPECTED_DUPLICATE_REF_IDS = ["R01", "R10"]

REQUIRED_EVENT_TOP_LEVEL_FIELDS = (
    "schema_version",
    "ref_id",
    "url",
    "canonical_url",
    "url_variant",
    "source_kind",
    "source_family",
    "normalized_identity",
    "identity_group",
    "candidate_pdf",
    "source_acquisition",
    "pdf_acquisition",
    "pdf_artifact",
    "safety_flags",
    "diagnostics",
)
REQUIRED_CANDIDATE_FIELDS = (
    "is_candidate",
    "candidate_kind",
    "url",
    "url_source",
    "metadata_pdf_url_present",
    "not_candidate_reason",
)
REQUIRED_PDF_ACQUISITION_FIELDS = ("status", "terminal", "reason")
REQUIRED_PDF_ARTIFACT_FIELDS = (
    "path",
    "exists",
    "content_type",
    "byte_count",
    "sha256",
    "checksum_verified",
    "signature_verified",
    "bytes_embedded",
)

UNSAFE_FLAG_KEYS = (
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

FORBIDDEN_PAYLOAD_MARKERS = (
    "<html",
    "</html>",
    "<!doctype html",
    "%pdf-",
    "raw_text",
    "raw_payload",
    "source_payload",
    "source_body",
    "body_text",
    "html_document",
    "pdf_bytes",
    "binary_payload",
    "chunk_text",
    "chunk_payload",
    "model_output",
    "trusted_fact",
    "graph_ready",
    "kg_ready",
    "parser_ready",
    "promoted_to_fact",
    "ladybugdb_written=true",
)

SAFE_DIAGNOSTIC_CODES = {
    "schema_mismatch",
    "corpus_scope_stale",
    "missing_new_refs",
    "identity_count_mismatch",
    "source_kind_drift",
    "source_family_drift",
    "selection_event_mismatch",
    "missing_pdf_event",
    "unexpected_pdf_event",
    "summary_event_mismatch",
    "candidate_classification_mismatch",
    "non_arxiv_pdf_promotion",
    "pdf_status_mismatch",
    "pdf_reason_mismatch",
    "existing_pdf_artifact_mismatch",
    "artifact_reference_broken",
    "artifact_checksum_mismatch",
    "malformed_existing_pdf_signature",
    "required_nullable_field_missing",
    "duplicate_identity_mismatch",
    "unsafe_claim_detected",
    "raw_payload_leakage",
}


class VerificationError(RuntimeError):
    """Raised when input files cannot be parsed enough to validate."""


def diagnostic(
    code: str, json_path: str, message: str, *, details: dict[str, Any] | None = None
) -> dict[str, Any]:
    if code not in SAFE_DIAGNOSTIC_CODES:
        raise AssertionError(f"unregistered diagnostic code: {code}")
    return {"code": code, "json_path": json_path, "message": message, "details": details or {}}


def read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise VerificationError(f"input_missing:{path}") from exc
    except json.JSONDecodeError as exc:
        raise VerificationError(f"json_malformed:{path}:{exc.lineno}:{exc.colno}") from exc
    if not isinstance(payload, dict):
        raise VerificationError(f"json_object_required:{path}")
    return payload


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise VerificationError(f"input_missing:{path}") from exc

    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as exc:
            raise VerificationError(f"jsonl_malformed:{path}:{line_number}:{exc.colno}") from exc
        if not isinstance(row, dict):
            raise VerificationError(f"jsonl_object_required:{path}:{line_number}")
        rows.append(row)
    return rows


def refs_from_selection(selection: dict[str, Any]) -> list[dict[str, Any]]:
    refs = selection.get("refs")
    if not isinstance(refs, list):
        raise VerificationError("selection_refs_required")
    if not all(isinstance(ref, dict) for ref in refs):
        raise VerificationError("selection_ref_object_required")
    return refs


def by_ref_id(rows: list[dict[str, Any]], source: str) -> dict[str, dict[str, Any]]:
    grouped: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(rows):
        ref_id = row.get("ref_id")
        if not isinstance(ref_id, str) or not ref_id:
            raise VerificationError(f"{source}_ref_id_required:{index}")
        if ref_id in grouped:
            raise VerificationError(f"{source}_ref_id_duplicate:{ref_id}")
        grouped[ref_id] = row
    return grouped


def counter_from(rows: list[dict[str, Any]], key: str) -> dict[str, int]:
    return dict(sorted(Counter(str(row.get(key)) for row in rows).items()))


def nested_get(payload: dict[str, Any], *keys: str) -> Any:
    value: Any = payload
    for key in keys:
        if not isinstance(value, dict):
            return None
        value = value.get(key)
    return value


def sorted_counter(counter: Counter[str]) -> dict[str, int]:
    return dict(sorted(counter.items()))


def safe_repo_path(repo_root: Path, path_value: Any) -> tuple[Path | None, str | None]:
    if not isinstance(path_value, str) or not path_value.strip():
        return None, "artifact_reference_broken"
    if "://" in path_value:
        return None, "artifact_reference_broken"
    normalized = PurePosixPath(path_value.replace("\\", "/"))
    if (
        normalized.is_absolute()
        or ".." in normalized.parts
        or any(part == "" for part in normalized.parts)
    ):
        return None, "artifact_reference_broken"
    repo_root_resolved = repo_root.resolve()
    resolved = (repo_root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(repo_root_resolved):
        return None, "artifact_reference_broken"
    return resolved, None


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def pdf_signature_verified(path: Path) -> bool:
    with path.open("rb") as handle:
        return handle.read(5) == b"%PDF-"


def iter_nested_string_values(payload: Any) -> Any:
    if isinstance(payload, dict):
        for value in payload.values():
            yield from iter_nested_string_values(value)
    elif isinstance(payload, list):
        for value in payload:
            yield from iter_nested_string_values(value)
    elif isinstance(payload, str):
        yield payload


def validate_schema(events: list[dict[str, Any]], summary: dict[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    if summary.get("schema_version") != EXPECTED_SUMMARY_SCHEMA_VERSION:
        diagnostics.append(
            diagnostic(
                "schema_mismatch",
                "$.schema_version",
                "PDF acquisition summary schema version does not match the S03 verifier contract.",
                details={
                    "actual": summary.get("schema_version"),
                    "expected": EXPECTED_SUMMARY_SCHEMA_VERSION,
                },
            )
        )
    bad_event_refs = [
        event.get("ref_id")
        for event in events
        if event.get("schema_version") != EXPECTED_EVENT_SCHEMA_VERSION
    ]
    if bad_event_refs:
        diagnostics.append(
            diagnostic(
                "schema_mismatch",
                "$.events[*].schema_version",
                "One or more PDF acquisition events have an unexpected schema version.",
                details={"ref_ids": bad_event_refs, "expected": EXPECTED_EVENT_SCHEMA_VERSION},
            )
        )
    return diagnostics


def validate_expanded_counts(
    refs: list[dict[str, Any]], events: list[dict[str, Any]], summary: dict[str, Any]
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    if (
        len(refs) != EXPECTED_URL_REF_COUNT
        or len(events) != EXPECTED_URL_REF_COUNT
        or summary.get("url_ref_count") != EXPECTED_URL_REF_COUNT
    ):
        diagnostics.append(
            diagnostic(
                "corpus_scope_stale",
                "$.url_ref_count",
                "M028 S03 must validate the expanded 21-URL-ref corpus, not the stale 14-ref scope.",
                details={
                    "selection": len(refs),
                    "events": len(events),
                    "summary": summary.get("url_ref_count"),
                },
            )
        )

    selection_ref_ids = {str(ref.get("ref_id")) for ref in refs}
    event_ref_ids = {str(event.get("ref_id")) for event in events}
    missing_new_refs = sorted(
        (EXPECTED_NEW_REF_IDS - selection_ref_ids) | (EXPECTED_NEW_REF_IDS - event_ref_ids)
    )
    if missing_new_refs:
        diagnostics.append(
            diagnostic(
                "missing_new_refs",
                "$.refs",
                "Expanded arXiv refs R15-R21 must be present in both selection and PDF acquisition events.",
                details={"missing": missing_new_refs},
            )
        )

    identity_count = len({str(ref.get("normalized_identity")) for ref in refs})
    if (
        identity_count != EXPECTED_NORMALIZED_IDENTITY_COUNT
        or summary.get("normalized_identity_count") != EXPECTED_NORMALIZED_IDENTITY_COUNT
    ):
        diagnostics.append(
            diagnostic(
                "identity_count_mismatch",
                "$.normalized_identity_count",
                "M028 S03 must preserve 20 normalized identities across 21 URL refs.",
                details={
                    "selection": identity_count,
                    "summary": summary.get("normalized_identity_count"),
                },
            )
        )
    return diagnostics


def validate_selection_event_alignment(
    refs: list[dict[str, Any]], events: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    refs_by_id = by_ref_id(refs, "selection")
    events_by_id = by_ref_id(events, "pdf")
    missing = sorted(set(refs_by_id) - set(events_by_id))
    unexpected = sorted(set(events_by_id) - set(refs_by_id))
    if missing:
        diagnostics.append(
            diagnostic(
                "missing_pdf_event",
                "$.events",
                "Selection refs are missing PDF acquisition events.",
                details={"ref_ids": missing},
            )
        )
    if unexpected:
        diagnostics.append(
            diagnostic(
                "unexpected_pdf_event",
                "$.events",
                "PDF acquisition events include refs absent from selection.",
                details={"ref_ids": unexpected},
            )
        )

    for ref_id in sorted(set(refs_by_id) & set(events_by_id)):
        ref = refs_by_id[ref_id]
        event = events_by_id[ref_id]
        missing_fields = [field for field in REQUIRED_EVENT_TOP_LEVEL_FIELDS if field not in event]
        if missing_fields:
            diagnostics.append(
                diagnostic(
                    "required_nullable_field_missing",
                    f"$.events[ref_id={ref_id}]",
                    "PDF acquisition events must preserve required typed fields, including nullable reason/path fields.",
                    details={"ref_id": ref_id, "fields": missing_fields},
                )
            )
        mismatches = {
            field: {"selection": ref.get(field), "event": event.get(field)}
            for field in ("url", "canonical_url", "source_kind", "normalized_identity")
            if ref.get(field) != event.get(field)
        }
        if mismatches:
            diagnostics.append(
                diagnostic(
                    "selection_event_mismatch",
                    f"$.events[ref_id={ref_id}]",
                    "PDF event must preserve selection URL, canonical URL, kind, and identity.",
                    details={"ref_id": ref_id, "mismatches": mismatches},
                )
            )
    return diagnostics


def validate_required_nullable_fields(events: list[dict[str, Any]]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    for event in events:
        ref_id = str(event.get("ref_id"))
        required: list[tuple[str, tuple[str, ...]]] = []
        required.extend(
            (f"candidate_pdf.{field}", ("candidate_pdf", field))
            for field in REQUIRED_CANDIDATE_FIELDS
        )
        required.extend(
            (f"pdf_acquisition.{field}", ("pdf_acquisition", field))
            for field in REQUIRED_PDF_ACQUISITION_FIELDS
        )
        required.extend(
            (f"pdf_artifact.{field}", ("pdf_artifact", field))
            for field in REQUIRED_PDF_ARTIFACT_FIELDS
        )
        missing = [
            path
            for path, keys in required
            if nested_get(event, *keys) is None
            and keys[-1]
            not in {
                "url",
                "url_source",
                "not_candidate_reason",
                "path",
                "content_type",
                "byte_count",
                "sha256",
            }
        ]
        # Nullable fields must be present even when their value is None.
        for path, keys in required:
            container = nested_get(event, *keys[:-1]) if len(keys) > 1 else event
            if not isinstance(container, dict) or keys[-1] not in container:
                missing.append(path)
        if missing:
            diagnostics.append(
                diagnostic(
                    "required_nullable_field_missing",
                    f"$.events[ref_id={ref_id}]",
                    "PDF acquisition events must include typed nullable candidate, acquisition reason, and artifact fields.",
                    details={"ref_id": ref_id, "fields": sorted(set(missing))},
                )
            )
    return diagnostics


def validate_source_counts(
    events: list[dict[str, Any]], summary: dict[str, Any]
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    event_kind_counts = counter_from(events, "source_kind")
    if (
        event_kind_counts != EXPECTED_SOURCE_KIND_COUNTS
        or summary.get("source_kind_counts") != EXPECTED_SOURCE_KIND_COUNTS
    ):
        diagnostics.append(
            diagnostic(
                "source_kind_drift",
                "$.source_kind_counts",
                "Source kind counts drifted from the accepted expanded corpus contract.",
                details={"events": event_kind_counts, "summary": summary.get("source_kind_counts")},
            )
        )

    event_family_counts = counter_from(events, "source_family")
    if (
        event_family_counts != EXPECTED_SOURCE_FAMILY_COUNTS
        or summary.get("source_family_counts") != EXPECTED_SOURCE_FAMILY_COUNTS
    ):
        diagnostics.append(
            diagnostic(
                "source_family_drift",
                "$.source_family_counts",
                "Source family counts drifted from arXiv/company-blog/Nature expectations.",
                details={
                    "events": event_family_counts,
                    "summary": summary.get("source_family_counts"),
                },
            )
        )
    return diagnostics


def validate_candidate_classification(
    events: list[dict[str, Any]], summary: dict[str, Any]
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    candidate_kind_counts = Counter(
        str(nested_get(event, "candidate_pdf", "candidate_kind")) for event in events
    )
    if (
        sorted_counter(candidate_kind_counts) != EXPECTED_CANDIDATE_KIND_COUNTS
        or summary.get("candidate_kind_counts") != EXPECTED_CANDIDATE_KIND_COUNTS
    ):
        diagnostics.append(
            diagnostic(
                "candidate_classification_mismatch",
                "$.candidate_kind_counts",
                "PDF candidate classification counts must match arXiv candidate and non-ArXiv not-applicable expectations.",
                details={
                    "events": sorted_counter(candidate_kind_counts),
                    "summary": summary.get("candidate_kind_counts"),
                },
            )
        )

    for event in events:
        ref_id = str(event.get("ref_id"))
        source_family = event.get("source_family")
        source_kind = event.get("source_kind")
        candidate = (
            event.get("candidate_pdf") if isinstance(event.get("candidate_pdf"), dict) else {}
        )
        if source_family == "arxiv":
            expected_kind = (
                "explicit_arxiv_pdf_url"
                if source_kind == "arxiv_pdf_url"
                else "arxiv_abs_pdf_candidate"
            )
            if (
                candidate.get("is_candidate") is not True
                or candidate.get("candidate_kind") != expected_kind
                or not isinstance(candidate.get("url"), str)
            ):
                diagnostics.append(
                    diagnostic(
                        "candidate_classification_mismatch",
                        f"$.events[ref_id={ref_id}].candidate_pdf",
                        "ArXiv refs must remain explicit PDF candidates with a candidate URL.",
                        details={"ref_id": ref_id, "candidate_pdf": candidate},
                    )
                )
        else:
            if (
                candidate.get("is_candidate") is not False
                or candidate.get("candidate_kind") != "not_applicable_non_arxiv"
                or candidate.get("not_candidate_reason") != "not_applicable_non_arxiv_pdf_source"
                or candidate.get("url") is not None
            ):
                diagnostics.append(
                    diagnostic(
                        "non_arxiv_pdf_promotion",
                        f"$.events[ref_id={ref_id}].candidate_pdf",
                        "Company-blog and Nature refs must remain explicitly not applicable, even if metadata exposes a PDF URL.",
                        details={"ref_id": ref_id, "candidate_pdf": candidate},
                    )
                )
    return diagnostics


def validate_pdf_statuses(
    events: list[dict[str, Any]], summary: dict[str, Any]
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    status_counts = Counter(str(nested_get(event, "pdf_acquisition", "status")) for event in events)
    reason_counts = Counter(
        str(nested_get(event, "pdf_acquisition", "reason"))
        for event in events
        if nested_get(event, "pdf_acquisition", "status") != "acquired_existing_pdf"
    )
    event_status_counts = sorted_counter(status_counts)
    event_reason_counts = sorted_counter(reason_counts)
    if event_status_counts != EXPECTED_PDF_STATUS_COUNTS:
        diagnostics.append(
            diagnostic(
                "pdf_status_mismatch",
                "$.events[*].pdf_acquisition.status",
                "PDF acquisition status counts must match acquired/not-acquired/not-applicable expectations.",
                details={"events": event_status_counts, "expected": EXPECTED_PDF_STATUS_COUNTS},
            )
        )
    if summary.get("pdf_status_counts") != event_status_counts:
        diagnostics.append(
            diagnostic(
                "summary_event_mismatch",
                "$.pdf_status_counts",
                "Summary pdf_status_counts must match per-event acquisition statuses.",
                details={
                    "events": event_status_counts,
                    "summary": summary.get("pdf_status_counts"),
                },
            )
        )
    if event_reason_counts != EXPECTED_NON_ACQUIRED_REASON_COUNTS:
        diagnostics.append(
            diagnostic(
                "pdf_reason_mismatch",
                "$.events[*].pdf_acquisition.reason",
                "Terminal non-acquired/not-applicable reasons must remain typed and counted.",
                details={
                    "events": event_reason_counts,
                    "expected": EXPECTED_NON_ACQUIRED_REASON_COUNTS,
                },
            )
        )
    if summary.get("non_acquired_reason_counts") != event_reason_counts:
        diagnostics.append(
            diagnostic(
                "summary_event_mismatch",
                "$.non_acquired_reason_counts",
                "Summary non_acquired_reason_counts must match per-event typed reasons.",
                details={
                    "events": event_reason_counts,
                    "summary": summary.get("non_acquired_reason_counts"),
                },
            )
        )

    for event in events:
        ref_id = str(event.get("ref_id"))
        source_kind = event.get("source_kind")
        family = event.get("source_family")
        acquisition = (
            event.get("pdf_acquisition") if isinstance(event.get("pdf_acquisition"), dict) else {}
        )
        if acquisition.get("terminal") is not True:
            diagnostics.append(
                diagnostic(
                    "pdf_status_mismatch",
                    f"$.events[ref_id={ref_id}].pdf_acquisition.terminal",
                    "PDF acquisition diagnostics must be terminal per URL ref.",
                    details={"ref_id": ref_id, "pdf_acquisition": acquisition},
                )
            )
        if source_kind == "arxiv_pdf_url":
            expected_status = "acquired_existing_pdf"
            expected_reason = "existing_pdf_checksum_signature_verified"
        elif family == "arxiv":
            expected_status = "not_acquired"
            expected_reason = "arxiv_abs_no_local_pdf_artifact"
        else:
            expected_status = "not_applicable"
            expected_reason = "not_applicable_non_arxiv_pdf_source"
        if (
            acquisition.get("status") != expected_status
            or acquisition.get("reason") != expected_reason
        ):
            diagnostics.append(
                diagnostic(
                    "pdf_reason_mismatch",
                    f"$.events[ref_id={ref_id}].pdf_acquisition",
                    "Each URL ref must retain its expected terminal status and typed reason.",
                    details={
                        "ref_id": ref_id,
                        "expected_status": expected_status,
                        "expected_reason": expected_reason,
                        "actual": acquisition,
                    },
                )
            )
    return diagnostics


def validate_artifacts(
    events: list[dict[str, Any]], summary: dict[str, Any], *, repo_root: Path
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    existing_refs = {
        str(event.get("ref_id"))
        for event in events
        if nested_get(event, "pdf_artifact", "exists") is True
    }
    if existing_refs != EXPECTED_EXISTING_PDF_REFS or summary.get(
        "existing_pdf_artifact_count"
    ) != len(EXPECTED_EXISTING_PDF_REFS):
        diagnostics.append(
            diagnostic(
                "existing_pdf_artifact_mismatch",
                "$.existing_pdf_artifact_count",
                "Exactly the four accepted arXiv PDF URL refs must have existing PDF artifacts.",
                details={
                    "events": sorted(existing_refs),
                    "summary": summary.get("existing_pdf_artifact_count"),
                },
            )
        )

    checksum_counts = Counter(
        "verified"
        if nested_get(event, "pdf_artifact", "checksum_verified") is True
        else "not_verified"
        for event in events
    )
    signature_counts = Counter(
        "verified"
        if nested_get(event, "pdf_artifact", "signature_verified") is True
        else "not_verified"
        for event in events
    )
    if (
        sorted_counter(checksum_counts) != EXPECTED_CHECKSUM_STATUS_COUNTS
        or summary.get("checksum_status_counts") != EXPECTED_CHECKSUM_STATUS_COUNTS
    ):
        diagnostics.append(
            diagnostic(
                "summary_event_mismatch",
                "$.checksum_status_counts",
                "Checksum status summary must match per-event verification flags.",
                details={
                    "events": sorted_counter(checksum_counts),
                    "summary": summary.get("checksum_status_counts"),
                },
            )
        )
    if (
        sorted_counter(signature_counts) != EXPECTED_SIGNATURE_STATUS_COUNTS
        or summary.get("signature_status_counts") != EXPECTED_SIGNATURE_STATUS_COUNTS
    ):
        diagnostics.append(
            diagnostic(
                "summary_event_mismatch",
                "$.signature_status_counts",
                "Signature status summary must match per-event verification flags.",
                details={
                    "events": sorted_counter(signature_counts),
                    "summary": summary.get("signature_status_counts"),
                },
            )
        )

    for event in events:
        ref_id = str(event.get("ref_id"))
        artifact = event.get("pdf_artifact") if isinstance(event.get("pdf_artifact"), dict) else {}
        if ref_id not in EXPECTED_EXISTING_PDF_REFS:
            if (
                artifact.get("exists") is not False
                or artifact.get("checksum_verified") is not False
                or artifact.get("signature_verified") is not False
            ):
                diagnostics.append(
                    diagnostic(
                        "existing_pdf_artifact_mismatch",
                        f"$.events[ref_id={ref_id}].pdf_artifact",
                        "Only explicit arXiv PDF URL refs may claim existing PDF artifacts.",
                        details={"ref_id": ref_id, "artifact": artifact},
                    )
                )
            continue

        resolved, path_error = safe_repo_path(repo_root, artifact.get("path"))
        if path_error is not None or resolved is None or not resolved.exists():
            diagnostics.append(
                diagnostic(
                    "artifact_reference_broken",
                    f"$.events[ref_id={ref_id}].pdf_artifact.path",
                    "Existing PDF artifact path must resolve to a file under the repository root.",
                    details={"ref_id": ref_id, "path": artifact.get("path")},
                )
            )
            continue
        actual_sha256 = sha256_file(resolved)
        if artifact.get("sha256") != actual_sha256 or artifact.get("checksum_verified") is not True:
            diagnostics.append(
                diagnostic(
                    "artifact_checksum_mismatch",
                    f"$.events[ref_id={ref_id}].pdf_artifact.sha256",
                    "Existing PDF artifact checksum claim drifted from the referenced file.",
                    details={
                        "ref_id": ref_id,
                        "expected": artifact.get("sha256"),
                        "actual": actual_sha256,
                    },
                )
            )
        if artifact.get("byte_count") != resolved.stat().st_size:
            diagnostics.append(
                diagnostic(
                    "artifact_reference_broken",
                    f"$.events[ref_id={ref_id}].pdf_artifact.byte_count",
                    "Existing PDF artifact byte_count claim drifted from the referenced file.",
                    details={
                        "ref_id": ref_id,
                        "expected": artifact.get("byte_count"),
                        "actual": resolved.stat().st_size,
                    },
                )
            )
        if artifact.get("signature_verified") is not True or not pdf_signature_verified(resolved):
            diagnostics.append(
                diagnostic(
                    "malformed_existing_pdf_signature",
                    f"$.events[ref_id={ref_id}].pdf_artifact.signature_verified",
                    "Existing PDF artifact must have a verified %PDF- signature.",
                    details={"ref_id": ref_id, "path": artifact.get("path")},
                )
            )
    return diagnostics


def validate_duplicate_identity(
    refs: list[dict[str, Any]], events: list[dict[str, Any]], summary: dict[str, Any]
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    refs_by_identity: dict[str, list[str]] = defaultdict(list)
    for ref in refs:
        refs_by_identity[str(ref.get("normalized_identity"))].append(str(ref.get("ref_id")))
    duplicate_ref_ids = refs_by_identity.get(EXPECTED_DUPLICATE_IDENTITY, [])

    summary_groups = (
        summary.get("duplicate_identity_groups")
        if isinstance(summary.get("duplicate_identity_groups"), list)
        else []
    )
    summary_group = next(
        (
            group
            for group in summary_groups
            if isinstance(group, dict)
            and group.get("normalized_identity") == EXPECTED_DUPLICATE_IDENTITY
        ),
        None,
    )
    event_groups = [
        event.get("identity_group")
        for event in events
        if event.get("normalized_identity") == EXPECTED_DUPLICATE_IDENTITY
    ]
    event_group_ref_ids = [
        group.get("ref_ids") for group in event_groups if isinstance(group, dict)
    ]
    event_group_counts = [
        group.get("url_ref_count") for group in event_groups if isinstance(group, dict)
    ]
    if (
        duplicate_ref_ids != EXPECTED_DUPLICATE_REF_IDS
        or not isinstance(summary_group, dict)
        or summary_group.get("ref_ids") != EXPECTED_DUPLICATE_REF_IDS
        or summary.get("duplicate_identity_group_count") != 1
        or any(ref_ids != EXPECTED_DUPLICATE_REF_IDS for ref_ids in event_group_ref_ids)
        or any(count != 2 for count in event_group_counts)
        or len(event_groups) != 2
    ):
        diagnostics.append(
            diagnostic(
                "duplicate_identity_mismatch",
                "$.duplicate_identity_groups",
                "The arxiv:2605.20897 duplicate URL identity must remain exactly R01/R10 without collapsing URL refs.",
                details={
                    "selection": duplicate_ref_ids,
                    "summary_group": summary_group,
                    "event_groups": event_group_ref_ids,
                },
            )
        )
    return diagnostics


def validate_safety(
    events: list[dict[str, Any]], summary: dict[str, Any], *, reject_unsafe_claims: bool
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    payload_blob = "\n".join(
        str(item).lower()
        for item in iter_nested_string_values({"events": events, "summary": summary})
    )
    leaked_markers = sorted(
        marker for marker in FORBIDDEN_PAYLOAD_MARKERS if marker in payload_blob
    )
    if leaked_markers:
        diagnostics.append(
            diagnostic(
                "raw_payload_leakage",
                "$",
                "PDF acquisition diagnostics must not embed raw HTML/PDF/base64/chunk/model payloads or readiness claims.",
                details={"markers": leaked_markers},
            )
        )

    unsafe_claims: list[dict[str, Any]] = []
    summary_flags = (
        summary.get("safety_flags") if isinstance(summary.get("safety_flags"), dict) else {}
    )
    unsafe_counts = (
        summary.get("unsafe_claim_counts")
        if isinstance(summary.get("unsafe_claim_counts"), dict)
        else {}
    )
    for flag in UNSAFE_FLAG_KEYS:
        if summary_flags.get(flag) is not False:
            unsafe_claims.append(
                {"path": f"$.summary.safety_flags.{flag}", "value": summary_flags.get(flag)}
            )
        if unsafe_counts.get(flag) not in (0, None):
            unsafe_claims.append(
                {"path": f"$.summary.unsafe_claim_counts.{flag}", "value": unsafe_counts.get(flag)}
            )
    for event in events:
        event_flags = (
            event.get("safety_flags") if isinstance(event.get("safety_flags"), dict) else {}
        )
        for flag in UNSAFE_FLAG_KEYS:
            if event_flags.get(flag) is not False:
                unsafe_claims.append(
                    {
                        "path": f"$.events[ref_id={event.get('ref_id')}].safety_flags.{flag}",
                        "value": event_flags.get(flag),
                    }
                )
        artifact = event.get("pdf_artifact") if isinstance(event.get("pdf_artifact"), dict) else {}
        if artifact.get("bytes_embedded") is not False:
            unsafe_claims.append(
                {
                    "path": f"$.events[ref_id={event.get('ref_id')}].pdf_artifact.bytes_embedded",
                    "value": artifact.get("bytes_embedded"),
                }
            )

    if unsafe_claims and reject_unsafe_claims:
        diagnostics.append(
            diagnostic(
                "unsafe_claim_detected",
                "$",
                "Parser, graph, KG, model, production-write, and payload flags must remain fail-closed.",
                details={"claims": unsafe_claims},
            )
        )
    return diagnostics


def validate_summary_consistency(
    events: list[dict[str, Any]], summary: dict[str, Any]
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    if summary.get("ref_ids") != [event.get("ref_id") for event in events]:
        diagnostics.append(
            diagnostic(
                "summary_event_mismatch",
                "$.ref_ids",
                "Summary ref_ids must exactly match PDF event order.",
                details={
                    "summary": summary.get("ref_ids"),
                    "events": [event.get("ref_id") for event in events],
                },
            )
        )
    if summary.get("candidate_ref_count") != sum(
        1 for event in events if nested_get(event, "candidate_pdf", "is_candidate") is True
    ):
        diagnostics.append(
            diagnostic(
                "summary_event_mismatch",
                "$.candidate_ref_count",
                "Summary candidate_ref_count must match per-event candidate flags.",
                details={"summary": summary.get("candidate_ref_count")},
            )
        )
    summary_diagnostics = (
        summary.get("diagnostic_counts")
        if isinstance(summary.get("diagnostic_counts"), dict)
        else {}
    )
    event_diagnostics: Counter[str] = Counter()
    for event in events:
        for item in (
            event.get("diagnostics", []) if isinstance(event.get("diagnostics"), list) else []
        ):
            if isinstance(item, dict):
                event_diagnostics[str(item.get("code"))] += 1
    if dict(sorted(summary_diagnostics.items())) != sorted_counter(event_diagnostics):
        diagnostics.append(
            diagnostic(
                "summary_event_mismatch",
                "$.diagnostic_counts",
                "Summary diagnostic_counts must match per-event diagnostics.",
                details={
                    "summary": summary_diagnostics,
                    "events": sorted_counter(event_diagnostics),
                },
            )
        )
    return diagnostics


def verify_contract(
    *,
    selection: dict[str, Any],
    events: list[dict[str, Any]],
    summary: dict[str, Any],
    repo_root: Path,
    reject_unsafe_claims: bool,
) -> list[dict[str, Any]]:
    refs = refs_from_selection(selection)
    diagnostics: list[dict[str, Any]] = []
    diagnostics.extend(validate_schema(events, summary))
    diagnostics.extend(validate_expanded_counts(refs, events, summary))
    diagnostics.extend(validate_selection_event_alignment(refs, events))
    diagnostics.extend(validate_required_nullable_fields(events))
    diagnostics.extend(validate_source_counts(events, summary))
    diagnostics.extend(validate_candidate_classification(events, summary))
    diagnostics.extend(validate_pdf_statuses(events, summary))
    diagnostics.extend(validate_artifacts(events, summary, repo_root=repo_root))
    diagnostics.extend(validate_duplicate_identity(refs, events, summary))
    diagnostics.extend(validate_safety(events, summary, reject_unsafe_claims=reject_unsafe_claims))
    diagnostics.extend(validate_summary_consistency(events, summary))
    return diagnostics


def verify_files(
    *,
    selection_path: Path,
    events_path: Path,
    summary_path: Path,
    repo_root: Path,
    reject_unsafe_claims: bool,
) -> list[dict[str, Any]]:
    return verify_contract(
        selection=read_json(selection_path),
        events=read_jsonl(events_path),
        summary=read_json(summary_path),
        repo_root=repo_root,
        reject_unsafe_claims=reject_unsafe_claims,
    )


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--events", required=True, type=Path)
    parser.add_argument("--summary", required=True, type=Path)
    parser.add_argument("--reject-unsafe-claims", action="store_true")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        diagnostics = verify_files(
            selection_path=args.selection,
            events_path=args.events,
            summary_path=args.summary,
            repo_root=Path.cwd(),
            reject_unsafe_claims=args.reject_unsafe_claims,
        )
    except VerificationError as exc:
        sys.stderr.write(f"verification_input_error:{exc}\n")
        return 2

    if diagnostics:
        for item in diagnostics:
            sys.stderr.write(json.dumps(item, sort_keys=True) + "\n")
        sys.stderr.write(
            f"PDF acquisition diagnostics verification failed: diagnostics={len(diagnostics)}\n"
        )
        return 1

    sys.stdout.write(
        "PDF acquisition diagnostics verification passed: "
        f"refs={EXPECTED_URL_REF_COUNT} identities={EXPECTED_NORMALIZED_IDENTITY_COUNT} "
        f"new_refs={','.join(sorted(EXPECTED_NEW_REF_IDS))} existing_pdfs={','.join(sorted(EXPECTED_EXISTING_PDF_REFS))}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
