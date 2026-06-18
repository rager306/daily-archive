#!/usr/bin/env python3
"""Verify M022/S04 reviewer packet prototype JSON and Markdown artifacts.

The verifier is read-only. It checks reviewer packet schema, packet count,
pending/non-importable review states, assessment verdict, zero unsafe counters,
forbidden payload keys, Markdown redaction/fence boundaries, and S02 lineage
subset consistency against both the generated packet artifacts and S03 source.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))

from research_graph.repair.chunk_repair_contract import (  # noqa: E402
    MARKDOWN_FORBIDDEN_PATTERNS,
    expected_audit_from_contract,
    scan_forbidden_payload_keys,
    validate_chunk_repair_contract,
    validate_chunk_repair_contract_markdown,
)
from research_graph.workflows.review_packet_prototype import (  # noqa: E402
    REVIEWER_PACKET_ASSESSMENT_VERSION,
    REVIEWER_PACKET_PROTOTYPE_VERSION,
    summarize_reviewer_packet_prototype,
)


@dataclass(frozen=True)
class ReviewerPacketVerificationFinding:
    """One redacted verifier finding."""

    code: str
    path: str
    object_type: str | None = None
    object_id: str | None = None


class ReviewerPacketPrototypeVerifyError(ValueError):
    """Raised when verifier inputs are unreadable or malformed."""


def load_json_object(path: str | Path, *, label: str) -> dict[str, Any]:
    """Load one JSON object with redacted parse diagnostics."""
    json_path = Path(path)
    if not json_path.exists():
        raise FileNotFoundError(f"{label} file not found: {json_path}")
    try:
        payload = json.loads(json_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ReviewerPacketPrototypeVerifyError(f"{label} JSON is malformed at line {exc.lineno} column {exc.colno}") from exc
    if not isinstance(payload, dict):
        raise ReviewerPacketPrototypeVerifyError(f"{label} root must be a JSON object")
    return payload


def verify_reviewer_packet_prototype(
    prototype: dict[str, Any],
    packet_markdown: str,
    assessment: dict[str, Any],
    assessment_markdown: str,
    *,
    repair_prototype: dict[str, Any],
    s02_contract: dict[str, Any],
) -> list[ReviewerPacketVerificationFinding]:
    """Return redacted safety, schema, Markdown, and lineage findings."""
    findings: list[ReviewerPacketVerificationFinding] = []
    packets = _list_of_dicts(prototype.get("packets"))

    findings.extend(_verify_s03_source(repair_prototype))
    findings.extend(_verify_packet_payload(prototype, packets))
    findings.extend(_verify_assessment_payload(assessment, packet_count=len(packets)))
    if prototype.get("assessment") != assessment:
        findings.append(ReviewerPacketVerificationFinding(code="assessment_json_mismatch", path="/assessment", object_type="assessment"))
    findings.extend(_markdown_findings(packet_markdown, base_path="/packet_markdown"))
    findings.extend(_markdown_findings(assessment_markdown, base_path="/assessment_markdown"))
    findings.extend(_lineage_subset_findings(prototype, repair_prototype, s02_contract))
    return findings


def verify_files(
    prototype_path: Path,
    markdown_path: Path,
    assessment_json_path: Path,
    assessment_markdown_path: Path,
    repair_prototype_path: Path,
    s02_contract_path: Path,
) -> dict[str, Any]:
    """Read and verify reviewer packet artifacts."""
    prototype = load_json_object(prototype_path, label="reviewer packet prototype")
    assessment = load_json_object(assessment_json_path, label="reviewer packet assessment")
    repair_prototype = load_json_object(repair_prototype_path, label="repair prototype")
    s02_contract = load_json_object(s02_contract_path, label="S02 contract")
    if not markdown_path.exists():
        raise FileNotFoundError(f"reviewer packet Markdown file not found: {markdown_path}")
    if not assessment_markdown_path.exists():
        raise FileNotFoundError(f"assessment Markdown file not found: {assessment_markdown_path}")
    packet_markdown = markdown_path.read_text(encoding="utf-8")
    assessment_markdown = assessment_markdown_path.read_text(encoding="utf-8")
    findings = verify_reviewer_packet_prototype(
        prototype,
        packet_markdown,
        assessment,
        assessment_markdown,
        repair_prototype=repair_prototype,
        s02_contract=s02_contract,
    )
    summary = summarize_reviewer_packet_prototype(prototype) if isinstance(prototype, dict) else {}
    counters = assessment.get("unsafe_counters") if isinstance(assessment.get("unsafe_counters"), dict) else {}
    return {
        "passed": not findings,
        "packet_count": len(_list_of_dicts(prototype.get("packets"))),
        "review_status_counts": summary.get("review_status_counts", {}),
        "repair_state_counts": summary.get("repair_state_counts", {}),
        "route_quality_state_counts": summary.get("route_quality_state_counts", {}),
        "assessment_verdict": assessment.get("verdict", "missing"),
        "unsafe_counters_zero": _unsafe_counters_zero(counters),
        "findings": [finding.__dict__ for finding in findings],
    }


def _verify_s03_source(repair_prototype: dict[str, Any]) -> list[ReviewerPacketVerificationFinding]:
    findings: list[ReviewerPacketVerificationFinding] = []
    validation = validate_chunk_repair_contract(repair_prototype, expected_audit=expected_audit_from_contract(repair_prototype))
    if not validation.passed:
        findings.extend(
            ReviewerPacketVerificationFinding(
                code=f"repair_contract_validation_failed:{diagnostic.code}",
                path=diagnostic.path,
                object_type=diagnostic.object_type,
                object_id=diagnostic.object_id,
            )
            for diagnostic in validation.diagnostics
        )
    if validation.target_count <= 0:
        findings.append(ReviewerPacketVerificationFinding(code="empty_repair_targets", path="/repair_targets", object_type="repair_prototype"))
    return findings


def _verify_packet_payload(prototype: dict[str, Any], packets: list[dict[str, Any]]) -> list[ReviewerPacketVerificationFinding]:
    findings: list[ReviewerPacketVerificationFinding] = []
    if prototype.get("schema_version") != REVIEWER_PACKET_PROTOTYPE_VERSION:
        findings.append(ReviewerPacketVerificationFinding(code="packet_schema_mismatch", path="/schema_version", object_type="prototype"))
    for forbidden in scan_forbidden_payload_keys(prototype):
        findings.append(ReviewerPacketVerificationFinding(code=forbidden.code, path=forbidden.path, object_type="prototype"))
    if prototype.get("packet_count") != len(packets):
        findings.append(ReviewerPacketVerificationFinding(code="packet_count_mismatch", path="/packet_count", object_type="prototype"))
    if not packets:
        findings.append(ReviewerPacketVerificationFinding(code="empty_packets", path="/packets", object_type="prototype"))
    for index, packet in enumerate(packets):
        packet_id = str(packet.get("packet_id", ""))
        base = f"/packets/{index}"
        if packet.get("review_status") != "pending_review":
            findings.append(ReviewerPacketVerificationFinding(code="packet_not_pending_review", path=f"{base}/review_status", object_type="packet", object_id=packet_id))
        if packet.get("importable") is not False:
            findings.append(ReviewerPacketVerificationFinding(code="packet_importable", path=f"{base}/importable", object_type="packet", object_id=packet_id))
        if packet.get("semantic_ready_for_kg") is not False:
            findings.append(ReviewerPacketVerificationFinding(code="packet_semantic_ready", path=f"{base}/semantic_ready_for_kg", object_type="packet", object_id=packet_id))
        if packet.get("raw_text_embedded") is not False:
            findings.append(ReviewerPacketVerificationFinding(code="packet_raw_payload_embedded", path=f"{base}/raw_text_embedded", object_type="packet", object_id=packet_id))
        safety = packet.get("safety_boundaries") if isinstance(packet.get("safety_boundaries"), dict) else {}
        if any(value is not False for value in safety.values()) or not safety:
            findings.append(ReviewerPacketVerificationFinding(code="packet_unsafe_safety_boundary", path=f"{base}/safety_boundaries", object_type="packet", object_id=packet_id))
    return findings


def _verify_assessment_payload(assessment: dict[str, Any], *, packet_count: int) -> list[ReviewerPacketVerificationFinding]:
    findings: list[ReviewerPacketVerificationFinding] = []
    if assessment.get("schema_version") != REVIEWER_PACKET_ASSESSMENT_VERSION:
        findings.append(ReviewerPacketVerificationFinding(code="assessment_schema_mismatch", path="/schema_version", object_type="assessment"))
    for forbidden in scan_forbidden_payload_keys(assessment):
        findings.append(ReviewerPacketVerificationFinding(code=forbidden.code, path=forbidden.path, object_type="assessment"))
    if assessment.get("verdict") in {"accepted", "accepting", "accepted_for_import", "import_ready", "importing"}:
        findings.append(ReviewerPacketVerificationFinding(code="assessment_unsafe_verdict", path="/verdict", object_type="assessment"))
    if assessment.get("import_allowed") is not False:
        findings.append(ReviewerPacketVerificationFinding(code="assessment_import_allowed", path="/import_allowed", object_type="assessment"))
    if assessment.get("semantic_ready_for_kg") is not False:
        findings.append(ReviewerPacketVerificationFinding(code="assessment_semantic_ready", path="/semantic_ready_for_kg", object_type="assessment"))
    counters = assessment.get("unsafe_counters") if isinstance(assessment.get("unsafe_counters"), dict) else {}
    if counters.get("packet_count") != packet_count:
        findings.append(ReviewerPacketVerificationFinding(code="assessment_packet_count_mismatch", path="/unsafe_counters/packet_count", object_type="assessment"))
    if not _unsafe_counters_zero(counters):
        for field, value in counters.items():
            if field in {"accepted_count", "importable_count", "semantic_ready_count", "raw_text_embedded_count", "unsafe_safety_boundary_count"} and value != 0:
                findings.append(ReviewerPacketVerificationFinding(code="unsafe_assessment_counter", path=f"/unsafe_counters/{field}", object_type="assessment"))
            if field in {"production_import_attempted", "ladybugdb_written", "secrets_included", "embeddings_included", "vectors_included"} and value is not False:
                findings.append(ReviewerPacketVerificationFinding(code="unsafe_assessment_counter", path=f"/unsafe_counters/{field}", object_type="assessment"))
    return findings


def _markdown_findings(markdown: str, *, base_path: str) -> list[ReviewerPacketVerificationFinding]:
    findings: list[ReviewerPacketVerificationFinding] = []
    if "```" in markdown:
        findings.append(ReviewerPacketVerificationFinding(code="markdown_code_fence", path=base_path, object_type="markdown"))
    for pattern in MARKDOWN_FORBIDDEN_PATTERNS:
        if pattern in markdown:
            findings.append(ReviewerPacketVerificationFinding(code="markdown_forbidden_marker", path=base_path, object_type="markdown"))
    for diagnostic in validate_chunk_repair_contract_markdown(markdown):
        findings.append(ReviewerPacketVerificationFinding(code=f"markdown_validation_failed:{diagnostic.code}", path=diagnostic.path, object_type=diagnostic.object_type, object_id=diagnostic.object_id))
    return findings


def _lineage_subset_findings(prototype: dict[str, Any], repair_prototype: dict[str, Any], s02_contract: dict[str, Any]) -> list[ReviewerPacketVerificationFinding]:
    known = _known_s02_ids(s02_contract)
    repair_ids = _known_repair_ids(repair_prototype)
    findings: list[ReviewerPacketVerificationFinding] = []
    for index, packet in enumerate(_list_of_dicts(prototype.get("packets"))):
        packet_id = str(packet.get("packet_id", ""))
        base = f"/packets/{index}"
        paper_id = str(packet.get("paper_id", ""))
        if paper_id not in known["paper_ids"]:
            findings.append(ReviewerPacketVerificationFinding(code="paper_id_not_in_s02_stable_ids", path=f"{base}/paper_id", object_type="packet", object_id=packet_id))
        locator_id = str(packet.get("locator_id", ""))
        if locator_id not in known["locator_ids"]:
            findings.append(ReviewerPacketVerificationFinding(code="locator_id_not_in_s02_stable_ids", path=f"{base}/locator_id", object_type="packet", object_id=packet_id))
        if locator_id not in repair_ids["locator_ids"]:
            findings.append(ReviewerPacketVerificationFinding(code="locator_id_not_in_repair_prototype", path=f"{base}/locator_id", object_type="packet", object_id=packet_id))
        for source_index, source_id in enumerate(packet.get("source_refs", []) if isinstance(packet.get("source_refs"), list) else []):
            source_id = str(source_id)
            if source_id not in known["source_ids"]:
                findings.append(ReviewerPacketVerificationFinding(code="source_id_not_in_s02_stable_ids", path=f"{base}/source_refs/{source_index}", object_type="packet", object_id=packet_id))
            if source_id not in repair_ids["source_ids"]:
                findings.append(ReviewerPacketVerificationFinding(code="source_id_not_in_repair_prototype", path=f"{base}/source_refs/{source_index}", object_type="packet", object_id=packet_id))
        for span_index, span in enumerate(_list_of_dicts(packet.get("span_refs"))):
            span_id = str(span.get("span_id", ""))
            source_id = str(span.get("source_id", ""))
            if span_id not in known["span_ids"]:
                findings.append(ReviewerPacketVerificationFinding(code="span_id_not_in_s02_stable_ids", path=f"{base}/span_refs/{span_index}/span_id", object_type="span_ref", object_id=span_id))
            if span_id not in repair_ids["span_ids"]:
                findings.append(ReviewerPacketVerificationFinding(code="span_id_not_in_repair_prototype", path=f"{base}/span_refs/{span_index}/span_id", object_type="span_ref", object_id=span_id))
            if source_id not in known["source_ids"]:
                findings.append(ReviewerPacketVerificationFinding(code="span_source_id_not_in_s02_stable_ids", path=f"{base}/span_refs/{span_index}/source_id", object_type="span_ref", object_id=span_id))
    return findings


def _known_s02_ids(contract: dict[str, Any]) -> dict[str, set[str]]:
    stable = contract.get("stable_ids") if isinstance(contract.get("stable_ids"), dict) else {}
    paper_ids = _string_set(stable.get("paper_ids")) | _string_set(expected_audit_from_contract(contract).get("paper_ids"))
    if contract.get("paper_id"):
        paper_ids.add(str(contract["paper_id"]))
    source_ids = _string_set(stable.get("source_ids"))
    locator_ids = _string_set(stable.get("locator_ids"))
    span_ids = _string_set(stable.get("span_ids"))
    for target in _list_of_dicts(contract.get("repair_targets")):
        if target.get("paper_id"):
            paper_ids.add(str(target["paper_id"]))
        if target.get("locator_id"):
            locator_ids.add(str(target["locator_id"]))
        for span in _list_of_dicts(target.get("source_spans")):
            if span.get("span_id"):
                span_ids.add(str(span["span_id"]))
            if span.get("source_id"):
                source_ids.add(str(span["source_id"]))
    return {"paper_ids": paper_ids, "source_ids": source_ids, "locator_ids": locator_ids, "span_ids": span_ids}


def _known_repair_ids(repair_prototype: dict[str, Any]) -> dict[str, set[str]]:
    source_ids: set[str] = set()
    locator_ids: set[str] = set()
    span_ids: set[str] = set()
    for target in _list_of_dicts(repair_prototype.get("repair_targets")):
        if target.get("locator_id"):
            locator_ids.add(str(target["locator_id"]))
        for source_id in target.get("source_artifact_refs", []) if isinstance(target.get("source_artifact_refs"), list) else []:
            source_ids.add(str(source_id))
        for span in _list_of_dicts(target.get("source_spans")):
            if span.get("span_id"):
                span_ids.add(str(span["span_id"]))
            if span.get("source_id"):
                source_ids.add(str(span["source_id"]))
    return {"source_ids": source_ids, "locator_ids": locator_ids, "span_ids": span_ids}


def _unsafe_counters_zero(counters: dict[str, Any]) -> bool:
    return all(
        counters.get(field) == 0
        for field in ("accepted_count", "importable_count", "semantic_ready_count", "raw_text_embedded_count", "unsafe_safety_boundary_count")
    ) and all(
        counters.get(field) is False
        for field in ("production_import_attempted", "ladybugdb_written", "secrets_included", "embeddings_included", "vectors_included")
    )


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _string_set(value: Any) -> set[str]:
    if not isinstance(value, list | tuple | set):
        return set()
    return {str(item) for item in value if item is not None and str(item)}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", "--packets", dest="json", type=Path, required=True, help="Generated reviewer packet JSON")
    parser.add_argument("--markdown", "--packets-markdown", dest="markdown", type=Path, required=True, help="Generated reviewer packet Markdown")
    parser.add_argument("--assessment-json", "--assessment", dest="assessment_json", type=Path, required=True, help="Generated standalone assessment JSON")
    parser.add_argument("--assessment-markdown", type=Path, required=True, help="Generated standalone assessment Markdown")
    parser.add_argument("--repair-prototype", type=Path, required=True, help="S03 bounded repair prototype JSON")
    parser.add_argument("--s02-contract", type=Path, required=True, help="S02 chunk-repair-contract JSON with stable IDs")
    args = parser.parse_args(argv)

    try:
        summary = verify_files(
            args.json,
            args.markdown,
            args.assessment_json,
            args.assessment_markdown,
            args.repair_prototype,
            args.s02_contract,
        )
    except (FileNotFoundError, ReviewerPacketPrototypeVerifyError, ValueError) as exc:
        sys.stderr.write(f"reviewer packet prototype verify failed: {exc}\n")
        return 2
    if not summary["passed"]:
        sys.stderr.write("reviewer packet prototype verify failed: " + json.dumps(summary, sort_keys=True) + "\n")
        return 2
    sys.stdout.write(
        "reviewer packet prototype verified: "
        f"packets={summary['packet_count']} "
        f"review_status={summary['review_status_counts']} "
        f"repair_states={summary['repair_state_counts']} "
        f"route_quality={summary['route_quality_state_counts']} "
        f"assessment_verdict={summary['assessment_verdict']} "
        f"unsafe_counters_zero={summary['unsafe_counters_zero']}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
