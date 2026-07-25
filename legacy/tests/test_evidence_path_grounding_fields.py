"""M276 E1.5: EvidencePath optional page/bbox grounding fields."""

from __future__ import annotations

from research_graph.application.corpus.evidence_resolvability import (
    evaluate_source_span_resolvability,
)
from research_graph.domain.semantic_chunks import EvidencePath


def test_legacy_construction_still_works() -> None:
    path = EvidencePath(
        paper_id="p",
        page_index_node_id="n1",
        semantic_chunk_id="c1",
        node_path=["root", "sec"],
    )
    assert path.artifact_hash is None
    assert path.page is None
    assert path.bbox is None


def test_grounding_fields_and_resolvability() -> None:
    path = EvidencePath(
        paper_id="p",
        page_index_node_id="n1",
        semantic_chunk_id="c1",
        node_path=["root"],
        artifact_hash="deadbeef",
        page=2,
        bbox=(0.0, 1.0, 2.0, 3.0),
        element_id="e9",
        char_start=10,
        char_end=40,
    )
    g = path.grounding_dict()
    assert g["page"] == 2
    assert g["bbox"] == [0.0, 1.0, 2.0, 3.0]
    v = evaluate_source_span_resolvability(g)
    assert v.resolvable is True
    assert v.import_eligible is False
