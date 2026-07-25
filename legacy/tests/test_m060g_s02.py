from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

import pytest

from scripts import m060g_figure_judge

ROOT = Path(__file__).resolve().parents[1]


def _fake_model_result(
    binding_id: str, model_id: str, latency_ms: float, scores: dict[str, Any]
) -> dict[str, Any]:
    return {
        "binding_id": binding_id,
        "model_id": model_id,
        "model_used": model_id,
        "modality": "multimodal"
        if binding_id == m060g_figure_judge.QUALITY_BINDING_ID
        else "text-only",
        "status": "passed",
        "status_code": 200,
        "latency_ms": latency_ms,
        "attempts": 1,
        "scores": scores,
        "response_text": json.dumps(scores),
        "usage": {"input_tokens": 10, "output_tokens": 10},
        "cost_estimate_usd": None,
        "cost_estimate_note": "Not measurable without an external MiniMax pricing table.",
        "error": None,
    }


def _sample_record(tmp_path: Path, figure_id: str = "paper::1") -> dict[str, Any]:
    fast_scores = {
        "caption_accuracy": 0.6,
        "figure_completeness": 0.7,
        "structural_fidelity": 0.8,
        "missing_elements": [],
    }
    quality_scores = {
        "caption_accuracy": 0.8,
        "figure_completeness": 0.9,
        "structural_fidelity": 0.85,
        "missing_elements": [],
    }
    fast = _fake_model_result(
        m060g_figure_judge.FAST_BINDING_ID,
        "MiniMax-M2.7-highspeed",
        1200.0,
        fast_scores,
    )
    quality = _fake_model_result(
        m060g_figure_judge.QUALITY_BINDING_ID,
        "MiniMax-M3",
        2400.0,
        quality_scores,
    )
    return {
        "figure": {
            "figure_id": figure_id,
            "safe_id": figure_id.replace("::", "__"),
            "arxiv_id": "paper",
            "figure_idx": 1,
            "category": "data_plot",
            "caption": "A caption.",
            "page_context_excerpt": "Context.",
            "source_json": "source.json",
            "source_image_path": str(tmp_path / "source.png"),
            "judge_png_path": str(tmp_path / "judge.png"),
            "label": "fig:plot",
            "name": "plot",
        },
        "safety_defaults": m060g_figure_judge.SAFETY_DEFAULTS,
        "diagnostic_llm_calls_override": m060g_figure_judge.DIAGNOSTIC_LLM_CALLS_OVERRIDE,
        "models": {
            m060g_figure_judge.FAST_BINDING_ID: fast,
            m060g_figure_judge.QUALITY_BINDING_ID: quality,
        },
        "comparison": m060g_figure_judge.compare_scores(fast, quality),
    }


def test_figure_judge_runs(tmp_path: Path) -> None:
    m060g_figure_judge.load_dotenv()
    if not (os.environ.get("ANTHROPIC_API_KEY") or os.environ.get("MINIMAX_API_KEY")):
        pytest.skip("MiniMax API key is not set; live S02 judge run skipped")
    output_dir = tmp_path / "m060g-live"
    report = m060g_figure_judge.run_judge(
        output_dir=output_dir,
        concurrency=2,
        timeout_seconds=90,
        max_retries=1,
        backoff_seconds=1.0,
        force=True,
    )
    assert report["aggregate"]["figure_count"] == 30
    assert report["aggregate"]["category_counts"] == {"data_plot": 15, "schema_diagram": 15}


def test_per_figure_output_schema(tmp_path: Path) -> None:
    record = _sample_record(tmp_path)
    path = tmp_path / "per-figure" / "paper__1.json"
    path.parent.mkdir()
    path.write_text(json.dumps(record))

    loaded = json.loads(path.read_text())
    assert set(loaded) == {
        "figure",
        "safety_defaults",
        "diagnostic_llm_calls_override",
        "models",
        "comparison",
    }
    assert set(loaded["models"]) == {
        m060g_figure_judge.FAST_BINDING_ID,
        m060g_figure_judge.QUALITY_BINDING_ID,
    }
    assert loaded["comparison"]["winner"] == m060g_figure_judge.QUALITY_BINDING_ID


def test_comparison_json_valid(tmp_path: Path) -> None:
    records = [_sample_record(tmp_path, f"paper::{idx}") for idx in range(1, 3)]
    comparison = m060g_figure_judge.write_reports(records, tmp_path)

    comparison_path = tmp_path / "comparison.json"
    summary_path = tmp_path / "judge-summary.json"
    markdown_path = tmp_path / "comparison.md"
    assert comparison_path.exists()
    assert summary_path.exists()
    assert markdown_path.exists()
    loaded = json.loads(comparison_path.read_text())
    assert loaded["aggregate"]["figure_count"] == 2
    assert loaded["aggregate"]["winner_counts"] == {m060g_figure_judge.QUALITY_BINDING_ID: 2}
    assert comparison["network_host_reference"] == "127.0.0.1"
    assert "localhost" not in markdown_path.read_text().lower()


def test_resolve_messages_endpoint_from_anthropic_base_url() -> None:
    binding = m060g_figure_judge.ModelBinding(
        binding_id="test",
        model_id="test-model",
        endpoint="https://api.minimax.io/anthropic/v1/messages",
        model_name="MiniMax-M3",
    )
    assert (
        m060g_figure_judge.resolve_messages_endpoint(binding, "https://api.minimax.io/anthropic")
        == "https://api.minimax.io/anthropic/v1/messages"
    )
    assert (
        m060g_figure_judge.resolve_messages_endpoint(
            binding, "https://api.minimax.io/anthropic/v1/messages"
        )
        == "https://api.minimax.io/anthropic/v1/messages"
    )


def test_4_dimensions_in_each_figure() -> None:
    payload = m060g_figure_judge.validate_score_payload(
        {
            "caption_accuracy": 0.5,
            "figure_completeness": 1,
            "structural_fidelity": 0,
            "missing_elements": ["legend"],
        }
    )
    assert set(payload) == set(m060g_figure_judge.REQUESTED_SCORE_KEYS)
    for key in m060g_figure_judge.DIMENSIONS:
        assert isinstance(payload[key], float)
        assert 0 <= payload[key] <= 1
    assert payload["missing_elements"] == ["legend"]


@pytest.mark.parametrize(
    "bad_payload",
    [
        {"caption_accuracy": 1, "figure_completeness": 1, "missing_elements": []},
        {
            "caption_accuracy": 1.2,
            "figure_completeness": 1,
            "structural_fidelity": 1,
            "missing_elements": [],
        },
        {
            "caption_accuracy": 1,
            "figure_completeness": 1,
            "structural_fidelity": 1,
            "missing_elements": "none",
        },
    ],
)
def test_score_payload_rejects_invalid_dimensions(bad_payload: dict[str, Any]) -> None:
    with pytest.raises(ValueError):
        m060g_figure_judge.validate_score_payload(bad_payload)


def test_5_safety_defaults() -> None:
    assert m060g_figure_judge.SAFETY_DEFAULTS == {
        "external_network_authorized": False,
        "graph_writes_authorized": False,
        "production_import_authorized": False,
        "fact_promotion_authorized": False,
        "llm_calls_authorized": False,
    }
    assert m060g_figure_judge.DIAGNOSTIC_LLM_CALLS_OVERRIDE["llm_calls_authorized"] is True
    assert "not authorized" in m060g_figure_judge.DIAGNOSTIC_LLM_CALLS_OVERRIDE["reason"]


def test_balanced_m058_selection_15_plots_15_schema() -> None:
    figures = m060g_figure_judge.load_figure_candidates()
    assert len(figures) == 30
    assert sum(figure.category == "data_plot" for figure in figures) == 15
    assert sum(figure.category == "schema_diagram" for figure in figures) == 15
    assert all(figure.caption for figure in figures)
    assert all(str(figure.image_path) for figure in figures)


def test_m050_m060g_s01_regression() -> None:
    from research_graph.infrastructure.papers.artifacts.worker import HttpTransport

    binding_ids = m060g_figure_judge.load_bindings()
    assert m060g_figure_judge.FAST_BINDING_ID in binding_ids
    assert m060g_figure_judge.QUALITY_BINDING_ID in binding_ids
    assert (
        HttpTransport(timeout_seconds=1, auth_env_var="ANTHROPIC_API_KEY").auth_env_var
        == "ANTHROPIC_API_KEY"
    )
    assert m060g_figure_judge.SAFETY_DEFAULTS["graph_writes_authorized"] is False
