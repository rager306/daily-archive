"""Contracts for governor's read-only GSD planning projection boundary."""

import re
from pathlib import Path

from governor.adapters.filesystem_gsd_planning_reader import (
    FilesystemGsdPlanningReader,
)
from governor.application.trajectory_service import TrajectoryService
from governor.ports.gsd_planning_reader import (
    GsdPlanningProjection,
    MilestonePlanningProjection,
)


def _write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _roadmap(root: Path, lines: str) -> None:
    _write(root / ".gsd" / "ROADMAP.md", f"# Roadmap\n\n{lines}\n")


def _plan(root: Path, phase: str, number: int, task_ids: list[str]) -> None:
    tasks = "\n".join(f"- [ ] **{task_id}**: work" for task_id in task_ids)
    _write(
        root / ".gsd" / "phases" / phase / f"{number:03d}-01-PLAN.md",
        f"# S01\n\n## Tasks\n\n{tasks}\n",
    )


def test_reads_active_suffixed_milestone_and_counts_unique_tasks(tmp_path: Path) -> None:
    _roadmap(
        tmp_path,
        "- 🔄 **M371-pr59ty: Pipeline Architecture** (`depends:[—]`)\n"
        "- ✅ **M370-6gaxxc: Complete predecessor** (`depends:[—]`)",
    )
    _plan(
        tmp_path,
        "371-pr59ty-pipeline-architecture",
        371,
        ["T01", "T02", "T02"],
    )

    result = FilesystemGsdPlanningReader(tmp_path).read()

    assert result.available is True
    assert result.diagnostics == ()
    assert [(m.milestone_id, m.status, m.task_count) for m in result.milestones] == [
        ("M371-pr59ty", "active", 2),
        ("M370-6gaxxc", "complete", 0),
    ]
    assert result.active_empty_milestone_ids == ()


def test_reports_active_empty_milestone_without_reading_database(tmp_path: Path) -> None:
    _roadmap(tmp_path, "- 🔄 **M372-ab12cd: Empty skeleton** (`depends:[—]`)")

    result = FilesystemGsdPlanningReader(tmp_path).read()

    assert result.available is True
    assert result.active_empty_milestone_ids == ("M372-ab12cd",)
    assert "phase projection not found for M372-ab12cd" in result.diagnostics


def test_bare_only_does_not_absorb_suffixed_phase_tasks(tmp_path: Path) -> None:
    _roadmap(tmp_path, "- 🔄 **M371: Bare shadow** (`depends:[—]`)")
    _plan(tmp_path, "371-pr59ty-product", 371, ["T01", "T02"])

    result = FilesystemGsdPlanningReader(tmp_path).read()

    assert result.active_empty_milestone_ids == ("M371",)
    assert result.milestones[0].task_count == 0
    assert "refusing suffixed phase merge" in result.diagnostics[0]


def test_exact_suffixed_phase_dir_counts_tasks(tmp_path: Path) -> None:
    _roadmap(tmp_path, "- 🔄 **M371-pr59ty: Exact phase** (`depends:[—]`)")
    _plan(tmp_path, "371-pr59ty", 371, ["T01"])

    result = FilesystemGsdPlanningReader(tmp_path).read()

    assert result.diagnostics == ()
    assert result.milestones[0].task_count == 1


def test_duplicate_base_ids_do_not_merge_task_counts(tmp_path: Path) -> None:
    _roadmap(
        tmp_path,
        "- 🔄 **M371: Bare duplicate** (`depends:[—]`)\n"
        "- 🔄 **M371-pr59ty: Canonical suffixed** (`depends:[—]`)",
    )
    _plan(tmp_path, "371-pr59ty-canonical", 371, ["T01"])

    result = FilesystemGsdPlanningReader(tmp_path).read()

    assert result.current_milestone_id == "M371-pr59ty"
    assert result.active_empty_milestone_ids == ()
    assert [(m.milestone_id, m.task_count) for m in result.milestones] == [
        ("M371", 0),
        ("M371-pr59ty", 1),
    ]
    assert not any("refusing suffixed phase merge" in item for item in result.diagnostics)


def test_state_active_pending_roadmap_row_is_projection_active(tmp_path: Path) -> None:
    _roadmap(tmp_path, "- ⬜ **M398-cz4pzx: Executing** (`depends:[—]`)")
    _plan(tmp_path, "398-cz4pzx-executing", 398, ["T01"])
    _write(
        tmp_path / ".gsd" / "STATE.md",
        "# State\n\n**Active Milestone:** M398-cz4pzx: Executing\n",
    )

    result = FilesystemGsdPlanningReader(tmp_path).read()

    assert result.available is True
    assert result.diagnostics == ()
    assert result.milestones[0].status == "active"
    assert result.milestones[0].task_count == 1


def test_state_and_roadmap_active_divergence_is_visible(tmp_path: Path) -> None:
    _roadmap(
        tmp_path,
        "- 🔄 **M371-pr59ty: Current** (`depends:[—]`)\n"
        "- 🔄 **M388-mhds9q: Newer active** (`depends:[—]`)",
    )
    _plan(tmp_path, "371-pr59ty-current", 371, ["T01"])
    _plan(tmp_path, "388-mhds9q-long", 388, ["T01"])
    _write(
        tmp_path / ".gsd" / "STATE.md",
        "# State\n\n**Active Milestone:** M363-ma2kmw: stale\n",
    )

    result = FilesystemGsdPlanningReader(tmp_path).read()

    assert result.milestones[0].task_count == 1
    assert len(result.diagnostics) == 1
    diag = result.diagnostics[0]
    assert "STATE active=M363-ma2kmw" in diag
    assert "ROADMAP active=" in diag
    assert "M371-pr59ty" in diag and "M388-mhds9q" in diag
    assert "preferred_active=M388-mhds9q" in diag
    assert "do not hand-edit engine database files" in diag


def test_preferred_active_milestone_prefers_highest_number_then_suffix() -> None:
    pick = FilesystemGsdPlanningReader._preferred_active_milestone
    assert pick({"M010-aaa", "M388-mhds9q", "M383-uber46"}) == "M388-mhds9q"
    assert pick({"M100", "M100-suffixed"}) == "M100-suffixed"
    assert pick(set()) == "none"


def test_missing_or_malformed_roadmap_is_unavailable(tmp_path: Path) -> None:
    missing = FilesystemGsdPlanningReader(tmp_path).read()
    assert missing.available is False
    assert missing.milestones == ()
    assert missing.diagnostics == ("planning projection missing: .gsd/ROADMAP.md",)

    _roadmap(tmp_path, "- this is not a milestone projection")
    malformed = FilesystemGsdPlanningReader(tmp_path).read()
    assert malformed.available is False
    assert malformed.milestones == ()
    assert malformed.diagnostics == (
        "planning projection contains no recognized milestone rows: .gsd/ROADMAP.md",
    )


def test_stale_roadmap_active_empty_is_not_current_empty(tmp_path: Path) -> None:
    _roadmap(
        tmp_path,
        "- 🔄 **M446-rw9a2p: Current with tasks** (`depends:[—]`)\n"
        "- 🔄 **M447-ckhjhv: Stale empty** (`depends:[—]`)\n",
    )
    _plan(tmp_path, "446-rw9a2p-current", 446, ["T01"])
    _write(
        tmp_path / ".gsd" / "STATE.md",
        "# State\n\n**Active Milestone:** M446-rw9a2p: Current with tasks\n",
    )

    result = FilesystemGsdPlanningReader(tmp_path).read()

    assert result.current_milestone_id == "M446-rw9a2p"
    assert result.active_empty_milestone_ids == ()
    assert not any("M447-ckhjhv" in item for item in result.diagnostics)
    assert not any("has no PLAN tasks" in item for item in result.diagnostics)


def test_state_active_empty_still_flags(tmp_path: Path) -> None:
    _roadmap(
        tmp_path,
        "- 🔄 **M446-rw9a2p: Empty current** (`depends:[—]`)\n"
        "- 🔄 **M447-ckhjhv: Also empty** (`depends:[—]`)\n",
    )
    _write(
        tmp_path / ".gsd" / "STATE.md",
        "# State\n\n**Active Milestone:** M446-rw9a2p: Empty current\n",
    )

    result = FilesystemGsdPlanningReader(tmp_path).read()

    assert result.current_milestone_id == "M446-rw9a2p"
    assert result.active_empty_milestone_ids == ("M446-rw9a2p",)
    assert any("M446-rw9a2p" in item for item in result.diagnostics)
    assert not any("M447-ckhjhv" in item for item in result.diagnostics)


def test_governor_runtime_does_not_access_engine_database() -> None:
    runtime_files = sorted(Path("scripts/governor").rglob("*.py"))
    violations = []
    for path in runtime_files:
        source = path.read_text(encoding="utf-8")
        if "sqlite3" in source or "gsd.db" in source:
            violations.append(str(path))
    assert violations == []


class _FakePlanningReader:
    def __init__(self, projection: GsdPlanningProjection):
        self.projection = projection

    def read(self) -> GsdPlanningProjection:
        return self.projection


def test_trajectory_service_flags_empty_and_unavailable_projections() -> None:
    service = TrajectoryService.__new__(TrajectoryService)
    service.gsd_planning_reader = _FakePlanningReader(
        GsdPlanningProjection(
            available=True,
            milestones=(
                MilestonePlanningProjection(
                    milestone_id="M372-ab12cd",
                    title="Empty",
                    status="active",
                    task_count=0,
                    source=".gsd/ROADMAP.md",
                ),
            ),
            diagnostics=("phase projection not found for M372-ab12cd",),
        )
    )
    flags = service._check_gsd_planning_projection()
    assert [flag.flag for flag in flags] == [
        "gsd_planning_projection_degraded",
        "active_milestone_zero_projected_plan_tasks",
    ]
    assert all(flag.severity == "medium" for flag in flags)
    assert "M372-ab12cd" in flags[1].evidence

    service.gsd_planning_reader = _FakePlanningReader(
        GsdPlanningProjection(
            available=False,
            diagnostics=("planning projection missing: .gsd/ROADMAP.md",),
        )
    )
    flags = service._check_gsd_planning_projection()
    assert [flag.flag for flag in flags] == ["gsd_planning_projection_unavailable"]
    assert flags[0].severity == "high"


def test_unplanned_active_slice_is_diagnosed(tmp_path: Path) -> None:
    _roadmap(tmp_path, "- 🔄 **M383-uber46: Revision** (`depends:[—]`)")
    phase = tmp_path / ".gsd" / "phases" / "383-uber46-revision"
    _write(
        phase / "383-ROADMAP.md",
        "# M383\n\n- [x] **S01**: done\n- [ ] **S02**: Revision Aware Materialization\n",
    )
    _write(phase / "383-01-PLAN.md", "# S01\n\n- [x] **T01**: work\n")

    result = FilesystemGsdPlanningReader(tmp_path).read()
    assert any("slice S02 has no PLAN tasks" in item for item in result.diagnostics)


def test_repository_projection_tracks_state_active_milestone() -> None:
    """Live repo smoke: the projection promotes the exact STATE-active row."""
    root = Path(__file__).resolve().parents[2]
    state_text = (root / ".gsd" / "STATE.md").read_text(encoding="utf-8")
    match = re.search(
        r"^\*\*Active Milestone:\*\*\s+(M\d{3}(?:-[a-z0-9]+)?):",
        state_text,
        flags=re.MULTILINE,
    )
    assert match is not None
    active_id = match.group(1)

    result = FilesystemGsdPlanningReader(root).read()
    milestones = {item.milestone_id: item for item in result.milestones}

    assert result.available is True
    assert active_id in milestones
    assert milestones[active_id].status == "active"
