"""Coverage of rustfmt checks across recorded task verifications.

`S##-T##-VERIFY.json` is the engine's machine-readable record of what a task actually
verified: `checks[]` with the exact command, exit code and verdict. That makes one
question answerable without touching the engine database — of the tasks verified with
a `cargo` command, how many also ran a rustfmt check?

This is measured as *evidence* for the commit-failure signal, not as a standing flag.
Measured on 2026-09-27: 54 tasks were cargo-verified in the preceding week and 44 had
no fmt check, so a per-task rule would fire on 81% of Rust work. A signal that is
nearly always red is not a signal, so the number explains a real failure instead of
standing alone.
"""

from __future__ import annotations

import json
import os
import time
from pathlib import Path
from typing import Any

_MAX_VERIFY_FILES = 4000
# `cargo test|build|run|clippy|fmt ...` is the canonical toolchain entry point. A task
# verified only by grep or pytest is not treated as Rust work.
_CARGO_PREFIX = "cargo "
_FMT_MARKERS = ("cargo fmt", "rustfmt")


def rust_verification_coverage(project_root: Path, window_days: int) -> dict[str, int]:
    """Count cargo-verified tasks and how many of them checked formatting."""
    phases = Path(project_root) / ".gsd" / "phases"
    rust_tasks = 0
    with_fmt = 0
    if not phases.is_dir() or window_days < 1:
        return {"rust_tasks": 0, "rust_tasks_with_fmt_check": 0}

    cutoff = time.time() - window_days * 86400
    scanned = 0
    for path in sorted(phases.glob("*/S*-VERIFY.json")):
        if scanned >= _MAX_VERIFY_FILES:
            break
        scanned += 1
        record = _load(path)
        if record is None:
            continue
        if not _in_window(record, path, cutoff):
            continue
        commands = _commands(record)
        if not any(command.startswith(_CARGO_PREFIX) for command in commands):
            continue
        rust_tasks += 1
        if any(marker in command for marker in _FMT_MARKERS for command in commands):
            with_fmt += 1
    return {"rust_tasks": rust_tasks, "rust_tasks_with_fmt_check": with_fmt}


def _load(path: Path) -> dict[str, Any] | None:
    try:
        value = json.loads(path.read_text(encoding="utf-8", errors="replace"))
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def _in_window(record: dict[str, Any], path: Path, cutoff: float) -> bool:
    """Prefer the recorded timestamp; fall back to mtime when it is unusable."""
    stamp = record.get("timestamp")
    if isinstance(stamp, (int, float)) and not isinstance(stamp, bool):
        seconds = stamp / 1000.0 if stamp > 1e11 else float(stamp)
        return seconds >= cutoff
    try:
        return os.path.getmtime(path) >= cutoff
    except OSError:
        return False


def _commands(record: dict[str, Any]) -> list[str]:
    return [
        str(check.get("command") or "").strip()
        for check in record.get("checks") or []
        if isinstance(check, dict)
    ]
