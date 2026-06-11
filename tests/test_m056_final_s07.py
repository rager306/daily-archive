from __future__ import annotations

import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / "artifacts" / "m056-bfs-graph" / "REPORT.md"
CANDIDATE_EDGES = ROOT / "artifacts" / "m056-bfs-graph" / "candidate-edges.json"
ADR_010 = ROOT / "doc" / "adr" / "ADR-010-bfs-scale-167-pdf.md"
ADR_INDEX = ROOT / "doc" / "adr" / "ADR-INDEX.md"
EMIT_SCRIPT = ROOT / "scripts" / "emit_m056_candidate_edges.py"


def _load_emit_module():
    spec = importlib.util.spec_from_file_location("emit_m056_candidate_edges", EMIT_SCRIPT)
    assert spec is not None
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def _read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def test_report_contains_executive_summary() -> None:
    report = REPORT.read_text(encoding="utf-8")

    assert "Schema version: `m056-bfs-graph-report.v1`" in report
    assert "## 1. Executive summary" in report
    assert "166 extracted references" in report
    assert "148 referenced PDFs" in report
    assert "149 unique PDFs" in report
    assert "7-8 cumulative directed edges" in report
    assert "2-hop expansion" in report


def test_report_contains_6_wave_summaries() -> None:
    report = REPORT.read_text(encoding="utf-8")

    for wave in range(1, 7):
        assert f"### 13.{wave} Wave {wave}" in report
        assert f"| {wave} |" in report
    assert "Wave 5: +0" in report
    assert "Wave 6: +0" in report


def test_candidate_edges_json_schema() -> None:
    payload = _read_json(CANDIDATE_EDGES)

    assert payload["schema_version"] == "m056-bfs-candidate-edges.v1"
    assert payload["diagnostic_only"] is True
    assert payload["graph_writes_authorized"] is False
    assert payload["production_import_authorized"] is False
    assert set(payload) >= {"nodes", "edges", "summary", "safety_defaults", "safety_flags"}
    assert payload["summary"]["corpus_unique_pdf_count"] == 166
    assert payload["summary"]["node_count"] == len(payload["nodes"])
    assert payload["summary"]["edge_count"] == len(payload["edges"])
    assert payload["summary"]["edge_count"] > 0

    node = payload["nodes"][0]
    assert set(node) >= {"arxiv_id", "title", "source_milestone", "in_corpus"}

    edge = payload["edges"][0]
    assert set(edge) >= {"paper_a", "paper_b", "edge_type", "citation_count", "evidence"}
    assert edge["edge_type"] == "cites"
    assert edge["evidence"] == "grobid_biblstruct"
    assert isinstance(edge["citation_count"], int)
    assert edge["citation_count"] >= 1


def test_adr_010_exists_and_references_m056() -> None:
    adr = ADR_010.read_text(encoding="utf-8")
    index = ADR_INDEX.read_text(encoding="utf-8")

    assert "# ADR-010: BFS Scale Evidence from 167-PDF 1-hop Run" in adr
    assert "**Status:** Accepted (binding)" in adr
    assert "M056-lchpnp" in adr
    assert "2605.18747" in adr
    assert "149 unique PDFs" in adr
    assert "7-8 target-set internal edges" in adr
    assert "M058" in adr
    assert "ADR-010" in index
    assert "ADR-010-bfs-scale-167-pdf.md" in index


def test_5_safety_defaults_all_false() -> None:
    payload = _read_json(CANDIDATE_EDGES)
    report = REPORT.read_text(encoding="utf-8")
    adr = ADR_010.read_text(encoding="utf-8")

    assert len(payload["safety_defaults"]) == 5
    assert all(value is False for value in payload["safety_defaults"].values())
    assert len(payload["safety_flags"]) == 5
    assert all(value is False for value in payload["safety_flags"].values())

    for key in payload["safety_defaults"]:
        assert f"`{key}` | `false`" in report
        assert f"`{key}` | `false`" in adr
    for key in payload["safety_flags"]:
        assert f"`{key}` | `false`" in report
        assert f"`{key}` | `false`" in adr

    assert "This evidence is not authorized for graph import or fact promotion." in report
    assert "This evidence is not authorized for graph import or fact promotion." in adr


def test_emit_candidate_edges_is_idempotent_with_tmp_path(tmp_path: Path) -> None:
    module = _load_emit_module()
    payload_a = module.build_candidate_edges(ROOT / "artifacts" / "m056-bfs-graph")
    payload_b = module.build_candidate_edges(ROOT / "artifacts" / "m056-bfs-graph")
    assert payload_a == payload_b

    output = tmp_path / "candidate-edges.json"
    module.write_candidate_edges(payload_a, output)
    assert _read_json(output) == payload_a


def test_m050_m055deep_and_wave_regression_assets_present() -> None:
    expected_paths = [
        ROOT / "tests" / "test_m050_article_artifact_reducer.py",
        ROOT / "tests" / "test_m050_article_artifact_worker.py",
        ROOT / "tests" / "test_m050_e2e_pipeline.py",
        ROOT / "tests" / "test_m055_benchmark_s01.py",
        ROOT / "tests" / "test_m055_benchmark_s02.py",
        ROOT / "tests" / "test_m055_benchmark_s03.py",
        ROOT / "tests" / "test_m055_benchmark_s04.py",
        ROOT / "tests" / "test_m055_benchmark_s05.py",
        ROOT / "tests" / "test_m055deep_benchmark_20.py",
        ROOT / "tests" / "test_m055deep_corpus_20.py",
        ROOT / "tests" / "test_m055deep_grobid_fulltext.py",
        ROOT / "tests" / "test_m055deep_hybrid_routing_20.py",
        ROOT / "tests" / "test_m055deep_opendataloader_correctness.py",
        ROOT / "tests" / "test_m055deep_report_s06.py",
        ROOT / "tests" / "test_m056_wave_1.py",
        ROOT / "tests" / "test_m056_wave_2.py",
        ROOT / "tests" / "test_m056_wave_3.py",
        ROOT / "tests" / "test_m056_wave_4.py",
        ROOT / "tests" / "test_m056_wave_5.py",
        ROOT / "tests" / "test_m056_wave_6.py",
    ]

    missing = [str(path.relative_to(ROOT)) for path in expected_paths if not path.exists()]
    assert missing == []
