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




def _run_architecture_gate(args) -> int:
    """Absorb scripts/verify_rust_architecture.sh — Rust architecture gate."""
    import subprocess
    root = Path(__file__).resolve().parent.parent.parent
    
    steps = [
        ("cargo fmt (check)", ["cargo", "fmt", "-p", "kg-ontology", "-p", "kg-storage",
                                "-p", "kg-embeddings", "-p", "kg-algorithms", "-p", "kg-pipeline",
                                "-p", "da-domain", "-p", "da-ports", "-p", "da-application",
                                "-p", "da-adapters", "-p", "da-parse", "-p", "da-cli", "-p", "da-mcp",
                                "--", "--check"]),
        ("cargo clippy (lib)", ["cargo", "clippy",
                                 "-p", "kg-ontology", "-p", "kg-storage", "-p", "kg-embeddings",
                                 "-p", "kg-algorithms", "-p", "kg-pipeline",
                                 "-p", "da-domain", "-p", "da-ports", "-p", "da-application",
                                 "-p", "da-adapters", "-p", "da-parse", "-p", "da-cli", "-p", "da-mcp",
                                 "--no-deps", "--", "-D", "warnings"]),
        ("cargo clippy (tests)", ["cargo", "clippy",
                                   "-p", "kg-ontology", "-p", "kg-storage", "-p", "kg-embeddings",
                                   "-p", "kg-algorithms", "-p", "kg-pipeline",
                                   "-p", "da-domain", "-p", "da-ports", "-p", "da-application",
                                   "-p", "da-adapters", "-p", "da-parse", "-p", "da-cli", "-p", "da-mcp",
                                   "--lib", "--tests", "--no-deps", "--", "-D", "warnings"]),
        ("feature-matrix build", ["cargo", "check", "-p", "da-cli",
                                   "--features", "pdf-classifier", "--examples"]),
    ]
    
    if not getattr(args, "quick", False):
        steps.extend([
            ("kg-ontology tests", ["cargo", "test", "-p", "kg-ontology", "--lib"]),
            ("kg-storage tests", ["cargo", "test", "-p", "kg-storage", "--lib"]),
            ("kg-embeddings tests", ["cargo", "test", "-p", "kg-embeddings", "--lib"]),
            ("kg-algorithms tests", ["cargo", "test", "-p", "kg-algorithms", "--lib"]),
            ("kg-pipeline tests", ["cargo", "test", "-p", "kg-pipeline", "--lib"]),
            ("da-domain tests", ["cargo", "test", "-p", "da-domain", "--lib"]),
            ("da-parse tests", ["cargo", "test", "-p", "da-parse", "--lib"]),
            ("da-application tests", ["cargo", "test", "-p", "da-application", "--tests"]),
            ("da-adapters tests", ["cargo", "test", "-p", "da-adapters", "--tests"]),
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
            # so naming it here fails the whole build command and no gate step
            # enables that feature (pre-existing coverage gap, out of this change).
            # A cargo build command fails as a unit, so one unbuildable name
            # silently drops every other example in the same step.
            ("da-cli examples (default)", ["cargo", "build", "-p", "da-cli",
                                            "--example", "acceptance_evidence",
                                            "--example", "cross_stage_semantic_invariants",
                                            "--example", "derived_maturity_rehearsal",
                                            "--example", "external_holdout_capture",
                                            "--example", "native_family_gate_rehearsal",
                                            "--example", "parse_batch_budget_shadow",
                                            "--example", "rest_batch_asset_shadow"]),
            ("da-cli tests", ["cargo", "test", "-p", "da-cli", "--tests"]),
            # Feature-gated example contracts. Twelve da-cli test files declare
            # `#![cfg(feature = "pdf-layout")]`; a plain
            # `cargo test -p da-cli --tests` compiles them to empty targets, so
            # without these steps their coverage is nominal rather than real.
            # `cargo test --test NAME` does NOT build examples, and neither does
            # `cargo test --example NAME` — that leaves a *libtest harness* which
            # rejects the example's own `--manifest` flag (native_visual_heads
            # documents this), so a plain `cargo build --example X` is the only
            # correct provisioning. Only these eleven runners are driven by the
            # gated tests: building `--examples` compiled all ~86 runners for
            # ~25G of target/debug/examples, which is what this list avoids. Add
            # a name here when a new gated test starts driving an example.
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
                                                        "--example", "layout_bakeoff",
                                                        "--example", "native_fulltext_composition_shadow",
                                                        "--example", "native_fulltext_corpus_shadow",
                                                        "--example", "native_visual_heads_shadow"]),
            ("da-cli feature-gated example contracts", ["cargo", "test", "-p", "da-cli",
                                                         "--features", "pdf-layout", "--tests"]),
        ])
    
    failed = 0
    for name, cmd in steps:
        print(f"  {name}...", end=" ", flush=True)
        result = subprocess.run(cmd, cwd=root, capture_output=True, text=True)
        if result.returncode == 0:
            print("OK")
        else:
            print("FAIL")
            # Show last 5 lines of stderr
            for line in result.stderr.strip().split('\n')[-5:]:
                print(f"    {line}")
            failed = 1
    
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
