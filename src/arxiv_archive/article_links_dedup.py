"""Metadata-only article link and deduplication manifest contract.

This module defines deterministic, redacted records for article evidence links and
preprint deduplication candidates.  It intentionally carries only identifiers,
hashes, source/page-index references, review states, counters, and diagnostics;
it never serializes article prose, raw references, abstracts, model output,
embeddings, vectors, secrets, graph writes, or import authorization.
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass, field
from typing import Any, Literal
from urllib.parse import urlsplit, urlunsplit

ARTICLE_LINKS_DEDUP_SCHEMA_VERSION = "m024-article-links-dedup.v1"

ReviewState = Literal["review_required", "blocked", "repair_required", "accepted", "rejected", "ambiguous"]
DiagnosticSeverity = Literal["info", "warning", "repair_required", "error"]
MetadataSignalType = Literal["doi", "arxiv_id", "url", "content_hash", "title_author_year_hash"]
DedupDecision = Literal[
    "candidate_same_work_review_required",
    "conflicting_metadata_review_required",
    "insufficient_metadata_review_required",
    "not_same_work_review_required",
]

ALLOWED_REVIEW_STATES = frozenset(ReviewState.__args__)  # type: ignore[attr-defined]
ALLOWED_STRUCTURAL_RELATIONSHIPS = frozenset({"located_in", "contains", "adjacent_to", "derived_from", "mentions"})
ALLOWED_METADATA_SIGNAL_TYPES = frozenset(MetadataSignalType.__args__)  # type: ignore[attr-defined]
ALLOWED_DEDUP_DECISIONS = frozenset(DedupDecision.__args__)  # type: ignore[attr-defined]
ALLOWED_CANDIDATE_FAMILIES = frozenset({"preprint_dedup"})
ALLOWED_CONFIDENCE_LABELS = frozenset({"low", "medium", "high", "unknown"})

DIAGNOSTIC_COUNTER_KEYS = (
    "duplicate_id_count",
    "malformed_source_ref_count",
    "missing_page_index_anchor_count",
    "bad_vocabulary_count",
    "conflict_count",
    "insufficient_metadata_count",
    "forbidden_payload_detection_count",
    "unsafe_authorization_count",
)

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
        "title",
        "abstract",
        "reference",
        "references",
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
UNSAFE_FLAG_KEYS = frozenset(
    {
        "trusted_kg_import_allowed",
        "ladybugdb_written",
        "production_import_attempted",
        "model_outputs_included",
        "raw_payloads_included",
        "import_eligible",
        "promoted_to_fact",
    }
)


@dataclass(frozen=True)
class ArticleLinksDedupDiagnostic:
    """One stable, redacted diagnostic for link/dedup validation."""

    code: str
    json_path: str
    severity: DiagnosticSeverity = "repair_required"
    object_id: str | None = None
    message: str = "Article link/dedup diagnostic; inspect stable code and JSON path, not source content."
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
class CitationLinkRecord:
    link_id: str
    source_page_index_node_id: str
    source_page_index_anchor_id: str
    target_ref: dict[str, Any]
    source_span_ids: tuple[str, ...] = ()
    evidence_signal_ids: tuple[str, ...] = ()
    review_state: ReviewState = "review_required"

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "link_id": self.link_id,
            "link_family": "citation",
            "source_page_index_node_id": self.source_page_index_node_id,
            "source_page_index_anchor_id": self.source_page_index_anchor_id,
            "target_ref": dict(self.target_ref),
            "source_span_ids": list(self.source_span_ids),
            "evidence_signal_ids": list(self.evidence_signal_ids),
            "review_state": self.review_state,
            "promoted_to_fact": False,
            "import_eligible": False,
        }


@dataclass(frozen=True)
class StructuralLinkRecord:
    link_id: str
    relationship: str
    source_page_index_node_id: str
    target_page_index_node_id: str
    source_page_index_anchor_id: str
    source_span_ids: tuple[str, ...] = ()
    review_state: ReviewState = "review_required"

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "link_id": self.link_id,
            "link_family": "structural",
            "relationship": self.relationship,
            "source_page_index_node_id": self.source_page_index_node_id,
            "target_page_index_node_id": self.target_page_index_node_id,
            "source_page_index_anchor_id": self.source_page_index_anchor_id,
            "source_span_ids": list(self.source_span_ids),
            "review_state": self.review_state,
            "promoted_to_fact": False,
            "import_eligible": False,
        }


@dataclass(frozen=True)
class MetadataSignalRecord:
    signal_id: str
    signal_type: str
    normalized_value: str
    source_page_index_anchor_id: str
    source_span_id: str
    normalization: dict[str, Any] = field(default_factory=dict)
    review_state: ReviewState = "review_required"

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "signal_id": self.signal_id,
            "signal_type": self.signal_type,
            "normalized_value": self.normalized_value,
            "source_page_index_anchor_id": self.source_page_index_anchor_id,
            "source_span_id": self.source_span_id,
            "normalization": dict(self.normalization),
            "review_state": self.review_state,
            "promoted_to_fact": False,
            "import_eligible": False,
        }


@dataclass(frozen=True)
class DedupCandidateRecord:
    candidate_id: str
    decision: str
    source_record_ref: str
    target_record_ref: str | None
    evidence_signal_ids: tuple[str, ...] = ()
    candidate_family: str = "preprint_dedup"
    confidence_label: str = "unknown"
    review_state: ReviewState = "review_required"

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "candidate_id": self.candidate_id,
            "candidate_family": self.candidate_family,
            "decision": self.decision,
            "source_record_ref": self.source_record_ref,
            "target_record_ref": self.target_record_ref,
            "evidence_signal_ids": list(self.evidence_signal_ids),
            "confidence_label": self.confidence_label,
            "review_state": self.review_state,
            "promoted_to_fact": False,
            "import_eligible": False,
        }


@dataclass(frozen=True)
class ArticleLinksDedupManifest:
    paper_id: str
    run_id: str
    source_refs: tuple[dict[str, Any], ...] = ()
    page_index_refs: dict[str, Any] = field(default_factory=dict)
    citation_links: tuple[CitationLinkRecord, ...] = ()
    structural_links: tuple[StructuralLinkRecord, ...] = ()
    metadata_signals: tuple[MetadataSignalRecord, ...] = ()
    dedup_candidates: tuple[DedupCandidateRecord, ...] = ()
    diagnostics: tuple[ArticleLinksDedupDiagnostic, ...] = ()

    def to_redacted_dict(self) -> dict[str, Any]:
        payload = {
            "schema_version": ARTICLE_LINKS_DEDUP_SCHEMA_VERSION,
            "paper_id": self.paper_id,
            "run_id": self.run_id,
            "source_refs": [dict(source) for source in self.source_refs],
            "page_index_refs": dict(self.page_index_refs),
            "citation_links": [record.to_redacted_dict() for record in self.citation_links],
            "structural_links": [record.to_redacted_dict() for record in self.structural_links],
            "metadata_signals": [record.to_redacted_dict() for record in self.metadata_signals],
            "dedup_candidates": [record.to_redacted_dict() for record in self.dedup_candidates],
            "diagnostics": [diagnostic.to_redacted_dict() for diagnostic in self.diagnostics],
        }
        payload["summary"] = summarize_article_links_dedup(payload)
        payload["bridge_subtree"] = default_bridge_subtree(payload["summary"]["diagnostic_counts"])
        payload["safety_flags"] = default_safety_flags()
        payload["import_eligible_count"] = 0
        payload["promoted_to_fact_count"] = 0
        return payload


def default_safety_flags() -> dict[str, bool]:
    """Return fail-closed safety flags for this review-only contract."""
    return {
        "metadata_only": True,
        "review_only": True,
        "trusted_kg_import_allowed": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "model_outputs_included": False,
        "raw_payloads_included": False,
    }


def default_bridge_subtree(diagnostic_counts: dict[str, int] | None = None) -> dict[str, Any]:
    blocked = any((diagnostic_counts or {}).get(key, 0) for key in DIAGNOSTIC_COUNTER_KEYS)
    return {
        "status": "blocked_review_only_not_import_eligible" if blocked else "review_only_not_import_eligible",
        "source_slice": "M024-0xjwh9/S05",
        "graph_import_claim": False,
        "trusted_kg_import_allowed": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "raw_payloads_included": False,
    }


def normalize_doi(value: str) -> str:
    """Normalize DOI metadata without preserving resolver URL prefixes."""
    normalized = value.strip().lower()
    normalized = re.sub(r"^https?://(dx\.)?doi\.org/", "", normalized)
    normalized = re.sub(r"^doi:\s*", "", normalized)
    return normalized.strip()


def normalize_arxiv_id(value: str) -> str:
    """Normalize arXiv IDs while preserving the work version signal."""
    normalized = value.strip().lower()
    normalized = re.sub(r"^https?://arxiv\.org/(abs|pdf)/", "", normalized)
    normalized = re.sub(r"\.pdf$", "", normalized)
    normalized = re.sub(r"^arxiv:\s*", "", normalized)
    return normalized.strip()


def normalize_url(value: str) -> str:
    """Canonicalize URL signals without query strings or fragments."""
    parsed = urlsplit(value.strip())
    scheme = parsed.scheme.lower() or "https"
    netloc = parsed.netloc.lower()
    path = parsed.path.rstrip("/") if parsed.path != "/" else ""
    return urlunsplit((scheme, netloc, path, "", ""))


def normalize_hash_signal(value: str) -> str:
    """Normalize SHA-256-style metadata hash signals."""
    normalized = value.strip().lower()
    if normalized.startswith("sha256:"):
        normalized = normalized.split(":", 1)[1]
    return normalized


def title_author_year_hash(*, title_hash: str | None = None, author_hashes: list[str] | None = None, year: int | str | None = None) -> str:
    """Build a deterministic hash signal from already-redacted title/author/year material."""
    payload = json.dumps(
        {"title_hash": title_hash, "author_hashes": sorted(author_hashes or []), "year": str(year) if year is not None else None},
        sort_keys=True,
    )
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def deterministic_id(prefix: str, *parts: Any, length: int = 16) -> str:
    """Return a stable ID from canonical non-payload fields."""
    payload = json.dumps([part for part in parts], sort_keys=True, separators=(",", ":"))
    return f"{prefix}:{hashlib.sha256(payload.encode('utf-8')).hexdigest()[:length]}"


def build_article_links_dedup_manifest(manifest: dict[str, Any]) -> dict[str, Any]:
    """Return a fail-closed, redacted manifest with rebuilt counters/diagnostics."""
    if not isinstance(manifest, dict):
        raise TypeError("manifest must be a dictionary")
    original = manifest
    pre_diagnostics = (
        _scan_forbidden_payload_keys(original)
        + _scan_unsafe_true_flags(original)
        + _scan_url_query_token_signals(original)
    )
    payload = _sanitize_value(original)
    if not isinstance(payload, dict):
        raise TypeError("sanitized manifest must be a dictionary")

    payload["schema_version"] = ARTICLE_LINKS_DEDUP_SCHEMA_VERSION
    payload.setdefault("paper_id", "unknown-paper")
    payload.setdefault("run_id", "unknown-run")
    payload["source_refs"] = _list_of_dicts(payload.get("source_refs"))
    payload["page_index_refs"] = payload.get("page_index_refs") if isinstance(payload.get("page_index_refs"), dict) else {}
    payload["citation_links"] = _normalize_records(_list_of_dicts(payload.get("citation_links")), family="citation")
    payload["structural_links"] = _normalize_records(_list_of_dicts(payload.get("structural_links")), family="structural")
    payload["metadata_signals"] = [_normalize_metadata_signal(signal, index) for index, signal in enumerate(_list_of_dicts(payload.get("metadata_signals")))]
    payload["dedup_candidates"] = [_normalize_dedup_candidate(candidate) for candidate in _list_of_dicts(payload.get("dedup_candidates"))]

    diagnostics = pre_diagnostics + _validate_payload(payload)
    payload["diagnostics"] = _unique_diagnostics([*(_list_of_dicts(payload.get("diagnostics"))), *diagnostics])
    payload["summary"] = summarize_article_links_dedup(payload)
    payload["bridge_subtree"] = default_bridge_subtree(payload["summary"]["diagnostic_counts"])
    payload["safety_flags"] = default_safety_flags()
    payload["import_eligible_count"] = 0
    payload["promoted_to_fact_count"] = 0
    return payload


def validate_article_links_dedup_manifest(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    """Return redacted diagnostics for manifest invariants without raising."""
    if not isinstance(manifest, dict):
        return [_diagnostic("malformed_manifest", "/").to_redacted_dict()]
    return _validate_payload(manifest)


def summarize_article_links_dedup(manifest: dict[str, Any]) -> dict[str, Any]:
    """Build deterministic benchmark counters for link/dedup coverage."""
    citation_links = _list_of_dicts(manifest.get("citation_links"))
    structural_links = _list_of_dicts(manifest.get("structural_links"))
    metadata_signals = _list_of_dicts(manifest.get("metadata_signals"))
    dedup_candidates = _list_of_dicts(manifest.get("dedup_candidates"))
    diagnostics = _list_of_dicts(manifest.get("diagnostics"))
    anchor_ids = set(_string_list((manifest.get("page_index_refs") or {}).get("anchor_ids") if isinstance(manifest.get("page_index_refs"), dict) else []))
    signal_by_id = {signal.get("signal_id"): signal for signal in metadata_signals if isinstance(signal.get("signal_id"), str)}

    required_anchors: list[str | None] = []
    required_spans: list[str | None] = []
    for link in citation_links + structural_links:
        required_anchors.append(_string_or_none(link.get("source_page_index_anchor_id")))
        required_spans.extend(_string_list(link.get("source_span_ids")))
    for signal in metadata_signals:
        required_anchors.append(_string_or_none(signal.get("source_page_index_anchor_id")))
        required_spans.append(_string_or_none(signal.get("source_span_id")))
    if citation_links or structural_links or metadata_signals or dedup_candidates:
        # One manifest-level bridge anchor/span coverage unit captures the bundle
        # itself for benchmark parity with the S05 fixture contract.
        required_anchors.append(_first_covered_anchor(metadata_signals, anchor_ids))
        required_spans.append(_first_span(metadata_signals))

    missing_anchor_count = sum(1 for anchor in required_anchors if not anchor or (anchor_ids and anchor not in anchor_ids))
    missing_span_count = sum(1 for span in required_spans if not span)
    diagnostic_counts = _diagnostic_counts(diagnostics)
    # Coverage counters are derived from references, while diagnostic counters are
    # derived from validation codes.  Include missing diagnostics if supplied by
    # prebuilt fixtures so stored summaries remain benchmarkable.
    missing_anchor_count = max(missing_anchor_count, diagnostic_counts.get("missing_page_index_anchor_count", 0))
    missing_span_count = max(missing_span_count, _count_codes(diagnostics, {"missing_source_span"}))

    return {
        "link_family_counts": {
            "citation": len(citation_links),
            "structural": len(structural_links),
            "metadata_signal": len(metadata_signals),
            "dedup_candidate": len(dedup_candidates),
        },
        "page_index_anchor_coverage": {
            "required_anchor_ref_count": len(required_anchors),
            "covered_anchor_ref_count": max(0, len(required_anchors) - missing_anchor_count),
            "missing_anchor_ref_count": missing_anchor_count,
        },
        "source_span_coverage": {
            "required_source_span_ref_count": len(required_spans),
            "covered_source_span_ref_count": max(0, len(required_spans) - missing_span_count),
            "missing_source_span_ref_count": missing_span_count,
        },
        "metadata_signal_counts": _counts(signal.get("signal_type") for signal in metadata_signals),
        "dedup_decision_counts": _counts(candidate.get("decision") for candidate in dedup_candidates),
        "diagnostic_counts": diagnostic_counts,
        "import_eligible_count": 0,
    }


def to_redacted_dict(value: ArticleLinksDedupManifest | dict[str, Any]) -> dict[str, Any]:
    """Convert a manifest object or mapping to a redacted dictionary."""
    if hasattr(value, "to_redacted_dict"):
        return value.to_redacted_dict()  # type: ignore[no-any-return]
    return build_article_links_dedup_manifest(dict(value))


def to_json(value: ArticleLinksDedupManifest | dict[str, Any]) -> str:
    """Serialize a link/dedup manifest deterministically."""
    return json.dumps(to_redacted_dict(value), indent=2, sort_keys=True) + "\n"


def _normalize_records(records: list[dict[str, Any]], *, family: str) -> list[dict[str, Any]]:
    normalized: list[dict[str, Any]] = []
    for record in records:
        item = dict(record)
        item["link_family"] = family
        item["promoted_to_fact"] = False
        item["import_eligible"] = False
        normalized.append(item)
    return normalized


def _normalize_metadata_signal(signal: dict[str, Any], index: int) -> dict[str, Any]:
    item = dict(signal)
    signal_type = item.get("signal_type")
    value = item.get("normalized_value")
    if isinstance(value, str):
        if signal_type == "doi":
            item["normalized_value"] = normalize_doi(value)
        elif signal_type == "arxiv_id":
            item["normalized_value"] = normalize_arxiv_id(value)
        elif signal_type == "url":
            item["normalized_value"] = normalize_url(value)
        elif signal_type in {"content_hash", "title_author_year_hash"}:
            item["normalized_value"] = normalize_hash_signal(value)
    item["promoted_to_fact"] = False
    item["import_eligible"] = False
    return item


def _normalize_dedup_candidate(candidate: dict[str, Any]) -> dict[str, Any]:
    item = dict(candidate)
    item["promoted_to_fact"] = False
    item["import_eligible"] = False
    return item


def _validate_payload(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[ArticleLinksDedupDiagnostic] = []
    if manifest.get("schema_version") != ARTICLE_LINKS_DEDUP_SCHEMA_VERSION:
        diagnostics.append(_diagnostic("invalid_schema_version", "/schema_version"))
    diagnostics.extend(_required(manifest, ("schema_version", "paper_id", "run_id", "source_refs", "page_index_refs"), ""))

    source_refs = _list_of_dicts(manifest.get("source_refs"))
    page_index_refs = manifest.get("page_index_refs") if isinstance(manifest.get("page_index_refs"), dict) else {}
    anchor_ids = set(_string_list(page_index_refs.get("anchor_ids")))
    manifest_span_ids = set(_collect_source_span_ids(manifest))

    for index, source in enumerate(source_refs):
        sha = source.get("sha256")
        if sha is not None and not _valid_sha256(sha):
            diagnostics.append(_diagnostic("malformed_source_ref", f"/source_refs[{index}]/sha256", _string_or_none(source.get("source_id"))))

    diagnostics.extend(_duplicate_id_diagnostics(_list_of_dicts(manifest.get("citation_links")), "link_id", "/citation_links"))
    diagnostics.extend(_duplicate_id_diagnostics(_list_of_dicts(manifest.get("structural_links")), "link_id", "/structural_links"))
    diagnostics.extend(_duplicate_id_diagnostics(_list_of_dicts(manifest.get("metadata_signals")), "signal_id", "/metadata_signals"))
    diagnostics.extend(_duplicate_id_diagnostics(_list_of_dicts(manifest.get("dedup_candidates")), "candidate_id", "/dedup_candidates"))

    signal_by_id = {signal.get("signal_id"): signal for signal in _list_of_dicts(manifest.get("metadata_signals")) if isinstance(signal.get("signal_id"), str)}

    for index, link in enumerate(_list_of_dicts(manifest.get("citation_links"))):
        object_id = _string_or_none(link.get("link_id"))
        diagnostics.extend(_validate_review_state(link, f"/citation_links[{index}]", object_id))
        _validate_anchor(link.get("source_page_index_anchor_id"), anchor_ids, f"/citation_links[{index}]/source_page_index_anchor_id", object_id, diagnostics)
        for span_index, span_id in enumerate(_string_list(link.get("source_span_ids"))):
            _validate_span_ref(span_id, manifest_span_ids, f"/citation_links[{index}]/source_span_ids[{span_index}]", object_id, diagnostics)
    for index, link in enumerate(_list_of_dicts(manifest.get("structural_links"))):
        object_id = _string_or_none(link.get("link_id"))
        if link.get("relationship") not in ALLOWED_STRUCTURAL_RELATIONSHIPS:
            diagnostics.append(_diagnostic("unsupported_structural_relationship", f"/structural_links[{index}]/relationship", object_id))
        diagnostics.extend(_validate_review_state(link, f"/structural_links[{index}]", object_id))
        _validate_anchor(link.get("source_page_index_anchor_id"), anchor_ids, f"/structural_links[{index}]/source_page_index_anchor_id", object_id, diagnostics)
        for span_index, span_id in enumerate(_string_list(link.get("source_span_ids"))):
            _validate_span_ref(span_id, manifest_span_ids, f"/structural_links[{index}]/source_span_ids[{span_index}]", object_id, diagnostics)
    for index, signal in enumerate(_list_of_dicts(manifest.get("metadata_signals"))):
        object_id = _string_or_none(signal.get("signal_id"))
        if signal.get("signal_type") not in ALLOWED_METADATA_SIGNAL_TYPES:
            diagnostics.append(_diagnostic("unsupported_metadata_signal_type", f"/metadata_signals[{index}]/signal_type", object_id))
        diagnostics.extend(_validate_review_state(signal, f"/metadata_signals[{index}]", object_id))
        _validate_anchor(signal.get("source_page_index_anchor_id"), anchor_ids, f"/metadata_signals[{index}]/source_page_index_anchor_id", object_id, diagnostics)
        _validate_span_ref(_string_or_none(signal.get("source_span_id")), manifest_span_ids, f"/metadata_signals[{index}]/source_span_id", object_id, diagnostics)
        if signal.get("signal_type") == "url" and isinstance(signal.get("normalized_value"), str):
            raw_value = signal["normalized_value"]
            if "?" in raw_value or "#" in raw_value:
                diagnostics.append(_diagnostic("url_query_tokens_removed", f"/metadata_signals[{index}]/normalized_value", object_id, severity="warning", blocks_import=False))
    for index, candidate in enumerate(_list_of_dicts(manifest.get("dedup_candidates"))):
        object_id = _string_or_none(candidate.get("candidate_id"))
        if candidate.get("candidate_family") not in ALLOWED_CANDIDATE_FAMILIES:
            diagnostics.append(_diagnostic("unsupported_candidate_family", f"/dedup_candidates[{index}]/candidate_family", object_id))
        if candidate.get("decision") not in ALLOWED_DEDUP_DECISIONS:
            diagnostics.append(_diagnostic("unsupported_dedup_decision", f"/dedup_candidates[{index}]/decision", object_id))
        diagnostics.extend(_validate_review_state(candidate, f"/dedup_candidates[{index}]", object_id))
        if candidate.get("confidence_label") not in ALLOWED_CONFIDENCE_LABELS:
            diagnostics.append(_diagnostic("unsupported_confidence_label", f"/dedup_candidates[{index}]/confidence_label", object_id))
        if candidate.get("decision") == "conflicting_metadata_review_required" or _candidate_has_conflicting_signals(candidate, signal_by_id):
            diagnostics.append(_diagnostic("conflicting_metadata_signals", f"/dedup_candidates[{index}]/evidence_signal_ids", object_id))
        if candidate.get("decision") == "insufficient_metadata_review_required" or not _string_list(candidate.get("evidence_signal_ids")):
            diagnostics.append(_diagnostic("insufficient_metadata_for_dedup", f"/dedup_candidates[{index}]/evidence_signal_ids", object_id))

    diagnostics.extend(_scan_forbidden_payload_keys(manifest))
    diagnostics.extend(_scan_unsafe_true_flags(manifest))
    return _unique_diagnostics([diagnostic.to_redacted_dict() for diagnostic in diagnostics])


def _validate_review_state(record: dict[str, Any], path: str, object_id: str | None) -> list[ArticleLinksDedupDiagnostic]:
    if record.get("review_state") not in ALLOWED_REVIEW_STATES:
        return [_diagnostic("unsupported_review_state", f"{path}/review_state", object_id)]
    return []


def _validate_anchor(value: Any, anchor_ids: set[str], path: str, object_id: str | None, diagnostics: list[ArticleLinksDedupDiagnostic]) -> None:
    anchor = _string_or_none(value)
    if not anchor or (anchor_ids and anchor not in anchor_ids):
        diagnostics.append(_diagnostic("missing_page_index_anchor", path, object_id))


def _validate_span_ref(value: Any, known_span_ids: set[str], path: str, object_id: str | None, diagnostics: list[ArticleLinksDedupDiagnostic]) -> None:
    span_id = _string_or_none(value)
    if not span_id or (known_span_ids and span_id not in known_span_ids):
        diagnostics.append(_diagnostic("missing_source_span", path, object_id, severity="warning"))


def _candidate_has_conflicting_signals(candidate: dict[str, Any], signal_by_id: dict[Any, dict[str, Any]]) -> bool:
    values_by_type: dict[str, set[str]] = {}
    for signal_id in _string_list(candidate.get("evidence_signal_ids")):
        signal = signal_by_id.get(signal_id)
        if not isinstance(signal, dict):
            continue
        signal_type = _string_or_none(signal.get("signal_type"))
        value = _string_or_none(signal.get("normalized_value"))
        if signal_type and value:
            values_by_type.setdefault(signal_type, set()).add(value)
    return any(len(values) > 1 for values in values_by_type.values())


def _collect_source_span_ids(manifest: dict[str, Any]) -> list[str]:
    spans: list[str] = []
    for link in _list_of_dicts(manifest.get("citation_links")) + _list_of_dicts(manifest.get("structural_links")):
        spans.extend(_string_list(link.get("source_span_ids")))
    for signal in _list_of_dicts(manifest.get("metadata_signals")):
        span = _string_or_none(signal.get("source_span_id"))
        if span:
            spans.append(span)
    return spans


def _scan_forbidden_payload_keys(value: Any, path: str = "", object_id: str | None = None) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    if isinstance(value, dict):
        current_object_id = object_id or _string_or_none(value.get("link_id") or value.get("signal_id") or value.get("candidate_id") or value.get("source_id") or value.get("paper_id"))
        for key, child in value.items():
            child_path = f"{path}/{key}" if path else f"/{key}"
            if key in FORBIDDEN_PAYLOAD_KEYS:
                diagnostics.append(_diagnostic("forbidden_payload_key", child_path, current_object_id).to_redacted_dict())
                continue
            diagnostics.extend(_scan_forbidden_payload_keys(child, child_path, current_object_id))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            diagnostics.extend(_scan_forbidden_payload_keys(child, f"{path}[{index}]", object_id))
    return diagnostics


def _scan_unsafe_true_flags(value: Any, path: str = "", object_id: str | None = None) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    if isinstance(value, dict):
        current_object_id = object_id or _string_or_none(value.get("link_id") or value.get("signal_id") or value.get("candidate_id") or value.get("source_id") or value.get("paper_id"))
        for key, child in value.items():
            child_path = f"{path}/{key}" if path else f"/{key}"
            if key in UNSAFE_FLAG_KEYS and child is True:
                diagnostics.append(_diagnostic(f"unsafe_import_flag_true:{key}", child_path, current_object_id).to_redacted_dict())
                continue
            diagnostics.extend(_scan_unsafe_true_flags(child, child_path, current_object_id))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            diagnostics.extend(_scan_unsafe_true_flags(child, f"{path}[{index}]", object_id))
    return diagnostics


def _scan_url_query_token_signals(value: Any) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    if not isinstance(value, dict):
        return diagnostics
    for index, signal in enumerate(_list_of_dicts(value.get("metadata_signals"))):
        if signal.get("signal_type") != "url" or not isinstance(signal.get("normalized_value"), str):
            continue
        raw_value = signal["normalized_value"]
        if "?" in raw_value or "#" in raw_value:
            diagnostics.append(
                _diagnostic(
                    "url_query_tokens_removed",
                    f"/metadata_signals[{index}]/normalized_value",
                    _string_or_none(signal.get("signal_id")),
                    severity="warning",
                    blocks_import=False,
                ).to_redacted_dict()
            )
    return diagnostics


def _sanitize_value(value: Any) -> Any:
    if isinstance(value, dict):
        sanitized: dict[str, Any] = {}
        for key, child in value.items():
            if key in FORBIDDEN_PAYLOAD_KEYS:
                continue
            if key in UNSAFE_FLAG_KEYS:
                sanitized[key] = False
                continue
            sanitized[key] = _sanitize_value(child)
        return sanitized
    if isinstance(value, list):
        return [_sanitize_value(child) for child in value]
    return value


def _diagnostic_counts(diagnostics: list[dict[str, Any]]) -> dict[str, int]:
    counts = {key: 0 for key in DIAGNOSTIC_COUNTER_KEYS}
    for diagnostic in diagnostics:
        code = str(diagnostic.get("code", ""))
        if code == "duplicate_id":
            counts["duplicate_id_count"] += 1
        elif code == "malformed_source_ref":
            counts["malformed_source_ref_count"] += 1
        elif code == "missing_page_index_anchor":
            counts["missing_page_index_anchor_count"] += 1
        elif code.startswith("unsupported_"):
            counts["bad_vocabulary_count"] += 1
        elif code == "conflicting_metadata_signals":
            counts["conflict_count"] += 1
        elif code == "insufficient_metadata_for_dedup":
            counts["insufficient_metadata_count"] += 1
        elif code == "forbidden_payload_key":
            counts["forbidden_payload_detection_count"] += 1
        elif code.startswith("unsafe_import_flag_true:"):
            counts["unsafe_authorization_count"] += 1
    return counts


def _count_codes(diagnostics: list[dict[str, Any]], codes: set[str]) -> int:
    return sum(1 for diagnostic in diagnostics if diagnostic.get("code") in codes)


def _duplicate_id_diagnostics(values: list[dict[str, Any]], field_name: str, path: str) -> list[ArticleLinksDedupDiagnostic]:
    diagnostics: list[ArticleLinksDedupDiagnostic] = []
    seen: set[str] = set()
    for index, value in enumerate(values):
        identifier = value.get(field_name)
        if not isinstance(identifier, str) or not identifier:
            continue
        if identifier in seen:
            diagnostics.append(_diagnostic("duplicate_id", f"{path}[{index}]/{field_name}", identifier))
        else:
            seen.add(identifier)
    return diagnostics


def _required(value: dict[str, Any], fields: tuple[str, ...], path: str) -> list[ArticleLinksDedupDiagnostic]:
    diagnostics: list[ArticleLinksDedupDiagnostic] = []
    for field_name in fields:
        if field_name not in value or value.get(field_name) is None:
            diagnostics.append(_diagnostic(f"missing_{field_name}", f"{path}/{field_name}" if path else f"/{field_name}"))
    return diagnostics


def _unique_diagnostics(diagnostics: list[dict[str, Any]]) -> list[dict[str, Any]]:
    unique: list[dict[str, Any]] = []
    seen: set[tuple[str, str, str | None]] = set()
    for diagnostic in diagnostics:
        if not isinstance(diagnostic, dict):
            continue
        code = _string_or_none(diagnostic.get("code"))
        json_path = _string_or_none(diagnostic.get("json_path"))
        object_id = _string_or_none(diagnostic.get("object_id"))
        if not code or not json_path:
            continue
        key = (code, json_path, object_id)
        if key in seen:
            continue
        seen.add(key)
        unique.append(
            {
                "code": code,
                "json_path": json_path,
                "object_id": object_id,
            }
        )
    return unique


def _first_covered_anchor(metadata_signals: list[dict[str, Any]], anchor_ids: set[str]) -> str | None:
    for signal in metadata_signals:
        anchor = _string_or_none(signal.get("source_page_index_anchor_id"))
        if anchor and (not anchor_ids or anchor in anchor_ids):
            return anchor
    return None


def _first_span(metadata_signals: list[dict[str, Any]]) -> str | None:
    for signal in metadata_signals:
        span = _string_or_none(signal.get("source_span_id"))
        if span:
            return span
    return None


def _diagnostic(code: str, json_path: str, object_id: str | None = None, *, severity: DiagnosticSeverity = "repair_required", blocks_import: bool = True) -> ArticleLinksDedupDiagnostic:
    return ArticleLinksDedupDiagnostic(code=code, json_path=json_path, object_id=object_id, severity=severity, blocks_import=blocks_import)


def _valid_sha256(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(character in "0123456789abcdef" for character in value.lower())


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _string_list(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [str(item) for item in value if item is not None]


def _string_or_none(value: Any) -> str | None:
    return str(value) if value is not None else None


def _counts(values: Any) -> dict[str, int]:
    counts: dict[str, int] = {}
    for value in values:
        if value is None:
            continue
        key = str(value)
        counts[key] = counts.get(key, 0) + 1
    return dict(sorted(counts.items()))


__all__ = [
    "ALLOWED_CANDIDATE_FAMILIES",
    "ALLOWED_DEDUP_DECISIONS",
    "ALLOWED_METADATA_SIGNAL_TYPES",
    "ALLOWED_REVIEW_STATES",
    "ALLOWED_STRUCTURAL_RELATIONSHIPS",
    "ARTICLE_LINKS_DEDUP_SCHEMA_VERSION",
    "ArticleLinksDedupDiagnostic",
    "ArticleLinksDedupManifest",
    "CitationLinkRecord",
    "DedupCandidateRecord",
    "MetadataSignalRecord",
    "StructuralLinkRecord",
    "build_article_links_dedup_manifest",
    "default_bridge_subtree",
    "default_safety_flags",
    "deterministic_id",
    "normalize_arxiv_id",
    "normalize_doi",
    "normalize_hash_signal",
    "normalize_url",
    "summarize_article_links_dedup",
    "title_author_year_hash",
    "to_json",
    "to_redacted_dict",
    "validate_article_links_dedup_manifest",
]
