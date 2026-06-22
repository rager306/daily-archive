from __future__ import annotations

import importlib.util
import json
import sys
from argparse import Namespace
from pathlib import Path
from typing import Any

import pytest

MODULE_PATH = Path(__file__).parents[1] / "scripts" / "replay_m027_end_to_end_mixed_replay.py"
spec = importlib.util.spec_from_file_location("replay_m027_end_to_end_mixed_replay", MODULE_PATH)
assert spec is not None and spec.loader is not None
replay = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = replay
spec.loader.exec_module(replay)

VERIFY_MODULE_PATH = (
    Path(__file__).parents[1] / "scripts" / "verify_m027_end_to_end_mixed_replay.py"
)
verify_spec = importlib.util.spec_from_file_location(
    "verify_m027_end_to_end_mixed_replay", VERIFY_MODULE_PATH
)
assert verify_spec is not None and verify_spec.loader is not None
verify_replay = importlib.util.module_from_spec(verify_spec)
sys.modules[verify_spec.name] = verify_replay
verify_spec.loader.exec_module(verify_replay)

EndToEndReplayError = replay.EndToEndReplayError


def _sha(path: Path) -> str:
    return replay.sha256_file(path)


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _fixture(
    tmp_path: Path,
    *,
    parser_text: str | None = None,
    metadata_only: bool = True,
    baseline_delta: bool = False,
) -> Namespace:
    root = tmp_path
    corpus = root / "corpus"
    converted = corpus / "conversion-quality" / "article_one" / "arxiv_pdf.txt"
    text = (
        parser_text
        or "# Fixture Paper\n\n## Introduction\n\nThis fixture has enough local converted article prose for the end to end replay path.\n\n## Method\n\nThe accepted behavior should create retrieval-only chunks and refuse import readiness.\n"
    )
    converted.parent.mkdir(parents=True, exist_ok=True)
    converted.write_text(text, encoding="utf-8")
    source_summary = corpus / "source-acquisition-summary.json"
    _write_json(source_summary, {"schema_version": "source-fixture", "articles": ["article/one"]})
    conversion_summary = corpus / "conversion-quality-summary.json"
    results: list[dict[str, Any]] = [
        {
            "article_ref": "article/one",
            "variant_id": "one:source:pdf",
            "source_role": "arxiv_pdf",
            "status": "converted",
            "parser_ready": True,
            "source_sha256": "source-sha",
            "converted_text_path": str(converted.relative_to(root)),
            "converted_text_sha256": _sha(converted),
            "converted_text_byte_size": converted.stat().st_size,
        }
    ]
    if metadata_only:
        results.append(
            {
                "article_ref": "article/one",
                "variant_id": "one:source:abs",
                "source_role": "arxiv_abs_page",
                "status": "metadata_only",
                "parser_ready": False,
                "source_sha256": "metadata-source-sha",
                "converted_text_path": None,
                "converted_text_sha256": None,
                "converted_text_byte_size": 0,
            }
        )
    _write_json(
        conversion_summary,
        {
            "schema_version": "m027-conversion-quality.v1",
            "milestone_id": "M027-aakeky",
            "slice_id": "S03",
            "selection_id": "m027-mixed-source-corpus-v1",
            "source_summary_path": str(source_summary.relative_to(root)),
            "source_summary_sha256": _sha(source_summary),
            "results": results,
        },
    )
    s03_summary = root / ".gsd" / "milestones" / "M027-aakeky" / "slices" / "S03" / "S03-SUMMARY.md"
    s03_summary.parent.mkdir(parents=True, exist_ok=True)
    s03_summary.write_text("# S03 summary\n", encoding="utf-8")
    baseline_results: list[dict[str, Any]] = []
    for row in results:
        chunk_count = 0 if row["parser_ready"] is not True else (1 if baseline_delta else 2)
        baseline_results.append(
            {
                "article_ref": row["article_ref"],
                "variant_id": row["variant_id"],
                "parser_ready": row["parser_ready"] is True,
                "baseline_artifact_path": "corpus/current-pipeline-baseline/article_one/baseline.json",
                "current_pipeline_metrics": {
                    "chunk_count": chunk_count,
                    "import_ready": False,
                    "import_eligible_chunk_count": 0,
                },
            }
        )
    baseline_summary = corpus / "current-pipeline-baseline-summary.json"
    _write_json(
        baseline_summary,
        {
            "schema_version": "m027-current-pipeline-baseline.v1",
            "milestone_id": "M027-aakeky",
            "slice_id": "S04",
            "selection_id": "m027-mixed-source-corpus-v1",
            "article_results": baseline_results,
        },
    )
    baseline_diagnostics = corpus / "current-pipeline-baseline-diagnostics.jsonl"
    baseline_diagnostics.write_text('{"diagnostic_code":"fixture"}\n', encoding="utf-8")
    return Namespace(
        conversion_summary=conversion_summary,
        s03_summary=s03_summary,
        baseline_summary=baseline_summary,
        baseline_diagnostics=baseline_diagnostics,
        output_summary=corpus / "end-to-end-mixed-replay-summary.json",
        output_diagnostics=corpus / "end-to-end-mixed-replay-diagnostics.jsonl",
        output_events=corpus / "end-to-end-mixed-replay-events.jsonl",
        output_report=corpus / "end-to-end-mixed-replay-report.md",
        readiness_decision=corpus / "end-to-end-mixed-replay-readiness-decision.json",
        output_dir=corpus / "end-to-end-mixed-replay",
        no_network=True,
    )


def _write_replay_outputs(
    args: Namespace,
) -> tuple[dict[str, Any], list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    summary, diagnostics, events, decision = replay.replay_end_to_end(args)
    replay.write_json(args.output_summary, summary)
    replay.write_jsonl(args.output_diagnostics, diagnostics)
    replay.write_jsonl(args.output_events, events)
    replay.write_json(args.readiness_decision, decision)
    replay.write_report(args.output_report, summary, decision)
    summary = replay.finalize_output_provenance(args, summary)
    replay.write_json(args.output_summary, summary)
    return summary, diagnostics, events, decision


def _verify_args(args: Namespace, tmp_path: Path) -> Namespace:
    return Namespace(
        summary=args.output_summary,
        diagnostics=args.output_diagnostics,
        events=args.output_events,
        report=args.output_report,
        readiness_decision=args.readiness_decision,
        output_dir=args.output_dir,
        conversion_summary=args.conversion_summary,
        baseline_summary=args.baseline_summary,
        baseline_diagnostics=args.baseline_diagnostics,
        verification_output=tmp_path / "corpus" / "end-to-end-mixed-replay-verification.json",
        verification_report=tmp_path / "corpus" / "end-to-end-mixed-replay-verification-report.md",
        root=tmp_path,
        expected_article_count=1,
        expected_variant_count=2,
    )


def _verify_codes(args: Namespace, tmp_path: Path) -> set[str]:
    _, findings = verify_replay.verify(_verify_args(args, tmp_path))
    return _diagnostic_codes(findings)


def _mutate_json(path: Path, mutator: Any) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    mutator(payload)
    _write_json(path, payload)
    return payload


def _diagnostic_codes(findings: list[dict[str, Any]]) -> set[str]:
    return {str(finding.get("diagnostic_code")) for finding in findings}


def test_replay_requires_no_network_and_s03_linkage(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    args.no_network = False

    with pytest.raises(EndToEndReplayError, match="requires --no-network"):
        replay.replay_end_to_end(args)

    args.no_network = True
    (tmp_path / "corpus" / "source-acquisition-summary.json").write_text(
        '{"changed": true}\n', encoding="utf-8"
    )

    with pytest.raises(EndToEndReplayError, match="stale S03 linkage"):
        replay.replay_end_to_end(args)


def test_replay_rejects_converted_payload_hash_mismatch(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    converted = tmp_path / "corpus" / "conversion-quality" / "article_one" / "arxiv_pdf.txt"
    converted.write_text("tampered after summary", encoding="utf-8")

    with pytest.raises(EndToEndReplayError, match="converted_text_sha256 mismatch"):
        replay.replay_end_to_end(args)


def test_replay_captures_boundaries_and_baseline_comparison(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)

    summary, diagnostics, events, decision = _write_replay_outputs(args)

    assert summary["article_count"] == 1
    assert summary["variant_count"] == 2
    assert summary["parser_ready_variant_count"] == 1
    assert summary["metadata_only_variant_count"] == 1
    assert summary["chunk_count"] >= 1
    assert summary["evidence_path_count"] >= 1
    assert summary["import_ready_count"] == 0
    assert summary["import_eligible_chunk_count"] == 0
    assert summary["network_fetch_attempted"] is False
    assert summary["graph_import_allowed"] is False
    assert summary["ladybugdb_written"] is False
    assert summary["provenance"]["milestone_id"] == "M027-aakeky"
    assert summary["provenance"]["slice_id"] == "S05"
    assert {row["role"] for row in summary["input_artifacts"]} == {
        "s03_conversion_summary",
        "s04_baseline_summary",
        "s04_baseline_diagnostics",
    }
    assert {row["role"] for row in summary["output_artifacts"]} >= {
        "summary",
        "diagnostics",
        "events",
        "report",
        "readiness_decision",
        "per_article_replay",
    }
    codes = {row["diagnostic_code"] for row in diagnostics}
    assert "s03_linkage_verified" in codes
    assert "converted_payload_hash_verified" in codes
    assert "end_to_end_boundaries_completed" in codes
    assert "metadata_only_not_parser_ready_skipped" in codes
    assert "s04_baseline_exact_match" in codes
    assert events[0]["event_type"] == "replay_started"
    assert events[-1]["event_type"] == "replay_completed"
    assert decision["ready_for_import"] is False
    artifact_path = args.output_dir / "article_one" / "replay.json"
    assert artifact_path.exists()
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    assert artifact["variant_count"] == 2
    parser_ready = next(row for row in summary["article_results"] if row["parser_ready"])
    assert parser_ready["boundary_metrics"]["loader"]["status"] == "structured_markdown"
    assert parser_ready["boundary_metrics"]["parser"]["element_count"] >= 1
    assert parser_ready["boundary_metrics"]["page_index"]["node_count"] >= 1
    assert parser_ready["boundary_metrics"]["chunking"]["chunk_count"] >= 1
    assert parser_ready["boundary_metrics"]["evidence"]["evidence_path_count"] >= 1
    assert parser_ready["baseline_comparison"]["category"] == "exact_match"


def test_replay_records_s04_baseline_metric_delta(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path, metadata_only=False, baseline_delta=True)

    summary, diagnostics, _, _ = _write_replay_outputs(args)

    assert summary["baseline_comparison_counts"]["metric_delta"] == 1
    assert "s04_baseline_metric_delta" in _diagnostic_codes(diagnostics)


def test_replay_preserves_parser_ready_zero_chunk_diagnostic(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path, parser_text="   \n", metadata_only=False)
    baseline = json.loads(args.baseline_summary.read_text(encoding="utf-8"))
    baseline["article_results"][0]["current_pipeline_metrics"]["chunk_count"] = 0
    _write_json(args.baseline_summary, baseline)

    summary, diagnostics, _, decision = _write_replay_outputs(args)

    assert summary["parser_ready_variant_count"] == 1
    assert summary["chunk_count"] == 0
    assert summary["zero_chunk_parser_ready_variant_count"] == 1
    assert "parser_ready_zero_chunks_preserved" in _diagnostic_codes(diagnostics)
    assert "parser_ready_zero_chunk_variants_preserved" in decision["blockers"]


def test_replay_skips_metadata_only_without_payload(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)

    summary, diagnostics, _, _ = _write_replay_outputs(args)

    metadata_record = next(row for row in summary["article_results"] if not row["parser_ready"])
    assert metadata_record["converted_payload"] == {"verified": False, "reason": "not_parser_ready"}
    assert metadata_record["boundary_metrics"]["loader"]["status"] == "skipped_metadata_only"
    assert metadata_record["boundary_metrics"]["chunking"]["chunk_count"] == 0
    assert "metadata_only_no_converted_payload_expected" in _diagnostic_codes(diagnostics)
    assert "metadata_only_not_parser_ready_skipped" in _diagnostic_codes(diagnostics)


def test_replay_rejects_unsafe_output_dir(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    args.output_dir = tmp_path.parent / "escaped-replay-output"

    with pytest.raises(EndToEndReplayError, match="unsafe_output_dir"):
        replay.replay_end_to_end(args)


def test_metadata_outputs_are_redacted(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)

    summary, diagnostics, events, decision = _write_replay_outputs(args)

    combined = "\n".join(
        [
            json.dumps(summary, sort_keys=True),
            "\n".join(json.dumps(row, sort_keys=True) for row in diagnostics),
            "\n".join(json.dumps(row, sort_keys=True) for row in events),
            json.dumps(decision, sort_keys=True),
            args.output_summary.read_text(encoding="utf-8"),
            args.output_diagnostics.read_text(encoding="utf-8"),
            args.output_events.read_text(encoding="utf-8"),
            args.output_report.read_text(encoding="utf-8"),
            args.readiness_decision.read_text(encoding="utf-8"),
            (args.output_dir / "article_one" / "replay.json").read_text(encoding="utf-8"),
        ]
    )
    assert "This fixture has enough local converted article prose" not in combined
    assert "chunk_text" not in combined
    assert '"content"' not in combined
    assert '"raw_text"' not in combined
    assert "<html" not in combined.lower()
    assert "%PDF-" not in combined


def test_redaction_guard_rejects_payload_keys_and_snippets() -> None:
    with pytest.raises(EndToEndReplayError, match="metadata payload key leakage"):
        replay.assert_no_metadata_leakage({"raw_text": "redacted?"})
    with pytest.raises(EndToEndReplayError, match="metadata payload leakage"):
        replay.assert_no_metadata_leakage({"safe": "RAW_PDF_SECRET <html"})


def test_validate_only_verifier_accepts_generated_replay_artifacts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)

    verifier_summary, findings = verify_replay.verify(_verify_args(args, tmp_path))

    assert findings == []
    assert verifier_summary["status"] == "passed"
    assert verifier_summary["network_fetch_attempted"] is False
    assert verifier_summary["production_import_attempted"] is False
    assert verifier_summary["ladybugdb_written"] is False


def test_validate_only_verifier_writes_closeout_artifacts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)
    verification_output = tmp_path / "corpus" / "end-to-end-mixed-replay-verification.json"
    verification_report = tmp_path / "corpus" / "end-to-end-mixed-replay-verification-report.md"

    exit_code = verify_replay.main(
        [
            "verify_m027_end_to_end_mixed_replay.py",
            "--summary",
            str(args.output_summary),
            "--diagnostics",
            str(args.output_diagnostics),
            "--events",
            str(args.output_events),
            "--report",
            str(args.output_report),
            "--readiness-decision",
            str(args.readiness_decision),
            "--output-dir",
            str(args.output_dir),
            "--conversion-summary",
            str(args.conversion_summary),
            "--baseline-summary",
            str(args.baseline_summary),
            "--baseline-diagnostics",
            str(args.baseline_diagnostics),
            "--verification-output",
            str(verification_output),
            "--verification-report",
            str(verification_report),
            "--root",
            str(tmp_path),
            "--expected-article-count",
            "1",
            "--expected-variant-count",
            "2",
        ]
    )

    assert exit_code == 0
    payload = json.loads(verification_output.read_text(encoding="utf-8"))
    report = verification_report.read_text(encoding="utf-8")
    assert payload["status"] == "passed"
    assert payload["diagnostic_count"] == 0
    assert payload["exit_status"] == 0
    assert "# M027 S05 End to End Mixed Replay Verification" in report
    combined = json.dumps(payload, sort_keys=True) + report
    assert "This fixture has enough local converted article prose" not in combined
    assert "RAW_PDF_SECRET" not in combined


def test_validate_only_verifier_rejects_missing_per_article_artifact(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)
    (args.output_dir / "article_one" / "replay.json").unlink()

    assert "missing_per_article_replay_artifact" in _verify_codes(args, tmp_path)


def test_validate_only_verifier_rejects_stale_s03_and_s04_hashes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)
    (tmp_path / "corpus" / "source-acquisition-summary.json").write_text(
        '{"changed": true}\n', encoding="utf-8"
    )
    _mutate_json(
        args.baseline_summary, lambda payload: payload.update({"changed_after_replay": True})
    )

    codes = _verify_codes(args, tmp_path)

    assert "s03_source_summary_sha256_mismatch" in codes
    assert "baseline_summary_sha256_mismatch" in codes


def test_validate_only_verifier_rejects_unsafe_true_flags_and_payload_leakage(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)
    _mutate_json(
        args.output_summary, lambda payload: payload.update({"network_fetch_attempted": True})
    )
    _mutate_json(
        args.output_dir / "article_one" / "replay.json",
        lambda payload: payload.update({"safe_note": "RAW_PDF_SECRET <html"}),
    )

    codes = _verify_codes(args, tmp_path)

    assert "unsafe_safety_flag_true" in codes
    assert "metadata_payload_snippet_leakage" in codes


def test_validate_only_verifier_rejects_missing_baseline_rows(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)
    _mutate_json(args.baseline_summary, lambda payload: payload.update({"article_results": []}))

    assert "s04_baseline_row_missing" in _verify_codes(args, tmp_path)


def test_validate_only_verifier_rejects_malformed_diagnostics(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)
    with args.output_diagnostics.open("a", encoding="utf-8") as handle:
        handle.write("{not-json}\n")

    assert "malformed_diagnostics_jsonl" in _verify_codes(args, tmp_path)


def test_validate_only_verifier_rejects_stale_output_provenance_hash(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)
    _mutate_json(
        args.output_summary,
        lambda payload: [
            row.update({"sha256": "0" * 64})
            for row in payload["output_artifacts"]
            if row["role"] == "report"
        ],
    )

    assert "output_artifact_sha256_mismatch" in _verify_codes(args, tmp_path)
