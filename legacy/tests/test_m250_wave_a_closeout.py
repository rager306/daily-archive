"""M250 S01: Wave A data-readiness closeout (import-blocked)."""

from __future__ import annotations

from research_graph.application.corpus.wave_a_closeout import (
    WaveACloseoutPackage,
    evaluate_wave_a_closeout,
)


def test_closed_when_thresholds_met() -> None:
    pkg = evaluate_wave_a_closeout(
        hybrid_found=49,
        readiness_signal="ready_for_review",
        import_hold_hits=0,
        preprocess_errors=0,
        preprocess_body_count=50,
        article_count=230,
    )
    assert pkg.closeout_signal == "wave_a_closed"
    assert pkg.closeout_pass is True
    assert pkg.import_eligible is False
    assert pkg.wave_b_gate_open is False
    assert pkg.min_hybrid_found == 49
    assert pkg.hybrid_fraction_residual_target == 0.35
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["closeout_pass"] is True
    assert d["wave_b_gate_open"] is False
    assert "hybrid_fraction" in d


def test_blocked_low_hybrid_found() -> None:
    pkg = evaluate_wave_a_closeout(
        hybrid_found=26,
        readiness_signal="ready_for_review",
        import_hold_hits=0,
        preprocess_errors=0,
        preprocess_body_count=26,
        article_count=230,
    )
    assert pkg.closeout_signal == "blocked"
    assert pkg.closeout_pass is False


def test_blocked_hybrid_found_40_under_new_floor() -> None:
    """M257: default min floor raised to 49; 40 is no longer closed."""
    pkg = evaluate_wave_a_closeout(
        hybrid_found=40,
        readiness_signal="ready_for_review",
        import_hold_hits=0,
        preprocess_errors=0,
        preprocess_body_count=41,
        article_count=230,
    )
    assert pkg.closeout_signal == "blocked"
    assert pkg.closeout_pass is False


def test_residual_fraction_tracked_not_blocking_by_default() -> None:
    pkg = evaluate_wave_a_closeout(
        hybrid_found=49,
        readiness_signal="ready_for_review",
        import_hold_hits=0,
        preprocess_errors=0,
        preprocess_body_count=50,
        article_count=230,
        hybrid_fraction=0.213,
    )
    assert pkg.closeout_pass is True
    assert pkg.hybrid_fraction_meets_residual_target is False
    assert pkg.hybrid_fraction == 0.213


def test_require_residual_fraction_blocks() -> None:
    pkg = evaluate_wave_a_closeout(
        hybrid_found=49,
        readiness_signal="ready_for_review",
        import_hold_hits=0,
        preprocess_errors=0,
        preprocess_body_count=50,
        article_count=230,
        hybrid_fraction=0.213,
        require_residual_fraction=True,
    )
    assert pkg.closeout_pass is False
    assert pkg.closeout_signal == "blocked"


def test_blocked_import_hold_hits() -> None:
    pkg = evaluate_wave_a_closeout(
        hybrid_found=50,
        readiness_signal="ready_for_review",
        import_hold_hits=1,
        preprocess_errors=0,
        preprocess_body_count=50,
        article_count=230,
    )
    assert pkg.closeout_signal == "blocked"
    assert pkg.closeout_pass is False


def test_blocked_bad_readiness_signal() -> None:
    pkg = evaluate_wave_a_closeout(
        hybrid_found=40,
        readiness_signal="repair",
        import_hold_hits=0,
        preprocess_errors=0,
        preprocess_body_count=40,
        article_count=230,
    )
    assert pkg.closeout_signal == "blocked"


def test_blocked_preprocess_errors() -> None:
    pkg = evaluate_wave_a_closeout(
        hybrid_found=40,
        readiness_signal="ready_for_review",
        import_hold_hits=0,
        preprocess_errors=3,
        preprocess_body_count=40,
        article_count=230,
    )
    assert pkg.closeout_signal == "blocked"


def test_rejects_import_true() -> None:
    import pytest

    with pytest.raises(ValueError):
        WaveACloseoutPackage(
            schema_version="x",
            closeout_signal="wave_a_closed",
            closeout_pass=True,
            hybrid_found=40,
            min_hybrid_found=40,
            readiness_signal="ready_for_review",
            import_hold_hits=0,
            preprocess_errors=0,
            preprocess_body_count=40,
            article_count=230,
            diagnostics=(),
            operator_commands=(),
            import_eligible=True,
        )


def test_wave_b_gate_never_auto_open() -> None:
    pkg = evaluate_wave_a_closeout(
        hybrid_found=100,
        readiness_signal="ready_for_review",
        import_hold_hits=0,
        preprocess_errors=0,
        preprocess_body_count=100,
        article_count=230,
    )
    assert pkg.closeout_pass is True
    assert pkg.wave_b_gate_open is False
