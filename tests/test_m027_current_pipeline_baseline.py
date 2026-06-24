from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path
from typing import Any

import pytest

from scripts import replay_m027_current_pipeline_baseline as replay
from scripts import verify_m027_current_pipeline_baseline as verifier

BaselineReplayError = replay.BaselineReplayError


def _sha(path: Path) -> str:
    return replay.sha256_file(path)


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _fixture(
    tmp_path: Path, *, parser_text: str | None = None, metadata_only: bool = True
) -> Namespace:
    root = tmp_path
    corpus = root / "corpus"
    converted = corpus / "conversion-quality" / "article_one" / "arxiv_pdf.txt"
    text = (
        parser_text
        or "# Fixture Paper\n\n## Introduction\n\nThis fixture has enough local converted article prose for the current baseline path.\n\n## Method\n\nThe accepted behavior should create retrieval-only chunks and refuse import readiness.\n"
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
    return Namespace(
        conversion_summary=conversion_summary,
        s03_summary=s03_summary,
        output_summary=corpus / "current-pipeline-baseline-summary.json",
        output_diagnostics=corpus / "current-pipeline-baseline-diagnostics.jsonl",
        output_report=corpus / "current-pipeline-baseline-report.md",
        output_dir=corpus / "current-pipeline-baseline",
        no_network=True,
    )


def _write_replay_outputs(args: Namespace) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    summary, diagnostics = replay.replay_baseline(args)
    replay.write_json(args.output_summary, summary)
    replay.write_jsonl(args.output_diagnostics, diagnostics)
    replay.write_report(args.output_report, summary)
    return summary, diagnostics


def _verifier_args(
    tmp_path: Path,
    args: Namespace,
    *,
    expected_article_count: int = 1,
    expected_variant_count: int = 2,
) -> Namespace:
    return Namespace(
        summary=args.output_summary,
        diagnostics=args.output_diagnostics,
        report=args.output_report,
        output_dir=args.output_dir,
        conversion_summary=args.conversion_summary,
        root=tmp_path,
        expected_article_count=expected_article_count,
        expected_variant_count=expected_variant_count,
    )


def _diagnostic_codes(findings: list[dict[str, Any]]) -> set[str]:
    return {str(finding.get("diagnostic_code")) for finding in findings}


def test_replay_requires_s03_linkage_and_no_network(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    args.no_network = False

    with pytest.raises(BaselineReplayError, match="requires --no-network"):
        replay.replay_baseline(args)

    args.no_network = True
    source_summary = tmp_path / "corpus" / "source-acquisition-summary.json"
    source_summary.write_text('{"changed": true}\n', encoding="utf-8")

    with pytest.raises(BaselineReplayError, match="stale S03 linkage"):
        replay.replay_baseline(args)


def test_replay_rejects_converted_payload_hash_mismatch(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    converted = tmp_path / "corpus" / "conversion-quality" / "article_one" / "arxiv_pdf.txt"
    converted.write_text("tampered after summary", encoding="utf-8")

    with pytest.raises(BaselineReplayError, match="converted_text_sha256 mismatch"):
        replay.replay_baseline(args)


def test_replay_captures_parser_ready_and_metadata_only_variants(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)

    summary, diagnostics = replay.replay_baseline(args)

    assert summary["article_count"] == 1
    assert summary["variant_count"] == 2
    assert summary["parser_ready_variant_count"] == 1
    assert summary["metadata_only_variant_count"] == 1
    assert summary["current_pipeline_chunk_count"] >= 1
    assert summary["import_ready_count"] == 0
    assert summary["import_eligible_chunk_count"] == 0
    assert summary["network_fetch_attempted"] is False
    assert summary["graph_import_allowed"] is False
    assert summary["ladybugdb_written"] is False
    codes = {row["diagnostic_code"] for row in diagnostics}
    assert "s03_linkage_verified" in codes
    assert "converted_payload_hash_verified" in codes
    assert "metadata_only_not_replayed" in codes
    assert "current_pipeline_retrieval_only_chunks" in codes
    artifact_path = args.output_dir / "article_one" / "baseline.json"
    assert artifact_path.exists()
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    assert artifact["article_ref"] == "article/one"
    assert artifact["variant_count"] == 2
    assert all(
        record["baseline_artifact_path"].endswith(
            "current-pipeline-baseline/article_one/baseline.json"
        )
        for record in summary["article_results"]
    )


def test_replay_records_zero_chunk_current_failure(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path, parser_text="   \n", metadata_only=False)

    summary, diagnostics = replay.replay_baseline(args)

    assert summary["parser_ready_variant_count"] == 1
    assert summary["current_pipeline_chunk_count"] == 0
    assert summary["zero_chunk_parser_ready_variant_count"] == 1
    assert summary["article_results"][0]["current_pipeline_metrics"]["package_state"] == "reject"
    assert any(row["diagnostic_code"] == "current_pipeline_zero_chunks" for row in diagnostics)


def test_metadata_artifacts_are_redacted(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    args = _fixture(tmp_path)

    summary, diagnostics = replay.replay_baseline(args)
    replay.write_json(args.output_summary, summary)
    replay.write_jsonl(args.output_diagnostics, diagnostics)
    replay.write_report(args.output_report, summary)

    combined = "\n".join(
        [
            args.output_summary.read_text(encoding="utf-8"),
            args.output_diagnostics.read_text(encoding="utf-8"),
            args.output_report.read_text(encoding="utf-8"),
            (args.output_dir / "article_one" / "baseline.json").read_text(encoding="utf-8"),
        ]
    )
    assert "This fixture has enough local converted article prose" not in combined
    assert "chunk_text" not in combined
    assert '"content"' not in combined
    assert '"raw_text"' not in combined
    assert "<html" not in combined.lower()
    assert "%PDF-" not in combined


def test_verifier_accepts_valid_replay_outputs(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    monkeypatch.setattr(verifier, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)

    verifier_summary, findings = verifier.verify(_verifier_args(tmp_path, args))

    assert findings == []
    assert verifier_summary["status"] == "passed"
    assert verifier_summary["network_fetch_attempted"] is False
    assert verifier_summary["production_import_attempted"] is False
    assert verifier_summary["ladybugdb_written"] is False
    assert verifier_summary["graph_import_allowed"] is False


def test_verifier_flags_stale_s03_source_summary_hash(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    monkeypatch.setattr(verifier, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)
    (tmp_path / "corpus" / "source-acquisition-summary.json").write_text(
        '{"changed": true}\n', encoding="utf-8"
    )

    _, findings = verifier.verify(_verifier_args(tmp_path, args))

    assert "s03_source_summary_sha256_mismatch" in _diagnostic_codes(findings)


def test_verifier_flags_missing_per_article_artifact(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    monkeypatch.setattr(verifier, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)
    (args.output_dir / "article_one" / "baseline.json").unlink()

    _, findings = verifier.verify(_verifier_args(tmp_path, args))

    assert "missing_baseline_artifact" in _diagnostic_codes(findings)


def test_verifier_flags_stale_converted_payload_hash(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    monkeypatch.setattr(verifier, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)
    converted = tmp_path / "corpus" / "conversion-quality" / "article_one" / "arxiv_pdf.txt"
    converted.write_text("tampered after baseline replay", encoding="utf-8")

    _, findings = verifier.verify(_verifier_args(tmp_path, args))

    codes = _diagnostic_codes(findings)
    assert "converted_payload_sha256_mismatch" in codes
    assert "converted_payload_byte_size_mismatch" in codes


def test_verifier_flags_unsafe_baseline_artifact_path(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    monkeypatch.setattr(verifier, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    summary, _ = _write_replay_outputs(args)
    summary["article_results"][0]["baseline_artifact_path"] = "../escape/baseline.json"
    replay.write_json(args.output_summary, summary)

    _, findings = verifier.verify(_verifier_args(tmp_path, args))

    assert any(
        code.startswith("unsafe_baseline_artifact_path") for code in _diagnostic_codes(findings)
    )


def test_verifier_flags_payload_leakage(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    monkeypatch.setattr(verifier, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    summary, _ = _write_replay_outputs(args)
    summary["article_results"][0]["raw_text"] = "RAW_PDF_SECRET <html"
    replay.write_json(args.output_summary, summary)

    _, findings = verifier.verify(_verifier_args(tmp_path, args))

    codes = _diagnostic_codes(findings)
    assert "metadata_payload_key_leakage" in codes
    assert "metadata_payload_snippet_leakage" in codes


def test_verifier_flags_missing_baseline_rows(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    monkeypatch.setattr(verifier, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    _write_replay_outputs(args)
    artifact_path = args.output_dir / "article_one" / "baseline.json"
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    artifact["variants"] = artifact["variants"][:1]
    artifact["variant_count"] = 1
    replay.write_json(artifact_path, artifact)

    _, findings = verifier.verify(_verifier_args(tmp_path, args))

    assert "baseline_artifact_variant_mismatch" in _diagnostic_codes(findings)


def test_verifier_flags_unsafe_true_flags(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(replay, "ROOT", tmp_path)
    monkeypatch.setattr(verifier, "ROOT", tmp_path)
    args = _fixture(tmp_path)
    summary, diagnostics = _write_replay_outputs(args)
    summary["network_fetch_attempted"] = True
    summary["article_results"][0]["graph_import_allowed"] = True
    diagnostics[0]["ladybugdb_written"] = True
    replay.write_json(args.output_summary, summary)
    replay.write_jsonl(args.output_diagnostics, diagnostics)

    _, findings = verifier.verify(_verifier_args(tmp_path, args))

    codes = _diagnostic_codes(findings)
    assert "summary_contract_mismatch" in codes
    assert "unsafe_safety_flag_true" in codes
