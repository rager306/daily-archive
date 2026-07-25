"""M254 S05: Wave B live hybrid extraction package scaffold."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

from research_graph.application.corpus.wave_b_hybrid_extraction_inventory import (
    HybridExtractionCandidate,
)
from research_graph.application.corpus.wave_b_live_hybrid_extraction import (
    WaveBLiveHybridExtractionPackage,
    build_wave_b_live_hybrid_extraction,
)

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_b_live_hybrid_extraction.py"


def _cand(pid: str, words: int = 10, empty: bool = False) -> HybridExtractionCandidate:
    return HybridExtractionCandidate(
        paper_id=pid,
        path=f"/tmp/{pid}.hybrid.body.md",
        body_root="/tmp",
        char_count=words * 5,
        word_count=words,
        empty=empty,
    )


def test_build_scaffold_pending_extraction() -> None:
    pkg = build_wave_b_live_hybrid_extraction(
        candidates=(_cand("a", 100), _cand("b", 50), _cand("c", 0, empty=True)),
        wave_b_gate_open=True,
        human_go=True,
        sample_limit=2,
    )
    assert pkg.import_eligible is False
    assert pkg.graph_writes_allowed is False
    assert pkg.dspy_optimizer_enabled is False
    assert pkg.extraction_status == "pending_extraction"
    assert pkg.candidate_count == 3
    assert pkg.empty_count == 1
    assert pkg.sampled_count == 2
    assert pkg.wave_b_gate_open is True
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["dspy_optimizer_enabled"] is False
    assert d["extraction_status"] == "pending_extraction"
    assert len(d["samples"]) == 2


def test_blocked_gate_marks_blocked() -> None:
    pkg = build_wave_b_live_hybrid_extraction(
        candidates=(_cand("a"),),
        wave_b_gate_open=False,
        human_go=False,
    )
    assert pkg.extraction_status == "blocked_gate"
    assert pkg.wave_b_gate_open is False


def test_rejects_import_true() -> None:
    with pytest.raises(ValueError, match="import"):
        WaveBLiveHybridExtractionPackage(
            schema_version="wave-b-live-hybrid-extraction.v1",
            extraction_status="pending_extraction",
            candidate_count=0,
            empty_count=0,
            sampled_count=0,
            total_words=0,
            samples=(),
            diagnostics=(),
            wave_b_gate_open=True,
            human_go=True,
            dspy_optimizer_enabled=False,
            import_eligible=True,
            graph_writes_allowed=False,
        )


def test_rejects_dspy_true() -> None:
    with pytest.raises(ValueError, match="DSPy|dspy"):
        WaveBLiveHybridExtractionPackage(
            schema_version="wave-b-live-hybrid-extraction.v1",
            extraction_status="pending_extraction",
            candidate_count=0,
            empty_count=0,
            sampled_count=0,
            total_words=0,
            samples=(),
            diagnostics=(),
            wave_b_gate_open=True,
            human_go=True,
            dspy_optimizer_enabled=True,
            import_eligible=False,
            graph_writes_allowed=False,
        )


def test_operator_script_no_stamp_blocked() -> None:
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--no-stamp", "--json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["import_eligible"] is False
    assert report["dspy_optimizer_enabled"] is False
    assert report["extraction_status"] in {"blocked_gate", "pending_extraction"}
    if report["extraction_status"] == "blocked_gate":
        assert report["wave_b_gate_open"] is False
