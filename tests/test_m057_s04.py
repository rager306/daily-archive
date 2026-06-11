from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import m057_build_graph_manifest as graph_manifest  # noqa: E402

ARTIFACT_ROOT = ROOT / "artifacts" / "m057-fd-marker"
COMBINED_EDGES = ARTIFACT_ROOT / "combined-edges.json"
PER_LAYER_SUMMARY = ARTIFACT_ROOT / "per-layer-summary.json"
REPORT = ARTIFACT_ROOT / "REPORT.md"
DEFERRED = ARTIFACT_ROOT / "decision-deferred.md"
ADR_011 = ROOT / "doc" / "adr" / "ADR-011-content-graph-via-fd.md"
FD_REPORT = ARTIFACT_ROOT / "fd-validation.json"
TABLE_SUMMARY = ARTIFACT_ROOT / "table-similarity" / "summary.json"
FIGURE_SUMMARY = ARTIFACT_ROOT / "figure-links" / "summary.json"

EXPECTED_SAFETY_DEFAULTS = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}
FORBIDDEN_LOOPBACK_ALIAS = "local" + "host"


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _assert_five_false_safety_defaults(payload: dict) -> None:
    safety = payload.get("safety_defaults")
    assert safety == EXPECTED_SAFETY_DEFAULTS
    assert len(safety) == 5
    assert all(value is False for value in safety.values())


def test_graph_manifest_combined(tmp_path: Path) -> None:
    combined_path = tmp_path / "combined-edges.json"
    summary_path = tmp_path / "per-layer-summary.json"

    manifest, summary = graph_manifest.run(combined_edges_path=combined_path, layer_summary_path=summary_path)

    assert combined_path.exists()
    assert summary_path.exists()
    assert manifest == _load_json(combined_path)
    assert summary == _load_json(summary_path)
    assert manifest["schema_version"] == "m057.combined-edges.v1"
    assert manifest["edge_count"] == 9403
    assert len(manifest["edges"]) == 9403
    assert {edge["evidence_layer"] for edge in manifest["edges"]} == {
        "citation",
        "table_similarity",
        "figure_similarity",
    }
    first = manifest["edges"][0]
    assert set(first) == {
        "source_paper_id",
        "source_artifact_type",
        "source_artifact_idx",
        "target_paper_id",
        "target_artifact_type",
        "target_artifact_idx",
        "similarity_score",
        "evidence_layer",
        "evidence_id",
    }
    assert manifest["base_url"] == "http://127.0.0.1:8000"
    assert FORBIDDEN_LOOPBACK_ALIAS not in json.dumps(manifest)


def test_graph_manifest_per_layer() -> None:
    summary = _load_json(PER_LAYER_SUMMARY)

    assert summary["schema_version"] == "m057.per-layer-summary.v1"
    assert summary["total_edges"] == 9403
    assert summary["layers"]["citation"]["count"] == 4454
    assert summary["layers"]["table_similarity"]["count"] == 4934
    assert summary["layers"]["figure_similarity"]["count"] == 15
    assert summary["layers"]["table_similarity"]["mean_similarity"] == 0.894583
    assert summary["layers"]["figure_similarity"]["mean_similarity"] == 0.819044
    assert summary["layers"]["citation"]["distinct_source_papers"] == 162
    assert summary["layers"]["table_similarity"]["distinct_target_papers"] == 81
    _assert_five_false_safety_defaults(summary)


def test_report_md_exists() -> None:
    text = REPORT.read_text(encoding="utf-8")

    assert len(text.encode("utf-8")) > 4096
    for heading in [
        "## 0. Резюме: итог M057",
        "## 1. Контекст и связь с ADR-010",
        "## 2. S01 fd validation",
        "## 3. S02 table similarity",
        "## 4. S03 figure similarity",
        "## 5. S04 chart extraction",
        "## 6. Combined graph",
        "## 7. Graph-readiness gate v1",
        "## 8. ADR-011 decision",
        "## 9. Marker недоступен в env",
        "## 10. Lessons + next milestones",
    ]:
        assert heading in text
    assert "9403" in text
    assert "4454 citation" in text
    assert "4934 table_similarity" in text
    assert "15 figure_similarity" in text
    assert "is disabled" in text
    assert "http://127.0.0.1:8000" in text
    assert FORBIDDEN_LOOPBACK_ALIAS not in text


def test_adr_011_binding() -> None:
    text = ADR_011.read_text(encoding="utf-8")

    assert text.startswith("# ADR-011: Content Graph via fd for M057")
    assert "**Status:** Accepted (binding)" in text
    assert "**Deciders:** agent" in text
    assert "**Supplements:** ADR-010" in text
    assert "binding supplement to ADR-010" in text
    assert "OpenDataLoader tables" in text
    assert "Graph-readiness gate v1 is unlocked" in text
    assert "Production import is disabled" in text
    assert "http://127.0.0.1:8000" in text
    assert FORBIDDEN_LOOPBACK_ALIAS not in text


def test_5_safety_defaults() -> None:
    for payload_path in [FD_REPORT, TABLE_SUMMARY, FIGURE_SUMMARY, COMBINED_EDGES, PER_LAYER_SUMMARY]:
        _assert_five_false_safety_defaults(_load_json(payload_path))

    for text_path in [REPORT, ADR_011, DEFERRED]:
        text = text_path.read_text(encoding="utf-8")
        for key in EXPECTED_SAFETY_DEFAULTS:
            assert f"`{key}`" in text or f'"{key}"' in text
        assert "true" not in text.lower()


def test_decision_deferred_documented() -> None:
    text = DEFERRED.read_text(encoding="utf-8")

    assert "Chart extraction через PlotExtract" in text
    assert "отложить chart extraction до M059" in text
    assert "Marker re-extraction" in text
    assert "transformers.onnx" in text
    assert "M059" in text
    assert "is not authorized" in text
    assert "http://127.0.0.1:8000" in text
    assert FORBIDDEN_LOOPBACK_ALIAS not in text


def test_m057_prior_slice_regression_artifacts() -> None:
    fd = _load_json(FD_REPORT)
    table = _load_json(TABLE_SUMMARY)
    figure = _load_json(FIGURE_SUMMARY)

    assert fd["summary"]["total"] == 7
    assert fd["summary"]["passed"] == 7
    assert fd["summary"]["failed"] == 0
    assert round(fd["summary"]["latency_p95_ms"]) == 253
    assert fd["summary"]["cache_hit_rate"] == 1.0
    assert "82x" in REPORT.read_text(encoding="utf-8")

    assert table["total_tables"] == 1468
    assert table["edges_total"] == 4934
    assert table["inter_doc_edges"] == 2591

    assert figure["total_figures"] == 937
    assert figure["edges_total"] == 15
    assert figure["inter_doc_edges"] == 15

    _assert_five_false_safety_defaults(fd)
    _assert_five_false_safety_defaults(table)
    _assert_five_false_safety_defaults(figure)
