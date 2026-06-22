"""Contract tests for M028 S05 Hermes digest projection."""

from __future__ import annotations

import importlib.util
import json
import shutil
import sys
from copy import deepcopy
from pathlib import Path
from types import ModuleType

import pytest

REPO_ROOT = Path(__file__).parents[1]
BUILD_SCRIPT_PATH = REPO_ROOT / "scripts" / "build_m028_hermes_digest_projection.py"
VERIFY_SCRIPT_PATH = REPO_ROOT / "scripts" / "verify_m028_hermes_digest_projection.py"
REAL_CORPUS_DIR = REPO_ROOT / "data" / "article_corpora" / "m028-universal-loader-runtime-smoke-v1"


def _load_module(module_name: str, script_path: Path) -> ModuleType:
    spec = importlib.util.spec_from_file_location(module_name, script_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module


def _load_script() -> ModuleType:
    return _load_module("build_m028_hermes_digest_projection", BUILD_SCRIPT_PATH)


def _load_verifier() -> ModuleType:
    _load_script()
    return _load_module("verify_m028_hermes_digest_projection", VERIFY_SCRIPT_PATH)


def _read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _read_jsonl(path: Path) -> list[dict[str, object]]:
    return [
        json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()
    ]


def _write_json(path: Path, payload: dict[str, object]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text(
        "\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n", encoding="utf-8"
    )


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


def _repo_relative_tmp_dir(tmp_path: Path) -> Path:
    relative = Path("tests") / "tmp_m028_hermes_digest_projection" / tmp_path.name
    absolute = REPO_ROOT / relative
    shutil.rmtree(absolute, ignore_errors=True)
    absolute.mkdir(parents=True, exist_ok=True)
    return relative


def _build_real_projection(tmp_path: Path) -> tuple[Path, Path, Path, Path]:
    module = _load_script()
    work_dir = _repo_relative_tmp_dir(tmp_path)
    bundles_path, summary_path = _write_real_inputs(work_dir)
    module.build_hermes_digest_projection(bundles_path, summary_path, work_dir)
    return (
        bundles_path,
        summary_path,
        work_dir / "hermes-digest-projection.json",
        work_dir / "hermes-digest-projection-report.md",
    )


def _diagnostic_codes(diagnostics: list[object]) -> set[str]:
    # pyrefly: ignore [missing-attribute]
    return {str(diagnostic.code) for diagnostic in diagnostics}  # ty:ignore[unresolved-attribute]


def _zero_unsafe_counts(module: ModuleType) -> dict[str, int]:
    return {str(key): 0 for key in module.UNSAFE_COUNTER_KEYS}


def _safe_flags(module: ModuleType) -> dict[str, bool]:
    return {str(key): False for key in module.UNSAFE_SAFETY_FLAG_KEYS}


def _artifact(
    path: str | None, content_type: str | None, *, byte_count: int | None = 42
) -> dict[str, object]:
    return {
        "path": path,
        "sha256": "1" * 64 if path is not None else None,
        "byte_count": byte_count if path is not None else None,
        "content_type": content_type,
        "payload_embedded": False,
    }


def _minimal_bundle(
    module: ModuleType,
    ref_id: str,
    source_kind: str,
    normalized_identity: str,
    *,
    pdf_status: str,
    source_quality_status: str,
    warning: bool = False,
) -> dict[str, object]:
    source_family = (
        "arxiv"
        if source_kind.startswith("arxiv_")
        else "nature"
        if source_kind == "nature_article_url"
        else "company_blog"
    )
    url_variant = (
        "pdf_url"
        if source_kind == "arxiv_pdf_url"
        else "abs_url"
        if source_kind == "arxiv_abs_url"
        else source_kind
    )
    source_path = f"fixtures/{ref_id}-{source_kind}.dat"
    content_type = (
        "application/pdf" if source_kind == "arxiv_pdf_url" else "text/html; charset=utf-8"
    )
    pdf_path = source_path if pdf_status == "acquired_existing_pdf" else None
    diagnostics: list[dict[str, object]] = []
    if warning:
        diagnostics.append(
            {
                "code": "pdf_not_acquired_fixture_warning",
                "json_path": "$.pdf_acquisition.status",
                "message": "PDF was not acquired for abs URL fixture.",
                "ref_id": ref_id,
                "severity": "warning",
            }
        )
    return {
        "schema_version": "m028.universal-loader-evidence-bundle.v1",
        "ref_id": ref_id,
        "url": f"https://example.test/{ref_id}",
        "canonical_url": f"https://example.test/{normalized_identity.replace(':', '/')}",
        "source_kind": source_kind,
        "source_family": source_family,
        "url_variant": url_variant,
        "normalized_identity": normalized_identity,
        "selection": {"loader_owns_selection": False, "selection_policy": "fixture"},
        "identity_group": None,
        "artifact_refs": {
            "source_artifact": _artifact(source_path, content_type),
            "metadata_artifact": _artifact(source_path, content_type),
            "pdf_artifact": _artifact(pdf_path, "application/pdf" if pdf_path else None),
        },
        "source_metadata": {
            "metadata_status": "metadata_available",
            "diagnostic_count": len(diagnostics),
            "optional_metadata_gaps": [
                {"field": field, "reason": "fixture_minimal_metadata"}
                for field in module.OPTIONAL_BIBLIOGRAPHIC_FIELDS
            ],
        },
        "loader_evidence": {
            "bundle_status": "metadata_only_bundle_ready",
            "evidence_level": "source_metadata_and_pdf_diagnostics",
            "source_quality_status": source_quality_status,
            "parser_output_available": False,
            "hermes_digest_ready": False,
            "kg_import_eligible": False,
            "production_import_eligible": False,
            "outcome": "safe_for_downstream_metadata_projection_only",
        },
        "pdf_diagnostic": {
            "candidate_kind": "explicit_arxiv_pdf_url"
            if source_kind == "arxiv_pdf_url"
            else "not_fixture_pdf_source",
            "status": pdf_status,
            "terminal": True,
            "reason": "fixture_status",
            "diagnostic_count": 0,
        },
        "safety_flags": _safe_flags(module),
        "diagnostics": diagnostics,
    }


def _minimal_inputs(module: ModuleType) -> tuple[list[dict[str, object]], dict[str, object]]:
    bundles = [
        _minimal_bundle(
            module,
            "R01",
            "arxiv_pdf_url",
            "arxiv:2605.20897",
            pdf_status="acquired_existing_pdf",
            source_quality_status="source_metadata_with_verified_pdf_artifact",
        ),
        _minimal_bundle(
            module,
            "R02",
            "arxiv_abs_url",
            "arxiv:2605.21401",
            pdf_status="not_acquired",
            source_quality_status="source_metadata_without_pdf_artifact",
            warning=True,
        ),
        _minimal_bundle(
            module,
            "R03",
            "nature_article_url",
            "nature:articles_example",
            pdf_status="not_applicable",
            source_quality_status="source_metadata_non_pdf_source",
        ),
        _minimal_bundle(
            module,
            "R04",
            "company_blog_url",
            "company_blog:nvidia:example",
            pdf_status="not_applicable",
            source_quality_status="source_metadata_non_pdf_source",
        ),
        _minimal_bundle(
            module,
            "R10",
            "arxiv_abs_url",
            "arxiv:2605.20897",
            pdf_status="not_acquired",
            source_quality_status="source_metadata_without_pdf_artifact",
        ),
    ]
    duplicate_group = {
        "group_id": "identity:arxiv:2605.20897",
        "has_url_variants": True,
        "normalized_identity": "arxiv:2605.20897",
        "ref_ids": ["R01", "R10"],
        "url_ref_count": 2,
        "url_variants": ["pdf_url", "abs_url"],
    }
    for bundle in bundles:
        if bundle["ref_id"] in {"R01", "R10"}:
            bundle["identity_group"] = duplicate_group
    summary = {
        "schema_version": "m028.universal-loader-evidence-summary.v1",
        "url_ref_count": 5,
        "ref_count": 5,
        "normalized_identity_count": 4,
        "source_kind_counts": {
            "arxiv_abs_url": 2,
            "arxiv_pdf_url": 1,
            "company_blog_url": 1,
            "nature_article_url": 1,
        },
        "duplicate_identity_groups": [duplicate_group],
        "unsafe_claim_counts": _zero_unsafe_counts(module),
        "input_fingerprints": {
            "selection": {"path": "fixtures/selection.json", "sha256": "2" * 64}
        },
    }
    # pyrefly: ignore [bad-return]
    return bundles, summary


def _configure_minimal_scope(monkeypatch: pytest.MonkeyPatch, module: ModuleType) -> None:
    monkeypatch.setattr(module, "EXPECTED_REF_COUNT", 5)
    monkeypatch.setattr(module, "EXPECTED_IDENTITY_COUNT", 4)
    monkeypatch.setattr(module, "EXPECTED_REF_IDS", ["R01", "R02", "R03", "R04", "R10"])
    monkeypatch.setattr(module, "EXPANDED_SCOPE_REF_IDS", ["R10"])
    monkeypatch.setattr(
        module,
        "EXPECTED_SOURCE_KIND_COUNTS",
        {"arxiv_abs_url": 2, "arxiv_pdf_url": 1, "company_blog_url": 1, "nature_article_url": 1},
    )
    monkeypatch.setattr(module, "EXPECTED_DUPLICATE_GROUP", ["R01", "R10"])


def test_minimal_fixture_projection_is_deterministic_metadata_only_and_linked(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = _load_script()
    _configure_minimal_scope(monkeypatch, module)
    bundles, summary = _minimal_inputs(module)
    bundles_path = tmp_path / "minimal-bundles.jsonl"
    summary_path = tmp_path / "minimal-summary.json"
    _write_jsonl(bundles_path, bundles)
    _write_json(summary_path, summary)

    first = module.build_hermes_digest_projection(bundles_path, summary_path, tmp_path / "out-a")
    second = module.build_hermes_digest_projection(bundles_path, summary_path, tmp_path / "out-b")

    assert first == second
    assert first["summary"]["url_ref_count"] == 5
    assert first["summary"]["normalized_identity_count"] == 4
    assert first["summary"]["source_kind_counts"] == {
        "arxiv_abs_url": 2,
        "arxiv_pdf_url": 1,
        "company_blog_url": 1,
        "nature_article_url": 1,
    }
    assert first["summary"]["pdf_status_counts"] == {
        "acquired_existing_pdf": 1,
        "not_acquired": 2,
        "not_applicable": 2,
    }
    assert first["summary"]["diagnostic_counts"] == {"pdf_not_acquired_fixture_warning": 1}
    assert first["summary"]["duplicate_identity_groups"][0]["ref_ids"] == ["R01", "R10"]
    assert all(value == 0 for value in first["unsafe_counters"].values())
    assert all(value is False for value in first["redaction_flags"].values())
    r02 = next(item for item in first["items"] if item["ref_id"] == "R02")
    assert r02["warnings"][0]["code"] == "pdf_not_acquired_fixture_warning"
    assert r02["skipped_diagnostics"][0]["code"] == "metadata_value_not_in_loader_evidence_bundle"
    r01 = next(item for item in first["items"] if item["ref_id"] == "R01")
    assert r01["artifact_refs"]["source_artifact"]["path"] == "fixtures/R01-arxiv_pdf_url.dat"
    assert r01["artifact_refs"]["source_artifact"]["payload_embedded"] is False
    assert r01["identity_group"]["ref_ids"] == ["R01", "R10"]
    assert "## Failure Modes" in (
        tmp_path / "out-a" / "hermes-digest-projection-report.md"
    ).read_text(encoding="utf-8")


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
    assert {item["ref_id"] for item in projection["items"] if item["ref_id"] >= "R15"} == {
        "R15",
        "R16",
        "R17",
        "R18",
        "R19",
        "R20",
        "R21",
    }
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
    assert (
        r01["bibliographic_fields"]["title"]["diagnostic"]
        == "metadata_value_not_in_loader_evidence_bundle"
    )
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
    bundles[0]["artifact_refs"]["source_artifact"]["path"] = "/tmp/leak.pdf"  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
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
    # pyrefly: ignore [unsupported-operation]
    bundles[0]["safety_flags"]["parser_attempted"] = True  # ty:ignore[invalid-assignment]
    bundles_path = tmp_path / "universal-loader-evidence-bundles.jsonl"
    summary_path = tmp_path / "universal-loader-evidence-summary.json"
    _write_jsonl(bundles_path, bundles)
    _write_json(summary_path, summary)

    with pytest.raises(module.HermesDigestProjectionInputError, match="UNSAFE_COUNTER_NONZERO"):
        module.build_hermes_digest_projection(bundles_path, summary_path, tmp_path)

    assert not (tmp_path / "hermes-digest-projection.json").exists()


def test_verifier_accepts_regenerated_real_projection_contract(tmp_path: Path) -> None:
    verifier = _load_verifier()
    bundles_path, summary_path, digest_path, report_path = _build_real_projection(tmp_path)

    diagnostics = verifier.validate_contract(
        bundles_path,
        summary_path,
        digest_path,
        report_path,
        reject_unsafe_claims=True,
    )
    digest = _read_json(digest_path)

    assert diagnostics == []
    assert digest["summary"]["url_ref_count"] == 21  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    assert digest["summary"]["normalized_identity_count"] == 20  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    assert set(digest["summary"]["expanded_scope_ref_ids"]) == {  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
        "R15",
        "R16",
        "R17",
        "R18",
        "R19",
        "R20",
        "R21",
    }
    assert digest["summary"]["duplicate_identity_groups"][0]["ref_ids"] == ["R01", "R10"]  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    # pyrefly: ignore [missing-attribute]
    assert all(value == 0 for value in digest["unsafe_counters"].values())  # ty:ignore[unresolved-attribute]


def test_verifier_reports_malformed_jsonl_row(tmp_path: Path) -> None:
    verifier = _load_verifier()
    bundles_path, summary_path, digest_path, report_path = _build_real_projection(tmp_path)
    bundles_path.write_text('{"ref_id":"R01"}\n{not json}\n', encoding="utf-8")

    diagnostics = verifier.validate_contract(
        bundles_path,
        summary_path,
        digest_path,
        report_path,
        reject_unsafe_claims=True,
    )

    assert "JSONL_MALFORMED" in _diagnostic_codes(diagnostics)


def test_verifier_reports_missing_bundle_item_and_expanded_ref(tmp_path: Path) -> None:
    verifier = _load_verifier()
    bundles_path, summary_path, digest_path, report_path = _build_real_projection(tmp_path)
    bundles = _read_jsonl(bundles_path)[:-1]
    _write_jsonl(bundles_path, bundles)

    diagnostics = verifier.validate_contract(
        bundles_path,
        summary_path,
        digest_path,
        report_path,
        reject_unsafe_claims=True,
    )
    codes = _diagnostic_codes(diagnostics)

    assert "SCOPE_REF_COUNT_MISMATCH" in codes
    assert "EXPANDED_SCOPE_REFS_MISSING" in codes
    assert "DIGEST_ITEM_REF_SET_MISMATCH" in codes


def test_verifier_reports_digest_missing_expanded_ref(tmp_path: Path) -> None:
    verifier = _load_verifier()
    bundles_path, summary_path, digest_path, report_path = _build_real_projection(tmp_path)
    digest = _read_json(digest_path)
    # pyrefly: ignore [not-iterable]
    digest["items"] = [item for item in digest["items"] if item["ref_id"] != "R21"]  # ty:ignore[not-iterable]
    digest["summary"]["ref_ids"] = [ref for ref in digest["summary"]["ref_ids"] if ref != "R21"]  # pyrefly: ignore [bad-assignment, bad-index, unsupported-operation]  # ty:ignore[invalid-assignment]
    _write_json(digest_path, digest)

    diagnostics = verifier.validate_contract(
        bundles_path,
        summary_path,
        digest_path,
        report_path,
        reject_unsafe_claims=True,
    )
    codes = _diagnostic_codes(diagnostics)

    assert "DIGEST_SUMMARY_MISMATCH" in codes
    assert "DIGEST_ITEM_REF_SET_MISMATCH" in codes


def test_verifier_rejects_summary_count_drift(tmp_path: Path) -> None:
    verifier = _load_verifier()
    bundles_path, summary_path, digest_path, report_path = _build_real_projection(tmp_path)
    digest = _read_json(digest_path)
    # pyrefly: ignore [unsupported-operation]
    digest["summary"]["url_ref_count"] = 20  # ty:ignore[invalid-assignment]
    _write_json(digest_path, digest)

    diagnostics = verifier.validate_contract(
        bundles_path,
        summary_path,
        digest_path,
        report_path,
        reject_unsafe_claims=True,
    )
    codes = _diagnostic_codes(diagnostics)

    assert "DIGEST_SUMMARY_MISMATCH" in codes
    assert "DIGEST_SCOPE_REF_COUNT_MISMATCH" in codes


def test_verifier_rejects_s04_unsafe_summary_counter(tmp_path: Path) -> None:
    verifier = _load_verifier()
    bundles_path, summary_path, digest_path, report_path = _build_real_projection(tmp_path)
    summary = _read_json(summary_path)
    # pyrefly: ignore [unsupported-operation]
    summary["unsafe_claim_counts"]["parser_attempted"] = 1  # ty:ignore[invalid-assignment]
    _write_json(summary_path, summary)

    diagnostics = verifier.validate_contract(
        bundles_path,
        summary_path,
        digest_path,
        report_path,
        reject_unsafe_claims=True,
    )
    codes = _diagnostic_codes(diagnostics)

    assert "UNSAFE_COUNTER_MISMATCH" in codes
    assert "UNSAFE_CLAIM_IN_BUNDLE" in codes


def test_verifier_rejects_nonzero_digest_unsafe_counter(tmp_path: Path) -> None:
    verifier = _load_verifier()
    bundles_path, summary_path, digest_path, report_path = _build_real_projection(tmp_path)
    digest = _read_json(digest_path)
    # pyrefly: ignore [unsupported-operation]
    digest["unsafe_counters"]["parser_attempted"] = 1  # ty:ignore[invalid-assignment]
    _write_json(digest_path, digest)

    diagnostics = verifier.validate_contract(
        bundles_path,
        summary_path,
        digest_path,
        report_path,
        reject_unsafe_claims=True,
    )
    codes = _diagnostic_codes(diagnostics)

    assert "UNSAFE_COUNTER_MISMATCH" in codes
    assert "UNSAFE_CLAIM_REJECTED" in codes


def test_verifier_rejects_parser_readiness_claim(tmp_path: Path) -> None:
    verifier = _load_verifier()
    bundles_path, summary_path, digest_path, report_path = _build_real_projection(tmp_path)
    digest = _read_json(digest_path)
    # pyrefly: ignore [unsupported-operation]
    digest["generator"]["parser_attempted"] = True  # ty:ignore[invalid-assignment]
    _write_json(digest_path, digest)

    diagnostics = verifier.validate_contract(
        bundles_path,
        summary_path,
        digest_path,
        report_path,
        reject_unsafe_claims=True,
    )
    codes = _diagnostic_codes(diagnostics)

    assert "UNSAFE_BOOLEAN_TRUE" in codes
    assert "GENERATOR_UNSAFE_CLAIM" in codes


def test_verifier_rejects_kg_readiness_claim(tmp_path: Path) -> None:
    verifier = _load_verifier()
    bundles_path, summary_path, digest_path, report_path = _build_real_projection(tmp_path)
    digest = _read_json(digest_path)
    digest["items"][0]["loader_evidence"]["kg_import_eligible"] = True  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    _write_json(digest_path, digest)

    diagnostics = verifier.validate_contract(
        bundles_path,
        summary_path,
        digest_path,
        report_path,
        reject_unsafe_claims=True,
    )
    codes = _diagnostic_codes(diagnostics)

    assert "UNSAFE_BOOLEAN_TRUE" in codes


def test_verifier_rejects_payload_marker_and_forbidden_key(tmp_path: Path) -> None:
    verifier = _load_verifier()
    bundles_path, summary_path, digest_path, report_path = _build_real_projection(tmp_path)
    digest = _read_json(digest_path)
    digest["items"][0]["raw_text"] = "<html>payload leak</html>"  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    _write_json(digest_path, digest)

    diagnostics = verifier.validate_contract(
        bundles_path,
        summary_path,
        digest_path,
        report_path,
        reject_unsafe_claims=True,
    )
    codes = _diagnostic_codes(diagnostics)

    assert "FORBIDDEN_KEY_PRESENT" in codes
    assert "FORBIDDEN_MARKER_PRESENT" in codes


def test_verifier_rejects_unsafe_artifact_path(tmp_path: Path) -> None:
    verifier = _load_verifier()
    bundles_path, summary_path, digest_path, report_path = _build_real_projection(tmp_path)
    digest = _read_json(digest_path)
    digest["items"][1]["artifact_refs"]["source_artifact"]["path"] = "../escape.pdf"  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    _write_json(digest_path, digest)

    diagnostics = verifier.validate_contract(
        bundles_path,
        summary_path,
        digest_path,
        report_path,
        reject_unsafe_claims=True,
    )
    codes = _diagnostic_codes(diagnostics)

    assert "ARTIFACT_PATH_UNSAFE" in codes


def test_verifier_reports_missing_report_section(tmp_path: Path) -> None:
    verifier = _load_verifier()
    bundles_path, summary_path, digest_path, report_path = _build_real_projection(tmp_path)
    report = report_path.read_text(encoding="utf-8")
    report = report.replace("## Safety\n", "## Removed Safety\n")
    report_path.write_text(report, encoding="utf-8")

    diagnostics = verifier.validate_contract(
        bundles_path,
        summary_path,
        digest_path,
        report_path,
        reject_unsafe_claims=True,
    )

    assert "REPORT_SECTION_MISSING" in _diagnostic_codes(diagnostics)
