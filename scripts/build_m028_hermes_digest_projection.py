#!/usr/bin/env python3
"""Build an evidence-only Hermes digest projection for M028 S05.

This projection consumes only the S04 universal-loader evidence bundle JSONL and
summary JSON.  It intentionally does not fetch URLs, read source/PDF payloads,
parse, chunk, call models, import graph state, or make production-write claims.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

SCHEMA_NAME = "m028.hermes-digest-projection"
SCHEMA_VERSION = "m028.hermes-digest-projection.v1"
DIGEST_FILENAME = "hermes-digest-projection.json"
REPORT_FILENAME = "hermes-digest-projection-report.md"
EXPECTED_REF_COUNT = 21
EXPECTED_IDENTITY_COUNT = 20
EXPECTED_REF_IDS = [f"R{index:02d}" for index in range(1, EXPECTED_REF_COUNT + 1)]
EXPANDED_SCOPE_REF_IDS = [f"R{index:02d}" for index in range(15, EXPECTED_REF_COUNT + 1)]
EXPECTED_SOURCE_KIND_COUNTS = {
    "arxiv_abs_url": 15,
    "arxiv_pdf_url": 4,
    "company_blog_url": 1,
    "nature_article_url": 1,
}
EXPECTED_DUPLICATE_GROUP = ["R01", "R10"]

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
UNSAFE_SAFETY_FLAG_KEYS = (
    "raw_article_text_embedded",
    "raw_pdf_bytes_embedded",
    "html_source_embedded",
    "source_payload_embedded",
    "binary_payload_embedded",
    "chunk_content_embedded",
    "chunk_payload_embedded",
    "model_output_embedded",
    "parser_attempted",
    "chunker_attempted",
    "hermes_digest_generated",
    "graph_write_attempted",
    "production_persistence_attempted",
    "parser_readiness_claimed",
    "kg_readiness_claimed",
    "graph_ready_claimed",
    "dspy_attempted",
    "rlm_attempted",
    "minimax_attempted",
    "production_import_attempted",
    "ladybugdb_written",
)
UNSAFE_COUNTER_KEYS = tuple(UNSAFE_SAFETY_FLAG_KEYS) + (
    "import_eligible_count",
    "promoted_to_fact_count",
    "hermes_digest_count",
)
OPTIONAL_BIBLIOGRAPHIC_FIELDS = ("title", "authors", "published_date", "updated_date", "doi", "pdf_url")


@dataclass(frozen=True)
class Diagnostic:
    """Stable validation diagnostic with agent-inspectable code and JSON path."""

    code: str
    path: str
    message: str

    def render(self) -> str:
        return f"{self.code}:{self.path}:{self.message}"

    def as_dict(self) -> dict[str, str]:
        return {"code": self.code, "json_path": self.path, "message": self.message}


class HermesDigestProjectionInputError(ValueError):
    """Raised when S04 evidence cannot safely produce a digest projection."""

    def __init__(self, diagnostics: list[Diagnostic] | str):
        if isinstance(diagnostics, str):
            self.diagnostics = [Diagnostic("INPUT_ERROR", "$.inputs", diagnostics)]
        else:
            self.diagnostics = diagnostics
        super().__init__("; ".join(diagnostic.render() for diagnostic in self.diagnostics))


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_input_ref(path: Path) -> dict[str, str]:
    return {"path": path.as_posix(), "sha256": sha256_file(path)}


def read_json(path: Path, diagnostics: list[Diagnostic], label: str) -> dict[str, Any] | None:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        diagnostics.append(Diagnostic("INPUT_MISSING", f"$.source_refs.{label}", f"missing input: {path.as_posix()}"))
        return None
    except json.JSONDecodeError as exc:
        diagnostics.append(Diagnostic("JSON_MALFORMED", f"$.source_refs.{label}", f"malformed JSON at {exc.lineno}:{exc.colno}"))
        return None
    if not isinstance(payload, dict):
        diagnostics.append(Diagnostic("JSON_OBJECT_REQUIRED", f"$.source_refs.{label}", "top-level JSON must be an object"))
        return None
    return payload


def read_jsonl(path: Path, diagnostics: list[Diagnostic], label: str) -> list[dict[str, Any]] | None:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        diagnostics.append(Diagnostic("INPUT_MISSING", f"$.source_refs.{label}", f"missing input: {path.as_posix()}"))
        return None
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as exc:
            diagnostics.append(Diagnostic("JSONL_MALFORMED", f"$.source_refs.{label}[{line_number}]", f"malformed JSONL at column {exc.colno}"))
            continue
        if not isinstance(row, dict):
            diagnostics.append(Diagnostic("JSONL_OBJECT_REQUIRED", f"$.source_refs.{label}[{line_number}]", "JSONL row must be an object"))
            continue
        rows.append(row)
    return rows


def json_path_join(prefix: str, part: str | int) -> str:
    if isinstance(part, int):
        return f"{prefix}[{part}]"
    return f"{prefix}.{part}" if prefix != "$" else f"$.{part}"


def safe_relative_path(path_value: Any) -> bool:
    if path_value is None:
        return True
    if not isinstance(path_value, str) or not path_value.strip() or "://" in path_value:
        return False
    normalized = PurePosixPath(path_value.replace("\\", "/"))
    return not normalized.is_absolute() and ".." not in normalized.parts and all(part for part in normalized.parts)


def walk_forbidden(payload: Any, diagnostics: list[Diagnostic], path: str = "$") -> None:
    if isinstance(payload, dict):
        for key, value in payload.items():
            child_path = json_path_join(path, str(key))
            if str(key) in FORBIDDEN_KEYS:
                diagnostics.append(Diagnostic("FORBIDDEN_KEY_PRESENT", child_path, f"forbidden payload-bearing key {key}"))
            walk_forbidden(value, diagnostics, child_path)
    elif isinstance(payload, list):
        for index, value in enumerate(payload):
            walk_forbidden(value, diagnostics, json_path_join(path, index))
    elif isinstance(payload, str):
        lower = payload.lower()
        for marker in FORBIDDEN_PAYLOAD_MARKERS:
            if marker in lower:
                diagnostics.append(Diagnostic("FORBIDDEN_MARKER_PRESENT", path, f"forbidden marker {marker}"))


def validate_artifact_paths(bundles: list[dict[str, Any]], diagnostics: list[Diagnostic]) -> None:
    for row_index, bundle in enumerate(bundles):
        artifact_refs = bundle.get("artifact_refs")
        if not isinstance(artifact_refs, dict):
            diagnostics.append(Diagnostic("ARTIFACT_REFS_OBJECT_REQUIRED", f"$.bundles[{row_index}].artifact_refs", "artifact_refs must be an object"))
            continue
        for artifact_name, artifact in artifact_refs.items():
            artifact_path = f"$.bundles[{row_index}].artifact_refs.{artifact_name}"
            if not isinstance(artifact, dict):
                diagnostics.append(Diagnostic("ARTIFACT_REF_OBJECT_REQUIRED", artifact_path, "artifact ref must be an object"))
                continue
            if artifact.get("payload_embedded") is not False:
                diagnostics.append(Diagnostic("ARTIFACT_PAYLOAD_FLAG_UNSAFE", f"{artifact_path}.payload_embedded", "artifact payload flag must be false"))
            if not safe_relative_path(artifact.get("path")):
                diagnostics.append(Diagnostic("ARTIFACT_PATH_UNSAFE", f"{artifact_path}.path", "artifact path must be relative and repository-contained"))


def validate_scope(bundles: list[dict[str, Any]], summary: dict[str, Any], diagnostics: list[Diagnostic]) -> None:
    if len(bundles) != EXPECTED_REF_COUNT:
        diagnostics.append(Diagnostic("SCOPE_REF_COUNT_MISMATCH", "$.bundles", f"expected {EXPECTED_REF_COUNT} bundles, found {len(bundles)}"))
    ref_ids = [str(bundle.get("ref_id")) for bundle in bundles if isinstance(bundle.get("ref_id"), str)]
    if ref_ids != EXPECTED_REF_IDS:
        diagnostics.append(Diagnostic("SCOPE_REF_IDS_MISMATCH", "$.bundles[*].ref_id", f"expected ordered refs {EXPECTED_REF_IDS}, found {ref_ids}"))
    missing_expanded = sorted(set(EXPANDED_SCOPE_REF_IDS) - set(ref_ids))
    if missing_expanded:
        diagnostics.append(Diagnostic("EXPANDED_SCOPE_REFS_MISSING", "$.bundles[*].ref_id", f"missing expanded refs {missing_expanded}"))
    identities = [str(bundle.get("normalized_identity")) for bundle in bundles if isinstance(bundle.get("normalized_identity"), str)]
    if len(set(identities)) != EXPECTED_IDENTITY_COUNT:
        diagnostics.append(Diagnostic("SCOPE_IDENTITY_COUNT_MISMATCH", "$.bundles[*].normalized_identity", f"expected {EXPECTED_IDENTITY_COUNT} identities, found {len(set(identities))}"))
    source_kind_counts = dict(sorted(Counter(str(bundle.get("source_kind")) for bundle in bundles if isinstance(bundle.get("source_kind"), str)).items()))
    if source_kind_counts != dict(sorted(EXPECTED_SOURCE_KIND_COUNTS.items())):
        diagnostics.append(Diagnostic("SCOPE_SOURCE_KIND_COUNTS_MISMATCH", "$.bundles[*].source_kind", f"expected {EXPECTED_SOURCE_KIND_COUNTS}, found {source_kind_counts}"))
    duplicate_groups = sorted(
        sorted(str(bundle.get("ref_id")) for bundle in bundles if bundle.get("normalized_identity") == identity)
        for identity in sorted(set(identities))
        if sum(1 for bundle in bundles if bundle.get("normalized_identity") == identity) > 1
    )
    if duplicate_groups != [EXPECTED_DUPLICATE_GROUP]:
        diagnostics.append(Diagnostic("DUPLICATE_IDENTITY_DRIFT", "$.bundles[*].normalized_identity", f"expected duplicate group {EXPECTED_DUPLICATE_GROUP}, found {duplicate_groups}"))
    if summary.get("url_ref_count", summary.get("ref_count")) != EXPECTED_REF_COUNT:
        diagnostics.append(Diagnostic("SUMMARY_REF_COUNT_MISMATCH", "$.summary.url_ref_count", "summary URL ref count drifted"))
    if summary.get("normalized_identity_count") != EXPECTED_IDENTITY_COUNT:
        diagnostics.append(Diagnostic("SUMMARY_IDENTITY_COUNT_MISMATCH", "$.summary.normalized_identity_count", "summary identity count drifted"))
    if dict(sorted((summary.get("source_kind_counts") or {}).items())) != dict(sorted(EXPECTED_SOURCE_KIND_COUNTS.items())):
        diagnostics.append(Diagnostic("SUMMARY_SOURCE_KIND_COUNTS_MISMATCH", "$.summary.source_kind_counts", "summary source-kind counts drifted"))


def count_unsafe_claims(bundles: list[dict[str, Any]], summary: dict[str, Any]) -> dict[str, int]:
    counts = dict.fromkeys(UNSAFE_COUNTER_KEYS, 0)
    for bundle in bundles:
        flags = bundle.get("safety_flags") if isinstance(bundle.get("safety_flags"), dict) else {}
        for key in UNSAFE_SAFETY_FLAG_KEYS:
            if flags.get(key) is not False:
                counts[key] += 1
        evidence = bundle.get("loader_evidence") if isinstance(bundle.get("loader_evidence"), dict) else {}
        if evidence.get("kg_import_eligible") is True or evidence.get("production_import_eligible") is True:
            counts["import_eligible_count"] += 1
        if evidence.get("outcome") == "promoted_to_fact":
            counts["promoted_to_fact_count"] += 1
        if evidence.get("hermes_digest_ready") is True:
            counts["hermes_digest_count"] += 1
    summary_counts = summary.get("unsafe_claim_counts")
    if isinstance(summary_counts, dict):
        for key in UNSAFE_COUNTER_KEYS:
            value = summary_counts.get(key, 0)
            if isinstance(value, int) and value > 0:
                counts[key] += value
    return counts


def validate_unsafe_counters(unsafe_counts: dict[str, int], diagnostics: list[Diagnostic]) -> None:
    for key, value in unsafe_counts.items():
        if value != 0:
            diagnostics.append(Diagnostic("UNSAFE_COUNTER_NONZERO", f"$.unsafe_counters.{key}", f"unsafe counter {key}={value}"))


def compact_artifact_refs(bundle: dict[str, Any]) -> dict[str, dict[str, Any]]:
    compact: dict[str, dict[str, Any]] = {}
    artifact_refs = bundle.get("artifact_refs") if isinstance(bundle.get("artifact_refs"), dict) else {}
    for name in ("source_artifact", "metadata_artifact", "pdf_artifact"):
        artifact = artifact_refs.get(name) if isinstance(artifact_refs.get(name), dict) else {}
        compact[name] = {
            "path": artifact.get("path") if isinstance(artifact.get("path"), str) else None,
            "sha256": artifact.get("sha256") if isinstance(artifact.get("sha256"), str) else None,
            "byte_count": artifact.get("byte_count") if isinstance(artifact.get("byte_count"), int) else None,
            "content_type": artifact.get("content_type") if isinstance(artifact.get("content_type"), str) else None,
            "payload_embedded": False,
        }
    return compact


def bibliographic_fields(bundle: dict[str, Any]) -> dict[str, dict[str, Any]]:
    gaps: dict[str, str] = {}
    source_metadata = bundle.get("source_metadata") if isinstance(bundle.get("source_metadata"), dict) else {}
    optional_gaps = source_metadata.get("optional_metadata_gaps") if isinstance(source_metadata.get("optional_metadata_gaps"), list) else []
    for gap in optional_gaps:
        if isinstance(gap, dict) and isinstance(gap.get("field"), str):
            gaps[str(gap["field"])] = str(gap.get("reason") or "not_in_loader_evidence_bundle")
    fields: dict[str, dict[str, Any]] = {}
    for field in OPTIONAL_BIBLIOGRAPHIC_FIELDS:
        fields[field] = {
            "value": None,
            "diagnostic": "metadata_value_not_in_loader_evidence_bundle",
            "reason": gaps.get(field, "not_in_loader_evidence_bundle"),
        }
    return fields


def item_digest_note(bundle: dict[str, Any]) -> str:
    evidence = bundle.get("loader_evidence") if isinstance(bundle.get("loader_evidence"), dict) else {}
    pdf = bundle.get("pdf_diagnostic") if isinstance(bundle.get("pdf_diagnostic"), dict) else {}
    quality = evidence.get("source_quality_status") if isinstance(evidence.get("source_quality_status"), str) else "source_quality_unknown"
    pdf_status = pdf.get("status") if isinstance(pdf.get("status"), str) else "pdf_status_unknown"
    return f"Evidence-only Hermes digest projection: {quality}; pdf_status={pdf_status}; no parser, chunker, model, graph, or production write attempted."


def build_item(bundle: dict[str, Any]) -> dict[str, Any]:
    diagnostics = bundle.get("diagnostics") if isinstance(bundle.get("diagnostics"), list) else []
    warning_diagnostics = [item for item in diagnostics if isinstance(item, dict) and str(item.get("severity", "info")) in {"warning", "error"}]
    source_metadata = bundle.get("source_metadata") if isinstance(bundle.get("source_metadata"), dict) else {}
    return {
        "ref_id": bundle.get("ref_id"),
        "canonical_url": bundle.get("canonical_url"),
        "normalized_identity": bundle.get("normalized_identity"),
        "source_kind": bundle.get("source_kind"),
        "source_family": bundle.get("source_family"),
        "url_variant": bundle.get("url_variant"),
        "identity_group": bundle.get("identity_group") if isinstance(bundle.get("identity_group"), dict) else None,
        "artifact_refs": compact_artifact_refs(bundle),
        "loader_evidence": bundle.get("loader_evidence") if isinstance(bundle.get("loader_evidence"), dict) else {},
        "pdf_diagnostic": bundle.get("pdf_diagnostic") if isinstance(bundle.get("pdf_diagnostic"), dict) else {},
        "quality": {
            "source_metadata_status": source_metadata.get("metadata_status"),
            "source_quality_status": (bundle.get("loader_evidence") or {}).get("source_quality_status") if isinstance(bundle.get("loader_evidence"), dict) else None,
            "diagnostic_count": len(diagnostics),
            "warning_count": len(warning_diagnostics),
        },
        "warnings": warning_diagnostics,
        "skipped_diagnostics": [
            {
                "code": "metadata_value_not_in_loader_evidence_bundle",
                "json_path": f"$.items[ref_id={bundle.get('ref_id')}].bibliographic_fields",
                "message": "Optional bibliographic metadata not present in S04 loader evidence remains null.",
            }
        ],
        "bibliographic_fields": bibliographic_fields(bundle),
        "digest_note": item_digest_note(bundle),
    }


def summarize_projection(bundles: list[dict[str, Any]], unsafe_counts: dict[str, int]) -> dict[str, Any]:
    source_kind_counts = Counter(str(bundle["source_kind"]) for bundle in bundles)
    source_family_counts = Counter(str(bundle["source_family"]) for bundle in bundles)
    pdf_status_counts = Counter(str((bundle.get("pdf_diagnostic") or {}).get("status")) for bundle in bundles if isinstance(bundle.get("pdf_diagnostic"), dict))
    quality_counts = Counter(str((bundle.get("loader_evidence") or {}).get("source_quality_status")) for bundle in bundles if isinstance(bundle.get("loader_evidence"), dict))
    diagnostic_counts: Counter[str] = Counter()
    for bundle in bundles:
        diagnostics = bundle.get("diagnostics") if isinstance(bundle.get("diagnostics"), list) else []
        for diagnostic in diagnostics:
            if isinstance(diagnostic, dict):
                diagnostic_counts[str(diagnostic.get("code", "diagnostic"))] += 1
    duplicate_identity_groups = []
    seen_groups: set[str] = set()
    for bundle in bundles:
        group = bundle.get("identity_group") if isinstance(bundle.get("identity_group"), dict) else None
        if group and int(group.get("url_ref_count", 0)) > 1 and str(group.get("group_id")) not in seen_groups:
            duplicate_identity_groups.append(group)
            seen_groups.add(str(group.get("group_id")))
    return {
        "url_ref_count": len(bundles),
        "normalized_identity_count": len({str(bundle["normalized_identity"]) for bundle in bundles}),
        "ref_ids": [str(bundle["ref_id"]) for bundle in bundles],
        "expanded_scope_ref_ids": EXPANDED_SCOPE_REF_IDS,
        "source_kind_counts": dict(sorted(source_kind_counts.items())),
        "source_family_counts": dict(sorted(source_family_counts.items())),
        "pdf_status_counts": dict(sorted(pdf_status_counts.items())),
        "source_quality_status_counts": dict(sorted(quality_counts.items())),
        "diagnostic_counts": dict(sorted(diagnostic_counts.items())),
        "duplicate_identity_group_count": len(duplicate_identity_groups),
        "duplicate_identity_groups": duplicate_identity_groups,
        "unsafe_counter_total": sum(unsafe_counts.values()),
    }


def render_report(projection: dict[str, Any]) -> str:
    summary = projection["summary"]
    lines = [
        "# M028 S05 Hermes Digest Projection Smoke",
        "",
        "Evidence-only consumer digest projection generated from S04 universal-loader evidence bundles. It keeps source selection, acquisition, parser/chunker semantics, graph import, model behavior, raw payloads, and production writes out of scope.",
        "",
        "## Scope",
        f"- URL refs: {summary['url_ref_count']}",
        f"- Normalized identities: {summary['normalized_identity_count']}",
        f"- Expanded refs R15-R21 present: `{json.dumps(summary['expanded_scope_ref_ids'])}`",
        f"- Duplicate identity groups: `{json.dumps(summary['duplicate_identity_groups'], sort_keys=True)}`",
        f"- Source kind counts: `{json.dumps(summary['source_kind_counts'], sort_keys=True)}`",
        "",
        "## Source References",
        f"- Loader bundles: `{projection['source_refs']['loader_bundle']['path']}` sha256={projection['source_refs']['loader_bundle']['sha256']}",
        f"- Loader summary: `{projection['source_refs']['loader_summary']['path']}` sha256={projection['source_refs']['loader_summary']['sha256']}",
        f"- Selection ref: `{projection['source_refs']['selection_ref']['path']}`",
        "",
        "## Summary",
        f"- Source family counts: `{json.dumps(summary['source_family_counts'], sort_keys=True)}`",
        f"- Source quality counts: `{json.dumps(summary['source_quality_status_counts'], sort_keys=True)}`",
        f"- PDF status counts: `{json.dumps(summary['pdf_status_counts'], sort_keys=True)}`",
        f"- Diagnostic counts: `{json.dumps(summary['diagnostic_counts'], sort_keys=True)}`",
        "",
        "## Warnings",
        f"- Warning diagnostics by item are retained in `items[*].warnings`; aggregate diagnostic counts are `{json.dumps(summary['diagnostic_counts'], sort_keys=True)}`.",
        "",
        "## Skipped Diagnostics",
        "- Optional bibliographic fields absent from S04 loader evidence remain null with `metadata_value_not_in_loader_evidence_bundle` diagnostics.",
        "- Parser, chunker, model, graph, KG import, and production writes are skipped by design.",
        "",
        "## Safety",
        f"- Redaction flags: `{json.dumps(projection['redaction_flags'], sort_keys=True)}`",
        f"- Unsafe counters: `{json.dumps(projection['unsafe_counters'], sort_keys=True)}`",
        "",
        "## Failure Modes",
    ]
    for item in projection["failure_modes"]:
        lines.append(f"- {item['dependency']}: {item['failure_path']}")
    lines.extend([
        "",
        "## Load Profile",
        f"- Expected refs: {projection['load_profile']['expected_url_refs']}; 10x refs: {projection['load_profile']['ten_x_url_refs']}",
        f"- First saturating resource: {projection['load_profile']['first_saturating_resource']}",
        f"- Protection: {projection['load_profile']['protection']}",
        "",
        "## Negative Tests",
    ])
    for test_name in projection["negative_tests"]:
        lines.append(f"- `{test_name}`")
    lines.extend([
        "",
        "## Observability Impact",
        "- Emits digest-level scope counters, input fingerprints, source refs, redaction flags, unsafe counters, per-ref quality/warning/skipped diagnostics, and stable validation codes/JSON paths for malformed inputs and safety drift.",
        "",
    ])
    return "\n".join(lines)


def build_projection_payload(bundles: list[dict[str, Any]], summary: dict[str, Any], bundles_path: Path, summary_path: Path) -> dict[str, Any]:
    unsafe_counts = count_unsafe_claims(bundles, summary)
    summary_payload = summarize_projection(bundles, unsafe_counts)
    selection_ref = summary.get("input_fingerprints", {}).get("selection") if isinstance(summary.get("input_fingerprints"), dict) else None
    return {
        "schema_name": SCHEMA_NAME,
        "schema_version": SCHEMA_VERSION,
        "created_at": "deterministic_from_input_sha256",
        "generated_at": "deterministic_from_input_sha256",
        "input_fingerprint": hashlib.sha256((sha256_file(bundles_path) + sha256_file(summary_path)).encode("utf-8")).hexdigest(),
        "generator": {
            "name": "build_m028_hermes_digest_projection.py",
            "mode": "evidence_only_s04_loader_projection",
            "network_calls_attempted": False,
            "parser_attempted": False,
            "chunker_attempted": False,
            "model_attempted": False,
            "graph_write_attempted": False,
            "production_write_attempted": False,
        },
        "source_refs": {
            "loader_bundle": safe_input_ref(bundles_path),
            "loader_summary": safe_input_ref(summary_path),
            "selection_ref": selection_ref if isinstance(selection_ref, dict) else {"path": None, "sha256": None},
        },
        "redaction_flags": {
            "raw_article_text_embedded": False,
            "html_source_embedded": False,
            "raw_pdf_bytes_embedded": False,
            "source_payload_embedded": False,
            "chunk_payload_embedded": False,
            "model_output_embedded": False,
            "local_absolute_paths_embedded": False,
            "graph_or_kg_claims_embedded": False,
        },
        "summary": summary_payload,
        "unsafe_counters": unsafe_counts,
        "items": [build_item(bundle) for bundle in bundles],
        "failure_modes": [
            {
                "dependency": "S04 universal-loader evidence bundle JSONL",
                "failure_path": "missing, malformed JSONL, non-object rows, scope drift, unsafe payload markers, absolute/escaping artifact paths, or unsafe bundle flags raise HermesDigestProjectionInputError before output writes",
            },
            {
                "dependency": "S04 universal-loader evidence summary JSON",
                "failure_path": "missing, malformed JSON, stale aggregate counts, nonzero unsafe counters, or missing selection/source fingerprints raise or localize stable diagnostics before projection writes",
            },
            {
                "dependency": "filesystem outputs",
                "failure_path": "output directory creation or file write failures bubble as filesystem exceptions; no network, subprocess, parser, model, graph, or production dependency exists",
            },
        ],
        "load_profile": {
            "expected_url_refs": EXPECTED_REF_COUNT,
            "ten_x_url_refs": EXPECTED_REF_COUNT * 10,
            "first_saturating_resource": "linear JSON/JSONL parsing and deterministic serialization of compact per-ref metadata-only digest items",
            "protection": "single-pass in-memory smoke-corpus processing, chunked SHA-256 for two input files, no source/PDF body reads, and no live network/parser/model/graph calls",
        },
        "negative_tests": [
            "tests/test_m028_hermes_digest_projection.py::test_rejects_scope_drift_before_projection_write",
            "tests/test_m028_hermes_digest_projection.py::test_rejects_payload_markers_and_absolute_paths",
            "tests/test_m028_hermes_digest_projection.py::test_rejects_nonzero_unsafe_counter",
        ],
    }


def build_hermes_digest_projection(bundles_path: Path, summary_path: Path, out_dir: Path) -> dict[str, Any]:
    diagnostics: list[Diagnostic] = []
    bundles = read_jsonl(bundles_path, diagnostics, "loader_bundle")
    summary = read_json(summary_path, diagnostics, "loader_summary")
    if diagnostics:
        raise HermesDigestProjectionInputError(diagnostics)
    assert bundles is not None
    assert summary is not None

    walk_forbidden(bundles, diagnostics, "$.bundles")
    walk_forbidden(summary, diagnostics, "$.summary")
    validate_artifact_paths(bundles, diagnostics)
    validate_scope(bundles, summary, diagnostics)
    unsafe_counts = count_unsafe_claims(bundles, summary)
    validate_unsafe_counters(unsafe_counts, diagnostics)
    if diagnostics:
        raise HermesDigestProjectionInputError(diagnostics)

    projection = build_projection_payload(bundles, summary, bundles_path, summary_path)
    # Re-check the generated projection for accidental forbidden payload leakage, excluding
    # deliberate diagnostic test names and report prose that mention forbidden categories.
    output_diagnostics: list[Diagnostic] = []
    walk_forbidden(projection["items"], output_diagnostics, "$.items")
    if output_diagnostics:
        raise HermesDigestProjectionInputError(output_diagnostics)

    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / DIGEST_FILENAME).write_text(json.dumps(projection, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    (out_dir / REPORT_FILENAME).write_text(render_report(projection), encoding="utf-8")
    return projection


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundles", required=True, type=Path)
    parser.add_argument("--summary", required=True, type=Path)
    parser.add_argument("--out-dir", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        projection = build_hermes_digest_projection(args.bundles, args.summary, args.out_dir)
    except HermesDigestProjectionInputError as exc:
        raise SystemExit(str(exc)) from exc
    sys.stdout.write(
        "wrote Hermes digest projection: "
        f"refs={projection['summary']['url_ref_count']} identities={projection['summary']['normalized_identity_count']} "
        f"digest={args.out_dir / DIGEST_FILENAME} report={args.out_dir / REPORT_FILENAME}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
