#!/usr/bin/env python3
"""Run graph-readiness validate-only review and emit M198 readiness evidence."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import UTC, datetime
from hashlib import sha256
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"
MODULE = "research_graph.infrastructure.graph.readiness.review"
RETIRED_ALIAS = ".".join(("arxiv_archive", "graph_readiness_review"))


def _load_contract() -> dict[str, Any]:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def _load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"required graph readiness artifact missing: {path.name}")
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, dict):
        raise ValueError(f"expected object artifact: {path.name}")
    return loaded


def _sha256(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _review_files(review_dir: Path) -> list[Path]:
    return sorted(path for path in review_dir.glob("*.md") if path.is_file())


def _reject_payload_terms(paths: list[Path], forbidden_terms: list[str]) -> None:
    text = "\n".join(path.read_text(encoding="utf-8") for path in paths if path.exists()).lower()
    for term in forbidden_terms:
        if term.lower() in text:
            raise ValueError(f"forbidden payload term found: {term}")


def _require_false(name: str, value: Any) -> None:
    if value is not False:
        raise ValueError(f"{name} must be false for graph readiness validate-only evidence")


def create_completed_review_fixture(review_dir: Path, events_path: Path) -> Path:
    review_dir.mkdir(parents=True, exist_ok=True)
    bundle_path = review_dir / "m198-graph-ready-fixture-review.md"
    summary_path = review_dir / "independent-review-summary.md"
    manifest_path = review_dir / "fixture-manifest.json"
    bundle_path.write_text(
        "# Independent Review Bundle: m198-graph-ready-fixture\n\n"
        "Reviewer Output Contract:\n"
        "- verdict: PASS\n"
        "- output_contract_completed: true\n"
        "- notes: completed metadata-only validation fixture\n",
        encoding="utf-8",
    )
    summary_path.write_text(
        "# Independent Review Summary\n\n"
        "Completed review summary for M198 graph readiness validate-only fixture.\n\n"
        "- verdict: PASS\n"
        "- output_contract_completed: true\n"
        "- placeholder_status: replaced\n",
        encoding="utf-8",
    )
    manifest_path.write_text(
        json.dumps(
            {
                "candidate_id": "m198-graph-ready-fixture",
                "graph_writes_allowed": False,
                "schema_migration_allowed": False,
                "import_eligible": False,
                "production_import_attempted": False,
                "promotion_allowed": False,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    events_path.parent.mkdir(parents=True, exist_ok=True)
    events_path.write_text(
        json.dumps(
            {
                "event": "independent_review.verdict",
                "paper_id": "m198-graph-ready-fixture",
                "verdict": "PASS",
                "output_contract_completed": True,
                "notes": "completed metadata-only validation fixture",
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    return manifest_path


def _run_validator(review_dir: Path, events_path: Path) -> dict[str, Any]:
    command = [
        sys.executable,
        "-m",
        MODULE,
        "--review-dir",
        str(review_dir),
        "--events",
        str(events_path),
        "--validate-only",
        "--require-completed-review",
    ]
    completed = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    if completed.returncode != 0:
        raise RuntimeError(completed.stdout.strip() or completed.stderr.strip())
    loaded = json.loads(completed.stdout)
    if not isinstance(loaded, dict):
        raise ValueError("validator returned non-object JSON")
    return loaded


def _alias_absent() -> bool:
    completed = subprocess.run(
        [sys.executable, "-m", RETIRED_ALIAS, "--help"],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    return completed.returncode != 0


def build_evidence(review_dir: Path, events_path: Path, *, correlation_id: str) -> dict[str, Any]:
    contract = _load_contract()
    manifest = _load_json(review_dir / "fixture-manifest.json")
    _require_false("graph_writes_allowed", manifest.get("graph_writes_allowed"))
    _require_false("schema_migration_allowed", manifest.get("schema_migration_allowed"))
    _require_false("import_eligible", manifest.get("import_eligible"))
    _require_false("production_import_attempted", manifest.get("production_import_attempted"))
    _require_false("promotion_allowed", manifest.get("promotion_allowed"))

    refs = [events_path, review_dir / "fixture-manifest.json", *_review_files(review_dir)]
    _reject_payload_terms(refs, contract["forbidden_payload_terms"])
    result = _run_validator(review_dir, events_path)
    if result.get("ok") is not True:
        raise ValueError(f"validate-only graph readiness failed: {result.get('diagnostics')}")

    return {
        "schema_version": contract["schema_version"],
        "evidence_id": "m198-graph-readiness-validate-only-probe",
        "source_kind": "graph_readiness_validate_only",
        "correlation_id": correlation_id,
        "status": "pass",
        "drift_class": "not_applicable",
        "timestamp": datetime.now(UTC).isoformat(),
        "graph_writes_allowed": False,
        "schema_migration_allowed": False,
        "import_eligible": False,
        "evidence_refs": sorted(str(path) for path in refs),
        "diagnostics": {
            "validator_module": MODULE,
            "retired_alias": RETIRED_ALIAS,
            "retired_alias_absent": _alias_absent(),
            "validate_only": True,
            "require_completed_review": True,
            "validator_ok": result.get("ok"),
            "validator_diagnostics": result.get("diagnostics", []),
            "review_bundle_count": len([path for path in _review_files(review_dir) if path.name != "independent-review-summary.md"]),
        },
        "non_goals": contract["blocked_transitions"],
        "source_command": (
            f"uv run python -m {MODULE} --review-dir <review-dir> --events <events.jsonl> "
            "--validate-only --require-completed-review"
        ),
        "source_artifact_refs": sorted(str(path) for path in refs),
        "source_checksums": {str(path): _sha256(path) for path in refs if path.exists()},
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--review-dir", required=True, type=Path)
    parser.add_argument("--events", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--correlation-id", default="m198-graph-readiness-probe")
    parser.add_argument("--skip-fixture", action="store_true", help="Validate an existing review fixture.")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    if not args.skip_fixture:
        create_completed_review_fixture(args.review_dir, args.events)
    evidence = build_evidence(args.review_dir, args.events, correlation_id=args.correlation_id)
    args.evidence.parent.mkdir(parents=True, exist_ok=True)
    args.evidence.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"m198_readiness_evidence={args.evidence}")
    print(f"retired_alias_absent={evidence['diagnostics']['retired_alias_absent']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
