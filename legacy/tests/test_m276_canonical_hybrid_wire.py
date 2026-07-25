"""M276: CanonicalDocument wire into hybrid resolve (unit, fake ports)."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from research_graph.application.corpus.canonical_document_build import (
    persist_canonical_from_odl_metrics,
)
from research_graph.workflows.composition.parser_body_resolve import (
    ArticleBodyRequest,
    resolve_article_body,
)


class _FakeODL:
    def __init__(self, metrics: dict[str, Any]) -> None:
        self.metrics = metrics

    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict[str, Any]:
        return dict(self.metrics)


class _FakeGrobid:
    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict[str, Any]:
        return {
            "status": "ok",
            "tei_sha256": "teihash",
            "tei_bytes": b"<TEI/>",
            "header": {"title": "T"},
            "citations": [],
            "structured_parse_ok": True,
        }


def test_persist_canonical_from_layout_metrics(tmp_path: Path) -> None:
    layout = {
        "elements": [
            {
                "type": "paragraph",
                "text": "Grounded sentence.",
                "page": 2,
                "bbox": [1.0, 2.0, 3.0, 4.0],
            }
        ]
    }
    doc, diag = persist_canonical_from_odl_metrics(
        body_dir=tmp_path,
        paper_id="p1",
        odl_metrics={
            "layout_json": layout,
            "markdown": "# Title\n\nBody.\n",
            "format": "json+markdown",
            "bbox_source": "layout_json",
        },
        grobid_metrics={"tei_sha256": "abc"},
        title="T",
    )
    assert doc is not None
    path = tmp_path / "p1.canonical.json"
    assert path.is_file()
    payload = json.loads(path.read_text(encoding="utf-8"))
    assert payload["import_eligible"] is False
    assert payload["schema_version"] == "canonical-document.v1"
    assert any(d.startswith("canonical_document_path:") for d in diag)
    assert any(d.startswith("canonical_grounded_blocks:") for d in diag)
    assert int(next(d.split(":")[1] for d in diag if d.startswith("canonical_grounded_blocks:"))) >= 1


def test_resolve_hybrid_writes_canonical_json(tmp_path: Path) -> None:
    pdf = tmp_path / "paper.pdf"
    pdf.write_bytes(b"%PDF-1.4 fake")
    work = tmp_path / "work"
    layout = {
        "elements": [
            {
                "type": "heading",
                "text": "Intro",
                "page": 1,
                "bbox": [0, 0, 10, 10],
            },
            {
                "type": "paragraph",
                "text": "Hello hybrid body with enough text for claim.",
                "page": 1,
                "bbox": [0, 12, 50, 40],
            },
        ]
    }
    odl = _FakeODL(
        {
            "status": "ok",
            "markdown": "# Intro\n\nHello hybrid body with enough text for claim.\n",
            "layout_json": layout,
            "format": "json+markdown",
            "bbox_source": "layout_json",
            "layout_element_count": 2,
            "bounding_box_count": 2,
        }
    )
    result = resolve_article_body(
        ArticleBodyRequest(
            source=str(pdf),
            work_dir=work,
            preference="hybrid",
            paper_id="2507.00001",
            allow_network=False,
        ),
        grobid=_FakeGrobid(),
        opendataloader=odl,
        hybrid_pdf_path=pdf,
    )
    assert result.route in {"hybrid", "hybrid_deferred"}
    canon = work / "body" / "2507.00001.canonical.json"
    assert canon.is_file(), list((work / "body").iterdir()) if (work / "body").exists() else "no body dir"
    payload = json.loads(canon.read_text(encoding="utf-8"))
    assert payload["import_eligible"] is False
    assert payload["paper_id"] == "2507.00001"
    assert any("canonical_document_path:" in d for d in result.diagnostics)
    # TEI also persisted when tei_bytes present
    tei = work / "body" / "2507.00001.grobid.tei.xml"
    assert tei.is_file()
