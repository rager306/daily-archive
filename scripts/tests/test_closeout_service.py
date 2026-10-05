"""Closeout reclamation: the slice-close path that bounds target/ under DATA policy.

What must hold:

- the cap is DATA, not a constant: changing the contract changes whether the report
  is over cap;
- the default is a dry run that deletes nothing;
- apply uses BOTH mechanisms — the cache GC (store entries) and per-package cleaning
  (target artifacts);
- a deliberate decline is recorded and reclaims nothing.
"""

import json
import os
import time
from pathlib import Path

from governor.application.closeout_service import (
    DEFAULT_PACKAGES,
    apply_closeout,
    build_closeout_report,
    load_closeout_policy,
    record_decline,
)
from governor.application.retention_service import load_retention_policy

CONTRACT = "scripts/governor/checks/gsd_control_plane.yaml"


def _age(path: Path, hours: float) -> None:
    when = time.time() - hours * 3600
    os.utime(path, (when, when))


def _contract(
    root: Path,
    *,
    target_cap_gb: int = 60,
    hygiene_enabled: bool = True,
    per_package_clean: bool = True,
    packages: list[str] | None = None,
) -> None:
    checks = root / "scripts" / "governor" / "checks"
    checks.mkdir(parents=True, exist_ok=True)
    packages = packages if packages is not None else ["da-cli", "da-parse"]
    body = (
        "retention:\n"
        "  days_exec: 30\n"
        "  days_quarantine: 30\n"
        "  days_activity: 14\n"
        "  min_age_hours: 24\n"
        "  build_cache_gb: 4\n"
        f"  target_cap_gb: {target_cap_gb}\n"
        "closeout:\n"
        f"  hygiene_enabled: {str(hygiene_enabled).lower()}\n"
        f"  per_package_clean: {str(per_package_clean).lower()}\n"
        '  opt_out_reason: ""\n'
        "  packages:\n" + "".join(f"    - {package}\n" for package in packages)
    )
    (checks / "gsd_control_plane.yaml").write_text(body, encoding="utf-8")


def test_cap_is_read_from_data_not_hardcoded(tmp_path: Path) -> None:
    """The same measured size flips verdict when only the contract cap changes."""
    measured = lambda _root: {"size_gb": 100.0}  # noqa: E731 - tiny test double

    _contract(tmp_path, target_cap_gb=60)
    over = build_closeout_report(tmp_path, measure_target=measured)
    assert over.target_cap_gb == 60
    assert over.over_cap is True

    _contract(tmp_path, target_cap_gb=200)
    under = build_closeout_report(tmp_path, measure_target=measured)
    assert under.target_cap_gb == 200
    assert under.over_cap is False


def test_retention_policy_reads_the_cap_from_the_same_contract(tmp_path: Path) -> None:
    _contract(tmp_path, target_cap_gb=7)
    assert load_retention_policy(tmp_path).target_cap_gb == 7


def test_closeout_policy_reads_opt_out_and_packages(tmp_path: Path) -> None:
    _contract(tmp_path, hygiene_enabled=False, per_package_clean=False, packages=["da-cli"])
    policy = load_closeout_policy(tmp_path)
    assert policy.hygiene_enabled is False
    assert policy.per_package_clean is False
    assert policy.packages == ("da-cli",)


def test_closeout_policy_falls_back_when_absent_or_junk(tmp_path: Path) -> None:
    assert load_closeout_policy(tmp_path).hygiene_enabled is True

    checks = tmp_path / "scripts" / "governor" / "checks"
    checks.mkdir(parents=True)
    (checks / "gsd_control_plane.yaml").write_text(
        "closeout:\n  hygiene_enabled: maybe\n  packages: not-a-list\n", encoding="utf-8"
    )
    policy = load_closeout_policy(tmp_path)
    assert policy.hygiene_enabled is True
    assert policy.packages == DEFAULT_PACKAGES


def test_dry_run_plan_deletes_nothing(tmp_path: Path) -> None:
    _contract(tmp_path)
    stale = tmp_path / "tmp" / "governor-test-stale"
    stale.mkdir(parents=True)
    _age(stale, 48)

    report = build_closeout_report(tmp_path, tmp_dir=tmp_path / "tmp")

    # The plan names the scratch dir and the package cleans, but nothing changed.
    assert report.retention_plan.counts()["scratch_dirs"] == 1
    assert len(report.package_commands) == 2
    assert stale.exists()


def test_apply_uses_cache_gc_and_per_package_clean(tmp_path: Path) -> None:
    _contract(tmp_path, target_cap_gb=0, packages=["da-cli", "da-parse"])
    stale = tmp_path / "tmp" / "governor-test-stale"
    stale.mkdir(parents=True)
    _age(stale, 48)

    report = build_closeout_report(
        tmp_path,
        tmp_dir=tmp_path / "tmp",
        measure_target=lambda _root: {"size_gb": 100.0},
    )

    calls: list[tuple[list[str], str | None]] = []

    def fake_runner(command, **kwargs):
        calls.append((command, kwargs.get("cwd")))

        class _Result:
            returncode = 0

        return _Result()

    manifest_path = apply_closeout(report, tmp_path, runner=fake_runner)

    # Mechanism 1: the cache GC removed the stale store entry.
    assert not stale.exists()
    # Mechanism 2: per-package cleaning ran for every DATA package.
    assert calls == [
        (["cargo", "clean", "-p", "da-cli"], str(tmp_path)),
        (["cargo", "clean", "-p", "da-parse"], str(tmp_path)),
    ]
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["mechanisms"] == ["cache_gc", "per_package_clean"]
    commands = [entry["command"] for entry in manifest["package_clean"]]
    assert commands == ["cargo clean -p da-cli", "cargo clean -p da-parse"]


def test_decline_is_recorded_and_reclaims_nothing(tmp_path: Path) -> None:
    _contract(tmp_path)
    stale = tmp_path / "tmp" / "governor-test-stale"
    stale.mkdir(parents=True)
    _age(stale, 48)

    path = record_decline(
        tmp_path, "disk reserved for corpus", milestone="M516", slice_id="S01"
    )

    record = json.loads(path.read_text(encoding="utf-8"))
    assert record["kind"] == "closeout_reclamation_decline"
    assert record["reason"] == "disk reserved for corpus"
    assert record["milestone"] == "M516"
    assert record["slice"] == "S01"
    assert stale.exists()


def test_declined_report_refuses_apply(tmp_path: Path) -> None:
    _contract(tmp_path)
    report = build_closeout_report(tmp_path, tmp_dir=tmp_path / "tmp", decline_reason="declined")
    assert report.declined is True
    assert report.decline_reason == "declined"

    try:
        apply_closeout(report, tmp_path, runner=lambda *args, **kwargs: None)
    except ValueError as error:
        assert "declined" in str(error)
    else:
        raise AssertionError("a declined report must not reclaim")


def test_report_names_each_category_with_its_policy(tmp_path: Path) -> None:
    _contract(tmp_path)
    report = build_closeout_report(tmp_path, tmp_dir=tmp_path / "tmp")
    categories = report.category_policy()
    assert [name for name, _count, _policy in categories] == [
        "scratch_dirs",
        "exec_captures",
        "quarantine_runs",
        "activity_files",
        "build_caches",
        "package_targets",
    ]
    assert all(policy for _name, _count, policy in categories)
