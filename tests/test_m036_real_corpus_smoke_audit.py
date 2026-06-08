from __future__ import annotations

import json
from pathlib import Path

from scripts.audit_m036_real_corpus_smoke import audit_smoke, write_markdown_report


def _write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")


def _fixtures(tmp_path: Path) -> tuple[Path, Path]:
    manifest = {
        "schema_version": "m036-real-corpus-smoke-manifest.v1",
        "article_count": 2,
        "safety_flags": {
            "graph_write_allowed": False,
            "promotion_allowed": False,
            "production_import_attempted": False,
            "import_eligible": False,
        },
        "articles": [
            {
                "article_key": "a1",
                "candidate_id": "real-article:a1",
                "evidence_refs": ["artifact:data/a1/article.json", "artifact:data/a1/source/article.html"],
                "source_file_count": 1,
                "loader_ref_count": 1,
                "diagnostics": ["safety_flags_missing_or_not_false"],
                "safety_flags": {
                    "graph_write_allowed": False,
                    "promotion_allowed": False,
                    "production_import_attempted": False,
                    "import_eligible": False,
                },
            },
            {
                "article_key": "a2",
                "candidate_id": "real-article:a2",
                "evidence_refs": ["artifact:data/a2/article.json"],
                "source_file_count": 1,
                "loader_ref_count": 0,
                "diagnostics": ["missing_loader_evidence"],
                "safety_flags": {
                    "graph_write_allowed": False,
                    "promotion_allowed": False,
                    "production_import_attempted": False,
                    "import_eligible": False,
                },
            },
        ],
    }
    run_summary = {
        "schema_version": "m036-real-corpus-no-write-smoke-summary.v1",
        "article_count": 2,
        "completed_handoff_count": 2,
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
        "articles": [
            {
                "article_key": "a1",
                "candidate_id": "real-article:a1",
                "queue_status": "ready",
                "artifact_dir": str(tmp_path / "run" / "articles" / "a1"),
                "graph_write_allowed": False,
                "promotion_allowed": False,
                "production_import_attempted": False,
                "import_eligible": False,
            },
            {
                "article_key": "a2",
                "candidate_id": "real-article:a2",
                "queue_status": "ready",
                "artifact_dir": str(tmp_path / "run" / "articles" / "a2"),
                "graph_write_allowed": False,
                "promotion_allowed": False,
                "production_import_attempted": False,
                "import_eligible": False,
            },
        ],
    }
    manifest_path = tmp_path / "manifest.json"
    run_dir = tmp_path / "run"
    _write_json(manifest_path, manifest)
    _write_json(run_dir / "summary.json", run_summary)
    for item in run_summary["articles"]:
        article_dir = Path(item["artifact_dir"])
        for name in ["candidate.json", "review_packet.json", "review_trace.json", "queue_inspect.json", "readiness_handoff.json"]:
            _write_json(article_dir / name, {"name": name, "graph_write_allowed": False, "promotion_allowed": False, "production_import_attempted": False})
    return manifest_path, run_dir


def test_audit_smoke_reports_coverage_diagnostics_and_no_write_flags(tmp_path: Path) -> None:
    manifest_path, run_dir = _fixtures(tmp_path)

    audit = audit_smoke(manifest_path, run_dir)

    assert audit["article_count"] == 2
    assert audit["completed_handoff_count"] == 2
    assert audit["safety"]["graph_write_allowed"] is False
    assert audit["safety"]["promotion_allowed"] is False
    assert audit["safety"]["production_import_attempted"] is False
    assert audit["safety"]["import_eligible"] is False
    assert audit["coverage"]["articles_with_source_refs"] == 2
    assert audit["coverage"]["articles_with_loader_refs"] == 1
    assert audit["diagnostics"]["missing_loader_evidence"] == 1
    assert audit["diagnostics"]["safety_flags_missing_or_not_false"] == 1
    assert "missing_loader_evidence" in audit["blockers_for_import"]
    assert "legacy_or_missing_article_safety_flags" in audit["blockers_for_import"]


def test_write_markdown_report_contains_next_step_boundaries(tmp_path: Path) -> None:
    manifest_path, run_dir = _fixtures(tmp_path)
    audit = audit_smoke(manifest_path, run_dir)
    output = tmp_path / "audit.md"

    write_markdown_report(audit, output)

    text = output.read_text(encoding="utf-8")
    assert "# M036 Real Corpus No Write Smoke Audit" in text
    assert "GraphDB write: false" in text
    assert "missing_loader_evidence" in text
    assert "Next safe step" in text
