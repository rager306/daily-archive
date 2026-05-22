#!/usr/bin/env python3
"""Render M022/S03 bounded repair prototype artifacts from S02 and M021 inputs.

The renderer reads only local JSON artifacts: an S02 chunk repair contract and a
redacted M021 deterministic locator batch. It builds JSON and Markdown fully in
memory, validates both outputs, then writes only after all checks pass.
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

from arxiv_archive.bounded_chunk_repair import (  # noqa: E402
    BoundedChunkRepairError,
    build_bounded_chunk_repair_contract,
    render_bounded_chunk_repair_markdown,
    summarize_bounded_chunk_repair_contract,
)
from arxiv_archive.chunk_repair_contract import (  # noqa: E402
    expected_audit_from_contract,
    validate_chunk_repair_contract,
    validate_chunk_repair_contract_markdown,
    validation_to_dict,
)


class BoundedRepairPrototypeRenderError(ValueError):
    """Raised when the S03 renderer must fail closed before writing outputs."""


def load_json_object(path: str | Path, *, label: str) -> dict[str, Any]:
    """Load one JSON object with redacted path/parse errors."""
    json_path = Path(path)
    if not json_path.exists():
        raise FileNotFoundError(f"{label} file not found: {json_path}")
    try:
        payload = json.loads(json_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise BoundedRepairPrototypeRenderError(f"{label} JSON is malformed at line {exc.lineno} column {exc.colno}") from exc
    if not isinstance(payload, dict):
        raise BoundedRepairPrototypeRenderError(f"{label} root must be a JSON object")
    return payload


def render_prototype_files(
    contract_path: Path,
    locator_batch_path: Path,
    json_output: Path,
    markdown_output: Path,
    *,
    max_target_count: int,
) -> dict[str, Any]:
    """Build, validate, and write bounded repair prototype outputs."""
    contract = load_json_object(contract_path, label="S02 contract")
    locator_batch = load_json_object(locator_batch_path, label="locator batch")
    payload = build_bounded_chunk_repair_contract(contract, locator_batch, max_target_count=max_target_count)

    validation = validate_chunk_repair_contract(payload, expected_audit=expected_audit_from_contract(payload))
    if not validation.passed:
        codes = ", ".join(sorted(validation.refusal_counts))
        raise BoundedRepairPrototypeRenderError(f"rendered prototype failed contract validation: {codes}")

    markdown = render_bounded_chunk_repair_markdown(payload)
    markdown_diagnostics = validate_chunk_repair_contract_markdown(markdown)
    if markdown_diagnostics:
        codes = ", ".join(sorted({diagnostic.code for diagnostic in markdown_diagnostics}))
        raise BoundedRepairPrototypeRenderError(f"rendered Markdown failed redaction checks: {codes}")

    json_text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    json_output.parent.mkdir(parents=True, exist_ok=True)
    markdown_output.parent.mkdir(parents=True, exist_ok=True)
    json_output.write_text(json_text, encoding="utf-8")
    markdown_output.write_text(markdown, encoding="utf-8")

    summary = summarize_bounded_chunk_repair_contract(payload)
    return {
        "schema_version": payload["schema_version"],
        "target_count": summary["target_count"],
        "repair_state_counts": summary["repair_state_counts"],
        "route_quality_state_counts": summary["route_quality_state_counts"],
        "unsafe_safety_counters": summary["unsafe_safety_counters"],
        "json_output": str(json_output),
        "markdown_output": str(markdown_output),
        "validation": validation_to_dict(validation),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract", type=Path, required=True, help="S02 chunk-repair-contract JSON")
    parser.add_argument("--locator-batch", type=Path, required=True, help="Redacted deterministic locator batch JSON")
    parser.add_argument("--json-output", type=Path, required=True, help="Destination for bounded repair prototype JSON")
    parser.add_argument("--markdown-output", type=Path, required=True, help="Destination for bounded repair prototype Markdown")
    parser.add_argument("--max-target-count", type=int, default=6, help="Maximum selected repair targets")
    args = parser.parse_args(argv)

    try:
        summary = render_prototype_files(
            args.contract,
            args.locator_batch,
            args.json_output,
            args.markdown_output,
            max_target_count=args.max_target_count,
        )
    except (FileNotFoundError, BoundedChunkRepairError, BoundedRepairPrototypeRenderError, ValueError) as exc:
        sys.stderr.write(f"bounded repair prototype render failed: {exc}\n")
        return 2

    sys.stdout.write(
        "bounded repair prototype rendered: "
        f"schema={summary['schema_version']} "
        f"targets={summary['target_count']} "
        f"repair_states={summary['repair_state_counts']} "
        f"route_quality={summary['route_quality_state_counts']} "
        f"unsafe={summary['unsafe_safety_counters']} "
        f"json={summary['json_output']} "
        f"markdown={summary['markdown_output']}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
