"""M213 S01: hybrid gate selection manifest paths resolve offline."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SELECTION = ROOT / "artifacts" / "m213-hybrid-gate" / "selection.json"


def test_m213_selection_manifest_shape_and_paths() -> None:
    assert SELECTION.is_file(), SELECTION
    payload = json.loads(SELECTION.read_text(encoding="utf-8"))
    assert payload["schema_version"] == "m213-hybrid-gate-selection.v1"
    assert payload["count"] == 10
    assert payload["import_eligible"] is False
    assert payload["graph_writes_allowed"] is False
    papers = payload["papers"]
    assert len(papers) == 10
    ids = {row["paper_id"] for row in papers}
    assert len(ids) == 10
    for row in papers:
        path = ROOT / row["pdf_path"]
        assert path.is_file(), path
        assert row["byte_size"] > 0
        assert len(row["sha256"]) == 64
        assert row["category"]
