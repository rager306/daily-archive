"""M277 E2.1: structure gate v2 IR signals; newline demoted to soft_legacy."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.structure_chunk_quality_gate import (
    evaluate_structure_chunk_quality_gate,
    score_structure_signals,
)


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def test_score_newline_only_is_soft_legacy_not_hard() -> None:
    # many newlines, no headings, no IR
    body = "word " * 80 + ("\n" * 12)
    s = score_structure_signals(body_text=body, canonical=None)
    assert s.hard_ok is False
    assert s.soft_legacy_ok is True
    assert s.mode == "soft_legacy_newline"
    assert s.newline_demoted is True


def test_score_markdown_heading_is_hard() -> None:
    body = (
        "# Introduction\n\n"
        + ("Scholarly body text with enough words for quality. " * 20)
        + "\n## Method\n\n"
        + ("More scholarly method description continues here. " * 15)
    )
    s = score_structure_signals(body_text=body, canonical=None)
    assert s.hard_ok is True
    assert s.mode in {"markdown_heading", "hard"}
    assert s.heading_count >= 1


def test_score_canonical_ir_hard() -> None:
    canon = {
        "schema_version": "canonical-document.v1",
        "paper_id": "x",
        "sections": [
            {
                "section_id": "s1",
                "title": "Introduction",
                "level": 1,
                "blocks": [
                    {
                        "block_id": "b1",
                        "kind": "heading",
                        "text": "Introduction",
                        "level": 1,
                        "spans": [
                            {
                                "artifact_role": "odl_layout",
                                "page": 1,
                                "bbox": [0, 0, 1, 1],
                                "artifact_hash": "h",
                            }
                        ],
                        "meta": {},
                    },
                    {
                        "block_id": "b2",
                        "kind": "paragraph",
                        "text": "Body",
                        "level": 0,
                        "spans": [{"artifact_role": "odl_layout", "page": 1, "bbox": [0, 1, 2, 3]}],
                        "meta": {},
                    },
                ],
                "children": [],
            }
        ],
        "blocks": [],
        "diagnostics": ["blocks_with_page_or_bbox:2"],
        "import_eligible": False,
    }
    s = score_structure_signals(body_text="short", canonical=canon)
    assert s.hard_ok is True
    assert s.mode == "canonical_ir"
    assert s.section_count >= 1
    assert s.grounded_blocks >= 1


def test_gate_prefers_canonical_json_sibling(tmp_path: Path) -> None:
    """Body with weak markdown but strong sibling canonical still structure-ok."""
    good_words = (
        "We present a method for grounded attribute learning using language. "
        "Experiments show improvements on standard benchmarks with clear methodology. "
        "Training uses standard optimizers and evaluation metrics reported in tables. "
    ) * 3
    # weak structure in md: no heading markers, few newlines — fail old hard path
    weak_md = good_words  # single paragraph blob
    root = tmp_path / "bodies"
    for i in range(12):
        body_path = root / f"p{i}" / "body" / f"p{i}.hybrid.body.md"
        _write(body_path, weak_md)
        canon = {
            "schema_version": "canonical-document.v1",
            "paper_id": f"p{i}",
            "sections": [
                {
                    "section_id": "s1",
                    "title": "Intro",
                    "level": 1,
                    "blocks": [
                        {
                            "block_id": "b1",
                            "kind": "heading",
                            "text": "Intro",
                            "spans": [{"page": 1, "bbox": [0, 0, 1, 1], "artifact_hash": "a"}],
                            "meta": {},
                            "level": 1,
                        },
                        {
                            "block_id": "b2",
                            "kind": "paragraph",
                            "text": "x",
                            "spans": [{"page": 1, "bbox": [0, 1, 2, 2], "artifact_hash": "a"}],
                            "meta": {},
                            "level": 0,
                        },
                        {
                            "block_id": "b3",
                            "kind": "table",
                            "text": "t",
                            "spans": [{"page": 2, "bbox": [0, 0, 3, 3], "artifact_hash": "a"}],
                            "meta": {},
                            "level": 0,
                        },
                    ],
                    "children": [],
                }
            ],
            "blocks": [],
            "import_eligible": False,
        }
        _write(
            root / f"p{i}" / "body" / f"p{i}.canonical.json",
            json.dumps(canon),
        )
    pkg = evaluate_structure_chunk_quality_gate(
        [root], sample_limit=12, min_sample=10, min_pass_rate=0.55
    )
    assert pkg.import_eligible is False
    assert pkg.gate_signal == "pass"
    assert pkg.continuity_gap_cleared is True
    assert getattr(pkg, "ir_hard_count", 0) >= 10
    assert "structure_gate_v2" in " ".join(pkg.diagnostics)


def test_gate_newline_only_does_not_clear_gap_as_hard_structure(tmp_path: Path) -> None:
    """Many newlines + scholarly words but no heading/IR: soft only, gap not cleared
    when min_pass_rate high for hard structure — or still may soft-pass depending policy.

    Policy M277: soft_legacy_newline counts as soft structure pass for continuity
    sample (body present) but is tracked as newline_demoted; continuity gap still
    can clear if soft+body_quality ok (same as soft_signal path) BUT diagnostics
    must report newline_demoted > 0 and ir_hard_count == 0 so operators know signal
    is weak.
    """
    blob = ("Scholarly method description with enough tokens for quality gate. " * 25) + (
        "\n" * 15
    )
    root = tmp_path / "nl"
    for i in range(12):
        _write(root / f"n{i}.hybrid.body.md", blob)
    pkg = evaluate_structure_chunk_quality_gate(
        [root], sample_limit=12, min_sample=10, min_pass_rate=0.55
    )
    assert pkg.import_eligible is False
    assert getattr(pkg, "newline_demoted_count", 0) >= 10
    # gap may clear via soft_legacy (body quality ok) — but must not claim ir_hard
    assert getattr(pkg, "ir_hard_count", 0) == 0
    assert any("newline_demoted" in d for d in pkg.diagnostics)
