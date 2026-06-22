#!/usr/bin/env python3
"""Verify M034 R/D consistency audit artifacts.

This verifier is intentionally document-contract oriented. It does not decide
whether the architecture is correct; it checks that the S01 audit package covers
all current Rxxx/Dxxx records and routes every non-final finding for later ADR,
PRD, contract, or user-discussion work.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

VALID_CLASSIFICATIONS = {
    "consistent",
    "historical-scope-only",
    "needs-clarification",
    "superseded-by-new-ADR",
    "conflict-needs-user-decision",
}
ROUTED_CLASSIFICATIONS = {
    "needs-clarification",
    "superseded-by-new-ADR",
    "conflict-needs-user-decision",
}
REQUIRED_SAFETY_MARKERS = [
    "graphdb_selection",
    "sidecar_outputs",
    "candidate evidence",
    "local-first universal knowledge base",
]


def parse_requirement_ids(text: str) -> set[str]:
    return set(re.findall(r"^### (R\d{3}) — ", text, flags=re.MULTILINE))


def parse_decision_ids(text: str) -> set[str]:
    return set(re.findall(r"^\| (D\d{3}) \|", text, flags=re.MULTILINE))


def load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise SystemExit(f"missing required artifact: {path}") from exc
    except json.JSONDecodeError as exc:
        raise SystemExit(f"invalid JSON in {path}: {exc}") from exc


def require(condition: bool, message: str, failures: list[str]) -> None:
    if not condition:
        failures.append(message)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package-dir", required=True, type=Path)
    parser.add_argument("--requirements", required=True, type=Path)
    parser.add_argument("--decisions", required=True, type=Path)
    args = parser.parse_args()

    package_dir: Path = args.package_dir
    failures: list[str] = []

    inventory = load_json(package_dir / "r-d-inventory.json")
    audit = load_json(package_dir / "r-d-consistency-audit.json")
    routes = load_json(package_dir / "correction-routes.json")

    audit_md_path = package_dir / "R-D-CONSISTENCY-AUDIT.md"
    checklist_path = package_dir / "correction-checklist.md"
    open_conflicts_path = package_dir / "open-conflicts-for-user.md"
    summary_path = package_dir / "r-d-inventory-summary.md"

    for path in [audit_md_path, checklist_path, open_conflicts_path, summary_path]:
        require(path.exists(), f"missing markdown artifact: {path}", failures)

    req_ids = parse_requirement_ids(args.requirements.read_text(encoding="utf-8"))
    dec_ids = parse_decision_ids(args.decisions.read_text(encoding="utf-8"))
    inventory_req_ids = {r["id"] for r in inventory.get("requirements", [])}
    inventory_dec_ids = {d["id"] for d in inventory.get("decisions", [])}

    require(
        req_ids == inventory_req_ids,
        "inventory requirement IDs do not match REQUIREMENTS.md",
        failures,
    )
    require(
        dec_ids == inventory_dec_ids, "inventory decision IDs do not match DECISIONS.md", failures
    )
    require(
        not inventory.get("duplicates", {}).get("requirements"),
        "duplicate requirement IDs in inventory",
        failures,
    )
    require(
        not inventory.get("duplicates", {}).get("decisions"),
        "duplicate decision IDs in inventory",
        failures,
    )

    audit_records = audit.get("records", [])
    audit_ids = {(r.get("kind"), r.get("id")) for r in audit_records}
    expected_ids = {("requirement", rid) for rid in req_ids} | {
        ("decision", did) for did in dec_ids
    }
    require(
        audit_ids == expected_ids,
        "audit records do not exactly cover inventory/source R/D IDs",
        failures,
    )

    for record in audit_records:
        classification = record.get("classification")
        require(
            classification in VALID_CLASSIFICATIONS,
            f"invalid classification for {record.get('id')}: {classification}",
            failures,
        )
        require(
            isinstance(record.get("findings"), list) and record["findings"],
            f"missing findings for {record.get('id')}",
            failures,
        )

    routed_ids = {(r.get("kind"), r.get("id")) for r in routes.get("routes", [])}
    needing_routes = {
        (r.get("kind"), r.get("id"))
        for r in audit_records
        if r.get("classification") in ROUTED_CLASSIFICATIONS
    }
    require(
        routed_ids == needing_routes,
        "correction routes do not cover all non-final audit findings",
        failures,
    )

    combined_text = "\n".join(
        [
            json.dumps(audit.get("architecture_frame", {}), ensure_ascii=False),
            audit_md_path.read_text(encoding="utf-8") if audit_md_path.exists() else "",
            checklist_path.read_text(encoding="utf-8") if checklist_path.exists() else "",
            open_conflicts_path.read_text(encoding="utf-8") if open_conflicts_path.exists() else "",
        ]
    ).lower()
    for marker in REQUIRED_SAFETY_MARKERS:
        require(marker.lower() in combined_text, f"missing required marker: {marker}", failures)

    classification_counts: dict[str, int] = {}
    for record in audit_records:
        classification_counts[record["classification"]] = (
            classification_counts.get(record["classification"], 0) + 1
        )
    require(
        classification_counts == audit.get("classification_counts"),
        "classification_counts mismatch audit records",
        failures,
    )

    if failures:
        sys.stderr.write("M034 R/D consistency audit verification failed:\n")
        for failure in failures:
            sys.stderr.write(f"- {failure}\n")
        return 1

    sys.stdout.write("M034 R/D consistency audit verification passed\n")
    sys.stdout.write(
        f"requirements={len(req_ids)} decisions={len(dec_ids)} records={len(audit_records)}\n"
    )
    sys.stdout.write(f"classification_counts={classification_counts}\n")
    sys.stdout.write(f"routed_findings={len(routed_ids)}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
