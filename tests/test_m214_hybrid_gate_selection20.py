"""M214 S01: 20-paper hybrid gate selection extends M213 offline."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SEL10 = ROOT / "artifacts" / "m213-hybrid-gate" / "selection.json"
SEL20 = ROOT / "artifacts" / "m213-hybrid-gate" / "selection-20.json"


def test_m214_selection20_extends_m213_and_paths_resolve() -> None:
    assert SEL20.is_file(), SEL20
    ten = json.loads(SEL10.read_text(encoding="utf-8"))
    twenty = json.loads(SEL20.read_text(encoding="utf-8"))
    assert twenty["schema_version"] == "m213-hybrid-gate-selection.v1"
    assert twenty["count"] == 20
    assert twenty["rung"] == 20
    assert twenty["import_eligible"] is False
    assert twenty["graph_writes_allowed"] is False
    papers = twenty["papers"]
    assert len(papers) == 20
    ids = [row["paper_id"] for row in papers]
    assert len(set(ids)) == 20
    # first 10 match M213 selection order/ids
    ten_ids = [row["paper_id"] for row in ten["papers"]]
    assert ids[:10] == ten_ids
    for row in papers:
        path = ROOT / row["pdf_path"]
        assert path.is_file(), path
        assert row["byte_size"] > 0
        assert len(row["sha256"]) == 64
