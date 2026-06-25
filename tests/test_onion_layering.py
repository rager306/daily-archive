"""Tests for the M104 S03 onion layering guard and composition root (D086).

Covers:
* :mod:`scripts.verify_onion_layering` catches a reverse import and passes on
  the real domain Core.
* :func:`build_wired_paper_pipeline` is the composition root — it adapts an
  :class:`LLMClientPort` into the application pipeline and runs through the Port.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
from dataclasses import replace
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GUARD_SCRIPT = ROOT / "scripts" / "verify_onion_layering.py"
DOMAIN_DIR = ROOT / "src" / "research_graph" / "domain"
APPLICATION_DIR = ROOT / "src" / "research_graph" / "application"
INFRASTRUCTURE_DIR = ROOT / "src" / "research_graph" / "infrastructure"
WORKFLOWS_DIR = ROOT / "src" / "research_graph" / "workflows"


def _run_guard(
    layer_root: Path,
    *,
    layer: str | None = None,
    json: bool = False,
) -> subprocess.CompletedProcess[str]:
    cmd = [sys.executable, str(GUARD_SCRIPT), "--root", str(layer_root)]
    if layer:
        cmd.extend(["--layer", layer])
    if json:
        cmd.append("--json")
    return subprocess.run(cmd, capture_output=True, text=True)


def _run_all_guard(*, json: bool = False) -> subprocess.CompletedProcess[str]:
    cmd = [sys.executable, str(GUARD_SCRIPT)]
    if json:
        cmd.append("--json")
    return subprocess.run(cmd, capture_output=True, text=True)


# ── Guard script ─────────────────────────────────────────────────────────────


class TestOnionLayeringGuard:
    def test_real_domain_core_is_clean(self) -> None:
        """The actual domain/ must pass the guard (no reverse imports)."""
        result = _run_guard(DOMAIN_DIR)
        assert result.returncode == 0, result.stderr
        assert "domain clean" in result.stdout

    def test_guard_catches_reverse_import(self) -> None:
        """A synthetic domain file importing infrastructure must fail (exit 1)."""
        import json

        with tempfile.TemporaryDirectory() as td:
            domain = Path(td) / "domain"
            domain.mkdir()
            (domain / "bad.py").write_text(
                "from research_graph.infrastructure.graph.ladybug_adapter import LadybugAdapter\n"
                "from research_graph.application.types import Pipeline\n"
            )
            result = _run_guard(domain, json=True)
        assert result.returncode == 1
        report = json.loads(result.stdout)
        assert report["status"] == "violations"
        assert report["violation_count"] == 2
        modules = {v["module"] for v in report["violations"]}
        assert any("infrastructure" in m for m in modules)
        assert any("application" in m for m in modules)

    def test_guard_passes_on_clean_synthetic(self) -> None:
        """A domain file importing only allowed roots must pass."""
        with tempfile.TemporaryDirectory() as td:
            domain = Path(td) / "domain"
            domain.mkdir()
            (domain / "good.py").write_text(
                "from research_graph.domain.schema import TypedEntity\n"
                "from research_graph.domain.ports import GraphDBPort\n"
            )
            result = _run_guard(domain)
        assert result.returncode == 0

    def test_real_application_layer_is_clean_and_scans_new_modules(self) -> None:
        """The actual application/ layer must scan corpus and graph use-case modules."""
        import json

        result = _run_all_guard(json=True)

        assert result.returncode == 0, result.stderr
        report = json.loads(result.stdout)
        app_report = report["layers"]["application"]
        scanned = set(app_report["scanned_files"])
        assert "corpus/coverage.py" in scanned
        assert "corpus/parser_replay.py" in scanned
        assert "graph/probe.py" in scanned
        assert app_report["violation_count"] == 0

    def test_application_guard_catches_infrastructure_and_script_imports(self) -> None:
        """Application code must not import concrete adapters or local scripts."""
        import json

        with tempfile.TemporaryDirectory() as td:
            application = Path(td) / "application"
            application.mkdir()
            (application / "bad.py").write_text(
                "from research_graph.infrastructure.graph.networkx_probe import NetworkXGraphProbeAdapter\n"
                "from scripts.build_r024_coverage_report import main\n"
            )
            result = _run_guard(application, layer="application", json=True)
        assert result.returncode == 1
        report = json.loads(result.stdout)
        modules = {v["module"] for v in report["violations"]}
        assert "research_graph.infrastructure.graph.networkx_probe" in modules
        assert "scripts.build_r024_coverage_report" in modules

    def test_real_infrastructure_layer_has_no_boundary_debt(self) -> None:
        """Infrastructure must not import CLI/workflow/script entry modules."""
        import json

        result = _run_guard(INFRASTRUCTURE_DIR, layer="infrastructure", json=True)

        assert result.returncode == 0, result.stderr
        report = json.loads(result.stdout)
        assert report["layer"] == "infrastructure"
        assert report["violation_count"] == 0
        assert report["allowed_violation_count"] == 0
        assert report["allowed_violations"] == []

    def test_infrastructure_guard_catches_unallowlisted_entry_imports(self) -> None:
        """Infrastructure code must not add new CLI/workflow/script imports."""
        import json

        with tempfile.TemporaryDirectory() as td:
            infrastructure = Path(td) / "infrastructure"
            infrastructure.mkdir()
            (infrastructure / "bad.py").write_text(
                "from research_graph.cli import DailyAnalysis\n"
                "from research_graph.workflows.validation.logging import log_event\n"
                "from scripts.run_quality_gate import main\n"
            )
            result = _run_guard(infrastructure, layer="infrastructure", json=True)
        assert result.returncode == 1
        report = json.loads(result.stdout)
        modules = {v["module"] for v in report["violations"]}
        assert "research_graph.cli" in modules
        assert "research_graph.workflows.validation.logging" in modules
        assert "scripts.run_quality_gate" in modules

    def test_real_workflows_layer_has_no_script_debt(self) -> None:
        """Workflow code must not import local script wrappers."""
        import json

        result = _run_guard(WORKFLOWS_DIR, layer="workflows", json=True)

        assert result.returncode == 0, result.stderr
        report = json.loads(result.stdout)
        assert report["layer"] == "workflows"
        assert report["violation_count"] == 0
        assert report["allowed_violation_count"] == 0
        assert report["allowed_violations"] == []

    def test_workflows_guard_catches_unallowlisted_script_imports(self) -> None:
        """Workflow code must not add new imports from script wrappers."""
        import json

        with tempfile.TemporaryDirectory() as td:
            workflows = Path(td) / "workflows"
            workflows.mkdir()
            (workflows / "bad.py").write_text("from scripts.new_wrapper import main\n")
            result = _run_guard(workflows, layer="workflows", json=True)
        assert result.returncode == 1
        report = json.loads(result.stdout)
        assert report["violations"] == [
            {
                "file": "bad.py",
                "module": "scripts.new_wrapper",
                "line": 0,
                "detail": "forbidden layer import",
            }
        ]


# ── Composition root ────────────────────────────────────────────────────────


class _FakeLLMProvider:
    """LLMClientPort double that records calls and returns canned extractions."""

    def __init__(self) -> None:
        self.calls: list[tuple[str, str]] = []  # (kind, prompt_prefix)

    def extract(self, prompt: str, kind: str, *, context=None) -> dict:
        self.calls.append((kind, prompt[:24]))
        if kind == "entities":
            return {
                "entities": [{"entity_type": "method", "canonical_name": "X", "confidence": 0.9}]
            }
        return {"relations": []}


def _load_wired() -> object:
    """Import build_wired_paper_pipeline lazily (keeps the test module standalone)."""
    from research_graph.application.profiles.paper import build_wired_paper_pipeline

    return build_wired_paper_pipeline


class TestCompositionRoot:
    def test_none_provider_falls_back_to_stub(self) -> None:
        build_wired = _load_wired()
        # pyrefly: ignore [not-callable]
        pipeline = build_wired(source_id="s")  # ty:ignore[call-non-callable]
        core = [s for s in pipeline.stages if s.stage_name == "core_entity_extractor"][0]
        assert core.llm_client is None

    def test_wires_port_into_llm_stages(self) -> None:
        build_wired = _load_wired()
        fake = _FakeLLMProvider()
        # pyrefly: ignore [not-callable]
        pipeline = build_wired(llm_provider=fake, source_id="arxiv:2605.18747")  # ty:ignore[call-non-callable]
        core = [s for s in pipeline.stages if s.stage_name == "core_entity_extractor"][0]
        rel = [s for s in pipeline.stages if s.stage_name == "relation_type_classifier"][0]
        assert core.llm_client is not None
        assert rel.llm_client is not None

    def test_pipeline_runs_through_port(self) -> None:
        from research_graph.application.types import PipelineContext

        build_wired = _load_wired()
        fake = _FakeLLMProvider()
        # pyrefly: ignore [not-callable]
        pipeline = build_wired(llm_provider=fake, source_id="arxiv:2605.18747")  # ty:ignore[call-non-callable]
        ctx = replace(
            PipelineContext(source_id="arxiv:2605.18747"),
            stage_outputs={
                "text_parts": ["Sparse attention reduces cost of self-attention mechanisms."]
            },
        )
        result = pipeline.run(ctx)
        patch = result.stage_outputs["core_entity_extractor"]
        # The Port was actually called via the adapter
        assert fake.calls, "provider.extract must be invoked"
        assert fake.calls[0][0] == "entities"
        assert len(patch.entities) >= 1
