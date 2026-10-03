"""Retention service: plans are conservative, deletion is recorded, scratch is swept.

The service is the answer to an unbounded runtime store (exec 67k files, activity
99 MB, quarantine 86 MB, 28 849 scratch directories). What must hold:

- a dry run never deletes;
- the `min_age_hours` floor protects stores a live run is still writing;
- an exec capture is removed whole or not at all;
- every deletion writes a manifest, because exec captures are evidence.
"""

import json
import os
import time
from pathlib import Path

from governor.adapters.retention_store_reader import unretained_counts
from governor.application.retention_service import (
    build_plan,
    apply_plan,
    load_retention_policy,
    store_report,
)
from governor.domain.retention import RetentionPolicy


def _age(path: Path, hours: float) -> None:
    """Backdate a path so the retention window can be exercised."""
    when = time.time() - hours * 3600
    os.utime(path, (when, when))


def _capture(root: Path, capture_id: str, hours_old: float) -> Path:
    exec_dir = root / ".gsd" / "exec"
    exec_dir.mkdir(parents=True, exist_ok=True)
    paths = [
        exec_dir / f"{capture_id}.meta.json",
        exec_dir / f"{capture_id}.stdout",
        exec_dir / f"{capture_id}.stderr",
    ]
    paths[0].write_text(json.dumps({"id": capture_id}), encoding="utf-8")
    paths[1].write_text("out", encoding="utf-8")
    paths[2].write_text("", encoding="utf-8")
    for path in paths:
        _age(path, hours_old)
    return exec_dir


def test_old_scratch_directories_are_planned_for_removal(tmp_path: Path) -> None:
    tmp_dir = tmp_path / "tmp"
    stale = tmp_dir / "governor-test-stale"
    fresh = tmp_dir / "governor-test-fresh"
    unrelated = tmp_dir / "unrelated"
    for path in (stale, fresh, unrelated):
        path.mkdir(parents=True)
    _age(stale, 48)
    plan = build_plan(tmp_path, RetentionPolicy(min_age_hours=24), tmp_dir=tmp_dir)
    # Only old, prefixed scratch is a candidate: a live run keeps its own dirs, and
    # an unrelated /tmp entry is never swept.
    assert plan.scratch_dirs == [stale]


def test_min_age_floor_protects_recent_evidence_even_within_the_window(tmp_path: Path) -> None:
    exec_dir = _capture(tmp_path, "recent", hours_old=2)
    plan = build_plan(
        tmp_path,
        RetentionPolicy(days_exec=1, min_age_hours=24),
        tmp_dir=tmp_path / "tmp",
    )
    assert plan.exec_groups == []
    assert (exec_dir / "recent.meta.json").exists()


def test_exec_capture_is_removed_whole_once_old_enough(tmp_path: Path) -> None:
    exec_dir = _capture(tmp_path, "old", hours_old=24 * 40)
    plan = build_plan(tmp_path, RetentionPolicy(days_exec=30), tmp_dir=tmp_path / "tmp")
    assert plan.counts()["exec_captures"] == 1
    assert len(plan.exec_groups[0]) == 3
    apply_plan(plan, tmp_path)
    assert not (exec_dir / "old.meta.json").exists()
    assert not (exec_dir / "old.stdout").exists()
    assert not (exec_dir / "old.stderr").exists()


def test_partial_capture_is_left_alone(tmp_path: Path) -> None:
    exec_dir = _capture(tmp_path, "partial", hours_old=24 * 40)
    (exec_dir / "partial.stderr").unlink()
    plan = build_plan(tmp_path, RetentionPolicy(days_exec=30), tmp_dir=tmp_path / "tmp")
    # A half-written capture is never half-deleted.
    assert plan.exec_groups == []


def test_apply_writes_a_manifest_of_what_left(tmp_path: Path) -> None:
    tmp_dir = tmp_path / "tmp"
    stale = tmp_dir / "governor-test-stale"
    stale.mkdir(parents=True)
    (stale / "payload.bin").write_bytes(b"x" * 1024)
    _age(stale, 48)
    plan = build_plan(tmp_path, RetentionPolicy(min_age_hours=24), tmp_dir=tmp_dir)
    manifest_path = apply_plan(plan, tmp_path)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["counts"]["scratch_dirs"] == 1
    assert manifest["bytes"] >= 1024
    assert str(stale) in manifest["removed"]["scratch_dirs"]
    assert not stale.exists()


def test_dry_run_plan_does_not_delete(tmp_path: Path) -> None:
    tmp_dir = tmp_path / "tmp"
    stale = tmp_dir / "governor-test-stale"
    stale.mkdir(parents=True)
    _age(stale, 48)
    plan = build_plan(tmp_path, RetentionPolicy(min_age_hours=24), tmp_dir=tmp_dir)
    assert plan.counts()["scratch_dirs"] == 1
    # build_plan is the dry run: nothing may change on disk.
    assert stale.exists()


def test_no_scratch_option_keeps_scratch_even_when_stale(tmp_path: Path) -> None:
    tmp_dir = tmp_path / "tmp"
    stale = tmp_dir / "governor-test-stale"
    stale.mkdir(parents=True)
    _age(stale, 48)
    plan = build_plan(
        tmp_path, RetentionPolicy(min_age_hours=24), tmp_dir=tmp_dir, include_scratch=False
    )
    assert plan.scratch_dirs == []


def test_activity_and_quarantine_respect_their_own_windows(tmp_path: Path) -> None:
    activity = tmp_path / ".gsd" / "activity"
    activity.mkdir(parents=True)
    old = activity / "old.jsonl"
    old.write_text("{}", encoding="utf-8")
    _age(old, 24 * 20)

    quarantine = tmp_path / ".gsd" / "quarantine" / "projections"
    old_run = quarantine / "2026-08-01T00-00-00-000Z"
    old_run.mkdir(parents=True)
    _age(old_run, 24 * 40)

    plan = build_plan(tmp_path, tmp_dir=tmp_path / "tmp")
    assert plan.activity_files == [old]
    assert plan.quarantine_dirs == [old_run]


def test_store_report_measures_known_stores(tmp_path: Path) -> None:
    exec_dir = tmp_path / ".gsd" / "exec"
    exec_dir.mkdir(parents=True)
    (exec_dir / "a.meta.json").write_text("{}", encoding="utf-8")
    sizes = store_report(tmp_path, tmp_path / "tmp")
    assert sizes["exec"]["files"] == 1
    assert "tmp_scratch" in sizes


def test_unretained_counts_measure_only_what_hygiene_would_remove(tmp_path: Path) -> None:
    # The signal a healthy tree can clear: entries past the window that are still
    # present. Total store size is deliberately not the measure — it only grows.
    _capture(tmp_path, "young", hours_old=2)
    _capture(tmp_path, "old", hours_old=24 * 40)
    quarantine = tmp_path / ".gsd" / "quarantine" / "projections"
    old_run = quarantine / "old-run"
    old_run.mkdir(parents=True)
    _age(old_run, 24 * 40)
    young_run = quarantine / "young-run"
    young_run.mkdir(parents=True)

    counts = unretained_counts(tmp_path, RetentionPolicy(days_exec=30, days_quarantine=30))
    assert counts == {"unretained_exec_captures": 1, "unretained_quarantine_runs": 1}

    # Hygiene clears exactly those, and the count goes to zero.
    apply_plan(build_plan(tmp_path, RetentionPolicy(days_exec=30, days_quarantine=30)), tmp_path)
    assert unretained_counts(
        tmp_path, RetentionPolicy(days_exec=30, days_quarantine=30)
    ) == {"unretained_exec_captures": 0, "unretained_quarantine_runs": 0}


def test_retention_window_and_floor_combine_by_taking_the_longer() -> None:
    # Regression pin. Each value is an independent reason to keep an entry, so the
    # longer one must win. Combining them with `min` silently shortened a 30-day
    # window to 24 hours and scheduled 22k evidence captures for deletion.
    reference = 1_000_000_000.0
    wide = RetentionPolicy(days_exec=30, min_age_hours=24)
    assert (reference - wide.exec_cutoff_seconds(reference)) / 86400 == 30

    floor_bites = RetentionPolicy(days_exec=1, min_age_hours=48)
    assert (reference - floor_bites.exec_cutoff_seconds(reference)) / 3600 == 48

    # Scratch is pure garbage: only the floor applies.
    assert (reference - wide.scratch_cutoff_seconds(reference)) / 3600 == 24


def test_unretained_counts_ignore_entries_inside_the_window(tmp_path: Path) -> None:
    # The 10-day-old capture is inside a 30-day window and must not be scheduled for
    # removal, while the 40-day-old one must be.
    _capture(tmp_path, "inside-window", hours_old=24 * 10)
    _capture(tmp_path, "past-window", hours_old=24 * 40)
    policy = RetentionPolicy(days_exec=30, min_age_hours=24)
    assert unretained_counts(tmp_path, policy) == {
        "unretained_exec_captures": 1,
        "unretained_quarantine_runs": 0,
    }
    plan = build_plan(tmp_path, policy)
    assert len(plan.exec_groups) == 1
    assert plan.exec_groups[0][0].name == "past-window.meta.json"


def test_policy_is_read_from_the_contract(tmp_path: Path) -> None:
    contract = tmp_path / "scripts" / "governor" / "checks"
    contract.mkdir(parents=True)
    (contract / "gsd_control_plane.yaml").write_text(
        "retention:\n  days_exec: 5\n  days_quarantine: 6\n  days_activity: 7\n  min_age_hours: 8\n",
        encoding="utf-8",
    )
    policy = load_retention_policy(tmp_path)
    assert (policy.days_exec, policy.days_quarantine, policy.days_activity, policy.min_age_hours) == (
        5,
        6,
        7,
        8,
    )


def test_policy_falls_back_to_defaults_when_contract_is_absent_or_junk(tmp_path: Path) -> None:
    assert load_retention_policy(tmp_path).days_exec == 30
    contract = tmp_path / "scripts" / "governor" / "checks"
    contract.mkdir(parents=True)
    (contract / "gsd_control_plane.yaml").write_text(
        "retention:\n  days_exec: not-a-number\n", encoding="utf-8"
    )
    assert load_retention_policy(tmp_path).days_exec == 30


def test_build_cache_is_planned_only_when_it_exceeds_the_policy_floor(
    tmp_path: Path,
) -> None:
    """target/ grows one dependency-graph copy per feature-set fingerprint.

    The kache store is derived, so pruning it costs rebuild time and never
    evidence, but a target/ that is merely warm must not be proposed for churn.
    """
    kache = tmp_path / "target" / "debug" / "incremental.kache-auto"
    kache.mkdir(parents=True)
    (kache / "blob.bin").write_bytes(b"c" * (5 * 1024 * 1024))

    below = build_plan(tmp_path, RetentionPolicy(build_cache_gb=50), tmp_dir=tmp_path / "tmp")
    assert below.build_cache_dirs == []

    # A 5 MB cache with a 1 GB floor stays untouched; a 0 GB floor means "any size".
    floor_above = build_plan(
        tmp_path, RetentionPolicy(build_cache_gb=1), tmp_dir=tmp_path / "tmp"
    )
    assert floor_above.build_cache_dirs == []

    above = build_plan(tmp_path, RetentionPolicy(build_cache_gb=0), tmp_dir=tmp_path / "tmp")
    assert above.build_cache_dirs == [kache]
    assert above.counts()["build_caches"] == 1
    assert above.total_bytes() > 0


def test_build_cache_prune_is_recorded_in_the_manifest(tmp_path: Path) -> None:
    """A prune that leaves no record cannot be audited; the cache is gone as named."""
    kache = tmp_path / "target" / "debug" / "incremental.kache-auto"
    kache.mkdir(parents=True)
    (kache / "blob.bin").write_bytes(b"c" * (5 * 1024 * 1024))

    plan = build_plan(tmp_path, RetentionPolicy(build_cache_gb=0), tmp_dir=tmp_path / "tmp")
    manifest_path = apply_plan(plan, tmp_path)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

    assert not kache.exists()
    assert manifest["counts"]["build_caches"] == 1
    assert str(kache) in manifest["removed"]["build_cache_dirs"]
    assert "regenerate" in manifest["note"]
