from __future__ import annotations

from pathlib import Path

from scripts import m066_graphdb_full_benchmark as benchmark

ROOT = Path(__file__).resolve().parents[1]
ARTIFACT_DIR = ROOT / "artifacts" / "m066-graphdb-reselection"
FALKOR_REPORT = ARTIFACT_DIR / "candidates" / "falkordb-report.md"
MATRIX = ARTIFACT_DIR / "scoring-matrix.md"
DISTRIBUTION_MODEL = ARTIFACT_DIR / "distribution-model.md"
SCRIPT = ROOT / "scripts" / "m066_graphdb_full_benchmark.py"


def _read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def test_falkordb_license_corrected_to_ssplv1() -> None:
    text = _read(FALKOR_REPORT)
    assert "FalkorDB is SSPLv1" in text
    assert "not AGPLv3" in text
    assert "## 7. License fit\nScore: **4/5**." in text
    assert "**Total score:** 70/90" in text


def test_distribution_model_assumption_documented() -> None:
    text = _read(DISTRIBUTION_MODEL)
    assert "daily-archive is a self-hosted research project" in text
    assert "single-user workflow" in text
    assert "future daily-archive distribution model is uncertain" in text
    assert "contact FalkorDB for a commercial license" in text
    assert "migrate to a permissive fallback such as Apache AGE" in text


def test_falkordb_self_hosted_viable() -> None:
    report = _read(FALKOR_REPORT)
    distribution = _read(DISTRIBUTION_MODEL)
    combined = f"{report}\n{distribution}"
    assert "self-hosted use does not require source disclosure" in combined
    assert "SaaS or a hosted service for third parties triggers" in combined
    assert "Internal proprietary applications are allowed" in combined
    assert "Evaluation, prototyping, and internal testing" in combined
    assert "Docker, Kubernetes, and standalone" in combined
    assert "Redis 8.0+" in combined
    assert "Startup: $73 per 1GB-month" in combined
    assert "Pro: $350 per 8GB-month" in combined


def test_scoring_matrix_updated() -> None:
    text = _read(MATRIX)
    assert "| **Total score** | **70/90**" in text
    assert "| FalkorDB | 35/45 | #2 | 70/90 | #1 self-hosted" in text
    assert "M067 self-hosted ranking" in text
    assert "FalkorDB 70/90 > Apache AGE 64/90 > LadybugDB 62/90" in text
    assert "Neo4j remains the highest total scorer at **76/90**" not in text
    assert "Neo4j remains the highest total scorer after advanced criteria" in text
    assert "AGPLv3 remains viral for self-hosted distribution" in text
    assert (
        "License-clean candidates remain Apache AGE 64/90, LadybugDB 62/90, and HelixDB 54/90"
        in text
    )


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
    source = _read(SCRIPT)
    assert source.count("_ENABLED_BY_DEFAULT = False") == 5
    assert "Production graph import is not authorized" in _read(MATRIX)
    assert "real DB connections are disabled" in _read(MATRIX)


def test_127_not_127() -> None:
    loopback = ".".join(["127", "0", "0", "1"])
    source = _read(SCRIPT)
    docs = "\n".join(_read(path) for path in [FALKOR_REPORT, MATRIX, DISTRIBUTION_MODEL])
    assert loopback in source
    local_host_alias = "local" + "host"
    assert local_host_alias not in docs.lower()
    assert loopback not in docs


def test_m050_m066_regression() -> None:
    m045 = ROOT / "artifacts" / "m045-project-trajectory" / "current" / "trajectory-report.json"
    m044 = ROOT / "artifacts" / "m044-grobid-architecture-guardrail" / "final-report.md"
    m050 = ROOT / "tests" / "test_m050_e2e_pipeline.py"
    m063_matrix = ROOT / "artifacts" / "m063-graphdb" / "scoring-matrix.md"
    adr_020 = ROOT / "doc" / "adr" / "ADR-020-graphdb-selection.md"
    adr_021 = ROOT / "doc" / "adr" / "ADR-021-graphdb-reselection.md"

    assert m045.exists()
    assert "on_track" in _read(m045)
    assert m044.exists()
    m044_text = _read(m044)
    assert "No graph import is authorized" in m044_text
    assert "disabled" in m044_text
    assert m050.exists()
    assert "LadybugDB" in _read(m063_matrix)
    assert "Status:** Accepted (binding)" in _read(adr_020)
    assert "Status:** Accepted (binding)" in _read(adr_021)
    assert "**Winner: Neo4j (76/90).**" not in _read(MATRIX)
