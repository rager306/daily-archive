"""Maintain the accepted-divergence baseline for the engine doctor.

The engine doctor has no notion of reviewed historical debt: it re-reports the same
findings on every run and can never be `ok`. The baseline
(`scripts/governor/checks/gsd_doctor_baseline.json`) is how a human says "this one was
already reviewed", which turns the doctor's output into a *what is new* signal. This
service is the supported way to read and edit that file, so nobody has to hand-edit
JSON and get the schema wrong.

Two deliberate refusals:

- an acceptance must name a `code` and either a `milestone` or an explicit
  `code_wide` with a reason. A bare `--accept-all` does not exist, because widening
  suppression is how a baseline stops being a signal.
- a `code` that the latest doctor run does not actually report is refused: accepting
  a typo would put noise in the file that no future run can match.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from governor.adapters.gsd_control_plane_reader import GsdControlPlaneReader
from governor.checks.gsd_control_plane import unbaselined_issues

BASELINE_PATH = Path("scripts/governor/checks/gsd_doctor_baseline.json")
_SCHEMA_VERSION = "gsd-doctor-baseline.v1"


@dataclass(frozen=True)
class BaselineReport:
    """What the doctor currently reports versus what the baseline already accepts."""

    latest_ts: str | None
    latest_errors: int
    unbaselined: tuple[str, ...]
    accepted_count: int
    available_codes: tuple[str, ...]


class BaselineError(Exception):
    """An acceptance request that cannot be honoured safely."""


def load_baseline(project_root: Path) -> dict[str, Any]:
    """Read the baseline, returning an empty skeleton when it is absent."""
    path = Path(project_root) / _baseline_path(project_root)
    if not path.is_file():
        return {"schema_version": _SCHEMA_VERSION, "accepted": []}
    try:
        payload = json.loads(path.read_text(encoding="utf-8", errors="replace"))
    except (OSError, json.JSONDecodeError) as error:
        raise BaselineError(f"baseline is not readable JSON: {error}") from error
    if not isinstance(payload, dict):
        raise BaselineError("baseline must be a JSON object")
    accepted = payload.get("accepted")
    if accepted is None:
        payload["accepted"] = []
    elif not isinstance(accepted, list):
        raise BaselineError("baseline.accepted must be a list")
    return payload


def report(project_root: Path) -> BaselineReport:
    """Compare the latest doctor run against the baseline without writing anything.

    The baseline is read strictly here: a trajectory report degrades gracefully on a
    broken baseline, but an operator command must not, because treating junk as "empty"
    would invite re-accepting divergence that is already recorded.
    """
    from governor.adapters.gsd_control_plane_reader import normalize_baseline_entries

    project_root = Path(project_root)
    entries = normalize_baseline_entries(load_baseline(project_root))
    latest = GsdControlPlaneReader(project_root).latest_doctor_run() or {}
    codes = tuple(
        sorted(
            {
                str(issue.get("code"))
                for issue in latest.get("issues") or []
                if isinstance(issue, dict) and issue.get("code")
            }
        )
    )
    return BaselineReport(
        latest_ts=latest.get("ts") if isinstance(latest.get("ts"), str) else None,
        latest_errors=int(latest.get("errors") or 0),
        unbaselined=tuple(unbaselined_issues(latest, entries)),
        accepted_count=len(entries),
        available_codes=codes,
    )


def accept(  # noqa: PLR0913 - an acceptance is a record; every field is part of it
    project_root: Path,
    *,
    code: str,
    milestone: str | None = None,
    code_wide: bool = False,
    reason: str | None = None,
    evidence: str | None = None,
    recorded_by: str = "agent",
) -> dict[str, Any]:
    """Record one accepted divergence, returning the written entry.

    Raises `BaselineError` rather than writing anything questionable: a code the
    latest run does not report, a milestone-shaped string that is not a milestone, or
    a code-wide acceptance without a stated reason.
    """
    project_root = Path(project_root)
    current = report(project_root)
    if code not in current.available_codes:
        raise BaselineError(
            f"the latest doctor run does not report {code!r}; "
            f"it reports {list(current.available_codes)}"
        )
    if milestone is not None and not _is_milestone(milestone):
        raise BaselineError(f"{milestone!r} is not a milestone id like M465")
    if milestone is None and not code_wide:
        raise BaselineError(f"accept {code!r} for a specific --milestone or say --code-wide")
    if code_wide and not reason:
        raise BaselineError("--code-wide suppresses the code everywhere; pass --reason")

    payload = load_baseline(project_root)
    accepted: list[dict[str, Any]] = payload["accepted"]
    scope = "code" if code_wide else "milestones"
    entry = next(
        (item for item in accepted if isinstance(item, dict) and item.get("code") == code and item.get("scope", "milestones") == scope),
        None,
    )
    if entry is None:
        entry = {"code": code, "scope": scope}
        if code_wide:
            entry["reason"] = reason
        else:
            entry["milestones"] = []
        accepted.append(entry)
    if not code_wide:
        milestones = entry.setdefault("milestones", [])
        if milestone not in milestones:
            milestones.append(milestone)
        entry["milestones"] = sorted(milestones)
    if evidence:
        entry["evidence"] = evidence
    entry["recorded_at"] = _today()
    entry["recorded_by"] = recorded_by

    payload.setdefault("schema_version", _SCHEMA_VERSION)
    _write(project_root, payload)
    return entry


def _baseline_path(project_root: Path) -> str:
    """The contract's baseline path, so this command and the flag read one file.

    Defaulting to `.gsd/doctor-baseline.json` here (as the reader does when no path is
    given) would silently compare against nothing while the flag compared against the
    real baseline — every accepted divergence would look unbaselined.
    """
    try:
        import yaml

        contract = yaml.safe_load(
            (project_root / "scripts/governor/checks/gsd_control_plane.yaml").read_text(
                encoding="utf-8"
            )
        )
        configured = contract.get("doctor_baseline_path") if isinstance(contract, dict) else None
        if isinstance(configured, str) and configured:
            return configured
    except (OSError, ValueError):
        pass
    return str(BASELINE_PATH)


def _write(project_root: Path, payload: dict[str, Any]) -> None:
    path = project_root / _baseline_path(project_root)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def _is_milestone(value: str) -> bool:
    if len(value) < 4 or not value.startswith("M"):
        return False
    rest = value[1:]
    number, _, suffix = rest.partition("-")
    return number.isdigit() and (not suffix or suffix.isalnum())


def _today() -> str:
    import time

    return time.strftime("%Y-%m-%d", time.gmtime())
