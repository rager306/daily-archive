from __future__ import annotations

import importlib.util
import json
import sys
from argparse import Namespace
from pathlib import Path
from typing import Any

import pytest

MODULE_PATH = Path(__file__).parents[1] / "scripts" / "verify_m025_boundary_replay_completion.py"
spec = importlib.util.spec_from_file_location("verify_m025_boundary_replay_completion", MODULE_PATH)
assert spec is not None and spec.loader is not None
verify_m025_boundary_replay_completion = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = verify_m025_boundary_replay_completion
spec.loader.exec_module(verify_m025_boundary_replay_completion)

BoundaryReplayError = verify_m025_boundary_replay_completion.BoundaryReplayError
run_replay = verify_m025_boundary_replay_completion.run_replay
summary_from_artifacts = verify_m025_boundary_replay_completion._summary_from_artifacts
write_report = verify_m025_boundary_replay_completion._write_report
main = verify_m025_boundary_replay_completion.main


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")


def _args(tmp_path: Path, *, no_network: bool = True) -> Namespace:
    corpus = tmp_path / "corpus"
    catalog = tmp_path / "catalog.json"
    index = tmp_path / "index.json"
    selection = corpus / "selection.json"
    source = corpus / "sources" / "article.md"
    chunking = corpus / "chunking" / "arxiv-cs-ai-2512.24601" / "chunks.json"
    evidence_dir = corpus / "evidence" / "arxiv-cs-ai-2512.24601"
    baseline = corpus / "baseline"
    boundary = corpus / "boundary-replay"
    write_events = corpus / "boundary-replay-events.jsonl"
    article_ref = "arxiv/cs-ai/2512.24601"

    source.parent.mkdir(parents=True, exist_ok=True)
    source.write_text(
        "# Recursive Language Models\n\nRoot body.\n\n## Method\n\nA local metadata-safe fixture.\n",
        encoding="utf-8",
    )
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
                    "primary_source_role": "local_markdown",
                    "full_text_path": str(source),
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
        chunking,
        {
            "chunks": [
                {"chunk_id": f"{article_ref}:chunk:0001", "chunk_type": "retrieval_context"},
                {"chunk_id": f"{article_ref}:chunk:0002", "chunk_type": "citation_context"},
            ],
            "diagnostics": [],
        },
    )
    for evidence_type in ("assets", "tables", "links", "identity"):
        _write_json(
            evidence_dir / f"{evidence_type}.json",
            {
                "article_ref": article_ref,
                "summary": {"item_count": 1, "diagnostic_count": 0},
                "safety_flags": {
                    "metadata_only": True,
                    "review_only": True,
                    "raw_payloads_included": False,
                    "production_import_attempted": False,
                    "ladybugdb_written": False,
                },
            },
        )
    _write_json(
        baseline / "arxiv-cs-ai-2512.24601" / "final.json",
        {
            "article_ref": article_ref,
            "metrics": {
                "parser_element_count": 2,
                "page_index_node_count": 2,
                "chunk_count": 2,
                "evidence_counts": {"assets": 1, "identity": 1, "links": 1, "tables": 1},
            },
        },
    )
    return Namespace(
        catalog=catalog,
        index=index,
        selection=selection,
        boundary=boundary,
        baseline=baseline,
        final_replay=None,
        write_events=write_events,
        events=None,
        no_network=no_network,
        require_no_network=True,
        require_no_import_flags=True,
        validate_only=False,
        write_summary=corpus / "boundary-replay-summary.json",
        write_report=corpus / "boundary-replay-report.md",
    )


def _write_events(path: Path, events: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(event, sort_keys=True) + "\n" for event in events), encoding="utf-8")


def test_boundary_replay_requires_no_network(tmp_path: Path) -> None:
    args = _args(tmp_path, no_network=False)

    with pytest.raises(BoundaryReplayError, match="requires --no-network"):
        run_replay(args)


def test_cli_accepts_task_plan_aliases_and_explicit_artifact_roots(tmp_path: Path) -> None:
    args = _args(tmp_path)

    exit_code = main(
        [
            "--catalog",
            str(args.catalog),
            "--index",
            str(args.index),
            "--selection",
            str(args.selection),
            "--baseline",
            str(args.baseline),
            "--chunking",
            str(args.selection.parent / "chunking"),
            "--evidence",
            str(args.selection.parent / "evidence"),
            "--boundary-replay",
            str(args.boundary),
            "--write-events",
            str(args.write_events),
            "--write-summary",
            str(args.write_summary),
            "--write-report",
            str(args.write_report),
            "--no-network",
            "--require-no-network",
            "--require-no-import-flags",
            "--require-redaction",
            "--expect-article-count",
            "1",
            "--reject-zero-chunk-without-diagnostic",
        ]
    )

    assert exit_code == 0
    assert (args.boundary / "arxiv-cs-ai-2512.24601" / "boundary.json").exists()
    summary = json.loads(args.write_summary.read_text(encoding="utf-8"))
    assert summary["article_count"] == 1
    assert summary["readiness"]["decision"] == "ready"


def test_boundary_replay_writes_metadata_safe_artifacts_summary_and_report(tmp_path: Path) -> None:
    args = _args(tmp_path)

    events = run_replay(args)
    _write_events(args.write_events, events)
    summary = summary_from_artifacts(args, events)
    _write_json(args.write_summary, summary)
    write_report(args.write_report, summary)

    artifact_path = args.boundary / "arxiv-cs-ai-2512.24601" / "boundary.json"
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    serialized = json.dumps(artifact)

    assert artifact["schema_version"] == "m025-boundary-replay-artifact.v00.01"
    assert artifact["boundary_status"] == {
        "loader": "loaded",
        "parser": "parsed",
        "page_index": "indexed",
        "chunking": "summarized",
        "evidence": "summarized",
        "baseline": "exact_match",
    }
    assert artifact["metrics"]["parser_element_count"] == 2
    assert artifact["metrics"]["page_index_node_count"] == 2
    assert artifact["metrics"]["chunk_count"] == 2
    assert artifact["loader"]["text_sha256"]
    assert "Root body" not in serialized
    assert "local metadata-safe fixture" not in serialized
    assert artifact["safety_state"]["production_import_attempted"] is False
    assert artifact["safety_state"]["ladybugdb_written"] is False
    assert artifact["readiness"]["graph_readiness_claim"] is False
    assert any(event["event_type"] == "boundary_replay.article_completed" for event in events)
    assert summary["validation_passed"] is True
    assert summary["readiness"]["decision"] == "ready"
    report = args.write_report.read_text(encoding="utf-8")
    assert "## Failure Modes" in report
    assert "## Load Profile" in report
    assert "## Negative Tests" in report


def test_boundary_replay_rejects_missing_local_evidence(tmp_path: Path) -> None:
    args = _args(tmp_path)
    (args.selection.parent / "evidence" / "arxiv-cs-ai-2512.24601" / "links.json").unlink()

    with pytest.raises(BoundaryReplayError, match="missing local evidence artifact"):
        run_replay(args)


def test_malformed_selection_fails_before_writing_ready_summary(tmp_path: Path) -> None:
    args = _args(tmp_path)
    _write_json(args.selection, {"selection_id": "broken", "articles": []})

    with pytest.raises(BoundaryReplayError, match="non-empty articles list"):
        run_replay(args)

    assert not args.boundary.exists()
    assert not args.write_summary.exists()


def test_missing_local_source_becomes_per_article_blocker_not_raw_payload(tmp_path: Path) -> None:
    args = _args(tmp_path)
    index = json.loads(args.index.read_text(encoding="utf-8"))
    index["articles"][0]["full_text_path"] = str(args.selection.parent / "sources" / "missing.md")
    _write_json(args.index, index)

    events = run_replay(args)
    summary = summary_from_artifacts(args, events)
    artifact = json.loads((args.boundary / "arxiv-cs-ai-2512.24601" / "boundary.json").read_text(encoding="utf-8"))

    assert artifact["boundary_status"]["loader"] == "blocked"
    assert artifact["boundary_status"]["parser"] == "blocked"
    assert any(diagnostic["code"] == "LOCAL_SOURCE_MISSING" for diagnostic in artifact["diagnostics"])
    assert "boundary_diagnostics" in summary["readiness"]["blockers"]
    assert summary["validation_passed"] is False
    assert "Recursive Language Models" not in json.dumps(artifact)


def test_validation_blocks_unsafe_safety_flags_redaction_graph_claim_and_zero_chunks(tmp_path: Path) -> None:
    args = _args(tmp_path)
    events = run_replay(args)
    artifact_path = args.boundary / "arxiv-cs-ai-2512.24601" / "boundary.json"
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    artifact["safety_state"]["production_import_attempted"] = True
    artifact["safety_state"]["graph_readiness_claim"] = True
    artifact["redaction_checks"]["raw_article_text_included"] = True
    artifact["unsafe_raw_text"] = "raw article text should never appear"
    artifact["metrics"]["chunk_count"] = 0
    artifact["diagnostics"] = [
        diagnostic for diagnostic in artifact["diagnostics"] if diagnostic.get("code") != "ZERO_CHUNKS_WITHOUT_DIAGNOSTIC"
    ]
    _write_json(artifact_path, artifact)

    summary = summary_from_artifacts(args, events)

    assert summary["validation_passed"] is False
    assert "safety_flag_violation" in summary["readiness"]["blockers"]
    assert "redaction_violation" in summary["readiness"]["blockers"]
    assert "zero_chunks_without_diagnostics" in summary["readiness"]["blockers"]
    assert summary["no_write_safety"]["safety_violations"]
    assert summary["redaction_checks"]["violations"][0]["json_path"] == "$.unsafe_raw_text"
    assert summary["zero_chunk_checks"]["violations"] == ["arxiv/cs-ai/2512.24601"]
