from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from replay_m031_chunk_evidence import build_review_corpus, main, sha256_file  # noqa: E402
from verify_m031_chunk_evidence_replay import main as verify_closeout_main  # noqa: E402

from research_graph.graph.readiness.review import validate_review_artifacts  # noqa: E402


def _write_json(path: Path, payload: dict[str, Any]) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return path


def _write_text(path: Path, text: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path


def _converted_markdown() -> str:
    return "\n".join(
        [
            "# Local Parser Ready Paper",
            "",
            "## Abstract",
            "",
            "This local parser ready artifact describes bounded replay evidence, deterministic structure parsing, and review only chunk routing.",
            "",
            "## Method",
            "",
            "The method keeps provenance identifiers, source spans, section hierarchy, and conservative route blockers without graph writes.",
            "",
            "## Results",
            "",
            "The result is a metadata only package with chunk diagnostics and explicit refusal states for import readiness review.",
        ]
    )


def _row(
    *,
    identity: str,
    role: str,
    status: str,
    parser_ready: bool = False,
    converted_path: Path | str | None = None,
    converted_sha256: str | None = None,
    converted_size: int = 0,
    diagnostic_code: str | None = None,
) -> dict[str, Any]:
    return {
        "identity": identity,
        "article_ref": "arxiv/cs-cl/2507.19457" if identity == "arxiv:2507.19457" else identity.replace(":", "/"),
        "article_key": identity.rsplit(":", 1)[-1],
        "variant_id": f"{identity}:source:{role}",
        "source_role": role,
        "status": status,
        "terminal_state": status,
        "diagnostic_code": diagnostic_code or ("parser_ready_converted_text" if parser_ready else f"{status}_refused"),
        "refusal_code": None if parser_ready else diagnostic_code or f"{status}_refused",
        "failure_reason": None if parser_ready else "fixture refusal",
        "converted_text_path": converted_path.as_posix() if isinstance(converted_path, Path) else converted_path,
        "converted_text_sha256": converted_sha256,
        "converted_text_byte_size": converted_size,
        "parser_ready": parser_ready,
        "graph_import_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "fail_closed_safety_flags": {
            "network_fetch_attempted": False,
            "graph_import_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
    }


def _fixture(tmp_path: Path) -> tuple[Path, Path, Path, Path, Path, dict[str, Any], dict[str, Any]]:
    project = tmp_path / "project"
    corpus = project / "data" / "article_corpora" / "m031-catalog-backed-replay-v1"
    converted = _write_text(corpus / "conversion-quality" / "converted-text" / "arxiv_cs-cl_2507.19457" / "arxiv_pdf.txt", _converted_markdown())
    converted_hash = sha256_file(converted)
    rows = [
        _row(identity="arxiv:2507.19457", role="arxiv_html", status="low_quality", diagnostic_code="converted_text_low_quality"),
        _row(
            identity="arxiv:2507.19457",
            role="arxiv_pdf",
            status="converted",
            parser_ready=True,
            converted_path=converted,
            converted_sha256=converted_hash,
            converted_size=converted.stat().st_size,
        ),
        _row(identity="arxiv:2507.19457", role="arxiv_abs_page", status="metadata_only", diagnostic_code="metadata_only_refused"),
        _row(identity="stanford:cs224n:gradient-notes", role="external_pdf", status="blocked", diagnostic_code="catalog_pdf_missing"),
        _row(identity="arxiv:2605.29548", role="arxiv_abs_page", status="metadata_only", diagnostic_code="metadata_only_refused"),
        _row(identity="arxiv:2605.29548", role="arxiv_pdf", status="blocked", diagnostic_code="missing_source_artifact"),
        _row(identity="arxiv:2605.26099", role="arxiv_abs_url", status="blocked", diagnostic_code="catalog_placeholder_pruned_no_article_record"),
    ]
    selection = _write_json(
        corpus / "selection.json",
        {
            "schema_version": "m031-catalog-backed-replay-selection.v1",
            "selection_id": "m031-catalog-backed-replay-v1",
            "milestone_id": "M031-vwpd8e",
        },
    )
    conversion_summary = {
        "schema_version": "m031-parser-conversion-replay.v1",
        "milestone_id": "M031-vwpd8e",
        "slice_id": "S03",
        "selection_id": "m031-catalog-backed-replay-v1",
        "row_count": 7,
        "parser_ready_count": 1,
        "counts": {"blocked": 3, "converted": 1, "low_quality": 1, "metadata_only": 2},
        "results": rows,
        "network_fetch_attempted": False,
        "graph_import_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }
    conversion_summary_path = _write_json(corpus / "conversion-quality" / "conversion-quality-summary.json", conversion_summary)
    closeout = {
        "schema_version": "m031-parser-conversion-closeout-verifier.v1",
        "milestone_id": "M031-vwpd8e",
        "slice_id": "S03",
        "selection_id": "m031-catalog-backed-replay-v1",
        "status": "passed",
        "failure_count": 0,
        "row_count": 7,
        "parser_ready_count": 1,
        "network_fetch_attempted": False,
        "graph_import_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "fail_closed_safety_flags": {
            "network_fetch_attempted": False,
            "graph_import_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
    }
    closeout_path = _write_json(corpus / "parser-conversion-closeout-summary.json", closeout)
    output = corpus / "chunk-evidence"
    return project, selection, conversion_summary_path, closeout_path, output, conversion_summary, closeout


def _run(project: Path, selection: Path, conversion_summary: Path, closeout: Path, output: Path) -> int:
    return main(
        [
            "--selection",
            selection.as_posix(),
            "--conversion-summary",
            conversion_summary.as_posix(),
            "--closeout-summary",
            closeout.as_posix(),
            "--output-dir",
            output.as_posix(),
            "--project-root",
            project.as_posix(),
        ]
    )


def _run_verify(project: Path, selection: Path, conversion_summary: Path, closeout: Path, output: Path) -> int:
    corpus = output.parent
    return verify_closeout_main(
        [
            "--selection",
            selection.as_posix(),
            "--conversion-summary",
            conversion_summary.as_posix(),
            "--s03-closeout-summary",
            closeout.as_posix(),
            "--chunk-summary",
            (output / "chunk-evidence-summary.json").as_posix(),
            "--chunk-diagnostics",
            (output / "chunk-evidence-diagnostics.jsonl").as_posix(),
            "--chunk-report",
            (output / "chunk-evidence-report.md").as_posix(),
            "--review-events",
            (output / "independent-review-events.jsonl").as_posix(),
            "--review-dir",
            (corpus / "graph-readiness-review").as_posix(),
            "--review-summary",
            (corpus / "graph-readiness-review" / "independent-review-summary.md").as_posix(),
            "--project-root",
            project.as_posix(),
            "--write-summary",
            (corpus / "chunk-evidence-closeout-summary.json").as_posix(),
            "--write-diagnostics",
            (corpus / "chunk-evidence-closeout-diagnostics.jsonl").as_posix(),
            "--write-report",
            (corpus / "chunk-evidence-closeout-report.md").as_posix(),
        ]
    )


def _closeout_findings(output: Path) -> list[dict[str, Any]]:
    diagnostics_path = output.parent / "chunk-evidence-closeout-diagnostics.jsonl"
    if not diagnostics_path.exists():
        return []
    return [json.loads(line) for line in diagnostics_path.read_text(encoding="utf-8").splitlines() if line.strip()]


def test_chunk_evidence_replay_writes_one_package_and_six_zero_chunk_refusals(tmp_path: Path) -> None:
    project, selection, conversion_summary, closeout, output, _summary_payload, _closeout_payload = _fixture(tmp_path)

    assert _run(project, selection, conversion_summary, closeout, output) == 0

    summary = json.loads((output / "chunk-evidence-summary.json").read_text(encoding="utf-8"))
    diagnostics = [json.loads(line) for line in (output / "chunk-evidence-diagnostics.jsonl").read_text(encoding="utf-8").splitlines()]
    report = (output / "chunk-evidence-report.md").read_text(encoding="utf-8")
    structure_package = json.loads((output / "packages" / "arxiv_cs-cl_2507.19457_arxiv_pdf" / "structure-aware-package.json").read_text(encoding="utf-8"))
    graph_package = json.loads((output / "packages" / "arxiv_cs-cl_2507.19457_arxiv_pdf" / "graph-readiness-package.json").read_text(encoding="utf-8"))
    review_corpus = json.loads((output / "review-corpus.json").read_text(encoding="utf-8"))
    review_events = [json.loads(line) for line in (output / "independent-review-events.jsonl").read_text(encoding="utf-8").splitlines()]
    review_dir = output.parent / "graph-readiness-review"
    review_markdown = (review_dir / "arxiv_cs-cl_2507.19457_arxiv_pdf-review.md").read_text(encoding="utf-8")
    review_summary = (review_dir / "independent-review-summary.md").read_text(encoding="utf-8")

    assert summary["row_count"] == 7
    assert summary["chunked_parser_ready_row_count"] == 1
    assert summary["zero_chunk_refusal_count"] == 6
    assert summary["counts"] == {"chunked": 1, "zero_chunk_refused": 6}
    assert summary["package_count"] == 1
    assert summary["graph_readiness_package_count"] == 1
    assert summary["pending_graph_readiness_review_count"] == 1
    assert summary["graph_readiness_review_blocker_count"] == 0
    assert summary["independent_review_completed_count"] == 0
    assert summary["automated_state_is_structural_only"] is True
    assert summary["import_eligible_chunk_count"] == 0
    assert summary["graph_import_allowed"] is False
    assert summary["trusted_kg_import_allowed"] is False
    assert summary["production_import_attempted"] is False
    assert summary["ladybugdb_written"] is False
    assert all(row["chunk_count"] == 0 for row in diagnostics if row["status"] == "zero_chunk_refused")
    assert [row for row in diagnostics if row["status"] == "chunked"][0]["chunk_count"] > 0
    assert structure_package["paper_id"] == "arxiv_cs-cl_2507.19457_arxiv_pdf"
    assert structure_package["diagnostics"]["counts_by_state"]
    assert graph_package["review_state"] == "pending_independent_graph_readiness_review"
    assert graph_package["output_contract_completed"] is False
    assert graph_package["validation"]["import_ready"] is False
    chunked_row = [row for row in summary["results"] if row["status"] == "chunked"][0]
    assert chunked_row["review_status"] == "pending_review"
    assert chunked_row["independent_review_completed"] is False
    assert chunked_row["automated_state_is_structural_only"] is True
    assert chunked_row["import_eligible_chunk_count"] == 0
    assert review_corpus["document_count"] == 1
    assert review_corpus["documents"][0]["paper_id"] == "arxiv_cs-cl_2507.19457_arxiv_pdf"
    assert review_corpus["documents"][0]["review_status"] == "pending_review"
    assert review_corpus["documents"][0]["independent_review_completed"] is False
    assert review_corpus["documents"][0]["import_eligible_count"] == 0
    assert any(event["event"] == "independent_review.requested" for event in review_events)
    assert any(event["event"] == "independent_review.summary" for event in review_events)
    assert all(event.get("event") != "independent_review.verdict" for event in review_events)
    assert all(event.get("output_contract_completed") is False for event in review_events)
    assert all(event.get("raw_text_included") is False for event in review_events)
    assert "Reviewer Output Contract" in review_markdown
    assert "bounded replay evidence" in review_markdown
    assert "Independent reviewer verdicts are still required" in review_summary
    assert validate_review_artifacts(review_dir=review_dir, events_path=output / "independent-review-events.jsonl").ok
    assert not validate_review_artifacts(review_dir=review_dir, events_path=output / "independent-review-events.jsonl", require_completed_review=True).ok
    assert "Local Parser Ready Paper" not in json.dumps(summary)
    assert "Local Parser Ready Paper" not in json.dumps(diagnostics)
    assert "Local Parser Ready Paper" not in json.dumps(structure_package)
    assert "Local Parser Ready Paper" not in json.dumps(graph_package)
    assert "Local Parser Ready Paper" not in json.dumps(review_corpus)
    assert "Local Parser Ready Paper" not in json.dumps(review_events)
    assert "metadata-only" in report
    assert "Review corpus" in report


@pytest.mark.parametrize(
    ("mutate", "expected_code"),
    [
        ("hash", "converted_text_sha256_mismatch"),
        ("outside_path", "converted_text_path_outside_project"),
        ("html_parser_ready", "non_pdf_parser_ready_refused"),
        ("stale_closeout", "s03_closeout_not_passed"),
        ("missing_converted", "missing_converted_text_artifact"),
        ("payload_marker", "metadata_payload_snippet_leakage"),
    ],
)
def test_chunk_evidence_replay_negative_fail_closed_cases(tmp_path: Path, capsys: pytest.CaptureFixture[str], mutate: str, expected_code: str) -> None:
    project, selection, conversion_summary_path, closeout_path, output, conversion_summary, closeout = _fixture(tmp_path)
    rows = conversion_summary["results"]
    parser_row = rows[1]
    if mutate == "hash":
        parser_row["converted_text_sha256"] = "0" * 64
        _write_json(conversion_summary_path, conversion_summary)
    elif mutate == "outside_path":
        outside = _write_text(tmp_path / "outside.txt", _converted_markdown())
        parser_row["converted_text_path"] = outside.as_posix()
        parser_row["converted_text_sha256"] = sha256_file(outside)
        parser_row["converted_text_byte_size"] = outside.stat().st_size
        _write_json(conversion_summary_path, conversion_summary)
    elif mutate == "html_parser_ready":
        rows[0]["status"] = "converted"
        rows[0]["parser_ready"] = True
        rows[0]["converted_text_path"] = parser_row["converted_text_path"]
        rows[0]["converted_text_sha256"] = parser_row["converted_text_sha256"]
        rows[0]["converted_text_byte_size"] = parser_row["converted_text_byte_size"]
        conversion_summary["parser_ready_count"] = 2
        closeout["parser_ready_count"] = 2
        _write_json(conversion_summary_path, conversion_summary)
        _write_json(closeout_path, closeout)
    elif mutate == "stale_closeout":
        closeout["status"] = "failed"
        closeout["failure_count"] = 1
        _write_json(closeout_path, closeout)
    elif mutate == "missing_converted":
        Path(parser_row["converted_text_path"]).unlink()
    elif mutate == "payload_marker":
        parser_row["identity"] = "deterministic fallback capture"
        _write_json(conversion_summary_path, conversion_summary)

    assert _run(project, selection, conversion_summary_path, closeout_path, output) == 2
    captured = capsys.readouterr()
    assert expected_code in captured.err
    assert not (output / "packages" / "arxiv_cs-cl_2507.19457_arxiv_pdf" / "structure-aware-package.json").exists()


def test_review_handoff_validation_fails_when_review_markdown_is_deleted(tmp_path: Path) -> None:
    project, selection, conversion_summary, closeout, output, _summary_payload, _closeout_payload = _fixture(tmp_path)
    assert _run(project, selection, conversion_summary, closeout, output) == 0
    review_dir = output.parent / "graph-readiness-review"
    (review_dir / "arxiv_cs-cl_2507.19457_arxiv_pdf-review.md").unlink()

    validation = validate_review_artifacts(review_dir=review_dir, events_path=output / "independent-review-events.jsonl")

    assert not validation.ok
    assert any("No review bundle files" in diagnostic for diagnostic in validation.diagnostics)
    summary = json.loads((output / "chunk-evidence-summary.json").read_text(encoding="utf-8"))
    assert summary["import_eligible_chunk_count"] == 0
    assert summary["graph_import_allowed"] is False


def test_review_handoff_rejects_stale_placeholder_and_fabricated_completed_verdict(tmp_path: Path) -> None:
    project, selection, conversion_summary, closeout, output, _summary_payload, _closeout_payload = _fixture(tmp_path)
    assert _run(project, selection, conversion_summary, closeout, output) == 0
    review_dir = output.parent / "graph-readiness-review"
    review_path = review_dir / "arxiv_cs-cl_2507.19457_arxiv_pdf-review.md"
    review_path.write_text("Reviewer Verdict Placeholder", encoding="utf-8")

    validation = validate_review_artifacts(review_dir=review_dir, events_path=output / "independent-review-events.jsonl")

    assert not validation.ok
    assert any("stale placeholder" in diagnostic for diagnostic in validation.diagnostics)

    with (output / "independent-review-events.jsonl").open("a", encoding="utf-8") as handle:
        handle.write(json.dumps({"event": "independent_review.verdict", "verdict": "PASS", "output_contract_completed": False}) + "\n")
    validation = validate_review_artifacts(review_dir=review_dir, events_path=output / "independent-review-events.jsonl", require_completed_review=True)
    assert not validation.ok
    assert any("output_contract_completed=true" in diagnostic for diagnostic in validation.diagnostics)
    summary = json.loads((output / "chunk-evidence-summary.json").read_text(encoding="utf-8"))
    assert summary["independent_review_completed_count"] == 0
    assert summary["trusted_kg_import_allowed"] is False


def test_review_corpus_refuses_non_parser_ready_rows_and_blocks_missing_graph_package(tmp_path: Path) -> None:
    project, selection, conversion_summary, closeout, output, _summary_payload, _closeout_payload = _fixture(tmp_path)
    assert _run(project, selection, conversion_summary, closeout, output) == 0
    summary = json.loads((output / "chunk-evidence-summary.json").read_text(encoding="utf-8"))
    diagnostics = summary["results"]
    chunked = [row for row in diagnostics if row["status"] == "chunked"][0]
    (project / chunked["graph_readiness_package_path"]).unlink()

    corpus, blocker_events = build_review_corpus(diagnostics=diagnostics, output_dir=output, project_root=project, run_id="test-run")

    assert corpus["document_count"] == 0
    assert len(blocker_events) == 1
    assert blocker_events[0]["event"] == "independent_review.blocker"
    assert blocker_events[0]["diagnostic_code"] == "missing_graph_readiness_package"
    assert chunked["review_status"] == "review_blocked_missing_graph_readiness_package"
    assert all(row["status"] != "chunked" or row.get("review_corpus_paper_id") is None for row in diagnostics)
    assert all(row["chunk_count"] == 0 for row in diagnostics if row["status"] == "zero_chunk_refused")
    assert corpus["import_eligible_count"] == 0
    assert corpus["trusted_kg_import_allowed"] is False


def test_chunk_evidence_replay_rejects_permissive_closeout_flags(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    project, selection, conversion_summary_path, closeout_path, output, _conversion_summary, closeout = _fixture(tmp_path)
    closeout["fail_closed_safety_flags"]["graph_import_allowed"] = True
    _write_json(closeout_path, closeout)

    assert _run(project, selection, conversion_summary_path, closeout_path, output) == 2

    captured = capsys.readouterr()
    assert "unsafe_closeout_flag" in captured.err
    assert not (output / "chunk-evidence-summary.json").exists()


def test_chunk_evidence_closeout_verifier_writes_passed_summary(tmp_path: Path) -> None:
    project, selection, conversion_summary, closeout, output, _summary_payload, _closeout_payload = _fixture(tmp_path)
    assert _run(project, selection, conversion_summary, closeout, output) == 0

    assert _run_verify(project, selection, conversion_summary, closeout, output) == 0

    closeout_summary = json.loads((output.parent / "chunk-evidence-closeout-summary.json").read_text(encoding="utf-8"))
    closeout_report = (output.parent / "chunk-evidence-closeout-report.md").read_text(encoding="utf-8")
    assert closeout_summary["status"] == "passed"
    assert closeout_summary["failure_count"] == 0
    assert closeout_summary["row_count"] == 7
    assert closeout_summary["parser_ready_row_count"] == 1
    assert closeout_summary["zero_chunk_refusal_count"] == 6
    assert closeout_summary["package_count"] == 1
    assert closeout_summary["graph_readiness_package_count"] == 1
    assert closeout_summary["chunk_count"] > 0
    assert closeout_summary["evidence_path_count"] == closeout_summary["chunk_count"]
    assert closeout_summary["graph_import_allowed"] is False
    assert closeout_summary["ladybugdb_written"] is False
    assert _closeout_findings(output) == []
    assert "## Failure Modes" in closeout_report
    assert "## Load Profile" in closeout_report
    assert "## Negative Tests" in closeout_report


@pytest.mark.parametrize(
    ("mutate", "expected_code"),
    [
        ("stale_closeout", "s03_closeout_not_passed"),
        ("missing_package", "missing_structure_package"),
        ("corrupt_package", "malformed_structure_package"),
        ("missing_evidence_path", "missing_chunk_evidence_path"),
        ("malformed_review_event", "malformed_review_event"),
        ("fabricated_review_event", "fabricated_completed_review_event"),
        ("payload_leak", "metadata_payload_key_leakage"),
        ("summary_import_flag", "unsafe_safety_flag_true"),
        ("graph_ladybugdb_flag", "unsafe_safety_flag_true"),
    ],
)
def test_chunk_evidence_closeout_verifier_negative_failures(tmp_path: Path, mutate: str, expected_code: str) -> None:
    project, selection, conversion_summary, closeout, output, _summary_payload, closeout_payload = _fixture(tmp_path)
    assert _run(project, selection, conversion_summary, closeout, output) == 0
    package_path = output / "packages" / "arxiv_cs-cl_2507.19457_arxiv_pdf" / "structure-aware-package.json"
    graph_path = output / "packages" / "arxiv_cs-cl_2507.19457_arxiv_pdf" / "graph-readiness-package.json"
    summary_path = output / "chunk-evidence-summary.json"
    events_path = output / "independent-review-events.jsonl"

    if mutate == "stale_closeout":
        closeout_payload["status"] = "failed"
        closeout_payload["failure_count"] = 1
        _write_json(closeout, closeout_payload)
    elif mutate == "missing_package":
        package_path.unlink()
    elif mutate == "corrupt_package":
        package_path.write_text("{not-json", encoding="utf-8")
    elif mutate == "missing_evidence_path":
        package = json.loads(package_path.read_text(encoding="utf-8"))
        package["chunks"][0].pop("source_span")
        _write_json(package_path, package)
    elif mutate == "malformed_review_event":
        events_path.write_text("{not-json\n", encoding="utf-8")
    elif mutate == "fabricated_review_event":
        with events_path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps({"event": "independent_review.verdict", "output_contract_completed": True}) + "\n")
    elif mutate == "payload_leak":
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        summary["raw_text"] = "Local Parser Ready Paper"
        _write_json(summary_path, summary)
    elif mutate == "summary_import_flag":
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        summary["graph_import_allowed"] = True
        _write_json(summary_path, summary)
    elif mutate == "graph_ladybugdb_flag":
        graph_package = json.loads(graph_path.read_text(encoding="utf-8"))
        graph_package["ladybugdb_written"] = True
        _write_json(graph_path, graph_package)

    assert _run_verify(project, selection, conversion_summary, closeout, output) == 1
    closeout_summary = json.loads((output.parent / "chunk-evidence-closeout-summary.json").read_text(encoding="utf-8"))
    findings = _closeout_findings(output)
    assert closeout_summary["status"] == "failed"
    assert closeout_summary["failure_count"] >= 1
    assert any(finding["diagnostic_code"] == expected_code for finding in findings)
    assert all(finding.get("severity") for finding in findings)
    assert all(finding.get("json_path") for finding in findings)
