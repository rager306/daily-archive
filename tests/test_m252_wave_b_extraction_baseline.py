"""M252 S01: Wave B extraction quality baseline (no DSPy, no import)."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.wave_b_extraction_baseline import (
    WaveBExtractionBaselinePackage,
    build_wave_b_extraction_baseline,
    read_human_go_stamp,
    write_human_go_stamp,
)


def test_baseline_scores_m072_fixtures() -> None:
    pkg = build_wave_b_extraction_baseline()
    assert pkg.import_eligible is False
    assert pkg.dspy_optimizer_enabled is False
    assert pkg.graph_writes_allowed is False
    assert pkg.train_case_count >= 1
    assert pkg.validation_case_count >= 1
    assert "entity_f1" in pkg.train_metrics
    assert "relation_f1" in pkg.train_metrics
    assert pkg.gate_verdict in {"proceed", "repair", "stop"}
    assert pkg.leakage_clean is True
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["dspy_optimizer_enabled"] is False
    assert d["wave"] == "B"


def test_rejects_import_true() -> None:
    import pytest

    with pytest.raises(ValueError):
        WaveBExtractionBaselinePackage(
            schema_version="x",
            train_case_count=0,
            validation_case_count=0,
            train_metrics={},
            validation_metrics={},
            gate_verdict="stop",
            gate_reasons=(),
            leakage_clean=True,
            diagnostics=(),
            human_go=True,
            import_eligible=True,
        )


def test_rejects_dspy_true() -> None:
    import pytest

    with pytest.raises(ValueError):
        WaveBExtractionBaselinePackage(
            schema_version="x",
            train_case_count=0,
            validation_case_count=0,
            train_metrics={},
            validation_metrics={},
            gate_verdict="stop",
            gate_reasons=(),
            leakage_clean=True,
            diagnostics=(),
            human_go=True,
            dspy_optimizer_enabled=True,
        )


def test_human_go_stamp_roundtrip(tmp_path: Path) -> None:
    path = tmp_path / "wave_b_human_go.json"
    write_human_go_stamp(
        path,
        authorized_by="user",
        decision_ref="D124",
        note="test stamp",
    )
    stamp = read_human_go_stamp(path)
    assert stamp is not None
    assert stamp["human_go"] is True
    assert stamp["decision_ref"] == "D124"
    assert stamp["import_eligible"] is False
    # missing path
    assert read_human_go_stamp(tmp_path / "missing.json") is None


def test_stamp_file_never_sets_import(tmp_path: Path) -> None:
    path = tmp_path / "wave_b_human_go.json"
    write_human_go_stamp(path, authorized_by="user", decision_ref="D124")
    raw = json.loads(path.read_text(encoding="utf-8"))
    assert raw["import_eligible"] is False
    assert raw["graph_writes_allowed"] is False


def test_write_human_go_stamp_refuses_mutation_without_force(tmp_path: Path) -> None:
    path = tmp_path / "wave_b_human_go.json"
    first = write_human_go_stamp(path, authorized_by="user", decision_ref="D124", note="first")
    at1 = first["authorized_at"]
    second = write_human_go_stamp(
        path, authorized_by="other", decision_ref="D999", note="bump"
    )
    assert second["authorized_at"] == at1
    assert second["decision_ref"] == "D124"
    raw = json.loads(path.read_text(encoding="utf-8"))
    assert raw["authorized_at"] == at1
    assert raw["decision_ref"] == "D124"


def test_write_human_go_stamp_force_rewrite(tmp_path: Path) -> None:
    path = tmp_path / "wave_b_human_go.json"
    first = write_human_go_stamp(path, authorized_by="user", decision_ref="D124")
    at1 = first["authorized_at"]
    second = write_human_go_stamp(
        path,
        authorized_by="user",
        decision_ref="D124",
        note="reauth",
        force_rewrite=True,
    )
    assert second["authorized_at"] != at1
    assert second.get("prior_authorized_at") == at1
    assert "force_rewrite" in str(second.get("note") or "")
