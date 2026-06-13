from __future__ import annotations

import hashlib
import importlib.util
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "artifacts" / "m061-2hop"
SCRIPT_PATH = ROOT / "scripts" / "m061_synthesis.py"
REPORT_PATH = BASE / "REPORT.md"
SUMMARY_PATH = BASE / "m061-summary.json"
DECISION_PATH = BASE / "m061-decision.md"
ADR_PATH = ROOT / "doc" / "adr" / "ADR-018-m061-2-hop-evidence-and-m064-trigger.md"
CLOSEOUT_SUMMARY_PATH = ROOT / ".gsd" / "milestones" / "M064-wqfgfa" / "M064-wqfgfa-SUMMARY.md"
VALIDATION_PATH = ROOT / ".gsd" / "milestones" / "M064-wqfgfa" / "M064-wqfgfa-VALIDATION.md"
CODE_MEMORY_ADR = ROOT / ".codebase-memory" / "adr.md"
CODE_MEMORY_GRAPH = ROOT / ".codebase-memory" / "governance-graph.json"

EXPECTED_SAFETY_DEFAULTS = {
    "external_network_authorized": False,
    "fact_promotion_authorized": False,
    "graph_writes_authorized": False,
    "llm_calls_authorized": False,
    "production_import_authorized": False,
}
PROTECTED_HASHES = {
    "artifacts/m061-2hop/s01-decision.md": "231cb251d89c5b77a68007ebf93efbde20be3ad97b32829500ca1b5e663a51e0",
    "artifacts/m061-2hop/s02-decision.md": "b1d64da4d19187475b6d671a0d97d41abc8cd272e2755d548fcbb8cccd352edb",
    "artifacts/m061-2hop/anchor-2605.18747/pipeline-summary.json": "28398554a4e6470956ed58cda6c0ec879ff509fb7eb49be6c81b1690d45544db",
    "artifacts/m061-2hop/5-anchor-5-layer-graph-manifest.json": "c98a561e6dd13b0a98a7451fb6193c59adaef8ba12cbf819afd4c20ebe79f78c",
}


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(data, dict)
    return data


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_synthesis_module() -> Any:
    spec = importlib.util.spec_from_file_location("m061_synthesis", SCRIPT_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["m061_synthesis"] = module
    spec.loader.exec_module(module)
    return module


def test_report_md_exists() -> None:
    assert REPORT_PATH.exists()
    report = REPORT_PATH.read_text(encoding="utf-8")
    headings = re.findall(r"^## (\d+)\.", report, flags=re.MULTILINE)
    assert headings == [str(index) for index in range(9)]
    assert "5 anchor" in report or "5 anchors" in report
    assert "8911 citation edges" in report
    assert "323 requests" in report
    assert "0 HTTP 429s" in report
    assert "7.11 papers/min" in report
    assert "CONFIRM DEFER M064" in report
    assert "127.0.0.1" in report


def test_adr_018_binding_full_template() -> None:
    assert ADR_PATH.exists()
    adr = ADR_PATH.read_text(encoding="utf-8")
    assert "**Status:** Accepted (binding)" in adr
    assert "**Binding Level:** binding" in adr
    expected_headings = [f"## {index}." for index in range(15)]
    for heading in expected_headings:
        assert heading in adr
    assert "## 14. LLM Reading Notes" in adr
    assert "CONFIRM DEFER" in adr
    assert "ADR-017" in adr
    assert "graph writes is not authorized" in adr
    assert "external network is disabled by default" in adr


def test_m061_closeout_artifacts() -> None:
    for path in [SUMMARY_PATH, DECISION_PATH, CLOSEOUT_SUMMARY_PATH, VALIDATION_PATH]:
        assert path.exists(), path
    summary = load_json(SUMMARY_PATH)
    assert summary["aggregate"]["anchor_count"] == 5
    assert summary["aggregate"]["total_arxiv_requests"] == 323
    assert summary["aggregate"]["total_http_429_count"] == 0
    assert round(summary["aggregate"]["cumulative_real_paper_throughput_per_min"], 2) == 7.11
    assert summary["graph"]["citation_node_count"] == 2662
    assert summary["graph"]["citation_edge_count"] == 8911
    assert summary["decision"]["adr_018_decision"] == "CONFIRM DEFER M064"
    validation = VALIDATION_PATH.read_text(encoding="utf-8")
    assert "verdict: pass" in validation
    assert "M045 trajectory is on_track" in validation
    assert "M044 guardrail is ok" in validation


def test_5_safety_defaults() -> None:
    summary = load_json(SUMMARY_PATH)
    assert summary["aggregate"]["safety_defaults"] == EXPECTED_SAFETY_DEFAULTS
    for anchor in summary["anchors"]:
        assert anchor["safety_defaults"] == EXPECTED_SAFETY_DEFAULTS
        assert anchor["sync_execution"] is True
        assert anchor["queue_execution"] is False
        assert anchor["network_host_reference"] == "127.0.0.1"
    generated_text = "\n".join(
        path.read_text(encoding="utf-8")
        for path in [SCRIPT_PATH, REPORT_PATH, DECISION_PATH, ADR_PATH, CLOSEOUT_SUMMARY_PATH, VALIDATION_PATH]
    )
    assert "local" + "host" not in generated_text
    assert "graph writes is not authorized" in generated_text
    assert "LLM calls are disabled by default" in generated_text


def test_code_memory_synced() -> None:
    assert CODE_MEMORY_ADR.exists()
    assert CODE_MEMORY_GRAPH.exists()
    mirror = CODE_MEMORY_ADR.read_text(encoding="utf-8")
    assert "ADR-018" in mirror
    assert "M061 2-hop Evidence and M064 Trigger Evaluation" in mirror
    graph = load_json(CODE_MEMORY_GRAPH)
    node_ids = {node["id"] for node in graph["nodes"]}
    assert "ADR-018" in node_ids


def test_m050_m064_s01_s02_regression() -> None:
    for relative_path, expected_hash in PROTECTED_HASHES.items():
        assert sha256(ROOT / relative_path) == expected_hash
    s01_decision = (BASE / "s01-decision.md").read_text(encoding="utf-8")
    s02_decision = (BASE / "s02-decision.md").read_text(encoding="utf-8")
    assert "GO to S02" in s01_decision
    assert "GO to S03 synthesis" in s02_decision
    assert "7.26" in s01_decision
    assert "7.11" in s02_decision


def test_synthesis_collect_summary_matches_written_artifact() -> None:
    module = load_synthesis_module()
    collected = module.collect_summary("2026-06-13T00:00:00Z")
    written = load_json(SUMMARY_PATH)
    assert collected["aggregate"] == written["aggregate"]
    assert collected["graph"] == written["graph"]
    assert [anchor["anchor_arxiv_id"] for anchor in collected["anchors"]] == [
        "2605.18747",
        "2401.04016",
        "2207.05608",
        "2505.19443",
        "2510.12157",
    ]
