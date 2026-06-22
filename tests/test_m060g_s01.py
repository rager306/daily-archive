from __future__ import annotations

import importlib.util
import os
import sys
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = ROOT / "scripts" / "m060g_smoke_test.py"
VALIDATOR_PATH = ROOT / "scripts"
if str(VALIDATOR_PATH) not in sys.path:
    sys.path.insert(0, str(VALIDATOR_PATH))

# pyrefly: ignore [missing-import]
import validate_models_yaml  # noqa: E402  # ty:ignore[unresolved-import]

spec = importlib.util.spec_from_file_location("m060g_smoke_test", SCRIPT_PATH)
assert spec is not None and spec.loader is not None
m060g_smoke_test = importlib.util.module_from_spec(spec)
sys.modules["m060g_smoke_test"] = m060g_smoke_test
spec.loader.exec_module(m060g_smoke_test)


def _registry() -> dict:
    return yaml.safe_load((ROOT / "models.yaml").read_text())


def _models_by_id() -> dict[str, dict]:
    return {model["id"]: model for model in _registry()["models"]}


def _bindings_by_id() -> dict[str, dict]:
    return {binding["binding_id"]: binding for binding in _registry()["bindings"]}


@pytest.fixture(scope="module")
def live_smoke_report(tmp_path_factory: pytest.TempPathFactory) -> dict:
    m060g_smoke_test.load_dotenv()
    if not (os.environ.get("ANTHROPIC_API_KEY") or os.environ.get("MINIMAX_API_KEY")):
        pytest.skip("MiniMax API key is not set; live M060-gakmo0 smoke tests skipped")

    output_path = tmp_path_factory.mktemp("m060g-smoke") / "smoke-test.json"
    report = m060g_smoke_test.run_smoke(output_path=output_path, timeout_seconds=60)
    assert output_path.exists()
    failed = [result for result in report["results"] if result["status"] == "failed"]
    assert failed == []
    return report


def test_models_yaml_has_m27_highspeed() -> None:
    models = _models_by_id()
    bindings = _bindings_by_id()

    assert models["minimax-m27-highspeed-anthropic"] == {
        "id": "minimax-m27-highspeed-anthropic",
        "provider": "anthropic",
        "endpoint": "https://api.minimax.io/anthropic/v1/messages",
        "model_name": "MiniMax-M2.7-highspeed",
        "tool_version": models["minimax-m27-highspeed-anthropic"]["tool_version"],
        "policy_version": "m060g-v0.1",
    }
    assert str(models["minimax-m27-highspeed-anthropic"]["tool_version"]) == "2026-05-15"
    assert bindings["figure-qa-judge-fast"]["model_id"] == "minimax-m27-highspeed-anthropic"


def test_models_yaml_has_m3_multimodal() -> None:
    models = _models_by_id()
    bindings = _bindings_by_id()

    assert models["minimax-m3-multimodal-anthropic"] == {
        "id": "minimax-m3-multimodal-anthropic",
        "provider": "anthropic",
        "endpoint": "https://api.minimax.io/anthropic/v1/messages",
        "model_name": "MiniMax-M3",
        "tool_version": models["minimax-m3-multimodal-anthropic"]["tool_version"],
        "policy_version": "m060g-v0.1",
    }
    assert str(models["minimax-m3-multimodal-anthropic"]["tool_version"]) == "2026-05-15"
    assert bindings["figure-qa-judge-quality"]["model_id"] == "minimax-m3-multimodal-anthropic"


def test_smoke_test_runs_both_models(live_smoke_report: dict) -> None:
    results = {result["test_id"]: result for result in live_smoke_report["results"]}

    assert set(results) == {"m27_text", "m3_text", "m3_multimodal_image"}
    assert results["m27_text"]["status"] == "passed"
    assert results["m3_text"]["status"] == "passed"
    assert results["m3_multimodal_image"]["status"] == "passed"
    assert "MiniMax-M2.7-highspeed" in live_smoke_report["latency_summary"]
    assert "MiniMax-M3" in live_smoke_report["latency_summary"]


def test_m27_response_format(live_smoke_report: dict) -> None:
    result = next(
        result for result in live_smoke_report["results"] if result["test_id"] == "m27_text"
    )

    assert result["status_code"] == 200
    assert result["model_used"] == "MiniMax-M2.7-highspeed"
    assert result["response_json"] == {"ok": True}


def test_m3_multimodal_image_input(live_smoke_report: dict) -> None:
    result = next(
        result
        for result in live_smoke_report["results"]
        if result["test_id"] == "m3_multimodal_image"
    )

    assert result["status_code"] == 200
    assert result["model_used"] == "MiniMax-M3"
    assert result["response_json"] == {"ok": True}
    assert result["image_media_type"] == "image/png"
    assert 9_000 <= result["image_bytes"] <= 15_000


def test_5_safety_defaults() -> None:
    defaults = m060g_smoke_test.SAFETY_DEFAULTS

    assert defaults == {
        "external_network_authorized": False,
        "graph_writes_authorized": False,
        "production_import_authorized": False,
        "fact_promotion_authorized": False,
        "llm_calls_authorized": False,
    }
    assert m060g_smoke_test.DIAGNOSTIC_LLM_CALLS_OVERRIDE["llm_calls_authorized"] is True
    assert "diagnostic" in m060g_smoke_test.DIAGNOSTIC_LLM_CALLS_OVERRIDE["scope"].lower()


def test_m050_m059_regression() -> None:
    registry = _registry()
    models = _models_by_id()
    bindings = _bindings_by_id()

    assert validate_models_yaml.validate_registry(registry) == []
    assert models["minimax-m3-512k-anthropic"]["model_name"] == "MiniMax-M3-512k"
    assert models["minimax-m3-openai"]["endpoint"] == "https://api.minimax.io/v1/chat/completions"
    assert bindings["article-artifact-classify"]["model_id"] == "minimax-m3-512k-anthropic"
    assert bindings["article-evidence-summarize"]["model_id"] == "minimax-m3-512k-anthropic"
    assert bindings["rlm-trajectory-capture"]["model_id"] == "minimax-m3-512k-anthropic"
