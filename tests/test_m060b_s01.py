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
import m060b_graph_validate as graph_validate  # noqa: E402

MANIFEST = ROOT / "artifacts" / "m058-pilot" / "combined-edges.json"
M054_MANIFEST = ROOT / "artifacts" / "m054-pdf-acquisition" / "manifest.json"
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


def test_graph_stats_runs_on_m058_manifest(tmp_path: Path) -> None:
    output_dir = tmp_path / "stats-output"
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPTS / "m060b_graph_stats.py"),
            "--manifest",
            str(MANIFEST),
            "--output",
            str(output_dir),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert (output_dir / "stats.json").exists()
    assert (output_dir / "stats.md").exists()


def test_graph_stats_total_nodes_edges(tmp_path: Path) -> None:
    stats = graph_stats.compute_stats(MANIFEST)
    json_path = tmp_path / "stats.json"
    md_path = tmp_path / "stats.md"
    graph_stats.write_stats(stats, json_path, md_path)

    persisted = json.loads(json_path.read_text(encoding="utf-8"))
    assert persisted["manifest_edge_count"] == 9418
    assert persisted["total_edges"] == 9418
    assert persisted["networkx_graph_edges"] == 9418
    assert persisted["total_nodes"] > 0
    assert persisted["self_loops"]["count"] == 0
    assert persisted["orphans"]["count"] == 0


def test_graph_stats_per_layer_counts_and_similarity() -> None:
    stats = graph_stats.compute_stats(MANIFEST)

    assert {layer: data["edge_count"] for layer, data in stats["per_layer"].items()} == EXPECTED_LAYER_COUNTS
    assert stats["per_layer"]["citation"]["mean_similarity"] is None
    assert stats["per_layer"]["table_similarity"]["mean_similarity"] is not None
    assert stats["per_layer"]["figure_similarity_v1"]["mean_similarity"] is not None
    assert stats["per_layer"]["figure_similarity_v2"]["mean_similarity"] is not None


def test_graph_validate_runs_on_m058_manifest(tmp_path: Path) -> None:
    output_dir = tmp_path / "validation-output"
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPTS / "m060b_graph_validate.py"),
            "--manifest",
            str(MANIFEST),
            "--output",
            str(output_dir),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads((output_dir / "validation.json").read_text(encoding="utf-8"))
    assert payload["summary"]["failed"] == 0
    assert payload["summary"]["passed"] >= 6
    assert payload["overall_status"] in {"pass", "pass_with_warnings"}


def test_graph_validate_catches_self_loops(tmp_path: Path) -> None:
    payload = json.loads(MANIFEST.read_text(encoding="utf-8"))
    payload["edges"][0]["target_artifact_id"] = payload["edges"][0]["source_artifact_id"]
    manifest_path = tmp_path / "self-loop-manifest.json"
    manifest_path.write_text(json.dumps(payload), encoding="utf-8")

    validation = graph_validate.validate_manifest(manifest_path)
    self_loop_check = next(check for check in validation["checks"] if check["id"] == "self_loops")

    assert validation["overall_status"] == "fail"
    assert self_loop_check["status"] == "FAIL"
    assert self_loop_check["details"]["count"] == 1


def test_5_safety_defaults_and_loopback_contract(tmp_path: Path) -> None:
    stats = graph_stats.compute_stats(MANIFEST)
    validation = graph_validate.validate_manifest(MANIFEST)
    validation_md = tmp_path / "validation.md"
    graph_validate.write_validation(validation, tmp_path / "validation.json", validation_md)

    assert graph_stats.SAFETY_DEFAULTS == EXPECTED_SAFETY_DEFAULTS
    assert graph_validate.SAFETY_DEFAULTS == EXPECTED_SAFETY_DEFAULTS
    assert stats["safety_defaults"] == EXPECTED_SAFETY_DEFAULTS
    assert stats["loopback_bind_host"] == "127.0.0.1"
    assert FORBIDDEN_LOOPBACK_ALIAS not in validation_md.read_text(encoding="utf-8")


def test_m050_m062_inputs_are_not_modified_by_stats_or_validation(tmp_path: Path) -> None:
    protected_paths = [MANIFEST, M054_MANIFEST]
    before = {path: path.stat().st_mtime_ns for path in protected_paths}

    stats = graph_stats.compute_stats(MANIFEST)
    graph_stats.write_stats(stats, tmp_path / "stats.json", tmp_path / "stats.md")
    validation = graph_validate.validate_manifest(MANIFEST)
    graph_validate.write_validation(validation, tmp_path / "validation.json", tmp_path / "validation.md")

    after = {path: path.stat().st_mtime_ns for path in protected_paths}
    assert after == before
