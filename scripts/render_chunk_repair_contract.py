#!/usr/bin/env python3
"""Render the M022/S02 review-only chunk repair contract from a redacted S01 audit.

Usage:
    uv run python scripts/render_chunk_repair_contract.py \
      --audit .gsd/milestones/M022-wvb20v/slices/S01/locator-evidence-audit.json \
      --json-output .gsd/milestones/M022-wvb20v/slices/S02/chunk-repair-contract.json \
      --markdown-output .gsd/milestones/M022-wvb20v/slices/S02/chunk-repair-contract.md

The renderer reads only the trusted local S01 audit JSON. It does not read paper
Markdown/PDF sources, generate embeddings, import KG facts, or write production
LadybugDB artifacts. JSON and Markdown are built and validated in memory before
any output path is written.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))

from research_graph.repair.chunk_repair_contract import (  # noqa: E402
    build_chunk_repair_contract_from_audit,
    expected_audit_from_contract,
    render_chunk_repair_contract_markdown,
    validate_chunk_repair_contract,
    validate_chunk_repair_contract_markdown,
    validate_locator_evidence_audit_for_repair_contract,
    validation_to_dict,
)


class ChunkRepairContractRenderError(ValueError):
    """Raised when the S02 contract renderer must fail closed."""


def load_source_audit(path: str | Path) -> dict[str, Any]:
    """Load one redacted S01 audit JSON with stable, redacted error messages."""
    audit_path = Path(path)
    if not audit_path.exists():
        raise FileNotFoundError(f"source audit file not found: {audit_path}")
    try:
        payload = json.loads(audit_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ChunkRepairContractRenderError(
            f"source audit JSON is malformed at line {exc.lineno} column {exc.colno}"
        ) from exc
    if not isinstance(payload, dict):
        raise ChunkRepairContractRenderError("source audit root must be a JSON object")
    diagnostics = validate_locator_evidence_audit_for_repair_contract(payload)
    if diagnostics:
        codes = ", ".join(sorted({diagnostic.code for diagnostic in diagnostics}))
        raise ChunkRepairContractRenderError(f"source audit failed contract preflight: {codes}")
    return payload


def render_contract_files(audit_path: Path, json_output: Path, markdown_output: Path) -> dict[str, Any]:
    """Build, validate, and write the contract outputs only after all checks pass."""
    audit = load_source_audit(audit_path)
    contract = build_chunk_repair_contract_from_audit(audit, source_audit_path=str(audit_path))
    validation = validate_chunk_repair_contract(contract, expected_audit=expected_audit_from_contract(contract))
    if not validation.passed:
        codes = ", ".join(sorted(validation.refusal_counts))
        raise ChunkRepairContractRenderError(f"rendered contract failed validator: {codes}")
    markdown = render_chunk_repair_contract_markdown(contract)
    markdown_diagnostics = validate_chunk_repair_contract_markdown(markdown)
    if markdown_diagnostics:
        codes = ", ".join(sorted({diagnostic.code for diagnostic in markdown_diagnostics}))
        raise ChunkRepairContractRenderError(f"rendered Markdown failed redaction checks: {codes}")

    json_text = json.dumps(contract, indent=2, sort_keys=True) + "\n"
    json_output.parent.mkdir(parents=True, exist_ok=True)
    markdown_output.parent.mkdir(parents=True, exist_ok=True)
    json_output.write_text(json_text, encoding="utf-8")
    markdown_output.write_text(markdown, encoding="utf-8")
    return {
        "schema_version": contract["schema_version"],
        "source_count": contract["stable_id_counts"]["source_count"],
        "locator_count": contract["stable_id_counts"]["locator_count"],
        "span_count": contract["stable_id_counts"]["span_count"],
        "target_count": validation.target_count,
        "json_output": str(json_output),
        "markdown_output": str(markdown_output),
        "validation": validation_to_dict(validation),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--audit", type=Path, required=True, help="Redacted S01 locator evidence audit JSON")
    parser.add_argument("--json-output", type=Path, required=True, help="Destination for chunk-repair-contract.json")
    parser.add_argument("--markdown-output", type=Path, required=True, help="Destination for chunk-repair-contract.md")
    args = parser.parse_args(argv)

    try:
        summary = render_contract_files(args.audit, args.json_output, args.markdown_output)
    except (FileNotFoundError, ChunkRepairContractRenderError, ValueError) as exc:
        sys.stderr.write(f"chunk repair contract render failed: {exc}\n")
        return 2

    sys.stdout.write(
        "chunk repair contract rendered: "
        f"schema={summary['schema_version']} "
        f"sources={summary['source_count']} "
        f"locators={summary['locator_count']} "
        f"spans={summary['span_count']} "
        f"targets={summary['target_count']} "
        f"json={summary['json_output']} "
        f"markdown={summary['markdown_output']}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
