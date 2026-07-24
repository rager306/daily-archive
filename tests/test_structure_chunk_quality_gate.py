"""Tests for continuous structure chunk quality gate (M273)."""

from __future__ import annotations

from pathlib import Path

from research_graph.application.corpus.structure_chunk_quality_gate import (
    CONTINUITY_GAP_CODE,
    evaluate_structure_chunk_quality_gate,
)


def _write_body(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def test_gate_pass_clears_continuity_gap(tmp_path: Path) -> None:
    good = (
        "# Introduction\n\n"
        "We present a method for grounded attribute learning using language and perception. "
        "The approach combines neural models with structured reasoning over attributes. "
        "Experiments show improvements on standard benchmarks with clear methodology.\n\n"
        "## Method\n\n"
        "Our Neural Machine Translation baseline uses attention and subword units for rare words. "
        "Training uses standard optimizers and evaluation metrics reported in tables.\n\n"
        "## Conclusion\n\n"
        "We conclude that structure-aware extraction helps scientific knowledge graphs.\n"
    )
    root = tmp_path / "bodies"
    for i in range(12):
        _write_body(root / f"p{i}" / "body" / f"p{i}.hybrid.body.md", good)
    pkg = evaluate_structure_chunk_quality_gate(
        [root], sample_limit=12, min_sample=10, min_pass_rate=0.55
    )
    assert pkg.import_eligible is False
    assert pkg.sampled >= 10
    assert pkg.gate_signal == "pass"
    assert pkg.continuity_gap_cleared is True
    assert CONTINUITY_GAP_CODE in pkg.to_dict()["gap_code"]


def test_gate_blocked_on_empty_roots(tmp_path: Path) -> None:
    empty = tmp_path / "empty"
    empty.mkdir()
    pkg = evaluate_structure_chunk_quality_gate(
        [empty], sample_limit=10, min_sample=10, min_pass_rate=0.55
    )
    assert pkg.sampled == 0
    assert pkg.gate_signal == "blocked"
    assert pkg.continuity_gap_cleared is False


def test_gate_partial_on_junk(tmp_path: Path) -> None:
    root = tmp_path / "junk"
    for i in range(12):
        _write_body(root / f"j{i}.hybrid.body.md", "###\n!!!\n$$$")
    pkg = evaluate_structure_chunk_quality_gate(
        [root], sample_limit=12, min_sample=10, min_pass_rate=0.55
    )
    assert pkg.continuity_gap_cleared is False
    assert pkg.gate_signal in {"blocked", "partial"}
