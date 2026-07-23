"""M251 S01: Wave B gate ratchet (default blocked, import-closed)."""

from __future__ import annotations

from research_graph.application.corpus.wave_b_gate import (
    WaveBGatePackage,
    evaluate_wave_b_gate,
)


def test_default_blocked_without_human_go() -> None:
    pkg = evaluate_wave_b_gate()
    assert pkg.wave_b_gate_open is False
    assert pkg.gate_signal == "blocked"
    assert pkg.import_eligible is False
    assert pkg.human_go is False
    d = pkg.to_dict()
    assert d["wave_b_gate_open"] is False
    assert d["import_eligible"] is False


def test_closeout_pass_alone_does_not_open() -> None:
    pkg = evaluate_wave_b_gate(
        human_go=False,
        wave_a_closeout_pass=True,
        wave_a_closeout_signal="wave_a_closed",
    )
    assert pkg.wave_b_gate_open is False
    assert pkg.gate_signal == "blocked"
    assert "closeout_not_authorization" in " ".join(pkg.diagnostics)


def test_human_go_opens_gate_still_import_false() -> None:
    pkg = evaluate_wave_b_gate(
        human_go=True,
        wave_a_closeout_pass=True,
        wave_a_closeout_signal="wave_a_closed",
    )
    assert pkg.wave_b_gate_open is True
    assert pkg.gate_signal == "open"
    assert pkg.import_eligible is False
    assert pkg.graph_writes_allowed is False


def test_human_go_without_closeout_still_open_but_warned() -> None:
    """Human go is sole authorizer; missing closeout is diagnostic only."""
    pkg = evaluate_wave_b_gate(human_go=True, wave_a_closeout_pass=False)
    assert pkg.wave_b_gate_open is True
    assert pkg.gate_signal == "open"
    assert any("closeout_pass_false" in d for d in pkg.diagnostics)


def test_rejects_import_true() -> None:
    import pytest

    with pytest.raises(ValueError):
        WaveBGatePackage(
            schema_version="x",
            gate_signal="blocked",
            wave_b_gate_open=False,
            human_go=False,
            wave_a_closeout_pass=None,
            wave_a_closeout_signal=None,
            diagnostics=(),
            import_eligible=True,
        )


def test_rejects_open_without_human_go_flag() -> None:
    import pytest

    with pytest.raises(ValueError):
        WaveBGatePackage(
            schema_version="x",
            gate_signal="open",
            wave_b_gate_open=True,
            human_go=False,
            wave_a_closeout_pass=True,
            wave_a_closeout_signal="wave_a_closed",
            diagnostics=(),
        )
