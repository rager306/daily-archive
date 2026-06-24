from __future__ import annotations

from typing import Any

# pyrefly: ignore [missing-import]
from scripts import verify_test_architecture as guardrail


def _file(path: str, bucket: str, **signals: bool) -> dict[str, Any]:
    defaults = {
        "imports_domain": False,
        "imports_application": False,
        "imports_infrastructure": False,
        "imports_workflows": False,
        "imports_cli": False,
        "imports_pipeline_legacy": False,
        "imports_scripts_normal": False,
        "dynamic_script_import": False,
        "subprocess_script_invocation": False,
        "acceptance_name": False,
    }
    defaults.update(signals)
    return {"path": path, "bucket": bucket, "signals": defaults, "imports": [], "reasons": []}


def _inventory(*files: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": "test",
        "summary": {"total_test_files": len(files)},
        "files": list(files),
        "pilot_candidates": [],
    }


def test_allowlisted_legacy_mixed_and_dynamic_script_import_pass() -> None:
    inventory = _inventory(
        _file("tests/test_legacy.py", "legacy-mixed", dynamic_script_import=True)
    )
    allowlist = {
        "legacy_mixed": ["tests/test_legacy.py"],
        "dynamic_script_import": ["tests/test_legacy.py"],
        "strict_application": [],
        "strict_domain": [],
        "strict_infrastructure": [],
        "strict_script_wrapper": [],
    }

    report = guardrail.verify_inventory(inventory, allowlist)

    assert report["status"] == "passed"
    assert report["summary"]["allowlisted_legacy_mixed"] == 1
    assert report["summary"]["allowlisted_dynamic_script_import"] == 1
    assert report["violations"] == []


def test_unallowlisted_legacy_mixed_is_violation() -> None:
    inventory = _inventory(
        _file("tests/test_legacy.py", "legacy-mixed", dynamic_script_import=True)
    )
    allowlist = {
        "legacy_mixed": [],
        "dynamic_script_import": [],
        "strict_application": [],
        "strict_domain": [],
        "strict_infrastructure": [],
        "strict_script_wrapper": [],
    }

    report = guardrail.verify_inventory(inventory, allowlist)

    assert report["status"] == "failed"
    assert {item["code"] for item in report["violations"]} == {
        "unallowlisted_legacy_mixed",
        "unallowlisted_dynamic_script_import",
    }


def test_strict_application_rejects_infrastructure_import() -> None:
    inventory = _inventory(
        _file(
            "tests/test_use_case.py",
            "infrastructure",
            imports_application=True,
            imports_infrastructure=True,
        )
    )
    allowlist = {
        "legacy_mixed": [],
        "dynamic_script_import": [],
        "strict_application": ["tests/test_use_case.py"],
        "strict_domain": [],
        "strict_infrastructure": [],
        "strict_script_wrapper": [],
    }

    report = guardrail.verify_inventory(inventory, allowlist)

    assert report["status"] == "failed"
    assert any(
        item["code"] == "application_forbidden_imports_infrastructure"
        for item in report["violations"]
    )


def test_strict_domain_rejects_application_import() -> None:
    inventory = _inventory(
        _file("tests/test_domain.py", "application", imports_domain=True, imports_application=True)
    )
    allowlist = {
        "legacy_mixed": [],
        "dynamic_script_import": [],
        "strict_application": [],
        "strict_domain": ["tests/test_domain.py"],
        "strict_infrastructure": [],
        "strict_script_wrapper": [],
    }

    report = guardrail.verify_inventory(inventory, allowlist)

    assert report["status"] == "failed"
    assert any(
        item["code"] == "domain_forbidden_imports_application" for item in report["violations"]
    )


def test_render_markdown_includes_summary_and_violations() -> None:
    report = {
        "schema_version": "test.v1",
        "status": "failed",
        "summary": {
            "total_test_files": 1,
            "violations": 1,
            "allowlisted_legacy_mixed": 0,
            "allowlisted_dynamic_script_import": 0,
            "strict_application": 1,
            "strict_domain": 0,
            "strict_infrastructure": 0,
            "strict_script_wrapper": 0,
        },
        "violations": [
            {
                "path": "tests/test_use_case.py",
                "code": "application_forbidden_imports_infrastructure",
                "message": "strict application test imports an outward layer or script surface",
            }
        ],
    }

    rendered = guardrail.render_markdown(report)

    assert "Status: `failed`" in rendered
    assert "application_forbidden_imports_infrastructure" in rendered
