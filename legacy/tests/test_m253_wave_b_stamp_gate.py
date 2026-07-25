"""M253: Wave B gate opens from durable human_go stamp."""

from __future__ import annotations

from pathlib import Path

from research_graph.application.corpus.wave_b_extraction_baseline import (
    write_human_go_stamp,
)
from research_graph.application.corpus.wave_b_gate import (
    evaluate_wave_b_gate_from_stamp,
)


def test_missing_stamp_blocked(tmp_path: Path) -> None:
    pkg = evaluate_wave_b_gate_from_stamp(tmp_path / "missing.json")
    assert pkg.wave_b_gate_open is False
    assert pkg.gate_signal == "blocked"
    assert pkg.import_eligible is False


def test_valid_stamp_opens_gate(tmp_path: Path) -> None:
    stamp = tmp_path / "human_go.json"
    write_human_go_stamp(stamp, authorized_by="user", decision_ref="D124")
    pkg = evaluate_wave_b_gate_from_stamp(
        stamp,
        wave_a_closeout_pass=True,
        wave_a_closeout_signal="wave_a_closed",
    )
    assert pkg.human_go is True
    assert pkg.wave_b_gate_open is True
    assert pkg.gate_signal == "open"
    assert pkg.import_eligible is False
    assert any("stamp_present:True" in d for d in pkg.diagnostics)


def test_stamp_with_import_true_is_ignored(tmp_path: Path) -> None:
    import json

    stamp = tmp_path / "bad.json"
    stamp.write_text(
        json.dumps(
            {
                "human_go": True,
                "import_eligible": True,
                "decision_ref": "X",
            }
        ),
        encoding="utf-8",
    )
    pkg = evaluate_wave_b_gate_from_stamp(stamp)
    assert pkg.wave_b_gate_open is False
