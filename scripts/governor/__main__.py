#!/usr/bin/env python3
"""CLI entry point for governor — hexagonal trajectory enforcement.

Usage:
    python3 -m governor [--phase preflight|active|closeout] [--root PATH]
    python3 -m governor --list-checks
    python3 -m governor --audit-artifacts
"""

import argparse
import json
import sys
from pathlib import Path

# Ensure scripts/ is on sys.path so `import governor` works when run as script.
_scripts_dir = Path(__file__).resolve().parent.parent
if str(_scripts_dir) not in sys.path:
    sys.path.insert(0, str(_scripts_dir))

from governor.adapters import (
    FilesystemADRRepository,
    FilesystemArtifactReader,
    FilesystemCodeScanner,
    FilesystemGsdPlanningReader,
    FilesystemMilestoneReader,
    GitReader,
    YAMLRuleRepository,
)
from governor.application import AuditService, TrajectoryService


def main() -> int:
    parser = argparse.ArgumentParser(
        prog="governor",
        description="Hexagonal trajectory enforcement CLI.",
    )
    parser.add_argument(
        "--phase",
        choices=["preflight", "active", "closeout"],
        default="preflight",
        help="Phase affects severity weighting (default: preflight).",
    )
    parser.add_argument(
        "--root",
        type=str,
        default=None,
        help="Project root directory (default: auto-detect).",
    )
    parser.add_argument(
        "--list-checks",
        action="store_true",
        help="List all loaded check rules from YAML and exit.",
    )
    parser.add_argument(
        "--audit-artifacts",
        action="store_true",
        help="Show per-artifact field audit (graph_writes_allowed, import_eligible).",
    )
    parser.add_argument(
        "--check-only",
        action="store_true",
        help="Run checks without writing trajectory-report.json (for pre-commit).",
    )
    parser.add_argument(
        "--validate-yaml",
        action="store_true",
        help="Validate all YAML rule/dimension files load without error.",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run governor unit tests (scripts/tests/test_governor.py).",
    )
    parser.add_argument(
        "--check-dead-code",
        action="store_true",
        help="Run cargo clippy per-crate and surface dead_code/unused warnings.",
    )
    args = parser.parse_args()

    project_root = Path(args.root).resolve() if args.root else _detect_root()
    checks_dir = Path(__file__).resolve().parent / "checks"

    # Wire up hexagonal dependencies (composition root).
    scanner = FilesystemCodeScanner(project_root)
    rule_repo = YAMLRuleRepository(checks_dir)
    adr_repo = FilesystemADRRepository(project_root)
    milestone_reader = FilesystemMilestoneReader(project_root)
    gsd_planning_reader = FilesystemGsdPlanningReader(project_root)
    artifact_reader = FilesystemArtifactReader(project_root)
    git_reader = GitReader(project_root)

    if args.validate_yaml:
        return _validate_yaml(rule_repo)

    if args.self_test:
        return _self_test()

    if args.check_dead_code:
        from governor.adapters import ClippyScanner

        scanner = ClippyScanner(project_root)
        return _check_dead_code(scanner)

    if args.list_checks:
        _print_checks(rule_repo)
        return 0

    if args.audit_artifacts:
        _print_artifact_audit(artifact_reader)
        return 0

    # Build and run trajectory report.
    audit_service = AuditService(scanner=scanner, rule_repo=rule_repo)
    trajectory_service = TrajectoryService(
        project_root=project_root,
        audit_service=audit_service,
        rule_repo=rule_repo,
        adr_repo=adr_repo,
        milestone_reader=milestone_reader,
        gsd_planning_reader=gsd_planning_reader,
        git_reader=git_reader,
    )

    report = trajectory_service.build_report(phase=args.phase)

    # Write JSON artifact.
    # Write JSON artifact (unless --check-only, used by pre-commit).
    if not args.check_only:
        output_dir = project_root / "artifacts" / "project-trajectory"
        output_dir.mkdir(parents=True, exist_ok=True)
        json_path = output_dir / "trajectory-report.json"
        json_path.write_text(
            json.dumps(report.to_dict(), indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
    else:
        json_path = project_root / "artifacts" / "project-trajectory" / "trajectory-report.json"

    # Console summary.
    n_flags = len(report.drift_flags)
    n_violations = report.reverse_adr_audit_details.get("violations_count", 0)
    print(
        f"trajectory report: verdict={report.verdict} "
        f"phase={report.phase} flags={n_flags} violations={n_violations}"
    )
    print(f"  report: {json_path}")
    for flag in report.drift_flags:
        name = flag.get("flag") if isinstance(flag, dict) else getattr(flag, "flag", "?")
        sev = flag.get("severity") if isinstance(flag, dict) else getattr(flag, "severity", "")
        ev = flag.get("evidence") if isinstance(flag, dict) else getattr(flag, "evidence", "")
        print(f"  - {name} [{sev}]: {ev}")

    # Exit code: 0 = on_track or drift_risk, 1 = blocked.
    # Handle both str and Enum(str) verdict values.
    verdict_value = report.verdict.value if hasattr(report.verdict, "value") else str(report.verdict)
    if verdict_value == "blocked":
        return 1
    return 0


def _validate_yaml(rule_repo) -> int:
    """Validate all YAML rule/dimension files load without error."""
    try:
        rules = rule_repo.load_adr_rules()
        dims = rule_repo.load_dimensions()
    except Exception as e:
        print(f"YAML validation FAILED: {e}", file=sys.stderr)
        return 1
    print(f"YAML validation PASS: {len(rules)} rules, {len(dims)} dimensions")
    return 0


def _check_dead_code(scanner) -> int:
    """Run cargo clippy per-crate and surface warnings from our code."""
    report = scanner.scan_all()
    if not report.has_warnings:
        print(
            f"Dead-code scan PASS: {report.crates_clean}/{report.crates_scanned} "
            "crates clean (0 warnings in our code)"
        )
        return 0
    print(f"Dead-code scan: {len(report.warnings)} warning(s) in our crates:")
    for w in report.warnings:
        rel = w.file
        # Shorten path for readability.
        if "/crates/" in rel:
            rel = rel[rel.index("/crates/") + 1 :]
        print(f"  [{w.crate}] {rel}:{w.line} :: {w.message}")
    return 1


def _self_test() -> int:
    """Run governor unit tests via subprocess."""
    import subprocess

    test_path = Path(__file__).resolve().parent.parent / "tests" / "test_governor.py"
    if not test_path.exists():
        print(f"Self-test FAILED: {test_path} not found", file=sys.stderr)
        return 1

    scripts_dir = Path(__file__).resolve().parent.parent
    env = {**__import__("os").environ, "PYTHONPATH": str(scripts_dir)}
    result = subprocess.run(
        [sys.executable, str(test_path)],
        capture_output=True,
        text=True,
        env=env,
    )
    # Print test output.
    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)
    return result.returncode


def _detect_root() -> Path:
    """Auto-detect project root by looking for Cargo.toml + .gsd/."""
    p = Path(__file__).resolve().parent
    for _ in range(5):
        if (p / "Cargo.toml").exists() and (p / ".gsd").exists():
            return p
        p = p.parent
    return Path.cwd()


def _print_checks(rule_repo) -> None:
    """Print all loaded check rules."""
    rules = rule_repo.load_adr_rules()
    print(f"=== ADR Audit Rules ({len(rules)}) ===")
    for rule in rules:
        print(f"  [{rule['severity']}] {rule['id']}")
        print(f"    anchor: {rule['anchor']}")
        print(f"    scan: {rule['scan']}")
        print(f"    rationale: {rule['rationale']}")
        print()

    dims = rule_repo.load_dimensions()
    print(f"=== Dimensions ({len(dims)}) ===")
    for dim in dims:
        print(f"  {dim['id']}: {dim.get('description', '')}")
    print()


def _print_artifact_audit(artifact_reader) -> None:
    """Show per-artifact field audit."""
    universal_fields = [
        "graph_writes_allowed",
        "graph_write_allowed",
        "graph_writes_any",
        "import_eligible",
    ]
    print("=== Artifact Field Audit ===")
    violations = 0
    checked = 0
    for path in artifact_reader.iter_artifacts():
        try:
            data = artifact_reader.read_json(path)
        except (json.JSONDecodeError, OSError):
            continue
        checked += 1
        for field in universal_fields:
            if field in data and data[field] is True:
                rel = path.relative_to(artifact_reader.root)
                print(f"  VIOLATION: {rel} :: {field}=true")
                violations += 1
    print(f"\nChecked: {checked} artifacts")
    print(f"Violations: {violations}")
    if violations == 0:
        print("All universal fields are false (clean).")


if __name__ == "__main__":
    sys.exit(main())
