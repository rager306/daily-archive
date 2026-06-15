from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RESULTS_PATH = ROOT / "artifacts" / "m068-fd-v2-integration-test" / "results.json"
REPORT_PATH = ROOT / "artifacts" / "m068-fd-v2-integration-test" / "REPORT.md"
SCRIPT_PATH = ROOT / "scripts" / "m068_integration_test.py"
ADR_PATH = ROOT / "doc" / "adr" / "ADR-019-fd-embedding-service-contract.md"
ADR_INDEX_PATH = ROOT / "doc" / "adr" / "ADR-INDEX.md"
SUMMARY_PATH = ROOT / ".gsd" / "milestones" / "M068-hlcxny" / "M068-hlcxny-SUMMARY.md"
VALIDATION_PATH = ROOT / ".gsd" / "milestones" / "M068-hlcxny" / "M068-hlcxny-VALIDATION.md"
CODEBASE_MEMORY_ADR_PATH = ROOT / ".codebase-memory" / "adr.md"
CODEBASE_MEMORY_GRAPH_PATH = ROOT / ".codebase-memory" / "governance-graph.json"

REGRESSION_TEST_FILES = [
    "tests/test_m044_sidecar_architecture_guardrail.py",
    "tests/test_m045_project_trajectory.py",
    "tests/test_m050_article_artifact_reducer.py",
    "tests/test_m050_article_artifact_worker.py",
    "tests/test_m050_e2e_pipeline.py",
    "tests/test_m062_s01.py",
    "tests/test_m062_s02.py",
    "tests/test_m062_s03.py",
]

SAFETY_DEFAULTS = {
    "graph_writes_authorized",
    "production_import_authorized",
    "fact_promotion_authorized",
    "external_network_authorized",
    "llm_calls_authorized",
}


def _results() -> dict:
    assert RESULTS_PATH.exists(), "S03 results.json must exist"
    return json.loads(RESULTS_PATH.read_text(encoding="utf-8"))


def test_integration_test_150_papers_completed() -> None:
    data = _results()
    assert SCRIPT_PATH.exists()
    assert REPORT_PATH.exists()
    assert data["selected_papers"] == 150
    assert len(data["paper_manifest"]) == 150
    assert sorted(data["anchors"]) == [
        "anchor-2207.05608",
        "anchor-2401.04016",
        "anchor-2505.19443",
        "anchor-2510.12157",
        "anchor-2605.18747",
    ]
    assert data["status"] in {"PASS", "SKIP"}
    if data["status"] == "PASS":
        assert data["processed_papers"] == 150
        assert data["successful_papers"] == 150
        assert data["failed_papers"] == 0
    else:
        assert data["processed_papers"] == 0
        assert data["skip_reason"]


def test_throughput_meets_target() -> None:
    data = _results()
    if data["status"] == "SKIP":
        assert "is not authorized" in data["skip_reason"] or "is disabled" in data["skip_reason"]
        assert data["throughput_papers_per_min"] == 0.0
        assert data["latency"] == {"p50_ms": None, "p95_ms": None, "p99_ms": None}
        assert data["error_rate"] is None
    else:
        assert data["throughput_papers_per_min"] >= 1.0
        assert data["latency"]["p50_ms"] is not None
        assert data["latency"]["p95_ms"] is not None
        assert data["latency"]["p99_ms"] is not None
        assert data["error_rate"] == 0.0


def test_adr_019_amendment_log_v2() -> None:
    adr = ADR_PATH.read_text(encoding="utf-8")
    assert "## Amendment Log" in adr
    amendment_rows = re.findall(r"^\| 2026-", adr, flags=re.MULTILINE)
    assert len(amendment_rows) >= 2
    assert "M068 S03" in adr
    for env_name in ("FD_API_KEY", "MODEL_ID", "TEI_URL", "REDIS_HOST", "REDIS_PORT"):
        assert env_name in adr

    index = ADR_INDEX_PATH.read_text(encoding="utf-8")
    assert "| ADR-019 | Accepted (binding) | M062 fd Embedding Service Contract |" in index
    assert "Amendment Log entries: 2" in index


def test_m068_closeout_artifacts() -> None:
    assert SUMMARY_PATH.exists()
    assert VALIDATION_PATH.exists()
    summary = SUMMARY_PATH.read_text(encoding="utf-8")
    validation = VALIDATION_PATH.read_text(encoding="utf-8")
    assert "id: M068-hlcxny" in summary
    assert "verification_result: needs-attention" in summary
    assert "verdict: needs-attention" in validation
    assert "M045/M044" in validation
    assert "127.0.0.1" not in validation


def test_code_memory_synced() -> None:
    mirror = CODEBASE_MEMORY_ADR_PATH.read_text(encoding="utf-8")
    graph = json.loads(CODEBASE_MEMORY_GRAPH_PATH.read_text(encoding="utf-8"))
    assert "ADR-019" in mirror
    assert "doc/adr/ADR-019-fd-embedding-service-contract.md" in mirror
    adr_019_nodes = [node for node in graph.get("nodes", []) if node.get("id") == "ADR-019"]
    assert adr_019_nodes
    assert adr_019_nodes[0].get("canonical_source") == "doc/adr/ADR-019-fd-embedding-service-contract.md"


def test_5_safety_defaults() -> None:
    data = _results()
    assert set(data["safety_defaults"]) == SAFETY_DEFAULTS
    assert all(value is False for value in data["safety_defaults"].values())
    serialized = RESULTS_PATH.read_text(encoding="utf-8")
    assert "Bearer " not in serialized


def test_127_not_127() -> None:
    script = SCRIPT_PATH.read_text(encoding="utf-8")
    report = REPORT_PATH.read_text(encoding="utf-8")
    assert "localhost" not in script.lower()
    assert "localhost" not in report.lower()
    assert "DEFAULT_TEI_URL = \"http://tei:80\"" in script
    assert "127.0.0.1" not in VALIDATION_PATH.read_text(encoding="utf-8")


def test_m050_m068_s01_s02_regression() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "pytest", *REGRESSION_TEST_FILES, "-q"],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
        timeout=240,
    )
    assert result.returncode == 0, result.stdout + result.stderr
