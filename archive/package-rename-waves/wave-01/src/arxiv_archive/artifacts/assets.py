"""Metadata-only article asset preservation for M024/S04.

This module preserves figures, diagrams, charts, tables, and equation images as
review assets with stable identifiers and provenance only.  It intentionally
never extracts image bytes, interprets captions/tables/equations, generates
embeddings/vectors, writes to LadybugDB, or marks assets as import eligible.

Formerly: src/arxiv_archive/article_assets.py
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from typing import Any, Literal

ARTICLE_ASSET_MANIFEST_SCHEMA_VERSION = "m024-article-assets.v1"
ARTICLE_ASSET_DIAGNOSTICS_SCHEMA_VERSION = "m024-article-assets-diagnostics.v1"
ARTICLE_ASSET_BUILDER = "metadata_only_article_assets_v1"

ArticleAssetType = Literal["figure", "diagram", "chart", "table", "equation_image"]
ArticleAssetPreservationState = Literal["placeholder_only", "source_linked", "binary_preserved", "unresolved"]
ArticleAssetInterpretationStatus = Literal[
    "not_interpreted",
    "needs_human_review",
    "interpretation_deferred",
    "not_applicable",
]
ArticleAssetDiagnosticSeverity = Literal["info", "warning", "repair_required", "error"]

ALLOWED_ASSET_TYPES = frozenset(ArticleAssetType.__args__)  # type: ignore[attr-defined]
ALLOWED_PRESERVATION_STATES = frozenset(ArticleAssetPreservationState.__args__)  # type: ignore[attr-defined]
ALLOWED_INTERPRETATION_STATUSES = frozenset(ArticleAssetInterpretationStatus.__args__)  # type: ignore[attr-defined]
TASK_ALLOWED_ASSET_TYPES = frozenset({"figure", "diagram", "chart", "table", "equation_image", "unknown_visual"})
TASK_ALLOWED_PRESERVATION_STATES = frozenset({"preserved", "linked_not_extracted", "missing_source", "blocked", "not_attempted"})
TASK_ALLOWED_INTERPRETATION_STATUSES = frozenset({"not_interpreted", "metadata_only", "review_required", "blocked"})
_ACCEPTED_ASSET_TYPES = ALLOWED_ASSET_TYPES | TASK_ALLOWED_ASSET_TYPES
_ACCEPTED_PRESERVATION_STATES = ALLOWED_PRESERVATION_STATES | TASK_ALLOWED_PRESERVATION_STATES
_ACCEPTED_INTERPRETATION_STATUSES = ALLOWED_INTERPRETATION_STATUSES | TASK_ALLOWED_INTERPRETATION_STATUSES
ALLOWED_COORDINATE_SPACES = frozenset({"page_bbox", "artifact_record", "normalized_markdown_char", "semantic_chunk_char"})

FORBIDDEN_PAYLOAD_KEYS = frozenset(
    {
        "text",
        "raw_text",
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
    }
)
FORBIDDEN_SOURCE_OF_TRUTH_KEYS = frozenset(
    {"source_of_truth", "source_of_truth_claim", "truth_source", "canonical_source", "minimax_source_of_truth"}
)
UNSAFE_TRUE_FLAGS = {
    "trusted_kg_import_allowed": "unsafe_trusted_import_flag",
    "production_import_attempted": "unsafe_production_import_flag",
    "ladybugdb_written": "unsafe_ladybugdb_written_flag",
    "import_eligible": "unsafe_import_eligible_flag",
    "promoted_to_fact": "unsafe_promoted_to_fact_flag",
}
UNSAFE_READINESS_STATUSES = frozenset({"ready_for_import", "import_ready", "trusted", "promoted", "fact"})
_ASSET_REF_RE = re.compile(r"^[A-Za-z0-9_.:-]+$")


def default_safety_flags() -> dict[str, bool]:
    """Return required false safety flags for the metadata-only asset boundary."""
    return {
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "raw_text_included": False,
        "raw_binary_included": False,
        "base64_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "model_outputs_included": False,
    }


@dataclass(frozen=True)
class ArticleAssetDiagnostic:
    """One stable, redacted diagnostic for asset preservation validation."""

    code: str
    json_path: str
    severity: ArticleAssetDiagnosticSeverity = "repair_required"
    object_id: str | None = None
    message: str = "Article asset diagnostic; inspect stable code and JSON path, not source content."
    blocks_import: bool = True

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "json_path": self.json_path,
            "severity": self.severity,
            "object_id": self.object_id,
            "message": self.message,
            "blocks_import": self.blocks_import,
        }


@dataclass(frozen=True)
class ArticleAssetRecord:
    """Metadata-only review asset with PageIndex/source provenance."""

    asset_id: str
    paper_id: str
    asset_type: str
    source_asset_ref: str
    source_file_id: str | None
    source_sha256: str | None
    source_span_id: str | None
    source_span: dict[str, Any] | None
    page_index_node_id: str | None
    page_index_anchor_id: str | None
    preservation_state: str
    interpretation_status: str

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "asset_id": self.asset_id,
            "paper_id": self.paper_id,
            "asset_type": self.asset_type,
            "source_asset_ref": self.source_asset_ref,
            "source_file_id": self.source_file_id,
            "source_sha256": self.source_sha256,
            "source_span_id": self.source_span_id,
            "source_span": _redacted_span(self.source_span),
            "page_index_node_id": self.page_index_node_id,
            "page_index_anchor_id": self.page_index_anchor_id,
            "preservation_state": self.preservation_state,
            "interpretation_status": self.interpretation_status,
            "raw_binary_embedded": False,
            "base64_embedded": False,
            "import_eligible": False,
            "promoted_to_fact": False,
        }


def build_article_asset_manifest(payload: dict[str, Any]) -> dict[str, Any]:
    """Build a deterministic metadata-only asset manifest from source/PageIndex refs.

    Malformed inputs are returned as redacted diagnostics rather than raised
    exceptions so callers can inspect blocker counts without exposing raw
    payloads or binary data.
    """
    safe_payload = payload if isinstance(payload, dict) else {}
    paper_id = _non_empty_string(safe_payload.get("paper_id")) or "unknown-paper"
    run_id = _non_empty_string(safe_payload.get("run_id")) or "unknown-run"
    source_refs = _list_of_dicts(safe_payload.get("source_refs"))
    page_index = safe_payload.get("page_index") if isinstance(safe_payload.get("page_index"), dict) else {}
    placeholders = _list_of_dicts(safe_payload.get("asset_placeholders"))

    diagnostics: list[ArticleAssetDiagnostic] = []
    diagnostics.extend(_validate_forbidden_keys(safe_payload))
    diagnostics.extend(_validate_source_of_truth_markers(safe_payload))
    diagnostics.extend(_validate_unsafe_flags(safe_payload, ""))
    diagnostics.extend(_validate_source_refs(source_refs, paper_id))

    sources_by_id = {str(source.get("source_id")): source for source in source_refs if _non_empty_string(source.get("source_id"))}
    nodes_by_id = {str(node.get("node_id")): node for node in _list_of_dicts(page_index.get("nodes")) if _non_empty_string(node.get("node_id"))}
    anchors_by_id = {
        str(anchor.get("anchor_id")): anchor
        for anchor in _list_of_dicts(page_index.get("anchors"))
        if _non_empty_string(anchor.get("anchor_id"))
    }

    records: list[dict[str, Any]] = []
    seen_asset_ids: set[str] = set()
    for index, placeholder in enumerate(placeholders):
        object_id = _object_id(placeholder, index)
        path = f"/asset_placeholders/{index}"
        diagnostics.extend(_validate_placeholder(placeholder, path, object_id, sources_by_id, nodes_by_id, anchors_by_id))
        record = _asset_record_from_placeholder(placeholder, paper_id, index, sources_by_id).to_redacted_dict()
        if record["asset_id"] in seen_asset_ids:
            diagnostics.append(_diagnostic("duplicate_asset_id", f"{path}/source_asset_ref", object_id))
        else:
            seen_asset_ids.add(record["asset_id"])
        records.append(record)

    diagnostics.extend(_validate_page_index_boundary(page_index))
    diagnostic_dicts = _dedupe_diagnostics(diagnostics)
    summary = summarize_article_assets(records, source_refs, diagnostic_dicts)
    status = "blocked" if summary["blocker_count"] else "review_only_not_import_eligible"
    return {
        "schema_version": ARTICLE_ASSET_MANIFEST_SCHEMA_VERSION,
        "diagnostics_schema_version": ARTICLE_ASSET_DIAGNOSTICS_SCHEMA_VERSION,
        "builder": ARTICLE_ASSET_BUILDER,
        "paper_id": paper_id,
        "run_id": run_id,
        "manifest_id": f"{paper_id}:article-assets:manifest:v1",
        "source_refs": [_redacted_source_ref(source) for source in source_refs],
        "page_index_manifest": {
            "schema_version": _string_or_none(page_index.get("schema_version")),
            "manifest_path": _string_or_none(page_index.get("manifest_path")),
            "manifest_sha256": _string_or_none(page_index.get("manifest_sha256")),
        },
        "assets": records,
        "summary": summary,
        "subtree": {
            "status": status,
            "asset_count": len(records),
            "blocker_count": summary["blocker_count"],
            "trusted_kg_import_allowed": False,
            "ladybugdb_written": False,
            "production_import_attempted": False,
        },
        "diagnostics": diagnostic_dicts,
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "safety_flags": default_safety_flags(),
    }


def validate_article_asset_manifest(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    """Validate a built asset manifest and return redacted diagnostics."""
    payload = manifest if isinstance(manifest, dict) else {}
    diagnostics: list[ArticleAssetDiagnostic] = []
    if payload.get("schema_version") != ARTICLE_ASSET_MANIFEST_SCHEMA_VERSION:
        diagnostics.append(_diagnostic("invalid_schema_version", "/schema_version"))
    if payload.get("diagnostics_schema_version") != ARTICLE_ASSET_DIAGNOSTICS_SCHEMA_VERSION:
        diagnostics.append(_diagnostic("invalid_diagnostics_schema_version", "/diagnostics_schema_version"))
    diagnostics.extend(_validate_forbidden_keys(payload))
    diagnostics.extend(_validate_source_of_truth_markers(payload))
    diagnostics.extend(_validate_unsafe_flags(payload, ""))
    diagnostics.extend(_validate_safety_flags(payload.get("safety_flags"), "/safety_flags"))
    for key in ("import_eligible_count", "promoted_to_fact_count"):
        if payload.get(key) != 0:
            diagnostics.append(_diagnostic(f"{key}_nonzero", f"/{key}"))
    for key, code in (("production_import_attempted", "unsafe_production_import_flag"), ("ladybugdb_written", "unsafe_ladybugdb_written_flag")):
        if payload.get(key) is not False:
            diagnostics.append(_diagnostic(code, f"/{key}"))

    source_refs = _list_of_dicts(payload.get("source_refs"))
    diagnostics.extend(_validate_source_refs(source_refs, _string_or_none(payload.get("paper_id"))))
    assets = _list_of_dicts(payload.get("assets"))
    seen: set[str] = set()
    for index, asset in enumerate(assets):
        object_id = _string_or_none(asset.get("source_asset_ref")) or _string_or_none(asset.get("asset_id"))
        path = f"/assets/{index}"
        diagnostics.extend(_required(asset, ("asset_id", "paper_id", "asset_type", "source_file_id", "preservation_state", "interpretation_status"), path, object_id))
        asset_id = asset.get("asset_id")
        if isinstance(asset_id, str) and asset_id:
            if asset_id in seen:
                diagnostics.append(_diagnostic("duplicate_asset_id", f"{path}/asset_id", object_id))
            seen.add(asset_id)
        if asset.get("asset_type") not in _ACCEPTED_ASSET_TYPES:
            diagnostics.append(_diagnostic("invalid_asset_type", f"{path}/asset_type", object_id))
        if asset.get("preservation_state") not in _ACCEPTED_PRESERVATION_STATES:
            diagnostics.append(_diagnostic("invalid_preservation_state", f"{path}/preservation_state", object_id))
        if asset.get("interpretation_status") not in _ACCEPTED_INTERPRETATION_STATUSES:
            diagnostics.append(_diagnostic("invalid_interpretation_status", f"{path}/interpretation_status", object_id))
        if asset.get("raw_binary_embedded") is not False:
            diagnostics.append(_diagnostic("raw_binary_embedded", f"{path}/raw_binary_embedded", object_id))
        if asset.get("base64_embedded") is not False:
            diagnostics.append(_diagnostic("base64_embedded", f"{path}/base64_embedded", object_id))
        if asset.get("import_eligible") is not False:
            diagnostics.append(_diagnostic("unsafe_import_eligible_flag", f"{path}/import_eligible", object_id))
        if asset.get("promoted_to_fact") is not False:
            diagnostics.append(_diagnostic("unsafe_promoted_to_fact_flag", f"{path}/promoted_to_fact", object_id))
        if isinstance(asset.get("source_span"), dict):
            diagnostics.extend(_validate_source_span(asset["source_span"], f"{path}/source_span", object_id))
    for index, diagnostic in enumerate(_list_of_dicts(payload.get("diagnostics"))):
        diagnostics.extend(_validate_diagnostic_record(diagnostic, f"/diagnostics/{index}"))
    return _dedupe_diagnostics(diagnostics)


def summarize_article_assets(
    assets: list[dict[str, Any]], source_refs: list[dict[str, Any]], diagnostics: list[dict[str, Any]] | None = None
) -> dict[str, Any]:
    """Return deterministic manifest-level observability counters."""
    diagnostics = diagnostics or []
    asset_count = len(assets)
    hash_count = sum(1 for asset in assets if _valid_sha256(asset.get("source_sha256")))
    anchor_count = sum(1 for asset in assets if _non_empty_string(asset.get("page_index_anchor_id")))
    span_count = sum(1 for asset in assets if isinstance(asset.get("source_span"), dict) and _non_empty_string(asset["source_span"].get("span_id")))
    return {
        "asset_count": asset_count,
        "asset_counts_by_type": _counts(asset.get("asset_type") for asset in assets),
        "preservation_state_counts": _counts(asset.get("preservation_state") for asset in assets),
        "interpretation_status_counts": _counts(asset.get("interpretation_status") for asset in assets),
        "source_ref_count": len(source_refs),
        "page_index_node_ref_count": len({asset.get("page_index_node_id") for asset in assets if asset.get("page_index_node_id")}),
        "page_index_anchor_ref_count": len({asset.get("page_index_anchor_id") for asset in assets if asset.get("page_index_anchor_id")}),
        "hash_coverage_rate": hash_count / asset_count if asset_count else 0.0,
        "page_index_anchor_coverage_rate": anchor_count / asset_count if asset_count else 0.0,
        "source_span_coverage_rate": span_count / asset_count if asset_count else 0.0,
        "blocker_count": sum(1 for diagnostic in diagnostics if diagnostic.get("blocks_import") is True),
        "import_ineligible_count": asset_count,
        "diagnostic_count_by_code": _counts(diagnostic.get("code") for diagnostic in diagnostics),
    }


def attach_article_assets_summary(
    evidence_bundle: dict[str, Any],
    manifest: dict[str, Any],
    *,
    manifest_path: str | None = None,
    manifest_sha256: str | None = None,
) -> dict[str, Any]:
    """Attach metadata-only asset observability to an ArticleEvidenceBundle dict."""
    payload = dict(evidence_bundle)
    summary = manifest.get("summary") if isinstance(manifest.get("summary"), dict) else {}
    subtree = manifest.get("subtree") if isinstance(manifest.get("subtree"), dict) else {}
    subtrees = dict(payload.get("subtrees") if isinstance(payload.get("subtrees"), dict) else {})
    subtrees["assets"] = {
        "status": subtree.get("status", "blocked"),
        "manifest_path": manifest_path,
        "manifest_sha256": manifest_sha256,
        "manifest_schema_version": ARTICLE_ASSET_MANIFEST_SCHEMA_VERSION,
        "asset_count": int(summary.get("asset_count", 0) or 0),
        "asset_counts_by_type": dict(summary.get("asset_counts_by_type", {})),
        "preservation_state_counts": dict(summary.get("preservation_state_counts", {})),
        "interpretation_status_counts": dict(summary.get("interpretation_status_counts", {})),
        "blocker_count": int(summary.get("blocker_count", 0) or 0),
        "import_ineligible_count": int(summary.get("import_ineligible_count", 0) or 0),
        "hash_coverage_rate": float(summary.get("hash_coverage_rate", 0.0) or 0.0),
        "page_index_anchor_coverage_rate": float(summary.get("page_index_anchor_coverage_rate", 0.0) or 0.0),
        "source_span_coverage_rate": float(summary.get("source_span_coverage_rate", 0.0) or 0.0),
        "trusted_kg_import_allowed": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }
    payload["subtrees"] = subtrees
    bundle_summary = dict(payload.get("summary") if isinstance(payload.get("summary"), dict) else {})
    bundle_summary["asset_count"] = subtrees["assets"]["asset_count"]
    bundle_summary["asset_blocker_count"] = subtrees["assets"]["blocker_count"]
    bundle_summary["asset_import_ineligible_count"] = subtrees["assets"]["import_ineligible_count"]
    payload["summary"] = bundle_summary
    payload["import_eligible_count"] = 0
    payload["promoted_to_fact_count"] = 0
    payload["production_import_attempted"] = False
    payload["ladybugdb_written"] = False
    return payload


def to_json(value: dict[str, Any]) -> str:
    """Serialize an asset artifact deterministically."""
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def _asset_record_from_placeholder(
    placeholder: dict[str, Any], paper_id: str, index: int, sources_by_id: dict[str, dict[str, Any]]
) -> ArticleAssetRecord:
    asset_type = _string_or_none(placeholder.get("asset_type")) or "figure"
    source_asset_ref = _string_or_none(placeholder.get("source_asset_ref")) or f"unknown:{index + 1}"
    source_file_id = _string_or_none(placeholder.get("source_file_id"))
    source = sources_by_id.get(source_file_id or "", {})
    source_span = placeholder.get("source_span") if isinstance(placeholder.get("source_span"), dict) else None
    return ArticleAssetRecord(
        asset_id=_asset_id(paper_id, asset_type, source_asset_ref, index),
        paper_id=paper_id,
        asset_type=asset_type,
        source_asset_ref=source_asset_ref,
        source_file_id=source_file_id,
        source_sha256=_string_or_none(source.get("sha256")),
        source_span_id=_string_or_none(placeholder.get("source_span_id")) or _string_or_none((source_span or {}).get("span_id")),
        source_span=source_span,
        page_index_node_id=_string_or_none(placeholder.get("page_index_node_id")),
        page_index_anchor_id=_string_or_none(placeholder.get("page_index_anchor_id")),
        preservation_state=_string_or_none(placeholder.get("preservation_state")) or "placeholder_only",
        interpretation_status=_string_or_none(placeholder.get("interpretation_status")) or "not_interpreted",
    )


def _asset_id(paper_id: str, asset_type: str, source_asset_ref: str, index: int) -> str:
    normalized_type = "equation-image" if asset_type == "equation_image" else _slug(asset_type or "unknown")
    ordinal = _ordinal(source_asset_ref) or f"{index + 1:04d}"
    return f"{paper_id}:asset:{normalized_type}:{ordinal}"


def _ordinal(value: str) -> str | None:
    tail = value.rsplit(":", 1)[-1]
    if tail.isdigit():
        return f"{int(tail):04d}"
    return None


def _validate_placeholder(
    placeholder: dict[str, Any],
    path: str,
    object_id: str,
    sources_by_id: dict[str, dict[str, Any]],
    nodes_by_id: dict[str, dict[str, Any]],
    anchors_by_id: dict[str, dict[str, Any]],
) -> list[ArticleAssetDiagnostic]:
    diagnostics: list[ArticleAssetDiagnostic] = []
    diagnostics.extend(_required(placeholder, ("source_asset_ref", "asset_type", "source_file_id", "page_index_node_id", "page_index_anchor_id"), path, object_id))
    source_asset_ref = placeholder.get("source_asset_ref")
    if not _non_empty_string(source_asset_ref) or not _ASSET_REF_RE.match(str(source_asset_ref)):
        diagnostics.append(_diagnostic("invalid_source_asset_ref", f"{path}/source_asset_ref", object_id))
    if placeholder.get("asset_type") not in _ACCEPTED_ASSET_TYPES:
        diagnostics.append(_diagnostic("invalid_asset_type", f"{path}/asset_type", object_id))
    if placeholder.get("preservation_state") not in _ACCEPTED_PRESERVATION_STATES:
        diagnostics.append(_diagnostic("invalid_preservation_state", f"{path}/preservation_state", object_id))
    if placeholder.get("interpretation_status") not in _ACCEPTED_INTERPRETATION_STATUSES:
        diagnostics.append(_diagnostic("invalid_interpretation_status", f"{path}/interpretation_status", object_id))
    source_file_id = placeholder.get("source_file_id")
    if not _non_empty_string(source_file_id) or str(source_file_id) not in sources_by_id:
        diagnostics.append(_diagnostic("missing_source_ref", f"{path}/source_file_id", object_id))
    node_id = placeholder.get("page_index_node_id")
    if not _non_empty_string(node_id) or str(node_id) not in nodes_by_id:
        diagnostics.append(_diagnostic("missing_page_index_ref", f"{path}/page_index_node_id", object_id))
    anchor_id = placeholder.get("page_index_anchor_id")
    if not _non_empty_string(anchor_id) or str(anchor_id) not in anchors_by_id:
        diagnostics.append(_diagnostic("missing_page_index_ref", f"{path}/page_index_anchor_id", object_id))
    source_span = placeholder.get("source_span")
    if isinstance(source_span, dict):
        diagnostics.extend(_validate_source_span(source_span, f"{path}/source_span", object_id))
    else:
        diagnostics.append(_diagnostic("missing_source_span", f"{path}/source_span", object_id))
    diagnostics.extend(_validate_unsafe_flags(placeholder, path, object_id))
    return diagnostics


def _validate_source_refs(source_refs: list[dict[str, Any]], paper_id: str | None) -> list[ArticleAssetDiagnostic]:
    diagnostics: list[ArticleAssetDiagnostic] = []
    seen: set[str] = set()
    for index, source in enumerate(source_refs):
        path = f"/source_refs/{index}"
        object_id = _string_or_none(source.get("source_id"))
        diagnostics.extend(_required(source, ("source_id", "paper_id", "source_path", "media_type", "sha256", "byte_size"), path, object_id))
        if object_id:
            if object_id in seen:
                diagnostics.append(_diagnostic("duplicate_source_id", f"{path}/source_id", object_id))
            seen.add(object_id)
        if paper_id is not None and source.get("paper_id") != paper_id:
            diagnostics.append(_diagnostic("source_ref_paper_id_mismatch", f"{path}/paper_id", object_id))
        if not _valid_sha256(source.get("sha256")):
            diagnostics.append(_diagnostic("malformed_sha256", f"{path}/sha256", object_id))
        if not isinstance(source.get("byte_size"), int) or int(source.get("byte_size", -1)) < 0:
            diagnostics.append(_diagnostic("invalid_byte_size", f"{path}/byte_size", object_id))
        if source.get("raw_text_embedded") is not False:
            diagnostics.append(_diagnostic("source_ref_raw_text_embedded", f"{path}/raw_text_embedded", object_id))
        if source.get("raw_binary_embedded") is not False:
            diagnostics.append(_diagnostic("source_ref_raw_binary_embedded", f"{path}/raw_binary_embedded", object_id))
    return diagnostics


def _validate_source_span(span: dict[str, Any], path: str, object_id: str | None) -> list[ArticleAssetDiagnostic]:
    diagnostics: list[ArticleAssetDiagnostic] = []
    diagnostics.extend(_required(span, ("span_id", "source_id", "coordinate_space", "span_hash", "raw_text_embedded"), path, object_id))
    if span.get("coordinate_space") not in ALLOWED_COORDINATE_SPACES:
        diagnostics.append(_diagnostic("invalid_coordinate_space", f"{path}/coordinate_space", object_id))
    if span.get("raw_text_embedded") is not False:
        diagnostics.append(_diagnostic("span_raw_text_embedded", f"{path}/raw_text_embedded", object_id))
    if span.get("span_hash") is not None and not _valid_sha256(span.get("span_hash")):
        diagnostics.append(_diagnostic("malformed_span_hash", f"{path}/span_hash", object_id))
    bbox = span.get("bbox")
    if bbox is not None and (not isinstance(bbox, list) or len(bbox) != 4 or not all(isinstance(value, int | float) for value in bbox)):
        diagnostics.append(_diagnostic("invalid_bbox", f"{path}/bbox", object_id))
    page_start = span.get("page_start")
    page_end = span.get("page_end")
    if page_start is not None and (not isinstance(page_start, int) or page_start < 1):
        diagnostics.append(_diagnostic("invalid_page_start", f"{path}/page_start", object_id))
    if page_end is not None and (not isinstance(page_end, int) or page_end < 1):
        diagnostics.append(_diagnostic("invalid_page_end", f"{path}/page_end", object_id))
    if isinstance(page_start, int) and isinstance(page_end, int) and page_end < page_start:
        diagnostics.append(_diagnostic("invalid_page_range", path, object_id))
    char_start = span.get("char_start")
    char_end = span.get("char_end")
    if char_start is not None and not isinstance(char_start, int):
        diagnostics.append(_diagnostic("invalid_char_start", f"{path}/char_start", object_id))
    if char_end is not None and not isinstance(char_end, int):
        diagnostics.append(_diagnostic("invalid_char_end", f"{path}/char_end", object_id))
    if isinstance(char_start, int) and isinstance(char_end, int) and char_end < char_start:
        diagnostics.append(_diagnostic("invalid_char_range", path, object_id))
    return diagnostics


def _validate_page_index_boundary(page_index: dict[str, Any]) -> list[ArticleAssetDiagnostic]:
    diagnostics: list[ArticleAssetDiagnostic] = []
    bridge = page_index.get("bridge_subtree")
    if isinstance(bridge, dict):
        diagnostics.extend(_validate_unsafe_flags(bridge, "/page_index/bridge_subtree"))
    return diagnostics


def _validate_unsafe_flags(value: Any, path: str, object_id: str | None = None) -> list[ArticleAssetDiagnostic]:
    diagnostics: list[ArticleAssetDiagnostic] = []
    if not isinstance(value, dict):
        return diagnostics
    for key, code in UNSAFE_TRUE_FLAGS.items():
        if value.get(key) is True:
            diagnostics.append(_diagnostic(code, f"{path}/{key}" if path else f"/{key}", object_id or _object_id(value)))
    readiness = value.get("readiness_status")
    if isinstance(readiness, str) and readiness in UNSAFE_READINESS_STATUSES:
        diagnostics.append(_diagnostic("unsafe_readiness_status", f"{path}/readiness_status" if path else "/readiness_status", object_id or _object_id(value)))
    return diagnostics


def _validate_safety_flags(flags: Any, path: str) -> list[ArticleAssetDiagnostic]:
    if not isinstance(flags, dict):
        return [_diagnostic("missing_safety_flags", path)]
    diagnostics: list[ArticleAssetDiagnostic] = []
    for key, expected in default_safety_flags().items():
        if flags.get(key) is not expected:
            diagnostics.append(_diagnostic(f"safety_flag_invalid:{key}", f"{path}/{key}"))
    return diagnostics


def _validate_diagnostic_record(record: dict[str, Any], path: str) -> list[ArticleAssetDiagnostic]:
    diagnostics = _required(record, ("code", "json_path", "severity", "blocks_import"), path, _string_or_none(record.get("object_id")))
    if record.get("severity") not in {"info", "warning", "repair_required", "error"}:
        diagnostics.append(_diagnostic("invalid_diagnostic_severity", f"{path}/severity", _string_or_none(record.get("object_id"))))
    if not isinstance(record.get("json_path"), str) or not str(record.get("json_path", "")).startswith("/"):
        diagnostics.append(_diagnostic("invalid_diagnostic_json_path", f"{path}/json_path", _string_or_none(record.get("object_id"))))
    return diagnostics


def _redacted_source_ref(source: dict[str, Any]) -> dict[str, Any]:
    allowed = {
        "source_id",
        "paper_id",
        "source_path",
        "source_type",
        "source_role",
        "media_type",
        "sha256",
        "byte_size",
        "parser_name",
        "loader_name",
        "load_outcome",
        "failure_reason",
        "warning_count",
        "duration_ms",
    }
    result = {key: source.get(key) for key in allowed if key in source}
    result["raw_text_embedded"] = False
    result["raw_binary_embedded"] = False
    return result


def _redacted_span(span: dict[str, Any] | None) -> dict[str, Any] | None:
    if not isinstance(span, dict):
        return None
    allowed = {
        "span_id",
        "source_id",
        "coordinate_space",
        "char_start",
        "char_end",
        "page_start",
        "page_end",
        "bbox",
        "span_hash",
        "raw_text_embedded",
    }
    result = {key: span.get(key) for key in allowed if key in span}
    result["raw_text_embedded"] = False
    return result


def _validate_forbidden_keys(value: Any, path: str = "", object_id: str | None = None) -> list[ArticleAssetDiagnostic]:
    diagnostics: list[ArticleAssetDiagnostic] = []
    if isinstance(value, dict):
        current_object_id = object_id or _object_id(value)
        for key, child in value.items():
            child_path = f"{path}/{key}" if path else f"/{key}"
            if key in FORBIDDEN_PAYLOAD_KEYS:
                diagnostics.append(_diagnostic("forbidden_payload_key", child_path, current_object_id))
                continue
            diagnostics.extend(_validate_forbidden_keys(child, child_path, current_object_id))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            diagnostics.extend(_validate_forbidden_keys(child, f"{path}/{index}" if path else f"/{index}", object_id))
    return diagnostics


def _validate_source_of_truth_markers(value: Any, path: str = "") -> list[ArticleAssetDiagnostic]:
    diagnostics: list[ArticleAssetDiagnostic] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}/{key}" if path else f"/{key}"
            if key.lower() in FORBIDDEN_SOURCE_OF_TRUTH_KEYS:
                diagnostics.append(_diagnostic("source_of_truth_claim", child_path, _object_id(value)))
                continue
            diagnostics.extend(_validate_source_of_truth_markers(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            diagnostics.extend(_validate_source_of_truth_markers(child, f"{path}/{index}" if path else f"/{index}"))
    return diagnostics


def _required(value: dict[str, Any], fields: tuple[str, ...], path: str, object_id: str | None) -> list[ArticleAssetDiagnostic]:
    diagnostics: list[ArticleAssetDiagnostic] = []
    for field_name in fields:
        if field_name not in value or value.get(field_name) is None:
            diagnostics.append(_diagnostic(f"missing_{field_name}", f"{path}/{field_name}", object_id))
    return diagnostics


def _diagnostic(
    code: str,
    json_path: str,
    object_id: str | None = None,
    *,
    severity: ArticleAssetDiagnosticSeverity = "repair_required",
    blocks_import: bool = True,
) -> ArticleAssetDiagnostic:
    return ArticleAssetDiagnostic(code=code, json_path=json_path, object_id=object_id, severity=severity, blocks_import=blocks_import)


def _dedupe_diagnostics(diagnostics: list[ArticleAssetDiagnostic]) -> list[dict[str, Any]]:
    seen: set[tuple[str, str, str | None]] = set()
    records: list[dict[str, Any]] = []
    for diagnostic in diagnostics:
        key = (diagnostic.code, diagnostic.json_path, diagnostic.object_id)
        if key in seen:
            continue
        seen.add(key)
        records.append(diagnostic.to_redacted_dict())
    return sorted(records, key=lambda item: (str(item.get("json_path")), str(item.get("code")), str(item.get("object_id"))))


def _object_id(value: dict[str, Any], index: int | None = None) -> str | None:
    for key in ("source_asset_ref", "asset_id", "source_file_id", "source_id", "node_id", "anchor_id", "span_id"):
        candidate = value.get(key)
        if _non_empty_string(candidate):
            return str(candidate)
    if index is not None:
        return f"asset-placeholder:{index}"
    return None


def _valid_sha256(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(character in "0123456789abcdef" for character in value.lower())


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _string_or_none(value: Any) -> str | None:
    return str(value) if value is not None else None


def _non_empty_string(value: Any) -> str | None:
    return value if isinstance(value, str) and value.strip() else None


def _slug(value: str) -> str:
    return re.sub(r"[^a-z0-9-]+", "-", value.lower().replace("_", "-")).strip("-") or "unknown"


def _counts(values: Any) -> dict[str, int]:
    counts: dict[str, int] = {}
    for value in values:
        if value is None:
            continue
        key = str(value)
        counts[key] = counts.get(key, 0) + 1
    return dict(sorted(counts.items()))


__all__ = [
    "ALLOWED_ASSET_TYPES",
    "ALLOWED_INTERPRETATION_STATUSES",
    "ALLOWED_PRESERVATION_STATES",
    "ARTICLE_ASSET_BUILDER",
    "ARTICLE_ASSET_DIAGNOSTICS_SCHEMA_VERSION",
    "ARTICLE_ASSET_MANIFEST_SCHEMA_VERSION",
    "ArticleAssetDiagnostic",
    "ArticleAssetRecord",
    "attach_article_assets_summary",
    "build_article_asset_manifest",
    "default_safety_flags",
    "summarize_article_assets",
    "to_json",
    "validate_article_asset_manifest",
]
