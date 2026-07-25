from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

# pyrefly: ignore [missing-import]
import m058_build_graph_manifest as graph_manifest  # noqa: E402  # ty:ignore[unresolved-import]

ARTIFACT_ROOT = ROOT / "artifacts" / "m058-pilot"
COMBINED_EDGES = ARTIFACT_ROOT / "combined-edges.json"
LAYER_SUMMARY = ARTIFACT_ROOT / "per-layer-summary.json"
REPORT = ARTIFACT_ROOT / "REPORT.md"
DEFERRED_DECISION = ARTIFACT_ROOT / "decision-deferred.md"
ADR_012 = ROOT / "doc" / "adr" / "ADR-012-figure-caption-v2.md"
S01_SUMMARY = ROOT / "artifacts" / "m058-plotextractor" / "summary.json"
S02_SUMMARY = ROOT / "artifacts" / "m058-marker" / "pilot-5" / "summary.json"
S02_DECISION = ROOT / "artifacts" / "m058-marker" / "pilot-5" / "decision.md"
FORBIDDEN_LOOPBACK_ALIAS = "local" + "host"
EXPECTED_SAFETY_DEFAULTS = {
    "external_network_authorized": False,
    "fact_promotion_authorized": False,
    "graph_writes_authorized": False,
    "llm_calls_authorized": False,
    "production_import_authorized": False,
}
EXPECTED_LAYER_COUNTS = {
    "citation": 4454,
    "table_similarity": 4934,
    "figure_similarity_v1": 15,
    "figure_similarity_v2": 15,
}


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _assert_five_false_safety_defaults(payload: dict) -> None:
    safety = payload.get("safety_defaults")
    assert safety == EXPECTED_SAFETY_DEFAULTS
    # pyrefly: ignore [bad-argument-type]
    assert len(safety) == 5  # ty:ignore[invalid-argument-type]
    # pyrefly: ignore [missing-attribute]
    assert all(value is False for value in safety.values())  # ty:ignore[unresolved-attribute]


def test_graph_manifest_combined(tmp_path: Path) -> None:
    combined_path = tmp_path / "combined-edges.json"
    summary_path = tmp_path / "per-layer-summary.json"

    first_combined, first_summary = graph_manifest.build_graph_manifest(
        combined_edges_path=combined_path,
        layer_summary_path=summary_path,
    )
    first_text = combined_path.read_text(encoding="utf-8")

    second_combined, second_summary = graph_manifest.build_graph_manifest(
        combined_edges_path=combined_path,
        layer_summary_path=summary_path,
    )

    assert first_combined == second_combined
    assert first_summary == second_summary
    assert combined_path.read_text(encoding="utf-8") == first_text
    assert first_combined["edge_count"] == 9418
    assert first_summary["total_edges"] == 9418
    assert first_combined["loopback_bind_host"] == "127.0.0.1"
    assert FORBIDDEN_LOOPBACK_ALIAS not in first_text


def test_graph_manifest_4_layers() -> None:
    combined = _load_json(COMBINED_EDGES)
    summary = _load_json(LAYER_SUMMARY)
    layers = {layer["evidence_layer"]: layer for layer in summary["layers"]}

    assert combined["edge_count"] == 9418
    assert summary["layer_count"] == 4
    assert combined["layer_order"] == list(EXPECTED_LAYER_COUNTS)
    assert {layer: payload["count"] for layer, payload in layers.items()} == EXPECTED_LAYER_COUNTS
    assert layers["citation"]["mean_similarity"] is None
    assert layers["table_similarity"]["mean_similarity"] == 0.894583
    assert layers["figure_similarity_v1"]["mean_similarity"] == 0.819044
    assert layers["figure_similarity_v2"]["mean_similarity"] == 0.779106
    assert {edge["evidence_layer"] for edge in combined["edges"]} == set(EXPECTED_LAYER_COUNTS)


def test_report_md_exists() -> None:
    text = REPORT.read_text(encoding="utf-8")

    assert len(text.encode("utf-8")) >= 4096
    for section in range(0, 11):
        assert f"## {section}." in text
    assert "S01" in text and "plotextractor" in text
    assert "S02" in text and "NO-GO" in text
    assert "S03" in text and "S04" in text and "cancelled" in text
    assert "9418" in text and "4 слоях" in text
    assert "127.0.0.1" in text
    assert FORBIDDEN_LOOPBACK_ALIAS not in text


def test_adr_012_binding() -> None:
    text = ADR_012.read_text(encoding="utf-8")

    assert text.startswith("# ADR-012: Figure Caption v2")
    assert "**Status:** Accepted (binding)" in text
    assert "**Binding Level:** binding supplement to ADR-011" in text
    assert "**Supplements:** ADR-011" in text
    assert "figure_similarity_v2" in text
    assert "Marker stage 2 and stage 3 are not authorized" in text
    assert "Production import is disabled" in text
    assert FORBIDDEN_LOOPBACK_ALIAS not in text


def test_5_safety_defaults() -> None:
    combined = _load_json(COMBINED_EDGES)
    summary = _load_json(LAYER_SUMMARY)
    s01_summary = _load_json(S01_SUMMARY)
    s02_summary = _load_json(S02_SUMMARY)

    for payload in (combined, summary, s01_summary, s02_summary):
        _assert_five_false_safety_defaults(payload)
    assert graph_manifest.SAFETY_DEFAULTS == EXPECTED_SAFETY_DEFAULTS


def test_decision_deferred_documented() -> None:
    text = DEFERRED_DECISION.read_text(encoding="utf-8")

    assert "M060" in text
    assert "2-hop BFS" in text
    assert "fd" in text
    assert "ADR-002 GraphDB selection" in text
    assert "Marker full-document scale-up is deferred" in text
    assert "Chart extraction is deferred" in text
    assert "Production import is disabled" in text
    assert FORBIDDEN_LOOPBACK_ALIAS not in text


def test_m050_m058_s01_s02_regression() -> None:
    s01_summary = _load_json(S01_SUMMARY)
    s02_summary = _load_json(S02_SUMMARY)
    s02_decision = S02_DECISION.read_text(encoding="utf-8")
    combined = _load_json(COMBINED_EDGES)

    assert s01_summary["schema_version"] == "m058.plotextractor.summary.v2"
    assert s01_summary["sample_size"] == 5
    assert s01_summary["total_figures"] == 104
    assert s01_summary["edges_total"] == 15
    assert s02_summary["schema_version"] == "m058.marker-pilot.summary.v1"
    assert s02_summary["sample_size"] == 5
    assert s02_summary["successful"] == 5
    assert s02_summary["page_range"] == "0"
    assert "Decision: go to S03" in s02_decision
    assert "**no**" in s02_decision
    assert combined["edge_count"] == 4454 + 4934 + 15 + 15
    assert all(edge["evidence_id"].startswith("m058:") for edge in combined["edges"])
