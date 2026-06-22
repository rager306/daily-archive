#!/usr/bin/env python3
"""Verify M028 S04 universal loader evidence bundles fail-closed.

The verifier is intentionally metadata-only: it reads JSON/JSONL contract
artifacts emitted by S02/S03/S04, validates linkage and aggregate consistency,
and rejects unsafe positive claims without fetching URLs, reading source bodies,
parsing PDFs, invoking models, or writing graph state.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

# Reuse the builder contract constants so verifier drift is visible in tests.
SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from build_m028_universal_loader_evidence_bundles import (  # noqa: E402
    EVENT_SCHEMA_VERSION,
    EXPECTED_IDENTITY_COUNT,
    EXPECTED_REF_COUNT,
    EXPECTED_SOURCE_KIND_COUNTS,
    FORBIDDEN_PAYLOAD_MARKERS,
    SAFETY_FLAGS,
    SUMMARY_SCHEMA_VERSION,
    UNSAFE_COUNTER_KEYS,
    classify_variant,
    count_unsafe_claims,
    metadata_status,
    source_family,
    source_quality_status,
    terminal_pdf_status,
)

EXPECTED_REF_IDS = [f"R{index:02d}" for index in range(1, EXPECTED_REF_COUNT + 1)]
EXPANDED_SCOPE_REF_IDS = [f"R{index:02d}" for index in range(15, EXPECTED_REF_COUNT + 1)]
REQUIRED_TOP_LEVEL_BUNDLE_KEYS = {
    "schema_version",
    "ref_id",
    "url",
    "canonical_url",
    "url_variant",
    "source_kind",
    "source_family",
    "normalized_identity",
    "identity_group",
    "selection",
    "source_metadata",
    "pdf_diagnostic",
    "artifact_refs",
    "loader_evidence",
    "safety_flags",
    "diagnostics",
}
REQUIRED_ARTIFACT_NAMES = ("source_artifact", "metadata_artifact", "pdf_artifact")
FORBIDDEN_KEYS = {
    "raw_text",
    "raw_payload",
    "source_body",
    "body_text",
    "html_document",
    "pdf_bytes",
    "binary_payload",
    "chunk_text",
    "chunk_payload",
    "model_output",
}


@dataclass(frozen=True)
class Diagnostic:
    """Stable verifier diagnostic for agent-inspectable failures."""

    code: str
    path: str
    message: str

    def render(self) -> str:
        return f"{self.code}\t{self.path}\t{self.message}"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_json(path: Path, diagnostics: list[Diagnostic], label: str) -> dict[str, Any] | None:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        diagnostics.append(
            Diagnostic("INPUT_MISSING", f"$.inputs.{label}", f"missing input: {path.as_posix()}")
        )
        return None
    except json.JSONDecodeError as exc:
        diagnostics.append(
            Diagnostic(
                "JSON_MALFORMED", f"$.inputs.{label}", f"malformed JSON at {exc.lineno}:{exc.colno}"
            )
        )
        return None
    if not isinstance(payload, dict):
        diagnostics.append(
            Diagnostic(
                "JSON_OBJECT_REQUIRED",
                f"$.inputs.{label}",
                "top-level JSON value must be an object",
            )
        )
        return None
    return payload


def read_jsonl(
    path: Path, diagnostics: list[Diagnostic], label: str
) -> list[dict[str, Any]] | None:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        diagnostics.append(
            Diagnostic("INPUT_MISSING", f"$.inputs.{label}", f"missing input: {path.as_posix()}")
        )
        return None
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as exc:
            diagnostics.append(
                Diagnostic(
                    "JSONL_MALFORMED",
                    f"$.inputs.{label}[{line_number}]",
                    f"malformed JSONL at column {exc.colno}",
                )
            )
            continue
        if not isinstance(row, dict):
            diagnostics.append(
                Diagnostic(
                    "JSONL_OBJECT_REQUIRED",
                    f"$.inputs.{label}[{line_number}]",
                    "JSONL row must be an object",
                )
            )
            continue
        rows.append(row)
    return rows


def rows_by_ref(
    rows: list[dict[str, Any]], label: str, diagnostics: list[Diagnostic]
) -> dict[str, dict[str, Any]]:
    by_ref: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(rows):
        ref_id = row.get("ref_id")
        if not isinstance(ref_id, str) or not ref_id:
            diagnostics.append(
                Diagnostic("REF_ID_REQUIRED", f"$.{label}[{index}].ref_id", "row is missing ref_id")
            )
            continue
        if ref_id in by_ref:
            diagnostics.append(
                Diagnostic(
                    "REF_ID_DUPLICATE", f"$.{label}[{index}].ref_id", f"duplicate ref_id {ref_id}"
                )
            )
            continue
        by_ref[ref_id] = row
    return by_ref


def json_path_join(prefix: str, part: str | int) -> str:
    if isinstance(part, int):
        return f"{prefix}[{part}]"
    return f"{prefix}.{part}" if prefix != "$" else f"$.{part}"


def walk_forbidden(payload: Any, diagnostics: list[Diagnostic], path: str = "$") -> None:
    if isinstance(payload, dict):
        for key, value in payload.items():
            child_path = json_path_join(path, str(key))
            if str(key) in FORBIDDEN_KEYS:
                diagnostics.append(
                    Diagnostic(
                        "FORBIDDEN_KEY_PRESENT", child_path, f"forbidden payload-bearing key {key}"
                    )
                )
            walk_forbidden(value, diagnostics, child_path)
    elif isinstance(payload, list):
        for index, value in enumerate(payload):
            walk_forbidden(value, diagnostics, json_path_join(path, index))
    elif isinstance(payload, str):
        lower = payload.lower()
        for marker in FORBIDDEN_PAYLOAD_MARKERS:
            if marker in lower:
                diagnostics.append(
                    Diagnostic("FORBIDDEN_MARKER_PRESENT", path, f"forbidden marker {marker}")
                )


def safe_relative_path(path_value: Any) -> bool:
    if path_value is None:
        return True
    if not isinstance(path_value, str) or not path_value.strip() or "://" in path_value:
        return False
    normalized = PurePosixPath(path_value.replace("\\", "/"))
    return (
        not normalized.is_absolute()
        and ".." not in normalized.parts
        and all(part for part in normalized.parts)
    )


def validate_selection(
    selection: dict[str, Any], diagnostics: list[Diagnostic]
) -> list[dict[str, Any]]:
    refs = selection.get("refs")
    if not isinstance(refs, list):
        diagnostics.append(
            Diagnostic(
                "SELECTION_REFS_REQUIRED", "$.selection.refs", "selection.refs must be a list"
            )
        )
        return []
    if len(refs) != EXPECTED_REF_COUNT:
        diagnostics.append(
            Diagnostic(
                "SCOPE_REF_COUNT_MISMATCH",
                "$.selection.refs",
                f"expected {EXPECTED_REF_COUNT} refs, found {len(refs)}",
            )
        )

    seen: set[str] = set()
    validated: list[dict[str, Any]] = []
    for index, ref in enumerate(refs):
        ref_path = f"$.selection.refs[{index}]"
        if not isinstance(ref, dict):
            diagnostics.append(
                Diagnostic(
                    "SELECTION_REF_OBJECT_REQUIRED", ref_path, "selection ref must be an object"
                )
            )
            continue
        for key in ("ref_id", "url", "canonical_url", "source_kind", "normalized_identity"):
            if not isinstance(ref.get(key), str) or not ref.get(key):
                diagnostics.append(
                    Diagnostic(
                        "SELECTION_REQUIRED_FIELD_MISSING",
                        f"{ref_path}.{key}",
                        f"selection ref requires {key}",
                    )
                )
        ref_id = ref.get("ref_id")
        if isinstance(ref_id, str):
            if ref_id in seen:
                diagnostics.append(
                    Diagnostic(
                        "SELECTION_REF_DUPLICATE",
                        f"{ref_path}.ref_id",
                        f"duplicate selection ref {ref_id}",
                    )
                )
            seen.add(ref_id)
        validated.append(ref)

    ref_ids = [str(ref.get("ref_id")) for ref in validated if isinstance(ref.get("ref_id"), str)]
    if ref_ids != EXPECTED_REF_IDS:
        diagnostics.append(
            Diagnostic(
                "SCOPE_REF_IDS_MISMATCH",
                "$.selection.refs[*].ref_id",
                f"expected ordered refs {EXPECTED_REF_IDS}",
            )
        )
    missing_expanded = sorted(set(EXPANDED_SCOPE_REF_IDS) - set(ref_ids))
    if missing_expanded:
        diagnostics.append(
            Diagnostic(
                "EXPANDED_SCOPE_REFS_MISSING",
                "$.selection.refs[*].ref_id",
                f"missing expanded refs {missing_expanded}",
            )
        )
    identities = [
        str(ref.get("normalized_identity"))
        for ref in validated
        if isinstance(ref.get("normalized_identity"), str)
    ]
    if len(set(identities)) != EXPECTED_IDENTITY_COUNT:
        diagnostics.append(
            Diagnostic(
                "SCOPE_IDENTITY_COUNT_MISMATCH",
                "$.selection.refs[*].normalized_identity",
                f"expected {EXPECTED_IDENTITY_COUNT} identities, found {len(set(identities))}",
            )
        )
    counts = dict(
        sorted(
            Counter(
                str(ref.get("source_kind"))
                for ref in validated
                if isinstance(ref.get("source_kind"), str)
            ).items()
        )
    )
    if counts != dict(sorted(EXPECTED_SOURCE_KIND_COUNTS.items())):
        diagnostics.append(
            Diagnostic(
                "SCOPE_SOURCE_KIND_COUNTS_MISMATCH",
                "$.selection.refs[*].source_kind",
                f"expected {EXPECTED_SOURCE_KIND_COUNTS}, found {counts}",
            )
        )
    duplicate_groups: dict[str, list[str]] = defaultdict(list)
    for ref in validated:
        if isinstance(ref.get("normalized_identity"), str) and isinstance(ref.get("ref_id"), str):
            duplicate_groups[str(ref["normalized_identity"])].append(str(ref["ref_id"]))
    duplicate_ref_groups = sorted(group for group in duplicate_groups.values() if len(group) > 1)
    if duplicate_ref_groups != [["R01", "R10"]]:
        diagnostics.append(
            Diagnostic(
                "DUPLICATE_IDENTITY_DRIFT",
                "$.selection.refs[*].normalized_identity",
                f"expected duplicate group ['R01', 'R10'], found {duplicate_ref_groups}",
            )
        )
    return validated


def expected_identity_group(
    ref: dict[str, Any], refs_by_identity: dict[str, list[dict[str, Any]]]
) -> dict[str, Any]:
    identity = str(ref["normalized_identity"])
    group_refs = refs_by_identity[identity]
    ref_ids = [str(item["ref_id"]) for item in group_refs]
    return {
        "group_id": f"identity:{identity}",
        "normalized_identity": identity,
        "ref_ids": ref_ids,
        "url_ref_count": len(ref_ids),
        "has_url_variants": len(ref_ids) > 1,
        "url_variants": [
            classify_variant(str(item["url"]), str(item["source_kind"])) for item in group_refs
        ],
    }


def validate_upstream_linkage(
    refs: list[dict[str, Any]],
    metadata_by_ref: dict[str, dict[str, Any]],
    pdf_by_ref: dict[str, dict[str, Any]],
    diagnostics: list[Diagnostic],
) -> None:
    ref_ids = {str(ref["ref_id"]) for ref in refs if isinstance(ref.get("ref_id"), str)}
    for label, by_ref in (("metadata_events", metadata_by_ref), ("pdf_events", pdf_by_ref)):
        missing = sorted(ref_ids - set(by_ref))
        extra = sorted(set(by_ref) - ref_ids)
        if missing or extra:
            diagnostics.append(
                Diagnostic(
                    "UPSTREAM_REF_SET_MISMATCH", f"$.{label}", f"missing={missing} extra={extra}"
                )
            )
    for ref in refs:
        if not isinstance(ref.get("ref_id"), str):
            continue
        ref_id = str(ref["ref_id"])
        for label, row in (
            ("metadata_events", metadata_by_ref.get(ref_id)),
            ("pdf_events", pdf_by_ref.get(ref_id)),
        ):
            if row is None:
                continue
            for key in ("url", "canonical_url", "source_kind", "normalized_identity"):
                if row.get(key) != ref.get(key):
                    diagnostics.append(
                        Diagnostic(
                            "UPSTREAM_LINKAGE_MISMATCH",
                            f"$.{label}.{ref_id}.{key}",
                            f"expected selection {key} for {ref_id}",
                        )
                    )


def validate_artifact_checks(
    bundle: dict[str, Any],
    metadata_event: dict[str, Any],
    pdf_event: dict[str, Any],
    diagnostics: list[Diagnostic],
    bundle_path: str,
) -> None:
    artifact_refs = bundle.get("artifact_refs")
    if not isinstance(artifact_refs, dict):
        diagnostics.append(
            Diagnostic(
                "ARTIFACT_REFS_OBJECT_REQUIRED",
                f"{bundle_path}.artifact_refs",
                "artifact_refs must be an object",
            )
        )
        return
    for artifact_name in REQUIRED_ARTIFACT_NAMES:
        artifact = artifact_refs.get(artifact_name)
        artifact_path = f"{bundle_path}.artifact_refs.{artifact_name}"
        if not isinstance(artifact, dict):
            diagnostics.append(
                Diagnostic("ARTIFACT_REF_REQUIRED", artifact_path, f"missing {artifact_name}")
            )
            continue
        if artifact.get("payload_embedded") is not False:
            diagnostics.append(
                Diagnostic(
                    "ARTIFACT_PAYLOAD_FLAG_UNSAFE",
                    f"{artifact_path}.payload_embedded",
                    "artifact payload_embedded must be false",
                )
            )
        if not safe_relative_path(artifact.get("path")):
            diagnostics.append(
                Diagnostic(
                    "ARTIFACT_PATH_UNSAFE",
                    f"{artifact_path}.path",
                    "artifact path must be null or safe repo-relative path",
                )
            )

    metadata_artifact = (
        metadata_event.get("artifact") if isinstance(metadata_event.get("artifact"), dict) else {}
    )
    bundle_metadata = (
        artifact_refs.get("metadata_artifact")
        if isinstance(artifact_refs.get("metadata_artifact"), dict)
        else {}
    )
    for key in ("path", "sha256", "byte_count", "content_type"):
        expected = metadata_artifact.get(key)
        if bundle_metadata.get(key) != expected:
            diagnostics.append(
                Diagnostic(
                    "METADATA_ARTIFACT_LINKAGE_MISMATCH",
                    f"{bundle_path}.artifact_refs.metadata_artifact.{key}",
                    f"metadata artifact {key} drift",
                )
            )
    if metadata_artifact.get("checksum_verified") is True and not isinstance(
        bundle_metadata.get("sha256"), str
    ):
        diagnostics.append(
            Diagnostic(
                "METADATA_CHECKSUM_SIGNATURE_MISSING",
                f"{bundle_path}.artifact_refs.metadata_artifact.sha256",
                "checksum_verified metadata artifact requires sha256",
            )
        )

    pdf_artifact = (
        pdf_event.get("pdf_artifact") if isinstance(pdf_event.get("pdf_artifact"), dict) else {}
    )
    bundle_pdf = (
        artifact_refs.get("pdf_artifact")
        if isinstance(artifact_refs.get("pdf_artifact"), dict)
        else {}
    )
    for key in ("path", "sha256", "byte_count", "content_type"):
        expected = pdf_artifact.get(key)
        if bundle_pdf.get(key) != expected:
            diagnostics.append(
                Diagnostic(
                    "PDF_ARTIFACT_LINKAGE_MISMATCH",
                    f"{bundle_path}.artifact_refs.pdf_artifact.{key}",
                    f"PDF artifact {key} drift",
                )
            )
    if pdf_artifact.get("checksum_verified") is True and not isinstance(
        bundle_pdf.get("sha256"), str
    ):
        diagnostics.append(
            Diagnostic(
                "PDF_CHECKSUM_SIGNATURE_MISSING",
                f"{bundle_path}.artifact_refs.pdf_artifact.sha256",
                "checksum_verified PDF artifact requires sha256",
            )
        )
    if (
        pdf_artifact.get("signature_verified") is True
        and bundle_pdf.get("payload_embedded") is not False
    ):
        diagnostics.append(
            Diagnostic(
                "PDF_SIGNATURE_PAYLOAD_UNSAFE",
                f"{bundle_path}.artifact_refs.pdf_artifact.payload_embedded",
                "signature-verified PDF evidence must remain metadata-only",
            )
        )


def validate_bundle_contract(
    refs: list[dict[str, Any]],
    bundles: list[dict[str, Any]],
    metadata_by_ref: dict[str, dict[str, Any]],
    pdf_by_ref: dict[str, dict[str, Any]],
    diagnostics: list[Diagnostic],
) -> None:
    if len(bundles) != EXPECTED_REF_COUNT:
        diagnostics.append(
            Diagnostic(
                "BUNDLE_REF_COUNT_MISMATCH",
                "$.bundles",
                f"expected {EXPECTED_REF_COUNT} bundles, found {len(bundles)}",
            )
        )
    bundle_by_ref = rows_by_ref(bundles, "bundles", diagnostics)
    refs_by_ref = {str(ref["ref_id"]): ref for ref in refs if isinstance(ref.get("ref_id"), str)}
    refs_by_identity: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for ref in refs:
        if isinstance(ref.get("normalized_identity"), str):
            refs_by_identity[str(ref["normalized_identity"])].append(ref)
    if set(bundle_by_ref) != set(refs_by_ref):
        diagnostics.append(
            Diagnostic(
                "BUNDLE_REF_SET_MISMATCH",
                "$.bundles[*].ref_id",
                f"missing={sorted(set(refs_by_ref) - set(bundle_by_ref))} extra={sorted(set(bundle_by_ref) - set(refs_by_ref))}",
            )
        )

    for index, bundle in enumerate(bundles):
        bundle_path = f"$.bundles[{index}]"
        ref_id = bundle.get("ref_id")
        if not isinstance(ref_id, str) or ref_id not in refs_by_ref:
            continue
        ref = refs_by_ref[ref_id]
        missing_keys = sorted(REQUIRED_TOP_LEVEL_BUNDLE_KEYS - set(bundle))
        if missing_keys:
            diagnostics.append(
                Diagnostic("BUNDLE_REQUIRED_KEYS_MISSING", bundle_path, f"missing {missing_keys}")
            )
        if bundle.get("schema_version") != EVENT_SCHEMA_VERSION:
            diagnostics.append(
                Diagnostic(
                    "BUNDLE_SCHEMA_VERSION_MISMATCH",
                    f"{bundle_path}.schema_version",
                    f"expected {EVENT_SCHEMA_VERSION}",
                )
            )
        for key in ("url", "canonical_url", "source_kind", "normalized_identity"):
            if bundle.get(key) != ref.get(key):
                diagnostics.append(
                    Diagnostic(
                        "BUNDLE_SELECTION_LINKAGE_MISMATCH",
                        f"{bundle_path}.{key}",
                        f"expected selection {key}",
                    )
                )
        if bundle.get("url_variant") != classify_variant(str(ref["url"]), str(ref["source_kind"])):
            diagnostics.append(
                Diagnostic(
                    "BUNDLE_URL_VARIANT_MISMATCH",
                    f"{bundle_path}.url_variant",
                    "url variant mapping drift",
                )
            )
        if bundle.get("source_family") != source_family(str(ref["source_kind"])):
            diagnostics.append(
                Diagnostic(
                    "BUNDLE_SOURCE_FAMILY_MISMATCH",
                    f"{bundle_path}.source_family",
                    "source family mapping drift",
                )
            )
        if bundle.get("identity_group") != expected_identity_group(ref, refs_by_identity):
            diagnostics.append(
                Diagnostic(
                    "BUNDLE_IDENTITY_GROUP_MISMATCH",
                    f"{bundle_path}.identity_group",
                    "identity group drift",
                )
            )

        metadata_event = metadata_by_ref.get(ref_id, {})
        pdf_event = pdf_by_ref.get(ref_id, {})
        source_metadata = (
            bundle.get("source_metadata") if isinstance(bundle.get("source_metadata"), dict) else {}
        )
        if source_metadata.get("metadata_status") != metadata_status(metadata_event):
            diagnostics.append(
                Diagnostic(
                    "SOURCE_METADATA_STATUS_MISMATCH",
                    f"{bundle_path}.source_metadata.metadata_status",
                    "metadata status mapping drift",
                )
            )
        if source_metadata.get("optional_metadata_gaps") != (
            metadata_event.get("optional_metadata_gaps")
            if isinstance(metadata_event.get("optional_metadata_gaps"), list)
            else []
        ):
            diagnostics.append(
                Diagnostic(
                    "SOURCE_METADATA_GAPS_MISMATCH",
                    f"{bundle_path}.source_metadata.optional_metadata_gaps",
                    "optional metadata gap linkage drift",
                )
            )

        pdf_diagnostic = (
            bundle.get("pdf_diagnostic") if isinstance(bundle.get("pdf_diagnostic"), dict) else {}
        )
        expected_status, expected_reason, expected_terminal = terminal_pdf_status(pdf_event)
        if (
            pdf_diagnostic.get("status"),
            pdf_diagnostic.get("reason"),
            pdf_diagnostic.get("terminal"),
        ) != (expected_status, expected_reason, expected_terminal):
            diagnostics.append(
                Diagnostic(
                    "PDF_DIAGNOSTIC_STATUS_MISMATCH",
                    f"{bundle_path}.pdf_diagnostic",
                    "PDF terminal diagnostic drift",
                )
            )
        expected_candidate = (
            (pdf_event.get("candidate_pdf") or {}).get("candidate_kind")
            if isinstance(pdf_event.get("candidate_pdf"), dict)
            else None
        )
        if pdf_diagnostic.get("candidate_kind") != expected_candidate:
            diagnostics.append(
                Diagnostic(
                    "PDF_CANDIDATE_KIND_MISMATCH",
                    f"{bundle_path}.pdf_diagnostic.candidate_kind",
                    "candidate PDF mapping drift",
                )
            )

        loader_evidence = (
            bundle.get("loader_evidence") if isinstance(bundle.get("loader_evidence"), dict) else {}
        )
        if loader_evidence.get("source_quality_status") != source_quality_status(
            metadata_event, pdf_event
        ):
            diagnostics.append(
                Diagnostic(
                    "SOURCE_QUALITY_STATUS_MISMATCH",
                    f"{bundle_path}.loader_evidence.source_quality_status",
                    "source quality mapping drift",
                )
            )
        expected_false_evidence = (
            "hermes_digest_ready",
            "parser_output_available",
            "kg_import_eligible",
            "production_import_eligible",
        )
        for key in expected_false_evidence:
            if loader_evidence.get(key) is not False:
                diagnostics.append(
                    Diagnostic(
                        "LOADER_EVIDENCE_UNSAFE_POSITIVE",
                        f"{bundle_path}.loader_evidence.{key}",
                        f"{key} must be false",
                    )
                )
        if loader_evidence.get("outcome") != "safe_for_downstream_metadata_projection_only":
            diagnostics.append(
                Diagnostic(
                    "LOADER_EVIDENCE_OUTCOME_UNSAFE",
                    f"{bundle_path}.loader_evidence.outcome",
                    "outcome must remain metadata projection only",
                )
            )

        safety_flags = bundle.get("safety_flags")
        if safety_flags != SAFETY_FLAGS:
            diagnostics.append(
                Diagnostic(
                    "SAFETY_FLAGS_MISMATCH",
                    f"{bundle_path}.safety_flags",
                    "safety flags must exactly match fail-closed contract",
                )
            )
        validate_artifact_checks(bundle, metadata_event, pdf_event, diagnostics, bundle_path)


def validate_summary(
    summary: dict[str, Any],
    bundles: list[dict[str, Any]],
    selection_path: Path,
    metadata_events_path: Path,
    pdf_events_path: Path,
    diagnostics: list[Diagnostic],
) -> None:
    if summary.get("schema_version") != SUMMARY_SCHEMA_VERSION:
        diagnostics.append(
            Diagnostic(
                "SUMMARY_SCHEMA_VERSION_MISMATCH",
                "$.summary.schema_version",
                f"expected {SUMMARY_SCHEMA_VERSION}",
            )
        )
    if summary.get("url_ref_count") != len(bundles) or summary.get("ref_count") != len(bundles):
        diagnostics.append(
            Diagnostic(
                "SUMMARY_BUNDLE_COUNT_MISMATCH",
                "$.summary.url_ref_count",
                "summary count does not match bundles",
            )
        )
    bundle_ref_ids = [
        str(bundle.get("ref_id")) for bundle in bundles if isinstance(bundle.get("ref_id"), str)
    ]
    if summary.get("ref_ids") != bundle_ref_ids:
        diagnostics.append(
            Diagnostic(
                "SUMMARY_REF_IDS_MISMATCH",
                "$.summary.ref_ids",
                "summary ref_ids must match bundle order",
            )
        )
    identity_count = len(
        {
            str(bundle.get("normalized_identity"))
            for bundle in bundles
            if isinstance(bundle.get("normalized_identity"), str)
        }
    )
    if summary.get("normalized_identity_count") != identity_count:
        diagnostics.append(
            Diagnostic(
                "SUMMARY_IDENTITY_COUNT_MISMATCH",
                "$.summary.normalized_identity_count",
                "summary identity count drift",
            )
        )
    source_kind_counts = dict(
        sorted(
            Counter(
                str(bundle.get("source_kind"))
                for bundle in bundles
                if isinstance(bundle.get("source_kind"), str)
            ).items()
        )
    )
    if summary.get("source_kind_counts") != source_kind_counts:
        diagnostics.append(
            Diagnostic(
                "SUMMARY_SOURCE_KIND_COUNTS_MISMATCH",
                "$.summary.source_kind_counts",
                "source kind count drift",
            )
        )
    quality_counts = dict(
        sorted(
            Counter(
                str((bundle.get("loader_evidence") or {}).get("source_quality_status"))
                for bundle in bundles
                if isinstance(bundle.get("loader_evidence"), dict)
            ).items()
        )
    )
    if summary.get("source_quality_status_counts") != quality_counts:
        diagnostics.append(
            Diagnostic(
                "SUMMARY_QUALITY_COUNTS_MISMATCH",
                "$.summary.source_quality_status_counts",
                "quality count drift",
            )
        )
    pdf_counts = dict(
        sorted(
            Counter(
                str((bundle.get("pdf_diagnostic") or {}).get("status"))
                for bundle in bundles
                if isinstance(bundle.get("pdf_diagnostic"), dict)
            ).items()
        )
    )
    if summary.get("pdf_status_counts") != pdf_counts:
        diagnostics.append(
            Diagnostic(
                "SUMMARY_PDF_COUNTS_MISMATCH",
                "$.summary.pdf_status_counts",
                "PDF status count drift",
            )
        )
    if summary.get("unsafe_claim_counts") != count_unsafe_claims(bundles):
        diagnostics.append(
            Diagnostic(
                "SUMMARY_UNSAFE_COUNTS_MISMATCH",
                "$.summary.unsafe_claim_counts",
                "unsafe claim counters drift from bundles",
            )
        )
    expected_fingerprints = {
        "selection": selection_path,
        "metadata_events": metadata_events_path,
        "pdf_events": pdf_events_path,
    }
    fingerprints = (
        summary.get("input_fingerprints")
        if isinstance(summary.get("input_fingerprints"), dict)
        else {}
    )
    for name, path in expected_fingerprints.items():
        fingerprint = fingerprints.get(name) if isinstance(fingerprints.get(name), dict) else {}
        if fingerprint.get("sha256") != sha256_file(path):
            diagnostics.append(
                Diagnostic(
                    "SUMMARY_INPUT_FINGERPRINT_MISMATCH",
                    f"$.summary.input_fingerprints.{name}.sha256",
                    f"fingerprint drift for {name}",
                )
            )


def validate_unsafe_counters(
    summary: dict[str, Any],
    bundles: list[dict[str, Any]],
    diagnostics: list[Diagnostic],
    reject_unsafe_claims: bool,
) -> None:
    computed = count_unsafe_claims(bundles)
    summary_counts = (
        summary.get("unsafe_claim_counts")
        if isinstance(summary.get("unsafe_claim_counts"), dict)
        else {}
    )
    for key in UNSAFE_COUNTER_KEYS:
        if key not in summary_counts:
            diagnostics.append(
                Diagnostic(
                    "UNSAFE_COUNTER_MISSING",
                    f"$.summary.unsafe_claim_counts.{key}",
                    "unsafe counter missing from summary",
                )
            )
        if computed.get(key, 0) != 0:
            diagnostics.append(
                Diagnostic(
                    "UNSAFE_CLAIM_IN_BUNDLE",
                    f"$.bundles[*].{key}",
                    f"computed unsafe counter {key}={computed.get(key, 0)}",
                )
            )
        if reject_unsafe_claims and summary_counts.get(key) != 0:
            diagnostics.append(
                Diagnostic(
                    "UNSAFE_CLAIM_REJECTED",
                    f"$.summary.unsafe_claim_counts.{key}",
                    f"summary unsafe counter {key}={summary_counts.get(key)}",
                )
            )


def verify_contract(
    selection_path: Path,
    metadata_events_path: Path,
    pdf_events_path: Path,
    bundles_path: Path,
    summary_path: Path,
    *,
    reject_unsafe_claims: bool = False,
) -> list[Diagnostic]:
    diagnostics: list[Diagnostic] = []
    selection = read_json(selection_path, diagnostics, "selection")
    metadata_rows = read_jsonl(metadata_events_path, diagnostics, "metadata_events")
    pdf_rows = read_jsonl(pdf_events_path, diagnostics, "pdf_events")
    bundles = read_jsonl(bundles_path, diagnostics, "bundles")
    summary = read_json(summary_path, diagnostics, "summary")
    if diagnostics:
        return diagnostics
    assert (
        selection is not None
        and metadata_rows is not None
        and pdf_rows is not None
        and bundles is not None
        and summary is not None
    )

    walk_forbidden(selection, diagnostics, "$.selection")
    walk_forbidden(metadata_rows, diagnostics, "$.metadata_events")
    walk_forbidden(pdf_rows, diagnostics, "$.pdf_events")
    walk_forbidden(bundles, diagnostics, "$.bundles")
    summary_for_marker_scan = {key: value for key, value in summary.items() if key != "report"}
    walk_forbidden(summary_for_marker_scan, diagnostics, "$.summary")

    refs = validate_selection(selection, diagnostics)
    metadata_by_ref = rows_by_ref(metadata_rows, "metadata_events", diagnostics)
    pdf_by_ref = rows_by_ref(pdf_rows, "pdf_events", diagnostics)
    validate_upstream_linkage(refs, metadata_by_ref, pdf_by_ref, diagnostics)
    validate_bundle_contract(refs, bundles, metadata_by_ref, pdf_by_ref, diagnostics)
    validate_summary(
        summary, bundles, selection_path, metadata_events_path, pdf_events_path, diagnostics
    )
    validate_unsafe_counters(summary, bundles, diagnostics, reject_unsafe_claims)
    return diagnostics


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--metadata-events", required=True, type=Path)
    parser.add_argument("--pdf-events", required=True, type=Path)
    parser.add_argument("--bundles", required=True, type=Path)
    parser.add_argument("--summary", required=True, type=Path)
    parser.add_argument(
        "--reject-unsafe-claims",
        action="store_true",
        help="fail if any unsafe claim counter is non-zero",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    diagnostics = verify_contract(
        args.selection,
        args.metadata_events,
        args.pdf_events,
        args.bundles,
        args.summary,
        reject_unsafe_claims=args.reject_unsafe_claims,
    )
    if diagnostics:
        sys.stderr.write("M028 universal loader evidence verification failed\n")
        for item in diagnostics:
            sys.stderr.write(item.render() + "\n")
        return 1
    sys.stdout.write(
        "M028 universal loader evidence verification passed: "
        f"refs={EXPECTED_REF_COUNT} identities={EXPECTED_IDENTITY_COUNT} "
        f"expanded_refs={','.join(EXPANDED_SCOPE_REF_IDS)} reject_unsafe_claims={args.reject_unsafe_claims}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
