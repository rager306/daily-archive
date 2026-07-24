"""TDD: parser-run artifact helpers (M274)."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.parser_run_artifacts import (
    build_parser_run_manifest,
    count_layout_elements,
    sha256_bytes,
    write_bytes_artifact,
    write_parser_run_manifest,
    write_text_artifact,
)


def test_count_layout_elements_with_bboxes() -> None:
    layout = {
        "elements": [
            {"type": "paragraph", "bbox": [0, 0, 1, 1], "text": "a"},
            {"type": "figure", "bounding_box": [1, 1, 2, 2]},
            {"type": "span", "text": "no box"},
        ]
    }
    elements, bboxes = count_layout_elements(layout)
    assert elements >= 4  # root + 3 children
    assert bboxes == 2


def test_count_layout_empty() -> None:
    assert count_layout_elements(None) == (0, 0)
    assert count_layout_elements([]) == (0, 0)


def test_write_artifacts_and_manifest(tmp_path: Path) -> None:
    tei = b"<TEI>hello</TEI>"
    tei_path = tmp_path / "paper.tei.xml"
    tei_hash = write_bytes_artifact(tei_path, tei)
    assert tei_hash == sha256_bytes(tei)
    assert tei_path.read_bytes() == tei

    md_path = tmp_path / "paper.md"
    md_hash = write_text_artifact(md_path, "# Hi\n")
    assert md_path.read_text(encoding="utf-8") == "# Hi\n"

    man = build_parser_run_manifest(
        paper_id="x",
        parser="grobid",
        artifact_paths={"tei": str(tei_path), "markdown": str(md_path)},
        content_hashes={"tei": tei_hash, "markdown": md_hash},
        config={"endpoint": "processFulltextDocument"},
        parser_version="0.8.0",
    )
    assert man.import_eligible is False
    out = write_parser_run_manifest(tmp_path / "parser-run.json", man)
    data = json.loads(out.read_text(encoding="utf-8"))
    assert data["parser"] == "grobid"
    assert data["content_hashes"]["tei"] == tei_hash
    assert data["import_eligible"] is False
