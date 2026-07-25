"""M281 S02: join gold + hybrid bodies and score resolvability."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.canary_gold_hybrid_join import (
    evaluate_joined_canary_resolvability,
    index_hybrid_bodies,
    join_gold_with_hybrid_bodies,
)


def test_join_and_metric(tmp_path: Path) -> None:
    body_root = tmp_path / "runs" / "p1" / "body"
    body_root.mkdir(parents=True)
    body = (
        "Abstract. Language Games are a Task. "
        "Dialogue Policy is our Method for multi-agent learning.\n"
    )
    (body_root / "p1.hybrid.body.md").write_text(body, encoding="utf-8")
    gold = [
        {
            "case_id": "case:p1",
            "paper_id": "arxiv:p1",
            "entities": [
                {"id": "e1", "label": "Language Games", "type": "Task"},
                {"id": "e2", "label": "Dialogue Policy", "type": "Method"},
                {"id": "e3", "label": "Absent Label", "type": "Dataset"},
            ],
            "relations": [],
        }
    ]
    index = index_hybrid_bodies([tmp_path / "runs"])
    assert "p1" in index
    joined = join_gold_with_hybrid_bodies(gold, index)
    assert len(joined) == 1
    pkg = evaluate_joined_canary_resolvability(
        gold_rows=gold, body_roots=[tmp_path / "runs"], target_rate=0.95
    )
    assert pkg.import_eligible is False
    assert pkg.joined_count == 1
    assert pkg.entity_grounded == 2
    assert pkg.entity_total == 3
    rate = pkg.resolvability["resolvability_rate"]
    assert rate > 0.0
    assert pkg.resolvability["import_eligible"] is False
    # write artifact shape
    out = tmp_path / "metric.json"
    out.write_text(json.dumps(pkg.to_dict(), indent=2) + "\n", encoding="utf-8")
    assert out.is_file()
