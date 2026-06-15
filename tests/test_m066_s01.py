from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "m066_graphdb_full_benchmark.py"
ARTIFACT_DIR = ROOT / "artifacts" / "m066-graphdb-reselection"
CANDIDATE_DIR = ARTIFACT_DIR / "candidates"
MATRIX = ARTIFACT_DIR / "scoring-matrix.md"
BENCHMARK_DATA = ARTIFACT_DIR / "benchmark-data.json"

spec = importlib.util.spec_from_file_location("m066_graphdb_full_benchmark", SCRIPT)
assert spec is not None
benchmark = importlib.util.module_from_spec(spec)
sys.modules["m066_graphdb_full_benchmark"] = benchmark
assert spec.loader is not None
spec.loader.exec_module(benchmark)

EXPECTED_REPORTS = {
    "falkordb-report.md",
    "ladybugdb-report.md",
    "neo4j-report.md",
    "helixdb-report.md",
    "age-report.md",
}

ADVANCED_SECTION_TITLES = {
    "## 13. Concurrent write semantics",
    "## 14. GRAFBLAS graph algorithms",
    "## 15. UDF support",
    "## 16. ACID transactions",
    "## 17. Multi-process safety",
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
    assert payload["winner"] == "Neo4j"
    assert payload["top_3"] == ["Neo4j", "FalkorDB", "Apache AGE"]
    assert len(payload["criteria_order"]) >= 17
    assert len(payload["candidates"]) == 5
    for candidate in payload["candidates"]:
        metrics = candidate["concurrent_write_metrics"]
        assert metrics["writer_count"] == 3
        assert metrics["writes_per_writer"] == 100
        assert metrics["attempted_writes"] == 300
        assert "DB_HOST" in candidate["env_config"]
        assert "DB_PORT" in candidate["env_config"]


def test_scoring_matrix_has_top_3() -> None:
    text = MATRIX.read_text(encoding="utf-8")
    assert "## Top-3 candidates" in text
    assert "### #1 Neo4j — 76/90" in text
    assert "### #2 FalkorDB — 68/90" in text
    assert "### #3 Apache AGE — 64/90" in text
    assert "**Winner: Neo4j (76/90).**" in text


def test_candidate_reports_have_17_criteria_each() -> None:
    for report_name in EXPECTED_REPORTS:
        text = (CANDIDATE_DIR / report_name).read_text(encoding="utf-8")
        criteria_sections = [line for line in text.splitlines() if line.startswith("## ") and ". " in line]
        assert len(criteria_sections) >= 17, report_name
        for section in range(1, 19):
            assert f"## {section}." in text, report_name
        assert "## Advanced features section" in text, report_name
        assert "## Concurrent write benchmark" in text, report_name


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


def test_m063_known_limitations_addressed() -> None:
    for report_name in EXPECTED_REPORTS:
        text = (CANDIDATE_DIR / report_name).read_text(encoding="utf-8")
        for title in ADVANCED_SECTION_TITLES:
            assert title in text, report_name
        assert "concurrent writes" in text.lower(), report_name
        assert "GRAFBLAS" in text, report_name
        assert "UDF" in text, report_name
        assert "ACID" in text, report_name
        assert "Multi-process" in text, report_name


def test_m050_m065_regression() -> None:
    m045 = ROOT / "artifacts" / "m045-project-trajectory" / "current" / "trajectory-report.json"
    m044 = ROOT / "artifacts" / "m044-grobid-architecture-guardrail" / "final-report.md"
    m050 = ROOT / "tests" / "test_m050_e2e_pipeline.py"
    m062 = ROOT / "artifacts" / "m062-fd-contract" / "fd-contract-results.json"
    m063_matrix = ROOT / "artifacts" / "m063-graphdb" / "scoring-matrix.md"

    assert m045.exists()
    assert "on_track" in m045.read_text(encoding="utf-8")
    m044_text = m044.read_text(encoding="utf-8")
    assert "No graph import is authorized" in m044_text
    assert "disabled" in m044_text
    assert m050.exists()
    assert m062.exists()
    assert "LadybugDB" in m063_matrix.read_text(encoding="utf-8")

    payload = json.loads(BENCHMARK_DATA.read_text(encoding="utf-8"))
    by_name = {candidate["name"]: candidate for candidate in payload["candidates"]}
    assert by_name["LadybugDB"]["legacy_m063_score_45"] == 39
    assert by_name["Neo4j"]["rank"] == 1
    assert by_name["LadybugDB"]["rank"] == 4
