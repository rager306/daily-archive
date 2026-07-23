"""M239 S01: verify_import_hold_inventory operator script."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from research_graph.application.corpus.composition_import_hold_inventory import (
    default_import_hold_roots,
    inventory_import_hold_trees,
)

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_import_hold_inventory.py"


def _run(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=cwd or ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def test_script_exists() -> None:
    assert SCRIPT.is_file()


def test_default_package_roots_exit_zero() -> None:
    # Precondition: inventory itself is clean.
    inv = inventory_import_hold_trees(default_import_hold_roots())
    assert inv["enablement_hit_count"] == 0

    proc = _run()
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "verdict: pass" in proc.stdout
    assert "import_eligible: false" in proc.stdout
    assert "enablement_hits: 0" in proc.stdout


def test_json_report_is_import_blocked() -> None:
    proc = _run("--json")
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["import_eligible"] is False
    assert report["graph_writes_allowed"] is False
    assert report["enablement_hit_count"] == 0
    assert report["verdict"] == "pass"
    assert report["tree_count"] == 4


def test_dirty_fixture_root_exits_one(tmp_path: Path) -> None:
    bad = tmp_path / "bad_wire.py"
    bad.write_text(
        "class Gate:\n    import_eligible = True\n    graph_writes_allowed = False\n",
        encoding="utf-8",
    )
    out = tmp_path / "report.json"
    proc = _run("--root", str(tmp_path), "--json", "--output", str(out))
    assert proc.returncode == 1, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["verdict"] == "fail"
    assert report["enablement_hit_count"] >= 1
    assert report["import_eligible"] is False
    assert report["graph_writes_allowed"] is False
    assert out.is_file()
    saved = json.loads(out.read_text(encoding="utf-8"))
    assert saved["enablement_hit_count"] >= 1
    assert saved["import_eligible"] is False


def test_string_marker_fixture_still_exits_zero(tmp_path: Path) -> None:
    # Docstring / JSON-ish markers must not fail the operator script.
    (tmp_path / "docs.py").write_text(
        '"""does not set import_eligible=true"""\n'
        'MARKERS = ("import_eligible: true",)\n'
        "import_eligible = False\n",
        encoding="utf-8",
    )
    proc = _run("--root", str(tmp_path), "--json")
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["enablement_hit_count"] == 0
    assert report["verdict"] == "pass"
