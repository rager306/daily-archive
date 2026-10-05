"""Closeout reclamation service — bound `target/` as part of finishing a slice.

The 2026-10 hygiene review measured `target/` at 54 GB (5.8 GB after pruning) and
found that `governor hygiene` was only ever run by hand: nothing invoked it at
slice closeout and the derived build tree had no enforced ceiling. This service is
that invocation. It reads the cap and the opt-out from the same DATA contract the
hygiene command already reads, reports a plan that names every reclamation with its
policy, and — only when explicitly applied — reclaims through BOTH mechanisms the
tree needs:

- the **cache GC** (`apply_plan`) releases derived store entries: the kache
  content-addressed compile cache and the age-windowed runtime stores;
- **per-package cleaning** (`cargo clean -p <crate>`) removes target artifacts for
  the workspace members. Only this frees the artifact space a second toolchain
  duplicates, because a new toolchain writes new fingerprint directories rather than
  replacing the old ones, and the kache store holds compile-cache entries rather than
  those artifacts.

Nothing is deleted without `apply`, and a slice may decline deliberately; the
decline is recorded. The default is the dry run the closeout path prints.
"""

from __future__ import annotations

import json
import subprocess
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, Mapping

import yaml

from governor.application.retention_service import (
    CONTRACT_PATH,
    RetentionPlan,
    apply_plan,
    build_plan,
    load_retention_policy,
)
from governor.checks.resource_safety import check_target_dir
from governor.domain.retention import (
    DEFAULT_TARGET_CAP_GB,
    RetentionPolicy,
)

# The workspace members whose target artifacts a second toolchain duplicates. Kept
# in DATA (`closeout.packages`) so the list is an edit to the contract, not code.
DEFAULT_PACKAGES: tuple[str, ...] = (
    "kg-ontology",
    "kg-storage",
    "kg-embeddings",
    "kg-algorithms",
    "kg-pipeline",
    "da-domain",
    "da-ports",
    "da-application",
    "da-parse",
    "da-adapters",
    "da-cli",
    "da-mcp",
)


@dataclass(frozen=True)
class CloseoutPolicy:
    """The slice-close reclamation contract read from the governor DATA file.

    `hygiene_enabled: false` is the project-level opt-out; the per-slice opt-out is a
    recorded decline (`record_decline`), because a single slice declining must leave a
    record rather than silence. `per_package_clean` and `packages` are DATA so the
    packages whose artifacts are reclaimed are never hardcoded.
    """

    hygiene_enabled: bool = True
    per_package_clean: bool = True
    packages: tuple[str, ...] = DEFAULT_PACKAGES
    opt_out_reason: str = ""

    @classmethod
    def from_mapping(cls, mapping: object) -> "CloseoutPolicy":
        if not isinstance(mapping, dict):
            return cls()

        def flag(key: str, fallback: bool) -> bool:
            value = mapping.get(key, fallback)
            return value if isinstance(value, bool) else fallback

        packages = mapping.get("packages")
        if isinstance(packages, list) and packages and all(
            isinstance(item, str) and item for item in packages
        ):
            resolved: tuple[str, ...] = tuple(packages)
        else:
            resolved = DEFAULT_PACKAGES

        reason = mapping.get("opt_out_reason")
        return cls(
            hygiene_enabled=flag("hygiene_enabled", True),
            per_package_clean=flag("per_package_clean", True),
            packages=resolved,
            opt_out_reason=reason if isinstance(reason, str) else "",
        )


def load_closeout_policy(project_root: Path) -> CloseoutPolicy:
    """Load the closeout contract from the same DATA file the hygiene command reads."""
    try:
        contract = yaml.safe_load((Path(project_root) / CONTRACT_PATH).read_text(encoding="utf-8"))
    except (OSError, yaml.YAMLError):
        return CloseoutPolicy()
    if not isinstance(contract, dict):
        return CloseoutPolicy()
    return CloseoutPolicy.from_mapping(contract.get("closeout"))


@dataclass
class CloseoutReport:
    """A closeout plan: what would be reclaimed, under which policy, and why."""

    policy: CloseoutPolicy
    retention: RetentionPolicy
    target_gb: float
    target_cap_gb: int
    over_cap: bool
    retention_plan: RetentionPlan
    package_commands: list[list[str]] = field(default_factory=list)
    declined: bool = False
    decline_reason: str | None = None

    def category_policy(self) -> list[tuple[str, int, str]]:
        """Every planned reclamation paired with the policy that selected it."""
        counts = self.retention_plan.counts()
        floor = self.retention.min_age_hours
        return [
            ("scratch_dirs", counts["scratch_dirs"], f"older than {floor}h"),
            (
                "exec_captures",
                counts["exec_captures"],
                f"older than {self.retention.days_exec}d (floor {floor}h)",
            ),
            (
                "quarantine_runs",
                counts["quarantine_runs"],
                f"older than {self.retention.days_quarantine}d (floor {floor}h)",
            ),
            (
                "activity_files",
                counts["activity_files"],
                f"older than {self.retention.days_activity}d (floor {floor}h)",
            ),
            (
                "build_caches",
                counts["build_caches"],
                f"cache GC for store entries >= {self.retention.build_cache_gb}GB",
            ),
            (
                "package_targets",
                len(self.package_commands),
                "cargo clean -p per DATA package (frees second-toolchain artifacts)",
            ),
        ]

    def reclaimed_bytes(self) -> int:
        return self.retention_plan.total_bytes()


def build_closeout_report(
    project_root: Path,
    *,
    retention_policy: RetentionPolicy | None = None,
    closeout_policy: CloseoutPolicy | None = None,
    tmp_dir: Path = Path("/tmp"),
    measure_target: Callable[[Path], Mapping[str, object]] | None = None,
    decline_reason: str | None = None,
) -> CloseoutReport:
    """Compute the closeout plan. Performs no deletion.

    `measure_target` is injectable so the cap can be tested against a stated size
    rather than a machine-specific `target/`; the production default is the same
    inode-aware measurement the governor already uses.
    """
    project_root = Path(project_root)
    retention = retention_policy or load_retention_policy(project_root)
    closeout = closeout_policy or load_closeout_policy(project_root)

    if measure_target is None:
        target: Mapping[str, object] = check_target_dir(str(project_root))
    else:
        target = measure_target(project_root)
    try:
        target_gb = float(target.get("size_gb", 0.0))  # type: ignore[arg-type]
    except (TypeError, ValueError):
        target_gb = 0.0

    plan = build_plan(project_root, retention, tmp_dir=tmp_dir)
    commands = (
        [["cargo", "clean", "-p", package] for package in closeout.packages]
        if closeout.per_package_clean
        else []
    )

    return CloseoutReport(
        policy=closeout,
        retention=retention,
        target_gb=round(target_gb, 2),
        target_cap_gb=retention.target_cap_gb,
        over_cap=target_gb > retention.target_cap_gb,
        retention_plan=plan,
        package_commands=commands,
        declined=decline_reason is not None,
        decline_reason=decline_reason,
    )


def apply_closeout(
    report: CloseoutReport,
    project_root: Path,
    *,
    runner: Callable[..., object] | None = None,
) -> Path:
    """Reclaim through BOTH mechanisms and record a closeout manifest.

    The cache GC releases derived store entries; per-package cleaning removes target
    artifacts. A declined report must never reach this function — a decline that
    deleted anything would not be an opt-out.
    """
    if report.declined:
        raise ValueError("a declined closeout must not reclaim")

    project_root = Path(project_root)
    run = runner or subprocess.run

    retention_manifest = apply_plan(report.retention_plan, project_root)

    package_results: list[dict[str, object]] = []
    for command in report.package_commands:
        result = run(command, cwd=str(project_root), capture_output=True, text=True)
        package_results.append(
            {
                "command": " ".join(command),
                "returncode": getattr(result, "returncode", None),
            }
        )

    manifests = project_root / ".gsd" / "retention"
    manifests.mkdir(parents=True, exist_ok=True)
    path = manifests / f"closeout-{_timestamp_compact()}.json"
    path.write_text(
        json.dumps(
            {
                "recorded_at": _timestamp(),
                "target_gb": report.target_gb,
                "target_cap_gb": report.target_cap_gb,
                "over_cap": report.over_cap,
                "reclaimed_bytes": report.reclaimed_bytes(),
                "retention_manifest": str(retention_manifest),
                "package_clean": package_results,
                "mechanisms": ["cache_gc", "per_package_clean"],
                "note": (
                    "Closeout reclamation used both mechanisms: the cache GC releases "
                    "derived store entries, while per-package cleaning removes target "
                    "artifacts — only the latter frees what a second toolchain duplicates."
                ),
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    return path


def record_decline(
    project_root: Path,
    reason: str,
    *,
    milestone: str | None = None,
    slice_id: str | None = None,
) -> Path:
    """Record a deliberate per-slice opt-out. Reclaims nothing."""
    record = {
        "recorded_at": _timestamp(),
        "kind": "closeout_reclamation_decline",
        "reason": reason,
        "milestone": milestone,
        "slice": slice_id,
    }
    manifests = Path(project_root) / ".gsd" / "retention"
    manifests.mkdir(parents=True, exist_ok=True)
    path = manifests / f"closeout-decline-{_timestamp_compact()}.json"
    path.write_text(json.dumps(record, indent=2), encoding="utf-8")
    return path


def _timestamp() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def _timestamp_compact() -> str:
    return time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
