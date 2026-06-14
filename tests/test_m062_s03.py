from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = ROOT / "scripts" / "test_fd_contract.py"
REPORT_DIR = ROOT / "artifacts" / "m062-fd-contract"
REPORT_MD = REPORT_DIR / "fd-contract-report.md"
GAP_MD = REPORT_DIR / "fd-actual-vs-required.md"


def _load_contract_module():
    spec = importlib.util.spec_from_file_location("test_fd_contract", SCRIPT_PATH)
    assert spec is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["test_fd_contract"] = module
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def test_fd_contract_report_md_exists() -> None:
    assert REPORT_MD.exists()
    text = REPORT_MD.read_text(encoding="utf-8")
    assert "# M062 fd Contract Report" in text
    assert "total=52" in text


def test_fd_contract_total_52_tests() -> None:
    module = _load_contract_module()
    test_ids = module.build_tests()
    assert len(test_ids) == 52
    assert len(set(test_ids)) == 52
    assert test_ids[:2] == ["T-H-1", "T-H-2"]
    assert test_ids[-4:] == ["T-ENV-1", "T-ENV-2", "T-ENV-3", "T-ENV-4"]


def test_fd_actual_vs_required_md_exists() -> None:
    assert GAP_MD.exists()
    text = GAP_MD.read_text(encoding="utf-8")
    assert "# M062 fd Actual vs Required" in text
    assert "| P0 |" in text
    assert "R-P0-1" in text
    assert "R-P2-6" in text


def test_gap_analysis_prioritized() -> None:
    text = REPORT_MD.read_text(encoding="utf-8")
    p0_index = text.index("### P0")
    p1_index = text.index("### P1")
    p2_index = text.index("### P2")
    assert p0_index < p1_index < p2_index
    assert "**R-P0-" in text


def test_contract_test_handles_fd_down(tmp_path: Path) -> None:
    env = os.environ.copy()
    env["FD_EMBEDDINGS_ENDPOINT"] = "invalid://fd-test"
    env["FD_EMBEDDINGS_ENDPOINT_BASE"] = "invalid://fd-test"
    env["FD_CONTRACT_REPORT_DIR"] = str(tmp_path)
    env["FD_CONTRACT_TIMEOUT_SECONDS"] = "0.2"
    result = subprocess.run(
        [sys.executable, str(SCRIPT_PATH)],
        cwd=ROOT,
        env=env,
        check=False,
        text=True,
        capture_output=True,
        timeout=30,
    )
    assert result.returncode == 0
    assert "Summary: total=52" in result.stdout
    assert "[FAIL] T-H-1" in result.stdout
    assert (tmp_path / "fd-contract-report.md").exists()
    assert (tmp_path / "fd-actual-vs-required.md").exists()


def test_m050_m062_s01_s02_regression_contracts() -> None:
    embedder = (ROOT / "src" / "arxiv_archive" / "embedder.py").read_text(encoding="utf-8")
    adr = (ROOT / "doc" / "adr" / "ADR-019-fd-embedding-service-contract.md").read_text(encoding="utf-8")
    assert '"graph_writes_authorized": False' in embedder
    assert '"production_import_authorized": False' in embedder
    assert '"fact_promotion_authorized": False' in embedder
    assert '"external_network_authorized": False' in embedder
    assert '"llm_calls_authorized": False' in embedder
    assert "**Status:** Accepted (binding)" in adr
    assert "fd v2 embedding service contract" in adr.lower()
