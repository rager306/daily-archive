from __future__ import annotations

import importlib.util
import json
import sys
from argparse import Namespace
from pathlib import Path
from typing import Any

import pytest

MODULE_PATH = Path(__file__).parents[1] / "scripts" / "verify_m025_evidence_boundaries.py"
spec = importlib.util.spec_from_file_location("verify_m025_evidence_boundaries", MODULE_PATH)
assert spec is not None and spec.loader is not None
verify_m025_evidence_boundaries = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = verify_m025_evidence_boundaries
spec.loader.exec_module(verify_m025_evidence_boundaries)

EvidenceReplayError = verify_m025_evidence_boundaries.EvidenceReplayError
replay = verify_m025_evidence_boundaries.replay


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload), encoding="utf-8")


def _args(tmp_path: Path) -> Namespace:
    catalog = tmp_path / "catalog.json"
    index = tmp_path / "index.json"
    selection = tmp_path / "selection.json"
    chunks = tmp_path / "chunking"
    evidence = tmp_path / "evidence"
    write_events = tmp_path / "events.jsonl"
    _write_json(catalog, {"schema_version": "article-catalog.v00.01"})
    _write_json(
        index,
        {
            "articles": [
                {
                    "article_ref": "arxiv/cs-ai/2512.24601",
                    "article_key": "2512.24601",
                    "source_code": "arxiv",
                    "primary_source_role": "arxiv_html",
                    "article_path": "article_catalog/arxiv/cs-ai/2512.24601/article.json",
                    "sha256": "a" * 64,
                }
            ]
        },
    )
    _write_json(
        selection,
        {
            "selection_id": "fixture-selection",
            "articles": [
                {
                    "article_ref": "arxiv/cs-ai/2512.24601",
                    "source_code": "arxiv",
                    "selection_role": "fixture",
                }
            ],
        },
    )
    return Namespace(
        catalog=catalog,
        index=index,
        selection=selection,
        chunks=chunks,
        evidence=evidence,
        write_events=write_events,
    )


def test_replay_fails_clearly_when_s06_chunking_directory_is_absent(tmp_path: Path) -> None:
    args = _args(tmp_path)

    with pytest.raises(EvidenceReplayError, match="missing S06 chunking directory"):
        replay(args)


def test_replay_writes_separated_metadata_only_artifacts_from_chunk_manifest(
    tmp_path: Path,
) -> None:
    args = _args(tmp_path)
    chunk_manifest = args.chunks / "arxiv-cs-ai-2512.24601" / "chunks.json"
    _write_json(
        chunk_manifest,
        {
            "chunks": [
                {
                    "chunk_id": "arxiv/cs-ai/2512.24601:chunk:0001",
                    "chunk_type": "figure_caption_context",
                },
                {
                    "chunk_id": "arxiv/cs-ai/2512.24601:chunk:0002",
                    "chunk_type": "table_context",
                    "route": "table_extraction",
                },
                {
                    "chunk_id": "arxiv/cs-ai/2512.24601:chunk:0003",
                    "chunk_type": "citation_context",
                    "route": "citation_graph",
                },
                {
                    "chunk_id": "arxiv/cs-ai/2512.24601:chunk:0004",
                    "chunk_type": "retrieval_context",
                },
            ]
        },
    )

    events = replay(args)

    article_dir = args.evidence / "arxiv-cs-ai-2512.24601"
    assets = json.loads((article_dir / "assets.json").read_text(encoding="utf-8"))
    tables = json.loads((article_dir / "tables.json").read_text(encoding="utf-8"))
    links = json.loads((article_dir / "links.json").read_text(encoding="utf-8"))
    identity = json.loads((article_dir / "identity.json").read_text(encoding="utf-8"))

    assert assets["summary"]["item_count"] == 1
    assert tables["summary"]["item_count"] == 1
    assert links["summary"]["item_count"] == 1
    assert identity["summary"]["item_count"] == 1
    assert all(
        payload["safety_flags"]["metadata_only"] is True
        for payload in (assets, tables, links, identity)
    )
    assert all(
        payload["import_eligible_count"] == 0 for payload in (assets, tables, links, identity)
    )
    assert all(
        payload["promoted_to_fact_count"] == 0 for payload in (assets, tables, links, identity)
    )
    serialized = json.dumps(
        {"assets": assets, "tables": tables, "links": links, "identity": identity}
    )
    assert "chunk_text" not in serialized
    assert "base64" not in serialized
    assert any(event["event_type"] == "evidence.artifact_written" for event in events)


def test_empty_separated_evidence_is_diagnostic_not_silent(tmp_path: Path) -> None:
    args = _args(tmp_path)
    chunk_manifest = args.chunks / "arxiv-cs-ai-2512.24601" / "chunks.json"
    _write_json(chunk_manifest, {"chunks": [{"chunk_id": "c1", "chunk_type": "retrieval_context"}]})

    replay(args)

    assets = json.loads(
        (args.evidence / "arxiv-cs-ai-2512.24601" / "assets.json").read_text(encoding="utf-8")
    )
    assert assets["items"] == []
    assert assets["summary"]["diagnostic_count"] == 1
    assert assets["diagnostics"][0]["code"] == "EVIDENCE_TYPE_NOT_OBSERVED"


def _write_fixture_chunks(args: Namespace) -> None:
    chunk_manifest = args.chunks / "arxiv-cs-ai-2512.24601" / "chunks.json"
    _write_json(
        chunk_manifest,
        {
            "chunks": [
                {
                    "chunk_id": "arxiv/cs-ai/2512.24601:chunk:0001",
                    "chunk_type": "figure_caption_context",
                },
                {
                    "chunk_id": "arxiv/cs-ai/2512.24601:chunk:0002",
                    "chunk_type": "table_context",
                    "route": "table_extraction",
                },
                {
                    "chunk_id": "arxiv/cs-ai/2512.24601:chunk:0003",
                    "chunk_type": "citation_context",
                    "route": "citation_graph",
                },
            ]
        },
    )


def _write_events(path: Path, events: list[dict[str, Any]]) -> None:
    path.write_text(
        "".join(json.dumps(event, sort_keys=True) + "\n" for event in events), encoding="utf-8"
    )


def _validation_args(args: Namespace) -> Namespace:
    return Namespace(
        catalog=args.catalog,
        index=args.index,
        selection=args.selection,
        evidence=args.evidence,
        events=args.write_events,
        require_redaction=True,
        require_no_import_flags=True,
        write_summary=args.evidence.parent / "summary.json",
        write_report=args.evidence.parent / "report.md",
    )


def test_validate_evidence_writes_summary_and_report(tmp_path: Path) -> None:
    args = _args(tmp_path)
    _write_fixture_chunks(args)
    events = replay(args)
    _write_events(args.write_events, events)
    validation_args = _validation_args(args)

    summary = verify_m025_evidence_boundaries.validate_evidence(validation_args)
    verify_m025_evidence_boundaries._write_json(validation_args.write_summary, summary)
    verify_m025_evidence_boundaries._write_report(validation_args.write_report, summary)

    assert summary["validation_passed"] is True
    assert summary["evidence_counts"] == {"assets": 1, "tables": 1, "links": 1, "identity": 1}
    assert summary["provenance_coverage"]["items_with_provenance_checked"] == 4
    assert summary["redaction_checks"]["passed"] is True
    assert summary["safety_state"]["production_import_attempted"] is False
    assert "## Per-Article Counts" in validation_args.write_report.read_text(encoding="utf-8")
    assert "## No-Import / No-Write Safety State" in validation_args.write_report.read_text(
        encoding="utf-8"
    )


def test_validate_evidence_fails_on_import_flag_violation(tmp_path: Path) -> None:
    args = _args(tmp_path)
    _write_fixture_chunks(args)
    events = replay(args)
    _write_events(args.write_events, events)
    assets_path = args.evidence / "arxiv-cs-ai-2512.24601" / "assets.json"
    assets = json.loads(assets_path.read_text(encoding="utf-8"))
    assets["safety_flags"]["production_import_attempted"] = True
    _write_json(assets_path, assets)

    summary = verify_m025_evidence_boundaries.validate_evidence(_validation_args(args))

    assert summary["validation_passed"] is False
    assert any(finding["code"] == "SAFETY_FLAG_MISMATCH" for finding in summary["findings"])
