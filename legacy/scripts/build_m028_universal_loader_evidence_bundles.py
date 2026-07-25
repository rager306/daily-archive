#!/usr/bin/env python3
"""Build metadata-only universal loader evidence bundles for M028 S04.

This builder fuses the accepted M028 source selection, S02 source metadata
adapter artifacts, and S03 PDF acquisition diagnostics into deterministic
per-ref evidence bundles.  It does not fetch URLs, parse source bodies, chunk
content, invoke models, generate Hermes digests, write graph/KG state, or claim
production import eligibility.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path, PurePosixPath
from typing import Any

EVENT_SCHEMA_VERSION = "m028.universal-loader-evidence-bundle.v1"
SUMMARY_SCHEMA_VERSION = "m028.universal-loader-evidence-summary.v1"
BUNDLES_FILENAME = "universal-loader-evidence-bundles.jsonl"
SUMMARY_FILENAME = "universal-loader-evidence-summary.json"
REPORT_FILENAME = "universal-loader-evidence-report.md"

EXPECTED_REF_COUNT = 21
EXPECTED_IDENTITY_COUNT = 20
EXPECTED_SOURCE_KIND_COUNTS = {
    "arxiv_abs_url": 15,
    "arxiv_pdf_url": 4,
    "company_blog_url": 1,
    "nature_article_url": 1,
}

SAFETY_FLAGS = {
    "raw_article_text_embedded": False,
    "raw_pdf_bytes_embedded": False,
    "html_source_embedded": False,
    "source_payload_embedded": False,
    "binary_payload_embedded": False,
    "chunk_content_embedded": False,
    "chunk_payload_embedded": False,
    "model_output_embedded": False,
    "parser_attempted": False,
    "chunker_attempted": False,
    "hermes_digest_generated": False,
    "graph_write_attempted": False,
    "production_persistence_attempted": False,
    "parser_readiness_claimed": False,
    "kg_readiness_claimed": False,
    "graph_ready_claimed": False,
    "dspy_attempted": False,
    "rlm_attempted": False,
    "minimax_attempted": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
}

UNSAFE_COUNTER_KEYS = tuple(SAFETY_FLAGS) + (
    "import_eligible_count",
    "promoted_to_fact_count",
    "hermes_digest_count",
)

FORBIDDEN_PAYLOAD_MARKERS = (
    "<html",
    "</html>",
    "<!doctype html",
    "%pdf-",
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
    "trusted_fact",
    "graph_ready",
    "kg_ready",
    "parser_ready",
    "promoted_to_fact",
    "ladybugdb_written=true",
)


class UniversalLoaderEvidenceInputError(ValueError):
    """Raised when S04 evidence-bundle inputs are malformed or unsafe."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise UniversalLoaderEvidenceInputError(f"input_missing:{path}") from exc
    except json.JSONDecodeError as exc:
        raise UniversalLoaderEvidenceInputError(
            f"json_malformed:{path}:{exc.lineno}:{exc.colno}"
        ) from exc
    if not isinstance(payload, dict):
        raise UniversalLoaderEvidenceInputError(f"json_object_required:{path}")
    return payload


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise UniversalLoaderEvidenceInputError(f"input_missing:{path}") from exc

    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as exc:
            raise UniversalLoaderEvidenceInputError(
                f"jsonl_malformed:{path}:{line_number}:{exc.colno}"
            ) from exc
        if not isinstance(row, dict):
            raise UniversalLoaderEvidenceInputError(f"jsonl_object_required:{path}:{line_number}")
        rows.append(row)
    return rows


def validate_selection(selection: dict[str, Any]) -> list[dict[str, Any]]:
    refs = selection.get("refs")
    if not isinstance(refs, list) or not refs:
        raise UniversalLoaderEvidenceInputError("selection_refs_required")
    seen: set[str] = set()
    validated: list[dict[str, Any]] = []
    for index, ref in enumerate(refs):
        if not isinstance(ref, dict):
            raise UniversalLoaderEvidenceInputError(f"selection_ref_object_required:{index}")
        required = (
            ref.get("ref_id"),
            ref.get("url"),
            ref.get("canonical_url"),
            ref.get("source_kind"),
            ref.get("normalized_identity"),
        )
        if not all(isinstance(value, str) and value for value in required):
            raise UniversalLoaderEvidenceInputError(f"selection_ref_required_fields:{index}")
        ref_id = str(ref["ref_id"])  # ty:ignore[invalid-argument-type]
        if ref_id in seen:
            raise UniversalLoaderEvidenceInputError(f"selection_ref_duplicate:{ref_id}")
        seen.add(ref_id)
        validated.append(ref)  # ty:ignore[invalid-argument-type]
    return validated


def rows_by_ref(rows: list[dict[str, Any]], *, label: str) -> dict[str, dict[str, Any]]:
    by_ref: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(rows):
        ref_id = row.get("ref_id")
        if not isinstance(ref_id, str) or not ref_id:
            raise UniversalLoaderEvidenceInputError(f"{label}_ref_id_required:{index}")
        if ref_id in by_ref:
            raise UniversalLoaderEvidenceInputError(f"{label}_ref_duplicate:{ref_id}")
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
            "url_variants": [
                classify_variant(str(ref["url"]), str(ref["source_kind"])) for ref in group_refs
            ],
        }
    return groups


def safe_repo_path(repo_root: Path, path_value: Any) -> tuple[str | None, str | None]:
    if path_value is None:
        return None, None
    if not isinstance(path_value, str) or not path_value.strip():
        return None, "artifact_path_missing"
    if "://" in path_value:
        return None, "artifact_path_is_url"
    normalized = PurePosixPath(path_value.replace("\\", "/"))
    if (
        normalized.is_absolute()
        or ".." in normalized.parts
        or any(part == "" for part in normalized.parts)
    ):
        return None, "artifact_path_unsafe"
    repo_root_resolved = repo_root.resolve()
    resolved = (repo_root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(repo_root_resolved):
        return None, "artifact_path_escapes_repo"
    return normalized.as_posix(), None


def diagnostic(
    code: str, ref_id: str, severity: str, json_path: str, details: dict[str, Any] | None = None
) -> dict[str, Any]:
    return {
        "code": code,
        "severity": severity,
        "ref_id": ref_id,
        "json_path": json_path,
        "message": "Universal loader evidence diagnostic; inspect code, severity, and JSON path.",
        "details": details or {},
    }


def ensure_scope(
    refs: list[dict[str, Any]],
    acquisition_by_ref: dict[str, dict[str, Any]],
    metadata_by_ref: dict[str, dict[str, Any]],
    pdf_by_ref: dict[str, dict[str, Any]],
    metadata_summary: dict[str, Any],
    pdf_summary: dict[str, Any],
    *,
    expected_ref_count: int,
    expected_identity_count: int,
    expected_source_kind_counts: dict[str, int],
) -> None:
    ref_ids = {str(ref["ref_id"]) for ref in refs}
    if len(refs) != expected_ref_count:
        raise UniversalLoaderEvidenceInputError(
            f"selection_ref_count_mismatch:expected={expected_ref_count}:actual={len(refs)}"
        )
    identity_count = len({str(ref["normalized_identity"]) for ref in refs})
    if identity_count != expected_identity_count:
        raise UniversalLoaderEvidenceInputError(
            f"selection_identity_count_mismatch:expected={expected_identity_count}:actual={identity_count}"
        )
    source_kind_counts = dict(sorted(Counter(str(ref["source_kind"]) for ref in refs).items()))
    if source_kind_counts != dict(sorted(expected_source_kind_counts.items())):
        raise UniversalLoaderEvidenceInputError("selection_source_kind_counts_mismatch")
    for label, by_ref in (
        ("source_acquisition", acquisition_by_ref),
        ("metadata", metadata_by_ref),
        ("pdf", pdf_by_ref),
    ):
        if set(by_ref) != ref_ids:
            missing = sorted(ref_ids - set(by_ref))
            extra = sorted(set(by_ref) - ref_ids)
            raise UniversalLoaderEvidenceInputError(
                f"{label}_ref_set_mismatch:missing={missing}:extra={extra}"
            )
    for label, summary in (("metadata_summary", metadata_summary), ("pdf_summary", pdf_summary)):
        if summary.get("url_ref_count", summary.get("ref_count")) != expected_ref_count:
            raise UniversalLoaderEvidenceInputError(f"{label}_ref_count_mismatch")
        if summary.get("normalized_identity_count") != expected_identity_count:
            raise UniversalLoaderEvidenceInputError(f"{label}_identity_count_mismatch")
        if dict(sorted((summary.get("source_kind_counts") or {}).items())) != dict(
            sorted(expected_source_kind_counts.items())
        ):
            raise UniversalLoaderEvidenceInputError(f"{label}_source_kind_counts_mismatch")

    for ref in refs:
        ref_id = str(ref["ref_id"])
        for label, row in (
            ("source_acquisition", acquisition_by_ref[ref_id]),
            ("metadata", metadata_by_ref[ref_id]),
            ("pdf", pdf_by_ref[ref_id]),
        ):
            if row.get("source_kind") != ref["source_kind"]:
                raise UniversalLoaderEvidenceInputError(f"{label}_source_kind_mismatch:{ref_id}")
            if row.get("normalized_identity") != ref["normalized_identity"]:
                raise UniversalLoaderEvidenceInputError(f"{label}_identity_mismatch:{ref_id}")


def assert_fail_closed_flags(row: dict[str, Any], *, label: str, ref_id: str) -> None:
    flags = row.get("safety_flags")
    if flags is None:
        return
    if not isinstance(flags, dict):
        raise UniversalLoaderEvidenceInputError(f"{label}_safety_flags_object_required:{ref_id}")
    unsafe = sorted(key for key, value in flags.items() if value is not False)
    if unsafe:
        raise UniversalLoaderEvidenceInputError(f"{label}_unsafe_claim:{ref_id}:{unsafe}")


def safe_artifact_ref(
    repo_root: Path,
    path_value: Any,
    sha256_value: Any,
    byte_count_value: Any,
    content_type_value: Any,
) -> dict[str, Any]:
    safe_path, path_error = safe_repo_path(repo_root, path_value)
    return {
        "path": safe_path,
        "path_error": path_error,
        "sha256": sha256_value if isinstance(sha256_value, str) else None,
        "byte_count": byte_count_value if isinstance(byte_count_value, int) else None,
        "content_type": content_type_value if isinstance(content_type_value, str) else None,
        "payload_embedded": False,
    }


def metadata_status(metadata_event: dict[str, Any]) -> str:
    value = metadata_event.get("metadata_status")
    if isinstance(value, str) and value:
        return value
    artifact = metadata_event.get("artifact")
    if isinstance(artifact, dict) and artifact.get("checksum_verified") is True:
        return "metadata_extracted"
    acquisition = metadata_event.get("acquisition")
    if isinstance(acquisition, dict) and acquisition.get("captured") is True:
        return "metadata_available"
    return "metadata_unverified"


def source_quality_status(metadata_event: dict[str, Any], pdf_event: dict[str, Any]) -> str:
    pdf_status = (
        (pdf_event.get("pdf_acquisition") or {}).get("status")
        if isinstance(pdf_event.get("pdf_acquisition"), dict)
        else None
    )
    if pdf_status == "acquired_existing_pdf":
        return "source_metadata_with_verified_pdf_artifact"
    if pdf_status == "not_acquired":
        return "source_metadata_only_pdf_not_acquired"
    if pdf_status == "not_applicable":
        return "source_metadata_only_non_pdf_source"
    if metadata_status(metadata_event) in {"metadata_extracted", "metadata_available"}:
        return "source_metadata_only"
    return "source_metadata_unverified"


def terminal_pdf_status(pdf_event: dict[str, Any]) -> tuple[str | None, str | None, bool | None]:
    pdf_acquisition = pdf_event.get("pdf_acquisition")
    if not isinstance(pdf_acquisition, dict):
        return None, None, None
    return (
        pdf_acquisition.get("status") if isinstance(pdf_acquisition.get("status"), str) else None,
        pdf_acquisition.get("reason") if isinstance(pdf_acquisition.get("reason"), str) else None,
        pdf_acquisition.get("terminal")
        if isinstance(pdf_acquisition.get("terminal"), bool)
        else None,
    )


def build_bundle(
    ref: dict[str, Any],
    source_acquisition: dict[str, Any],
    metadata_event: dict[str, Any],
    pdf_event: dict[str, Any],
    identity_groups: dict[str, dict[str, Any]],
    repo_root: Path,
) -> dict[str, Any]:
    ref_id = str(ref["ref_id"])
    assert_fail_closed_flags(metadata_event, label="metadata", ref_id=ref_id)
    assert_fail_closed_flags(pdf_event, label="pdf", ref_id=ref_id)

    source_artifact = safe_artifact_ref(
        repo_root,
        source_acquisition.get("artifact_path"),
        source_acquisition.get("sha256"),
        source_acquisition.get("byte_count"),
        source_acquisition.get("content_type"),
    )
    metadata_artifact_payload = (
        metadata_event.get("artifact") if isinstance(metadata_event.get("artifact"), dict) else {}
    )
    metadata_artifact = safe_artifact_ref(
        repo_root,
        metadata_artifact_payload.get("path"),  # ty:ignore[unresolved-attribute]
        metadata_artifact_payload.get("sha256"),  # ty:ignore[unresolved-attribute]
        metadata_artifact_payload.get("byte_count"),  # ty:ignore[unresolved-attribute]
        metadata_artifact_payload.get("content_type"),  # ty:ignore[unresolved-attribute]
    )
    pdf_artifact_payload = (
        pdf_event.get("pdf_artifact") if isinstance(pdf_event.get("pdf_artifact"), dict) else {}
    )
    pdf_artifact = safe_artifact_ref(
        repo_root,
        pdf_artifact_payload.get("path"),  # ty:ignore[unresolved-attribute]
        pdf_artifact_payload.get("sha256"),  # ty:ignore[unresolved-attribute]
        pdf_artifact_payload.get("byte_count"),  # ty:ignore[unresolved-attribute]
        pdf_artifact_payload.get("content_type"),  # ty:ignore[unresolved-attribute]
    )

    diagnostics: list[dict[str, Any]] = []
    for artifact_name, artifact in (
        ("source_artifact", source_artifact),
        ("metadata_artifact", metadata_artifact),
        ("pdf_artifact", pdf_artifact),
    ):
        if artifact["path_error"] is not None:
            diagnostics.append(
                diagnostic(
                    str(artifact["path_error"]),
                    ref_id,
                    "error",
                    f"artifact_refs.{artifact_name}.path",
                )
            )
    for upstream_label, upstream_event in (("metadata", metadata_event), ("pdf", pdf_event)):
        upstream_diagnostics = upstream_event.get("diagnostics")
        if isinstance(upstream_diagnostics, list):
            for index, item in enumerate(upstream_diagnostics):
                if isinstance(item, dict):
                    diagnostics.append(
                        diagnostic(
                            f"upstream_{upstream_label}_{item.get('code', 'diagnostic')}",
                            ref_id,
                            str(item.get("severity") or "info"),
                            f"upstream.{upstream_label}.diagnostics[{index}]",
                            {
                                "upstream_json_path": item.get("json_path"),
                                "upstream_details": item.get("details") or {},
                            },
                        )
                    )

    pdf_status, pdf_reason, pdf_terminal = terminal_pdf_status(pdf_event)
    source_kind = str(ref["source_kind"])
    family = source_family(source_kind)
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
        "selection": {
            "loader_owns_selection": bool(ref.get("loader_owns_selection"))
            if isinstance(ref.get("loader_owns_selection"), bool)
            else False,
            "selection_policy": ref.get("selection_policy")
            if isinstance(ref.get("selection_policy"), str)
            else None,
        },
        "source_metadata": {
            "metadata_status": metadata_status(metadata_event),
            "optional_metadata_gaps": metadata_event.get("optional_metadata_gaps")
            if isinstance(metadata_event.get("optional_metadata_gaps"), list)
            else [],
            "diagnostic_count": len(metadata_event.get("diagnostics") or [])
            if isinstance(metadata_event.get("diagnostics"), list)
            else 0,
        },
        "pdf_diagnostic": {
            "status": pdf_status,
            "reason": pdf_reason,
            "terminal": pdf_terminal,
            "candidate_kind": (pdf_event.get("candidate_pdf") or {}).get("candidate_kind")
            if isinstance(pdf_event.get("candidate_pdf"), dict)
            else None,
            "diagnostic_count": len(pdf_event.get("diagnostics") or [])
            if isinstance(pdf_event.get("diagnostics"), list)
            else 0,
        },
        "artifact_refs": {
            "source_artifact": source_artifact,
            "metadata_artifact": metadata_artifact,
            "pdf_artifact": pdf_artifact,
        },
        "loader_evidence": {
            "bundle_status": "metadata_only_bundle_ready",
            "evidence_level": "source_metadata_and_pdf_diagnostics",
            "source_quality_status": source_quality_status(metadata_event, pdf_event),
            "outcome": "safe_for_downstream_metadata_projection_only",
            "hermes_digest_ready": False,
            "parser_output_available": False,
            "kg_import_eligible": False,
            "production_import_eligible": False,
        },
        "safety_flags": dict(SAFETY_FLAGS),
        "diagnostics": diagnostics,
    }


def count_unsafe_claims(bundles: list[dict[str, Any]]) -> dict[str, int]:
    counts = dict.fromkeys(UNSAFE_COUNTER_KEYS, 0)
    for bundle in bundles:
        flags = bundle.get("safety_flags") if isinstance(bundle.get("safety_flags"), dict) else {}
        for key in SAFETY_FLAGS:
            if flags.get(key) is not False:  # ty:ignore[unresolved-attribute]
                counts[key] += 1
        evidence = (
            bundle.get("loader_evidence") if isinstance(bundle.get("loader_evidence"), dict) else {}
        )
        if (
            evidence.get("kg_import_eligible") is True  # ty:ignore[unresolved-attribute]
            or evidence.get("production_import_eligible") is True  # ty:ignore[unresolved-attribute]
        ):
            counts["import_eligible_count"] += 1
        if evidence.get("outcome") == "promoted_to_fact":  # ty:ignore[unresolved-attribute]
            counts["promoted_to_fact_count"] += 1
        if evidence.get("hermes_digest_ready") is True:  # ty:ignore[unresolved-attribute]
            counts["hermes_digest_count"] += 1
    return counts


def string_values(payload: Any) -> list[str]:
    if isinstance(payload, str):
        return [payload]
    if isinstance(payload, dict):
        values: list[str] = []
        for value in payload.values():
            values.extend(string_values(value))
        return values
    if isinstance(payload, list):
        values = []
        for value in payload:
            values.extend(string_values(value))
        return values
    return []


def assert_no_payload_markers(
    bundles: list[dict[str, Any]], summary_without_report: dict[str, Any]
) -> None:
    serialized_values = "\n".join(
        string_values(bundles) + string_values(summary_without_report)
    ).lower()
    for marker in FORBIDDEN_PAYLOAD_MARKERS:
        if marker in serialized_values:
            raise UniversalLoaderEvidenceInputError(f"raw_payload_leakage:{marker}")


def summarize(
    bundles: list[dict[str, Any]],
    identity_groups: dict[str, dict[str, Any]],
    input_paths: dict[str, Path],
) -> dict[str, Any]:
    source_kind_counts = Counter(str(bundle["source_kind"]) for bundle in bundles)
    source_family_counts = Counter(str(bundle["source_family"]) for bundle in bundles)
    quality_counts = Counter(
        str(bundle["loader_evidence"]["source_quality_status"]) for bundle in bundles
    )
    pdf_status_counts = Counter(str(bundle["pdf_diagnostic"]["status"]) for bundle in bundles)
    diagnostic_counts: Counter[str] = Counter()
    for bundle in bundles:
        for item in bundle["diagnostics"]:
            diagnostic_counts[str(item["code"])] += 1
    duplicate_groups = [group for group in identity_groups.values() if group["url_ref_count"] > 1]
    input_fingerprints = {
        name: {"path": path.as_posix(), "sha256": sha256_file(path)}
        for name, path in sorted(input_paths.items())
    }
    return {
        "schema_version": SUMMARY_SCHEMA_VERSION,
        "generated_at": "deterministic_from_input_sha256",
        "url_ref_count": len(bundles),
        "ref_count": len(bundles),
        "normalized_identity_count": len(identity_groups),
        "duplicate_identity_group_count": len(duplicate_groups),
        "duplicate_identity_groups": duplicate_groups,
        "ref_ids": [str(bundle["ref_id"]) for bundle in bundles],
        "identity_groups": list(identity_groups.values()),
        "source_kind_counts": dict(sorted(source_kind_counts.items())),
        "source_family_counts": dict(sorted(source_family_counts.items())),
        "source_quality_status_counts": dict(sorted(quality_counts.items())),
        "pdf_status_counts": dict(sorted(pdf_status_counts.items())),
        "diagnostic_counts": dict(sorted(diagnostic_counts.items())),
        "bundle_status_counts": dict(
            sorted(
                Counter(
                    str(bundle["loader_evidence"]["bundle_status"]) for bundle in bundles
                ).items()
            )
        ),
        "safety_flags": dict(SAFETY_FLAGS),
        "unsafe_claim_counts": count_unsafe_claims(bundles),
        "input_fingerprints": input_fingerprints,
        "failure_modes": [
            {
                "dependency": "selection JSON",
                "failure_path": "missing, malformed, stale-count, duplicate, or required-field errors raise UniversalLoaderEvidenceInputError before output write",
            },
            {
                "dependency": "source acquisition JSONL",
                "failure_path": "missing, malformed, duplicate, missing-linkage, source-kind drift, identity drift, or unsafe artifact paths raise stable input errors or typed per-ref diagnostics",
            },
            {
                "dependency": "source metadata events/summary",
                "failure_path": "missing, malformed, stale count, missing linkage, source-kind drift, identity drift, or unsafe safety flags raise stable input errors before output write",
            },
            {
                "dependency": "PDF acquisition events/summary",
                "failure_path": "missing, malformed, stale count, missing linkage, source-kind drift, identity drift, or unsafe safety flags raise stable input errors before output write",
            },
            {
                "dependency": "filesystem outputs",
                "failure_path": "output directory creation or write failures bubble as filesystem exceptions; no network or subprocess dependency exists",
            },
        ],
        "load_profile": {
            "expected_url_refs": EXPECTED_REF_COUNT,
            "ten_x_url_refs": EXPECTED_REF_COUNT * 10,
            "first_saturating_resource": "JSON/JSONL input size and deterministic serialization of per-ref metadata-only bundles",
            "protection": "single-pass in-memory processing for small smoke corpus, no live network/parser/model/graph calls, chunked SHA-256 only for six input artifact fingerprints, and no source/PDF payload reads",
        },
        "negative_tests": [
            "tests/test_m028_universal_loader_evidence_bundles.py::test_fixture_build_preserves_duplicate_identity_and_fail_closed_flags",
            "tests/test_m028_universal_loader_evidence_bundles.py::test_missing_pdf_event_is_stable_input_error",
            "tests/test_m028_universal_loader_evidence_bundles.py::test_upstream_unsafe_flag_is_stable_input_error",
            "tests/test_m028_universal_loader_evidence_bundles.py::test_real_corpus_build_contract",
        ],
    }


def render_report(summary: dict[str, Any]) -> str:
    lines = [
        "# M028 S04 Universal Loader Evidence Bundles",
        "",
        "Metadata-only evidence bundles for the accepted mixed-source smoke corpus. This report fuses S02 source metadata and S03 PDF diagnostics without live acquisition, parser/chunker work, graph writes, Hermes digest generation, model calls, or raw payload serialization.",
        "",
        "## Scope",
        f"- URL refs: {summary['url_ref_count']}",
        f"- Normalized identities: {summary['normalized_identity_count']}",
        f"- Duplicate identity groups: {summary['duplicate_identity_group_count']}",
        f"- Source kind counts: `{json.dumps(summary['source_kind_counts'], sort_keys=True)}`",
        f"- Source quality counts: `{json.dumps(summary['source_quality_status_counts'], sort_keys=True)}`",
        "",
        "## Loader Evidence Outcomes",
        f"- Bundle status counts: `{json.dumps(summary['bundle_status_counts'], sort_keys=True)}`",
        f"- PDF status counts: `{json.dumps(summary['pdf_status_counts'], sort_keys=True)}`",
        f"- Diagnostics: `{json.dumps(summary['diagnostic_counts'], sort_keys=True)}`",
        "",
        "## Safety Flags",
        f"- All fail-closed flags false: `{json.dumps(summary['safety_flags'], sort_keys=True)}`",
        f"- Unsafe claim counts: `{json.dumps(summary['unsafe_claim_counts'], sort_keys=True)}`",
        "",
        "## Failure Modes",
    ]
    for item in summary["failure_modes"]:
        lines.append(f"- {item['dependency']}: {item['failure_path']}")
    lines.extend(
        [
            "",
            "## Load Profile",
            f"- Expected refs: {summary['load_profile']['expected_url_refs']}; 10x refs: {summary['load_profile']['ten_x_url_refs']}",
            f"- First saturating resource: {summary['load_profile']['first_saturating_resource']}",
            f"- Protection: {summary['load_profile']['protection']}",
            "",
            "## Negative Tests",
        ]
    )
    for item in summary["negative_tests"]:
        lines.append(f"- `{item}`")
    lines.extend(
        [
            "",
            "## Observability Impact",
            "- Emits per-ref metadata-only bundle records, source/PDF diagnostic rollups, duplicate identity membership, input fingerprints, stable diagnostic codes/JSON paths, and fail-closed aggregate counters for downstream S05 inspection.",
            "",
        ]
    )
    return "\n".join(lines)


def build_universal_loader_evidence_outputs(
    selection_path: Path,
    source_acquisition_events_path: Path,
    metadata_events_path: Path,
    metadata_summary_path: Path,
    pdf_events_path: Path,
    pdf_summary_path: Path,
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
    source_acquisition_by_ref = rows_by_ref(
        read_jsonl(source_acquisition_events_path), label="source_acquisition"
    )
    metadata_by_ref = rows_by_ref(read_jsonl(metadata_events_path), label="metadata")
    pdf_by_ref = rows_by_ref(read_jsonl(pdf_events_path), label="pdf")
    metadata_summary = read_json(metadata_summary_path)
    pdf_summary = read_json(pdf_summary_path)
    ensure_scope(
        refs,
        source_acquisition_by_ref,
        metadata_by_ref,
        pdf_by_ref,
        metadata_summary,
        pdf_summary,
        expected_ref_count=expected_ref_count,
        expected_identity_count=expected_identity_count,
        expected_source_kind_counts=expected_source_kind_counts,
    )
    identity_groups = build_identity_groups(refs)
    bundles = [
        build_bundle(
            ref,
            source_acquisition_by_ref[str(ref["ref_id"])],
            metadata_by_ref[str(ref["ref_id"])],
            pdf_by_ref[str(ref["ref_id"])],
            identity_groups,
            repo_root,
        )
        for ref in refs
    ]
    input_paths = {
        "selection": selection_path,
        "source_acquisition_events": source_acquisition_events_path,
        "metadata_events": metadata_events_path,
        "metadata_summary": metadata_summary_path,
        "pdf_events": pdf_events_path,
        "pdf_summary": pdf_summary_path,
    }
    summary = summarize(bundles, identity_groups, input_paths)
    assert_no_payload_markers(bundles, summary)

    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / BUNDLES_FILENAME).write_text(
        "\n".join(json.dumps(bundle, sort_keys=True) for bundle in bundles) + "\n", encoding="utf-8"
    )
    (out_dir / SUMMARY_FILENAME).write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (out_dir / REPORT_FILENAME).write_text(render_report(summary), encoding="utf-8")
    return bundles, summary


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--source-acquisition-events", required=True, type=Path)
    parser.add_argument("--metadata-events", required=True, type=Path)
    parser.add_argument("--metadata-summary", required=True, type=Path)
    parser.add_argument("--pdf-events", required=True, type=Path)
    parser.add_argument("--pdf-summary", required=True, type=Path)
    parser.add_argument("--out-dir", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        _bundles, summary = build_universal_loader_evidence_outputs(
            args.selection,
            args.source_acquisition_events,
            args.metadata_events,
            args.metadata_summary,
            args.pdf_events,
            args.pdf_summary,
            args.out_dir,
        )
    except UniversalLoaderEvidenceInputError as exc:
        raise SystemExit(str(exc)) from exc
    sys.stdout.write(
        "wrote universal loader evidence bundles: "
        f"refs={summary['url_ref_count']} identities={summary['normalized_identity_count']} "
        f"bundles={args.out_dir / BUNDLES_FILENAME} summary={args.out_dir / SUMMARY_FILENAME} report={args.out_dir / REPORT_FILENAME}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
