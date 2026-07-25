"""Tests for GEPA vs header same-n comparison (M268)."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from research_graph.application.corpus.wave_b_gepa_constrained_spike import (
    COMPONENT_ENTITY,
    COMPONENT_RELATION,
)
from research_graph.application.corpus.wave_b_gepa_vs_header import (
    candidate_from_gepa_artifact,
    compare_header_vs_gepa_instruction,
    evaluate_val_gap_guard,
)

ROOT = Path(__file__).resolve().parents[1]


def test_candidate_from_gepa_artifact_nested() -> None:
    raw = {
        "best_candidate": {
            COMPONENT_ENTITY: "SELECT_MAX: 2\nTYPE_HINT: Foo -> Method\n",
            COMPONENT_RELATION: "RELATION_HINT: Method APPLIED_TO Task\n",
        }
    }
    c = candidate_from_gepa_artifact(raw)
    assert "TYPE_HINT" in c[COMPONENT_ENTITY]
    assert "RELATION_HINT" in c[COMPONENT_RELATION]


def test_val_gap_guard_blocks_large_gap() -> None:
    ok, blocker = evaluate_val_gap_guard(
        train_entity_f1=0.93, val_entity_f1=0.08, max_val_gap=0.35
    )
    assert ok is False
    assert blocker is not None and blocker.startswith("val_gap:")


def test_val_gap_guard_allows_small_gap() -> None:
    ok, blocker = evaluate_val_gap_guard(
        train_entity_f1=0.7, val_entity_f1=0.55, max_val_gap=0.35
    )
    assert ok is True
    assert blocker is None


def test_compare_header_vs_gepa_on_tiny_cases() -> None:
    body = (
        "## Language and Perception for Grounded Attribute Learning\n\n"
        "We study Language and Perception methods for Grounded Attribute Learning."
    )
    gold = {
        "case_id": "case:test:tiny",
        "paper_id": "tiny",
        "entities": [
            {
                "id": "e1",
                "label": "Language and Perception",
                "type": "Field",
                "evidence_refs": ["span:0"],
            },
            {
                "id": "e2",
                "label": "Grounded Attribute Learning",
                "type": "Task",
                "evidence_refs": ["span:1"],
            },
        ],
        "relations": [
            {
                "id": "r1",
                "type": "APPLIED_TO",
                "source": "e1",
                "target": "e2",
                "evidence_refs": ["span:0"],
            }
        ],
    }
    cases = [
        {
            "case_id": "case:test:tiny",
            "paper_id": "tiny",
            "gold": gold,
            "body_text": body,
        }
    ]
    # GEPA-style hints that match gold surfaces
    cand = {
        COMPONENT_ENTITY: (
            "SELECT_MAX: 2\n"
            "TYPE_HINT: Language and Perception -> Field\n"
            "TYPE_HINT: Grounded Attribute Learning -> Task\n"
        ),
        COMPONENT_RELATION: "RELATION_HINT: Field APPLIED_TO Task\n",
        "train_entity_f1": 1.0,
        "val_entity_f1": 0.9,
    }
    pkg = compare_header_vs_gepa_instruction(cases=cases, gepa_candidate=cand)
    assert pkg.import_eligible is False
    assert pkg.joined_count == 1
    assert pkg.gepa["entity_f1"] is not None
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert "delta_vs_header" in d


def test_operator_help() -> None:
    script = ROOT / "scripts" / "verify_wave_b_gepa_vs_header.py"
    proc = subprocess.run(
        [sys.executable, str(script), "--help"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0
    assert "GEPA" in proc.stdout or "gepa" in proc.stdout.lower()
