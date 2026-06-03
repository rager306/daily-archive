"""Contract tests for M028 S05 Hermes digest projection."""

from __future__ import annotations

import importlib.util
import json
import sys
from copy import deepcopy
from pathlib import Path
from types import ModuleType

import pytest

REPO_ROOT = Path(__file__).parents[1]
BUILD_SCRIPT_PATH = REPO_ROOT / "scripts" / "build_m028_hermes_digest_projection.py"
REAL_CORPUS_DIR = REPO_ROOT / "data" / "article_corpora" / "m028-universal-loader-runtime-smoke-v1"


def _load_script() -> ModuleType:
    module_name = "build_m028_hermes_digest_projection"
    spec = importlib.util.spec_from_file_location(module_name, BUILD_SCRIPT_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module


def _read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _read_jsonl(path: Path) -> list[dict[str, object]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def _write_json(path: Path, payload: dict[str, object]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text("\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n", encoding="utf-8")


def _real_inputs() -> tuple[list[dict[str, object]], dict[str, object]]:
    return (
        _read_jsonl(REAL_CORPUS_DIR / "universal-loader-evidence-bundles.jsonl"),
        _read_json(REAL_CORPUS_DIR / "universal-loader-evidence-summary.json"),
    )


def _write_real_inputs(tmp_path: Path) -> tuple[Path, Path]:
    bundles, summary = _real_inputs()
    bundles_path = tmp_path / "universal-loader-evidence-bundles.jsonl"
    summary_path = tmp_path / "universal-loader-evidence-summary.json"
    _write_jsonl(bundles_path, bundles)
    _write_json(summary_path, summary)
    return bundles_path, summary_path


def test_projection_from_real_s04_scope_is_digest_only_and_fail_closed(tmp_path: Path) -> None:
    module = _load_script()
    bundles_path, summary_path = _write_real_inputs(tmp_path)

    projection = module.build_hermes_digest_projection(bundles_path, summary_path, tmp_path)

    assert projection["schema_name"] == "m028.hermes-digest-projection"
    assert projection["schema_version"] == "m028.hermes-digest-projection.v1"
    assert projection["generated_at"] == "deterministic_from_input_sha256"
    assert projection["summary"]["url_ref_count"] == 21
    assert projection["summary"]["normalized_identity_count"] == 20
    assert projection["summary"]["source_kind_counts"] == {
        "arxiv_abs_url": 15,
        "arxiv_pdf_url": 4,
        "company_blog_url": 1,
        "nature_article_url": 1,
    }
    assert {item["ref_id"] for item in projection["items"] if item["ref_id"] >= "R15"} == {"R15", "R16", "R17", "R18", "R19", "R20", "R21"}
    r01 = next(item for item in projection["items"] if item["ref_id"] == "R01")
    r10 = next(item for item in projection["items"] if item["ref_id"] == "R10")
    assert r01["identity_group"]["ref_ids"] == ["R01", "R10"]
    assert r10["identity_group"]["ref_ids"] == ["R01", "R10"]
    assert all(value == 0 for value in projection["unsafe_counters"].values())
    assert projection["redaction_flags"] == {
        "raw_article_text_embedded": False,
        "html_source_embedded": False,
        "raw_pdf_bytes_embedded": False,
        "source_payload_embedded": False,
        "chunk_payload_embedded": False,
        "model_output_embedded": False,
        "local_absolute_paths_embedded": False,
        "graph_or_kg_claims_embedded": False,
    }
    assert projection["generator"]["network_calls_attempted"] is False
    assert projection["generator"]["parser_attempted"] is False
    assert projection["generator"]["chunker_attempted"] is False
    assert projection["generator"]["model_attempted"] is False
    assert projection["generator"]["graph_write_attempted"] is False
    assert r01["bibliographic_fields"]["title"]["value"] is None
    assert r01["bibliographic_fields"]["title"]["diagnostic"] == "metadata_value_not_in_loader_evidence_bundle"
    assert "source_artifact" in r01["artifact_refs"]
    assert (tmp_path / "hermes-digest-projection.json").exists()
    report = (tmp_path / "hermes-digest-projection-report.md").read_text(encoding="utf-8")
    for heading in (
        "## Scope",
        "## Source References",
        "## Summary",
        "## Warnings",
        "## Skipped Diagnostics",
        "## Safety",
        "## Failure Modes",
        "## Load Profile",
        "## Negative Tests",
        "## Observability Impact",
    ):
        assert heading in report


def test_rejects_scope_drift_before_projection_write(tmp_path: Path) -> None:
    module = _load_script()
    bundles, summary = _real_inputs()
    bundles = deepcopy(bundles[:-1])
    summary = deepcopy(summary)
    summary["url_ref_count"] = 20
    bundles_path = tmp_path / "universal-loader-evidence-bundles.jsonl"
    summary_path = tmp_path / "universal-loader-evidence-summary.json"
    _write_jsonl(bundles_path, bundles)
    _write_json(summary_path, summary)

    with pytest.raises(module.HermesDigestProjectionInputError, match="SCOPE_REF_COUNT_MISMATCH"):
        module.build_hermes_digest_projection(bundles_path, summary_path, tmp_path)

    assert not (tmp_path / "hermes-digest-projection.json").exists()


def test_rejects_payload_markers_and_absolute_paths(tmp_path: Path) -> None:
    module = _load_script()
    bundles, summary = _real_inputs()
    bundles = deepcopy(bundles)
    bundles[0]["artifact_refs"]["source_artifact"]["path"] = "/tmp/leak.pdf"
    bundles[1]["raw_text"] = "forbidden source body"
    bundles_path = tmp_path / "universal-loader-evidence-bundles.jsonl"
    summary_path = tmp_path / "universal-loader-evidence-summary.json"
    _write_jsonl(bundles_path, bundles)
    _write_json(summary_path, summary)

    with pytest.raises(module.HermesDigestProjectionInputError) as exc_info:
        module.build_hermes_digest_projection(bundles_path, summary_path, tmp_path)

    message = str(exc_info.value)
    assert "ARTIFACT_PATH_UNSAFE" in message or "FORBIDDEN_KEY_PRESENT" in message
    assert not (tmp_path / "hermes-digest-projection.json").exists()


def test_rejects_nonzero_unsafe_counter(tmp_path: Path) -> None:
    module = _load_script()
    bundles, summary = _real_inputs()
    bundles = deepcopy(bundles)
    bundles[0]["safety_flags"]["parser_attempted"] = True
    bundles_path = tmp_path / "universal-loader-evidence-bundles.jsonl"
    summary_path = tmp_path / "universal-loader-evidence-summary.json"
    _write_jsonl(bundles_path, bundles)
    _write_json(summary_path, summary)

    with pytest.raises(module.HermesDigestProjectionInputError, match="UNSAFE_COUNTER_NONZERO"):
        module.build_hermes_digest_projection(bundles_path, summary_path, tmp_path)

    assert not (tmp_path / "hermes-digest-projection.json").exists()
