# Formerly: src/arxiv_archive/chunk_import_contract.py

"""Import-ready chunk package contract validation.

This module implements the executable subset of the M005/S01 contract. It
validates synthetic and future package dictionaries without reading raw paper
text, generating embeddings, calling LLMs, or writing graph storage.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from research_graph.workflows.universal_kb.contracts import SafetyFlags

EXPECTED_CONTRACT_VERSION = "import-ready-chunk-contract.v1"
EXPECTED_SCHEMA_VERSION = "m005-import-ready-chunk-package.v1"
GRAPH_READY_STATE = "ok_for_graph"
RETRIEVAL_ONLY_STATE = "ok_for_retrieval_only"
REPAIR_REQUIRED_STATE = "repair_required"
REJECT_STATE = "reject"
TRUSTED_IMPORT_USE = "trusted_kg_import"
FORBIDDEN_RAW_FIELDS = frozenset({"text", "raw_text", "chunk_text", "paper_text", "claim_text"})
FORBIDDEN_EMBEDDING_FIELDS = frozenset({"embedding", "embeddings"})
FORBIDDEN_VECTOR_FIELDS = frozenset({"vector", "vectors"})
FORBIDDEN_SECRET_FIELDS = frozenset(
    {"secret", "secrets", "token", "tokens", "api_key", "credentials"}
)
FORBIDDEN_OPTIMIZER_FIELDS = frozenset({"optimizer_trace", "optimizer_traces"})
CLAIM_ROUTE_FORBIDDEN_CHUNK_TYPES = frozenset(
    {"reference_entry", "metadata", "administrative", "noise", "unknown"}
)
VALID_STATES = frozenset(
    {GRAPH_READY_STATE, RETRIEVAL_ONLY_STATE, REPAIR_REQUIRED_STATE, REJECT_STATE}
)
VALID_ROUTES = frozenset(
    {
        "claim_extraction",
        "method_extraction",
        "entity_candidate_extraction",
        "relation_extraction",
        "table_extraction",
        "citation_graph",
        "metadata_graph",
        "retrieval_only",
        "exclude_from_extraction",
    }
)
NON_IMPORT_ROUTES = frozenset({"retrieval_only", "exclude_from_extraction"})
ROUTE_COMPATIBLE_CHUNK_TYPES = {
    "claim_extraction": frozenset({"claim_candidate", "result_candidate", "definition_candidate"}),
    "method_extraction": frozenset({"method_candidate"}),
    "entity_candidate_extraction": frozenset(
        {"claim_candidate", "method_candidate", "result_candidate", "definition_candidate"}
    ),
    "relation_extraction": frozenset(
        {"claim_candidate", "result_candidate", "table_context", "table_row_group"}
    ),
    "table_extraction": frozenset({"table_context", "table_row_group"}),
    "citation_graph": frozenset({"citation_context", "reference_entry"}),
    "metadata_graph": frozenset({"metadata", "administrative"}),
    "retrieval_only": frozenset(
        {
            "claim_candidate",
            "method_candidate",
            "result_candidate",
            "definition_candidate",
            "table_context",
            "table_row_group",
            "figure_caption_context",
            "equation_context",
            "citation_context",
            "reference_entry",
            "metadata",
            "administrative",
            "retrieval_context",
        }
    ),
    "exclude_from_extraction": frozenset({"noise", "unknown", "administrative", "metadata"}),
}


@dataclass(frozen=True)
class ContractDiagnostic:
    """One structured contract validation finding."""

    reason: str
    object_id: str | None = None
    object_type: str | None = None
    route: str | None = None
    blocks_import: bool = True


@dataclass(frozen=True)
class ContractValidationResult:
    """Validation outcome for one import-ready chunk package."""

    valid_package: bool
    import_eligible_chunk_count: int
    refused_chunk_count: int
    diagnostics: list[ContractDiagnostic]

    @property
    def passed(self) -> bool:
        """True when the package is structurally valid; not a guarantee of import readiness."""
        return self.valid_package and not self.diagnostics

    @property
    def has_import_eligible_chunks(self) -> bool:
        """True when at least one chunk satisfies dry-run import eligibility."""
        return self.import_eligible_chunk_count > 0

    @property
    def import_ready(self) -> bool:
        """True only when validation passes and at least one chunk is import-eligible."""
        return self.passed and self.has_import_eligible_chunks

    @property
    def refusal_counts(self) -> dict[str, int]:
        counts: dict[str, int] = {}
        for diagnostic in self.diagnostics:
            counts[diagnostic.reason] = counts.get(diagnostic.reason, 0) + 1
        return dict(sorted(counts.items()))


def validate_import_ready_package(package: dict[str, Any]) -> ContractValidationResult:
    """Validate one package against M005/S01 import-ready chunk invariants."""
    diagnostics: list[ContractDiagnostic] = []
    diagnostics.extend(_validate_package_header(package))
    diagnostics.extend(
        _validate_redaction(
            package, object_id=_string_or_none(package.get("paper_id")), object_type="package"
        )
    )

    paper_id = _string_or_none(package.get("paper_id"))
    elements = _list_of_dicts(package.get("elements"))
    chunks = _list_of_dicts(package.get("chunks"))
    annotations = _list_of_dicts(package.get("annotations"))
    evidence_paths = _list_of_dicts(package.get("evidence_paths"))

    diagnostics.extend(_validate_paper_identity(package.get("paper"), package_paper_id=paper_id))
    diagnostics.extend(
        _validate_conversion_record(package.get("conversion"), package_paper_id=paper_id)
    )
    diagnostics.extend(_validate_elements(elements=elements, package_paper_id=paper_id))

    element_ids = {_string_or_none(element.get("element_id")) for element in elements}
    element_ids.discard(None)
    chunk_ids = {_string_or_none(chunk.get("chunk_id")) for chunk in chunks}
    chunk_ids.discard(None)
    evidence_path_ids = {_string_or_none(path.get("evidence_path_id")) for path in evidence_paths}
    evidence_path_ids.discard(None)
    evidence_paths_by_id = {
        str(path["evidence_path_id"]): path
        for path in evidence_paths
        if _string_or_none(path.get("evidence_path_id")) is not None
    }

    import_eligible = 0
    refused = 0
    for chunk in chunks:
        chunk_diagnostics = _validate_chunk(
            chunk=chunk,
            package_paper_id=paper_id,
            element_ids=element_ids,
            evidence_path_ids=evidence_path_ids,
            evidence_paths_by_id=evidence_paths_by_id,
        )
        diagnostics.extend(chunk_diagnostics)
        if _is_import_eligible_chunk(chunk) and not chunk_diagnostics:
            import_eligible += 1
        else:
            refused += 1

    for annotation in annotations:
        diagnostics.extend(
            _validate_annotation(
                annotation=annotation, chunk_ids=chunk_ids, package_paper_id=paper_id
            )
        )

    for evidence_path in evidence_paths:
        diagnostics.extend(
            _validate_evidence_path(
                evidence_path=evidence_path,
                chunk_ids=chunk_ids,
                element_ids=element_ids,
                package_paper_id=paper_id,
            )
        )

    diagnostics.extend(
        _validate_package_diagnostics(
            package.get("diagnostics"),
            computed_import_eligible=import_eligible,
            computed_refused=refused,
        )
    )

    return ContractValidationResult(
        valid_package=not diagnostics,
        import_eligible_chunk_count=import_eligible,
        refused_chunk_count=refused,
        diagnostics=diagnostics,
    )


def validation_to_dict(result: ContractValidationResult) -> dict[str, Any]:
    """Serialize validation results without raw text or embeddings."""
    return {
        "schema_version": "m005-import-contract-validation.v1",
        "valid_package": result.valid_package,
        "passed": result.passed,
        "has_import_eligible_chunks": result.has_import_eligible_chunks,
        "import_ready": result.import_ready,
        "import_eligible_chunk_count": result.import_eligible_chunk_count,
        "refused_chunk_count": result.refused_chunk_count,
        "refusal_counts": result.refusal_counts,
        "diagnostics": [diagnostic.__dict__ for diagnostic in result.diagnostics],
        "safety_flags": SafetyFlags().to_dict(),
        "raw_text_included": False,
        "embeddings_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def _validate_package_header(package: dict[str, Any]) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    if package.get("schema_version") != EXPECTED_SCHEMA_VERSION:
        diagnostics.append(
            ContractDiagnostic(reason="schema_version_mismatch", object_type="package")
        )
    if package.get("contract_version") != EXPECTED_CONTRACT_VERSION:
        diagnostics.append(
            ContractDiagnostic(reason="contract_version_mismatch", object_type="package")
        )
    if not package.get("paper_id"):
        diagnostics.append(ContractDiagnostic(reason="missing_paper_id", object_type="package"))
    for field in (
        "paper",
        "conversion",
        "elements",
        "chunks",
        "annotations",
        "evidence_paths",
        "diagnostics",
    ):
        if field not in package:
            diagnostics.append(ContractDiagnostic(reason=f"missing_{field}", object_type="package"))
    return diagnostics


def _validate_paper_identity(
    value: Any, *, package_paper_id: str | None
) -> list[ContractDiagnostic]:
    if not isinstance(value, dict):
        return [ContractDiagnostic(reason="missing_paper", object_type="paper")]
    diagnostics = _required_fields(
        value,
        fields=("paper_id", "source_artifacts"),
        object_id=package_paper_id,
        object_type="paper",
    )
    diagnostics.extend(_validate_redaction(value, object_id=package_paper_id, object_type="paper"))
    if _string_or_none(value.get("paper_id")) != package_paper_id:
        diagnostics.append(
            ContractDiagnostic(
                reason="paper_id_mismatch", object_id=package_paper_id, object_type="paper"
            )
        )
    if not isinstance(value.get("source_artifacts"), list):
        diagnostics.append(
            ContractDiagnostic(
                reason="missing_source_artifacts", object_id=package_paper_id, object_type="paper"
            )
        )
    return diagnostics


def _validate_conversion_record(
    value: Any, *, package_paper_id: str | None
) -> list[ContractDiagnostic]:
    if not isinstance(value, dict):
        return [
            ContractDiagnostic(
                reason="missing_conversion", object_id=package_paper_id, object_type="conversion"
            )
        ]
    diagnostics = _required_fields(
        value,
        fields=("conversion_id", "converter", "source_artifact", "quality_state", "warnings"),
        object_id=_string_or_none(value.get("conversion_id")),
        object_type="conversion",
    )
    diagnostics.extend(
        _validate_redaction(
            value, object_id=_string_or_none(value.get("conversion_id")), object_type="conversion"
        )
    )
    if value.get("raw_text_included") is not False:
        diagnostics.append(
            ContractDiagnostic(
                reason="raw_text_leakage", object_id=package_paper_id, object_type="conversion"
            )
        )
    if value.get("embeddings_included") is not False:
        diagnostics.append(
            ContractDiagnostic(
                reason="embedding_leakage", object_id=package_paper_id, object_type="conversion"
            )
        )
    if value.get("quality_state") not in VALID_STATES:
        diagnostics.append(
            ContractDiagnostic(
                reason="invalid_state_for_import",
                object_id=package_paper_id,
                object_type="conversion",
            )
        )
    return diagnostics


def _validate_elements(
    *, elements: list[dict[str, Any]], package_paper_id: str | None
) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    for element in elements:
        element_id = _string_or_none(element.get("element_id"))
        diagnostics.extend(
            _required_fields(
                element,
                fields=(
                    "element_id",
                    "paper_id",
                    "element_type",
                    "section_path",
                    "order_index",
                    "quality_state",
                    "warnings",
                ),
                object_id=element_id,
                object_type="element",
            )
        )
        diagnostics.extend(
            _validate_redaction(element, object_id=element_id, object_type="element")
        )
        if _string_or_none(element.get("paper_id")) != package_paper_id:
            diagnostics.append(
                ContractDiagnostic(
                    reason="missing_paper_id", object_id=element_id, object_type="element"
                )
            )
        if element.get("quality_state") == GRAPH_READY_STATE and not _valid_source_span(
            element.get("source_span")
        ):
            diagnostics.append(
                ContractDiagnostic(
                    reason="missing_source_span", object_id=element_id, object_type="element"
                )
            )
    return diagnostics


def _validate_chunk(
    *,
    chunk: dict[str, Any],
    package_paper_id: str | None,
    element_ids: set[str],
    evidence_path_ids: set[str],
    evidence_paths_by_id: dict[str, dict[str, Any]],
) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    chunk_id = _string_or_none(chunk.get("chunk_id"))
    route = _string_or_none(chunk.get("route"))
    state = _string_or_none(chunk.get("state"))
    chunk_type = _string_or_none(chunk.get("chunk_type"))
    diagnostics.extend(_validate_redaction(chunk, object_id=chunk_id, object_type="chunk"))
    diagnostics.extend(
        _required_fields(
            chunk,
            fields=(
                "chunk_id",
                "paper_id",
                "parent_element_ids",
                "section_path",
                "chunk_type",
                "route",
                "state",
                "allowed_uses",
                "excluded_uses",
                "order_index",
                "source_artifact",
                "quality_warnings",
                "redaction",
            ),
            object_id=chunk_id,
            object_type="chunk",
            route=route,
        )
    )
    if chunk_id is None:
        diagnostics.append(
            ContractDiagnostic(reason="missing_chunk_id", object_type="chunk", route=route)
        )
    if _string_or_none(chunk.get("paper_id")) != package_paper_id:
        diagnostics.append(
            ContractDiagnostic(
                reason="missing_paper_id", object_id=chunk_id, object_type="chunk", route=route
            )
        )
    if chunk_type is None:
        diagnostics.append(
            ContractDiagnostic(
                reason="missing_chunk_type", object_id=chunk_id, object_type="chunk", route=route
            )
        )
    if route is None:
        diagnostics.append(
            ContractDiagnostic(reason="missing_route", object_id=chunk_id, object_type="chunk")
        )
    if state not in VALID_STATES:
        diagnostics.append(
            ContractDiagnostic(
                reason="invalid_state_for_import",
                object_id=chunk_id,
                object_type="chunk",
                route=route,
            )
        )
    parent_ids = chunk.get("parent_element_ids")
    if not isinstance(parent_ids, list) or not parent_ids:
        diagnostics.append(
            ContractDiagnostic(
                reason="missing_parent_element",
                object_id=chunk_id,
                object_type="chunk",
                route=route,
            )
        )
    else:
        for parent_id in parent_ids:
            if str(parent_id) not in element_ids:
                diagnostics.append(
                    ContractDiagnostic(
                        reason="unresolved_parent_element",
                        object_id=chunk_id,
                        object_type="chunk",
                        route=route,
                    )
                )
    if state == GRAPH_READY_STATE:
        diagnostics.extend(
            _validate_graph_ready_chunk(
                chunk=chunk,
                chunk_id=chunk_id,
                route=route,
                chunk_type=chunk_type,
                evidence_path_ids=evidence_path_ids,
                evidence_paths_by_id=evidence_paths_by_id,
            )
        )
    elif state == RETRIEVAL_ONLY_STATE and TRUSTED_IMPORT_USE in _string_list(
        chunk.get("allowed_uses")
    ):
        diagnostics.append(
            ContractDiagnostic(
                reason="retrieval_only_not_importable",
                object_id=chunk_id,
                object_type="chunk",
                route=route,
            )
        )
    elif state == REPAIR_REQUIRED_STATE and TRUSTED_IMPORT_USE in _string_list(
        chunk.get("allowed_uses")
    ):
        diagnostics.append(
            ContractDiagnostic(
                reason="repair_required_not_importable",
                object_id=chunk_id,
                object_type="chunk",
                route=route,
            )
        )
    elif state == REJECT_STATE and TRUSTED_IMPORT_USE in _string_list(chunk.get("allowed_uses")):
        diagnostics.append(
            ContractDiagnostic(
                reason="rejected_not_importable",
                object_id=chunk_id,
                object_type="chunk",
                route=route,
            )
        )
    if route == "claim_extraction" and chunk_type in CLAIM_ROUTE_FORBIDDEN_CHUNK_TYPES:
        reason = (
            "reference_pollutes_claim_route"
            if chunk_type == "reference_entry"
            else "metadata_pollutes_claim_route"
        )
        diagnostics.append(
            ContractDiagnostic(reason=reason, object_id=chunk_id, object_type="chunk", route=route)
        )
    return diagnostics


def _validate_graph_ready_chunk(
    *,
    chunk: dict[str, Any],
    chunk_id: str | None,
    route: str | None,
    chunk_type: str | None,
    evidence_path_ids: set[str],
    evidence_paths_by_id: dict[str, dict[str, Any]],
) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    source_span = chunk.get("source_span")
    if not _valid_source_span(source_span):
        diagnostics.append(
            ContractDiagnostic(
                reason="missing_source_span", object_id=chunk_id, object_type="chunk", route=route
            )
        )
    evidence_path_id = _string_or_none(chunk.get("evidence_path_id"))
    if evidence_path_id is None:
        diagnostics.append(
            ContractDiagnostic(
                reason="missing_evidence_path", object_id=chunk_id, object_type="chunk", route=route
            )
        )
    elif evidence_path_id not in evidence_path_ids:
        diagnostics.append(
            ContractDiagnostic(
                reason="unresolved_evidence_path",
                object_id=chunk_id,
                object_type="chunk",
                route=route,
            )
        )
    elif not _evidence_span_contains_chunk_span(
        evidence_paths_by_id[evidence_path_id].get("source_span"),
        source_span,
    ):
        diagnostics.append(
            ContractDiagnostic(
                reason="invalid_source_span", object_id=chunk_id, object_type="chunk", route=route
            )
        )
    allowed_uses = _string_list(chunk.get("allowed_uses"))
    excluded_uses = _string_list(chunk.get("excluded_uses"))
    if TRUSTED_IMPORT_USE not in allowed_uses or TRUSTED_IMPORT_USE in excluded_uses:
        diagnostics.append(
            ContractDiagnostic(
                reason="route_excluded_from_import",
                object_id=chunk_id,
                object_type="chunk",
                route=route,
            )
        )
    if route in NON_IMPORT_ROUTES:
        reason = (
            "retrieval_only_not_importable"
            if route == "retrieval_only"
            else "route_excluded_from_import"
        )
        diagnostics.append(
            ContractDiagnostic(reason=reason, object_id=chunk_id, object_type="chunk", route=route)
        )
    if route not in VALID_ROUTES:
        diagnostics.append(
            ContractDiagnostic(
                reason="invalid_route", object_id=chunk_id, object_type="chunk", route=route
            )
        )
    elif chunk_type not in ROUTE_COMPATIBLE_CHUNK_TYPES.get(route, frozenset()):
        diagnostics.append(
            ContractDiagnostic(
                reason="route_chunk_type_mismatch",
                object_id=chunk_id,
                object_type="chunk",
                route=route,
            )
        )
    warnings = _list_of_dicts(chunk.get("quality_warnings"))
    for warning in warnings:
        diagnostics.extend(
            _validate_quality_warning(
                warning, fallback_object_id=chunk_id, object_type="warning", route=route
            )
        )
        if warning.get("blocks_import") is True or warning.get("severity") in {
            "repair_required",
            "blocker",
        }:
            diagnostics.append(
                ContractDiagnostic(
                    reason="warning_blocks_import",
                    object_id=chunk_id,
                    object_type="chunk",
                    route=route,
                )
            )
            break
    return diagnostics


def _validate_annotation(
    *,
    annotation: dict[str, Any],
    chunk_ids: set[str],
    package_paper_id: str | None,
) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    annotation_id = _string_or_none(annotation.get("annotation_id"))
    chunk_id = _string_or_none(annotation.get("chunk_id"))
    diagnostics.extend(
        _validate_redaction(annotation, object_id=annotation_id, object_type="annotation")
    )
    diagnostics.extend(
        _required_fields(
            annotation,
            fields=(
                "annotation_id",
                "paper_id",
                "chunk_id",
                "method",
                "annotation_type",
                "values",
                "confidence_class",
                "promoted_to_fact",
                "warnings",
            ),
            object_id=annotation_id,
            object_type="annotation",
        )
    )
    if _string_or_none(annotation.get("paper_id")) != package_paper_id:
        diagnostics.append(
            ContractDiagnostic(
                reason="missing_paper_id", object_id=annotation_id, object_type="annotation"
            )
        )
    if chunk_id not in chunk_ids:
        diagnostics.append(
            ContractDiagnostic(
                reason="unresolved_annotation_chunk",
                object_id=annotation_id,
                object_type="annotation",
            )
        )
    if annotation.get("promoted_to_fact") is True:
        diagnostics.append(
            ContractDiagnostic(
                reason="annotation_promoted_to_fact",
                object_id=annotation_id,
                object_type="annotation",
            )
        )
    return diagnostics


def _validate_evidence_path(
    *,
    evidence_path: dict[str, Any],
    chunk_ids: set[str],
    element_ids: set[str],
    package_paper_id: str | None,
) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    evidence_path_id = _string_or_none(evidence_path.get("evidence_path_id"))
    chunk_id = _string_or_none(evidence_path.get("chunk_id"))
    diagnostics.extend(
        _validate_redaction(evidence_path, object_id=evidence_path_id, object_type="evidence_path")
    )
    diagnostics.extend(
        _required_fields(
            evidence_path,
            fields=(
                "evidence_path_id",
                "paper_id",
                "chunk_id",
                "source_element_ids",
                "source_artifact",
                "source_span",
                "provenance_chain",
            ),
            object_id=evidence_path_id,
            object_type="evidence_path",
        )
    )
    if _string_or_none(evidence_path.get("paper_id")) != package_paper_id:
        diagnostics.append(
            ContractDiagnostic(
                reason="missing_paper_id", object_id=evidence_path_id, object_type="evidence_path"
            )
        )
    if chunk_id not in chunk_ids:
        diagnostics.append(
            ContractDiagnostic(
                reason="unresolved_evidence_path",
                object_id=evidence_path_id,
                object_type="evidence_path",
            )
        )
    for element_id in _string_list(evidence_path.get("source_element_ids")):
        if element_id not in element_ids:
            diagnostics.append(
                ContractDiagnostic(
                    reason="unresolved_parent_element",
                    object_id=evidence_path_id,
                    object_type="evidence_path",
                )
            )
    if not _valid_source_span(evidence_path.get("source_span")):
        diagnostics.append(
            ContractDiagnostic(
                reason="missing_source_span",
                object_id=evidence_path_id,
                object_type="evidence_path",
            )
        )
    return diagnostics


def _validate_package_diagnostics(
    value: Any,
    *,
    computed_import_eligible: int,
    computed_refused: int,
) -> list[ContractDiagnostic]:
    if not isinstance(value, dict):
        return [ContractDiagnostic(reason="missing_diagnostics", object_type="diagnostics")]
    diagnostics = _required_fields(
        value,
        fields=(
            "package_state",
            "valid_package",
            "import_eligible_chunk_count",
            "refused_chunk_count",
            "counts_by_state",
            "counts_by_route",
            "counts_by_chunk_type",
            "refusal_counts",
            "source_span_coverage",
            "parent_reference_resolution_rate",
            "evidence_path_resolution_rate",
            "raw_text_included",
            "embeddings_included",
            "ladybugdb_written",
            "production_import_attempted",
        ),
        object_id=None,
        object_type="diagnostics",
    )
    diagnostics.extend(_validate_redaction(value, object_id=None, object_type="diagnostics"))
    if value.get("raw_text_included") is not False:
        diagnostics.append(ContractDiagnostic(reason="raw_text_leakage", object_type="diagnostics"))
    if value.get("embeddings_included") is not False:
        diagnostics.append(
            ContractDiagnostic(reason="embedding_leakage", object_type="diagnostics")
        )
    if value.get("ladybugdb_written") is not False:
        diagnostics.append(
            ContractDiagnostic(reason="production_write_attempted", object_type="diagnostics")
        )
    if value.get("production_import_attempted") is not False:
        diagnostics.append(
            ContractDiagnostic(reason="production_import_attempted", object_type="diagnostics")
        )
    if value.get("import_eligible_chunk_count") != computed_import_eligible:
        diagnostics.append(
            ContractDiagnostic(
                reason="diagnostics_import_count_mismatch", object_type="diagnostics"
            )
        )
    if value.get("refused_chunk_count") != computed_refused:
        diagnostics.append(
            ContractDiagnostic(
                reason="diagnostics_refusal_count_mismatch", object_type="diagnostics"
            )
        )
    return diagnostics


def _validate_quality_warning(
    warning: dict[str, Any],
    *,
    fallback_object_id: str | None,
    object_type: str,
    route: str | None,
) -> list[ContractDiagnostic]:
    diagnostics = _required_fields(
        warning,
        fields=("code", "severity", "message", "object_id", "blocks_import"),
        object_id=_string_or_none(warning.get("object_id")) or fallback_object_id,
        object_type=object_type,
        route=route,
    )
    diagnostics.extend(
        _validate_redaction(
            warning,
            object_id=_string_or_none(warning.get("object_id")) or fallback_object_id,
            object_type=object_type,
        )
    )
    return diagnostics


def _validate_redaction(
    payload: dict[str, Any], *, object_id: str | None, object_type: str
) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    for field in sorted(FORBIDDEN_RAW_FIELDS & set(payload)):
        diagnostics.append(
            ContractDiagnostic(
                reason="raw_text_leakage", object_id=object_id or field, object_type=object_type
            )
        )
    for field in sorted(FORBIDDEN_EMBEDDING_FIELDS & set(payload)):
        diagnostics.append(
            ContractDiagnostic(
                reason="embedding_leakage", object_id=object_id or field, object_type=object_type
            )
        )
    for field in sorted(FORBIDDEN_VECTOR_FIELDS & set(payload)):
        diagnostics.append(
            ContractDiagnostic(
                reason="vector_leakage", object_id=object_id or field, object_type=object_type
            )
        )
    for field in sorted(FORBIDDEN_SECRET_FIELDS & set(payload)):
        diagnostics.append(
            ContractDiagnostic(
                reason="secret_leakage", object_id=object_id or field, object_type=object_type
            )
        )
    for field in sorted(FORBIDDEN_OPTIMIZER_FIELDS & set(payload)):
        diagnostics.append(
            ContractDiagnostic(
                reason="optimizer_trace_leakage",
                object_id=object_id or field,
                object_type=object_type,
            )
        )
    redaction = payload.get("redaction")
    if isinstance(redaction, dict):
        if (
            redaction.get("raw_text_included") is True
            or redaction.get("chunk_text_included") is True
        ):
            diagnostics.append(
                ContractDiagnostic(
                    reason="raw_text_leakage", object_id=object_id, object_type=object_type
                )
            )
        if redaction.get("embeddings_included") is True:
            diagnostics.append(
                ContractDiagnostic(
                    reason="embedding_leakage", object_id=object_id, object_type=object_type
                )
            )
        if redaction.get("vectors_included") is True:
            diagnostics.append(
                ContractDiagnostic(
                    reason="vector_leakage", object_id=object_id, object_type=object_type
                )
            )
        if redaction.get("secrets_included") is True:
            diagnostics.append(
                ContractDiagnostic(
                    reason="secret_leakage", object_id=object_id, object_type=object_type
                )
            )
    diagnostics.extend(
        _validate_nested_redaction(payload, object_id=object_id, object_type=object_type, path=())
    )
    return diagnostics


def _validate_nested_redaction(
    value: Any,
    *,
    object_id: str | None,
    object_type: str,
    path: tuple[str, ...],
) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    if isinstance(value, dict):
        forbidden_fields = (
            FORBIDDEN_RAW_FIELDS
            | FORBIDDEN_EMBEDDING_FIELDS
            | FORBIDDEN_VECTOR_FIELDS
            | FORBIDDEN_SECRET_FIELDS
            | FORBIDDEN_OPTIMIZER_FIELDS
        ) & set(value)
        if path:
            for field in sorted(forbidden_fields):
                if field in FORBIDDEN_RAW_FIELDS:
                    reason = "raw_text_leakage"
                elif field in FORBIDDEN_EMBEDDING_FIELDS:
                    reason = "embedding_leakage"
                elif field in FORBIDDEN_VECTOR_FIELDS:
                    reason = "vector_leakage"
                elif field in FORBIDDEN_SECRET_FIELDS:
                    reason = "secret_leakage"
                else:
                    reason = "optimizer_trace_leakage"
                diagnostics.append(
                    ContractDiagnostic(
                        reason=reason,
                        object_id=_redaction_path(
                            object_id=object_id, object_type=object_type, path=(*path, str(field))
                        ),
                        object_type=object_type,
                    )
                )
        for key, nested_value in value.items():
            diagnostics.extend(
                _validate_nested_redaction(
                    nested_value,
                    object_id=object_id,
                    object_type=object_type,
                    path=(*path, str(key)),
                )
            )
    elif isinstance(value, list):
        for index, nested_value in enumerate(value):
            diagnostics.extend(
                _validate_nested_redaction(
                    nested_value,
                    object_id=object_id,
                    object_type=object_type,
                    path=(*path, str(index)),
                )
            )
    return diagnostics


def _redaction_path(*, object_id: str | None, object_type: str, path: tuple[str, ...]) -> str:
    prefix = object_id or object_type
    return f"{prefix}:{'.'.join(path)}"


def _required_fields(
    payload: dict[str, Any],
    *,
    fields: tuple[str, ...],
    object_id: str | None,
    object_type: str,
    route: str | None = None,
) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    for field in fields:
        if field not in payload or payload.get(field) is None:
            diagnostics.append(
                ContractDiagnostic(
                    reason=f"missing_{field}",
                    object_id=object_id,
                    object_type=object_type,
                    route=route,
                )
            )
    return diagnostics


def _is_import_eligible_chunk(chunk: dict[str, Any]) -> bool:
    route = _string_or_none(chunk.get("route"))
    chunk_type = _string_or_none(chunk.get("chunk_type"))
    return (
        chunk.get("state") == GRAPH_READY_STATE
        and route in VALID_ROUTES
        and route not in NON_IMPORT_ROUTES
        and chunk_type in ROUTE_COMPATIBLE_CHUNK_TYPES.get(route or "", frozenset())
        and TRUSTED_IMPORT_USE in _string_list(chunk.get("allowed_uses"))
        and TRUSTED_IMPORT_USE not in _string_list(chunk.get("excluded_uses"))
    )


def _evidence_span_contains_chunk_span(evidence_span: Any, chunk_span: Any) -> bool:
    if not _valid_source_span(evidence_span) or not _valid_source_span(chunk_span):
        return False
    return (
        evidence_span.get("coordinate_space") == chunk_span.get("coordinate_space")
        and evidence_span["char_start"] <= chunk_span["char_start"]
        and evidence_span["char_end"] >= chunk_span["char_end"]
    )


def _valid_source_span(value: Any) -> bool:
    return (
        isinstance(value, dict)
        and bool(value.get("coordinate_space"))
        and isinstance(value.get("char_start"), int)
        and isinstance(value.get("char_end"), int)
        and value["char_end"] > value["char_start"]
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
    if value is None:
        return None
    text = str(value)
    return text if text else None
