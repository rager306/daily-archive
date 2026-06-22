"""Pre-KG article artifact detection contract.

This module defines redacted, JSON-native records for structural and scientific
article artifacts before any trusted KG import. The contract stores identifiers,
coordinates, hashes, review states, link vocabulary, and diagnostics only. It
must not store raw paper text, model output, binary payloads, embeddings,
vectors, secrets, optimizer traces, or LadybugDB write state.

Formerly: src/arxiv_archive/article_artifacts.py
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any, Literal

from research_graph.workflows.universal_kb.contracts import SafetyFlags

ARTICLE_ARTIFACT_SCHEMA_VERSION = "m023-article-artifacts.v1"
ARTICLE_ARTIFACT_RUN_SCHEMA_VERSION = "m023-article-artifact-run.v1"
ARTICLE_ARTIFACT_DIAGNOSTICS_SCHEMA_VERSION = "m023-article-artifact-diagnostics.v1"
TRUSTED_IMPORT_USE = "trusted_kg_import"

ArtifactType = Literal[
    "figure",
    "table",
    "equation",
    "reference",
    "dataset",
    "code",
    "method",
    "metric",
    "claim",
    "section",
    "scientific_term",
    "experiment",
]
CandidateLinkType = Literal[
    "supports",
    "refutes",
    "mentions",
    "defines",
    "uses",
    "measures",
    "reports",
    "cites",
    "contains",
    "derived_from",
    "located_in",
    "candidate_for",
]
ReviewState = Literal[
    "detected_unreviewed",
    "review_required",
    "accepted",
    "rejected",
    "ambiguous",
    "repair_required",
    "retrieval_only",
]
DiagnosticSeverity = Literal["info", "warning", "repair_required", "error"]
CoordinateSpace = Literal[
    "normalized_markdown_char",
    "semantic_chunk_char",
    "page_bbox",
    "artifact_record",
]

ALLOWED_ARTIFACT_TYPES = frozenset(ArtifactType.__args__)  # type: ignore[attr-defined]
ALLOWED_CANDIDATE_LINK_TYPES = frozenset(CandidateLinkType.__args__)  # type: ignore[attr-defined]
ALLOWED_REVIEW_STATES = frozenset(ReviewState.__args__)  # type: ignore[attr-defined]
ALLOWED_COORDINATE_SPACES = frozenset(CoordinateSpace.__args__)  # type: ignore[attr-defined]
ALLOWED_SEVERITIES = frozenset(DiagnosticSeverity.__args__)  # type: ignore[attr-defined]

ALLOWED_USES = ("artifact_review", "candidate_link_review", "provenance_diagnostics")
EXCLUDED_USES = (
    TRUSTED_IMPORT_USE,
    "production_ladybugdb_write",
    "embedding_generation",
    "source_of_truth_claim",
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
    {
        "source_of_truth",
        "source_of_truth_claim",
        "truth_source",
        "canonical_source",
        "minimax_source_of_truth",
    }
)


def default_safety_flags() -> dict[str, bool]:
    """Return required false safety flags for pre-KG artifact records.

    M035 centralizes the graph/import/write flags in `SafetyFlags`. The
    article-artifact contract keeps its existing redaction and payload flags in
    the same dictionary so older diagnostics retain stable key names.
    """
    return {
        **SafetyFlags().to_dict(),
        "trusted_kg_import_allowed": False,
        "raw_text_included": False,
        "chunk_text_included": False,
        "raw_binary_included": False,
        "base64_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "optimizer_traces_included": False,
        "model_outputs_included": False,
    }


@dataclass(frozen=True)
class ArticleArtifactDiagnostic:
    """One stable, redacted diagnostic for artifact contract validation."""

    code: str
    json_path: str
    severity: DiagnosticSeverity = "repair_required"
    object_id: str | None = None
    message: str = "Artifact contract diagnostic; inspect code and JSON path, not source content."
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
class SourceSpan:
    """Coordinate pointer into a source artifact without carrying content."""

    span_id: str
    source_id: str
    coordinate_space: CoordinateSpace
    char_start: int | None = None
    char_end: int | None = None
    page_start: int | None = None
    page_end: int | None = None
    bbox: tuple[float, float, float, float] | None = None
    span_hash: str | None = None

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "span_id": self.span_id,
            "source_id": self.source_id,
            "coordinate_space": self.coordinate_space,
            "char_start": self.char_start,
            "char_end": self.char_end,
            "page_start": self.page_start,
            "page_end": self.page_end,
            "bbox": list(self.bbox) if self.bbox is not None else None,
            "span_hash": self.span_hash,
            "raw_text_embedded": False,
        }


@dataclass(frozen=True)
class SourceReference:
    """Redacted source reference used by detected artifacts."""

    source_id: str
    paper_id: str
    source_role: str
    source_path: str | None = None
    sha256: str | None = None
    media_type: str | None = None
    conversion_status: str = "review_required"

    @classmethod
    def from_loader_result(
        cls,
        result: Any,
        *,
        paper_id: str | None = None,
        source_role: str | None = None,
        conversion_status: str | None = None,
    ) -> SourceReference:
        """Build a redacted source reference from loader provenance metadata."""
        resolved_paper_id = paper_id or getattr(result, "paper_id", None)
        if not resolved_paper_id:
            raise ValueError(
                "paper_id is required to build a source reference from loader provenance"
            )
        return cls(
            source_id=str(result.source_id),
            paper_id=str(resolved_paper_id),
            source_role=source_role or str(getattr(result, "source_type", "article_source")),
            source_path=str(result.source_path)
            if getattr(result, "source_path", None) is not None
            else None,
            sha256=getattr(result, "sha256", None),
            media_type=getattr(result, "media_type", None),
            conversion_status=conversion_status
            or str(getattr(result, "outcome", "review_required")),
        )

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "source_id": self.source_id,
            "paper_id": self.paper_id,
            "source_role": self.source_role,
            "source_path": self.source_path,
            "sha256": self.sha256,
            "media_type": self.media_type,
            "conversion_status": self.conversion_status,
            "raw_text_embedded": False,
            "raw_binary_embedded": False,
        }


@dataclass(frozen=True)
class SectionLineage:
    """Section ancestry by IDs/ordinals only; no section title or body text."""

    section_id: str
    parent_section_id: str | None = None
    section_type: str | None = None
    ordinal_path: tuple[int, ...] = ()
    source_span: SourceSpan | None = None

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "section_id": self.section_id,
            "parent_section_id": self.parent_section_id,
            "section_type": self.section_type,
            "ordinal_path": list(self.ordinal_path),
            "source_span": self.source_span.to_redacted_dict()
            if self.source_span is not None
            else None,
        }


@dataclass(frozen=True)
class CandidateLink:
    """Review-only relationship candidate between artifacts or external refs."""

    link_id: str
    source_artifact_id: str
    target_ref: str
    link_type: CandidateLinkType
    review_state: ReviewState = "review_required"
    source_spans: tuple[SourceSpan, ...] = ()
    confidence_label: str = "unknown"
    diagnostic_codes: tuple[str, ...] = ()

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "link_id": self.link_id,
            "source_artifact_id": self.source_artifact_id,
            "target_ref": self.target_ref,
            "link_type": self.link_type,
            "review_state": self.review_state,
            "source_spans": [span.to_redacted_dict() for span in self.source_spans],
            "confidence_label": self.confidence_label,
            "diagnostic_codes": list(self.diagnostic_codes),
            "allowed_uses": list(ALLOWED_USES),
            "excluded_uses": list(EXCLUDED_USES),
            "promoted_to_fact": False,
            "import_eligible": False,
        }


@dataclass(frozen=True)
class ArticleArtifactRecord:
    """One structural or scientific article artifact before KG promotion."""

    artifact_id: str
    paper_id: str
    artifact_type: ArtifactType
    review_state: ReviewState = "detected_unreviewed"
    source_refs: tuple[SourceReference, ...] = ()
    source_spans: tuple[SourceSpan, ...] = ()
    section_lineage: SectionLineage | None = None
    candidate_links: tuple[CandidateLink, ...] = ()
    confidence_label: str = "unknown"
    detector: str = "pre_kg_artifact_scaffold"
    diagnostic_codes: tuple[str, ...] = ()
    metadata: dict[str, Any] = field(default_factory=dict)

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "artifact_id": self.artifact_id,
            "paper_id": self.paper_id,
            "artifact_type": self.artifact_type,
            "review_state": self.review_state,
            "source_refs": [source.to_redacted_dict() for source in self.source_refs],
            "source_spans": [span.to_redacted_dict() for span in self.source_spans],
            "section_lineage": self.section_lineage.to_redacted_dict()
            if self.section_lineage is not None
            else None,
            "candidate_links": [link.to_redacted_dict() for link in self.candidate_links],
            "confidence_label": self.confidence_label,
            "detector": self.detector,
            "diagnostic_codes": list(self.diagnostic_codes),
            "metadata": dict(self.metadata),
            "safety_flags": default_safety_flags(),
            "allowed_uses": list(ALLOWED_USES),
            "excluded_uses": list(EXCLUDED_USES),
            "promoted_to_fact": False,
            "import_eligible": False,
        }


@dataclass(frozen=True)
class ArticleArtifactManifest:
    """Per-paper pre-KG article artifact manifest."""

    paper_id: str
    run_id: str
    artifacts: tuple[ArticleArtifactRecord, ...] = ()
    source_refs: tuple[SourceReference, ...] = ()
    diagnostics: tuple[ArticleArtifactDiagnostic, ...] = ()

    def to_redacted_dict(self) -> dict[str, Any]:
        artifacts = [artifact.to_redacted_dict() for artifact in self.artifacts]
        source_refs = [source.to_redacted_dict() for source in self.source_refs]
        summary = summarize_article_artifacts(artifacts)
        diagnostics = [diagnostic.to_redacted_dict() for diagnostic in self.diagnostics]
        return {
            "schema_version": ARTICLE_ARTIFACT_SCHEMA_VERSION,
            "run_id": self.run_id,
            "paper_id": self.paper_id,
            "source_refs": source_refs,
            "artifacts": artifacts,
            "summary": summary,
            "diagnostics": diagnostics,
            "safety_flags": default_safety_flags(),
            "allowed_uses": list(ALLOWED_USES),
            "excluded_uses": list(EXCLUDED_USES),
            "promoted_to_fact_count": 0,
            "import_eligible_count": 0,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        }


@dataclass(frozen=True)
class ArticleArtifactRunSummary:
    """Run-level summary for one or more article artifact manifests."""

    run_id: str
    manifests: tuple[dict[str, Any], ...]
    input_hashes: dict[str, str] = field(default_factory=dict)
    output_paths: dict[str, Any] = field(default_factory=dict)

    def to_redacted_dict(self) -> dict[str, Any]:
        artifact_count = sum(
            len(_list_of_dicts(manifest.get("artifacts"))) for manifest in self.manifests
        )
        diagnostic_count = sum(
            len(_list_of_dicts(manifest.get("diagnostics"))) for manifest in self.manifests
        )
        diagnostic_codes = sorted(
            {
                str(diagnostic.get("code"))
                for manifest in self.manifests
                for diagnostic in _list_of_dicts(manifest.get("diagnostics"))
                if isinstance(diagnostic.get("code"), str) and diagnostic.get("code")
            }
        )
        return {
            "schema_version": ARTICLE_ARTIFACT_RUN_SCHEMA_VERSION,
            "diagnostics_schema_version": ARTICLE_ARTIFACT_DIAGNOSTICS_SCHEMA_VERSION,
            "manifest_schema_version": ARTICLE_ARTIFACT_SCHEMA_VERSION,
            "run_id": self.run_id,
            "paper_count": len(self.manifests),
            "paper_ids": [
                str(manifest.get("paper_id"))
                for manifest in self.manifests
                if manifest.get("paper_id")
            ],
            "artifact_count": artifact_count,
            "diagnostic_count": diagnostic_count,
            "diagnostic_codes": diagnostic_codes,
            "artifact_counts_by_type": _merge_counts(
                manifest.get("summary", {}).get("artifact_counts_by_type", {})
                for manifest in self.manifests
            ),
            "review_state_counts": _merge_counts(
                manifest.get("summary", {}).get("review_state_counts", {})
                for manifest in self.manifests
            ),
            "candidate_link_type_counts": _merge_counts(
                manifest.get("summary", {}).get("candidate_link_type_counts", {})
                for manifest in self.manifests
            ),
            "promoted_to_fact_count": 0,
            "import_eligible_count": 0,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
            "safety_flags": default_safety_flags(),
            "input_hashes": dict(self.input_hashes),
            "output_paths": dict(self.output_paths),
        }


def build_article_artifact_run_diagnostics_artifact(
    *,
    run_id: str,
    manifests: tuple[dict[str, Any], ...],
    input_hashes: dict[str, str] | None = None,
    output_paths: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build a redacted diagnostics artifact for a deterministic detection run."""
    diagnostics = [
        dict(diagnostic)
        for manifest in manifests
        for diagnostic in _list_of_dicts(manifest.get("diagnostics"))
    ]
    return {
        "schema_version": ARTICLE_ARTIFACT_DIAGNOSTICS_SCHEMA_VERSION,
        "run_schema_version": ARTICLE_ARTIFACT_RUN_SCHEMA_VERSION,
        "manifest_schema_version": ARTICLE_ARTIFACT_SCHEMA_VERSION,
        "run_id": run_id,
        "paper_ids": [
            str(manifest.get("paper_id")) for manifest in manifests if manifest.get("paper_id")
        ],
        "diagnostic_count": len(diagnostics),
        "diagnostics": diagnostics,
        "diagnostic_counts_by_code": _counts(diagnostic.get("code") for diagnostic in diagnostics),
        "diagnostic_codes": sorted(
            {
                str(diagnostic.get("code"))
                for diagnostic in diagnostics
                if isinstance(diagnostic.get("code"), str) and diagnostic.get("code")
            }
        ),
        "manifest_diagnostic_summaries": {
            str(manifest.get("paper_id")): build_article_artifact_diagnostics_summary(manifest)
            for manifest in manifests
            if manifest.get("paper_id")
        },
        "input_hashes": dict(input_hashes or {}),
        "output_paths": dict(output_paths or {}),
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "promoted_to_fact_count": 0,
        "import_eligible_count": 0,
        "safety_flags": default_safety_flags(),
    }


def summarize_article_artifacts(artifacts: list[dict[str, Any]]) -> dict[str, Any]:
    """Build a redacted summary from serialized artifact records."""
    links = [
        link for artifact in artifacts for link in _list_of_dicts(artifact.get("candidate_links"))
    ]
    return {
        "artifact_count": len(artifacts),
        "candidate_link_count": len(links),
        "artifact_counts_by_type": _counts(artifact.get("artifact_type") for artifact in artifacts),
        "review_state_counts": _counts(artifact.get("review_state") for artifact in artifacts),
        "candidate_link_type_counts": _counts(link.get("link_type") for link in links),
        "candidate_link_review_state_counts": _counts(link.get("review_state") for link in links),
        "repair_required_count": sum(
            1 for artifact in artifacts if artifact.get("review_state") == "repair_required"
        ),
        "ambiguous_count": sum(
            1 for artifact in artifacts if artifact.get("review_state") == "ambiguous"
        ),
        "promoted_to_fact_count": 0,
        "import_eligible_count": 0,
        "safety_flags": default_safety_flags(),
    }


REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION = "m023-redacted-article-structure.v1"
DETERMINISTIC_FIXTURE_DETECTOR = "redacted_fixture_v1"
DEFAULT_DETERMINISTIC_RUN_ID = "m023-s02-deterministic-fixture-run"


def build_article_artifact_manifest_from_structure(
    structure: dict[str, Any], *, run_id: str = DEFAULT_DETERMINISTIC_RUN_ID
) -> dict[str, Any]:
    """Generate review-only artifact candidates from a redacted structure fixture.

    The detector only consumes explicit IDs, coordinates, hashes, section lineage,
    caption/citation placeholders, and structured marker records already present
    in the fixture. It never derives artifacts from raw prose or model output.
    """
    _validate_redacted_structure_boundary(structure)
    paper_id = str(structure["paper_id"])
    source_refs = tuple(
        _source_ref_from_structure(source, paper_id)
        for source in _list_of_dicts(structure.get("source_refs"))
    )
    spans = {
        _string_or_none(span.get("span_id")): _span_from_structure(span)
        for span in _list_of_dicts(structure.get("safe_spans"))
    }
    spans.pop(None, None)
    sections = _list_of_dicts(structure.get("sections"))
    section_by_id = {
        section.get("section_id"): section
        for section in sections
        if isinstance(section.get("section_id"), str)
    }
    placeholders = _list_of_dicts(structure.get("artifact_placeholders"))
    markers = _list_of_dicts(structure.get("structured_markers")) + _list_of_dicts(
        structure.get("scientific_markers")
    )
    all_placeholders = placeholders + markers
    contained_by_section: dict[str, list[dict[str, Any]]] = {}
    for placeholder in all_placeholders:
        section_id = _string_or_none(placeholder.get("section_id"))
        if section_id:
            contained_by_section.setdefault(section_id, []).append(placeholder)

    missing_span_count = 0
    diagnostics: list[ArticleArtifactDiagnostic] = []
    artifacts: list[ArticleArtifactRecord] = []

    for section in sections:
        if section.get("section_type") == "root":
            continue
        section_id = _string_or_none(section.get("section_id"))
        if not section_id:
            continue
        section_span = spans.get(_string_or_none(section.get("span_id")))
        if section.get("span_id") and section_span is None:
            missing_span_count += 1
            diagnostics.append(_missing_span_diagnostic(section.get("span_id"), section_id))
        section_artifact_id = _section_artifact_id(paper_id, section_id)
        candidate_links = tuple(
            CandidateLink(
                link_id=f"{paper_id}:link:{_artifact_slug(section_artifact_id)}:contains-{_artifact_slug(str(child.get('artifact_id', 'unknown')))}",
                source_artifact_id=section_artifact_id,
                target_ref=str(child.get("artifact_id")),
                link_type="contains",
                review_state="review_required",
                source_spans=(section_span,) if section_span is not None else (),
                confidence_label="deterministic_structure",
            )
            for child in contained_by_section.get(section_id, [])
            if isinstance(child.get("artifact_id"), str) and child.get("artifact_id")
        )
        artifacts.append(
            ArticleArtifactRecord(
                artifact_id=section_artifact_id,
                paper_id=paper_id,
                artifact_type="section",
                review_state="review_required",
                source_spans=(section_span,) if section_span is not None else (),
                # pyrefly: ignore [bad-argument-type]
                section_lineage=_section_lineage(section, spans),  # ty:ignore[invalid-argument-type]
                candidate_links=candidate_links,
                confidence_label="deterministic_structure",
                detector=DETERMINISTIC_FIXTURE_DETECTOR,
                metadata={"fixture_role": "section_lineage_anchor"},
            )
        )

    for placeholder in all_placeholders:
        artifact_id = _string_or_none(placeholder.get("artifact_id"))
        artifact_type = _string_or_none(placeholder.get("artifact_type"))
        if not artifact_id or artifact_type not in ALLOWED_ARTIFACT_TYPES:
            continue
        artifact_span = spans.get(_string_or_none(placeholder.get("span_id")))
        if placeholder.get("span_id") and artifact_span is None:
            missing_span_count += 1
            diagnostics.append(_missing_span_diagnostic(placeholder.get("span_id"), artifact_id))
        section = section_by_id.get(placeholder.get("section_id"), {})  # pyrefly: ignore [bad-assignment, no-matching-overload]
        # pyrefly: ignore [bad-argument-type]
        candidate_links = _placeholder_candidate_links(paper_id, placeholder, artifact_span, spans)  # ty:ignore[invalid-argument-type]
        diagnostic_codes: tuple[str, ...] = ()
        # pyrefly: ignore [bad-argument-type]
        metadata: dict[str, Any] = {"fixture_role": _fixture_role_for_placeholder(artifact_type)}
        if artifact_type in {"figure", "table"} and isinstance(
            placeholder.get("caption_span_id"), str
        ):
            metadata["caption_span_id"] = placeholder["caption_span_id"]
            diagnostic_codes = ("caption_span_present",)
        if artifact_type == "reference":
            diagnostic_codes = ("needs_reference_review",)
        artifacts.append(
            ArticleArtifactRecord(
                artifact_id=artifact_id,
                paper_id=paper_id,
                artifact_type=artifact_type,  # type: ignore[arg-type]  # ty:ignore[invalid-argument-type]
                review_state="review_required",
                source_spans=(artifact_span,) if artifact_span is not None else (),
                # pyrefly: ignore [bad-argument-type]
                section_lineage=_section_lineage(section, spans) if section else None,  # ty:ignore[invalid-argument-type]
                candidate_links=candidate_links,
                confidence_label="deterministic_structure",
                detector=DETERMINISTIC_FIXTURE_DETECTOR,
                diagnostic_codes=diagnostic_codes,
                metadata=metadata,
            )
        )

    manifest = ArticleArtifactManifest(
        paper_id=paper_id,
        run_id=str(structure.get("run_id") or run_id),
        source_refs=source_refs,
        artifacts=tuple(artifacts),
        diagnostics=tuple(diagnostics),
    ).to_redacted_dict()
    manifest["summary"]["missing_span_count"] = missing_span_count
    manifest["summary"]["diagnostic_summary"] = build_article_artifact_diagnostics_summary(manifest)
    return manifest


def build_article_artifact_diagnostics_summary(manifest: dict[str, Any]) -> dict[str, Any]:
    """Return observability counts for deterministic fixture detection."""
    summary = manifest.get("summary", {}) if isinstance(manifest.get("summary"), dict) else {}
    diagnostics = _list_of_dicts(manifest.get("diagnostics"))
    return {
        "artifact_counts_by_type": dict(summary.get("artifact_counts_by_type", {})),
        "candidate_link_type_counts": dict(summary.get("candidate_link_type_counts", {})),
        "review_state_counts": dict(summary.get("review_state_counts", {})),
        # pyrefly: ignore [bad-argument-type]
        "missing_span_count": int(summary.get("missing_span_count", 0) or 0),
        "diagnostic_counts_by_code": _counts(diagnostic.get("code") for diagnostic in diagnostics),
    }


def _validate_redacted_structure_boundary(structure: dict[str, Any]) -> None:
    if structure.get("schema_version") != REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION:
        raise ValueError(
            f"input structure must use schema {REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION}"
        )
    if not isinstance(structure.get("paper_id"), str) or not structure["paper_id"]:
        raise ValueError("input structure must include a non-empty paper_id")
    safety_flags = structure.get("safety_flags")
    if not isinstance(safety_flags, dict):
        raise ValueError("input structure must include safety_flags")
    for key in default_safety_flags():
        if safety_flags.get(key) is True:
            raise ValueError(f"input structure safety flag must be false: {key}")
    if _validate_forbidden_keys(structure):
        raise ValueError("input structure contains forbidden raw payload keys")
    if _validate_source_of_truth_markers(structure):
        raise ValueError("input structure contains source-of-truth markers")


def _source_ref_from_structure(source: dict[str, Any], paper_id: str) -> SourceReference:
    return SourceReference(
        source_id=str(source.get("source_id")),
        paper_id=paper_id,
        source_role="redacted_article_structure",
        source_path=_string_or_none(source.get("source_path")),
        sha256=_string_or_none(source.get("sha256")),
        media_type=_string_or_none(source.get("media_type")),
    )


def _span_from_structure(span: dict[str, Any]) -> SourceSpan:
    bbox = span.get("bbox")
    return SourceSpan(
        span_id=str(span.get("span_id")),
        source_id=str(span.get("source_id")),
        coordinate_space=str(span.get("coordinate_space")),  # type: ignore[arg-type]  # ty:ignore[invalid-argument-type]
        char_start=span.get("char_start") if isinstance(span.get("char_start"), int) else None,
        char_end=span.get("char_end") if isinstance(span.get("char_end"), int) else None,
        page_start=span.get("page_start") if isinstance(span.get("page_start"), int) else None,
        page_end=span.get("page_end") if isinstance(span.get("page_end"), int) else None,
        # pyrefly: ignore [bad-argument-type]
        bbox=tuple(float(value) for value in bbox)
        if isinstance(bbox, list) and len(bbox) == 4
        else None,  # ty:ignore[invalid-argument-type]
        span_hash=_string_or_none(span.get("span_hash")),
    )


def _section_lineage(section: dict[str, Any], spans: dict[str, SourceSpan]) -> SectionLineage:
    ordinal = section.get("ordinal_path")
    return SectionLineage(
        section_id=str(section.get("section_id")),
        parent_section_id=_string_or_none(section.get("parent_section_id")),
        section_type=_string_or_none(section.get("section_type")),
        ordinal_path=tuple(value for value in ordinal if isinstance(value, int))
        if isinstance(ordinal, list)
        else (),
        # pyrefly: ignore [bad-argument-type]
        source_span=spans.get(_string_or_none(section.get("span_id"))),
    )


def _placeholder_candidate_links(
    paper_id: str,
    placeholder: dict[str, Any],
    artifact_span: SourceSpan | None,
    spans: dict[str, SourceSpan],
) -> tuple[CandidateLink, ...]:
    artifact_id = str(placeholder.get("artifact_id"))
    artifact_type = str(placeholder.get("artifact_type"))
    links: list[CandidateLink] = []
    # pyrefly: ignore [bad-argument-type]
    link_span = spans.get(_string_or_none(placeholder.get("caption_span_id"))) or artifact_span
    for target in _string_list(placeholder.get("candidate_link_targets")):
        link_type: CandidateLinkType = (
            "supports" if artifact_type in {"figure", "table"} else "candidate_for"
        )
        links.append(
            CandidateLink(
                link_id=f"{paper_id}:link:{_artifact_slug(artifact_id)}:{link_type}-{_artifact_slug(target)}",
                source_artifact_id=artifact_id,
                target_ref=target,
                link_type=link_type,
                review_state="review_required",
                source_spans=(link_span,) if link_span is not None else (),
                confidence_label="needs_semantic_review",
                diagnostic_codes=("needs_semantic_review",),
            )
        )
    if isinstance(placeholder.get("target_ref"), str):
        links.append(
            CandidateLink(
                link_id=f"{paper_id}:link:{_artifact_slug(artifact_id)}:cites-external",
                source_artifact_id=artifact_id,
                target_ref=str(placeholder["target_ref"]),
                link_type="cites",
                review_state="review_required",
                source_spans=(artifact_span,) if artifact_span is not None else (),
                confidence_label="needs_reference_review",
                diagnostic_codes=("needs_reference_review",),
            )
        )
    elif artifact_type == "equation" and isinstance(placeholder.get("section_id"), str):
        links.append(
            CandidateLink(
                link_id=f"{paper_id}:link:{_artifact_slug(artifact_id)}:located-in-{_section_slug(str(placeholder['section_id']))}",
                source_artifact_id=artifact_id,
                target_ref=str(placeholder["section_id"]),
                link_type="located_in",
                review_state="review_required",
                source_spans=(artifact_span,) if artifact_span is not None else (),
                confidence_label="deterministic_structure",
            )
        )
    return tuple(links)


def _section_artifact_id(paper_id: str, section_id: str) -> str:
    return f"{paper_id}:artifact:section:{_section_slug(section_id)}"


def _section_slug(section_id: str) -> str:
    return section_id.rsplit(":", 1)[-1]


def _artifact_slug(artifact_id: str) -> str:
    if ":artifact:" in artifact_id:
        artifact_id = artifact_id.split(":artifact:", 1)[1]
    return artifact_id.replace(":", "-")


def _fixture_role_for_placeholder(artifact_type: str) -> str:
    roles = {
        "figure": "captioned_figure_placeholder",
        "table": "captioned_table_placeholder",
        "equation": "equation_placeholder",
        "reference": "citation_placeholder",
        "dataset": "structured_dataset_marker",
        "method": "structured_method_marker",
        "metric": "structured_metric_marker",
        "experiment": "structured_experiment_marker",
    }
    return roles.get(artifact_type, "structured_artifact_marker")


def _missing_span_diagnostic(span_id: Any, object_id: str) -> ArticleArtifactDiagnostic:
    return ArticleArtifactDiagnostic(
        code="missing_span",
        json_path="/safe_spans",
        severity="warning",
        object_id=object_id,
        message="Fixture references a span ID that is not present in safe_spans; no raw content was inspected.",
        blocks_import=True,
    )


def validate_article_artifact_manifest(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    """Return redacted diagnostics with stable codes and JSON paths."""
    diagnostics: list[ArticleArtifactDiagnostic] = []
    if manifest.get("schema_version") != ARTICLE_ARTIFACT_SCHEMA_VERSION:
        diagnostics.append(_diagnostic("invalid_schema_version", "/schema_version"))
    diagnostics.extend(
        _required(
            manifest,
            (
                "schema_version",
                "run_id",
                "paper_id",
                "artifacts",
                "summary",
                "diagnostics",
                "safety_flags",
            ),
            "",
        )
    )
    diagnostics.extend(_validate_forbidden_keys(manifest))
    diagnostics.extend(_validate_source_of_truth_markers(manifest))
    diagnostics.extend(_validate_safety_flags(manifest.get("safety_flags"), "/safety_flags"))
    if manifest.get("production_import_attempted") is not False:
        diagnostics.append(
            _diagnostic("production_import_attempted", "/production_import_attempted")
        )
    if manifest.get("ladybugdb_written") is not False:
        diagnostics.append(_diagnostic("ladybugdb_written", "/ladybugdb_written"))
    if manifest.get("promoted_to_fact_count") != 0:
        diagnostics.append(_diagnostic("promoted_to_fact_count_nonzero", "/promoted_to_fact_count"))
    if manifest.get("import_eligible_count") != 0:
        diagnostics.append(_diagnostic("import_eligible_count_nonzero", "/import_eligible_count"))

    artifacts = _list_of_dicts(manifest.get("artifacts"))
    manifest_sources = _list_of_dicts(manifest.get("source_refs"))
    known_source_ids = {
        source.get("source_id")
        for source in manifest_sources
        if isinstance(source.get("source_id"), str)
    }
    diagnostics.extend(
        _validate_duplicate_ids(artifacts, "artifact_id", "/artifacts", "duplicate_artifact_id")
    )
    diagnostics.extend(
        _validate_duplicate_ids(
            manifest_sources, "source_id", "/source_refs", "duplicate_source_id"
        )
    )
    for index, artifact in enumerate(artifacts):
        diagnostics.extend(
            _validate_artifact(
                artifact,
                f"/artifacts[{index}]",
                manifest.get("paper_id"),
                known_source_ids,  # ty:ignore[invalid-argument-type]
            )
        )
    for index, source in enumerate(manifest_sources):
        diagnostics.extend(
            _validate_source_ref(source, f"/source_refs[{index}]", manifest.get("paper_id"))
        )
    for index, diagnostic_record in enumerate(_list_of_dicts(manifest.get("diagnostics"))):
        diagnostics.extend(_validate_diagnostic_record(diagnostic_record, f"/diagnostics[{index}]"))
    diagnostics.extend(_validate_summary(manifest.get("summary"), artifacts))
    return [diagnostic.to_redacted_dict() for diagnostic in diagnostics]


def to_json(value: dict[str, Any]) -> str:
    """Serialize a contract dictionary deterministically."""
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def _validate_artifact(
    artifact: dict[str, Any], path: str, paper_id: Any, known_source_ids: set[str]
) -> list[ArticleArtifactDiagnostic]:
    diagnostics: list[ArticleArtifactDiagnostic] = []
    artifact_id = _string_or_none(artifact.get("artifact_id"))
    diagnostics.extend(
        _required(
            artifact,
            (
                "artifact_id",
                "paper_id",
                "artifact_type",
                "review_state",
                "safety_flags",
                "allowed_uses",
                "excluded_uses",
            ),
            path,
        )
    )
    diagnostics.extend(
        _validate_non_empty_ids(artifact, ("artifact_id", "paper_id"), path, artifact_id)
    )
    if artifact.get("paper_id") != paper_id:
        diagnostics.append(_diagnostic("paper_id_mismatch", f"{path}/paper_id", artifact_id))
    if artifact.get("artifact_type") not in ALLOWED_ARTIFACT_TYPES:
        diagnostics.append(
            _diagnostic("invalid_artifact_type", f"{path}/artifact_type", artifact_id)
        )
    if artifact.get("review_state") not in ALLOWED_REVIEW_STATES:
        diagnostics.append(_diagnostic("invalid_review_state", f"{path}/review_state", artifact_id))
    diagnostics.extend(
        _validate_safety_flags(artifact.get("safety_flags"), f"{path}/safety_flags", artifact_id)
    )
    if artifact.get("promoted_to_fact") is not False:
        diagnostics.append(
            _diagnostic("artifact_promoted_to_fact", f"{path}/promoted_to_fact", artifact_id)
        )
    if artifact.get("import_eligible") is not False:
        diagnostics.append(
            _diagnostic("artifact_import_eligible", f"{path}/import_eligible", artifact_id)
        )
    diagnostics.extend(_validate_uses(artifact, path, artifact_id))
    for index, source in enumerate(_list_of_dicts(artifact.get("source_refs"))):
        diagnostics.extend(_validate_source_ref(source, f"{path}/source_refs[{index}]", paper_id))
    for index, span in enumerate(_list_of_dicts(artifact.get("source_spans"))):
        diagnostics.extend(
            _validate_span(span, f"{path}/source_spans[{index}]", artifact_id, known_source_ids)
        )
    lineage = artifact.get("section_lineage")
    if isinstance(lineage, dict) and isinstance(lineage.get("source_span"), dict):
        diagnostics.extend(
            _validate_span(
                lineage["source_span"],
                f"{path}/section_lineage/source_span",
                artifact_id,
                known_source_ids,
            )
        )
    for index, link in enumerate(_list_of_dicts(artifact.get("candidate_links"))):
        diagnostics.extend(
            _validate_candidate_link(
                link, f"{path}/candidate_links[{index}]", artifact_id, known_source_ids
            )
        )
    diagnostics.extend(
        _validate_duplicate_ids(
            _list_of_dicts(artifact.get("candidate_links")),
            "link_id",
            f"{path}/candidate_links",
            "duplicate_candidate_link_id",
        )
    )
    diagnostics.extend(
        _validate_duplicate_ids(
            _list_of_dicts(artifact.get("source_spans")),
            "span_id",
            f"{path}/source_spans",
            "duplicate_source_span_id",
        )
    )
    return diagnostics


def _validate_candidate_link(
    link: dict[str, Any], path: str, artifact_id: str | None, known_source_ids: set[str]
) -> list[ArticleArtifactDiagnostic]:
    diagnostics: list[ArticleArtifactDiagnostic] = []
    link_id = _string_or_none(link.get("link_id"))
    diagnostics.extend(
        _required(
            link,
            (
                "link_id",
                "source_artifact_id",
                "target_ref",
                "link_type",
                "review_state",
                "allowed_uses",
                "excluded_uses",
            ),
            path,
        )
    )
    diagnostics.extend(
        _validate_non_empty_ids(
            link, ("link_id", "source_artifact_id", "target_ref"), path, link_id
        )
    )
    if link.get("source_artifact_id") != artifact_id:
        diagnostics.append(
            _diagnostic("candidate_link_source_mismatch", f"{path}/source_artifact_id", link_id)
        )
    if link.get("link_type") not in ALLOWED_CANDIDATE_LINK_TYPES:
        diagnostics.append(_diagnostic("invalid_candidate_link_type", f"{path}/link_type", link_id))
    if link.get("review_state") not in ALLOWED_REVIEW_STATES:
        diagnostics.append(
            _diagnostic("invalid_candidate_link_review_state", f"{path}/review_state", link_id)
        )
    if link.get("promoted_to_fact") is not False:
        diagnostics.append(
            _diagnostic("candidate_link_promoted_to_fact", f"{path}/promoted_to_fact", link_id)
        )
    if link.get("import_eligible") is not False:
        diagnostics.append(
            _diagnostic("candidate_link_import_eligible", f"{path}/import_eligible", link_id)
        )
    diagnostics.extend(_validate_uses(link, path, link_id))
    for index, span in enumerate(_list_of_dicts(link.get("source_spans"))):
        diagnostics.extend(
            _validate_span(span, f"{path}/source_spans[{index}]", link_id, known_source_ids)
        )
    return diagnostics


def _validate_source_ref(
    source: dict[str, Any], path: str, paper_id: Any
) -> list[ArticleArtifactDiagnostic]:
    diagnostics = _required(
        source,
        ("source_id", "paper_id", "source_role", "raw_text_embedded", "raw_binary_embedded"),
        path,
    )
    source_id = _string_or_none(source.get("source_id"))
    diagnostics.extend(
        _validate_non_empty_ids(source, ("source_id", "paper_id", "source_role"), path, source_id)
    )
    if source.get("paper_id") != paper_id:
        diagnostics.append(
            _diagnostic("source_ref_paper_id_mismatch", f"{path}/paper_id", source_id)
        )
    if source.get("raw_text_embedded") is not False:
        diagnostics.append(
            _diagnostic("source_ref_raw_text_embedded", f"{path}/raw_text_embedded", source_id)
        )
    if source.get("raw_binary_embedded") is not False:
        diagnostics.append(
            _diagnostic("source_ref_raw_binary_embedded", f"{path}/raw_binary_embedded", source_id)
        )
    sha = source.get("sha256")
    if sha is not None and not _valid_sha256(sha):
        diagnostics.append(_diagnostic("invalid_sha256", f"{path}/sha256", source_id))
    return diagnostics


def _validate_span(
    span: dict[str, Any], path: str, object_id: str | None, known_source_ids: set[str]
) -> list[ArticleArtifactDiagnostic]:
    diagnostics = _required(
        span, ("span_id", "source_id", "coordinate_space", "raw_text_embedded"), path
    )
    diagnostics.extend(_validate_non_empty_ids(span, ("span_id", "source_id"), path, object_id))
    if isinstance(span.get("source_id"), str) and span.get("source_id") not in known_source_ids:
        diagnostics.append(_diagnostic("unknown_source_id", f"{path}/source_id", object_id))
    if span.get("coordinate_space") not in ALLOWED_COORDINATE_SPACES:
        diagnostics.append(
            _diagnostic("invalid_coordinate_space", f"{path}/coordinate_space", object_id)
        )
    if span.get("raw_text_embedded") is not False:
        diagnostics.append(
            _diagnostic("span_raw_text_embedded", f"{path}/raw_text_embedded", object_id)
        )
    if span.get("coordinate_space") != "artifact_record":
        char_start = span.get("char_start")
        char_end = span.get("char_end")
        has_chars = (
            isinstance(char_start, int) and isinstance(char_end, int) and char_end > char_start >= 0
        )
        has_page_bbox = (
            span.get("coordinate_space") == "page_bbox"
            and isinstance(span.get("bbox"), list)
            and len(span.get("bbox")) == 4  # ty:ignore[invalid-argument-type]
        )
        if not has_chars and not has_page_bbox:
            diagnostics.append(_diagnostic("invalid_source_span_coordinates", path, object_id))
    return diagnostics


def _validate_diagnostic_record(
    record: dict[str, Any], path: str
) -> list[ArticleArtifactDiagnostic]:
    diagnostics = _required(record, ("code", "json_path", "severity", "blocks_import"), path)
    if record.get("severity") not in ALLOWED_SEVERITIES:
        diagnostics.append(
            _diagnostic(
                "invalid_diagnostic_severity",
                f"{path}/severity",
                _string_or_none(record.get("object_id")),
            )
        )
    if not isinstance(record.get("json_path"), str) or not str(
        record.get("json_path", "")
    ).startswith("/"):
        diagnostics.append(
            _diagnostic(
                "invalid_diagnostic_json_path",
                f"{path}/json_path",
                _string_or_none(record.get("object_id")),
            )
        )
    return diagnostics


def _validate_summary(
    value: Any, artifacts: list[dict[str, Any]]
) -> list[ArticleArtifactDiagnostic]:
    if not isinstance(value, dict):
        return [_diagnostic("missing_summary", "/summary")]
    diagnostics = _required(
        value,
        (
            "artifact_count",
            "candidate_link_count",
            "artifact_counts_by_type",
            "review_state_counts",
            "safety_flags",
        ),
        "/summary",
    )
    if value.get("artifact_count") != len(artifacts):
        diagnostics.append(
            _diagnostic("summary_artifact_count_mismatch", "/summary/artifact_count")
        )
    if value.get("promoted_to_fact_count") != 0:
        diagnostics.append(
            _diagnostic("summary_promoted_to_fact_count_nonzero", "/summary/promoted_to_fact_count")
        )
    if value.get("import_eligible_count") != 0:
        diagnostics.append(
            _diagnostic("summary_import_eligible_count_nonzero", "/summary/import_eligible_count")
        )
    diagnostics.extend(_validate_safety_flags(value.get("safety_flags"), "/summary/safety_flags"))
    return diagnostics


def _validate_uses(
    value: dict[str, Any], path: str, object_id: str | None
) -> list[ArticleArtifactDiagnostic]:
    diagnostics: list[ArticleArtifactDiagnostic] = []
    allowed_uses = set(_string_list(value.get("allowed_uses")))
    excluded_uses = set(_string_list(value.get("excluded_uses")))
    if TRUSTED_IMPORT_USE in allowed_uses:
        diagnostics.append(_diagnostic("trusted_import_allowed", f"{path}/allowed_uses", object_id))
    for use in EXCLUDED_USES:
        if use not in excluded_uses:
            diagnostics.append(
                _diagnostic("missing_excluded_use", f"{path}/excluded_uses", object_id)
            )
    return diagnostics


def _validate_safety_flags(
    value: Any, path: str, object_id: str | None = None
) -> list[ArticleArtifactDiagnostic]:
    if not isinstance(value, dict):
        return [_diagnostic("missing_safety_flags", path, object_id)]
    diagnostics: list[ArticleArtifactDiagnostic] = []
    universal_keys = set(SafetyFlags().to_dict())
    for key, expected in default_safety_flags().items():
        if key in universal_keys and key not in value:
            # M035 adds Universal KB SafetyFlags to new article artifacts, but
            # older M023 fixtures did not carry every M034 key. Preserve
            # compatibility while still rejecting unsafe values when the key is
            # present.
            continue
        if value.get(key) is not expected:
            code = (
                f"safety_flag_true:{key}"
                if value.get(key) is True
                else f"safety_flag_invalid:{key}"
            )
            diagnostics.append(_diagnostic(code, f"{path}/{key}", object_id))
    return diagnostics


def _validate_forbidden_keys(value: Any, path: str = "") -> list[ArticleArtifactDiagnostic]:
    diagnostics: list[ArticleArtifactDiagnostic] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}/{key}" if path else f"/{key}"
            if key in FORBIDDEN_PAYLOAD_KEYS:
                diagnostics.append(_diagnostic("forbidden_payload_key", child_path))
            diagnostics.extend(_validate_forbidden_keys(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            diagnostics.extend(_validate_forbidden_keys(child, f"{path}[{index}]"))
    return diagnostics


def _validate_non_empty_ids(
    value: dict[str, Any], fields: tuple[str, ...], path: str, object_id: str | None
) -> list[ArticleArtifactDiagnostic]:
    diagnostics: list[ArticleArtifactDiagnostic] = []
    for field_name in fields:
        if (
            field_name in value
            and isinstance(value.get(field_name), str)
            and not value[field_name].strip()
        ):
            diagnostics.append(
                _diagnostic(f"empty_{field_name}", f"{path}/{field_name}", object_id)
            )
    return diagnostics


def _validate_duplicate_ids(
    values: list[dict[str, Any]], field_name: str, path: str, code: str
) -> list[ArticleArtifactDiagnostic]:
    diagnostics: list[ArticleArtifactDiagnostic] = []
    seen: set[str] = set()
    for index, value in enumerate(values):
        identifier = value.get(field_name)
        if not isinstance(identifier, str) or not identifier:
            continue
        if identifier in seen:
            diagnostics.append(_diagnostic(code, f"{path}[{index}]/{field_name}", identifier))
        else:
            seen.add(identifier)
    return diagnostics


def _validate_source_of_truth_markers(
    value: Any, path: str = ""
) -> list[ArticleArtifactDiagnostic]:
    diagnostics: list[ArticleArtifactDiagnostic] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}/{key}" if path else f"/{key}"
            normalized_key = key.lower()
            if normalized_key in FORBIDDEN_SOURCE_OF_TRUTH_KEYS:
                diagnostics.append(_diagnostic("source_of_truth_claim", child_path))
                if isinstance(child, str) and "minimax" in child.lower():
                    diagnostics.append(_diagnostic("minimax_source_of_truth", child_path))
            diagnostics.extend(_validate_source_of_truth_markers(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            diagnostics.extend(_validate_source_of_truth_markers(child, f"{path}[{index}]"))
    return diagnostics


def _required(
    value: dict[str, Any], fields: tuple[str, ...], path: str
) -> list[ArticleArtifactDiagnostic]:
    diagnostics: list[ArticleArtifactDiagnostic] = []
    for field_name in fields:
        if field_name not in value or value.get(field_name) is None:
            diagnostics.append(
                _diagnostic(
                    f"missing_{field_name}", f"{path}/{field_name}" if path else f"/{field_name}"
                )
            )
    return diagnostics


def _diagnostic(
    code: str, json_path: str, object_id: str | None = None
) -> ArticleArtifactDiagnostic:
    return ArticleArtifactDiagnostic(code=code, json_path=json_path, object_id=object_id)


def _valid_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value.lower())
    )


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


def _merge_counts(values: Any) -> dict[str, int]:
    counts: dict[str, int] = {}
    for value in values:
        if not isinstance(value, dict):
            continue
        for key, count in value.items():
            if isinstance(count, int):
                counts[str(key)] = counts.get(str(key), 0) + count
    return dict(sorted(counts.items()))


__all__ = [
    "ALLOWED_ARTIFACT_TYPES",
    "ALLOWED_CANDIDATE_LINK_TYPES",
    "ALLOWED_REVIEW_STATES",
    "ARTICLE_ARTIFACT_RUN_SCHEMA_VERSION",
    "ARTICLE_ARTIFACT_SCHEMA_VERSION",
    "build_article_artifact_diagnostics_summary",
    "build_article_artifact_manifest_from_structure",
    "DEFAULT_DETERMINISTIC_RUN_ID",
    "DETERMINISTIC_FIXTURE_DETECTOR",
    "REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION",
    "ArticleArtifactDiagnostic",
    "ArticleArtifactManifest",
    "ArticleArtifactRecord",
    "ArticleArtifactRunSummary",
    "CandidateLink",
    "SectionLineage",
    "SourceReference",
    "SourceSpan",
    "default_safety_flags",
    "summarize_article_artifacts",
    "to_json",
    "validate_article_artifact_manifest",
]
