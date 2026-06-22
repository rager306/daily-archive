#!/usr/bin/env python3
"""Verify the M044 sidecar architecture guardrail context pack."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CONTEXT_PACK = (
    ROOT / "artifacts" / "m044-grobid-architecture-guardrail" / "architecture-context-pack.json"
)
REQUIRED_DECISIONS = {"M033", "ADR-003", "ADR-004", "ADR-005", "ADR-007", "D078"}
REQUIRED_SYSTEMS = {
    "current_baseline",
    "grobid",
    "opendataloader_pdf",
    "adaptix",
    "quant_mind_patterns",
    "combined_architecture",
}
REQUIRED_PROHIBITED_CLAIMS = {
    "graph_import_authorized",
    "production_import_authorized",
    "fact_promotion_allowed",
    "sidecar_success_as_semantic_truth",
    "quant_mind_runtime_adopted",
    "raw_payload_promoted",
}
REQUIRED_PACKET_FLAGS = {
    "candidate_only": True,
    "graph_write_allowed": False,
    "promotion_allowed": False,
    "production_import_attempted": False,
    "import_eligible": False,
}
REQUIRED_SOURCE_KEYS = {
    "m033_summary",
    "adr_003",
    "adr_004",
    "adr_005",
    "adr_007",
    "decisions",
    "m043_fit",
}


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def verify_context_pack(pack: dict[str, Any], *, root: Path = ROOT) -> list[str]:
    errors: list[str] = []
    if pack.get("pack_id") != "m044-sidecar-architecture-context-v1":
        errors.append("pack_id must be m044-sidecar-architecture-context-v1")

    source_refs = pack.get("source_refs") if isinstance(pack.get("source_refs"), dict) else {}
    missing_source_keys = REQUIRED_SOURCE_KEYS - set(source_refs)
    if missing_source_keys:
        errors.append(f"missing source refs: {sorted(missing_source_keys)}")
    for key in REQUIRED_SOURCE_KEYS.intersection(source_refs):
        path = root / str(source_refs[key])
        if not path.exists():
            errors.append(f"source ref missing on disk: {key}={source_refs[key]}")

    decisions = (
        pack.get("mandatory_decisions") if isinstance(pack.get("mandatory_decisions"), list) else []
    )
    decision_ids = {str(item.get("id")) for item in decisions if isinstance(item, dict)}
    missing_decisions = REQUIRED_DECISIONS - decision_ids
    if missing_decisions:
        errors.append(f"missing mandatory decisions: {sorted(missing_decisions)}")

    systems = set(pack.get("required_systems", []))
    missing_systems = REQUIRED_SYSTEMS - systems
    if missing_systems:
        errors.append(f"missing required systems: {sorted(missing_systems)}")

    prohibited = set(pack.get("prohibited_claims", []))
    missing_claims = REQUIRED_PROHIBITED_CLAIMS - prohibited
    if missing_claims:
        errors.append(f"missing prohibited claims: {sorted(missing_claims)}")

    flags = (
        pack.get("required_packet_flags")
        if isinstance(pack.get("required_packet_flags"), dict)
        else {}
    )
    for key, expected in REQUIRED_PACKET_FLAGS.items():
        if flags.get(key) is not expected:
            errors.append(f"required_packet_flags.{key} must be {expected}")
        if key != "candidate_only" and pack.get(key) is not False:
            errors.append(f"top-level {key} must be false")

    commands = (
        pack.get("required_preflight_commands")
        if isinstance(pack.get("required_preflight_commands"), list)
        else []
    )
    if "uv run python scripts/verify_m044_sidecar_architecture_guardrail.py" not in commands:
        errors.append("required preflight command missing")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--context-pack", type=Path, default=DEFAULT_CONTEXT_PACK)
    args = parser.parse_args()
    errors = verify_context_pack(load_json(args.context_pack))
    if errors:
        for error in errors:
            sys.stderr.write(f"guardrail error: {error}\n")
        return 1
    sys.stdout.write("m044 sidecar architecture guardrail ok\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
