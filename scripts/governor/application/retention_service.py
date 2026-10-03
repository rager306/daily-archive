"""Retention service for the GSD runtime stores.

The 2026-09-20..27 log review measured `exec` at 67k files, `activity` at 99 MB,
`quarantine` at 86 MB, and 28 849 `governor-test-*` scratch directories in /tmp.
Nothing in the engine prunes any of it, so the stores only grow while `target/` is
already 34 GB.

Placement note: this module lives under `scripts/governor/` on purpose. Rule
`no_new_product_python_scripts` (D237) forbids new Python under `scripts/` except
the governance surface, which is what the governor is. The same logic written as
`scripts/gsd_hygiene.py` was correctly rejected by the reverse-ADR audit with 250
`high` violations before it was moved here.

Rules this service follows:

- **Dry run is the default.** Deletion needs an explicit `apply=True`.
- **Evidence is not scratch.** `.gsd/exec` captures can be cited by SUMMARY
  artifacts, so pruning writes a manifest of exactly what left, and only touches
  entries older than the retention window.
- **Never race a live job.** Anything modified within `min_age_hours` (default 24)
  is kept regardless of the window.
- **Scratch is scratch.** `/tmp/governor-test-*` directories are test leftovers and
  are removed whole.
"""

from __future__ import annotations

import json
import shutil
import time
from dataclasses import dataclass, field
from pathlib import Path

import yaml

from governor.domain.retention import (
    DEFAULT_DAYS_ACTIVITY,
    DEFAULT_DAYS_EXEC,
    DEFAULT_DAYS_QUARANTINE,
    DEFAULT_MIN_AGE_HOURS,
    RetentionPolicy,
)

SCRATCH_PREFIX = "governor-test-"
_STORE_DIRS = ("exec", "activity", "audit", "quarantine", "journal")
# One capture is three files; a group is removed only when every member is present,
# so a half-written capture is never left broken.
_EXEC_SUFFIXES = (".stdout", ".stderr")
# The retention policy is DATA in the governor contract so that the threshold which
# fires and the tool that clears it cannot drift apart.
CONTRACT_PATH = Path("scripts/governor/checks/gsd_control_plane.yaml")


def load_retention_policy(project_root: Path) -> RetentionPolicy:
    """Load the policy from the governor contract, falling back to the defaults."""
    try:
        contract = yaml.safe_load((Path(project_root) / CONTRACT_PATH).read_text(encoding="utf-8"))
    except (OSError, yaml.YAMLError):
        return RetentionPolicy()
    if not isinstance(contract, dict):
        return RetentionPolicy()
    return RetentionPolicy.from_mapping(contract.get("retention"))


@dataclass
class RetentionPlan:
    """What a run would remove, grouped for reporting and replay."""

    scratch_dirs: list[Path] = field(default_factory=list)
    exec_groups: list[list[Path]] = field(default_factory=list)
    quarantine_dirs: list[Path] = field(default_factory=list)
    activity_files: list[Path] = field(default_factory=list)
    build_cache_dirs: list[Path] = field(default_factory=list)

    def counts(self) -> dict[str, int]:
        return {
            "scratch_dirs": len(self.scratch_dirs),
            "exec_captures": len(self.exec_groups),
            "quarantine_runs": len(self.quarantine_dirs),
            "activity_files": len(self.activity_files),
            "build_caches": len(self.build_cache_dirs),
        }

    def total_bytes(self) -> int:
        total = sum(tree_bytes(path) for path in self.scratch_dirs + self.quarantine_dirs)
        total += sum(file_bytes(item) for group in self.exec_groups for item in group)
        total += sum(file_bytes(item) for item in self.activity_files)
        total += sum(tree_bytes(path) for path in self.build_cache_dirs)
        return total

    def is_empty(self) -> bool:
        return not (
            self.scratch_dirs
            or self.exec_groups
            or self.quarantine_dirs
            or self.activity_files
            or self.build_cache_dirs
        )


def build_plan(
    project_root: Path,
    policy: RetentionPolicy | None = None,
    *,
    tmp_dir: Path = Path("/tmp"),
    include_scratch: bool = True,
    include_build_cache: bool = True,
) -> RetentionPlan:
    """Compute what is old enough to remove. Performs no deletion."""
    policy = policy or RetentionPolicy()
    gsd = project_root / ".gsd"
    now = time.time()
    # Every cutoff comes from the policy, so the same numbers decide what the check
    # calls unretained and what this plan removes.
    plan = RetentionPlan()

    if include_scratch:
        scratch_cutoff = policy.scratch_cutoff_seconds(now)
        plan.scratch_dirs = [
            entry
            for entry in sorted(tmp_dir.glob(f"{SCRATCH_PREFIX}*"))
            if entry.is_dir() and _mtime(entry) < scratch_cutoff
        ]

    plan.exec_groups = _stale_exec_groups(gsd / "exec", policy.exec_cutoff_seconds(now))
    plan.quarantine_dirs = _stale_entries(
        gsd / "quarantine" / "projections",
        policy.quarantine_cutoff_seconds(now),
        want_dir=True,
    )
    plan.activity_files = _stale_entries(
        gsd / "activity", policy.activity_cutoff_seconds(now), want_dir=False
    )
    if include_build_cache:
        plan.build_cache_dirs = _build_cache_dirs(project_root, policy.build_cache_gb)
    return plan


def apply_plan(plan: RetentionPlan, project_root: Path) -> Path:
    """Remove the plan and record a manifest; returns the manifest path."""
    manifest = {
        "recorded_at": _timestamp(),
        "counts": plan.counts(),
        "bytes": plan.total_bytes(),
        "removed": {
            "scratch_dirs": [str(path) for path in plan.scratch_dirs[:500]],
            "exec_captures": [[str(item) for item in group] for group in plan.exec_groups[:500]],
            "quarantine_dirs": [str(path) for path in plan.quarantine_dirs[:500]],
            "activity_files": [str(path) for path in plan.activity_files[:500]],
            "build_cache_dirs": [str(path) for path in plan.build_cache_dirs[:500]],
        },
        "note": (
            "GSD runtime retention. Exec captures are evidence and may be cited by "
            "SUMMARY artifacts; this manifest is the record of what left. Build "
            "caches under target/ are derived and regenerate on the next build, so "
            "removing them costs rebuild time, never evidence."
        ),
    }
    for path in plan.scratch_dirs + plan.quarantine_dirs:
        shutil.rmtree(path, ignore_errors=True)
    for group in plan.exec_groups:
        for item in group:
            _unlink(item)
    for item in plan.activity_files:
        _unlink(item)
    for path in plan.build_cache_dirs:
        shutil.rmtree(path, ignore_errors=True)

    manifests = project_root / ".gsd" / "retention"
    manifests.mkdir(parents=True, exist_ok=True)
    path = manifests / f"prune-{time.strftime('%Y%m%dT%H%M%SZ', time.gmtime())}.json"
    path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return path


def store_report(project_root: Path, tmp_dir: Path = Path("/tmp")) -> dict[str, dict[str, float]]:
    """File counts and megabytes for every runtime store this service can influence."""
    gsd = project_root / ".gsd"
    sizes: dict[str, dict[str, float]] = {}
    for name in _STORE_DIRS:
        files, total = _measure(gsd / name)
        sizes[name] = {"files": float(files), "mb": round(total / 1e6, 1)}
    scratch = [entry for entry in tmp_dir.glob(f"{SCRATCH_PREFIX}*") if entry.is_dir()]
    sizes["tmp_scratch"] = {
        "files": float(len(scratch)),
        "mb": round(sum(tree_bytes(entry) for entry in scratch) / 1e6, 1),
    }
    return sizes


# ── helpers ────────────────────────────────────────────────────────────────


def _build_cache_dirs(project_root: Path, min_gb: int) -> list[Path]:
    """Derived build caches under `target/` that are large enough to be worth pruning.

    Cargo keeps one artifact set per distinct feature-set fingerprint, so target/
    accumulates a copy of the dependency graph for every combination the
    verification stack compiles (default, pdf-layout, pdf-classifier, both). None
    of that is evidence: the kache content-addressed store is a compile cache and
    regenerates on the next build. Pruning it costs rebuild time and nothing else.

    `min_gb` is the size below which a cache is left alone, so this never proposes
    churn on a target/ that is merely warm.
    """
    target_debug = project_root / "target" / "debug"
    if not target_debug.is_dir():
        return []
    floor = max(0, min_gb) * 1024**3
    candidates = []
    for entry in sorted(target_debug.glob("incremental.*")):
        if not entry.is_dir():
            continue
        if tree_bytes(entry) >= floor:
            candidates.append(entry)
    return candidates


def _stale_exec_groups(exec_dir: Path, cutoff: float) -> list[list[Path]]:
    """Exec captures older than `cutoff`, each as its complete file group."""
    if not exec_dir.is_dir():
        return []
    groups: list[list[Path]] = []
    for meta in sorted(exec_dir.glob("*.meta.json")):
        if _mtime(meta) >= cutoff:
            continue
        capture_id = meta.name.removesuffix(".meta.json")
        group = [meta] + [exec_dir / f"{capture_id}{suffix}" for suffix in _EXEC_SUFFIXES]
        if all(item.exists() for item in group):
            groups.append(group)
    return groups


def _stale_entries(directory: Path, cutoff: float, *, want_dir: bool) -> list[Path]:
    """Directory entries older than `cutoff`, filtered to files or directories."""
    if not directory.is_dir():
        return []
    return [
        entry
        for entry in sorted(directory.iterdir())
        if entry.is_dir() is want_dir and _mtime(entry) < cutoff
    ]


def _unlink(path: Path) -> None:
    try:
        path.unlink()
    except OSError:
        return


def _mtime(path: Path) -> float:
    try:
        return path.stat().st_mtime
    except OSError:
        return time.time()


def file_bytes(path: Path) -> int:
    try:
        return path.stat().st_size
    except OSError:
        return 0


def tree_bytes(path: Path) -> int:
    total = 0
    try:
        for entry in path.rglob("*"):
            if entry.is_file():
                total += file_bytes(entry)
    except OSError:
        return total
    return total


def _measure(path: Path) -> tuple[int, int]:
    if not path.is_dir():
        return 0, 0
    files = 0
    total = 0
    try:
        for entry in path.rglob("*"):
            if entry.is_file():
                files += 1
                total += file_bytes(entry)
    except OSError:
        pass
    return files, total


def _timestamp() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
