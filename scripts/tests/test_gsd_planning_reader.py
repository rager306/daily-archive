"""Contracts for governor's read-only GSD planning projection boundary."""

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

    assert result.active_empty_milestone_ids == ("M371",)
    assert [(m.milestone_id, m.task_count) for m in result.milestones] == [
        ("M371", 0),
        ("M371-pr59ty", 1),
    ]
    assert any("refusing suffixed phase merge" in item for item in result.diagnostics)


def test_state_and_roadmap_active_divergence_is_visible(tmp_path: Path) -> None:
    _roadmap(tmp_path, "- 🔄 **M371-pr59ty: Current** (`depends:[—]`)")
    _plan(tmp_path, "371-pr59ty-current", 371, ["T01"])
    _write(
        tmp_path / ".gsd" / "STATE.md",
        "# State\n\n**Active Milestone:** M363-ma2kmw: stale\n",
    )

    result = FilesystemGsdPlanningReader(tmp_path).read()

    assert result.milestones[0].task_count == 1
    assert result.diagnostics == (
        "planning projection divergence: STATE active=M363-ma2kmw, "
        "ROADMAP active=['M371-pr59ty']",
    )


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


def test_repository_projection_reports_m371_tasks() -> None:
    root = Path(__file__).resolve().parents[2]
    result = FilesystemGsdPlanningReader(root).read()
    milestones = {item.milestone_id: item for item in result.milestones}

    assert result.available is True
    assert milestones["M371-pr59ty"].status == "active"
    assert milestones["M371-pr59ty"].task_count > 0
    assert "M371-pr59ty" not in result.active_empty_milestone_ids
