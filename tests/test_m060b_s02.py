from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import m060b_graph_stats as graph_stats  # noqa: E402
import m060b_graph_visualize as graph_visualize  # noqa: E402
import m060b_two_hop_preview as two_hop_preview  # noqa: E402

MANIFEST = ROOT / "artifacts" / "m058-pilot" / "combined-edges.json"
STATS_JSON = ROOT / "artifacts" / "m060b-graph" / "stats.json"
EXPECTED_LAYER_COUNTS = {
    "citation": 4454,
    "table_similarity": 4934,
    "figure_similarity_v1": 15,
    "figure_similarity_v2": 15,
}
EXPECTED_SAFETY_DEFAULTS = {
    "external_network_authorized": False,
    "fact_promotion_authorized": False,
    "graph_writes_authorized": False,
    "llm_calls_authorized": False,
    "production_import_authorized": False,
}
FORBIDDEN_LOOPBACK_ALIAS = "local" + "host"


def test_visualize_runs(tmp_path: Path) -> None:
    output = tmp_path / "graph-viz.png"
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPTS / "m060b_graph_visualize.py"),
            "--manifest",
            str(MANIFEST),
            "--output",
            str(output),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert "Rendered M060b graph preview" in result.stdout


def test_png_file_exists(tmp_path: Path) -> None:
    output = tmp_path / "graph-viz.png"
    result = graph_visualize.render_graph(MANIFEST, output)

    assert output.exists()
    assert output.stat().st_size > 1000
    assert output.read_bytes().startswith(b"\x89PNG\r\n\x1a\n")
    assert result["rendered_nodes"] <= graph_visualize.MAX_VISUALIZATION_NODES
    assert result["loopback_bind_host"] == "127.0.0.1"


def test_two_hop_preview_runs(tmp_path: Path) -> None:
    output = tmp_path / "two-hop-preview.json"
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPTS / "m060b_two_hop_preview.py"),
            "--manifest",
            str(MANIFEST),
            "--output",
            str(output),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    persisted = json.loads(output.read_text(encoding="utf-8"))
    assert persisted["schema_version"] == "m060b.two_hop_preview.v1"
    assert persisted["mode"] == "algorithm_only_preview_not_acquisition"
    assert persisted["m061_estimated_edges"] == persisted["two_hop_unique_edges"]


def test_two_hop_anchor_2605_18747(tmp_path: Path) -> None:
    output = tmp_path / "two-hop-preview.json"
    preview = two_hop_preview.run_preview(MANIFEST, output, anchor="2605.18747")

    assert preview["anchor"] == "2605.18747"
    assert preview["one_hop_unique_nodes"] == 171
    assert preview["one_hop_unique_edges"] == 171
    assert preview["two_hop_new_unique_nodes"] > preview["one_hop_unique_nodes"]
    assert preview["two_hop_unique_edges"] > preview["one_hop_unique_edges"]
    assert preview["per_layer_two_hop_edge_counts"]["citation"] >= 171
    assert output.exists()


def test_5_safety_defaults() -> None:
    assert graph_visualize.SAFETY_DEFAULTS == EXPECTED_SAFETY_DEFAULTS
    assert two_hop_preview.SAFETY_DEFAULTS == EXPECTED_SAFETY_DEFAULTS
    assert graph_visualize.LOOPBACK_BIND_HOST == "127.0.0.1"
    assert two_hop_preview.LOOPBACK_BIND_HOST == "127.0.0.1"
    assert FORBIDDEN_LOOPBACK_ALIAS not in Path(graph_visualize.__file__).read_text(
        encoding="utf-8"
    )
    assert FORBIDDEN_LOOPBACK_ALIAS not in Path(two_hop_preview.__file__).read_text(
        encoding="utf-8"
    )


def test_m050_m063_s01_regression_stats_contract() -> None:
    stats = json.loads(STATS_JSON.read_text(encoding="utf-8"))

    assert stats["manifest_edge_count"] == 9418
    assert stats["total_nodes"] == 3421
    assert stats["total_edges"] == 9418
    assert stats["networkx_graph_edges"] == 9418
    assert stats["safety_defaults"] == EXPECTED_SAFETY_DEFAULTS
    assert {
        layer: row["edge_count"] for layer, row in stats["per_layer"].items()
    } == EXPECTED_LAYER_COUNTS
    assert graph_stats.SAFETY_DEFAULTS == EXPECTED_SAFETY_DEFAULTS
