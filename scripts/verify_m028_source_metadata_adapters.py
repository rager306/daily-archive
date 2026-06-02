#!/usr/bin/env python3
"""Verify expanded M028 S02 source metadata adapter outputs.

This verifier is intentionally metadata-only. It validates the accepted 21-URL-ref
contract, artifact/provenance linkage, nullable optional metadata diagnostics, and
fail-closed safety flags without reading parser outputs, graph state, source bodies,
or production persistence surfaces.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

EXPECTED_EVENT_SCHEMA_VERSION = "m028.source-metadata-event.v1"
EXPECTED_SUMMARY_SCHEMA_VERSION = "m028.source-metadata-summary.v1"
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
EXPECTED_DUPLICATE_IDENTITY = "arxiv:2605.20897"
EXPECTED_DUPLICATE_REF_IDS = ["R01", "R10"]

REQUIRED_OPTIONAL_FIELDS = (
    "title",
    "authors",
    "published_date",
    "updated_date",
    "doi",
    "artifact_arxiv_id",
    "pdf_url",
)

UNSAFE_SUMMARY_FLAGS = (
    "graph_write_attempted",
    "production_persistence_attempted",
    "parser_readiness_claimed",
    "kg_readiness_claimed",
    "dspy_attempted",
    "rlm_attempted",
    "minimax_attempted",
    "source_payload_embedded",
    "binary_payload_embedded",
    "chunk_payload_embedded",
    "model_output_embedded",
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
    "ladybugdb_written",
)

SAFE_DIAGNOSTIC_CODES = {
    "corpus_scope_stale",
    "missing_new_refs",
    "schema_mismatch",
    "identity_count_mismatch",
    "source_kind_drift",
    "source_family_drift",
    "duplicate_identity_mismatch",
    "selection_metadata_mismatch",
    "missing_metadata_event",
    "unexpected_metadata_event",
    "acquisition_reference_broken",
    "artifact_reference_broken",
    "artifact_checksum_unverified",
    "terminal_acquisition_mismatch",
    "required_nullable_field_missing",
    "optional_metadata_gap_missing",
    "unsafe_claim_detected",
    "raw_payload_leakage",
    "summary_event_mismatch",
}


class VerificationError(RuntimeError):
    """Raised when input files cannot be parsed enough to validate."""


def diagnostic(code: str, json_path: str, message: str, *, details: dict[str, Any] | None = None) -> dict[str, Any]:
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


def validate_expanded_counts(refs: list[dict[str, Any]], events: list[dict[str, Any]], summary: dict[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    if len(refs) != EXPECTED_URL_REF_COUNT or len(events) != EXPECTED_URL_REF_COUNT or summary.get("url_ref_count") != EXPECTED_URL_REF_COUNT:
        diagnostics.append(
            diagnostic(
                "corpus_scope_stale",
                "$.url_ref_count",
                "M028 S02 must validate the expanded 21-URL-ref corpus, not the stale 14-ref scope.",
                details={"selection": len(refs), "events": len(events), "summary": summary.get("url_ref_count")},
            )
        )

    selection_ref_ids = {str(ref.get("ref_id")) for ref in refs}
    event_ref_ids = {str(event.get("ref_id")) for event in events}
    missing_new_refs = sorted(EXPECTED_NEW_REF_IDS - selection_ref_ids) + sorted(EXPECTED_NEW_REF_IDS - event_ref_ids)
    if missing_new_refs:
        diagnostics.append(
            diagnostic(
                "missing_new_refs",
                "$.refs",
                "Expanded arXiv refs R15-R21 must be present in both selection and metadata events.",
                details={"missing": sorted(set(missing_new_refs))},
            )
        )

    identity_count = len({str(ref.get("normalized_identity")) for ref in refs})
    if identity_count != EXPECTED_NORMALIZED_IDENTITY_COUNT or summary.get("normalized_identity_count") != EXPECTED_NORMALIZED_IDENTITY_COUNT:
        diagnostics.append(
            diagnostic(
                "identity_count_mismatch",
                "$.normalized_identity_count",
                "M028 S02 must preserve 20 normalized identities across 21 URL refs.",
                details={"selection": identity_count, "summary": summary.get("normalized_identity_count")},
            )
        )
    return diagnostics


def validate_source_counts(refs: list[dict[str, Any]], events: list[dict[str, Any]], summary: dict[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    selection_kind_counts = counter_from(refs, "source_kind")
    event_kind_counts = counter_from(events, "source_kind")
    if selection_kind_counts != EXPECTED_SOURCE_KIND_COUNTS or event_kind_counts != EXPECTED_SOURCE_KIND_COUNTS or summary.get("source_kind_counts") != EXPECTED_SOURCE_KIND_COUNTS:
        diagnostics.append(
            diagnostic(
                "source_kind_drift",
                "$.source_kind_counts",
                "Source kind counts drifted from the accepted expanded corpus contract.",
                details={"selection": selection_kind_counts, "events": event_kind_counts, "summary": summary.get("source_kind_counts")},
            )
        )

    event_family_counts = counter_from(events, "source_family")
    if event_family_counts != EXPECTED_SOURCE_FAMILY_COUNTS or summary.get("source_family_counts") != EXPECTED_SOURCE_FAMILY_COUNTS:
        diagnostics.append(
            diagnostic(
                "source_family_drift",
                "$.source_family_counts",
                "Source family classification counts drifted from arXiv/company-blog/Nature expectations.",
                details={"events": event_family_counts, "summary": summary.get("source_family_counts")},
            )
        )
    return diagnostics


def validate_duplicate_identity(refs: list[dict[str, Any]], events: list[dict[str, Any]], summary: dict[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    refs_by_identity: dict[str, list[str]] = defaultdict(list)
    for ref in refs:
        refs_by_identity[str(ref.get("normalized_identity"))].append(str(ref.get("ref_id")))

    duplicate_ref_ids = refs_by_identity.get(EXPECTED_DUPLICATE_IDENTITY, [])
    summary_groups = summary.get("duplicate_identity_groups") if isinstance(summary.get("duplicate_identity_groups"), list) else []
    expected_summary_group = next(
        (group for group in summary_groups if isinstance(group, dict) and group.get("normalized_identity") == EXPECTED_DUPLICATE_IDENTITY),
        None,
    )
    event_groups = [event.get("identity_group") for event in events if event.get("normalized_identity") == EXPECTED_DUPLICATE_IDENTITY]
    event_group_ref_ids = [group.get("ref_ids") for group in event_groups if isinstance(group, dict)]
    if (
        duplicate_ref_ids != EXPECTED_DUPLICATE_REF_IDS
        or not isinstance(expected_summary_group, dict)
        or expected_summary_group.get("ref_ids") != EXPECTED_DUPLICATE_REF_IDS
        or any(ref_ids != EXPECTED_DUPLICATE_REF_IDS for ref_ids in event_group_ref_ids)
        or summary.get("duplicate_identity_group_count") != 1
    ):
        diagnostics.append(
            diagnostic(
                "duplicate_identity_mismatch",
                "$.duplicate_identity_groups",
                "The arxiv:2605.20897 duplicate URL identity must remain exactly R01/R10.",
                details={"selection": duplicate_ref_ids, "summary_group": expected_summary_group, "event_groups": event_group_ref_ids},
            )
        )
    return diagnostics


def validate_schema(events: list[dict[str, Any]], summary: dict[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    if summary.get("schema_version") != EXPECTED_SUMMARY_SCHEMA_VERSION:
        diagnostics.append(
            diagnostic(
                "schema_mismatch",
                "$.schema_version",
                "Summary schema version does not match the metadata adapter verifier contract.",
                details={"actual": summary.get("schema_version"), "expected": EXPECTED_SUMMARY_SCHEMA_VERSION},
            )
        )
    bad_event_refs = [event.get("ref_id") for event in events if event.get("schema_version") != EXPECTED_EVENT_SCHEMA_VERSION]
    if bad_event_refs:
        diagnostics.append(
            diagnostic(
                "schema_mismatch",
                "$.events[*].schema_version",
                "One or more metadata events have an unexpected schema version.",
                details={"ref_ids": bad_event_refs, "expected": EXPECTED_EVENT_SCHEMA_VERSION},
            )
        )
    return diagnostics


def validate_selection_event_alignment(refs: list[dict[str, Any]], events: list[dict[str, Any]]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    refs_by_id = by_ref_id(refs, "selection")
    events_by_id = by_ref_id(events, "metadata")
    missing = sorted(set(refs_by_id) - set(events_by_id))
    unexpected = sorted(set(events_by_id) - set(refs_by_id))
    if missing:
        diagnostics.append(diagnostic("missing_metadata_event", "$.events", "Selection refs are missing metadata events.", details={"ref_ids": missing}))
    if unexpected:
        diagnostics.append(diagnostic("unexpected_metadata_event", "$.events", "Metadata events include refs absent from selection.", details={"ref_ids": unexpected}))

    for ref_id in sorted(set(refs_by_id) & set(events_by_id)):
        ref = refs_by_id[ref_id]
        event = events_by_id[ref_id]
        mismatches = {
            field: {"selection": ref.get(field), "event": event.get(field)}
            for field in ("url", "canonical_url", "source_kind", "normalized_identity")
            if ref.get(field) != event.get(field)
        }
        if mismatches:
            diagnostics.append(
                diagnostic(
                    "selection_metadata_mismatch",
                    f"$.events[ref_id={ref_id}]",
                    "Metadata event must preserve selection URL, canonical URL, kind, and identity.",
                    details={"ref_id": ref_id, "mismatches": mismatches},
                )
            )
    return diagnostics


def validate_acquisition_linkage(
    events: list[dict[str, Any]], acquisition_events: list[dict[str, Any]], *, repo_root: Path
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    acquisition_by_id = by_ref_id(acquisition_events, "acquisition")
    for event in events:
        ref_id = str(event.get("ref_id"))
        acquisition = acquisition_by_id.get(ref_id)
        if acquisition is None:
            diagnostics.append(
                diagnostic(
                    "acquisition_reference_broken",
                    f"$.events[ref_id={ref_id}].acquisition",
                    "Metadata event has no matching acquisition event.",
                    details={"ref_id": ref_id},
                )
            )
            continue

        event_acquisition = event.get("acquisition") if isinstance(event.get("acquisition"), dict) else {}
        if event_acquisition.get("status") != acquisition.get("status") or event_acquisition.get("terminal") != acquisition.get("terminal"):
            diagnostics.append(
                diagnostic(
                    "terminal_acquisition_mismatch",
                    f"$.events[ref_id={ref_id}].acquisition",
                    "Metadata acquisition status/terminal fields must link to terminal acquisition events.",
                    details={"ref_id": ref_id, "event": event_acquisition, "acquisition": acquisition},
                )
            )
        if acquisition.get("terminal") is not True or acquisition.get("status") != "captured":
            diagnostics.append(
                diagnostic(
                    "terminal_acquisition_mismatch",
                    f"$.acquisition_events[ref_id={ref_id}]",
                    "Expanded S02 expects terminal captured acquisition events for all refs.",
                    details={"ref_id": ref_id, "status": acquisition.get("status"), "terminal": acquisition.get("terminal")},
                )
            )

        event_artifact = event.get("artifact") if isinstance(event.get("artifact"), dict) else {}
        artifact_path = event_artifact.get("path")
        acquisition_path = acquisition.get("artifact_path")
        if artifact_path != acquisition_path or not isinstance(artifact_path, str) or not artifact_path:
            diagnostics.append(
                diagnostic(
                    "artifact_reference_broken",
                    f"$.events[ref_id={ref_id}].artifact.path",
                    "Metadata artifact path must match the acquisition artifact reference.",
                    details={"ref_id": ref_id, "event_path": artifact_path, "acquisition_path": acquisition_path},
                )
            )
            continue
        resolved_artifact = repo_root / artifact_path
        if not resolved_artifact.exists():
            diagnostics.append(
                diagnostic(
                    "artifact_reference_broken",
                    f"$.events[ref_id={ref_id}].artifact.path",
                    "Referenced acquisition artifact does not exist under the repository root.",
                    details={"ref_id": ref_id, "artifact_path": artifact_path},
                )
            )
        if event_artifact.get("sha256") != acquisition.get("sha256") or event_artifact.get("checksum_verified") is not True:
            diagnostics.append(
                diagnostic(
                    "artifact_checksum_unverified",
                    f"$.events[ref_id={ref_id}].artifact.sha256",
                    "Metadata artifact checksum must match acquisition and be marked verified.",
                    details={"ref_id": ref_id},
                )
            )
    return diagnostics


def validate_optional_metadata(events: list[dict[str, Any]]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    for event in events:
        ref_id = str(event.get("ref_id"))
        optional_metadata = event.get("optional_metadata") if isinstance(event.get("optional_metadata"), dict) else {}
        gaps = event.get("optional_metadata_gaps") if isinstance(event.get("optional_metadata_gaps"), list) else []
        gap_fields = {gap.get("field") for gap in gaps if isinstance(gap, dict)}
        event_diagnostics = event.get("diagnostics") if isinstance(event.get("diagnostics"), list) else []
        diagnostic_gap_fields = {
            item.get("details", {}).get("field")
            for item in event_diagnostics
            if isinstance(item, dict) and item.get("code") == "optional_metadata_missing" and isinstance(item.get("details"), dict)
        }
        for field in REQUIRED_OPTIONAL_FIELDS:
            field_value = optional_metadata.get(field)
            if not isinstance(field_value, dict) or "status" not in field_value or "value" not in field_value or "missing_reason" not in field_value:
                diagnostics.append(
                    diagnostic(
                        "required_nullable_field_missing",
                        f"$.events[ref_id={ref_id}].optional_metadata.{field}",
                        "Optional metadata fields must be present as typed nullable records.",
                        details={"ref_id": ref_id, "field": field},
                    )
                )
                continue
            if field_value.get("status") == "missing" and (field not in gap_fields or field not in diagnostic_gap_fields):
                diagnostics.append(
                    diagnostic(
                        "optional_metadata_gap_missing",
                        f"$.events[ref_id={ref_id}].optional_metadata.{field}",
                        "Missing optional metadata must be represented in both gaps and diagnostics.",
                        details={"ref_id": ref_id, "field": field},
                    )
                )
    return diagnostics


def iter_nested_string_values(payload: Any) -> Any:
    if isinstance(payload, dict):
        for value in payload.values():
            yield from iter_nested_string_values(value)
    elif isinstance(payload, list):
        for value in payload:
            yield from iter_nested_string_values(value)
    elif isinstance(payload, str):
        yield payload


def validate_metadata_only(events: list[dict[str, Any]], summary: dict[str, Any], *, reject_unsafe_claims: bool) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    payload_blob = "\n".join(str(item).lower() for item in iter_nested_string_values({"events": events, "summary": summary}))
    leaked_markers = sorted(marker for marker in FORBIDDEN_PAYLOAD_MARKERS if marker in payload_blob)
    if leaked_markers:
        diagnostics.append(
            diagnostic(
                "raw_payload_leakage",
                "$",
                "Metadata adapter outputs must not embed raw source payloads or graph/parser/KG readiness markers.",
                details={"markers": leaked_markers},
            )
        )

    unsafe_claims: list[dict[str, Any]] = []
    summary_flags = summary.get("safety_flags") if isinstance(summary.get("safety_flags"), dict) else {}
    unsafe_counts = summary.get("unsafe_claim_counts") if isinstance(summary.get("unsafe_claim_counts"), dict) else {}
    for flag in UNSAFE_SUMMARY_FLAGS:
        if summary_flags.get(flag) not in (False, None):
            unsafe_claims.append({"path": f"$.summary.safety_flags.{flag}", "value": summary_flags.get(flag)})
        if unsafe_counts.get(flag, 0) not in (0, None):
            unsafe_claims.append({"path": f"$.summary.unsafe_claim_counts.{flag}", "value": unsafe_counts.get(flag)})
    for event in events:
        event_flags = event.get("safety_flags") if isinstance(event.get("safety_flags"), dict) else {}
        for flag, value in event_flags.items():
            if value is not False:
                unsafe_claims.append({"path": f"$.events[ref_id={event.get('ref_id')}].safety_flags.{flag}", "value": value})
        artifact = event.get("artifact") if isinstance(event.get("artifact"), dict) else {}
        if artifact.get("payload_embedded") is not False:
            unsafe_claims.append({"path": f"$.events[ref_id={event.get('ref_id')}].artifact.payload_embedded", "value": artifact.get("payload_embedded")})

    if unsafe_claims and reject_unsafe_claims:
        diagnostics.append(
            diagnostic(
                "unsafe_claim_detected",
                "$",
                "Unsafe graph/parser/KG/readiness/payload flags must remain fail-closed.",
                details={"claims": unsafe_claims},
            )
        )
    return diagnostics


def validate_summary_consistency(events: list[dict[str, Any]], summary: dict[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    if summary.get("ref_ids") != [event.get("ref_id") for event in events]:
        diagnostics.append(
            diagnostic(
                "summary_event_mismatch",
                "$.ref_ids",
                "Summary ref_ids must exactly match metadata event order.",
                details={"summary": summary.get("ref_ids"), "events": [event.get("ref_id") for event in events]},
            )
        )
    if summary.get("metadata_status_counts") != counter_from(events, "metadata_status"):
        diagnostics.append(
            diagnostic(
                "summary_event_mismatch",
                "$.metadata_status_counts",
                "Summary metadata_status_counts must match event rows.",
                details={"summary": summary.get("metadata_status_counts"), "events": counter_from(events, "metadata_status")},
            )
        )
    return diagnostics


def verify_contract(
    *,
    selection: dict[str, Any],
    acquisition_events: list[dict[str, Any]],
    metadata_events: list[dict[str, Any]],
    summary: dict[str, Any],
    repo_root: Path,
    reject_unsafe_claims: bool,
) -> list[dict[str, Any]]:
    refs = refs_from_selection(selection)
    diagnostics: list[dict[str, Any]] = []
    diagnostics.extend(validate_schema(metadata_events, summary))
    diagnostics.extend(validate_expanded_counts(refs, metadata_events, summary))
    diagnostics.extend(validate_source_counts(refs, metadata_events, summary))
    diagnostics.extend(validate_duplicate_identity(refs, metadata_events, summary))
    diagnostics.extend(validate_selection_event_alignment(refs, metadata_events))
    diagnostics.extend(validate_acquisition_linkage(metadata_events, acquisition_events, repo_root=repo_root))
    diagnostics.extend(validate_optional_metadata(metadata_events))
    diagnostics.extend(validate_metadata_only(metadata_events, summary, reject_unsafe_claims=reject_unsafe_claims))
    diagnostics.extend(validate_summary_consistency(metadata_events, summary))
    return diagnostics


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--acquisition-events", required=True, type=Path)
    parser.add_argument("--metadata-events", required=True, type=Path)
    parser.add_argument("--summary", required=True, type=Path)
    parser.add_argument("--reject-unsafe-claims", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path.cwd()
    try:
        diagnostics = verify_contract(
            selection=read_json(args.selection),
            acquisition_events=read_jsonl(args.acquisition_events),
            metadata_events=read_jsonl(args.metadata_events),
            summary=read_json(args.summary),
            repo_root=repo_root,
            reject_unsafe_claims=args.reject_unsafe_claims,
        )
    except VerificationError as exc:
        sys.stderr.write(f"verification_input_error:{exc}\n")
        return 2

    if diagnostics:
        for item in diagnostics:
            sys.stderr.write(json.dumps(item, sort_keys=True) + "\n")
        sys.stderr.write(f"metadata adapter verification failed: diagnostics={len(diagnostics)}\n")
        return 1

    sys.stdout.write(
        "metadata adapter verification passed: "
        f"refs={EXPECTED_URL_REF_COUNT} identities={EXPECTED_NORMALIZED_IDENTITY_COUNT} "
        f"new_refs={','.join(sorted(EXPECTED_NEW_REF_IDS))}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
