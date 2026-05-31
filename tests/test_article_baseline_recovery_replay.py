from __future__ import annotations

import importlib.util
import json
import sys
from argparse import Namespace
from pathlib import Path
from typing import Any

import pytest

RECOVERY_MODULE_PATH = Path(__file__).parents[1] / "scripts" / "verify_m025_baseline_recovery_replay.py"
recovery_spec = importlib.util.spec_from_file_location("verify_m025_baseline_recovery_replay", RECOVERY_MODULE_PATH)
assert recovery_spec is not None and recovery_spec.loader is not None
verify_m025_baseline_recovery_replay = importlib.util.module_from_spec(recovery_spec)
sys.modules[recovery_spec.name] = verify_m025_baseline_recovery_replay
recovery_spec.loader.exec_module(verify_m025_baseline_recovery_replay)

OUTPUTS_MODULE_PATH = Path(__file__).parents[1] / "scripts" / "verify_m025_baseline_recovery_outputs.py"
outputs_spec = importlib.util.spec_from_file_location("verify_m025_baseline_recovery_outputs", OUTPUTS_MODULE_PATH)
assert outputs_spec is not None and outputs_spec.loader is not None
verify_m025_baseline_recovery_outputs = importlib.util.module_from_spec(outputs_spec)
sys.modules[outputs_spec.name] = verify_m025_baseline_recovery_outputs
outputs_spec.loader.exec_module(verify_m025_baseline_recovery_outputs)

BaselineRecoveryError = verify_m025_baseline_recovery_replay.BaselineRecoveryError
run_recovery = verify_m025_baseline_recovery_replay.run_recovery

FIXTURE_CONTRACT = Path(__file__).parent / "fixtures" / "article_baseline_recovery_v00_01" / "contract.json"


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
    write_events = corpus_root / "baseline-recovery-events.jsonl"
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
        {
            "chunks": [
                {"chunk_id": f"{article_ref}:chunk:0001", "chunk_type": "retrieval_context"},
                {"chunk_id": f"{article_ref}:chunk:0002", "chunk_type": "retrieval_context"},
            ],
            "diagnostics": [],
        },
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
        write_events=write_events,
        write_summary=None,
        write_report=None,
        no_network=no_network,
        require_no_import_flags=False,
    )


def test_contract_fixture_defines_baseline_recovery_shape() -> None:
    contract = _contract()

    assert contract["schema_version"] == "m025-baseline-recovery-artifact.v00.01"
    assert set(contract["required_final_artifact_refs"]) == {"chunking", "assets", "tables", "links", "identity"}
    assert contract["required_baseline_provenance"]["kind"] == "regenerated_local_baseline"
    assert contract["required_network"] == {"no_network_required": True, "network_fetch_attempted": False}
    assert "production_import_attempted" in contract["required_false_safety_flags"]
    assert "ladybugdb_written" in contract["required_false_safety_flags"]


def test_baseline_recovery_requires_no_network_execution(tmp_path: Path) -> None:
    args = _args(tmp_path, no_network=False)

    with pytest.raises(BaselineRecoveryError, match="requires --no-network"):
        run_recovery(args)


def test_baseline_recovery_writes_contract_compliant_artifact_summary_and_report(tmp_path: Path) -> None:
    args = _args(tmp_path)
    args.write_summary = args.selection.parent / "baseline-recovery-summary.json"
    args.write_report = args.selection.parent / "baseline-recovery-report.md"
    args.require_no_import_flags = True

    events = run_recovery(args)
    summary = verify_m025_baseline_recovery_replay._summary_from_artifacts(args, events)
    verify_m025_baseline_recovery_replay._write_json(args.write_summary, summary)
    verify_m025_baseline_recovery_replay._write_report(args.write_report, summary)

    artifact_path = args.baseline / "arxiv-cs-ai-2512.24601" / "final.json"
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    contract = _contract()

    for field in contract["required_top_level_fields"]:
        assert field in artifact
    assert artifact["schema_version"] == contract["schema_version"]
    assert set(artifact["final_artifact_refs"]) == set(contract["required_final_artifact_refs"])
    assert artifact["metrics"]["chunk_count"] == 2
    assert set(artifact["metrics"]["evidence_counts"]) == set(contract["required_evidence_counts"])
    assert artifact["baseline_provenance"]["kind"] == "regenerated_local_baseline"
    assert artifact["network"] == contract["required_network"]
    assert artifact["readiness"]["final_replay_compatible"] is True
    for flag in contract["required_false_safety_flags"]:
        assert artifact["safety_state"][flag] is False
    assert any(event["event_type"] == "baseline_recovery.article_completed" for event in events)
    assert summary["readiness"]["baseline_recovery_completed"] is True
    assert summary["no_network_proof"]["network_fetch_attempted"] is False
    assert summary["no_write_safety"]["safety_violations"] == []
    report = args.write_report.read_text(encoding="utf-8")
    assert "## Failure Modes" in report
    assert "## Load Profile" in report
    assert "## Negative Tests" in report


def test_baseline_recovery_rejects_missing_local_chunking_without_fetching(tmp_path: Path) -> None:
    args = _args(tmp_path)
    (args.selection.parent / "chunking" / "arxiv-cs-ai-2512.24601" / "chunks.json").unlink()

    with pytest.raises(BaselineRecoveryError, match="missing local chunking artifact"):
        run_recovery(args)


def test_baseline_recovery_rejects_missing_local_evidence_without_fetching(tmp_path: Path) -> None:
    args = _args(tmp_path)
    (args.selection.parent / "evidence" / "arxiv-cs-ai-2512.24601" / "links.json").unlink()

    with pytest.raises(BaselineRecoveryError, match="missing local evidence artifact"):
        run_recovery(args)


def test_baseline_recovery_requires_local_paths(tmp_path: Path) -> None:
    args = _args(tmp_path)
    args.catalog = Path("https://example.test/catalog.json")

    with pytest.raises(BaselineRecoveryError, match="must be a local filesystem path"):
        run_recovery(args)


def test_validation_helper_accepts_generated_baseline_outputs(tmp_path: Path) -> None:
    args = _args(tmp_path)
    run_recovery(args)

    results = verify_m025_baseline_recovery_outputs.validate_baseline_artifacts(
        args.baseline,
        require_no_network=True,
        require_no_import_flags=True,
    )

    assert results == [
        {
            "path": str(args.baseline / "arxiv-cs-ai-2512.24601" / "final.json"),
            "article_ref": "arxiv/cs-ai/2512.24601",
            "chunk_count": 2,
            "evidence_counts": {"assets": 1, "identity": 1, "links": 1, "tables": 1},
            "baseline_provenance_kind": "regenerated_local_baseline",
        }
    ]


def test_validation_helper_rejects_baseline_missing_final_summary(tmp_path: Path) -> None:
    args = _args(tmp_path)
    run_recovery(args)
    final_summary = args.selection.parent / "final-replay-summary.json"
    _write_json(
        final_summary,
        {
            "baseline_comparison_counts": {"baseline_missing": 1},
            "no_network_proof": {"network_fetch_attempted": False},
            "no_write_safety": {"safety_violations": []},
        },
    )

    with pytest.raises(
        verify_m025_baseline_recovery_outputs.BaselineOutputValidationError,
        match="baseline_missing",
    ):
        verify_m025_baseline_recovery_outputs.validate_final_outputs(
            final=None,
            final_summary=final_summary,
            expect_article_count=None,
            require_no_network=True,
            require_no_import_flags=True,
            reject_baseline_missing=True,
            require_ready=False,
        )


def test_validation_helper_rejects_unsafe_baseline_flags(tmp_path: Path) -> None:
    args = _args(tmp_path)
    run_recovery(args)
    artifact_path = args.baseline / "arxiv-cs-ai-2512.24601" / "final.json"
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    artifact["safety_state"]["ladybugdb_written"] = True
    _write_json(artifact_path, artifact)

    exit_code = verify_m025_baseline_recovery_outputs.main(
        [
            "--baseline",
            str(args.baseline),
            "--require-no-network",
            "--require-no-import-flags",
        ]
    )

    assert exit_code == 2


def test_validation_helper_accepts_plan_alias_flags_and_recovery_surfaces(tmp_path: Path) -> None:
    args = _args(tmp_path)
    args.write_summary = args.selection.parent / "baseline-recovery-summary.json"
    args.write_report = args.selection.parent / "baseline-recovery-report.md"
    args.require_no_import_flags = True
    events = run_recovery(args)
    args.write_events.write_text("".join(json.dumps(event, sort_keys=True) + "\n" for event in events), encoding="utf-8")
    summary = verify_m025_baseline_recovery_replay._summary_from_artifacts(args, events)
    verify_m025_baseline_recovery_replay._write_json(args.write_summary, summary)

    exit_code = verify_m025_baseline_recovery_outputs.main(
        [
            "--baseline-dir",
            str(args.baseline),
            "--baseline-summary",
            str(args.write_summary),
            "--baseline-events",
            str(args.write_events),
            "--expect-article-count",
            "1",
            "--require-no-network",
            "--require-no-import-flags",
        ]
    )

    assert exit_code == 0


def test_validation_helper_accepts_t03_final_summary_and_readiness_decision_flags(tmp_path: Path) -> None:
    final_summary = tmp_path / "final-replay-summary.json"
    readiness_decision = tmp_path / "readiness-decision.json"
    _write_json(
        final_summary,
        {
            "article_count": 5,
            "baseline_comparison_counts": {"exact_match": 5, "baseline_missing": 0},
            "no_network_proof": {"network_fetch_attempted": False},
            "no_write_safety": {"safety_violations": []},
            "readiness": {
                "larger_preprocessing_validation_ready": True,
                "decision": "ready",
                "blockers": [],
                "graph_readiness_claim": False,
            },
        },
    )
    _write_json(
        readiness_decision,
        {
            "decision": "ready",
            "larger_preprocessing_validation_ready": True,
            "blockers": [],
            "graph_readiness_claim": False,
            "evidence": {
                "article_count": 5,
                "no_network_proof": {"network_fetch_attempted": False},
                "no_write_safety": {"safety_violations": []},
            },
        },
    )

    exit_code = verify_m025_baseline_recovery_outputs.main(
        [
            "--final-summary",
            str(final_summary),
            "--readiness-decision",
            str(readiness_decision),
            "--expect-article-count",
            "5",
            "--expect-no-baseline-missing",
            "--require-ready",
            "--require-no-network",
            "--require-no-import-flags",
        ]
    )

    assert exit_code == 0
