"""M240 S01: pre-commit wiring for import-hold inventory verify."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PRECOMMIT = ROOT / ".pre-commit-config.yaml"
SCRIPT = ROOT / "scripts" / "verify_import_hold_inventory.py"


def test_precommit_config_exists() -> None:
    assert PRECOMMIT.is_file()


def test_import_hold_hook_is_wired() -> None:
    text = PRECOMMIT.read_text(encoding="utf-8")
    assert "m239-import-hold-inventory" in text
    assert "verify_import_hold_inventory.py" in text
    # Scoped to package hold surface, not always_run.
    assert re.search(
        r"id:\s*m239-import-hold-inventory[\s\S]*?always_run:\s*false",
        text,
    )
    assert "pass_filenames: false" in text
    # Must cover domain/application/composition/infrastructure surfaces.
    assert "src/research_graph/(domain|application|infrastructure)" in text or re.search(
        r"domain\|application\|infrastructure", text
    )
    assert "workflows/composition" in text or "composition" in text


def test_hook_entry_uses_uv_run_script() -> None:
    text = PRECOMMIT.read_text(encoding="utf-8")
    assert "uv run python scripts/verify_import_hold_inventory.py" in text


def test_verify_script_still_exits_zero() -> None:
    proc = subprocess.run(
        [sys.executable, str(SCRIPT)],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "verdict: pass" in proc.stdout
    assert "enablement_hits: 0" in proc.stdout
