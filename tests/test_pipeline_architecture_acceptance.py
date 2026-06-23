from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path
from types import ModuleType

ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "scripts" / "run_pipeline_architecture_acceptance.py"
ACCEPTANCE_SUMMARY = ROOT / "data" / "pipeline-script-architecture" / "acceptance-summary.json"
ACCEPTANCE_README = ROOT / "data" / "pipeline-script-architecture" / "README.md"


def _load_runner() -> ModuleType:
    spec = importlib.util.spec_from_file_location("run_pipeline_architecture_acceptance", RUNNER)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_acceptance_runner_writes_success_summary_with_counts(tmp_path: Path) -> None:
    runner = _load_runner()
    artifact = tmp_path / "phase-summary.json"
    summary_path = tmp_path / "acceptance-summary.json"
    command = (
        sys.executable,
        "-c",
        (
            "import json, pathlib; "
            f"path=pathlib.Path({str(artifact)!r}); "
            "path.write_text(json.dumps({'ok': 219, 'errors': 0}))"
        ),
    )
    phases = (
        runner.AcceptancePhase(
            name="fake_parser",
            command=command,
            artifacts=(artifact,),
            count_artifact=artifact,
            count_fields=("ok", "errors"),
        ),
    )

    summary = runner.run_acceptance(phases=phases, summary_path=summary_path, cwd=tmp_path)

    assert summary_path.exists()
    persisted = json.loads(summary_path.read_text())
    assert summary == persisted
    assert persisted["schema_version"] == "pipeline-architecture-acceptance.v00.01"
    assert persisted["succeeded"] is True
    assert persisted["first_failure"] is None
    phase = persisted["phases"][0]
    assert phase["name"] == "fake_parser"
    assert phase["status"] == "pass"
    assert phase["counts"] == {"ok": 219, "errors": 0}
    assert phase["artifacts"][0]["exists"] is True
    assert phase["artifacts"][0]["size_bytes"] > 0


def test_acceptance_runner_fails_fast_and_records_reason(tmp_path: Path) -> None:
    runner = _load_runner()
    marker = tmp_path / "should-not-run.txt"
    summary_path = tmp_path / "acceptance-summary.json"
    phases = (
        runner.AcceptancePhase(
            name="broken_phase",
            command=(sys.executable, "-c", "import sys; print('bad', file=sys.stderr); sys.exit(7)"),
            artifacts=(tmp_path / "missing.json",),
        ),
        runner.AcceptancePhase(
            name="later_phase",
            command=(sys.executable, "-c", f"from pathlib import Path; Path({str(marker)!r}).write_text('ran')"),
            artifacts=(marker,),
        ),
    )

    summary = runner.run_acceptance(phases=phases, summary_path=summary_path, cwd=tmp_path)

    assert summary["succeeded"] is False
    assert len(summary["phases"]) == 1
    assert not marker.exists()
    failure = summary["first_failure"]
    assert failure == summary["phases"][0]["first_failure"]
    assert failure["phase"] == "broken_phase"
    assert failure["code"] == "nonzero_exit"
    assert failure["exit_code"] == 7
    assert "bad" in failure["stderr_tail"]


def test_acceptance_default_phases_run_migrated_wrappers_in_dependency_order() -> None:
    runner = _load_runner()

    phases = runner.default_phases()

    assert [phase.name for phase in phases] == [
        "catalog_ingest_wrapper",
        "parser_replay_wrapper",
        "networkx_probe_wrapper",
        "coverage_report_wrapper",
    ]
    assert "scripts/ingest_to_canonical_catalog.py" in phases[0].command
    assert "scripts/replay_r024_218_document_parser_chunking.py" in phases[1].command
    assert "scripts/build_r024_218_document_networkx_probe.py" in phases[2].command
    assert "scripts/build_r024_coverage_report.py" in phases[3].command
    assert any(str(path).endswith("parser-chunking/summary.json") for path in phases[1].artifacts)
    assert any(str(path).endswith("networkx-probe/summary.json") for path in phases[2].artifacts)
    assert any(str(path).endswith("R024-COVERAGE.md") for path in phases[3].artifacts)


def test_persisted_acceptance_summary_records_m121_pipeline_counts() -> None:
    summary = json.loads(ACCEPTANCE_SUMMARY.read_text())

    assert summary["schema_version"] == "pipeline-architecture-acceptance.v00.01"
    assert summary["succeeded"] is True
    assert summary["phase_count"] == 4
    phases = {phase["name"]: phase for phase in summary["phases"]}
    parser = phases["parser_replay_wrapper"]["counts"]
    assert parser == {
        "total": 221,
        "ok": 219,
        "skipped": 2,
        "errors": 0,
        "chunk_count_total": 2576,
    }
    graph = phases["networkx_probe_wrapper"]["counts"]
    assert graph["corpus_size"] == 219
    assert graph["skipped_metadata_only"] == 2
    assert graph["n_nodes"] == 3891
    assert graph["n_edges"] == 10102
    assert graph["citation_relations_count"] == 6212
    coverage = phases["coverage_report_wrapper"]["counts"]
    assert coverage["catalog_records"] == 221
    assert coverage["source_backed_records"] == 219
    assert coverage["metadata_only_records"] == 2
    assert coverage["parser_errors"] == 0
    assert summary["first_failure"] is None


def test_acceptance_readme_documents_intentionally_out_of_scope_scripts() -> None:
    text = ACCEPTANCE_README.read_text()

    assert "Intentionally out of scope" in text
    assert "quality-metrics" in text
    assert "scripts/extract_r024_quality_metrics.py" in text
    assert "scripts/extract_r024_53_document_quality_metrics.py" in text
    assert "scripts/build_r024_entity_networkx_probe.py" in text
    assert "Do not add corpus body text" in text
