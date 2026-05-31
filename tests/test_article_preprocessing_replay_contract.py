from __future__ import annotations

import importlib.util
import json
import sys
from argparse import Namespace
from pathlib import Path
from typing import Any

import pytest

MODULE_PATH = Path(__file__).parents[1] / "scripts" / "verify_m025_final_preprocessing_replay.py"
spec = importlib.util.spec_from_file_location("verify_m025_final_preprocessing_replay", MODULE_PATH)
assert spec is not None and spec.loader is not None
verify_m025_final_preprocessing_replay = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = verify_m025_final_preprocessing_replay
spec.loader.exec_module(verify_m025_final_preprocessing_replay)

FinalReplayError = verify_m025_final_preprocessing_replay.FinalReplayError
run_replay = verify_m025_final_preprocessing_replay.run_replay

FIXTURE_CONTRACT = Path(__file__).parent / "fixtures" / "article_preprocessing_replay_v00_01" / "contract.json"


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")


def _contract() -> dict[str, Any]:
    return json.loads(FIXTURE_CONTRACT.read_text(encoding="utf-8"))


def _args(tmp_path: Path, *, no_network: bool = True) -> Namespace:
    corpus_root = tmp_path / "corpus"
    catalog = tmp_path / "catalog.json"
    index = tmp_path / "index.json"
    selection = corpus_root / "selection.json"
    baseline = corpus_root / "baseline"
    final = corpus_root / "final-replay"
    write_events = corpus_root / "final-replay-events.jsonl"
    article_ref = "arxiv/cs-ai/2512.24601"
    slug = "arxiv-cs-ai-2512.24601"

    _write_json(catalog, {"schema_version": "article-catalog.v00.01"})
    _write_json(
        index,
        {
            "schema_version": "article-catalog-index.v00.01",
            "articles": [
                {
                    "article_ref": article_ref,
                    "article_key": "2512.24601",
                    "source_code": "arxiv",
                    "primary_source_role": "arxiv_html",
                    "article_path": "article_catalog/arxiv/cs-ai/2512.24601/article.json",
                    "title": "Recursive Language Models",
                }
            ],
        },
    )
    _write_json(
        selection,
        {
            "selection_id": "fixture-selection",
            "articles": [{"article_ref": article_ref, "source_code": "arxiv", "selection_role": "fixture"}],
            "safety_flags": {
                "graph_import_allowed": False,
                "production_ladybugdb_write_allowed": False,
                "trusted_kg_import_allowed": False,
                "production_import_attempted": False,
                "ladybugdb_written": False,
            },
        },
    )
    _write_json(
        corpus_root / "chunking" / slug / "chunks.json",
        {"chunks": [{"chunk_id": f"{article_ref}:chunk:0001", "chunk_type": "retrieval_context"}]},
    )
    for evidence_type in ("assets", "tables", "links", "identity"):
        _write_json(
            corpus_root / "evidence" / slug / f"{evidence_type}.json",
            {
                "article_ref": article_ref,
                "summary": {"item_count": 1, "diagnostic_count": 0},
                "safety_flags": {"metadata_only": True, "review_only": True},
            },
        )
    return Namespace(
        catalog=catalog,
        index=index,
        selection=selection,
        baseline=baseline,
        final=final,
        write_events=write_events,
        no_network=no_network,
    )


def test_contract_fixture_defines_final_replay_shape() -> None:
    contract = _contract()

    assert contract["schema_version"] == "m025-article-preprocessing-final-artifact.v00.01"
    assert set(contract["required_final_artifact_refs"]) == {"chunking", "assets", "tables", "links", "identity"}
    assert "baseline_missing" in contract["allowed_baseline_comparison_categories"]
    assert "production_import_attempted" in contract["required_false_safety_flags"]
    assert "ladybugdb_written" in contract["required_false_safety_flags"]


def test_final_replay_requires_no_network_execution(tmp_path: Path) -> None:
    args = _args(tmp_path, no_network=False)

    with pytest.raises(FinalReplayError, match="requires --no-network"):
        run_replay(args)


def test_final_replay_writes_contract_compliant_per_article_artifact(tmp_path: Path) -> None:
    args = _args(tmp_path)

    events = run_replay(args)

    artifact_path = args.final / "arxiv-cs-ai-2512.24601" / "final.json"
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    contract = _contract()

    for field in contract["required_top_level_fields"]:
        assert field in artifact
    assert artifact["schema_version"] == contract["schema_version"]
    assert set(artifact["final_artifact_refs"]) == set(contract["required_final_artifact_refs"])
    assert artifact["network"] == {"no_network_required": True, "network_fetch_attempted": False}
    assert artifact["baseline_comparison"]["category"] in contract["allowed_baseline_comparison_categories"]
    assert artifact["baseline_comparison"]["category"] == "baseline_missing"
    assert artifact["readiness"]["larger_validation_ready"] is False
    for flag in contract["required_false_safety_flags"]:
        assert artifact["safety_state"][flag] is False
    assert any(event["event_type"] == "final_replay.article_completed" for event in events)


def test_final_replay_rejects_missing_local_evidence_instead_of_fetching(tmp_path: Path) -> None:
    args = _args(tmp_path)
    (args.selection.parent / "evidence" / "arxiv-cs-ai-2512.24601" / "links.json").unlink()

    with pytest.raises(FinalReplayError, match="missing local evidence artifact"):
        run_replay(args)


def test_final_replay_summary_report_and_decision_are_blocked_without_baseline(tmp_path: Path) -> None:
    args = _args(tmp_path)
    args.events = args.write_events
    args.require_no_import_flags = True
    events = run_replay(args)
    summary = verify_m025_final_preprocessing_replay._summary_from_artifacts(args, events)

    verify_m025_final_preprocessing_replay._write_summary(args.selection.parent / "summary.json", summary)
    verify_m025_final_preprocessing_replay._write_report(args.selection.parent / "report.md", summary)
    verify_m025_final_preprocessing_replay._write_decision(args.selection.parent / "decision.json", summary)

    decision = json.loads((args.selection.parent / "decision.json").read_text(encoding="utf-8"))
    report = (args.selection.parent / "report.md").read_text(encoding="utf-8")
    assert summary["readiness"]["decision"] == "blocked"
    assert summary["readiness"]["graph_readiness_claim"] is False
    assert summary["no_network_proof"]["network_fetch_attempted"] is False
    assert summary["no_write_safety"]["safety_violations"] == []
    assert decision["larger_preprocessing_validation_ready"] is False
    assert decision["graph_readiness_claim"] is False
    assert "## Failure Modes" in report
    assert "## Load Profile" in report
    assert "## Negative Tests" in report


def test_require_no_import_flags_fails_on_safety_violation(tmp_path: Path) -> None:
    args = _args(tmp_path)
    events = run_replay(args)
    args.write_events.write_text("".join(json.dumps(event) + "\n" for event in events), encoding="utf-8")
    artifact_path = args.final / "arxiv-cs-ai-2512.24601" / "final.json"
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    artifact["safety_state"]["ladybugdb_written"] = True
    _write_json(artifact_path, artifact)

    exit_code = verify_m025_final_preprocessing_replay.main(
        [
            "--catalog",
            str(args.catalog),
            "--index",
            str(args.index),
            "--selection",
            str(args.selection),
            "--baseline",
            str(args.baseline),
            "--final",
            str(args.final),
            "--events",
            str(args.write_events),
            "--require-no-network",
            "--require-no-import-flags",
            "--write-summary",
            str(args.selection.parent / "summary.json"),
        ]
    )

    assert exit_code == 2
