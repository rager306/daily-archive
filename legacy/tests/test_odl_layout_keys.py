"""M283: ODL real layout keys (spaced) + safe heading level parse."""

from __future__ import annotations

from research_graph.application.corpus.canonical_document_build import (
    build_canonical_document_from_odl,
)
from research_graph.application.corpus.layout_span_upgrade import (
    iter_layout_elements,
    upgrade_spans_with_layout_json,
)
from research_graph.application.corpus.parser_run_artifacts import count_layout_elements


def _odl_like_layout() -> dict:
    """Shape mirrors live OpenDataLoader fixed JSON (spaced keys)."""
    return {
        "file name": "x.pdf",
        "number of pages": 2,
        "kids": [
            {
                "type": "heading",
                "level": "Doctitle",
                "heading level": 1,
                "page number": 1,
                "bounding box": [10.0, 20.0, 100.0, 40.0],
                "content": "Seq2Seq Models for Knowledge Graph Link Prediction",
                "id": 1,
            },
            {
                "type": "paragraph",
                "page number": 1,
                "bounding box": [10.0, 50.0, 200.0, 80.0],
                "content": "We study Seq2Seq Models on link prediction.",
                "id": 2,
            },
        ],
    }


def test_count_layout_elements_spaced_bbox_keys() -> None:
    elements, bboxes = count_layout_elements(_odl_like_layout())
    assert elements >= 3  # root + 2 kids (walk counts mappings)
    assert bboxes >= 2


def test_canonical_build_handles_doctitle_level_and_page_bbox() -> None:
    doc = build_canonical_document_from_odl(
        paper_id="x",
        layout_json=_odl_like_layout(),
        layout_json_sha256="abc",
        title="T",
    )
    assert doc.import_eligible is False
    assert len(doc.blocks) >= 2
    grounded = [
        b
        for b in doc.blocks
        if any(s.page is not None and s.bbox is not None for s in b.spans)
    ]
    assert len(grounded) >= 2
    assert grounded[0].spans[0].page == 1
    assert grounded[0].level in {0, 1}


def test_layout_upgrade_reads_spaced_keys() -> None:
    layout = _odl_like_layout()
    els = iter_layout_elements(layout)
    assert any(e.get("page") == 1 and e.get("bbox") for e in els)
    spans = [
        {
            "surface": "Seq2Seq Models",
            "artifact_hash": "h",
            "char_start": 0,
            "char_end": 13,
            "justified_char_only": True,
            "page": None,
            "bbox": None,
        }
    ]
    new_spans, stats = upgrade_spans_with_layout_json(spans, layout)
    assert stats["upgraded"] >= 1
    assert new_spans[0].get("page") == 1
    assert new_spans[0].get("bbox") is not None
    assert new_spans[0].get("justified_char_only") is False
