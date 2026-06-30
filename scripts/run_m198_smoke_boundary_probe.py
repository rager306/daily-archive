#!/usr/bin/env python3
"""Convert Universal KB smoke runner output into M198 readiness evidence."""

from __future__ import annotations

import argparse
import json
from datetime import UTC, datetime
from hashlib import sha256
from pathlib import Path
from typing import Any

from research_graph.workflows.universal_kb.smoke_runner import run_article

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"


def _load_contract() -> dict[str, Any]:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def _load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"required smoke artifact missing: {path.name}")
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


def _default_article() -> dict[str, Any]:
    return {
        "candidate_id": "m198-smoke-fixture",
        "article_key": "m198-smoke-fixture",
        "title": "M198 Smoke Fixture",
        "abstract": "Metadata-only readiness smoke fixture.",
        "safety_flags": {
            "graph_write_allowed": False,
            "schema_migration_allowed": False,
            "import_eligible": False,
            "production_import_attempted": False,
            "promotion_allowed": False,
        },
    }


def _validate_article(article: dict[str, Any]) -> None:
    if not article.get("candidate_id"):
        raise ValueError("candidate_id is required for smoke boundary evidence")
    flags = article.get("safety_flags") or {}
    for key in (
        "graph_write_allowed",
        "schema_migration_allowed",
        "import_eligible",
        "production_import_attempted",
        "promotion_allowed",
    ):
        if flags.get(key) is not False:
            raise ValueError(f"article safety flag {key} must be false")


def _json_artifacts(article_dir: Path) -> list[Path]:
    return sorted(path for path in article_dir.glob("*.json") if path.is_file())


def _reject_payload_terms(article_dir: Path, forbidden_terms: list[str]) -> None:
    text = "\n".join(path.read_text(encoding="utf-8") for path in _json_artifacts(article_dir)).lower()
    for term in forbidden_terms:
        if term.lower() in text:
            raise ValueError(f"forbidden payload term found: {term}")


def _require_false(name: str, value: Any) -> None:
    if value is not False:
        raise ValueError(f"{name} must be false for smoke boundary evidence")


def _load_article(path: Path | None) -> dict[str, Any]:
    if path is None:
        return _default_article()
    return _load_json(path)


def run_smoke_boundary(output_dir: Path, article: dict[str, Any]) -> Path:
    _validate_article(article)
    result = run_article(article, output_dir=output_dir)
    result_path = output_dir / "smoke_result.json"
    result_path.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return result_path


def build_evidence(smoke_result_path: Path, *, correlation_id: str) -> dict[str, Any]:
    contract = _load_contract()
    result = _load_json(smoke_result_path)
    article_dir = Path(str(result.get("artifact_dir") or ""))
    if not article_dir.exists():
        raise FileNotFoundError(f"required smoke artifact missing: {article_dir}")

    continuity = _load_json(article_dir / "continuity.json")
    handoff = _load_json(article_dir / "readiness_handoff.json")
    queue_inspect = _load_json(article_dir / "queue_inspect.json")
    _reject_payload_terms(article_dir, contract["forbidden_payload_terms"])

    _require_false("graph_write_allowed", result.get("graph_write_allowed"))
    _require_false("import_eligible", result.get("import_eligible"))
    _require_false("production_import_attempted", result.get("production_import_attempted"))
    _require_false("promotion_allowed", result.get("promotion_allowed"))
    flags = result.get("safety_flags") or {}
    _require_false("safety_flags.graph_write_allowed", flags.get("graph_write_allowed"))
    _require_false("safety_flags.import_eligible", flags.get("import_eligible"))
    _require_false("safety_flags.production_import_attempted", flags.get("production_import_attempted"))
    _require_false("safety_flags.promotion_allowed", flags.get("promotion_allowed"))
    import_eligibility = continuity.get("import_eligibility") or {}
    _require_false("continuity.import_eligible", import_eligibility.get("import_eligible"))
    _require_false("handoff.graph_write_allowed", handoff.get("graph_write_allowed"))
    _require_false("handoff.production_import_attempted", handoff.get("production_import_attempted"))
    _require_false("handoff.promotion_allowed", handoff.get("promotion_allowed"))

    refs = [smoke_result_path, *_json_artifacts(article_dir)]
    checksums = {str(path): _sha256(path) for path in refs if path.exists()}
    events = queue_inspect.get("events") or []
    diagnostics = result.get("diagnostics") or []

    return {
        "schema_version": contract["schema_version"],
        "evidence_id": "m198-smoke-boundary-probe",
        "source_kind": "smoke_boundary",
        "correlation_id": correlation_id,
        "status": "pass",
        "drift_class": "not_applicable",
        "timestamp": datetime.now(UTC).isoformat(),
        "graph_writes_allowed": False,
        "schema_migration_allowed": False,
        "import_eligible": False,
        "evidence_refs": sorted(str(path) for path in refs),
        "diagnostics": {
            "article_key": result.get("article_key"),
            "candidate_id": result.get("candidate_id"),
            "queue_status": result.get("queue_status"),
            "queue_event_count": len(events) if isinstance(events, list) else 0,
            "loader_evidence_status": result.get("loader_evidence_status"),
            "loader_ref_count": result.get("loader_ref_count"),
            "source_ref_count": result.get("source_ref_count"),
            "smoke_diagnostic_count": len(diagnostics) if isinstance(diagnostics, list) else 0,
            "continuity_ref": result.get("continuity_ref"),
            "metadata_only": continuity.get("metadata_only"),
            "production_import_attempted": result.get("production_import_attempted"),
            "promotion_allowed": result.get("promotion_allowed"),
        },
        "non_goals": contract["blocked_transitions"],
        "source_command": "uv run python scripts/run_m198_smoke_boundary_probe.py --artifact-dir <dir> --evidence <evidence.json>",
        "source_artifact_refs": sorted(str(path) for path in refs),
        "source_checksums": checksums,
        "queue_status": result.get("queue_status"),
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact-dir", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--article-json", type=Path)
    parser.add_argument("--smoke-result", type=Path)
    parser.add_argument("--correlation-id", default="m198-smoke-boundary-probe")
    parser.add_argument("--skip-run", action="store_true", help="Build evidence from an existing smoke_result.json.")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    args.artifact_dir.mkdir(parents=True, exist_ok=True)
    smoke_result_path = args.smoke_result or args.artifact_dir / "smoke_result.json"
    if not args.skip_run:
        smoke_result_path = run_smoke_boundary(args.artifact_dir, _load_article(args.article_json))
    evidence = build_evidence(smoke_result_path, correlation_id=args.correlation_id)
    args.evidence.parent.mkdir(parents=True, exist_ok=True)
    args.evidence.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"m198_readiness_evidence={args.evidence}")
    print(f"queue_status={evidence['queue_status']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
