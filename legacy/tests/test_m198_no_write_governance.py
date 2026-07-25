from __future__ import annotations

import importlib.util
import re
from pathlib import Path
from types import ModuleType
from typing import Any

ROOT = Path(__file__).resolve().parents[1]

M198_READINESS_SCRIPTS = [
    ROOT / "scripts/run_m198_dry_run_probe.py",
    ROOT / "scripts/run_m198_sync_rehearsal_probe.py",
    ROOT / "scripts/run_m198_smoke_boundary_probe.py",
    ROOT / "scripts/run_m198_graph_readiness_probe.py",
    ROOT / "scripts/run_m198_drift_classifier.py",
    ROOT / "scripts/run_m198_evidence_index.py",
    ROOT / "scripts/run_m198_operator_diagnostics.py",
    ROOT / "scripts/run_m198_readiness_report.py",
]

REQUIRED_NON_GOALS = {
    "production_graph_import",
    "schema_migration",
    "queue_dependency_semantic_change",
    "smoke_semantic_change",
    "rehearsal_semantic_change",
    "retired_graph_readiness_shim",
    "import_eligible_true",
}

FORBIDDEN_ENABLED_PATTERNS = {
    "graph_writes_allowed enabled": re.compile(r"[\"']?graph_writes_allowed[\"']?\s*[:=]\s*(?:True|true)"),
    "schema_migration_allowed enabled": re.compile(r"[\"']?schema_migration_allowed[\"']?\s*[:=]\s*(?:True|true)"),
    "import_eligible enabled": re.compile(r"[\"']?import_eligible[\"']?\s*[:=]\s*(?:True|true)"),
    "production graph import enabled": re.compile(r"[\"']?production_graph_import_enabled[\"']?\s*[:=]\s*(?:True|true)"),
}


def _load_report_module() -> ModuleType:
    spec = importlib.util.spec_from_file_location("m198_readiness_report", ROOT / "scripts/run_m198_readiness_report.py")
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _index(non_goals: set[str] | None = None) -> dict[str, Any]:
    required = [
        "reactive_dry_run",
        "sync_no_write_rehearsal",
        "smoke_boundary",
        "graph_readiness_validate_only",
        "governance_ratchet",
    ]
    return {
        "schema_version": "m198.readiness_evidence_index.v1",
        "status": "pass",
        "required_source_kinds": required,
        "observed_source_kinds": required,
        "missing_source_kinds": [],
        "entry_count": len(required),
        "entries": [
            {"source_kind": source, "status": "pass", "drift_class": "expected"} for source in required
        ],
        "non_goal_coverage": sorted(non_goals or REQUIRED_NON_GOALS),
        "warnings": [],
        "blockers": [],
        "metadata_only": True,
        "payload_policy": {
            "stores_paths": True,
            "stores_checksums": True,
            "stores_payload_text": False,
            "stores_embeddings": False,
            "stores_vectors": False,
            "stores_credentials": False,
            "stores_queue_database_bytes": False,
        },
    }


def _diagnostics() -> dict[str, Any]:
    required = _index()["required_source_kinds"]
    return {
        "schema_version": "m198.operator_diagnostics.v1",
        "verdict": "ready",
        "ready": True,
        "index_status": "pass",
        "source_coverage": {
            "required_count": len(required),
            "observed_count": len(required),
            "missing_count": 0,
            "required_source_kinds": required,
            "observed_source_kinds": required,
            "missing_source_kinds": [],
        },
        "entry_count": len(required),
        "warnings": [],
        "blockers": [],
        "blocked_transitions": [],
        "payload_policy_confirmed": True,
        "metadata_only": True,
        "next_actions": [],
    }


def test_m198_readiness_scripts_do_not_enable_writes_or_import_eligibility() -> None:
    retired_module = ".".join(("arxiv_archive", "graph_readiness_review"))
    failures: list[str] = []
    for path in M198_READINESS_SCRIPTS:
        text = path.read_text(encoding="utf-8")
        if retired_module in text:
            failures.append(f"{path}: retired graph readiness shim restored")
        for label, pattern in FORBIDDEN_ENABLED_PATTERNS.items():
            if pattern.search(text):
                failures.append(f"{path}: {label}")
    assert failures == []


def test_readiness_report_preserves_required_non_goals_as_blocked_transitions() -> None:
    report_module = _load_report_module()

    report = report_module.build_report(_index(), _diagnostics())

    assert report["schema_version"] == "m198.readiness_report.v1"
    assert REQUIRED_NON_GOALS.issubset(set(report["blocked_transitions"]))
    assert report["metadata_only"] is True
    assert report["payload_policy_confirmed"] is True
    assert report["verdict"] == "ready"


def test_readiness_report_blocks_missing_non_goal_coverage() -> None:
    report_module = _load_report_module()
    index = _index(non_goals=REQUIRED_NON_GOALS - {"import_eligible_true"})
    diagnostics = _diagnostics()

    report = report_module.build_report(index, diagnostics)
    missing_non_goals = REQUIRED_NON_GOALS - set(report["blocked_transitions"])

    assert missing_non_goals == {"import_eligible_true"}
    assert "import_eligible_true" not in report["blocked_transitions"]


def test_readiness_report_payload_policy_remains_fail_closed() -> None:
    report_module = _load_report_module()
    index = _index()
    index["payload_policy"]["stores_credentials"] = True
    diagnostics = _diagnostics()

    report = report_module.build_report(index, diagnostics)

    assert report["verdict"] == "blocked"
    assert report["payload_policy_confirmed"] is False
    assert "metadata-only payload policy is not confirmed" in report["blockers"]
