"""TDD: CanonicalDocument IR + ODL builder (M275)."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.canonical_document_build import (
    build_canonical_document_from_odl,
)
from research_graph.domain.canonical_document import CanonicalDocument


def test_build_from_layout_json_with_bbox() -> None:
    layout = {
        "elements": [
            {
                "type": "heading",
                "text": "Introduction",
                "page": 1,
                "bbox": [10.0, 20.0, 100.0, 40.0],
                "id": "h1",
            },
            {
                "type": "paragraph",
                "text": "We study grounded learning.",
                "page": 1,
                "bbox": [10.0, 50.0, 200.0, 80.0],
            },
        ]
    }
    doc = build_canonical_document_from_odl(
        paper_id="x",
        layout_json=layout,
        layout_json_sha256="abc",
        title="Paper X",
        source_hashes={"odl_layout": "abc"},
    )
    assert isinstance(doc, CanonicalDocument)
    assert doc.import_eligible is False
    assert doc.paper_id == "x"
    assert len(doc.blocks) >= 2
    grounded = [
        b
        for b in doc.blocks
        if any(s.bbox is not None and s.page is not None for s in b.spans)
    ]
    assert len(grounded) >= 2
    assert grounded[0].spans[0].artifact_hash == "abc"
    assert "blocks_with_page_or_bbox:2" in doc.diagnostics or any(
        d.startswith("blocks_with_page_or_bbox:") for d in doc.diagnostics
    )
    payload = doc.to_dict()
    assert payload["import_eligible"] is False
    assert payload["sections"]


def test_build_from_markdown_fallback() -> None:
    md = "# Title\n\nFirst paragraph.\n\nSecond paragraph.\n"
    doc = build_canonical_document_from_odl(
        paper_id="m",
        markdown=md,
        source_hashes={"markdown": "mdhash"},
    )
    assert len(doc.blocks) >= 2
    assert any(b.kind == "heading" for b in doc.blocks)
    assert all(b.spans[0].artifact_role == "markdown" for b in doc.blocks if b.spans)


def test_canonical_rejects_import() -> None:
    with pytest.raises(ValueError, match="import"):
        CanonicalDocument(
            schema_version="x",
            paper_id="p",
            title=None,
            sections=(),
            blocks=(),
            parser_runs=(),
            source_hashes={},
            diagnostics=(),
            import_eligible=True,
        )
