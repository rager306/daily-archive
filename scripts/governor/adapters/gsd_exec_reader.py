"""Read-only GSD exec evidence audit adapter (M371 session hardening).

Detects commands whose process exit code is zero while their captured output
contains an explicit failing test/build marker. Only evidence newer than the
latest committed source is actionable: older incidents are considered reviewed
by the commit boundary and remain in immutable GSD history.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

_FAILURE_MARKERS = (
    "test result: FAILED",
    "FAILED (failures=",
    "BUILD FAILED",
    "MILESTONE VERIFICATION FAILED",
)
_MAX_OUTPUT_BYTES = 256_000
_MAX_META_FILES = 200


@dataclass(frozen=True)
class FalseSuccessEvidence:
    evidence_id: str
    purpose: str
    marker: str
    finished_at: str


@dataclass(frozen=True)
class TimeoutEvidence:
    """A bounded command whose process was killed at the timeout cap.

    A timeout is never a success. `exit_code` is carried because the 2026-09-20..27
    review found six captures that reported `exit_code == 0` while `timed_out` was
    true, which a reader trusting exit codes alone would count as a clean run.
    """

    evidence_id: str
    purpose: str
    exit_code: int | None
    duration_ms: int
    finished_at: str


class GsdExecReader:
    """Bounded, best-effort reader for `.gsd/exec` evidence files."""

    def __init__(self, project_root: Path):
        self.exec_dir = project_root / ".gsd" / "exec"

    def false_successes_since(self, since: datetime | None) -> list[FalseSuccessEvidence]:
        if not self.exec_dir.is_dir():
            return []
        findings: list[FalseSuccessEvidence] = []
        meta_paths = sorted(
            self.exec_dir.glob("*.meta.json"),
            key=lambda path: path.stat().st_mtime,
            reverse=True,
        )[:_MAX_META_FILES]
        for meta_path in meta_paths:
            meta = self._load_meta(meta_path)
            if not meta or meta.get("exit_code") != 0:
                continue
            finished = _parse_timestamp(meta.get("finished_at"))
            if since is not None and (finished is None or finished <= since):
                continue
            marker = self._find_failure_marker(meta)
            if marker is not None:
                findings.append(
                    FalseSuccessEvidence(
                        evidence_id=str(meta.get("id") or meta_path.name.removesuffix(".meta.json")),
                        purpose=str(meta.get("purpose") or "unknown purpose"),
                        marker=marker,
                        finished_at=str(meta.get("finished_at") or "unknown time"),
                    )
                )
        return findings

    def timeouts_since(self, since: datetime | None) -> list[TimeoutEvidence]:
        """Return captures killed at the timeout cap, newest first.

        Bounded exactly like `false_successes_since`: only the newest
        `_MAX_META_FILES` captures are examined, so the cost stays flat as the store
        grows. `since` applies the same commit-boundary rule — older timeouts are
        historical and were already triaged when the tree moved on.
        """
        if not self.exec_dir.is_dir():
            return []
        findings: list[TimeoutEvidence] = []
        meta_paths = sorted(
            self.exec_dir.glob("*.meta.json"),
            key=lambda path: path.stat().st_mtime,
            reverse=True,
        )[:_MAX_META_FILES]
        for meta_path in meta_paths:
            meta = self._load_meta(meta_path)
            if not meta or not meta.get("timed_out"):
                continue
            finished = _parse_timestamp(meta.get("finished_at"))
            if since is not None and (finished is None or finished <= since):
                continue
            exit_code = meta.get("exit_code")
            findings.append(
                TimeoutEvidence(
                    evidence_id=str(meta.get("id") or meta_path.name.removesuffix(".meta.json")),
                    purpose=str(meta.get("purpose") or "unknown purpose"),
                    exit_code=exit_code if isinstance(exit_code, int) else None,
                    duration_ms=int(meta.get("duration_ms") or 0),
                    finished_at=str(meta.get("finished_at") or "unknown time"),
                )
            )
        return findings

    @staticmethod
    def _load_meta(path: Path) -> dict | None:
        try:
            value = json.loads(path.read_text(encoding="utf-8", errors="replace"))
        except (OSError, json.JSONDecodeError):
            return None
        return value if isinstance(value, dict) else None

    @staticmethod
    def _find_failure_marker(meta: dict) -> str | None:
        for key in ("stdout_path", "stderr_path"):
            raw_path = meta.get(key)
            if not raw_path:
                continue
            path = Path(str(raw_path))
            try:
                with path.open("rb") as handle:
                    data = handle.read(_MAX_OUTPUT_BYTES + 1)
            except OSError:
                continue
            if len(data) > _MAX_OUTPUT_BYTES:
                data = data[-_MAX_OUTPUT_BYTES:]
            text = data.decode("utf-8", errors="replace")
            for marker in _FAILURE_MARKERS:
                if marker in text:
                    return marker
        return None


def _parse_timestamp(value: object) -> datetime | None:
    if not isinstance(value, str):
        return None
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)
