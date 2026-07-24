"""M281 S03: EvidenceAssertion staging JSONL persist."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.evidence_assertion_build import (
    build_evidence_assertion,
)
from research_graph.application.corpus.evidence_assertion_persist import (
    build_and_persist_from_grounded_gold,
    persist_evidence_assertions_jsonl,
)
from research_graph.application.corpus.gold_char_span_grounding import (
    attach_char_spans_to_gold_case,
)


def test_persist_jsonl_import_false(tmp_path: Path) -> None:
    spans = [{"artifact_hash": "h", "page": 1, "bbox": [0, 0, 1, 1]}]
    ea = build_evidence_assertion(
        paper_id="p",
        subject="A",
        predicate="USES_TECHNIQUE",
        object="B",
        claim_text="A uses technique B carefully",
        span_text="A uses technique B carefully in the section.",
        spans=spans,
    )
    path = tmp_path / "p.assertions.staging.jsonl"
    meta = persist_evidence_assertions_jsonl(path, [ea])
    assert meta["assertion_count"] == 1
    assert meta["import_eligible"] is False
    row = json.loads(path.read_text(encoding="utf-8").strip())
    assert row["import_eligible"] is False
    assert row["staging"] is True
    assert row["graph_writes_allowed"] is False


def test_build_and_persist_from_grounded_gold(tmp_path: Path) -> None:
    body = "Language Games appear here as Task for agents.\n"
    gold = {
        "paper_id": "1606.02447",
        "entities": [{"id": "e1", "label": "Language Games", "type": "Task"}],
        "relations": [],
    }
    grounded = attach_char_spans_to_gold_case(gold=gold, body_text=body).gold
    path = tmp_path / "1606.02447.assertions.staging.jsonl"
    meta = build_and_persist_from_grounded_gold(path, grounded, body_text=body)
    assert meta["assertion_count"] == 1
    assert path.is_file()
