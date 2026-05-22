#!/usr/bin/env python3
"""Render M022/S04 reviewer packet prototype artifacts from S03 outputs.

The renderer reads local JSON only: a validated S03 bounded repair prototype and
an optional S02 stable-ID contract. It builds reviewer packet JSON, standalone
assessment JSON, and both Markdown artifacts fully in memory, validates every
artifact, then writes all outputs after validation succeeds.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))

from arxiv_archive.chunk_repair_contract import (  # noqa: E402
    MARKDOWN_FORBIDDEN_PATTERNS,
    scan_forbidden_payload_keys,
    validate_chunk_repair_contract_markdown,
)
from arxiv_archive.reviewer_packet_prototype import (  # noqa: E402
    REVIEWER_PACKET_ASSESSMENT_VERSION,
    REVIEWER_PACKET_PROTOTYPE_VERSION,
    ReviewerPacketError,
    build_reviewer_packet_prototype,
    render_reviewer_packet_markdown,
    summarize_reviewer_packet_prototype,
)


class ReviewerPacketPrototypeRenderError(ValueError):
    """Raised when the S04 renderer must fail closed before writing outputs."""


def load_json_object(path: str | Path, *, label: str) -> dict[str, Any]:
    """Load one JSON object with redacted path/parse errors."""
    json_path = Path(path)
    if not json_path.exists():
        raise FileNotFoundError(f"{label} file not found: {json_path}")
    try:
        payload = json.loads(json_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ReviewerPacketPrototypeRenderError(f"{label} JSON is malformed at line {exc.lineno} column {exc.colno}") from exc
    if not isinstance(payload, dict):
        raise ReviewerPacketPrototypeRenderError(f"{label} root must be a JSON object")
    return payload


def render_assessment_markdown(assessment: dict[str, Any]) -> str:
    """Render a redacted standalone assessment Markdown artifact."""
    _validate_assessment_payload(assessment, packet_count=None)
    counters = assessment["unsafe_counters"]
    dimension_results = assessment.get("dimension_results") if isinstance(assessment.get("dimension_results"), dict) else {}
    lines = [
        "# S04 Reviewer Packet Independent Assessment",
        "",
        "This review-only assessment records deterministic readiness and safety counts. It does not authorize KG import, semantic acceptance, production writes, source payload copying, model payloads, or secret material.",
        "",
        "## Summary",
        "",
        f"- Assessment schema: {assessment['schema_version']}",
        f"- Independent reviewer: {assessment['reviewer_id']}",
        f"- Assessment verdict: {assessment['verdict']}",
        f"- Import allowed: {str(assessment['import_allowed']).lower()}",
        f"- Semantic KG readiness: {str(assessment['semantic_ready_for_kg']).lower()}",
        f"- Next step: {assessment['next_step']}",
        "",
        "## Unsafe Counter Status",
        "",
        f"- Packet count: {counters['packet_count']}",
        f"- Pending review packets: {counters['pending_review_count']}",
        f"- Accepted packets: {counters['accepted_count']}",
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
        "## Dimension Results",
        "",
        "| Dimension | Status | Blocks import | Finding codes |",
        "|---|---|---|---|",
    ]
    for dimension, result in sorted(dimension_results.items()):
        if not isinstance(result, dict):
            continue
        codes = ", ".join(str(code) for code in result.get("finding_codes", [])) or "none"
        lines.append(f"| {dimension} | {result.get('status', 'unknown')} | {str(result.get('blocks_import')).lower()} | {codes} |")
    lines.extend(["", "## Packet Findings", ""])
    for finding in assessment.get("packet_findings", []) if isinstance(assessment.get("packet_findings"), list) else []:
        if isinstance(finding, dict):
            lines.append(f"- {finding.get('code', 'unknown')} at {finding.get('path', '/')} for {finding.get('packet_id', 'unknown')}")
    lines.append("")
    markdown = "\n".join(lines)
    _validate_markdown(markdown, label="assessment Markdown")
    return markdown


def render_prototype_files(
    repair_prototype_path: Path,
    s02_contract_path: Path,
    json_output: Path,
    markdown_output: Path,
    assessment_json_output: Path,
    assessment_markdown_output: Path,
) -> dict[str, Any]:
    """Build, validate, and write all reviewer packet prototype outputs."""
    repair_payload = load_json_object(repair_prototype_path, label="repair prototype")
    s02_contract = load_json_object(s02_contract_path, label="S02 contract")
    prototype = build_reviewer_packet_prototype(repair_payload, s02_contract=s02_contract)
    assessment = prototype.get("assessment")
    if not isinstance(assessment, dict):
        raise ReviewerPacketPrototypeRenderError("generated assessment root must be a JSON object")

    _validate_packet_payload(prototype)
    _validate_assessment_payload(assessment, packet_count=len(_list_of_dicts(prototype.get("packets"))))
    packet_markdown = render_reviewer_packet_markdown(prototype)
    _validate_markdown(packet_markdown, label="packet Markdown")
    assessment_markdown = render_assessment_markdown(assessment)
    _validate_markdown(assessment_markdown, label="assessment Markdown")

    json_text = json.dumps(prototype, indent=2, sort_keys=True) + "\n"
    assessment_json_text = json.dumps(assessment, indent=2, sort_keys=True) + "\n"
    _write_all_after_validation(
        {
            json_output: json_text,
            markdown_output: packet_markdown,
            assessment_json_output: assessment_json_text,
            assessment_markdown_output: assessment_markdown,
        }
    )

    summary = summarize_reviewer_packet_prototype(prototype)
    counters = assessment["unsafe_counters"]
    return {
        "schema_version": prototype["schema_version"],
        "packet_count": summary["packet_count"],
        "review_status_counts": summary["review_status_counts"],
        "repair_state_counts": summary["repair_state_counts"],
        "route_quality_state_counts": summary["route_quality_state_counts"],
        "assessment_verdict": assessment["verdict"],
        "unsafe_counters_zero": _unsafe_counters_zero(counters),
        "json_output": str(json_output),
        "markdown_output": str(markdown_output),
        "assessment_json_output": str(assessment_json_output),
        "assessment_markdown_output": str(assessment_markdown_output),
    }


def _validate_packet_payload(prototype: dict[str, Any]) -> None:
    if prototype.get("schema_version") != REVIEWER_PACKET_PROTOTYPE_VERSION:
        raise ReviewerPacketPrototypeRenderError("packet JSON schema mismatch at /schema_version")
    for finding in scan_forbidden_payload_keys(prototype):
        raise ReviewerPacketPrototypeRenderError(f"packet JSON forbidden key {finding.code} at {finding.path}")
    packets = _list_of_dicts(prototype.get("packets"))
    if len(packets) <= 0:
        raise ReviewerPacketPrototypeRenderError("packet JSON has no packets at /packets")
    for index, packet in enumerate(packets):
        if packet.get("review_status") != "pending_review":
            raise ReviewerPacketPrototypeRenderError(f"packet JSON unsafe review status at /packets/{index}/review_status object={packet.get('packet_id', '')}")
        if packet.get("importable") is not False:
            raise ReviewerPacketPrototypeRenderError(f"packet JSON importable packet at /packets/{index}/importable object={packet.get('packet_id', '')}")
        if packet.get("semantic_ready_for_kg") is not False:
            raise ReviewerPacketPrototypeRenderError(f"packet JSON semantic-ready packet at /packets/{index}/semantic_ready_for_kg object={packet.get('packet_id', '')}")


def _validate_assessment_payload(assessment: dict[str, Any], *, packet_count: int | None) -> None:
    if assessment.get("schema_version") != REVIEWER_PACKET_ASSESSMENT_VERSION:
        raise ReviewerPacketPrototypeRenderError("assessment JSON schema mismatch at /schema_version")
    for finding in scan_forbidden_payload_keys(assessment):
        raise ReviewerPacketPrototypeRenderError(f"assessment JSON forbidden key {finding.code} at {finding.path}")
    if assessment.get("verdict") in {"accepted", "accepting", "accepted_for_import", "import_ready", "importing"}:
        raise ReviewerPacketPrototypeRenderError("assessment JSON unsafe verdict at /verdict")
    if assessment.get("import_allowed") is not False:
        raise ReviewerPacketPrototypeRenderError("assessment JSON import allowed at /import_allowed")
    if assessment.get("semantic_ready_for_kg") is not False:
        raise ReviewerPacketPrototypeRenderError("assessment JSON semantic ready at /semantic_ready_for_kg")
    counters = assessment.get("unsafe_counters") if isinstance(assessment.get("unsafe_counters"), dict) else None
    if counters is None:
        raise ReviewerPacketPrototypeRenderError("assessment JSON missing unsafe counters at /unsafe_counters")
    if packet_count is not None and counters.get("packet_count") != packet_count:
        raise ReviewerPacketPrototypeRenderError("assessment JSON packet count mismatch at /unsafe_counters/packet_count")
    if not _unsafe_counters_zero(counters):
        raise ReviewerPacketPrototypeRenderError("assessment JSON unsafe counter at /unsafe_counters")


def _validate_markdown(markdown: str, *, label: str) -> None:
    if "```" in markdown:
        raise ReviewerPacketPrototypeRenderError(f"{label} contains forbidden code fence at /")
    for pattern in MARKDOWN_FORBIDDEN_PATTERNS:
        if pattern in markdown:
            raise ReviewerPacketPrototypeRenderError(f"{label} contains forbidden marker at /")
    diagnostics = validate_chunk_repair_contract_markdown(markdown)
    if diagnostics:
        first = diagnostics[0]
        raise ReviewerPacketPrototypeRenderError(f"{label} failed redaction checks: {first.code} at {first.path}")


def _write_all_after_validation(outputs: dict[Path, str]) -> None:
    temp_paths: list[Path] = []
    try:
        for output, text in outputs.items():
            output.parent.mkdir(parents=True, exist_ok=True)
            temp_path = output.with_name(f".{output.name}.tmp-{os.getpid()}")
            temp_path.write_text(text, encoding="utf-8")
            temp_paths.append(temp_path)
        for output, temp_path in zip(outputs.keys(), temp_paths, strict=True):
            temp_path.replace(output)
    finally:
        for temp_path in temp_paths:
            if temp_path.exists():
                temp_path.unlink()


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


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repair-prototype", type=Path, required=True, help="S03 bounded repair prototype JSON")
    parser.add_argument("--s02-contract", type=Path, required=True, help="S02 chunk-repair-contract JSON with stable IDs")
    parser.add_argument("--json-output", type=Path, required=True, help="Destination for reviewer packet JSON")
    parser.add_argument("--markdown-output", type=Path, required=True, help="Destination for reviewer packet Markdown")
    parser.add_argument("--assessment-json-output", type=Path, required=True, help="Destination for standalone assessment JSON")
    parser.add_argument("--assessment-markdown-output", type=Path, required=True, help="Destination for standalone assessment Markdown")
    args = parser.parse_args(argv)

    try:
        summary = render_prototype_files(
            args.repair_prototype,
            args.s02_contract,
            args.json_output,
            args.markdown_output,
            args.assessment_json_output,
            args.assessment_markdown_output,
        )
    except (FileNotFoundError, ReviewerPacketError, ReviewerPacketPrototypeRenderError, ValueError) as exc:
        sys.stderr.write(f"reviewer packet prototype render failed: {exc}\n")
        return 2

    sys.stdout.write(
        "reviewer packet prototype rendered: "
        f"schema={summary['schema_version']} "
        f"packets={summary['packet_count']} "
        f"review_status={summary['review_status_counts']} "
        f"repair_states={summary['repair_state_counts']} "
        f"route_quality={summary['route_quality_state_counts']} "
        f"assessment_verdict={summary['assessment_verdict']} "
        f"unsafe_counters_zero={summary['unsafe_counters_zero']} "
        f"json={summary['json_output']} "
        f"markdown={summary['markdown_output']} "
        f"assessment_json={summary['assessment_json_output']} "
        f"assessment_markdown={summary['assessment_markdown_output']}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
