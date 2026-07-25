#!/usr/bin/env python3
"""Build a metadata-only index over M198 readiness evidence files."""

from __future__ import annotations

import argparse
import json
from datetime import UTC, datetime
from hashlib import sha256
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"
INDEX_SCHEMA_VERSION = "m198.readiness_evidence_index.v1"
REQUIRED_SOURCE_KINDS = (
    "reactive_dry_run",
    "sync_no_write_rehearsal",
    "smoke_boundary",
    "graph_readiness_validate_only",
    "governance_ratchet",
)


def _load_contract() -> dict[str, Any]:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def _sha256(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _load_json(path: Path) -> dict[str, Any]:
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, dict):
        raise ValueError(f"expected JSON object: {path}")
    return loaded


def _load_expected_checksums(path: Path | None) -> dict[str, str]:
    if path is None:
        return {}
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, dict):
        raise ValueError("expected checksum map object")
    return {str(key): str(value) for key, value in loaded.items()}


def _forbidden_terms(raw_text: str, forbidden_terms: list[str]) -> list[str]:
    lowered = raw_text.lower()
    return [term for term in forbidden_terms if term.lower() in lowered]


def _entry(path: Path, evidence: dict[str, Any], file_checksum: str) -> dict[str, Any]:
    diagnostics = evidence.get("diagnostics") or {}
    warnings = diagnostics.get("warnings") if isinstance(diagnostics, dict) else None
    blockers = diagnostics.get("blockers") if isinstance(diagnostics, dict) else None
    return {
        "path": str(path),
        "file_checksum": file_checksum,
        "schema_version": evidence.get("schema_version"),
        "source_kind": evidence.get("source_kind"),
        "evidence_id": evidence.get("evidence_id"),
        "status": evidence.get("status"),
        "drift_class": evidence.get("drift_class"),
        "graph_writes_allowed": evidence.get("graph_writes_allowed"),
        "schema_migration_allowed": evidence.get("schema_migration_allowed"),
        "import_eligible": evidence.get("import_eligible"),
        "evidence_ref_count": len(evidence.get("evidence_refs") or []),
        "source_checksum_count": len(evidence.get("source_checksums") or {}),
        "non_goals": sorted(evidence.get("non_goals") or []),
        "warning_count": len(warnings) if isinstance(warnings, list) else 0,
        "blocker_count": len(blockers) if isinstance(blockers, list) else 0,
    }


def build_index(paths: list[Path], *, expected_checksums: dict[str, str]) -> dict[str, Any]:
    contract = _load_contract()
    entries: list[dict[str, Any]] = []
    blockers: list[str] = []
    warnings: list[str] = []
    seen: dict[str, str] = {}
    observed: set[str] = set()
    non_goal_coverage: set[str] = set()

    for path in paths:
        raw_text = path.read_text(encoding="utf-8")
        checksum = _sha256(path)
        expected = expected_checksums.get(str(path))
        if expected is not None and expected != checksum:
            blockers.append(f"checksum mismatch for {path}")
        for term in _forbidden_terms(raw_text, contract["forbidden_payload_terms"]):
            blockers.append(f"{path} contains forbidden payload term: {term}")
        evidence = json.loads(raw_text)
        if not isinstance(evidence, dict):
            blockers.append(f"{path} is not a JSON object")
            continue
        source_kind = str(evidence.get("source_kind"))
        if source_kind in seen:
            blockers.append(f"duplicate source kind: {source_kind}")
        else:
            seen[source_kind] = str(path)
        observed.add(source_kind)
        for field in ("graph_writes_allowed", "schema_migration_allowed", "import_eligible"):
            if evidence.get(field) is not False:
                blockers.append(f"{source_kind} has {field}={evidence.get(field)!r}")
        if not evidence.get("evidence_refs"):
            blockers.append(f"{source_kind} has no evidence_refs")
        non_goal_coverage.update(str(item) for item in (evidence.get("non_goals") or []))
        entries.append(_entry(path, evidence, checksum))

    for source_kind in REQUIRED_SOURCE_KINDS:
        if source_kind not in observed:
            blockers.append(f"missing required source kind: {source_kind}")

    extra_sources = sorted(kind for kind in observed if kind not in REQUIRED_SOURCE_KINDS)
    if extra_sources:
        warnings.append(f"extra source kinds indexed: {', '.join(extra_sources)}")

    missing_non_goals = sorted(set(contract["blocked_transitions"]) - non_goal_coverage)
    if missing_non_goals:
        warnings.append(f"missing non-goal coverage: {', '.join(missing_non_goals)}")

    status = "fail" if blockers else "pass"
    return {
        "schema_version": INDEX_SCHEMA_VERSION,
        "generated_at": datetime.now(UTC).isoformat(),
        "status": status,
        "required_source_kinds": list(REQUIRED_SOURCE_KINDS),
        "observed_source_kinds": sorted(observed),
        "missing_source_kinds": [kind for kind in REQUIRED_SOURCE_KINDS if kind not in observed],
        "entry_count": len(entries),
        "entries": sorted(entries, key=lambda item: str(item["source_kind"])),
        "non_goal_coverage": sorted(non_goal_coverage),
        "warnings": warnings,
        "blockers": blockers,
        "metadata_only": True,
        "payload_policy": {
            "stores_paths": True,
            "stores_checksums": True,
            "stores_payload_text": False,
            "stores_embeddings": False,
            "stores_vectors": False,
            "stores_credentials": False,
            "stores_queue_database_bytes": False,
        },
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path, action="append")
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--expected-checksums", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    index = build_index(args.evidence, expected_checksums=_load_expected_checksums(args.expected_checksums))
    args.index.parent.mkdir(parents=True, exist_ok=True)
    args.index.write_text(json.dumps(index, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"m198_evidence_index={args.index}")
    print(f"status={index['status']}")
    return 2 if index["status"] == "fail" else 0


if __name__ == "__main__":
    raise SystemExit(main())
