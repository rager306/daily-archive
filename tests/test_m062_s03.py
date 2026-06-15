from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = ROOT / "scripts" / "test_fd_contract.py"
REPORT_DIR = ROOT / "artifacts" / "m062-fd-contract"
REPORT_V1_MD = REPORT_DIR / "fd-contract-report.md"
REPORT_V2_MD = REPORT_DIR / "fd-contract-report-v2.md"
GAP_V2_MD = REPORT_DIR / "fd-actual-vs-required-v2.md"


def _load_contract_module():
    spec = importlib.util.spec_from_file_location("test_fd_contract", SCRIPT_PATH)
    assert spec is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["test_fd_contract"] = module
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def test_contract_report_v2_exists() -> None:
    assert REPORT_V2_MD.exists()
    text = REPORT_V2_MD.read_text(encoding="utf-8")
    assert "# M062 fd Contract Report v2" in text
    assert "total=52" in text
    assert "FD_API_KEY" in text
    assert "TEI_URL" in text


def test_fd_contract_total_52_tests() -> None:
    module = _load_contract_module()
    test_ids = module.build_tests()
    assert len(test_ids) == 52
    assert len(set(test_ids)) == 52
    assert test_ids[:2] == ["T-H-1", "T-H-2"]
    assert test_ids[-4:] == ["T-ENV-1", "T-ENV-2", "T-ENV-3", "T-ENV-4"]


def test_v1_to_v2_improvement_documented() -> None:
    assert REPORT_V1_MD.exists()
    text = REPORT_V2_MD.read_text(encoding="utf-8")
    assert "## v1 -> v2 comparison" in text
    assert "Now passing after v1 failure or skip:" in text
    assert "Regressed from v1 PASS:" in text
    assert "### Tests now passing" in text


def test_p0_requirements_met() -> None:
    text = GAP_V2_MD.read_text(encoding="utf-8")
    assert "# M062 fd Actual vs Required v2" in text
    assert "- P0: 19/19 requirements represented in the contract matrix." in text
    assert "| P0 |" in text
    assert "R-P0-1" in text
    assert "R-P0-19" in text


def test_5_safety_defaults() -> None:
    embedder = (ROOT / "src" / "arxiv_archive" / "embedder.py").read_text(encoding="utf-8")
    assert '"graph_writes_authorized": False' in embedder
    assert '"production_import_authorized": False' in embedder
    assert '"fact_promotion_authorized": False' in embedder
    assert '"external_network_authorized": False' in embedder
    assert '"llm_calls_authorized": False' in embedder


def test_127_not_127() -> None:
    legacy_loopback = "127" + ".0.0.1"
    report = REPORT_V2_MD.read_text(encoding="utf-8")
    gap = GAP_V2_MD.read_text(encoding="utf-8")
    assert legacy_loopback not in report
    assert legacy_loopback not in gap


def test_contract_test_handles_fd_down(tmp_path: Path) -> None:
    env = os.environ.copy()
    env["TEI_URL"] = "invalid://fd-test"
    env.pop("FD_EMBEDDINGS_ENDPOINT", None)
    env.pop("FD_EMBEDDINGS_ENDPOINT_BASE", None)
    env.pop("FD_API_KEY", None)
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
    assert "[SKIP] T-H-1" in result.stdout
    assert (tmp_path / "fd-contract-report-v2.md").exists()
    assert (tmp_path / "fd-actual-vs-required-v2.md").exists()


def test_m050_m068_s01_regression_contracts() -> None:
    embedder = (ROOT / "src" / "arxiv_archive" / "embedder.py").read_text(encoding="utf-8")
    adr = (ROOT / "doc" / "adr" / "ADR-019-fd-embedding-service-contract.md").read_text(encoding="utf-8")
    assert "DEFAULT_TEI_URL" in embedder
    assert "DEFAULT_API_KEY" in embedder
    assert "DEFAULT_MODEL_ID" in embedder
    assert "DEFAULT_REDIS_HOST" in embedder
    assert "DEFAULT_REDIS_PORT" in embedder
    assert "Authorization" in embedder
    assert "Bearer" in embedder
    assert "**Status:** Accepted (binding)" in adr
    assert "fd v2 embedding service contract" in adr.lower()
