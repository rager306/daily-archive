from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.universal_kb_smoke import SmokePaths, run_all, run_verify, write_manifest


def test_run_all_fast_profile_executes_selector_runner_and_audit(tmp_path: Path) -> None:
    paths = SmokePaths(base_dir=tmp_path / "smoke")

    result = run_all(limit=3, profile="fast", paths=paths)

    assert result["profile"] == "fast"
    assert result["article_count"] == 3
    assert result["completed_handoff_count"] == 3
    assert result["graph_write_allowed"] is False
    assert result["promotion_allowed"] is False
    assert result["production_import_attempted"] is False
    assert result["import_eligible"] is False
    assert paths.manifest.exists()
    assert paths.run_summary.exists()
    assert paths.audit_json.exists()
    audit = json.loads(paths.audit_json.read_text(encoding="utf-8"))
    assert audit["article_count"] == 3
    assert audit["blockers_for_import"] == []
    assert audit["coverage"]["continuity_artifacts_present"] == 3


def test_run_verify_fast_profile_reuses_current_artifacts(tmp_path: Path) -> None:
    paths = SmokePaths(base_dir=tmp_path / "smoke")
    run_all(limit=3, profile="fast", paths=paths)

    result = run_verify(profile="fast", paths=paths)

    assert result["profile"] == "fast"
    assert result["article_count"] == 3
    assert result["completed_handoff_count"] == 3
    assert result["json_artifacts_scanned"] > 0
    assert result["graph_write_allowed"] is False


def test_run_verify_rejects_persisted_true_safety_flags(tmp_path: Path) -> None:
    paths = SmokePaths(base_dir=tmp_path / "smoke")
    run_all(limit=3, profile="fast", paths=paths)
    summary = json.loads(paths.run_summary.read_text(encoding="utf-8"))
    summary["graph_write_allowed"] = True
    paths.run_summary.write_text(json.dumps(summary), encoding="utf-8")

    try:
        run_verify(profile="fast", paths=paths)
    except ValueError as exc:
        assert "summary.graph_write_allowed" in str(exc)
    else:  # pragma: no cover - explicit failure branch
        raise AssertionError("true persisted safety flag should fail")


def test_run_verify_allows_normalized_batch_expansion(tmp_path: Path) -> None:
    paths = SmokePaths(base_dir=tmp_path / "smoke")

    result = run_all(limit=10, profile="fast", paths=paths)

    assert result["article_count"] == 10
    assert result["completed_handoff_count"] == 10
    assert result["blockers_for_import"] == []
    verified = run_verify(profile="fast", paths=paths)
    assert verified["article_count"] == 10


def test_write_manifest_rejects_more_than_thirty_articles(tmp_path: Path) -> None:
    paths = SmokePaths(base_dir=tmp_path / "smoke")

    try:
        write_manifest(paths.manifest, limit=31)
    except ValueError as exc:
        assert "between 3 and 30" in str(exc)
    else:  # pragma: no cover - explicit failure branch
        raise AssertionError("write_manifest must reject unsafe large limits")


def test_run_verify_rejects_more_than_thirty_articles(tmp_path: Path) -> None:
    paths = SmokePaths(base_dir=tmp_path / "smoke")
    run_all(limit=3, profile="fast", paths=paths)
    summary = json.loads(paths.run_summary.read_text(encoding="utf-8"))
    summary["article_count"] = 31
    summary["completed_handoff_count"] = 31
    paths.run_summary.write_text(json.dumps(summary), encoding="utf-8")

    try:
        run_verify(profile="fast", paths=paths)
    except AssertionError as exc:
        assert "between 3 and 30" in str(exc)
    else:  # pragma: no cover - explicit failure branch
        raise AssertionError("verify must reject unsafe batch expansion")


def test_run_verify_rejects_unknown_profile(tmp_path: Path) -> None:
    paths = SmokePaths(base_dir=tmp_path / "smoke")

    try:
        run_verify(profile="release", paths=paths)
    except ValueError as exc:
        assert "profile" in str(exc)
    else:  # pragma: no cover - explicit failure branch
        raise AssertionError("unknown profile should fail")
