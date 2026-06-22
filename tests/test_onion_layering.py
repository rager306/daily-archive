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


def _run_guard(domain_root: Path, *, json: bool = False) -> subprocess.CompletedProcess[str]:
    cmd = [sys.executable, str(GUARD_SCRIPT), "--root", str(domain_root)]
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
