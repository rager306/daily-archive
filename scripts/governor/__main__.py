#!/usr/bin/env python3
"""CLI entry point for governor — hexagonal trajectory enforcement.

Usage:
    python3 -m governor [--phase preflight|active|closeout] [--root PATH]
    python3 -m governor --list-checks
    python3 -m governor --audit-artifacts
"""

import argparse
import json
import os
import re
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
        description="Unified control CLI — trajectory, architecture, docs drift.",
    )
    sub = parser.add_subparsers(dest="command")

    # ── Subcommand: architecture ─────────────────────────────────────
    arch = sub.add_parser("architecture", help="Rust architecture gate (fmt+clippy+hexagonal+tests)")
    arch.add_argument("--quick", action="store_true", help="Skip per-crate tests (fmt+clippy only)")

    # ── Subcommand: docs-drift ────────────────────────────────────────
    sub.add_parser("docs-drift", help="Check CLI-rendered doc tables in GRAPH-SCHEMA.md")

    # ── Subcommand: doctor ────────────────────────────────────────────
    doctor = sub.add_parser("doctor", help="Full repository health check (all gates)")

    # ── Subcommand: trajectory (explicit, same as default) ────────────
    traj = sub.add_parser("trajectory", help="Trajectory check (same as default)")
    traj.add_argument("--phase", choices=["preflight", "active", "closeout"], default="preflight")
    traj.add_argument("--root", default=None)
    traj.add_argument("--check-only", action="store_true")

    # ── Subcommand: review (read-only review registry validation/status) ──
    review = sub.add_parser("review", help="Validate a controlled evidence registry; review prose itself is never a work source")
    review.add_argument("--root", default=None)
    review.add_argument("--registry", default="scripts/governor/checks/grobid_absorption_review.yaml")

    # ── Subcommand: hygiene (GSD runtime-store retention, dry run by default) ──
    hygiene = sub.add_parser("hygiene", help="Report and prune GSD runtime stores (dry run by default)")
    hygiene.add_argument("--root", default=None)
    hygiene.add_argument("--tmp-dir", default="/tmp", help="scratch directory to sweep")
    hygiene.add_argument("--apply", action="store_true", help="perform deletions (default: dry run)")
    hygiene.add_argument("--days-exec", type=int, default=None, help="retention for .gsd/exec evidence")
    hygiene.add_argument("--days-quarantine", type=int, default=None, help="retention for quarantined projections")
    hygiene.add_argument("--days-activity", type=int, default=None, help="retention for .gsd/activity")
    hygiene.add_argument("--min-age-hours", type=int, default=None, help="never touch anything newer than this")
    hygiene.add_argument("--no-scratch", action="store_true", help="keep /tmp scratch directories")

    # ── Subcommand: doctor-baseline (accepted engine-doctor divergence) ──
    baseline = sub.add_parser(
        "doctor-baseline",
        help="List unbaselined engine-doctor divergence, or accept one after review",
    )
    baseline.add_argument("--root", default=None)
    baseline.add_argument("--accept", default=None, help="divergence code to record as accepted")
    baseline.add_argument("--milestone", default=None, help="milestone the acceptance is scoped to")
    baseline.add_argument(
        "--code-wide",
        action="store_true",
        help="accept the code for every milestone (requires --reason)",
    )
    baseline.add_argument("--reason", default=None, help="why a code-wide acceptance is safe")
    baseline.add_argument("--evidence", default=None, help="what was checked/where the file went")
    baseline.add_argument("--by", default="agent", choices=["agent", "human"])

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
    # ── Route subcommands ─────────────────────────────────────────
    if args.command == "architecture":
        return _run_architecture_gate(args)
    elif args.command == "docs-drift":
        return _run_docs_drift()
    elif args.command == "doctor":
        return _run_doctor(args)
    elif args.command == "trajectory":
        # Re-parse with trajectory-specific args
        args.phase = args.phase
        args.root = args.root
        args.check_only = args.check_only
        # fall through to default trajectory logic
    elif args.command == "review":
        return _run_review_registry(args)
    elif args.command == "hygiene":
        return _run_hygiene(args)
    elif args.command == "doctor-baseline":
        return _run_doctor_baseline(args)


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

    # D491 / R141: expose the effective active-research policy the report ran
    # under, so the authorization DATA is a consumed policy rather than an
    # unread file. Absent section = C3/rights-review gates still in force.
    for policy in getattr(report, "active_policy", None) or []:
        print(
            "  active policy: "
            f"{policy.get('policy_id')} ({policy.get('recorded_decision')}) "
            f"c3_process_gates={policy.get('c3_process_gates')} "
            f"independent_rights_review_wait={policy.get('independent_rights_review_wait')} "
            f"graph_locks={policy.get('graph_locks')} "
            f"third_party_legal_rights={policy.get('third_party_legal_rights')} "
            f"quality_pass={policy.get('quality_pass')} "
            f"historical_c3={policy.get('historical_c3_decision')} "
            f"active_blocker={policy.get('historical_c3_active_blocker')}"
        )

    # Exit code: 0 = on_track or drift_risk, 1 = blocked.
    # Handle both str and Enum(str) verdict values.
    verdict_value = report.verdict.value if hasattr(report.verdict, "value") else str(report.verdict)
    if verdict_value == "blocked":
        return 1
    return 0


def _resolve_review_contract(project_root: Path, registry: str | None) -> str | None:
    """Resolve a review contract argument against the committed contract home.

    `--registry` is a path argument, so a bare filename such as
    `grobid_absorption_review.yaml` would otherwise be read as `<root>/<name>`
    and fail closed as a missing contract even though the contract exists at
    its committed home `scripts/governor/checks/`. A bare filename therefore
    resolves to that home; every other form (repo-relative, absolute) is
    passed through unchanged, so a contract that genuinely does not exist
    still fails closed instead of silently selecting a different registry.
    """
    if not registry:
        return registry
    requested = Path(registry)
    if requested.is_absolute() or requested.parent != Path("."):
        return registry
    committed = Path("scripts") / "governor" / "checks" / requested.name
    if (project_root / committed).is_file():
        return str(committed)
    return registry


def _run_hygiene(args) -> int:
    """Report and (with --apply) prune GSD runtime stores. Dry run by default."""
    from dataclasses import replace

    from governor.application.retention_service import (
        apply_plan,
        build_plan,
        load_retention_policy,
        store_report,
    )

    project_root = Path(args.root).resolve() if getattr(args, "root", None) else _detect_root()
    tmp_dir = Path(getattr(args, "tmp_dir", "/tmp"))
    if not (project_root / ".gsd").exists():
        print(f"not a GSD project: {project_root}", file=sys.stderr)
        return 1

    # The contract owns the policy so the flag that fires and the command that clears
    # it cannot disagree; an explicit CLI flag is a deliberate one-off override.
    policy = load_retention_policy(project_root)
    overrides = {
        key: value
        for key, value in (
            ("days_exec", args.days_exec),
            ("days_quarantine", args.days_quarantine),
            ("days_activity", args.days_activity),
            ("min_age_hours", args.min_age_hours),
        )
        if value is not None
    }
    if overrides:
        policy = replace(policy, **overrides)

    sizes = store_report(project_root, tmp_dir)
    print("GSD runtime stores:")
    for name, data in sizes.items():
        print(f"  {name:12s} {int(data['files']):7d} files  {data['mb']:9.1f} MB")
    print(
        f"policy: exec {policy.days_exec}d, quarantine {policy.days_quarantine}d, "
        f"activity {policy.days_activity}d, min age {policy.min_age_hours}h"
    )

    plan = build_plan(
        project_root,
        policy,
        tmp_dir=tmp_dir,
        include_scratch=not getattr(args, "no_scratch", False),
    )
    counts = plan.counts()
    print(
        "plan: "
        f"{counts['scratch_dirs']} scratch dirs, {counts['exec_captures']} exec captures, "
        f"{counts['quarantine_runs']} quarantine runs, {counts['activity_files']} activity files "
        f"({plan.total_bytes() / 1e6:.1f} MB)"
    )
    if not getattr(args, "apply", False):
        print("dry run — pass --apply to delete")
        return 0
    if plan.is_empty():
        print("nothing to remove")
        return 0
    manifest = apply_plan(plan, project_root)
    print(f"removed; manifest at {manifest}")
    return 0


def _run_doctor_baseline(args) -> int:
    """List unbaselined doctor divergence, or record one explicit acceptance."""
    from governor.application.doctor_baseline_service import (
        BaselineError,
        accept,
        report,
    )

    project_root = Path(args.root).resolve() if getattr(args, "root", None) else _detect_root()

    if getattr(args, "accept", None):
        try:
            entry = accept(
                project_root,
                code=args.accept,
                milestone=getattr(args, "milestone", None),
                code_wide=bool(getattr(args, "code_wide", False)),
                reason=getattr(args, "reason", None),
                evidence=getattr(args, "evidence", None),
                recorded_by=getattr(args, "by", "agent"),
            )
        except BaselineError as error:
            print(f"refused: {error}", file=sys.stderr)
            return 1
        print(f"accepted: {json.dumps(entry, ensure_ascii=False)}")
        return 0

    try:
        current = report(project_root)
    except BaselineError as error:
        print(f"baseline unreadable: {error}", file=sys.stderr)
        return 1
    print(
        f"latest doctor run: {current.latest_ts or 'none'} "
        f"({current.latest_errors} error(s) counted, {current.accepted_count} acceptance(s) recorded)"
    )
    if not current.unbaselined:
        print("no unbaselined divergence")
        return 0
    print("unbaselined divergence:")
    for label in current.unbaselined:
        print(f"  - {label}")
    print("codes reported by the latest run:", ", ".join(current.available_codes) or "(none)")
    print(
        "accept one after review: governor doctor-baseline --accept <code> "
        "--milestone M### --evidence \"...\""
    )
    return 0


def _run_review_registry(args) -> int:
    """Validate a review registry and report unresolved/enclosed counts (read-only)."""
    from governor.adapters.checks.review_registry_dispatch import check_review_registry

    project_root = Path(args.root).resolve() if args.root else _detect_root()
    registry = _resolve_review_contract(project_root, getattr(args, "registry", None))
    flags = check_review_registry(project_root, registry)
    status_flags = [f for f in flags if f.flag.endswith("_status")]
    invalid = [f for f in flags if f.flag.endswith(("_invalid", "_missing"))]
    broken = [f for f in flags if f.flag.endswith("_broken")]
    for flag in flags:
        print(f"  - {flag.flag} [{flag.severity}]: {flag.evidence}")
    if invalid or broken:
        print("review registry: INVALID", file=sys.stderr)
        return 2
    if status_flags:
        print(f"review registry: {status_flags[-1].evidence}")
    else:
        print("review registry: no status")
    return 0


def _validate_yaml(rule_repo) -> int:
    """Validate all YAML rule/dimension files load without error."""
    try:
        rules = rule_repo.load_adr_rules()
        dims = rule_repo.load_dimensions()
    except Exception as e:
        print(f"YAML validation FAILED: {e}", file=sys.stderr)
        return 1
    dim_ids = ", ".join(str(d.get("id", "?")) for d in dims)
    print(f"YAML validation PASS: {len(rules)} rules, "
          f"{len(dims)} dimensions ({dim_ids})")
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




# Wall-clock ceiling for one architecture-gate step. A hung step must fail,
# not silently truncate: `subprocess.TimeoutExpired` is caught below and
# reported as TIMEOUT, so a timed-out capture can never be read as PASS.
# 3600s is deliberately generous — it catches a genuine hang without
# false-failing a slow-but-progressing step. The gate as a whole runs longer
# than one gsd_exec capture (600s), so run it detached and poll; this ceiling
# is a hang guard, not a time budget.
_ARCHITECTURE_STEP_TIMEOUT_S = 3600


def architecture_gate_steps(quick: bool = False) -> list[tuple[str, list[str], int]]:
    """Architecture-gate steps as DATA: (name, argv, timeout_seconds).

    Exposed as a function so the test inventory
    (`scripts/tests/test_absorption_test_inventory.py`) can assert which test
    targets and feature combinations the gate really executes instead of
    trusting a comment. `_run_architecture_gate` is the only runner.
    """
    t = _ARCHITECTURE_STEP_TIMEOUT_S
    steps: list[tuple[str, list[str], int]] = [
        ("cargo fmt (check)", ["cargo", "fmt", "-p", "kg-ontology", "-p", "kg-storage",
                                "-p", "kg-embeddings", "-p", "kg-algorithms", "-p", "kg-pipeline",
                                "-p", "da-domain", "-p", "da-ports", "-p", "da-application",
                                "-p", "da-adapters", "-p", "da-parse", "-p", "da-cli", "-p", "da-mcp",
                                "--", "--check"], t),
        ("cargo clippy (lib)", ["cargo", "clippy",
                                 "-p", "kg-ontology", "-p", "kg-storage", "-p", "kg-embeddings",
                                 "-p", "kg-algorithms", "-p", "kg-pipeline",
                                 "-p", "da-domain", "-p", "da-ports", "-p", "da-application",
                                 "-p", "da-adapters", "-p", "da-parse", "-p", "da-cli", "-p", "da-mcp",
                                 "--no-deps", "--", "-D", "warnings"], t),
        ("cargo clippy (tests)", ["cargo", "clippy",
                                   "-p", "kg-ontology", "-p", "kg-storage", "-p", "kg-embeddings",
                                   "-p", "kg-algorithms", "-p", "kg-pipeline",
                                   "-p", "da-domain", "-p", "da-ports", "-p", "da-application",
                                   "-p", "da-adapters", "-p", "da-parse", "-p", "da-cli", "-p", "da-mcp",
                                   "--lib", "--tests", "--no-deps", "--", "-D", "warnings"], t),
        ("feature-matrix build", ["cargo", "check", "-p", "da-cli",
                                   "--features", "pdf-classifier", "--examples"], t),
    ]

    if quick:
        return steps

    steps.extend([
            ("kg-ontology tests", ["cargo", "test", "-p", "kg-ontology", "--lib"], t),
            ("kg-storage tests", ["cargo", "test", "-p", "kg-storage", "--lib"], t),
            ("kg-embeddings tests", ["cargo", "test", "-p", "kg-embeddings", "--lib"], t),
            ("kg-algorithms tests", ["cargo", "test", "-p", "kg-algorithms", "--lib"], t),
            ("kg-pipeline tests", ["cargo", "test", "-p", "kg-pipeline", "--lib"], t),
            ("da-domain tests", ["cargo", "test", "-p", "da-domain", "--lib"], t),
            # `--tests`, not `--lib`: the integration test binaries under
            # `crates/da-parse/tests/` were never executed by this gate — the
            # old `--lib` step ran only the unit tests, so every cross-module
            # contract compiled and never ran (M514/S01 context: "Governor
            # da-parse --lib пропускает integration tests"). `--tests` includes
            # the lib target, so the parse-plane article locks still run.
            ("da-parse tests", ["cargo", "test", "-p", "da-parse", "--tests"], t),
            ("da-application tests", ["cargo", "test", "-p", "da-application", "--tests"], t),
            ("da-adapters tests", ["cargo", "test", "-p", "da-adapters", "--tests"], t),
            # The da-cli tests below exec example binaries out of
            # `target/debug/examples/` and `cargo test --tests` does NOT build
            # examples, so they must be provisioned explicitly first. Only these
            # seven are executed by the ungated test files:
            #   acceptance_evidence_test, cross_stage_semantic_invariants_example,
            #   derived_maturity_rehearsal_example, external_holdout_capture_test,
            #   native_family_gate_rehearsal_test, parse_batch_budget_shadow_test,
            #   rest_batch_asset_shadow_test.
            # Building `--examples` would compile all ~86 example runners, each
            # statically linking the dependency tree (~0.3G per binary); naming
            # the seven keeps the coverage and drops the rest. Add a name here
            # when a new ungated test starts driving an example.
            # NOTE: policy_ablation_artifact_test is NOT in this list on purpose -
            # it is `#![cfg(feature = "learning-evaluation")]` and its example
            # `parse_policy_ablation` carries required-features=["learning-evaluation"],
            # so naming it here fails the whole build command. A cargo build
            # command fails as a unit, so one unbuildable name silently drops
            # every other example in the same step. The learning-evaluation
            # runner therefore stays OUT of this default-feature step and is
            # provisioned by the feature-enabled step at the end of this list.
            ("da-cli examples (default)", ["cargo", "build", "-p", "da-cli",
                                            "--example", "acceptance_evidence",
                                            "--example", "cross_stage_semantic_invariants",
                                            "--example", "derived_maturity_rehearsal",
                                            "--example", "external_holdout_capture",
                                            "--example", "native_family_gate_rehearsal",
                                            "--example", "parse_batch_budget_shadow",
                                            "--example", "rest_batch_asset_shadow"], t),
            ("da-cli tests", ["cargo", "test", "-p", "da-cli", "--tests"], t),
            # Feature-gated example contracts. Twelve da-cli test files declare
            # `#![cfg(feature = "pdf-layout")]`; a plain
            # `cargo test -p da-cli --tests` compiles them to empty targets, so
            # without these steps their coverage is nominal rather than real.
            # `cargo test --test NAME` does NOT build examples, and neither does
            # `cargo test --example NAME` — that leaves a *libtest harness* which
            # rejects the example's own `--manifest` flag (native_visual_heads
            # documents this), so a plain `cargo build --example X` is the only
            # correct provisioning. Only these runners are driven by the gated
            # tests: building `--examples` compiled all ~86 runners for
            # ~25G of target/debug/examples, which is what this list avoids. Add
            # a name here when a new gated test starts driving an example.
            # `java_free_vertical_slice` (S03/T04) is provisioned here by name so
            # its binary exists for the S03/T05 contract test and for the
            # receipt emitter itself; it is a layout example like its siblings.
            # `native_family_parity` (S04/T04) is the sibling receipt emitter for
            # the header/bibliography families and is provisioned by name for the
            # same reason.
            # `native_exclusion_recovery` (S04/T05) is the third S04 receipt
            # emitter (the 79 typed exclusions); provisioning it by name keeps
            # every gated receipt emitter built under the gate feature rather
            # than compiling to a silently-missing binary.
            # Both steps run LAST: enabling the feature here would otherwise
            # force a rebuild for every step above.
            ("da-cli feature-gated example runners", ["cargo", "build", "-p", "da-cli",
                                                        "--features", "pdf-layout",
                                                        "--example", "callout_matching_shadow",
                                                        "--example", "consolidation_specialized_shadow",
                                                        "--example", "document_segmentation_shadow",
                                                        "--example", "external_holdout_release_probe",
                                                        "--example", "feature_tagger_training_shadow",
                                                        "--example", "fulltext_block_semantics_shadow",
                                                        "--example", "java_free_vertical_slice",
                                                        "--example", "layout_bakeoff",
                                                        "--example", "native_exclusion_recovery",
                                                        "--example", "native_family_parity",
                                                        "--example", "native_fulltext_composition_shadow",
                                                        "--example", "native_fulltext_corpus_shadow",
                                                        "--example", "native_visual_heads_shadow"], t),
            ("da-cli feature-gated example contracts", ["cargo", "test", "-p", "da-cli",
                                                         "--features", "pdf-layout", "--tests"], t),
            # learning-evaluation is the second feature-gated plane. The test
            # file `policy_ablation_artifact_test.rs` is
            # `#![cfg(feature = "learning-evaluation")]` and its runner example
            # `parse_policy_ablation` carries
            # required-features=["learning-evaluation"], so neither the default
            # `cargo test -p da-cli --tests` nor a feature-less `--example` build
            # reaches it: the first compiles an EMPTY test target, the second
            # fails the whole command. Every example carrying the feature is
            # provisioned by NAME — `--examples` would compile all ~86 runners,
            # which is what the lists above avoid. This closes the gap the
            # default-examples comment used to record as "out of this change".
            #
            # `parse_learning_cycle` (S07/T03) was added to da-cli with the same
            # `required-features = ["learning-evaluation"]` but was NOT added
            # here, so scripts/tests/test_absorption_test_inventory.py stayed red
            # after S07 closed. Named explicitly for the same reason as its two
            # siblings above.
            ("da-cli learning-evaluation example runners", ["cargo", "build", "-p", "da-cli",
                                                              "--features", "learning-evaluation",
                                                              "--example", "parse_policy_ablation",
                                                              "--example", "parse_learning_ablation",
                                                              "--example", "parse_learning_cycle"], t),
            ("da-cli learning-evaluation tests", ["cargo", "test", "-p", "da-cli",
                                                   "--features", "learning-evaluation", "--tests"], t),
            # `native_content_receipt` (S05/T06) is the content receipt emitter and
            # the only da-cli example that needs the `pdf-classifier` plane in
            # addition to `pdf-layout` (it partitions the corpus by `PdfType`).
            # It is provisioned in its own step with both features rather than the
            # layout-only list above, because a cargo command fails as a unit: a
            # name whose required feature is off would silently drop the step.
            ("da-cli content-receipt example runner", ["cargo", "build", "-p", "da-cli",
                                                         "--features", "pdf-layout,pdf-classifier",
                                                         "--example", "native_content_receipt"], t),
            # `native_route_e2e` (S06/T05) is the Java-forbidden end-to-end
            # native-route runner. Like `native_content_receipt` it declares
            # `pdf-classifier` on top of `pdf-layout`, so it is provisioned in
            # its own two-feature step rather than the layout-only list above.
            # A cargo command fails as a unit, so the name is placed only in a
            # step whose `--features` cover its manifest `required-features`;
            # the inventory test asserts that binding.
            ("da-cli route-e2e example runner", ["cargo", "build", "-p", "da-cli",
                                                   "--features", "pdf-layout,pdf-classifier",
                                                   "--example", "native_route_e2e"], t),
            # `native_corpus_acceptance` (S08/T01) is the corpus-bound native
            # acceptance measurement runner. Like `native_content_receipt` and
            # `native_route_e2e` it declares `pdf-classifier` on top of
            # `pdf-layout`, so it is provisioned in its own two-feature step
            # rather than the layout-only list above. A cargo command fails as a
            # unit, so the name is placed only in a step whose `--features`
            # cover its manifest `required-features`; the inventory test asserts
            # that binding.
            ("da-cli native-acceptance example runner", ["cargo", "build", "-p", "da-cli",
                                                           "--features", "pdf-layout,pdf-classifier",
                                                           "--example", "native_corpus_acceptance"], t),
        ])
    return steps


#: Environment override for the directory a failing gate step's full capture is
#: written to. The default is `<root>/target/architecture-gate-logs`, which is
#: gitignored with `target/`, so a failure leaves durable stdout/stderr beside the
#: build without ever being added to the tree.
_ARCHITECTURE_GATE_LOG_ENV = "GOVERNOR_ARCHITECTURE_GATE_LOG_DIR"

#: libtest prints `test <name> ... FAILED` per failing test and repeats the names
#: under a `failures:` heading. Matching the per-test status line identifies the
#: test directly and can never mistake a passing `... ok` line for a failure.
_GATE_FAILING_TEST_RE = re.compile(
    r"^\s*test\s+(?P<name>\S+)\s+\.\.\.\s+FAILED\s*$", re.MULTILINE
)

#: A compile failure or a panic. Used only when no libtest status line matched:
#: a cargo step that never reached the harness prints no test name.
_GATE_ERROR_RE = re.compile(
    r"^\s*(?:error(?:\[[A-Za-z0-9]+\])?:\s*\S.*|"
    r".*panicked at\s+\S.*|process didn't exit successfully.*)$",
    re.MULTILINE,
)


def _architecture_gate_log_dir(root: Path) -> Path:
    """Where a failing step's full stdout/stderr is persisted."""
    override = os.environ.get(_ARCHITECTURE_GATE_LOG_ENV)
    if override:
        return Path(override)
    return root / "target" / "architecture-gate-logs"


def _as_text(value: object) -> str:
    """A subprocess capture as text; ``TimeoutExpired`` may hand back bytes."""
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", "replace")
    return value if isinstance(value, str) else str(value)


def _failing_tests(output: str) -> list[str]:
    """libtest ``test <name> ... FAILED`` names, in first-seen order."""
    names: list[str] = []
    for match in _GATE_FAILING_TEST_RE.finditer(output):
        name = match.group("name")
        if name not in names:
            names.append(name)
    return names


def _first_gate_error(output: str) -> str | None:
    """The first compile/panic line, when no libtest failing test was printed."""
    match = _GATE_ERROR_RE.search(output)
    return match.group(0).strip() if match else None


def _slug(text: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return slug or "step"


def _persist_gate_capture(
    log_dir: Path, index: int, name: str, stdout: str, stderr: str
) -> tuple[Path, Path]:
    """Write one failing step's full stdout and stderr; return both paths."""
    log_dir.mkdir(parents=True, exist_ok=True)
    stem = f"{index:02d}-{_slug(name)}"
    stdout_path = log_dir / f"{stem}.stdout.log"
    stderr_path = log_dir / f"{stem}.stderr.log"
    stdout_path.write_text(stdout, encoding="utf-8")
    stderr_path.write_text(stderr, encoding="utf-8")
    return stdout_path, stderr_path


def _report_gate_step_failure(
    name: str,
    index: int,
    stdout: str,
    stderr: str,
    *,
    log_dir: Path,
    timed_out: bool,
) -> None:
    """Print and persist everything needed to identify one failing step.

    The gate previously printed only the last five stderr lines and discarded
    the capture, which is exactly why the first-run failure recorded in
    ``doc/review/grobid-absorption-progress-baseline.md`` could not be attributed
    to a test. The full stdout and stderr of the failing step are now persisted
    (gitignored), the failing libtest names are extracted, and both stream tails
    are shown, so the next failure identifies itself instead of its hypothesis.
    """
    stdout_path, stderr_path = _persist_gate_capture(log_dir, index, name, stdout, stderr)
    combined = f"{stdout}\n{stderr}"
    failing = _failing_tests(combined)
    if failing:
        shown = ", ".join(failing[:10])
        remainder = f" (+{len(failing) - 10} more)" if len(failing) > 10 else ""
        print(f"    failing tests: {shown}{remainder}")
    elif not timed_out:
        first_error = _first_gate_error(combined)
        if first_error:
            print(f"    failure: {first_error}")
    for stream, label in ((stdout, "stdout"), (stderr, "stderr")):
        for line in [line for line in stream.strip().splitlines() if line.strip()][-5:]:
            print(f"    [{label}] {line}")
    print(f"    full capture: {stdout_path} | {stderr_path}")


def _run_architecture_gate(args) -> int:
    """Absorb scripts/verify_rust_architecture.sh — Rust architecture gate."""
    import subprocess

    root = Path(__file__).resolve().parent.parent.parent
    steps = architecture_gate_steps(bool(getattr(args, "quick", False)))
    log_dir = _architecture_gate_log_dir(root)

    failed = 0
    timed_out = 0
    for index, (name, cmd, timeout_s) in enumerate(steps, start=1):
        print(f"  {name}...", end=" ", flush=True)
        try:
            result = subprocess.run(
                cmd, cwd=root, capture_output=True, text=True, timeout=timeout_s,
            )
        except subprocess.TimeoutExpired as exc:
            # A timeout is a failure whatever a truncated capture would say:
            # never report a step that never finished as OK.
            print(f"TIMEOUT (>{timeout_s}s)")
            _report_gate_step_failure(
                name,
                index,
                _as_text(getattr(exc, "stdout", None)),
                _as_text(getattr(exc, "stderr", None)),
                log_dir=log_dir,
                timed_out=True,
            )
            failed = 1
            timed_out += 1
            continue
        if result.returncode == 0:
            print("OK")
            # A passing step's capture is not evidence for anything and can be
            # large; only a failing step's bytes are persisted.
            continue
        print("FAIL")
        _report_gate_step_failure(
            name,
            index,
            _as_text(result.stdout),
            _as_text(result.stderr),
            log_dir=log_dir,
            timed_out=False,
        )
        failed = 1

    print(f"\n  architecture gate steps: {len(steps) - failed}/{len(steps)} passed, "
          f"{timed_out} timed out")
    if failed:
        print("\n  ARCHITECTURE GATE: FAIL")
    else:
        print("\n  ARCHITECTURE GATE: PASS")
    return failed


def _run_docs_drift() -> int:
    """Absorb scripts/docs_drift_check.sh — doc table drift detection."""
    import subprocess
    root = Path(__file__).resolve().parent.parent.parent
    
    result = subprocess.run(
        ["bash", str(root / "scripts" / "docs_drift_check.sh")],
        cwd=root, capture_output=True, text=True,
    )
    print(result.stdout)
    if result.returncode != 0:
        print(result.stderr)
    return result.returncode


def _run_doctor(args) -> int:
    """Full repository health: trajectory + architecture + docs + artifacts."""
    print("=== GOVERNOR DOCTOR ===\n")
    
    print("--- 1. Trajectory ---")
    # Reuse the canonical trajectory composition (same as the default path).
    from governor.application import AuditService, TrajectoryService
    from governor.adapters import (
        FilesystemADRRepository, FilesystemCodeScanner,
        FilesystemGsdPlanningReader, FilesystemMilestoneReader,
        GitReader, YAMLRuleRepository,
    )
    from pathlib import Path as _P
    root = _P(args.root) if args.root else Path.cwd()
    checks_dir = Path(__file__).resolve().parent / "checks"

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    service = TrajectoryService(
        project_root=root,
        audit_service=AuditService(scanner=scanner, rule_repo=rule_repo),
        rule_repo=rule_repo,
        adr_repo=FilesystemADRRepository(root),
        milestone_reader=FilesystemMilestoneReader(root),
        gsd_planning_reader=FilesystemGsdPlanningReader(root),
        git_reader=GitReader(root),
    )
    report = service.build_report(phase="closeout")
    print(f"  Verdict: {report.verdict}")
    for flag in report.drift_flags:
        print(f"  [{flag.get('severity')}] {flag.get('flag')}: {str(flag.get('evidence'))[:100]}")
    
    print("\n--- 2. Architecture ---")
    arch_result = _run_architecture_gate(args)
    
    print("\n--- 3. Docs drift ---")
    docs_result = _run_docs_drift()
    
    overall = max(arch_result, docs_result, 0 if report.verdict.value == "on_track" else 1)
    print(f"\n=== DOCTOR: {'PASS' if overall == 0 else 'FAIL'} ===")
    return overall


if __name__ == "__main__":
    sys.exit(main())
