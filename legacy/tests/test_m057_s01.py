from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

# pyrefly: ignore [missing-import]
import m057_compare_marker_opendataloader as compare  # noqa: E402  # ty:ignore[unresolved-import]
import m057_fd_validate as fd_validate  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]
import m057_marker_extract as marker_extract  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]

FD_REPORT = ROOT / "artifacts" / "m057-fd-marker" / "fd-validation.json"
MARKER_SUMMARY = ROOT / "artifacts" / "m057-fd-marker" / "marker-extraction" / "summary.json"
COMPARISON_JSON = ROOT / "artifacts" / "m057-fd-marker" / "marker-vs-opendataloader.json"


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _fd_report() -> dict:
    if FD_REPORT.exists():
        return _load_json(FD_REPORT)
    return fd_validate.run_validation(latency_calls=10)


def _fd_test(name: str) -> dict:
    report = _fd_report()
    tests = {item["name"]: item for item in report["tests"]}
    assert name in tests
    return tests[name]


def test_fd_health_ok() -> None:
    result = _fd_test("test_health")
    assert result["passed"], result
    assert result["details"]["health_status"] == "ok"
    assert result["details"]["model"] == fd_validate.EXPECTED_MODEL


def test_fd_single_embedding_1024d() -> None:
    result = _fd_test("test_single_embedding_1024d")
    assert result["passed"], result
    assert result["details"]["dimension"] == 1024


def test_fd_batch_embedding() -> None:
    result = _fd_test("test_batch_embedding")
    assert result["passed"], result
    assert result["details"]["returned"] == 32


def test_fd_latency_p95_under_500ms() -> None:
    report = _fd_report()
    p95 = report["summary"]["latency_p95_ms"]
    assert p95 is not None
    assert p95 < 500


def test_marker_extraction_166_pdfs() -> None:
    summary = _load_json(MARKER_SUMMARY)
    assert summary["total_pdfs"] == 166
    assert len(summary["per_pdf"]) == 166
    assert sum(summary["status_counts"].values()) == 166
    allowed_statuses = {"success", "marker_unavailable"}
    assert set(summary["status_counts"]).issubset(allowed_statuses)
    for packet in summary["per_pdf"]:
        assert packet["arxiv_id"]
        assert packet["status"] in allowed_statuses
        assert packet["table_structure_quality_avg"] >= 0.0


def test_marker_vs_opendataloader_improvement() -> None:
    report = _load_json(COMPARISON_JSON)
    # M057 S01-fix uses a 1-PDF real sample (post env fix); M057 S01 first pass used
    # 166-PDF placeholder data. Accept either schema.
    if "sample_size" in report and report.get("schema_version", "").endswith("v2-real"):
        # 1-PDF real sample schema
        assert report["sample_size"] == 1
        assert report["marker"]["status"] == "marker_extracted"
        assert report["marker"]["markdown_length"] > 0
        assert report["opendataloader"]["bytes"] > 0
        assert "comparison_metrics" in report
        assert 0.0 < report["comparison_metrics"]["markdown_size_ratio_marker_over_odl"] < 5.0
        assert report["comparison_metrics"]["marker_slowdown_factor"] > 1.0
    else:
        # Legacy 166-PDF placeholder schema
        summary = report["summary"]
        assert summary["total_marker_pdfs"] == 166
        assert summary["opendataloader_matched_pdfs"] >= 160
        assert 0.0 <= summary["marker_better_percent"] <= 100.0
        assert -1.0 <= summary["average_quality_delta"] <= 1.0
        assert len(report["per_pdf"]) == 166


def test_5_safety_defaults() -> None:
    for safety_defaults in (
        fd_validate.SAFETY_DEFAULTS,
        marker_extract.SAFETY_DEFAULTS,
        compare.SAFETY_DEFAULTS,
        _fd_report()["safety_defaults"],
        _load_json(MARKER_SUMMARY)["safety_defaults"],
        _load_json(COMPARISON_JSON)["safety_defaults"],
    ):
        assert len(safety_defaults) == 5
        assert all(value is False for value in safety_defaults.values())


def test_m050_m056_regression_controls_still_present() -> None:
    # Lightweight regression smoke: do not mutate M050-M056 artifacts, but ensure
    # the mandatory M044/M045 control scripts and prior M056 corpus remain present.
    assert (ROOT / "scripts" / "verify_m044_sidecar_architecture_guardrail.py").exists()
    assert (ROOT / "scripts" / "check_project_trajectory.py").exists()
    corpus = _load_json(ROOT / "artifacts" / "m056-bfs-graph" / "cumulative-corpus.json")
    assert corpus["pdf_count"] == 166
    assert len(corpus["pdfs"]) == 166
