"""Import-ready chunk package contract validation.

This module implements the executable subset of the M005/S01 contract. It
validates synthetic and future package dictionaries without reading raw paper
text, generating embeddings, calling LLMs, or writing graph storage.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

EXPECTED_CONTRACT_VERSION = "import-ready-chunk-contract.v1"
EXPECTED_SCHEMA_VERSION = "m005-import-ready-chunk-package.v1"
GRAPH_READY_STATE = "ok_for_graph"
RETRIEVAL_ONLY_STATE = "ok_for_retrieval_only"
REPAIR_REQUIRED_STATE = "repair_required"
REJECT_STATE = "reject"
TRUSTED_IMPORT_USE = "trusted_kg_import"
FORBIDDEN_FIELDS = frozenset(
    {
        "text",
        "raw_text",
        "chunk_text",
        "paper_text",
        "claim_text",
        "embedding",
        "embeddings",
        "vector",
        "vectors",
    }
)
CLAIM_ROUTE_FORBIDDEN_CHUNK_TYPES = frozenset(
    {
        "reference_entry",
        "metadata",
        "administrative",
        "noise",
        "unknown",
    }
)


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
        return self.valid_package and not self.diagnostics

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
    diagnostics.extend(_validate_redaction(package, object_id=_string_or_none(package.get("paper_id")), object_type="package"))

    paper_id = _string_or_none(package.get("paper_id"))
    elements = _list_of_dicts(package.get("elements"))
    chunks = _list_of_dicts(package.get("chunks"))
    annotations = _list_of_dicts(package.get("annotations"))
    evidence_paths = _list_of_dicts(package.get("evidence_paths"))

    element_ids = {_string_or_none(element.get("element_id")) for element in elements}
    element_ids.discard(None)
    chunk_ids = {_string_or_none(chunk.get("chunk_id")) for chunk in chunks}
    chunk_ids.discard(None)
    evidence_path_ids = {_string_or_none(path.get("evidence_path_id")) for path in evidence_paths}
    evidence_path_ids.discard(None)

    import_eligible = 0
    refused = 0
    for chunk in chunks:
        chunk_diagnostics = _validate_chunk(
            chunk=chunk,
            package_paper_id=paper_id,
            element_ids=element_ids,
            evidence_path_ids=evidence_path_ids,
        )
        diagnostics.extend(chunk_diagnostics)
        if _is_import_eligible_chunk(chunk) and not chunk_diagnostics:
            import_eligible += 1
        else:
            refused += 1

    for annotation in annotations:
        diagnostics.extend(_validate_annotation(annotation=annotation, chunk_ids=chunk_ids, package_paper_id=paper_id))

    for evidence_path in evidence_paths:
        diagnostics.extend(_validate_evidence_path(evidence_path=evidence_path, chunk_ids=chunk_ids, element_ids=element_ids))

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
        "import_eligible_chunk_count": result.import_eligible_chunk_count,
        "refused_chunk_count": result.refused_chunk_count,
        "refusal_counts": result.refusal_counts,
        "diagnostics": [diagnostic.__dict__ for diagnostic in result.diagnostics],
        "raw_text_included": False,
        "embeddings_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def _validate_package_header(package: dict[str, Any]) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    if package.get("schema_version") != EXPECTED_SCHEMA_VERSION:
        diagnostics.append(ContractDiagnostic(reason="schema_version_mismatch", object_type="package"))
    if package.get("contract_version") != EXPECTED_CONTRACT_VERSION:
        diagnostics.append(ContractDiagnostic(reason="contract_version_mismatch", object_type="package"))
    if not package.get("paper_id"):
        diagnostics.append(ContractDiagnostic(reason="missing_paper_id", object_type="package"))
    for field in ("elements", "chunks", "annotations", "evidence_paths", "diagnostics"):
        if field not in package:
            diagnostics.append(ContractDiagnostic(reason=f"missing_{field}", object_type="package"))
    return diagnostics


def _validate_chunk(
    *,
    chunk: dict[str, Any],
    package_paper_id: str | None,
    element_ids: set[str],
    evidence_path_ids: set[str],
) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    chunk_id = _string_or_none(chunk.get("chunk_id"))
    route = _string_or_none(chunk.get("route"))
    state = _string_or_none(chunk.get("state"))
    chunk_type = _string_or_none(chunk.get("chunk_type"))
    diagnostics.extend(_validate_redaction(chunk, object_id=chunk_id, object_type="chunk"))
    if chunk_id is None:
        diagnostics.append(ContractDiagnostic(reason="missing_chunk_id", object_type="chunk", route=route))
    if _string_or_none(chunk.get("paper_id")) != package_paper_id:
        diagnostics.append(ContractDiagnostic(reason="missing_paper_id", object_id=chunk_id, object_type="chunk", route=route))
    if chunk_type is None:
        diagnostics.append(ContractDiagnostic(reason="missing_chunk_type", object_id=chunk_id, object_type="chunk", route=route))
    if route is None:
        diagnostics.append(ContractDiagnostic(reason="missing_route", object_id=chunk_id, object_type="chunk"))
    parent_ids = chunk.get("parent_element_ids")
    if not isinstance(parent_ids, list) or not parent_ids:
        diagnostics.append(ContractDiagnostic(reason="missing_parent_element", object_id=chunk_id, object_type="chunk", route=route))
    else:
        for parent_id in parent_ids:
            if str(parent_id) not in element_ids:
                diagnostics.append(
                    ContractDiagnostic(reason="unresolved_parent_element", object_id=chunk_id, object_type="chunk", route=route)
                )
    if state == GRAPH_READY_STATE:
        diagnostics.extend(_validate_graph_ready_chunk(chunk=chunk, chunk_id=chunk_id, route=route, evidence_path_ids=evidence_path_ids))
    elif state == RETRIEVAL_ONLY_STATE and TRUSTED_IMPORT_USE in _string_list(chunk.get("allowed_uses")):
        diagnostics.append(ContractDiagnostic(reason="retrieval_only_not_importable", object_id=chunk_id, object_type="chunk", route=route))
    elif state == REPAIR_REQUIRED_STATE and TRUSTED_IMPORT_USE in _string_list(chunk.get("allowed_uses")):
        diagnostics.append(ContractDiagnostic(reason="repair_required_not_importable", object_id=chunk_id, object_type="chunk", route=route))
    elif state == REJECT_STATE and TRUSTED_IMPORT_USE in _string_list(chunk.get("allowed_uses")):
        diagnostics.append(ContractDiagnostic(reason="rejected_not_importable", object_id=chunk_id, object_type="chunk", route=route))
    if route == "claim_extraction" and chunk_type in CLAIM_ROUTE_FORBIDDEN_CHUNK_TYPES:
        reason = "reference_pollutes_claim_route" if chunk_type == "reference_entry" else "metadata_pollutes_claim_route"
        diagnostics.append(ContractDiagnostic(reason=reason, object_id=chunk_id, object_type="chunk", route=route))
    return diagnostics


def _validate_graph_ready_chunk(
    *,
    chunk: dict[str, Any],
    chunk_id: str | None,
    route: str | None,
    evidence_path_ids: set[str],
) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    source_span = chunk.get("source_span")
    if not _valid_source_span(source_span):
        diagnostics.append(ContractDiagnostic(reason="missing_source_span", object_id=chunk_id, object_type="chunk", route=route))
    evidence_path_id = _string_or_none(chunk.get("evidence_path_id"))
    if evidence_path_id is None:
        diagnostics.append(ContractDiagnostic(reason="missing_evidence_path", object_id=chunk_id, object_type="chunk", route=route))
    elif evidence_path_id not in evidence_path_ids:
        diagnostics.append(ContractDiagnostic(reason="unresolved_evidence_path", object_id=chunk_id, object_type="chunk", route=route))
    allowed_uses = _string_list(chunk.get("allowed_uses"))
    excluded_uses = _string_list(chunk.get("excluded_uses"))
    if TRUSTED_IMPORT_USE not in allowed_uses or TRUSTED_IMPORT_USE in excluded_uses:
        diagnostics.append(ContractDiagnostic(reason="route_excluded_from_import", object_id=chunk_id, object_type="chunk", route=route))
    warnings = _list_of_dicts(chunk.get("quality_warnings"))
    for warning in warnings:
        if warning.get("blocks_import") is True or warning.get("severity") in {"repair_required", "blocker"}:
            diagnostics.append(ContractDiagnostic(reason="warning_blocks_import", object_id=chunk_id, object_type="chunk", route=route))
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
    diagnostics.extend(_validate_redaction(annotation, object_id=annotation_id, object_type="annotation"))
    if _string_or_none(annotation.get("paper_id")) != package_paper_id:
        diagnostics.append(ContractDiagnostic(reason="missing_paper_id", object_id=annotation_id, object_type="annotation"))
    if chunk_id not in chunk_ids:
        diagnostics.append(ContractDiagnostic(reason="unresolved_evidence_path", object_id=annotation_id, object_type="annotation"))
    if annotation.get("promoted_to_fact") is True:
        diagnostics.append(ContractDiagnostic(reason="annotation_promoted_to_fact", object_id=annotation_id, object_type="annotation"))
    return diagnostics


def _validate_evidence_path(
    *,
    evidence_path: dict[str, Any],
    chunk_ids: set[str],
    element_ids: set[str],
) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    evidence_path_id = _string_or_none(evidence_path.get("evidence_path_id"))
    chunk_id = _string_or_none(evidence_path.get("chunk_id"))
    diagnostics.extend(_validate_redaction(evidence_path, object_id=evidence_path_id, object_type="evidence_path"))
    if chunk_id not in chunk_ids:
        diagnostics.append(ContractDiagnostic(reason="unresolved_evidence_path", object_id=evidence_path_id, object_type="evidence_path"))
    for element_id in _string_list(evidence_path.get("source_element_ids")):
        if element_id not in element_ids:
            diagnostics.append(ContractDiagnostic(reason="unresolved_parent_element", object_id=evidence_path_id, object_type="evidence_path"))
    if not _valid_source_span(evidence_path.get("source_span")):
        diagnostics.append(ContractDiagnostic(reason="missing_source_span", object_id=evidence_path_id, object_type="evidence_path"))
    return diagnostics


def _validate_redaction(payload: dict[str, Any], *, object_id: str | None, object_type: str) -> list[ContractDiagnostic]:
    diagnostics: list[ContractDiagnostic] = []
    forbidden_present = FORBIDDEN_FIELDS & set(payload)
    for field in sorted(forbidden_present):
        reason = "embedding_leakage" if field in {"embedding", "embeddings"} else "vector_leakage" if field in {"vector", "vectors"} else "raw_text_leakage"
        diagnostics.append(ContractDiagnostic(reason=reason, object_id=object_id, object_type=object_type))
    redaction = payload.get("redaction")
    if isinstance(redaction, dict):
        if redaction.get("raw_text_included") is True or redaction.get("chunk_text_included") is True:
            diagnostics.append(ContractDiagnostic(reason="raw_text_leakage", object_id=object_id, object_type=object_type))
        if redaction.get("embeddings_included") is True:
            diagnostics.append(ContractDiagnostic(reason="embedding_leakage", object_id=object_id, object_type=object_type))
        if redaction.get("vectors_included") is True:
            diagnostics.append(ContractDiagnostic(reason="vector_leakage", object_id=object_id, object_type=object_type))
    return diagnostics


def _is_import_eligible_chunk(chunk: dict[str, Any]) -> bool:
    return (
        chunk.get("state") == GRAPH_READY_STATE
        and TRUSTED_IMPORT_USE in _string_list(chunk.get("allowed_uses"))
        and TRUSTED_IMPORT_USE not in _string_list(chunk.get("excluded_uses"))
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
