#!/usr/bin/env python3
"""Verify M022/S03 bounded repair prototype JSON and Markdown artifacts.

The verifier is read-only. It checks contract validity, redaction boundaries,
zero unsafe counters, false target safety flags, Markdown safety, and lineage
subset consistency against S02 stable IDs.
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

from research_graph.infrastructure.repair.chunk_repair_contract import (  # noqa: E402
    REQUIRED_FALSE_SAFETY_FIELDS,
    expected_audit_from_contract,
    scan_forbidden_payload_keys,
    validate_chunk_repair_contract,
    validate_chunk_repair_contract_markdown,
)


@dataclass(frozen=True)
class PrototypeVerificationFinding:
    """One redacted verifier finding."""

    code: str
    path: str
    object_type: str | None = None
    object_id: str | None = None


class BoundedRepairPrototypeVerifyError(ValueError):
    """Raised when verifier inputs are unreadable or malformed."""


def load_json_object(path: str | Path, *, label: str) -> dict[str, Any]:
    """Load one JSON object with redacted parse diagnostics."""
    json_path = Path(path)
    if not json_path.exists():
        raise FileNotFoundError(f"{label} file not found: {json_path}")
    try:
        payload = json.loads(json_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise BoundedRepairPrototypeVerifyError(
            f"{label} JSON is malformed at line {exc.lineno} column {exc.colno}"
        ) from exc
    if not isinstance(payload, dict):
        raise BoundedRepairPrototypeVerifyError(f"{label} root must be a JSON object")
    return payload


def verify_bounded_repair_prototype(
    prototype: dict[str, Any],
    markdown: str,
    *,
    s02_contract: dict[str, Any],
) -> list[PrototypeVerificationFinding]:
    """Return redacted safety and lineage findings for generated artifacts."""
    findings: list[PrototypeVerificationFinding] = []
    validation = validate_chunk_repair_contract(
        prototype, expected_audit=expected_audit_from_contract(prototype)
    )
    if not validation.passed:
        findings.extend(
            PrototypeVerificationFinding(
                code=f"contract_validation_failed:{diagnostic.code}",
                path=diagnostic.path,
                object_type=diagnostic.object_type,
                object_id=diagnostic.object_id,
            )
            for diagnostic in validation.diagnostics
        )
    if validation.target_count <= 0:
        findings.append(
            PrototypeVerificationFinding(
                code="empty_repair_targets", path="/repair_targets", object_type="contract"
            )
        )
    if validation.import_eligible_count != 0:
        findings.append(
            PrototypeVerificationFinding(
                code="unsafe_import_count",
                path="/diagnostics/import_eligible_count",
                object_type="diagnostics",
            )
        )
    if validation.production_write_count != 0:
        findings.append(
            PrototypeVerificationFinding(
                code="unsafe_write_count",
                path="/diagnostics/production_write_count",
                object_type="diagnostics",
            )
        )
    if validation.semantic_ready_count != 0:
        findings.append(
            PrototypeVerificationFinding(
                code="unsafe_semantic_ready_count",
                path="/diagnostics/semantic_ready_count",
                object_type="diagnostics",
            )
        )

    for forbidden in scan_forbidden_payload_keys(prototype):
        findings.append(
            PrototypeVerificationFinding(
                code=forbidden.code, path=forbidden.path, object_type="payload"
            )
        )

    diagnostics = (
        prototype.get("diagnostics") if isinstance(prototype.get("diagnostics"), dict) else {}
    )
    for field in (
        "import_eligible_count",
        "promoted_to_fact_count",
        "production_write_count",
        "semantic_ready_count",
    ):
        if diagnostics.get(field) != 0:
            findings.append(
                PrototypeVerificationFinding(
                    code="unsafe_diagnostic_counter",
                    path=f"/diagnostics/{field}",
                    object_type="diagnostics",
                )
            )
    for field in (
        "raw_text_included",
        "chunk_text_included",
        "embeddings_included",
        "vectors_included",
        "secrets_included",
        "ladybugdb_written",
        "production_import_attempted",
    ):
        if diagnostics.get(field) is not False:
            findings.append(
                PrototypeVerificationFinding(
                    code="unsafe_diagnostic_flag",
                    path=f"/diagnostics/{field}",
                    object_type="diagnostics",
                )
            )

    for index, target in enumerate(_list_of_dicts(prototype.get("repair_targets"))):
        target_id = str(target.get("target_id", ""))
        safety = target.get("safety_boundaries")
        if not isinstance(safety, dict):
            findings.append(
                PrototypeVerificationFinding(
                    code="missing_safety_boundaries",
                    path=f"/repair_targets/{index}/safety_boundaries",
                    object_type="repair_target",
                    object_id=target_id,
                )
            )
            continue
        for field in REQUIRED_FALSE_SAFETY_FIELDS:
            if safety.get(field) is not False:
                findings.append(
                    PrototypeVerificationFinding(
                        code="unsafe_target_safety_flag",
                        path=f"/repair_targets/{index}/safety_boundaries/{field}",
                        object_type="repair_target",
                        object_id=target_id,
                    )
                )

    for diagnostic in validate_chunk_repair_contract_markdown(markdown):
        findings.append(
            PrototypeVerificationFinding(
                code=f"markdown_validation_failed:{diagnostic.code}",
                path=diagnostic.path,
                object_type=diagnostic.object_type,
                object_id=diagnostic.object_id,
            )
        )

    findings.extend(_lineage_subset_findings(prototype, s02_contract))
    return findings


def verify_files(
    prototype_path: Path, markdown_path: Path, s02_contract_path: Path
) -> dict[str, Any]:
    """Read and verify JSON/Markdown artifacts."""
    prototype = load_json_object(prototype_path, label="bounded repair prototype")
    s02_contract = load_json_object(s02_contract_path, label="S02 contract")
    if not markdown_path.exists():
        raise FileNotFoundError(f"prototype Markdown file not found: {markdown_path}")
    markdown = markdown_path.read_text(encoding="utf-8")
    findings = verify_bounded_repair_prototype(prototype, markdown, s02_contract=s02_contract)
    targets = _list_of_dicts(prototype.get("repair_targets"))
    diagnostics = (
        prototype.get("diagnostics") if isinstance(prototype.get("diagnostics"), dict) else {}
    )
    return {
        "passed": not findings,
        "target_count": len(targets),
        "repair_state_counts": diagnostics.get("repair_state_counts", {}),
        "route_quality_state_counts": diagnostics.get("route_quality_state_counts", {}),
        "unsafe_counters_zero": not any(
            diagnostics.get(field) not in (0, False)
            for field in (
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
        ),
        "findings": [finding.__dict__ for finding in findings],
    }


def _lineage_subset_findings(
    prototype: dict[str, Any], s02_contract: dict[str, Any]
) -> list[PrototypeVerificationFinding]:
    stable_ids = (
        s02_contract.get("stable_ids") if isinstance(s02_contract.get("stable_ids"), dict) else {}
    )
    known_locators = _string_set(stable_ids.get("locator_ids"))
    known_sources = _string_set(stable_ids.get("source_ids"))
    known_spans = _string_set(stable_ids.get("span_ids"))
    findings: list[PrototypeVerificationFinding] = []
    for index, target in enumerate(_list_of_dicts(prototype.get("repair_targets"))):
        target_id = str(target.get("target_id", ""))
        locator_id = str(target.get("locator_id", ""))
        if locator_id not in known_locators:
            findings.append(
                PrototypeVerificationFinding(
                    code="locator_id_not_in_s02_stable_ids",
                    path=f"/repair_targets/{index}/locator_id",
                    object_type="repair_target",
                    object_id=target_id,
                )
            )
        for source_index, source_id in enumerate(
            target.get("source_artifact_refs", [])
            if isinstance(target.get("source_artifact_refs"), list)
            else []
        ):
            if str(source_id) not in known_sources:
                findings.append(
                    PrototypeVerificationFinding(
                        code="source_id_not_in_s02_stable_ids",
                        path=f"/repair_targets/{index}/source_artifact_refs/{source_index}",
                        object_type="repair_target",
                        object_id=target_id,
                    )
                )
        for span_index, span in enumerate(_list_of_dicts(target.get("source_spans"))):
            span_id = str(span.get("span_id", ""))
            if span_id not in known_spans:
                findings.append(
                    PrototypeVerificationFinding(
                        code="span_id_not_in_s02_stable_ids",
                        path=f"/repair_targets/{index}/source_spans/{span_index}/span_id",
                        object_type="source_span",
                        object_id=span_id,
                    )
                )
            source_id = str(span.get("source_id", ""))
            if source_id not in known_sources:
                findings.append(
                    PrototypeVerificationFinding(
                        code="span_source_id_not_in_s02_stable_ids",
                        path=f"/repair_targets/{index}/source_spans/{span_index}/source_id",
                        object_type="source_span",
                        object_id=span_id,
                    )
                )
    return findings


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
    parser.add_argument(
        "--prototype", type=Path, required=True, help="Generated bounded repair prototype JSON"
    )
    parser.add_argument(
        "--markdown", type=Path, required=True, help="Generated bounded repair prototype Markdown"
    )
    parser.add_argument(
        "--s02-contract",
        type=Path,
        required=True,
        help="S02 chunk-repair-contract JSON with stable IDs",
    )
    args = parser.parse_args(argv)

    try:
        summary = verify_files(args.prototype, args.markdown, args.s02_contract)
    except (FileNotFoundError, BoundedRepairPrototypeVerifyError, ValueError) as exc:
        sys.stderr.write(f"bounded repair prototype verify failed: {exc}\n")
        return 2
    if not summary["passed"]:
        sys.stderr.write(
            "bounded repair prototype verify failed: " + json.dumps(summary, sort_keys=True) + "\n"
        )
        return 2
    sys.stdout.write(
        "bounded repair prototype verified: "
        f"targets={summary['target_count']} "
        f"repair_states={summary['repair_state_counts']} "
        f"route_quality={summary['route_quality_state_counts']} "
        f"unsafe_counters_zero={summary['unsafe_counters_zero']}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
