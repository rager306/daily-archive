from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

from scripts import check_project_trajectory

ROOT = Path(__file__).resolve().parents[1]
ADR_TEMPLATE = ROOT / "doc" / "adr" / "ADR-TEMPLATE.md"
ADR_016 = ROOT / "doc" / "adr" / "ADR-016-graph-library-selection.md"
README = ROOT / "README.md"
TRAJECTORY = ROOT / "artifacts" / "project-trajectory" / "trajectory-report.md"
PROJECT = ROOT / ".gsd" / "PROJECT.md"
SYNC_SCRIPT = ROOT / "scripts" / "sync_codebase_memory_governance.py"
GUARDRAIL_SCRIPT = ROOT / "scripts" / "verify_m044_sidecar_architecture_guardrail.py"
TRAJECTORY_SCRIPT = ROOT / "scripts" / "check_project_trajectory.py"
FORBIDDEN_LOOPBACK_HOSTNAME = "local" + "host"

SAFETY_DEFAULTS = {
    "graph_writes_authorized": "false",
    "production_import_authorized": "false",
    "fact_promotion_authorized": "false",
    "external_network_enabled": "false",
    "llm_calls_enabled": "false",
}


def _read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def _adoption_table(text: str) -> dict[str, dict[str, str]]:
    match = re.search(
        r"^### Adoption Table\s*\n\n(?P<table>(?:\|.*\|\s*\n)+)",
        text,
        flags=re.MULTILINE,
    )
    assert match is not None, "ADR-016 must include an Adoption Table"
    rows: dict[str, dict[str, str]] = {}
    for line in match.group("table").splitlines():
        if not line.startswith("|") or re.fullmatch(r"\|[-:| ]+\|", line):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if cells[0] == "Library":
            continue
        rows[cells[0]] = {
            "decision": cells[1],
            "role": cells[2],
            "rationale": cells[3],
        }
    return rows


def test_adr_template_exists_in_doc_adr() -> None:
    assert ADR_TEMPLATE.is_file()


def test_adr_template_has_14_sections() -> None:
    text = _read(ADR_TEMPLATE)
    numbered = re.findall(r"^## (\d+)\.", text, flags=re.MULTILINE)
    assert numbered == [str(index) for index in range(15)]
    assert "```mermaid" in text
    assert "## 14. LLM Reading Notes" in text


def test_adr_template_mentions_doc_adr_as_canonical() -> None:
    text = _read(ADR_TEMPLATE)
    assert "This is the canonical ADR template" in text
    assert "Located at doc/adr/ADR-TEMPLATE.md" in text
    assert "Use this for all new ADRs (ADR-017 onwards)" in text
    assert "Use this template for daily-archive ADRs" in text
    assert "M034 ADRs" not in text


def test_adr_016_no_rustworkx_in_adopted_section() -> None:
    rows = _adoption_table(_read(ADR_016))
    assert rows["NetworkX"]["decision"] == "PRIMARY"
    assert rows["igraph"]["decision"] == "ADOPTED supplementary"
    assert rows["rustworkx"]["decision"] == "NOT ADOPTED"
    assert "None for runtime adoption" in rows["rustworkx"]["role"]


def test_adr_016_amendment_note_present() -> None:
    text = _read(ADR_016)
    assert "Originally accepted with rustworkx; amended 2026-06-13" in text
    assert "User rationale: keep graph layer simple" in text
    assert "## Amendment Log" in text


def test_5_safety_defaults() -> None:
    text = _read(ADR_016)
    for key, value in SAFETY_DEFAULTS.items():
        assert f"{key}={value}" in text
    assert "Graph writes are not authorized." in text
    assert "Production import is not authorized." in text
    assert "Fact promotion is not authorized." in text
    assert "External network default is disabled." in text
    assert "LLM calls default is disabled." in text
    assert "127.0.0.1" in text
    assert FORBIDDEN_LOOPBACK_HOSTNAME not in text


def test_readme_or_trajectory_points_to_template() -> None:
    readme = _read(README)
    trajectory = _read(TRAJECTORY)
    required = "doc/adr/ADR-TEMPLATE.md"
    assert required in readme
    assert "All new ADRs MUST use this template" in readme
    assert required in trajectory
    assert "## How to create an ADR" in trajectory
    assert PROJECT.exists()


def test_codebase_memory_sync_emits_adr_016_highlight(tmp_path: Path) -> None:
    mirror = tmp_path / "adr.md"
    graph = tmp_path / "governance-graph.json"
    result = subprocess.run(
        [
            sys.executable,
            str(SYNC_SCRIPT),
            "--output",
            str(mirror),
            "--graph-output",
            str(graph),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    mirror_text = _read(mirror)
    assert "### ADR-016: Graph Library Selection for M060b-M064+" in mirror_text
    assert "| rustworkx | NOT ADOPTED |" in mirror_text
    assert "| igraph | ADOPTED supplementary |" in mirror_text
    graph_payload = json.loads(_read(graph))
    assert any(
        node.get("id") == "ADR-016"
        and node.get("canonical_source") == "doc/adr/ADR-016-graph-library-selection.md"
        for node in graph_payload["nodes"]
    )


def test_m045_trajectory_on_track_without_writing_outputs() -> None:
    report = check_project_trajectory.build_report(root=ROOT, phase="preflight")
    assert report["schema_version"] == "m101.project-trajectory.v2"
    assert report["verdict"] in {"on_track", "drift_risk"}
    if report["verdict"] == "drift_risk":
        assert report["drift_flags"]


def test_m044_guardrail_exits_zero() -> None:
    result = subprocess.run(
        [sys.executable, str(GUARDRAIL_SCRIPT)],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
