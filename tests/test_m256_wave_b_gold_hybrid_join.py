"""M256 S01: M072 gold ↔ hybrid body join inventory."""

from __future__ import annotations

from pathlib import Path

import pytest

from research_graph.application.corpus.wave_b_gold_hybrid_join import (
    GoldHybridJoinPackage,
    inventory_m072_gold_hybrid_join,
    normalize_paper_id,
)


def test_normalize_paper_id() -> None:
    assert normalize_paper_id("arxiv:1206.6423") == "1206.6423"
    assert normalize_paper_id("1206.6423v2") == "1206.6423"
    assert normalize_paper_id("http://arxiv.org/abs/1409.0473") == "1409.0473"


def test_join_with_tmp_hybrid(tmp_path: Path) -> None:
    # minimal gold records
    gold = [
        {
            "case_id": "case:train:1206.6423",
            "paper_id": "arxiv:1206.6423",
            "entities": [],
            "relations": [],
        },
        {
            "case_id": "case:train:9999.9999",
            "paper_id": "arxiv:9999.9999",
            "entities": [],
            "relations": [],
        },
    ]
    body_root = tmp_path / "runs"
    body = body_root / "1206.6423" / "body" / "1206.6423.hybrid.body.md"
    body.parent.mkdir(parents=True)
    body.write_text("Language and Perception grounded attribute learning.", encoding="utf-8")

    pkg = inventory_m072_gold_hybrid_join(
        gold_records=gold,
        body_roots=(body_root,),
    )
    assert pkg.import_eligible is False
    assert pkg.dspy_optimizer_enabled is False
    assert pkg.gold_case_count == 2
    assert pkg.joined_count == 1
    assert pkg.missing_hybrid_count == 1
    assert pkg.joined[0]["paper_id"] == "1206.6423"
    assert pkg.joined[0]["case_id"] == "case:train:1206.6423"
    assert Path(pkg.joined[0]["body_path"]).is_file()
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["joined_count"] == 1


def test_rejects_import_true() -> None:
    with pytest.raises(ValueError, match="import"):
        GoldHybridJoinPackage(
            schema_version="m256-wave-b-gold-hybrid-join.v1",
            gold_case_count=0,
            hybrid_unique_count=0,
            joined_count=0,
            missing_hybrid_count=0,
            joined=(),
            missing_paper_ids=(),
            diagnostics=(),
            dspy_optimizer_enabled=False,
            import_eligible=True,
            graph_writes_allowed=False,
        )


def test_live_join_if_fixtures_present() -> None:
    fixtures = Path("artifacts/m072-reviewed-extraction-benchmark/fixtures/train-gold.jsonl")
    if not fixtures.is_file():
        return
    from research_graph.application.extraction_ablations import load_m072_split
    from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

    train_g, _ = load_m072_split("train")
    val_g, _ = load_m072_split("validation")
    pkg = inventory_m072_gold_hybrid_join(
        gold_records=train_g + val_g,
        body_roots=tuple(Path(r) for r in DEFAULT_BODY_ROOTS),
    )
    assert pkg.import_eligible is False
    assert pkg.joined_count >= 1
    # known live overlap is 6 when full hybrid fleet present
    assert pkg.joined_count <= pkg.gold_case_count
