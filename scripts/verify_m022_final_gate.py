#!/usr/bin/env python3
"""Verify and optionally build the M022 final no-import gate artifact.

The verifier is intentionally redacted: diagnostics include stable codes, JSON
paths, object types, and object IDs only. It never echoes source payload values,
corpus text, embeddings, vectors, or secrets from input artifacts.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
for import_path in (ROOT, SRC):
    if str(import_path) not in sys.path:
        sys.path.insert(0, str(import_path))

from research_graph.infrastructure.repair.chunk_repair_contract import (
    scan_forbidden_payload_keys,  # noqa: E402
)
from scripts.verify_reviewer_packet_prototype import (  # noqa: E402
    ReviewerPacketPrototypeVerifyError,
    _list_of_dicts,
    load_json_object,
    verify_reviewer_packet_prototype,
)

FINAL_GATE_SCHEMA_VERSION = "m022-final-gate.v1"
REQUIRED_REQUIREMENT_OUTCOMES = ("R024", "R027", "R028", "R029")
FINAL_RECOMMENDATION = "Proceed only with human semantic review or another bounded repair iteration; do not import to KG."
SAFE_ASSESSMENT_VERDICT = "blocked_pending_semantic_acceptance"
UNSAFE_COUNTER_FIELDS = (
    "accepted_count",
    "importable_count",
    "semantic_ready_count",
    "raw_text_embedded_count",
    "unsafe_safety_boundary_count",
)
UNSAFE_BOOLEAN_FIELDS = (
    "production_import_attempted",
    "ladybugdb_written",
    "secrets_included",
    "embeddings_included",
    "vectors_included",
)
BLOCKED_BOUNDARIES = {
    "kg_import_blocked": True,
    "semantic_readiness_blocked": True,
    "production_write_blocked": True,
    "raw_payload_output_blocked": True,
    "embedding_vector_output_blocked": True,
    "secrets_output_blocked": True,
    "import_allowed": False,
    "semantic_ready_for_kg": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "raw_payloads_included": False,
    "embeddings_included": False,
    "vectors_included": False,
    "secrets_included": False,
}


@dataclass(frozen=True)
class FinalGateFinding:
    """One redacted final-gate diagnostic."""

    code: str
    path: str
    object_type: str | None = None
    object_id: str | None = None


class FinalGateVerifyError(ValueError):
    """Raised when final-gate inputs are unreadable or malformed."""


def build_final_gate(
    *,
    packet_json_path: Path,
    packet_markdown_path: Path,
    assessment_json_path: Path,
    assessment_markdown_path: Path,
    repair_prototype_path: Path,
    s02_contract_path: Path,
    verification_commands: list[str] | None = None,
) -> dict[str, Any]:
    """Build the deterministic final-gate JSON object from verified source artifacts."""
    inputs = _load_inputs(
        packet_json_path=packet_json_path,
        packet_markdown_path=packet_markdown_path,
        assessment_json_path=assessment_json_path,
        assessment_markdown_path=assessment_markdown_path,
        repair_prototype_path=repair_prototype_path,
        s02_contract_path=s02_contract_path,
    )
    findings = verify_source_artifacts(**inputs)
    if findings:
        raise FinalGateVerifyError("source artifacts failed final-gate verification")

    packets = _list_of_dicts(inputs["packet_json"].get("packets"))
    assessment = inputs["assessment_json"]
    unsafe_counters = (
        assessment.get("unsafe_counters")
        if isinstance(assessment.get("unsafe_counters"), dict)
        else {}
    )
    repair_targets = _list_of_dicts(inputs["repair_prototype"].get("repair_targets"))
    final_gate = {
        "schema_version": FINAL_GATE_SCHEMA_VERSION,
        "source_artifacts": {
            "reviewer_packet_json": _stable_path(packet_json_path),
            "reviewer_packet_markdown": _stable_path(packet_markdown_path),
            "independent_assessment_json": _stable_path(assessment_json_path),
            "independent_assessment_markdown": _stable_path(assessment_markdown_path),
            "bounded_repair_prototype_json": _stable_path(repair_prototype_path),
            "chunk_repair_contract_json": _stable_path(s02_contract_path),
        },
        "packet_summary": {
            "packet_count": len(packets),
            "review_status_counts": dict(
                sorted(
                    Counter(
                        str(packet.get("review_status", "missing")) for packet in packets
                    ).items()
                )
            ),
            "repair_state_counts": dict(
                sorted(
                    Counter(
                        str(packet.get("repair_state", "missing")) for packet in packets
                    ).items()
                )
            ),
            "route_counts": dict(
                sorted(Counter(str(packet.get("route", "missing")) for packet in packets).items())
            ),
            "route_quality_state_counts": dict(
                sorted(
                    Counter(
                        str(packet.get("route_quality_state", "missing")) for packet in packets
                    ).items()
                )
            ),
            "repair_target_count": len(repair_targets),
            "unsafe_counters_zero": _unsafe_counters_zero(unsafe_counters),
        },
        "assessment_verdict": assessment.get("verdict", "missing"),
        "requirement_outcomes": _requirement_outcomes(len(packets)),
        "blocked_boundaries": dict(BLOCKED_BOUNDARIES),
        "final_recommendation": {
            "action": "human_semantic_review_or_bounded_repair_only",
            "recommendation": FINAL_RECOMMENDATION,
            "kg_import_allowed": False,
            "semantic_ready_for_kg_claimed": False,
            "production_write_attempted": False,
            "positive_import_recommendation_claimed": False,
        },
        "verification_commands": list(verification_commands or _default_verification_commands()),
    }
    gate_findings = validate_final_gate(final_gate, expected_packet_count=len(packets))
    if gate_findings:
        raise FinalGateVerifyError("built final gate failed self-validation")
    return final_gate


def verify_files(
    packet_json_path: Path,
    packet_markdown_path: Path,
    assessment_json_path: Path,
    assessment_markdown_path: Path,
    repair_prototype_path: Path,
    s02_contract_path: Path,
    final_gate_path: Path | None = None,
) -> dict[str, Any]:
    """Read source artifacts and optional final-gate JSON, returning a redacted summary."""
    inputs = _load_inputs(
        packet_json_path=packet_json_path,
        packet_markdown_path=packet_markdown_path,
        assessment_json_path=assessment_json_path,
        assessment_markdown_path=assessment_markdown_path,
        repair_prototype_path=repair_prototype_path,
        s02_contract_path=s02_contract_path,
    )
    findings = verify_source_artifacts(**inputs)
    packets = _list_of_dicts(inputs["packet_json"].get("packets"))
    assessment = inputs["assessment_json"]
    unsafe_counters = (
        assessment.get("unsafe_counters")
        if isinstance(assessment.get("unsafe_counters"), dict)
        else {}
    )
    final_gate = None
    if final_gate_path is not None:
        final_gate = load_json_object(final_gate_path, label="M022 final gate")
        findings.extend(validate_final_gate(final_gate, expected_packet_count=len(packets)))
    return {
        "passed": not findings,
        "schema_version": final_gate.get("schema_version")
        if isinstance(final_gate, dict)
        else FINAL_GATE_SCHEMA_VERSION,
        "packet_count": len(packets),
        "review_status_counts": dict(
            sorted(
                Counter(str(packet.get("review_status", "missing")) for packet in packets).items()
            )
        ),
        "repair_state_counts": dict(
            sorted(
                Counter(str(packet.get("repair_state", "missing")) for packet in packets).items()
            )
        ),
        "route_counts": dict(
            sorted(Counter(str(packet.get("route", "missing")) for packet in packets).items())
        ),
        "route_quality_state_counts": dict(
            sorted(
                Counter(
                    str(packet.get("route_quality_state", "missing")) for packet in packets
                ).items()
            )
        ),
        "assessment_verdict": assessment.get("verdict", "missing"),
        "requirement_outcomes": _requirement_outcomes(len(packets)),
        "blocked_boundaries": _boundary_summary(final_gate),
        "final_recommendation_action": _recommendation_action(final_gate),
        "unsafe_counters_zero": _unsafe_counters_zero(unsafe_counters),
        "findings": [finding.__dict__ for finding in findings],
    }


def verify_source_artifacts(
    *,
    packet_json: dict[str, Any],
    packet_markdown: str,
    assessment_json: dict[str, Any],
    assessment_markdown: str,
    repair_prototype: dict[str, Any],
    s02_contract: dict[str, Any],
) -> list[FinalGateFinding]:
    """Validate all source artifacts and return stable redacted findings."""
    findings = [
        FinalGateFinding(finding.code, finding.path, finding.object_type, finding.object_id)
        for finding in verify_reviewer_packet_prototype(
            packet_json,
            packet_markdown,
            assessment_json,
            assessment_markdown,
            repair_prototype=repair_prototype,
            s02_contract=s02_contract,
        )
    ]
    findings.extend(_forbidden_key_findings(repair_prototype, base_type="repair_prototype"))
    findings.extend(_forbidden_key_findings(s02_contract, base_type="s02_contract"))
    packets = _list_of_dicts(packet_json.get("packets"))
    if len(packets) != 6:
        findings.append(
            FinalGateFinding("final_gate_packet_count_not_six", "/packets", "prototype")
        )
    if assessment_json.get("verdict") != SAFE_ASSESSMENT_VERDICT:
        findings.append(FinalGateFinding("assessment_verdict_drift", "/verdict", "assessment"))
    if assessment_json.get("import_allowed") is not False:
        findings.append(
            FinalGateFinding("assessment_import_allowed", "/import_allowed", "assessment")
        )
    if assessment_json.get("semantic_ready_for_kg") is not False:
        findings.append(
            FinalGateFinding("assessment_semantic_ready", "/semantic_ready_for_kg", "assessment")
        )
    unsafe_counters = (
        assessment_json.get("unsafe_counters")
        if isinstance(assessment_json.get("unsafe_counters"), dict)
        else {}
    )
    findings.extend(
        _unsafe_counter_findings(
            unsafe_counters, base_path="/unsafe_counters", object_type="assessment"
        )
    )
    findings.extend(_repair_source_boundary_findings(repair_prototype))
    findings.extend(_contract_boundary_findings(s02_contract))
    return findings


def validate_final_gate(
    final_gate: dict[str, Any], *, expected_packet_count: int | None = None
) -> list[FinalGateFinding]:
    """Validate a generated final-gate JSON object fail-closed."""
    findings: list[FinalGateFinding] = []
    findings.extend(_forbidden_key_findings(final_gate, base_type="final_gate"))
    if final_gate.get("schema_version") != FINAL_GATE_SCHEMA_VERSION:
        findings.append(
            FinalGateFinding("final_gate_schema_mismatch", "/schema_version", "final_gate")
        )
    source_artifacts = (
        final_gate.get("source_artifacts")
        if isinstance(final_gate.get("source_artifacts"), dict)
        else {}
    )
    for key in (
        "reviewer_packet_json",
        "reviewer_packet_markdown",
        "independent_assessment_json",
        "independent_assessment_markdown",
        "bounded_repair_prototype_json",
        "chunk_repair_contract_json",
    ):
        if not isinstance(source_artifacts.get(key), str) or not source_artifacts.get(key):
            findings.append(
                FinalGateFinding(
                    "final_gate_missing_source_artifact", f"/source_artifacts/{key}", "final_gate"
                )
            )
    packet_summary = (
        final_gate.get("packet_summary")
        if isinstance(final_gate.get("packet_summary"), dict)
        else {}
    )
    if (
        expected_packet_count is not None
        and packet_summary.get("packet_count") != expected_packet_count
    ):
        findings.append(
            FinalGateFinding(
                "final_gate_packet_count_mismatch", "/packet_summary/packet_count", "final_gate"
            )
        )
    if packet_summary.get("packet_count") != 6:
        findings.append(
            FinalGateFinding(
                "final_gate_packet_count_not_six", "/packet_summary/packet_count", "final_gate"
            )
        )
    if packet_summary.get("review_status_counts") != {"pending_review": 6}:
        findings.append(
            FinalGateFinding(
                "final_gate_review_status_not_pending",
                "/packet_summary/review_status_counts",
                "final_gate",
            )
        )
    if packet_summary.get("unsafe_counters_zero") is not True:
        findings.append(
            FinalGateFinding(
                "final_gate_unsafe_counters_not_zero",
                "/packet_summary/unsafe_counters_zero",
                "final_gate",
            )
        )
    if final_gate.get("assessment_verdict") != SAFE_ASSESSMENT_VERDICT:
        findings.append(
            FinalGateFinding(
                "final_gate_assessment_verdict_drift", "/assessment_verdict", "final_gate"
            )
        )
    findings.extend(_requirement_findings(final_gate.get("requirement_outcomes")))
    findings.extend(_blocked_boundary_findings(final_gate.get("blocked_boundaries")))
    findings.extend(_recommendation_findings(final_gate.get("final_recommendation")))
    commands = final_gate.get("verification_commands")
    if (
        not isinstance(commands, list)
        or not commands
        or not all(isinstance(command, str) and command for command in commands)
    ):
        findings.append(
            FinalGateFinding(
                "final_gate_missing_verification_commands", "/verification_commands", "final_gate"
            )
        )
    return findings


def write_final_gate(final_gate: dict[str, Any], output_path: Path) -> None:
    """Write final-gate JSON deterministically."""
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(final_gate, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def _load_inputs(
    *,
    packet_json_path: Path,
    packet_markdown_path: Path,
    assessment_json_path: Path,
    assessment_markdown_path: Path,
    repair_prototype_path: Path,
    s02_contract_path: Path,
) -> dict[str, Any]:
    packet_json = load_json_object(packet_json_path, label="reviewer packet prototype")
    assessment_json = load_json_object(assessment_json_path, label="reviewer packet assessment")
    repair_prototype = load_json_object(repair_prototype_path, label="repair prototype")
    s02_contract = load_json_object(s02_contract_path, label="S02 contract")
    if not packet_markdown_path.exists():
        raise FileNotFoundError(f"reviewer packet Markdown file not found: {packet_markdown_path}")
    if not assessment_markdown_path.exists():
        raise FileNotFoundError(f"assessment Markdown file not found: {assessment_markdown_path}")
    return {
        "packet_json": packet_json,
        "packet_markdown": packet_markdown_path.read_text(encoding="utf-8"),
        "assessment_json": assessment_json,
        "assessment_markdown": assessment_markdown_path.read_text(encoding="utf-8"),
        "repair_prototype": repair_prototype,
        "s02_contract": s02_contract,
    }


def _forbidden_key_findings(payload: dict[str, Any], *, base_type: str) -> list[FinalGateFinding]:
    return [
        FinalGateFinding(finding.code, finding.path, base_type)
        for finding in scan_forbidden_payload_keys(payload)
    ]


def _unsafe_counter_findings(
    counters: dict[str, Any], *, base_path: str, object_type: str
) -> list[FinalGateFinding]:
    findings: list[FinalGateFinding] = []
    for field in UNSAFE_COUNTER_FIELDS:
        if counters.get(field) != 0:
            findings.append(
                FinalGateFinding("unsafe_counter_nonzero", f"{base_path}/{field}", object_type)
            )
    for field in UNSAFE_BOOLEAN_FIELDS:
        if counters.get(field) is not False:
            findings.append(
                FinalGateFinding("unsafe_counter_true", f"{base_path}/{field}", object_type)
            )
    return findings


def _repair_source_boundary_findings(repair_prototype: dict[str, Any]) -> list[FinalGateFinding]:
    findings: list[FinalGateFinding] = []
    diagnostics = (
        repair_prototype.get("diagnostics")
        if isinstance(repair_prototype.get("diagnostics"), dict)
        else {}
    )
    for field in (
        "import_eligible_count",
        "production_write_count",
        "promoted_to_fact_count",
        "semantic_ready_count",
        "accepted_count",
    ):
        if diagnostics.get(field) != 0:
            findings.append(
                FinalGateFinding(
                    "repair_diagnostic_unsafe_count", f"/diagnostics/{field}", "repair_prototype"
                )
            )
    for field in (
        "chunk_text_included",
        "raw_text_included",
        "embeddings_included",
        "vectors_included",
        "secrets_included",
        "production_import_attempted",
        "ladybugdb_written",
    ):
        if diagnostics.get(field) is not False:
            findings.append(
                FinalGateFinding(
                    "repair_diagnostic_unsafe_boolean", f"/diagnostics/{field}", "repair_prototype"
                )
            )
    return findings


def _contract_boundary_findings(contract: dict[str, Any]) -> list[FinalGateFinding]:
    findings: list[FinalGateFinding] = []
    safety = (
        contract.get("safety_boundary") if isinstance(contract.get("safety_boundary"), dict) else {}
    )
    for field in (
        "import_eligible",
        "ladybugdb_written",
        "production_write_attempted",
        "promoted_to_fact",
        "semantic_ready_for_kg",
        "source_payloads_included",
        "embeddings_included",
        "vectors_included",
        "secrets_included",
        "trusted_kg_import_allowed",
    ):
        if safety.get(field) is not False:
            findings.append(
                FinalGateFinding(
                    "s02_safety_boundary_unsafe", f"/safety_boundary/{field}", "s02_contract"
                )
            )
    return findings


def _requirement_outcomes(packet_count: int) -> dict[str, dict[str, Any]]:
    return {
        "R024": {
            "status": "blocked_boundary_preserved",
            "evidence": "import_allowed_false_and_semantic_ready_false",
            "packet_count": packet_count,
            "import_allowed_claimed": False,
            "semantic_ready_for_kg_claimed": False,
        },
        "R027": {
            "status": "redacted_diagnostics_verified",
            "evidence": "stable_counts_codes_paths_and_ids_only",
            "raw_payload_values_included": False,
        },
        "R028": {
            "status": "bounded_reviewer_packet_verified",
            "evidence": "six_pending_non_importable_packets",
            "packet_count": packet_count,
        },
        "R029": {
            "status": "final_no_import_gate_verified",
            "evidence": "final_gate_schema_and_blocked_boundaries",
            "positive_import_recommendation_claimed": False,
        },
    }


def _requirement_findings(value: Any) -> list[FinalGateFinding]:
    findings: list[FinalGateFinding] = []
    if not isinstance(value, dict):
        return [
            FinalGateFinding(
                "final_gate_missing_requirement_outcomes", "/requirement_outcomes", "final_gate"
            )
        ]
    for requirement_id in REQUIRED_REQUIREMENT_OUTCOMES:
        outcome = value.get(requirement_id)
        if not isinstance(outcome, dict):
            findings.append(
                FinalGateFinding(
                    "final_gate_missing_requirement_outcome",
                    f"/requirement_outcomes/{requirement_id}",
                    "requirement",
                    requirement_id,
                )
            )
            continue
        status = outcome.get("status")
        if not isinstance(status, str) or not status:
            findings.append(
                FinalGateFinding(
                    "final_gate_requirement_status_missing",
                    f"/requirement_outcomes/{requirement_id}/status",
                    "requirement",
                    requirement_id,
                )
            )
        for key, field_value in outcome.items():
            if (
                key.endswith(("_claimed", "_included", "_attempted", "_allowed"))
                and field_value is not False
            ):
                findings.append(
                    FinalGateFinding(
                        "final_gate_requirement_unsafe_claim",
                        f"/requirement_outcomes/{requirement_id}/{key}",
                        "requirement",
                        requirement_id,
                    )
                )
    return findings


def _blocked_boundary_findings(value: Any) -> list[FinalGateFinding]:
    findings: list[FinalGateFinding] = []
    if not isinstance(value, dict):
        return [
            FinalGateFinding(
                "final_gate_missing_blocked_boundaries", "/blocked_boundaries", "final_gate"
            )
        ]
    for key, expected in BLOCKED_BOUNDARIES.items():
        if value.get(key) is not expected:
            findings.append(
                FinalGateFinding(
                    "final_gate_boundary_polarity_drift",
                    f"/blocked_boundaries/{key}",
                    "boundary",
                    key,
                )
            )
    for key, field_value in value.items():
        if key.endswith("_blocked") and field_value is not True:
            findings.append(
                FinalGateFinding(
                    "final_gate_boundary_not_blocked",
                    f"/blocked_boundaries/{key}",
                    "boundary",
                    str(key),
                )
            )
        if (
            key.endswith(("_allowed", "_claimed", "_included", "_attempted"))
            and field_value is not False
        ):
            findings.append(
                FinalGateFinding(
                    "final_gate_boundary_unsafe_claim",
                    f"/blocked_boundaries/{key}",
                    "boundary",
                    str(key),
                )
            )
    return findings


def _recommendation_findings(value: Any) -> list[FinalGateFinding]:
    if not isinstance(value, dict):
        return [
            FinalGateFinding(
                "final_gate_missing_recommendation", "/final_recommendation", "final_gate"
            )
        ]
    findings: list[FinalGateFinding] = []
    if value.get("action") != "human_semantic_review_or_bounded_repair_only":
        findings.append(
            FinalGateFinding(
                "final_gate_recommendation_action_drift",
                "/final_recommendation/action",
                "final_gate",
            )
        )
    for key in (
        "kg_import_allowed",
        "semantic_ready_for_kg_claimed",
        "production_write_attempted",
        "positive_import_recommendation_claimed",
    ):
        if value.get(key) is not False:
            findings.append(
                FinalGateFinding(
                    "final_gate_recommendation_unsafe_claim",
                    f"/final_recommendation/{key}",
                    "final_gate",
                )
            )
    return findings


def _unsafe_counters_zero(counters: dict[str, Any]) -> bool:
    return all(counters.get(field) == 0 for field in UNSAFE_COUNTER_FIELDS) and all(
        counters.get(field) is False for field in UNSAFE_BOOLEAN_FIELDS
    )


def _boundary_summary(final_gate: dict[str, Any] | None) -> dict[str, bool]:
    if isinstance(final_gate, dict) and isinstance(final_gate.get("blocked_boundaries"), dict):
        return {
            key: bool(final_gate["blocked_boundaries"].get(key))
            for key in sorted(BLOCKED_BOUNDARIES)
        }
    return dict(BLOCKED_BOUNDARIES)


def _recommendation_action(final_gate: dict[str, Any] | None) -> str:
    if isinstance(final_gate, dict) and isinstance(final_gate.get("final_recommendation"), dict):
        return str(final_gate["final_recommendation"].get("action", "missing"))
    return "human_semantic_review_or_bounded_repair_only"


def _stable_path(path: Path) -> str:
    try:
        return path.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return path.name


def _default_verification_commands() -> tuple[str, str]:
    return (
        "uv run pytest tests/test_m022_final_gate.py -q",
        "uv run ruff check scripts/verify_m022_final_gate.py tests/test_m022_final_gate.py",
    )


def _summary_line(summary: dict[str, Any]) -> str:
    return (
        "M022 final gate verified: "
        f"schema={summary['schema_version']} "
        f"packets={summary['packet_count']} "
        f"review_status={summary['review_status_counts']} "
        f"repair_states={summary['repair_state_counts']} "
        f"routes={summary['route_counts']} "
        f"route_quality={summary['route_quality_state_counts']} "
        f"assessment_verdict={summary['assessment_verdict']} "
        f"requirements={sorted(summary['requirement_outcomes'])} "
        f"blocked_boundaries={summary['blocked_boundaries']} "
        f"recommendation={summary['final_recommendation_action']} "
        f"unsafe_counters_zero={summary['unsafe_counters_zero']}"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packets-json", type=Path, required=True, help="S04 reviewer packet JSON")
    parser.add_argument(
        "--packets-markdown", type=Path, required=True, help="S04 reviewer packet Markdown"
    )
    parser.add_argument(
        "--assessment-json", type=Path, required=True, help="S04 independent assessment JSON"
    )
    parser.add_argument(
        "--assessment-markdown",
        type=Path,
        required=True,
        help="S04 independent assessment Markdown",
    )
    parser.add_argument(
        "--repair-prototype", type=Path, required=True, help="S03 bounded repair prototype JSON"
    )
    parser.add_argument(
        "--s02-contract", type=Path, required=True, help="S02 chunk repair contract JSON"
    )
    parser.add_argument(
        "--final-gate", type=Path, help="Optional existing final-gate JSON to validate"
    )
    parser.add_argument(
        "--write-final-gate",
        type=Path,
        help="Write a deterministic final-gate JSON after source validation",
    )
    args = parser.parse_args(argv)

    try:
        if args.write_final_gate is not None:
            final_gate = build_final_gate(
                packet_json_path=args.packets_json,
                packet_markdown_path=args.packets_markdown,
                assessment_json_path=args.assessment_json,
                assessment_markdown_path=args.assessment_markdown,
                repair_prototype_path=args.repair_prototype,
                s02_contract_path=args.s02_contract,
            )
            write_final_gate(final_gate, args.write_final_gate)
            final_gate_path = args.write_final_gate
        else:
            final_gate_path = args.final_gate
        summary = verify_files(
            args.packets_json,
            args.packets_markdown,
            args.assessment_json,
            args.assessment_markdown,
            args.repair_prototype,
            args.s02_contract,
            final_gate_path,
        )
    except (
        FileNotFoundError,
        FinalGateVerifyError,
        ReviewerPacketPrototypeVerifyError,
        ValueError,
    ) as exc:
        sys.stderr.write(f"M022 final gate verify failed: {exc}\n")
        return 2
    if not summary["passed"]:
        sys.stderr.write(
            "M022 final gate verify failed: " + json.dumps(summary, sort_keys=True) + "\n"
        )
        return 2
    sys.stdout.write(_summary_line(summary) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
