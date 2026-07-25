"""M274: ODL layout metrics + GROBID TEI persist (unit, no live sidecars)."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from research_graph.application.corpus.parser_run_artifacts import count_layout_elements
from research_graph.infrastructure.corpus.parsing.live_sidecar_adapters import (
    _odl_convert_json_and_markdown,
)
from research_graph.workflows.composition.parser_body_resolve import (
    _persist_grobid_structured_artifacts,
    _persist_odl_layout_artifacts,
)


def test_odl_convert_prefers_json_and_markdown(tmp_path: Path, monkeypatch: Any) -> None:
    """Fake ODL module writes json+md when format includes json."""

    class FakeODL:
        def convert(self, pdf: str, output_dir: str, format=None, quiet=True):  # noqa: A002
            out = Path(output_dir)
            stem = Path(pdf).stem
            if format and "json" in format:
                (out / f"{stem}.json").write_text(
                    json.dumps(
                        {
                            "elements": [
                                {"type": "p", "bbox": [0, 0, 1, 1], "text": "hi"}
                            ]
                        }
                    ),
                    encoding="utf-8",
                )
            (out / f"{stem}.md").write_text("# Body\n\nHello world.\n", encoding="utf-8")

    result = _odl_convert_json_and_markdown(FakeODL(), tmp_path / "paper.pdf")
    assert "Hello" in result["markdown"]
    assert isinstance(result["layout_json"], dict)
    elements, bboxes = count_layout_elements(result["layout_json"])
    assert bboxes >= 1
    assert elements >= 1


def test_persist_grobid_tei_from_bytes(tmp_path: Path) -> None:
    tei = b"<TEI xmlns='http://www.tei-c.org/ns/1.0'><text>x</text></TEI>"
    metrics = {
        "header": {"title": "T", "authors": []},
        "citations": [{"id": "c1", "title": "C"}],
        "tei_bytes": tei,
        "tei_sha256": None,
    }
    diag = _persist_grobid_structured_artifacts(
        body_dir=tmp_path, paper_id="2507.19457", grobid_metrics=metrics
    )
    tei_path = tmp_path / "2507.19457.grobid.tei.xml"
    assert tei_path.is_file()
    assert tei_path.read_bytes() == tei
    assert any(d.startswith("grobid_tei_artifact:") for d in diag)
    assert any(d.startswith("grobid_tei_sha256:") for d in diag)
    assert (tmp_path / "2507.19457.grobid.parser-run.json").is_file()
    assert (tmp_path / "2507.19457.hybrid.header.json").is_file()


def test_persist_odl_layout_json(tmp_path: Path) -> None:
    layout = {"elements": [{"type": "p", "bbox": [0, 0, 10, 10]}]}
    metrics = {
        "layout_json": layout,
        "format": "json+markdown",
        "bbox_source": "layout_json",
        "layout_element_count": 2,
        "bounding_box_count": 1,
    }
    diag = _persist_odl_layout_artifacts(
        body_dir=tmp_path, paper_id="x", odl_metrics=metrics
    )
    path = tmp_path / "x.odl.layout.json"
    assert path.is_file()
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["elements"][0]["bbox"] == [0, 0, 10, 10]
    assert any("odl_layout_artifact:" in d for d in diag)
    assert (tmp_path / "x.odl.parser-run.json").is_file()


def test_persist_odl_absent_layout_still_reports_counts() -> None:
    diag = _persist_odl_layout_artifacts(
        body_dir=Path("/tmp"),
        paper_id="x",
        odl_metrics={
            "bbox_source": "newline_proxy",
            "bounding_box_count": 12,
            "layout_element_count": 0,
        },
    )
    assert "odl_bbox_source:newline_proxy" in diag
    assert "odl_bounding_box_count:12" in diag
