from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "scripts" / "check_project_trajectory.py"
spec = importlib.util.spec_from_file_location("check_project_trajectory", MODULE_PATH)
assert spec is not None
traj = importlib.util.module_from_spec(spec)
sys.modules["check_project_trajectory"] = traj
assert spec.loader is not None
spec.loader.exec_module(traj)


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload), encoding="utf-8")


def _project(tmp_path: Path, *, readme: str | None = None, project_extra: str = "") -> Path:
    _write(tmp_path / ".gsd" / "PROJECT.md", "# Project\n\nNo graph import is authorized.\n" + project_extra)
    _write(
        tmp_path / ".gsd" / "REQUIREMENTS.md",
        "# Requirements\n\n### R001 — Unified trajectory check\n- Status: active\n",
    )
    _write(
        tmp_path / ".gsd" / "DECISIONS.md",
        "| # | When | Scope | Decision | Choice | Rationale | Revisable? | Made By |\n"
        "|---|---|---|---|---|---|---|---|\n"
        "| D001 | M001 | architecture | Decide trajectory | Thin wrapper | Avoid drift | Yes | collaborative |\n",
    )
    _write(tmp_path / "doc" / "adr" / "m034" / "ADR-001.md", "# ADR-001\n")
    _write_json(
        tmp_path / ".codebase-memory" / "governance-graph.json",
        {"nodes": [{"type": "Decision"}, {"type": "Requirement"}], "edges": []},
    )
    _write(tmp_path / ".codebase-memory" / "adr.md", "# mirror\n")
    _write(
        tmp_path / ".gsd" / "milestones" / "M001" / "M001-SUMMARY.md",
        "---\ntitle: \"First milestone\"\nstatus: complete\n---\nNo graph import is authorized.\n",
    )
    _write(tmp_path / "README.md", readme if readme is not None else "# README\n\nM001 complete. Next gate: continue safely.\n")
    return tmp_path


def test_build_report_healthy_project_with_codebase_memory_snapshot(tmp_path, monkeypatch):
    root = _project(tmp_path)
    snapshot = tmp_path / "cbm.json"
    _write_json(snapshot, {"project": "root-daily-archive", "results": 2})
    monkeypatch.setattr(traj, "ROOT", root)

    report = traj.build_report(root=root, codebase_memory_snapshot=snapshot)

    assert report["verdict"] in {"on_track", "drift_risk"}
    assert set(report["dimensions"]) == set(traj.DIMENSIONS)
    assert report["codebase_memory"] == {
        "provided": True,
        "canonical": False,
        "snapshot": {"project": "root-daily-archive", "results": 2},
    }
    assert report["graph_write_allowed"] is False


def test_build_report_flags_missing_latest_readme_reference(tmp_path, monkeypatch):
    root = _project(tmp_path, readme="# README\n\nNext gate: continue safely.\n")
    monkeypatch.setattr(traj, "ROOT", root)

    report = traj.build_report(root=root)

    assert any(flag["flag"] == "latest_milestone_missing_readme_reference" for flag in report["drift_flags"])
    assert report["verdict"] == "drift_risk"


def test_build_report_flags_missing_governance_mirror(tmp_path, monkeypatch):
    root = _project(tmp_path)
    (root / ".codebase-memory" / "governance-graph.json").unlink()
    monkeypatch.setattr(traj, "ROOT", root)

    report = traj.build_report(root=root)

    assert any(flag["flag"] == "governance_mirror_missing" for flag in report["drift_flags"])
    assert report["verdict"] == "blocked"


def test_build_report_flags_prohibited_claim_without_counterterm(tmp_path, monkeypatch):
    root = _project(
        tmp_path,
        readme="# README\n\nM001 complete. Next gate: continue safely. Production import is now authorized for parser outputs.\n",
    )
    monkeypatch.setattr(traj, "ROOT", root)

    report = traj.build_report(root=root)

    assert any(flag["flag"] == "production_import_authorized" for flag in report["drift_flags"])
    assert report["verdict"] == "blocked"


def test_build_report_does_not_flag_no_import_counterterm(tmp_path, monkeypatch):
    root = _project(tmp_path, project_extra="\nNo graph import is authorized by this report.\n")
    monkeypatch.setattr(traj, "ROOT", root)

    report = traj.build_report(root=root)

    assert not any(flag["flag"] == "graph_import_authorized" for flag in report["drift_flags"])


def test_render_markdown_contains_dimensions_and_next_actions(tmp_path, monkeypatch):
    root = _project(tmp_path)
    monkeypatch.setattr(traj, "ROOT", root)
    report = traj.build_report(root=root)

    markdown = traj.render_markdown(report)

    assert "Project Trajectory Report" in markdown
    assert "module_code" in markdown
    assert "Next actions" in markdown
    assert "codebase-memory snapshot provided" in markdown
