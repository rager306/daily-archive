"""Tests for multi-root hybrid hygiene plan (M267)."""

from __future__ import annotations

import os
from pathlib import Path

from research_graph.application.corpus.multi_root_hygiene import (
    apply_multi_root_hardlinks,
    plan_multi_root_hygiene,
)


def test_plan_identical_hardlink_actions(tmp_path: Path) -> None:
    r1 = tmp_path / "runs-live-20"
    r2 = tmp_path / "runs-live-scholarly-20"
    for r in (r1, r2):
        body = r / "paperA" / "body"
        body.mkdir(parents=True)
        (body / "paperA.hybrid.body.md").write_text("same body\n", encoding="utf-8")
    plan = plan_multi_root_hygiene(
        [r1, r2],
        primary_root_order=("runs-live-20", "runs-live-scholarly-20"),
    )
    assert plan.import_eligible is False
    assert plan.multi_root_paper_id_count == 1
    assert plan.identical_content_count == 1
    assert plan.divergent_content_count == 0
    acts = [a.action for a in plan.actions]
    assert "keep_primary" in acts
    assert "hardlink_duplicate" in acts


def test_apply_hardlink_shares_inode(tmp_path: Path) -> None:
    r1 = tmp_path / "runs-live-20"
    r2 = tmp_path / "runs-live-scholarly-20"
    p1 = r1 / "paperB" / "body"
    p2 = r2 / "paperB" / "body"
    p1.mkdir(parents=True)
    p2.mkdir(parents=True)
    f1 = p1 / "paperB.hybrid.body.md"
    f2 = p2 / "paperB.hybrid.body.md"
    f1.write_text("identical\n", encoding="utf-8")
    f2.write_text("identical\n", encoding="utf-8")
    assert f1.stat().st_ino != f2.stat().st_ino
    plan = plan_multi_root_hygiene(
        [r1, r2],
        primary_root_order=("runs-live-20", "runs-live-scholarly-20"),
    )
    applied = apply_multi_root_hardlinks(plan, apply_hardlinks=True)
    assert applied.applied_hardlinks >= 1
    assert f1.stat().st_ino == f2.stat().st_ino
    assert f2.read_text(encoding="utf-8") == "identical\n"


def test_plan_only_no_mutation(tmp_path: Path) -> None:
    r1 = tmp_path / "a"
    r2 = tmp_path / "b"
    for r in (r1, r2):
        b = r / "x" / "body"
        b.mkdir(parents=True)
        (b / "x.hybrid.body.md").write_text("z\n", encoding="utf-8")
    plan = plan_multi_root_hygiene([r1, r2])
    applied = apply_multi_root_hardlinks(plan, apply_hardlinks=False)
    assert applied.applied_hardlinks == 0
