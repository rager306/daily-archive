from __future__ import annotations

import json
from pathlib import Path

import pytest

# pyrefly: ignore [missing-import]
from scripts.audit_m036_real_corpus_smoke import audit_smoke, write_markdown_report

FALSE_FLAGS = {
    "graph_write_allowed": False,
    "promotion_allowed": False,
    "production_import_attempted": False,
    "import_eligible": False,
}


def _write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")


def _continuity(article_key: str, candidate_id: str, *, loader_present: bool) -> dict:
    loader_refs = [f"artifact:data/{article_key}/loader/summary.json"] if loader_present else []
    diagnostics = [
        "loader_evidence_present" if loader_present else "loader_evidence_absent_explicit",
        "article_safety_flags_explicit_false",
    ]
    return {
        "schema_version": "m040-real-corpus-continuity.v1",
        "article_key": article_key,
        "candidate_id": candidate_id,
        "candidate_type": "real_article_metadata",
        "safety_flags": dict(FALSE_FLAGS),
        "source_evidence": {
            "status": "present",
            "refs": [f"artifact:data/{article_key}/article.json"],
            "ref_count": 1,
        },
        "loader_evidence": {
            "status": "present" if loader_present else "absent_explicit",
            "refs": loader_refs,
            "ref_count": len(loader_refs),
            "diagnostic": None if loader_present else "loader_evidence_absent_explicit",
        },
        "diagnostics": diagnostics,
        "import_eligibility": {
            "import_eligible": False,
            "reason": "real_corpus_no_write_smoke_continuity_only",
        },
        "metadata_only": True,
    }


def _fixtures(tmp_path: Path) -> tuple[Path, Path]:
    manifest = {
        "schema_version": "m036-real-corpus-smoke-manifest.v1",
        "article_count": 2,
        "safety_flags": dict(FALSE_FLAGS),
        "articles": [
            {
                "article_key": "a1",
                "candidate_id": "real-article:a1",
                "evidence_refs": [
                    "artifact:data/a1/article.json",
                    "artifact:data/a1/source/article.html",
                    "artifact:data/a1/loader/summary.json",
                ],
                "source_file_count": 1,
                "loader_ref_count": 1,
                "diagnostics": ["safety_flags_missing_or_not_false"],
                "safety_flags": dict(FALSE_FLAGS),
            },
            {
                "article_key": "a2",
                "candidate_id": "real-article:a2",
                "evidence_refs": ["artifact:data/a2/article.json"],
                "source_file_count": 1,
                "loader_ref_count": 0,
                "diagnostics": ["missing_loader_evidence"],
                "safety_flags": dict(FALSE_FLAGS),
            },
        ],
    }
    run_summary = {
        "schema_version": "m036-real-corpus-no-write-smoke-summary.v1",
        "article_count": 2,
        "completed_handoff_count": 2,
        **FALSE_FLAGS,
        "articles": [
            {
                "article_key": "a1",
                "candidate_id": "real-article:a1",
                "queue_status": "ready",
                "artifact_dir": str(tmp_path / "run" / "articles" / "a1"),
                "safety_flags": dict(FALSE_FLAGS),
                **FALSE_FLAGS,
            },
            {
                "article_key": "a2",
                "candidate_id": "real-article:a2",
                "queue_status": "ready",
                "artifact_dir": str(tmp_path / "run" / "articles" / "a2"),
                "safety_flags": dict(FALSE_FLAGS),
                **FALSE_FLAGS,
            },
        ],
    }
    manifest_path = tmp_path / "manifest.json"
    run_dir = tmp_path / "run"
    _write_json(manifest_path, manifest)
    _write_json(run_dir / "summary.json", run_summary)
    for item in run_summary["articles"]:
        # pyrefly: ignore [bad-argument-type]
        article_dir = Path(item["artifact_dir"])  # ty:ignore[invalid-argument-type]
        for name in [
            "candidate.json",
            "review_packet.json",
            "review_trace.json",
            "queue_inspect.json",
            "readiness_handoff.json",
        ]:
            _write_json(article_dir / name, {"name": name, **FALSE_FLAGS})
        _write_json(
            article_dir / "continuity.json",
            _continuity(
                str(item["article_key"]),
                str(item["candidate_id"]),
                loader_present=item["article_key"] == "a1",
            ),
        )
    return manifest_path, run_dir


def test_audit_smoke_reports_normalized_continuity_and_no_write_flags(tmp_path: Path) -> None:
    manifest_path, run_dir = _fixtures(tmp_path)

    audit = audit_smoke(manifest_path, run_dir)

    assert audit["article_count"] == 2
    assert audit["completed_handoff_count"] == 2
    assert audit["safety"] == dict(FALSE_FLAGS)
    assert audit["coverage"]["articles_with_source_refs"] == 2
    assert audit["coverage"]["articles_with_loader_refs"] == 1
    assert audit["coverage"]["articles_with_explicit_loader_absence"] == 1
    assert audit["coverage"]["continuity_artifacts_present"] == 2
    assert audit["diagnostics"]["loader_evidence_absent_explicit"] == 1
    assert audit["diagnostics"]["article_safety_flags_explicit_false"] == 2
    assert "missing_loader_evidence" not in audit["blockers_for_import"]
    assert "legacy_or_missing_article_safety_flags" not in audit["blockers_for_import"]
    assert audit["blockers_for_import"] == []


def test_audit_smoke_blocks_missing_continuity_metadata(tmp_path: Path) -> None:
    manifest_path, run_dir = _fixtures(tmp_path)
    (run_dir / "articles" / "a2" / "continuity.json").unlink()

    audit = audit_smoke(manifest_path, run_dir)

    assert "missing_continuity_metadata" in audit["blockers_for_import"]
    assert "missing_smoke_artifacts" in audit["blockers_for_import"]


def test_audit_smoke_rejects_continuity_that_claims_import_authority(tmp_path: Path) -> None:
    manifest_path, run_dir = _fixtures(tmp_path)
    continuity_path = run_dir / "articles" / "a1" / "continuity.json"
    continuity = json.loads(continuity_path.read_text(encoding="utf-8"))
    continuity["safety_flags"]["import_eligible"] = True
    _write_json(continuity_path, continuity)

    with pytest.raises(ValueError, match="continuity contains true"):
        audit_smoke(manifest_path, run_dir)


def test_write_markdown_report_contains_next_step_boundaries(tmp_path: Path) -> None:
    manifest_path, run_dir = _fixtures(tmp_path)
    audit = audit_smoke(manifest_path, run_dir)
    output = tmp_path / "audit.md"

    write_markdown_report(audit, output)

    text = output.read_text(encoding="utf-8")
    assert "# M036 Real Corpus No Write Smoke Audit" in text
    assert "GraphDB write: false" in text
    assert "Loader absence explicit: 1/2" in text
    assert "none for no-write smoke scope" in text
    assert "Next safe step" in text
