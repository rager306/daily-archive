#!/usr/bin/env python3
"""Verify test-layer boundary status with a ratchetable allowlist.

The checker is intentionally narrower than the inventory. It allows known legacy
mixed tests from an explicit allowlist and enforces strict rules only for files
listed in strict_* pilot sets.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from audit_test_architecture import (  # pyrefly: ignore [missing-import]
    DEFAULT_OUTPUT_DIR,
    DEFAULT_TESTS_DIR,
    build_inventory,
)

SCHEMA_VERSION = "daily-archive-test-architecture-guardrail.v1"
DEFAULT_ALLOWLIST = DEFAULT_OUTPUT_DIR / "test-architecture-allowlist.json"

BLOCKED_FOR_APPLICATION = (
    "imports_infrastructure",
    "imports_workflows",
    "imports_cli",
    "imports_pipeline_legacy",
    "imports_scripts_normal",
    "dynamic_script_import",
    "subprocess_script_invocation",
)
BLOCKED_FOR_DOMAIN = (
    "imports_application",
    "imports_infrastructure",
    "imports_workflows",
    "imports_cli",
    "imports_pipeline_legacy",
    "imports_scripts_normal",
    "dynamic_script_import",
    "subprocess_script_invocation",
)
BLOCKED_FOR_INFRASTRUCTURE = (
    "imports_scripts_normal",
    "dynamic_script_import",
)
BLOCKED_FOR_WORKFLOWS = (
    "imports_scripts_normal",
    "dynamic_script_import",
    "subprocess_script_invocation",
)


def load_allowlist(path: Path = DEFAULT_ALLOWLIST) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def verify_inventory(inventory: dict[str, Any], allowlist: dict[str, Any]) -> dict[str, Any]:
    files = inventory["files"]
    by_path = {item["path"]: item for item in files}
    legacy_allow = set(allowlist.get("legacy_mixed", []))
    dynamic_allow = set(allowlist.get("dynamic_script_import", []))
    strict_application = set(allowlist.get("strict_application", []))
    strict_domain = set(allowlist.get("strict_domain", []))
    strict_infrastructure = set(allowlist.get("strict_infrastructure", []))
    strict_workflows = set(allowlist.get("strict_workflows", []))
    strict_script_wrapper = set(allowlist.get("strict_script_wrapper", []))

    violations: list[dict[str, Any]] = []
    for item in files:
        path = item["path"]
        signals = item["signals"]
        if item["bucket"] == "legacy-mixed" and path not in legacy_allow and path not in strict_workflows:
            violations.append(
                violation(path, "unallowlisted_legacy_mixed", "legacy-mixed test is not allowlisted")
            )
        if signals.get("dynamic_script_import") and path not in dynamic_allow:
            violations.append(
                violation(
                    path,
                    "unallowlisted_dynamic_script_import",
                    "dynamic script import test is not allowlisted",
                )
            )

    for path in sorted(strict_application):
        item = by_path.get(path)
        if item is None:
            violations.append(violation(path, "missing_strict_application", "strict file missing"))
            continue
        if not item["signals"].get("imports_application"):
            violations.append(
                violation(path, "application_missing_application_import", "strict application test should import application layer")
            )
        for signal in BLOCKED_FOR_APPLICATION:
            if item["signals"].get(signal):
                violations.append(
                    violation(path, f"application_forbidden_{signal}", "strict application test imports an outward layer or script surface")
                )

    for path in sorted(strict_domain):
        item = by_path.get(path)
        if item is None:
            violations.append(violation(path, "missing_strict_domain", "strict file missing"))
            continue
        if not item["signals"].get("imports_domain"):
            violations.append(
                violation(path, "domain_missing_domain_import", "strict domain test should import domain layer")
            )
        for signal in BLOCKED_FOR_DOMAIN:
            if item["signals"].get(signal):
                violations.append(
                    violation(path, f"domain_forbidden_{signal}", "strict domain test imports an outward layer or script surface")
                )

    for path in sorted(strict_infrastructure):
        item = by_path.get(path)
        if item is None:
            violations.append(violation(path, "missing_strict_infrastructure", "strict file missing"))
            continue
        if not item["signals"].get("imports_infrastructure"):
            violations.append(
                violation(path, "infrastructure_missing_infrastructure_import", "strict infrastructure test should import infrastructure layer")
            )
        for signal in BLOCKED_FOR_INFRASTRUCTURE:
            if item["signals"].get(signal):
                violations.append(
                    violation(path, f"infrastructure_forbidden_{signal}", "strict infrastructure test should not dynamic-import scripts")
                )

    for path in sorted(strict_workflows):
        item = by_path.get(path)
        if item is None:
            violations.append(violation(path, "missing_strict_workflows", "strict file missing"))
            continue
        if not item["signals"].get("imports_workflows"):
            violations.append(
                violation(path, "workflows_missing_workflows_import", "strict workflow test should import workflows layer")
            )
        for signal in BLOCKED_FOR_WORKFLOWS:
            if item["signals"].get(signal):
                violations.append(
                    violation(path, f"workflows_forbidden_{signal}", "strict workflow test should not depend on script surfaces")
                )

    for path in sorted(strict_script_wrapper):
        item = by_path.get(path)
        if item is None:
            violations.append(violation(path, "missing_strict_script_wrapper", "strict file missing"))
            continue
        if item["bucket"] not in {"script-wrapper", "acceptance"}:
            violations.append(
                violation(path, "script_wrapper_bucket_mismatch", "strict script-wrapper test should classify as script-wrapper or acceptance")
            )

    return {
        "schema_version": SCHEMA_VERSION,
        "status": "passed" if not violations else "failed",
        "summary": {
            "total_test_files": inventory["summary"]["total_test_files"],
            "violations": len(violations),
            "allowlisted_legacy_mixed": len(legacy_allow),
            "allowlisted_dynamic_script_import": len(dynamic_allow),
            "strict_application": len(strict_application),
            "strict_domain": len(strict_domain),
            "strict_infrastructure": len(strict_infrastructure),
            "strict_workflows": len(strict_workflows),
            "strict_script_wrapper": len(strict_script_wrapper),
        },
        "violations": violations,
    }


def violation(path: str, code: str, message: str) -> dict[str, str]:
    return {"path": path, "code": code, "message": message}


def write_outputs(report: dict[str, Any], output_dir: Path) -> tuple[Path, Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    json_path = output_dir / "test-architecture-guardrail.json"
    markdown_path = output_dir / "test-architecture-guardrail.md"
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    markdown_path.write_text(render_markdown(report), encoding="utf-8")
    return json_path, markdown_path


def render_markdown(report: dict[str, Any]) -> str:
    summary = report["summary"]
    lines = [
        "# Test Architecture Guardrail",
        "",
        f"Schema: `{report['schema_version']}`",
        f"Status: `{report['status']}`",
        "",
        "## Summary",
        "",
        f"- Total test files: `{summary['total_test_files']}`",
        f"- Violations: `{summary['violations']}`",
        f"- Allowlisted legacy mixed: `{summary['allowlisted_legacy_mixed']}`",
        f"- Allowlisted dynamic script import: `{summary['allowlisted_dynamic_script_import']}`",
        f"- Strict application files: `{summary['strict_application']}`",
        f"- Strict domain files: `{summary['strict_domain']}`",
        f"- Strict infrastructure files: `{summary['strict_infrastructure']}`",
        f"- Strict workflow files: `{summary['strict_workflows']}`",
        f"- Strict script-wrapper files: `{summary['strict_script_wrapper']}`",
        "",
        "## Violations",
        "",
    ]
    if not report["violations"]:
        lines.append("- none")
    else:
        for item in report["violations"]:
            lines.append(f"- `{item['path']}` `{item['code']}` — {item['message']}")
    return "\n".join(lines) + "\n"


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Verify test architecture boundaries.")
    parser.add_argument("--tests-dir", type=Path, default=DEFAULT_TESTS_DIR)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--allowlist", type=Path, default=DEFAULT_ALLOWLIST)
    parser.add_argument("--json", action="store_true", help="Print guardrail JSON to stdout.")
    return parser.parse_args()


def main() -> int:
    args = _parse_args()
    inventory = build_inventory(args.tests_dir)
    allowlist = load_allowlist(args.allowlist)
    report = verify_inventory(inventory, allowlist)
    json_path, markdown_path = write_outputs(report, args.output_dir)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(
            " | ".join(
                [
                    "test architecture guardrail",
                    f"status: {report['status']}",
                    f"violations: {report['summary']['violations']}",
                    f"json: {json_path}",
                    f"markdown: {markdown_path}",
                ]
            )
        )
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
