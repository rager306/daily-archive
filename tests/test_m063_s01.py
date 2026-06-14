from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "m063_graphdb_benchmark.py"
ARTIFACT_DIR = ROOT / "artifacts" / "m063-graphdb"
CANDIDATE_DIR = ARTIFACT_DIR / "candidates"
MATRIX = ARTIFACT_DIR / "scoring-matrix.md"

spec = importlib.util.spec_from_file_location("m063_graphdb_benchmark", SCRIPT)
assert spec is not None
benchmark = importlib.util.module_from_spec(spec)
sys.modules["m063_graphdb_benchmark"] = benchmark
assert spec.loader is not None
spec.loader.exec_module(benchmark)

EXPECTED_REPORTS = {
    "falkordb-report.md",
    "ladybugdb-report.md",
    "neo4j-report.md",
    "helixdb-report.md",
    "age-report.md",
}


def test_5_candidate_reports_exist() -> None:
    reports = {path.name for path in CANDIDATE_DIR.glob("*-report.md")}
    assert EXPECTED_REPORTS <= reports


def test_benchmark_script_runs(tmp_path: Path) -> None:
    output = tmp_path / "benchmark-data.json"
    completed = subprocess.run(
        [sys.executable, str(SCRIPT), "--output", str(output)],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    payload = json.loads(output.read_text(encoding="utf-8"))
    assert completed.returncode == 0
    assert len(payload["candidates"]) == 5
    assert len(payload["empirical_candidates"]) >= 2
    assert payload["source_data"]["workload_edges"] == 9000


def test_scoring_matrix_has_top_3() -> None:
    text = MATRIX.read_text(encoding="utf-8")
    assert "## Top-3 candidates" in text
    assert "LadybugDB" in text and "#1" in text
    assert "FalkorDB" in text and "#2" in text
    assert "Neo4j" in text and "#3" in text


def test_candidate_reports_have_10_criteria_each() -> None:
    for report_name in EXPECTED_REPORTS:
        text = (CANDIDATE_DIR / report_name).read_text(encoding="utf-8")
        numbered_sections = [line for line in text.splitlines() if line.startswith("## ")]
        assert len(numbered_sections) >= 17, report_name
        for section in range(0, 17):
            assert f"## {section}." in text, report_name


def test_5_safety_defaults() -> None:
    defaults = benchmark.SAFETY_DEFAULTS
    assert set(defaults) == {
        "network_enabled_by_default",
        "production_import_enabled_by_default",
        "graph_writes_enabled_by_default",
        "vendor_source_mutation_enabled_by_default",
        "real_db_connection_enabled_by_default",
    }
    assert all(value is False for value in defaults.values())
    source = SCRIPT.read_text(encoding="utf-8")
    assert source.count("_ENABLED_BY_DEFAULT = False") == 5


def test_127_not_127() -> None:
    checked_paths = [SCRIPT, MATRIX, *sorted(CANDIDATE_DIR.glob("*-report.md"))]
    combined = "\n".join(path.read_text(encoding="utf-8") for path in checked_paths)
    assert "127.0.0.1" in combined
    assert "localhost" not in combined.lower()


def test_m050_m065_regression() -> None:
    m045 = ROOT / "artifacts" / "m045-project-trajectory" / "current" / "trajectory-report.json"
    m044 = ROOT / "artifacts" / "m044-grobid-architecture-guardrail" / "final-report.md"
    m050 = ROOT / "tests" / "test_m050_e2e_pipeline.py"
    m062 = ROOT / "artifacts" / "m062-fd-contract" / "fd-contract-results.json"
    benchmark_json = ARTIFACT_DIR / "benchmark-data.json"

    assert m045.exists()
    assert "on_track" in m045.read_text(encoding="utf-8")
    m044_text = m044.read_text(encoding="utf-8")
    assert "No graph import is authorized" in m044_text
    assert "disabled" in m044_text
    assert m050.exists()
    assert m062.exists()

    payload = json.loads(benchmark_json.read_text(encoding="utf-8"))
    assert payload["source_data"]["layer_counts"] == {
        "citation": 8911,
        "table": 4934,
        "figure_v1": 15,
        "figure_v2": 15,
        "judge": 150,
    }
