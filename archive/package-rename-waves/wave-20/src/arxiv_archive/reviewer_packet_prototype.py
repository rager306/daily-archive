# Formerly: src/arxiv_archive/reviewer_packet_prototype.py

"""Review-only reviewer packet prototype construction.

This module turns an already validated S03 bounded repair prototype into S04
reviewer packets plus an independent deterministic assessment. It is purposely
side-effect free: callers provide JSON-like dictionaries and receive JSON-like
artifacts or redacted fail-closed errors. It never reads corpus payloads,
embeds text, imports KG facts, or writes production storage.
"""

from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass
from typing import Any

from arxiv_archive.chunk_repair_contract import (
    EXCLUDED_USES,
    REQUIRED_FALSE_SAFETY_FIELDS,
    expected_audit_from_contract,
    scan_forbidden_payload_keys,
    validate_chunk_repair_contract,
    validate_chunk_repair_contract_markdown,
)

REVIEWER_PACKET_PROTOTYPE_VERSION = "reviewer-packet-prototype.v1"
REVIEWER_PACKET_ASSESSMENT_VERSION = "reviewer-packet-assessment.v1"
REVIEWER_ID = "independent-agent"
ALLOWED_NON_IMPORTING_DECISIONS = (
    "request_human_semantic_review",
    "request_span_boundary_repair",
    "keep_pending_non_importable",
    "reject_for_kg_import",
)
ASSESSMENT_DIMENSIONS = (
    "semantic_usefulness",
    "safety_redaction",
    "reproducibility",
    "next_step_readiness",
)
UNSAFE_COUNTER_FIELDS = (
    "import_eligible_count",
    "promoted_to_fact_count",
    "production_write_count",
    "semantic_ready_count",
    "raw_text_included",
    "chunk_text_included",
    "embeddings_included",
    "vectors_included",
    "secrets_included",
    "ladybugdb_written",
    "production_import_attempted",
)
MARKDOWN_TITLE = "# S04 Reviewer Packet Prototype"


@dataclass(frozen=True)
class ReviewerPacketError(ValueError):
    """Redacted fail-closed reviewer packet builder error."""

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


def build_reviewer_packet_prototype(
    repair_payload: dict[str, Any],
    *,
    s02_contract: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build deterministic review-only packets and assessment from S03 payload.

    ``repair_payload`` must be a populated S03 chunk repair contract. It is
    validated using the existing S02/S03 contract validator with
    ``expected_audit_from_contract(repair_payload)`` before any packets are
    returned. When ``s02_contract`` is provided, locator/source/span/paper IDs
    must be a subset of the S02 stable IDs; unresolved IDs produce redacted
    code/path errors rather than inferred identifiers.
    """
    _validate_inputs(repair_payload, s02_contract)
    targets = _list_of_dicts(repair_payload.get("repair_targets"))
    if not targets:
        raise ReviewerPacketError(code="empty_repair_targets", path="/repair_targets", object_type="repair_payload")

    packets = [_packet_from_target(target, index=index) for index, target in enumerate(targets, start=1)]
    assessment = build_reviewer_packet_assessment(packets)
    prototype = {
        "schema_version": REVIEWER_PACKET_PROTOTYPE_VERSION,
        "source_schema_version": str(repair_payload.get("schema_version")),
        "run_id": str(repair_payload.get("run_id", "not-recorded")),
        "paper_id": str(repair_payload.get("paper_id", "not-recorded")),
        "packet_count": len(packets),
        "packets": packets,
        "assessment": assessment,
        "diagnostics": _prototype_diagnostics(packets, assessment),
        "safety_boundaries": _copied_global_false_boundaries(repair_payload),
    }
    _validate_prototype_safety(prototype)
    return prototype


def build_reviewer_packet_assessment(packets: list[dict[str, Any]]) -> dict[str, Any]:
    """Build a deterministic independent assessment for packet readiness."""
    if not isinstance(packets, list) or not packets:
        raise ReviewerPacketError(code="empty_packets", path="/packets", object_type="assessment")

    findings: list[dict[str, Any]] = []
    for index, packet in enumerate(packets):
        path = f"/packets/{index}"
        packet_id = str(packet.get("packet_id", f"packet-{index + 1}"))
        review_status = packet.get("review_status")
        importable = packet.get("importable")
        semantic_ready = packet.get("semantic_ready_for_kg")
        if review_status != "pending_review":
            findings.append(_finding("packet_not_pending_review", path=f"{path}/review_status", packet_id=packet_id))
        if importable is not False:
            findings.append(_finding("packet_importable", path=f"{path}/importable", packet_id=packet_id))
        if semantic_ready is not False:
            findings.append(_finding("packet_semantic_ready", path=f"{path}/semantic_ready_for_kg", packet_id=packet_id))
        if packet.get("raw_text_embedded") is not False:
            findings.append(_finding("packet_raw_payload_embedded", path=f"{path}/raw_text_embedded", packet_id=packet_id))
        if not packet.get("review_questions"):
            findings.append(_finding("packet_missing_review_questions", path=f"{path}/review_questions", packet_id=packet_id))
        if _unsafe_boundary_true(packet.get("safety_boundaries")):
            findings.append(_finding("packet_unsafe_safety_boundary", path=f"{path}/safety_boundaries", packet_id=packet_id))

    pending_count = sum(1 for packet in packets if packet.get("review_status") == "pending_review")
    verdict = "continue_repair" if findings else "blocked_pending_semantic_acceptance"
    return {
        "schema_version": REVIEWER_PACKET_ASSESSMENT_VERSION,
        "reviewer_id": REVIEWER_ID,
        "verdict": verdict,
        "dimension_results": {
            "semantic_usefulness": {
                "status": "needs_human_semantic_acceptance",
                "blocks_import": True,
                "finding_codes": ["pending_semantic_acceptance"],
            },
            "safety_redaction": {
                "status": "passed",
                "blocks_import": False,
                "finding_codes": [],
            },
            "reproducibility": {
                "status": "passed",
                "blocks_import": False,
                "finding_codes": [],
            },
            "next_step_readiness": {
                "status": "blocked_pending_semantic_acceptance",
                "blocks_import": True,
                "finding_codes": ["human_review_required_before_import"],
            },
        },
        "packet_findings": findings or [_finding("pending_semantic_acceptance", path="/packets", packet_id="all")],
        "unsafe_counters": {
            "packet_count": len(packets),
            "pending_review_count": pending_count,
            "accepted_count": sum(1 for packet in packets if packet.get("review_status") == "accepted"),
            "importable_count": sum(1 for packet in packets if packet.get("importable") is True),
            "semantic_ready_count": sum(1 for packet in packets if packet.get("semantic_ready_for_kg") is True),
            "raw_text_embedded_count": sum(1 for packet in packets if packet.get("raw_text_embedded") is True),
            "unsafe_safety_boundary_count": sum(1 for packet in packets if _unsafe_boundary_true(packet.get("safety_boundaries"))),
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "secrets_included": False,
            "embeddings_included": False,
            "vectors_included": False,
        },
        "import_allowed": False,
        "semantic_ready_for_kg": False,
        "next_step": "human semantic review or bounded repair iteration only",
    }


def summarize_reviewer_packet_prototype(prototype: dict[str, Any]) -> dict[str, Any]:
    """Return redacted CLI-friendly counts for reviewer packet artifacts."""
    packets = _list_of_dicts(prototype.get("packets"))
    route_counts: dict[str, int] = {}
    repair_counts: dict[str, int] = {}
    route_quality_counts: dict[str, int] = {}
    for packet in packets:
        _increment(route_counts, str(packet.get("route", "unknown")))
        _increment(repair_counts, str(packet.get("repair_state", "unknown")))
        _increment(route_quality_counts, str(packet.get("route_quality_state", "unknown")))
    assessment = prototype.get("assessment") if isinstance(prototype.get("assessment"), dict) else {}
    return {
        "schema_version": "reviewer-packet-summary.v1",
        "packet_count": len(packets),
        "review_status_counts": _count_by(packets, "review_status"),
        "route_counts": dict(sorted(route_counts.items())),
        "repair_state_counts": dict(sorted(repair_counts.items())),
        "route_quality_state_counts": dict(sorted(route_quality_counts.items())),
        "assessment_verdict": assessment.get("verdict", "missing"),
        "unsafe_counters": deepcopy(assessment.get("unsafe_counters", {})),
    }


def render_reviewer_packet_markdown(prototype: dict[str, Any]) -> str:
    """Render JSON-derived Markdown without code fences or payload markers."""
    _validate_prototype_safety(prototype)
    summary = summarize_reviewer_packet_prototype(prototype)
    assessment = prototype["assessment"]
    lines = [
        MARKDOWN_TITLE,
        "",
        "This review-only packet set contains stable identifiers, coordinate ranges, hashes, diagnostic codes, review questions, and safety decisions only. It does not authorize KG import, fact promotion, semantic readiness, production writes, source payload copying, model payloads, or secret material.",
        "",
        "## Summary",
        "",
        f"- Packet schema: {prototype['schema_version']}",
        f"- Packet count: {summary['packet_count']}",
        f"- Assessment schema: {assessment['schema_version']}",
        f"- Independent reviewer: {assessment['reviewer_id']}",
        f"- Assessment verdict: {assessment['verdict']}",
        f"- Import allowed: {str(assessment['import_allowed']).lower()}",
        f"- Semantic KG readiness: {str(assessment['semantic_ready_for_kg']).lower()}",
        "",
        "## Review Status Counts",
        "",
        "| Status | Count |",
        "|---|---:|",
    ]
    for status, count in summary["review_status_counts"].items():
        lines.append(f"| {status} | {count} |")
    lines.extend(["", "## Route Quality Counts", "", "| Route quality | Count |", "|---|---:|"])
    for state, count in summary["route_quality_state_counts"].items():
        lines.append(f"| {state} | {count} |")
    lines.extend(["", "## Unsafe Counter Status", ""])
    counters = assessment["unsafe_counters"]
    lines.extend(
        [
            f"- Importable packets: {counters['importable_count']}",
            f"- Semantic ready packets: {counters['semantic_ready_count']}",
            f"- Source payload embedded packets: {counters['raw_text_embedded_count']}",
            f"- Unsafe safety boundary packets: {counters['unsafe_safety_boundary_count']}",
            f"- LadybugDB write attempted: {str(counters['ladybugdb_written']).lower()}",
            f"- Production import attempted: {str(counters['production_import_attempted']).lower()}",
            f"- Secret values included: {str(counters['secrets_included']).lower()}",
            f"- Model embedding payloads included: {str(counters['embeddings_included']).lower()}",
            f"- Model vector payloads included: {str(counters['vectors_included']).lower()}",
            "",
            "## Packets",
            "",
        ]
    )
    for packet in _list_of_dicts(prototype.get("packets")):
        before_codes = ", ".join(packet["before_diagnostic_codes"]) or "none"
        after_codes = ", ".join(packet["after_diagnostic_codes"]) or "none"
        span_refs = ", ".join(span["span_id"] for span in _list_of_dicts(packet.get("span_refs"))) or "none"
        source_refs = ", ".join(packet.get("source_refs", [])) or "none"
        questions = "; ".join(packet.get("review_questions", []))
        decisions = ", ".join(packet.get("allowed_non_importing_decisions", []))
        lines.extend(
            [
                f"### {packet['packet_id']}",
                "",
                f"- Target ID: {packet['target_id']}",
                f"- Paper ID: {packet['paper_id']}",
                f"- Locator ID: {packet['locator_id']}",
                f"- Route: {packet['route']}",
                f"- State: {packet['state']}",
                f"- Repair state: {packet['repair_state']}",
                f"- Route quality: {packet['route_quality_state']}",
                f"- Review status: {packet['review_status']}",
                f"- Importable: {str(packet['importable']).lower()}",
                f"- Source refs: {source_refs}",
                f"- Span refs: {span_refs}",
                f"- Before diagnostic codes: {before_codes}",
                f"- After diagnostic codes: {after_codes}",
                f"- Section lineage status: {packet['section_lineage']['status']}",
                f"- Allowed non-importing decisions: {decisions}",
                f"- Review questions: {questions}",
                "",
            ]
        )
    lines.extend(["## Assessment Findings", ""])
    for finding in _list_of_dicts(assessment.get("packet_findings")):
        lines.append(f"- {finding['code']} at {finding['path']} for {finding['packet_id']}")
    lines.append("")
    markdown = "\n".join(lines)
    diagnostics = validate_chunk_repair_contract_markdown(markdown)
    if diagnostics:
        first = diagnostics[0]
        raise ReviewerPacketError(
            code=f"markdown_validation_failed:{first.code}",
            path=first.path,
            object_id=first.object_id,
            object_type=first.object_type,
        )
    return markdown


def _validate_inputs(repair_payload: dict[str, Any], s02_contract: dict[str, Any] | None) -> None:
    if not isinstance(repair_payload, dict):
        raise ReviewerPacketError(code="repair_payload_not_object", path="/", object_type="repair_payload")
    if s02_contract is not None and not isinstance(s02_contract, dict):
        raise ReviewerPacketError(code="s02_contract_not_object", path="/", object_type="s02_contract")
    for finding in scan_forbidden_payload_keys(repair_payload):
        raise ReviewerPacketError(code=finding.code, path=finding.path, object_type="repair_payload")
    if s02_contract is not None:
        for finding in scan_forbidden_payload_keys(s02_contract):
            raise ReviewerPacketError(code=finding.code, path=finding.path, object_type="s02_contract")
    if isinstance(repair_payload.get("repair_targets"), list) and not repair_payload.get("repair_targets"):
        raise ReviewerPacketError(code="empty_repair_targets", path="/repair_targets", object_type="repair_payload")

    validation = validate_chunk_repair_contract(repair_payload, expected_audit=expected_audit_from_contract(repair_payload))
    if not validation.passed:
        first = validation.diagnostics[0]
        raise ReviewerPacketError(
            code=f"contract_validation_failed:{first.code}",
            path=first.path,
            object_id=first.object_id,
            object_type=first.object_type,
        )
    _validate_zero_unsafe_contract_counters(repair_payload)
    if s02_contract is not None:
        _validate_s02_subset(repair_payload, s02_contract)


def _validate_s02_subset(repair_payload: dict[str, Any], s02_contract: dict[str, Any]) -> None:
    known = _known_ids(s02_contract)
    for target_index, target in enumerate(_list_of_dicts(repair_payload.get("repair_targets"))):
        target_id = str(target.get("target_id", ""))
        base = f"/repair_targets/{target_index}"
        if str(target.get("paper_id", "")) not in known["paper_ids"]:
            raise ReviewerPacketError(code="paper_id_not_in_s02_stable_ids", path=f"{base}/paper_id", object_id=target_id, object_type="repair_target")
        if str(target.get("locator_id", "")) not in known["locator_ids"]:
            raise ReviewerPacketError(code="locator_id_not_in_s02_stable_ids", path=f"{base}/locator_id", object_id=target_id, object_type="repair_target")
        for source_index, source_id in enumerate(target.get("source_artifact_refs", [])):
            if str(source_id) not in known["source_ids"]:
                raise ReviewerPacketError(code="source_id_not_in_s02_stable_ids", path=f"{base}/source_artifact_refs/{source_index}", object_id=target_id, object_type="repair_target")
        for span_index, span in enumerate(_list_of_dicts(target.get("source_spans"))):
            span_id = str(span.get("span_id", ""))
            if span_id not in known["span_ids"]:
                raise ReviewerPacketError(code="span_id_not_in_s02_stable_ids", path=f"{base}/source_spans/{span_index}/span_id", object_id=span_id, object_type="source_span")
            if str(span.get("source_id", "")) not in known["source_ids"]:
                raise ReviewerPacketError(code="source_id_not_in_s02_stable_ids", path=f"{base}/source_spans/{span_index}/source_id", object_id=span_id, object_type="source_span")


def _packet_from_target(target: dict[str, Any], *, index: int) -> dict[str, Any]:
    target_id = str(target["target_id"])
    before = target.get("before_diagnostics") if isinstance(target.get("before_diagnostics"), dict) else {}
    after = target.get("after_diagnostics") if isinstance(target.get("after_diagnostics"), dict) else {}
    section_lineage = target.get("section_lineage") if isinstance(target.get("section_lineage"), dict) else {}
    return {
        "packet_id": f"reviewer-packet-{index:03d}-{target_id}",
        "target_id": target_id,
        "paper_id": str(target["paper_id"]),
        "locator_id": str(target["locator_id"]),
        "route": str(target["route"]),
        "state": str(target["state"]),
        "repair_state": str(target.get("repair_state", target.get("state"))),
        "route_quality_state": str(target.get("route_quality_state", target.get("state"))),
        "review_status": str(target["review_status"]),
        "importable": False,
        "semantic_ready_for_kg": False,
        "raw_text_embedded": False,
        "source_refs": sorted(str(source_id) for source_id in target.get("source_artifact_refs", [])),
        "span_refs": [_span_ref(span) for span in _list_of_dicts(target.get("source_spans"))],
        "section_lineage": {
            "status": str(section_lineage.get("status", "unresolved")),
            "basis": str(section_lineage.get("basis", "stable_locator_and_span_ids_only")),
            "section_path_proven": section_lineage.get("section_path_proven") is True,
            "section_path_labels": [str(label) for label in target.get("section_path", [])],
        },
        "before_diagnostic_codes": sorted(str(code) for code in before.get("codes", target.get("diagnostic_codes", []))),
        "after_diagnostic_codes": sorted(str(code) for code in after.get("codes", ["kg_import_blocked"])),
        "review_questions": _review_questions(target),
        "allowed_non_importing_decisions": list(ALLOWED_NON_IMPORTING_DECISIONS),
        "excluded_uses": list(EXCLUDED_USES),
        "safety_boundaries": _copied_false_safety_boundaries(target),
    }


def _span_ref(span: dict[str, Any]) -> dict[str, Any]:
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


def _review_questions(target: dict[str, Any]) -> list[str]:
    state = str(target.get("state", "review_required"))
    questions = [
        "Do the stable span coordinates and hash identify the intended evidence region without reading source payload text from this packet?",
        "Should this target stay pending, be rejected for KG import, or be sent to another bounded repair iteration?",
    ]
    if state in {"ambiguous_span", "missing_span", "conflicting_evidence", "repair_required"}:
        questions.append("What non-importing repair action is needed before a semantic acceptance decision?")
    else:
        questions.append("Is retrieval-only context sufficient for reviewer navigation while remaining non-importable?")
    return questions


def _prototype_diagnostics(packets: list[dict[str, Any]], assessment: dict[str, Any]) -> dict[str, Any]:
    return {
        "packet_count": len(packets),
        "pending_review_count": sum(1 for packet in packets if packet.get("review_status") == "pending_review"),
        "assessment_verdict": assessment["verdict"],
        "route_counts": _count_by(packets, "route"),
        "repair_state_counts": _count_by(packets, "repair_state"),
        "route_quality_state_counts": _count_by(packets, "route_quality_state"),
        "unsafe_counters": deepcopy(assessment["unsafe_counters"]),
    }


def _validate_zero_unsafe_contract_counters(payload: dict[str, Any]) -> None:
    diagnostics = payload.get("diagnostics") if isinstance(payload.get("diagnostics"), dict) else {}
    expected_zero_or_false = {
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
    }
    for field, expected in expected_zero_or_false.items():
        if diagnostics.get(field) != expected:
            raise ReviewerPacketError(code="unsafe_contract_counter", path=f"/diagnostics/{field}", object_type="repair_payload")


def _validate_prototype_safety(prototype: dict[str, Any]) -> None:
    if not isinstance(prototype, dict):
        raise ReviewerPacketError(code="prototype_not_object", path="/", object_type="prototype")
    if prototype.get("schema_version") != REVIEWER_PACKET_PROTOTYPE_VERSION:
        raise ReviewerPacketError(code="prototype_schema_mismatch", path="/schema_version", object_type="prototype")
    for finding in scan_forbidden_payload_keys(prototype):
        raise ReviewerPacketError(code=finding.code, path=finding.path, object_type="prototype")
    packets = _list_of_dicts(prototype.get("packets"))
    if not packets:
        raise ReviewerPacketError(code="missing_packets", path="/packets", object_type="prototype")
    for index, packet in enumerate(packets):
        base = f"/packets/{index}"
        if packet.get("review_status") != "pending_review":
            raise ReviewerPacketError(code="packet_not_pending_review", path=f"{base}/review_status", object_id=str(packet.get("packet_id", "")), object_type="packet")
        if packet.get("importable") is not False:
            raise ReviewerPacketError(code="packet_importable", path=f"{base}/importable", object_id=str(packet.get("packet_id", "")), object_type="packet")
        if packet.get("semantic_ready_for_kg") is not False:
            raise ReviewerPacketError(code="packet_semantic_ready", path=f"{base}/semantic_ready_for_kg", object_id=str(packet.get("packet_id", "")), object_type="packet")
        if packet.get("raw_text_embedded") is not False:
            raise ReviewerPacketError(code="packet_raw_payload_embedded", path=f"{base}/raw_text_embedded", object_id=str(packet.get("packet_id", "")), object_type="packet")
        if _unsafe_boundary_true(packet.get("safety_boundaries")):
            raise ReviewerPacketError(code="packet_unsafe_safety_boundary", path=f"{base}/safety_boundaries", object_id=str(packet.get("packet_id", "")), object_type="packet")
    assessment = prototype.get("assessment") if isinstance(prototype.get("assessment"), dict) else {}
    counters = assessment.get("unsafe_counters") if isinstance(assessment.get("unsafe_counters"), dict) else {}
    for field in ("accepted_count", "importable_count", "semantic_ready_count", "raw_text_embedded_count", "unsafe_safety_boundary_count"):
        if counters.get(field) != 0:
            raise ReviewerPacketError(code="unsafe_assessment_counter", path=f"/assessment/unsafe_counters/{field}", object_type="assessment")
    for field in ("production_import_attempted", "ladybugdb_written", "secrets_included", "embeddings_included", "vectors_included"):
        if counters.get(field) is not False:
            raise ReviewerPacketError(code="unsafe_assessment_counter", path=f"/assessment/unsafe_counters/{field}", object_type="assessment")


def _copied_false_safety_boundaries(target: dict[str, Any]) -> dict[str, bool]:
    safety = target.get("safety_boundaries") if isinstance(target.get("safety_boundaries"), dict) else {}
    copied: dict[str, bool] = {}
    for field in REQUIRED_FALSE_SAFETY_FIELDS:
        if safety.get(field) is not False:
            raise ReviewerPacketError(code="unsafe_target_safety_boundary", path=f"/safety_boundaries/{field}", object_id=str(target.get("target_id", "")), object_type="repair_target")
        copied[field] = False
    return copied


def _copied_global_false_boundaries(repair_payload: dict[str, Any]) -> dict[str, Any]:
    diagnostics = repair_payload.get("diagnostics") if isinstance(repair_payload.get("diagnostics"), dict) else {}
    return {field: diagnostics.get(field) for field in UNSAFE_COUNTER_FIELDS}


def _unsafe_boundary_true(value: Any) -> bool:
    if not isinstance(value, dict):
        return True
    return any(value.get(field) is not False for field in REQUIRED_FALSE_SAFETY_FIELDS)


def _known_ids(contract: dict[str, Any]) -> dict[str, set[str]]:
    stable = contract.get("stable_ids") if isinstance(contract.get("stable_ids"), dict) else {}
    source_ids = _string_set(stable.get("source_ids")) or {str(item["source_id"]) for item in _list_of_dicts(contract.get("source_ledger")) if item.get("source_id")}
    locator_ids = _string_set(stable.get("locator_ids"))
    span_ids = _string_set(stable.get("span_ids"))
    for target in _list_of_dicts(contract.get("repair_targets")):
        if target.get("locator_id"):
            locator_ids.add(str(target["locator_id"]))
        for span in _list_of_dicts(target.get("source_spans")):
            if span.get("span_id"):
                span_ids.add(str(span["span_id"]))
    paper_ids = set(expected_audit_from_contract(contract).get("paper_ids") or [])
    if contract.get("paper_id"):
        paper_ids.add(str(contract["paper_id"]))
    return {"source_ids": source_ids, "locator_ids": locator_ids, "span_ids": span_ids, "paper_ids": paper_ids}


def _finding(code: str, *, path: str, packet_id: str) -> dict[str, Any]:
    return {"code": code, "path": path, "packet_id": packet_id, "blocks_import": True}


def _count_by(items: list[dict[str, Any]], field: str) -> dict[str, int]:
    counts: dict[str, int] = {}
    for item in items:
        _increment(counts, str(item.get(field, "unknown")))
    return dict(sorted(counts.items()))


def _increment(counts: dict[str, int], key: str) -> None:
    counts[key] = counts.get(key, 0) + 1


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _string_set(value: Any) -> set[str]:
    if not isinstance(value, list | tuple | set):
        return set()
    return {str(item) for item in value if item is not None and str(item)}


__all__ = [
    "ALLOWED_NON_IMPORTING_DECISIONS",
    "REVIEWER_PACKET_ASSESSMENT_VERSION",
    "REVIEWER_PACKET_PROTOTYPE_VERSION",
    "ReviewerPacketError",
    "build_reviewer_packet_assessment",
    "build_reviewer_packet_prototype",
    "render_reviewer_packet_markdown",
    "summarize_reviewer_packet_prototype",
]
