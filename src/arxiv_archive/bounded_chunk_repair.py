"""Bounded review-only chunk repair target construction.

This module converts already-redacted deterministic candidate locator batches into
S02 chunk-repair-contract targets. It never reads corpus files, copies payload
text, generates embeddings, writes KG state, or marks anything import-ready.
"""

from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass
from typing import Any

from arxiv_archive.candidate_locators import (
    ALLOWED_USES,
    CANDIDATE_LOCATOR_PROTOCOL_VERSION,
    EXCLUDED_USES,
    default_safety_flags,
    validate_candidate_locator_artifact,
)
from arxiv_archive.chunk_repair_contract import (
    CHUNK_REPAIR_CONTRACT_VERSION,
    REQUIRED_FALSE_SAFETY_FIELDS,
    expected_audit_from_contract,
    scan_forbidden_payload_keys,
    validate_chunk_repair_contract,
)

DEFAULT_MAX_TARGET_COUNT = 6
REQUIRED_FALSE_LOCATOR_SAFETY_FIELDS = tuple(default_safety_flags())
TARGET_COVERAGE_CATEGORIES = (
    "broad_signal_many_matches",
    "overlapping_signal_window",
    "retrieval_or_review_required",
    "method_location",
)


@dataclass(frozen=True)
class BoundedChunkRepairError(ValueError):
    """Redacted fail-closed builder error."""

    code: str
    path: str
    object_id: str | None = None
    object_type: str | None = None

    def __str__(self) -> str:
        parts = [self.code, self.path]
        if self.object_type:
            parts.append(self.object_type)
        if self.object_id:
            parts.append(self.object_id)
        return ":".join(parts)


def build_bounded_chunk_repair_contract(
    contract: dict[str, Any],
    locator_batch: dict[str, Any],
    *,
    max_target_count: int = DEFAULT_MAX_TARGET_COUNT,
) -> dict[str, Any]:
    """Return a review-only repair contract populated with bounded targets.

    Inputs must already be loaded JSON-like dictionaries. The returned payload is
    a deep copy of ``contract`` with deterministic ``repair_targets`` and updated
    diagnostics. All import/write/readiness boundaries remain false.
    """
    _validate_builder_inputs(contract, locator_batch, max_target_count=max_target_count)
    known_ids = _known_contract_ids(contract)
    locators = _list_of_dicts(locator_batch.get("locators"))
    _validate_locator_lineage(locators, known_ids)

    selected = _select_locators(locators, max_target_count=max_target_count)
    if not selected:
        raise BoundedChunkRepairError(code="no_eligible_locators", path="/locators", object_type="locator_batch")

    repaired_contract = deepcopy(contract)
    repaired_contract["schema_version"] = CHUNK_REPAIR_CONTRACT_VERSION
    repaired_contract["contract_version"] = CHUNK_REPAIR_CONTRACT_VERSION
    repaired_contract["repair_targets"] = [_target_from_locator(locator, index=index) for index, locator in enumerate(selected, start=1)]
    repaired_contract["diagnostics"] = _contract_diagnostics(repaired_contract["repair_targets"])

    expected_audit = expected_audit_from_contract(repaired_contract)
    validation = validate_chunk_repair_contract(repaired_contract, expected_audit=expected_audit)
    if not validation.passed:
        first = validation.diagnostics[0]
        raise BoundedChunkRepairError(
            code=f"contract_validation_failed:{first.code}",
            path=first.path,
            object_id=first.object_id,
            object_type=first.object_type,
        )
    return repaired_contract


def summarize_bounded_chunk_repair_contract(payload: dict[str, Any]) -> dict[str, Any]:
    """Return redacted CLI-friendly counts for a populated bounded contract."""
    targets = _list_of_dicts(payload.get("repair_targets"))
    repair_states: dict[str, int] = {}
    route_quality_states: dict[str, int] = {}
    for target in targets:
        repair_state = str(target.get("repair_state", "unknown"))
        route_quality_state = str(target.get("route_quality_state", "unknown"))
        repair_states[repair_state] = repair_states.get(repair_state, 0) + 1
        route_quality_states[route_quality_state] = route_quality_states.get(route_quality_state, 0) + 1
    diagnostics = payload.get("diagnostics") if isinstance(payload.get("diagnostics"), dict) else {}
    return {
        "schema_version": "bounded-chunk-repair-summary.v1",
        "target_count": len(targets),
        "repair_state_counts": dict(sorted(repair_states.items())),
        "route_quality_state_counts": dict(sorted(route_quality_states.items())),
        "unsafe_safety_counters": {
            "import_eligible_count": diagnostics.get("import_eligible_count"),
            "promoted_to_fact_count": diagnostics.get("promoted_to_fact_count"),
            "production_write_count": diagnostics.get("production_write_count"),
            "semantic_ready_count": diagnostics.get("semantic_ready_count"),
            "raw_text_included": diagnostics.get("raw_text_included"),
            "chunk_text_included": diagnostics.get("chunk_text_included"),
            "embeddings_included": diagnostics.get("embeddings_included"),
            "vectors_included": diagnostics.get("vectors_included"),
            "secrets_included": diagnostics.get("secrets_included"),
            "ladybugdb_written": diagnostics.get("ladybugdb_written"),
            "production_import_attempted": diagnostics.get("production_import_attempted"),
        },
    }


def _validate_builder_inputs(contract: dict[str, Any], locator_batch: dict[str, Any], *, max_target_count: int) -> None:
    if not isinstance(contract, dict):
        raise BoundedChunkRepairError(code="contract_not_object", path="/", object_type="contract")
    if not isinstance(locator_batch, dict):
        raise BoundedChunkRepairError(code="locator_batch_not_object", path="/", object_type="locator_batch")
    if max_target_count < 1:
        raise BoundedChunkRepairError(code="invalid_max_target_count", path="/max_target_count", object_type="builder_config")
    if contract.get("schema_version") != CHUNK_REPAIR_CONTRACT_VERSION:
        raise BoundedChunkRepairError(code="contract_schema_mismatch", path="/schema_version", object_type="contract")
    if locator_batch.get("schema_version") != CANDIDATE_LOCATOR_PROTOCOL_VERSION:
        raise BoundedChunkRepairError(code="locator_schema_mismatch", path="/schema_version", object_type="locator_batch")

    for finding in scan_forbidden_payload_keys(locator_batch):
        raise BoundedChunkRepairError(code=finding.code, path=finding.path, object_type="locator_batch")
    for finding in scan_forbidden_payload_keys(contract):
        raise BoundedChunkRepairError(code=finding.code, path=finding.path, object_type="contract")

    locator_diagnostics = validate_candidate_locator_artifact(locator_batch)
    if locator_diagnostics:
        raise BoundedChunkRepairError(
            code=f"locator_validation_failed:{locator_diagnostics[0]}",
            path="/locators",
            object_type="locator_batch",
        )
    _validate_locator_safety(locator_batch)


def _validate_locator_safety(locator_batch: dict[str, Any]) -> None:
    safety_flags = locator_batch.get("safety_flags")
    if not isinstance(safety_flags, dict):
        raise BoundedChunkRepairError(code="missing_safety_flags", path="/safety_flags", object_type="locator_batch")
    for field in REQUIRED_FALSE_LOCATOR_SAFETY_FIELDS:
        if safety_flags.get(field) is not False:
            raise BoundedChunkRepairError(code="unsafe_safety_flag", path=f"/safety_flags/{field}", object_type="locator_batch")
    summary = locator_batch.get("summary") if isinstance(locator_batch.get("summary"), dict) else {}
    for field in ("import_eligible_count", "promoted_to_fact_count"):
        if summary.get(field) != 0:
            raise BoundedChunkRepairError(code="unsafe_summary_counter", path=f"/summary/{field}", object_type="locator_batch")


def _known_contract_ids(contract: dict[str, Any]) -> dict[str, set[str]]:
    stable_ids = contract.get("stable_ids") if isinstance(contract.get("stable_ids"), dict) else {}
    source_ids = _string_set(stable_ids.get("source_ids")) or _source_ids_from_contract(contract)
    locator_ids = _string_set(stable_ids.get("locator_ids")) or _locator_ids_from_contract(contract)
    span_ids = _string_set(stable_ids.get("span_ids")) or _span_ids_from_contract(contract)
    paper_ids = set(expected_audit_from_contract(contract).get("paper_ids") or [])
    if not paper_ids:
        paper_ids = _string_set([contract.get("paper_id")])
    return {"source_ids": source_ids, "locator_ids": locator_ids, "span_ids": span_ids, "paper_ids": paper_ids}


def _validate_locator_lineage(locators: list[dict[str, Any]], known_ids: dict[str, set[str]]) -> None:
    if not locators:
        raise BoundedChunkRepairError(code="missing_locators", path="/locators", object_type="locator_batch")
    for index, locator in enumerate(locators):
        locator_id = str(locator.get("locator_id", ""))
        path = f"/locators/{index}"
        if locator_id not in known_ids["locator_ids"]:
            raise BoundedChunkRepairError(code="unresolved_locator_id", path=f"{path}/locator_id", object_id=locator_id, object_type="locator")
        if str(locator.get("paper_id", "")) not in known_ids["paper_ids"]:
            raise BoundedChunkRepairError(code="unresolved_paper_id", path=f"{path}/paper_id", object_id=locator_id, object_type="locator")
        spans = _list_of_dicts(locator.get("source_spans"))
        if not spans:
            raise BoundedChunkRepairError(code="missing_source_spans", path=f"{path}/source_spans", object_id=locator_id, object_type="locator")
        for span_index, span in enumerate(spans):
            span_id = str(span.get("span_id", ""))
            source_id = str(span.get("source_id", ""))
            if span_id not in known_ids["span_ids"]:
                raise BoundedChunkRepairError(code="unresolved_span_id", path=f"{path}/source_spans/{span_index}/span_id", object_id=span_id, object_type="source_span")
            if source_id not in known_ids["source_ids"]:
                raise BoundedChunkRepairError(code="unresolved_source_id", path=f"{path}/source_spans/{span_index}/source_id", object_id=span_id, object_type="source_span")


def _select_locators(locators: list[dict[str, Any]], *, max_target_count: int) -> list[dict[str, Any]]:
    eligible = [locator for locator in locators if _is_supported_locator(locator)]
    selected: list[dict[str, Any]] = []
    selected_ids: set[str] = set()
    for category in TARGET_COVERAGE_CATEGORIES:
        match = next((locator for locator in sorted(eligible, key=_locator_sort_key) if _locator_matches_category(locator, category)), None)
        if match and match["locator_id"] not in selected_ids:
            selected.append(match)
            selected_ids.add(str(match["locator_id"]))
        if len(selected) >= max_target_count:
            return selected
    for locator in sorted(eligible, key=_locator_sort_key):
        if locator["locator_id"] in selected_ids:
            continue
        selected.append(locator)
        selected_ids.add(str(locator["locator_id"]))
        if len(selected) >= max_target_count:
            break
    return selected


def _target_from_locator(locator: dict[str, Any], *, index: int) -> dict[str, Any]:
    locator_id = str(locator["locator_id"])
    route = str(locator["route"])
    state = str(locator["state"])
    spans = [_span_from_locator_span(span) for span in _list_of_dicts(locator["source_spans"])]
    source_ids = sorted({span["source_id"] for span in spans})
    before_codes = sorted(str(code) for code in locator.get("diagnostic_codes", []) if code is not None)
    return {
        "target_id": f"bounded-repair-target-{index:03d}-{locator_id}",
        "paper_id": str(locator["paper_id"]),
        "locator_id": locator_id,
        "repair_kind": _repair_kind(route=route, state=state),
        "candidate_type": str(locator["candidate_type"]),
        "route": route,
        "state": state,
        "review_status": "pending_review",
        "section_path": ["unresolved_section_lineage"],
        "source_artifact_refs": source_ids,
        "source_spans": spans,
        "diagnostic_codes": before_codes or ["review_required"],
        "allowed_uses": list(ALLOWED_USES),
        "excluded_uses": list(EXCLUDED_USES),
        "safety_boundaries": dict.fromkeys(REQUIRED_FALSE_SAFETY_FIELDS, False),
        "reviewer": None,
        "repair_state": _repair_state(state),
        "route_quality_state": _route_quality_state(locator),
        "before_diagnostics": {
            "source": "candidate_locator_protocol.v1",
            "codes": before_codes,
            "support_level": str(locator.get("support_level", "unknown")),
            "uncertainty_label": str(locator.get("uncertainty_label", "unknown")),
        },
        "after_diagnostics": {
            "state": "pending_human_review",
            "codes": ["bounded_target_created", "kg_import_blocked"],
            "safe_to_import": False,
        },
        "section_lineage": {
            "status": "unresolved",
            "basis": "stable_locator_and_span_ids_only",
            "section_path_proven": False,
        },
    }


def _span_from_locator_span(span: dict[str, Any]) -> dict[str, Any]:
    return {
        "span_id": str(span["span_id"]),
        "source_id": str(span["source_id"]),
        "coordinate_space": str(span["coordinate_space"]),
        "char_start": int(span["char_start"]),
        "char_end": int(span["char_end"]),
        "line_start": span.get("line_start"),
        "line_end": span.get("line_end"),
        "span_hash": str(span["span_hash"]),
        "raw_text_embedded": False,
    }


def _contract_diagnostics(targets: list[dict[str, Any]]) -> dict[str, Any]:
    repair_state_counts: dict[str, int] = {}
    route_quality_counts: dict[str, int] = {}
    for target in targets:
        repair_state_counts[target["repair_state"]] = repair_state_counts.get(target["repair_state"], 0) + 1
        route_quality_counts[target["route_quality_state"]] = route_quality_counts.get(target["route_quality_state"], 0) + 1
    return {
        "target_count": len(targets),
        "pending_review_count": len(targets),
        "accepted_count": 0,
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
        "production_write_count": 0,
        "semantic_ready_count": 0,
        "raw_text_included": False,
        "chunk_text_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "repair_state_counts": dict(sorted(repair_state_counts.items())),
        "route_quality_state_counts": dict(sorted(route_quality_counts.items())),
    }


def _repair_kind(*, route: str, state: str) -> str:
    if route == "retrieval_context":
        return "retrieval_only_review"
    if route == "repair_context" or state in {"ambiguous_span", "missing_span", "conflicting_evidence", "repair_required"}:
        return "chunk_span_repair"
    return "section_route_review"


def _repair_state(state: str) -> str:
    if state in {"retrieval_only", "review_required", "ambiguous_span", "missing_span", "conflicting_evidence", "repair_required"}:
        return state
    return "review_required"


def _route_quality_state(locator: dict[str, Any]) -> str:
    codes = {str(code) for code in locator.get("diagnostic_codes", [])}
    if "broad_signal_many_matches" in codes:
        return "broad_signal_many_matches"
    if "overlapping_signal_window" in codes:
        return "overlapping_signal_window"
    if locator.get("route") == "method_location":
        return "method_location"
    return str(locator.get("state", "review_required"))


def _is_supported_locator(locator: dict[str, Any]) -> bool:
    if locator.get("route") == "repair_context":
        return locator.get("state") in {"repair_required", "ambiguous_span", "missing_span", "conflicting_evidence"}
    if locator.get("route") == "retrieval_context":
        return locator.get("state") in {"retrieval_only", "review_required"}
    return locator.get("state") in {"review_required", "ambiguous_span", "missing_span", "conflicting_evidence"}


def _locator_matches_category(locator: dict[str, Any], category: str) -> bool:
    codes = {str(code) for code in locator.get("diagnostic_codes", [])}
    if category in {"broad_signal_many_matches", "overlapping_signal_window"}:
        return category in codes
    if category == "retrieval_or_review_required":
        return locator.get("route") == "retrieval_context" and locator.get("state") in {"retrieval_only", "review_required"}
    if category == "method_location":
        return locator.get("route") == "method_location"
    return False


def _locator_sort_key(locator: dict[str, Any]) -> tuple[str, str, str, str]:
    return (
        str(locator.get("paper_id", "")),
        str(locator.get("route", "")),
        str(locator.get("state", "")),
        str(locator.get("locator_id", "")),
    )


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _string_set(value: Any) -> set[str]:
    if not isinstance(value, list | tuple | set):
        return set()
    return {str(item) for item in value if item is not None and str(item)}


def _source_ids_from_contract(contract: dict[str, Any]) -> set[str]:
    return {str(item["source_id"]) for item in _list_of_dicts(contract.get("source_ledger")) if item.get("source_id")}


def _locator_ids_from_contract(contract: dict[str, Any]) -> set[str]:
    return {str(target["locator_id"]) for target in _list_of_dicts(contract.get("repair_targets")) if target.get("locator_id")}


def _span_ids_from_contract(contract: dict[str, Any]) -> set[str]:
    span_ids: set[str] = set()
    for target in _list_of_dicts(contract.get("repair_targets")):
        for span in _list_of_dicts(target.get("source_spans")):
            if span.get("span_id"):
                span_ids.add(str(span["span_id"]))
    return span_ids


__all__ = [
    "BoundedChunkRepairError",
    "build_bounded_chunk_repair_contract",
    "summarize_bounded_chunk_repair_contract",
]
