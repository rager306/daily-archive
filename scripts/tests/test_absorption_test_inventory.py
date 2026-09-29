"""Compiled-vs-executed inventory of the Rust test targets the architecture gate owns.

Why this file exists
--------------------
The architecture gate (``governor architecture``, ``scripts/governor/__main__.py``)
is the project's full verification surface. A test file that is *compiled* is
not necessarily *executed*:

* ``cargo test -p <crate> --lib`` compiles the lib and never runs
  ``crates/<crate>/tests/``;
* ``cargo test -p <crate> --tests`` compiles every integration target, but a
  ``#![cfg(feature = "X")]`` file compiles to an EMPTY binary unless the step
  also enables ``X``;
* ``cargo clippy --lib --tests`` compiles test code without running it.

Both recorded incidents are this failure mode: the M441 lesson (a documented CLI
feature stayed broken for a month because no gate layer compiled it) and the
M514/S01 finding (``governor da-parse --lib`` skipped every integration test;
``learning-evaluation`` was never enabled).

This inventory makes the distinction explicit and structural. It reads the gate
step list as DATA (``architecture_gate_steps``) and cross-checks it against the
workspace test sources. A gap must be *named* in ``DECLARED_UNEXECUTED_FEATURES``;
silence is a test failure. No cargo command is run, so the check is fast and
hermetic.

Scope
-----
``SCOPE_CRATES`` (``da-parse``, ``da-cli``) is the absorption/measurement plane
this task hardens: every test target there must be executed. For every other
crate the inventory still records feature-gated targets and requires each
unexecuted one to be declared, so a new ``cfg(feature = "X")`` target cannot
vanish into an empty binary anywhere in the workspace.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

_SCRIPTS_DIR = Path(__file__).resolve().parent.parent
_REPO_ROOT = _SCRIPTS_DIR.parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

from governor.__main__ import _ARCHITECTURE_STEP_TIMEOUT_S, architecture_gate_steps


# ── inventory configuration (DATA) ───────────────────────────────────────────

# Every test target in these crates must be *executed*, not merely compiled.
SCOPE_CRATES = ("da-parse", "da-cli")

# The two feature planes this task wired into the gate. They must be executed
# for the scoped crates; if either disappears the gate regressed to the M514
# state where the `learning-evaluation` test target compiled to an empty binary.
REQUIRED_EXECUTED_FEATURES = {
    ("da-cli", "pdf-layout"),
    ("da-cli", "learning-evaluation"),
}

# Feature-gated test targets the gate does not execute today, keyed by
# (crate, feature). Naming them here is the point: the failure mode this
# inventory guards is *silence*. A new key must be added deliberately, or a gate
# step must be added that enables the feature.
DECLARED_UNEXECUTED_FEATURES: dict[tuple[str, str], str] = {
    ("da-cli", "redb"): (
        "parse_batch_test.rs gates four accelerator tests on `#[cfg(feature = "
        '"redb")]`; the feature is opt-in (D295) and no gate step enables it for '
        "da-cli, so those branches are compiled out. Declared, not silent."
    ),
    ("da-adapters", "pdf-classifier"): (
        'hub_ocr_probe.rs is `#![cfg(feature = "pdf-classifier")]`; the gate runs '
        "`cargo test -p da-adapters --tests` without the feature, so the target "
        "compiles empty and never runs."
    ),
    ("da-adapters", "pdf-layout"): (
        'four `layout_*_contract.rs` files are `#![cfg(feature = "pdf-layout")]`; '
        "the gate enables pdf-layout for da-cli test targets only, which compiles "
        "the da-adapters library but not its test targets."
    ),
    ("da-adapters", "redb"): (
        'parse_learning_index.rs is `#![cfg(feature = "redb")]`; opt-in and not '
        "enabled by any gate step."
    ),
    ("kg-storage", "algorithms"): (
        "algo_adapter / csr_export_compose / ppr_export_compose / subgraph_adapter "
        'are `#![cfg(feature = "algorithms")]`; the gate runs '
        "`cargo test -p kg-storage --lib` only, so these integration targets are "
        "neither compiled with the feature nor executed."
    ),
    ("kg-storage", "samyama"): (
        'resp_server_smoke.rs (file-level) and contract_samyama.rs (required-features) '
        'are gated on `samyama`; the same `--lib`-only step means they never run.'
    ),
}

# Crates whose gate test step runs `--lib` only (or which have no gate test step
# at all). Their integration targets are compiled by the workspace clippy step
# but never executed. These are pre-existing gaps outside this task's mandate
# (`da-parse`, `da-cli`, and the feature planes): recording them here keeps the
# "compiled but not executed" column explicit, so migrating a crate to
# `--tests` shows up as a diff instead of a silent improvement.
DECLARED_LIB_ONLY_CRATES: dict[str, str] = {
    "da-domain": "gate runs `cargo test -p da-domain --lib`; 5 integration files compiled only.",
    "da-ports": "no gate test step at all; its 1 integration file is compiled by clippy only.",
    "kg-embeddings": "gate runs `--lib`; its 1 integration file is compiled only.",
    "kg-ontology": "gate runs `--lib`; its 1 integration file is compiled only.",
    "kg-storage": (
        "gate runs `--lib`; 8 integration files (incl. contract_samyama with "
        "required-features = [\"samyama\"]) are compiled only."
    ),
}


# `cargo build --examples` compiles all ~86 da-cli runners (~25G). The gate
# provisions the runners it needs by NAME. `cargo check --examples` (the pinned
# pdf-classifier feature-matrix step) is allowed; `cargo test/build --workspace`
# is never allowed at this layer.
_FORBIDDEN_WORKSPACE_FLAGS = ("--workspace", "--all")


# ── parsing ──────────────────────────────────────────────────────────────────

_INNER_FEATURE_RE = re.compile(
    r'^\s*#!\[cfg\(feature\s*=\s*"([^"]+)"\)\]', re.MULTILINE
)
_FN_FEATURE_RE = re.compile(r'^\s*#\[cfg\(feature\s*=\s*"([^"]+)"\)\]', re.MULTILINE)


def _parse_gate_cmd(cmd: list[str]) -> dict:
    """Parse a cargo argv into the fields the coverage model needs."""
    argv = list(cmd)
    sub = argv[1] if len(argv) > 1 else ""
    packages: list[str] = []
    features: set[str] = set()
    examples: list[str] = []
    flags: set[str] = set()
    i = 2
    while i < len(argv):
        tok = argv[i]
        if tok == "-p":
            packages.append(argv[i + 1])
            i += 2
            continue
        if tok == "--features":
            features.update(part for part in argv[i + 1].split(",") if part)
            i += 2
            continue
        if tok == "--example":
            examples.append(argv[i + 1])
            i += 2
            continue
        if tok.startswith("--"):
            flags.add(tok)
        i += 1
    return {
        "sub": sub,
        "packages": packages,
        "features": features,
        "examples": examples,
        "flags": flags,
    }


def _gate_steps() -> list[dict]:
    return [dict(_parse_gate_cmd(cmd), name=name, timeout_s=t, argv=list(cmd))
            for name, cmd, t in architecture_gate_steps()]


def _step_compiles_tests(step: dict, crate: str) -> bool:
    """Does this step build the crate's integration test targets?"""
    return (
        step["sub"] in {"test", "clippy", "check", "build"}
        and crate in step["packages"]
        and ("--tests" in step["flags"] or "--all-targets" in step["flags"])
    )


def _step_executes_tests(step: dict, crate: str) -> bool:
    """Does this step run the crate's integration test targets?"""
    return (
        step["sub"] == "test"
        and crate in step["packages"]
        and "--tests" in step["flags"]
    )


def _scan_crate_tests(crate: str) -> dict[str, dict]:
    tests_dir = _REPO_ROOT / "crates" / crate / "tests"
    if not tests_dir.is_dir():
        return {}
    out: dict[str, dict] = {}
    for path in sorted(tests_dir.glob("*.rs")):
        text = path.read_text(encoding="utf-8")
        inner = _INNER_FEATURE_RE.search(text)
        out[path.name] = {
            "file_feature": inner.group(1) if inner else None,
            "fn_features": sorted(set(_FN_FEATURE_RE.findall(text))),
        }
    return out


def _all_test_crates() -> list[str]:
    crates_dir = _REPO_ROOT / "crates"
    return sorted(
        p.name for p in crates_dir.iterdir() if (p / "tests").is_dir()
    )


def _example_required_features(crate: str) -> dict[str, frozenset[str]]:
    """`[[example]]` name -> required-features, from the crate manifest.

    Cargo auto-discovers `examples/*.rs`, so a name may exist with no
    `[[example]]` block; such an example carries no required features and is
    folded in here with an empty set.
    """
    manifest = _REPO_ROOT / "crates" / crate / "Cargo.toml"
    if not manifest.is_file():
        return {}
    out: dict[str, frozenset[str]] = {}
    current = None
    for line in manifest.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if stripped == "[[example]]":
            if current is not None:
                out[current[0]] = frozenset(current[1])
            current = (None, set())
            continue
        if current is None:
            continue
        if stripped.startswith("[") and stripped != "[[example]]":
            if current[0] is not None:
                out[current[0]] = frozenset(current[1])
            current = None
            continue
        name = re.match(r'name\s*=\s*"([^"]+)"', stripped)
        if name and current[0] is None:
            current = (name.group(1), current[1])
            continue
        required = re.match(r"required-features\s*=\s*\[(.*)\]", stripped)
        if required:
            values = {
                part.strip().strip('"')
                for part in required.group(1).split(",")
                if part.strip()
            }
            current[1].update(values)
    if current is not None and current[0] is not None:
        out[current[0]] = frozenset(current[1])

    # Auto-discovered examples: a `.rs` file in examples/ with no [[example]]
    # block. `setdefault` keeps the manifest's required-features when present.
    examples_dir = _REPO_ROOT / "crates" / crate / "examples"
    if examples_dir.is_dir():
        for path in examples_dir.glob("*.rs"):
            out.setdefault(path.stem, frozenset())
    return out


# ── inventory ────────────────────────────────────────────────────────────────


def build_inventory() -> dict:
    """The compiled/executed/cfg/timeout inventory the gate must satisfy."""
    steps = _gate_steps()

    file_states: list[dict] = []
    feature_pairs: dict[tuple[str, str], dict] = {}

    for crate in _all_test_crates():
        for filename, info in _scan_crate_tests(crate).items():
            feature = info["file_feature"]
            if feature is None:
                executed = any(_step_executes_tests(s, crate) for s in steps)
            else:
                executed = any(
                    _step_executes_tests(s, crate) and feature in s["features"]
                    for s in steps
                )
            file_states.append({
                "crate": crate,
                "file": filename,
                "file_feature": feature,
                "fn_features": info["fn_features"],
                "executed": executed,
            })
            for pair_feature in ([feature] if feature else []) + info["fn_features"]:
                compiled = any(
                    _step_compiles_tests(s, crate) and pair_feature in s["features"]
                    for s in steps
                )
                pair_executed = any(
                    _step_executes_tests(s, crate) and pair_feature in s["features"]
                    for s in steps
                )
                feature_pairs[(crate, pair_feature)] = {
                    "compiled": compiled,
                    "executed": pair_executed,
                }

    return {
        "steps": steps,
        "files": file_states,
        "feature_pairs": feature_pairs,
        "counts": {
            "gate_steps": len(steps),
            "test_files": len(file_states),
            "executed_test_files": sum(1 for f in file_states if f["executed"]),
            "feature_pairs": len(feature_pairs),
            "executed_feature_pairs": sum(
                1 for s in feature_pairs.values() if s["executed"]
            ),
        },
    }


# ── gate-step invariants ─────────────────────────────────────────────────────


def test_every_gate_step_declares_a_positive_finite_timeout():
    """A step with no timeout can hang; a timed-out step must fail, not pass."""
    assert _ARCHITECTURE_STEP_TIMEOUT_S > 0
    steps = architecture_gate_steps()
    assert steps, "architecture gate declares no steps"
    for name, _cmd, timeout_s in steps:
        assert isinstance(timeout_s, int), f"{name}: timeout is not an int"
        assert timeout_s > 0, f"{name}: timeout must be positive, got {timeout_s}"


def test_gate_runner_reports_a_timed_out_step_as_failure(monkeypatch, capsys, tmp_path):
    """A step that never finishes must never be reported as OK.

    This is the behavioural half of the timeout invariant: the runner catches
    ``subprocess.TimeoutExpired`` and returns a failing exit code. A truncated
    capture with a stale green status is the failure mode the rule forbids.
    """
    import governor.__main__ as gov

    def fake_steps(quick=False):  # noqa: ARG001 - mirrors the real signature
        return [("sleeper", [sys.executable, "-c", "import time; time.sleep(30)"], 1)]

    monkeypatch.setattr(gov, "architecture_gate_steps", fake_steps)
    monkeypatch.setenv(gov._ARCHITECTURE_GATE_LOG_ENV, str(tmp_path))

    class _Args:
        quick = False

    rc = gov._run_architecture_gate(_Args())
    out = capsys.readouterr().out
    assert rc == 1
    assert "TIMEOUT" in out
    assert "ARCHITECTURE GATE: FAIL" in out


def test_gate_runner_persists_failing_step_capture_and_names_the_test(
    monkeypatch, capsys, tmp_path
):
    """A failing step must identify itself: full capture kept, failing test named.

    The gate that produced the M514/S01 first-run failure printed only the last
    five stderr lines and discarded the capture, so the failing test was never
    known. This is the deterministic negative half of that fix, driven by a fake
    subprocess: the full stdout and stderr must be persisted, and the libtest
    ``test <name> ... FAILED`` name must be printed.
    """
    import governor.__main__ as gov

    script = (
        "import sys\n"
        "print('running 2 tests')\n"
        "print('test backend_contract::outage ... FAILED')\n"
        "print('test backend_contract::healthy ... ok')\n"
        "print('assertion failed: expected Err', file=sys.stderr)\n"
        "print('test result: FAILED. 1 passed; 1 failed', file=sys.stderr)\n"
        "sys.exit(1)\n"
    )

    def fake_steps(quick=False):  # noqa: ARG001 - mirrors the real signature
        return [("da-parse tests", [sys.executable, "-c", script], 60)]

    monkeypatch.setattr(gov, "architecture_gate_steps", fake_steps)
    monkeypatch.setenv(gov._ARCHITECTURE_GATE_LOG_ENV, str(tmp_path))

    class _Args:
        quick = False

    rc = gov._run_architecture_gate(_Args())
    out = capsys.readouterr().out
    assert rc == 1
    assert "ARCHITECTURE GATE: FAIL" in out
    # The failing test is identified, and only the failing one.
    failing_lines = [line for line in out.splitlines() if "failing tests:" in line]
    assert len(failing_lines) == 1
    assert "backend_contract::outage" in failing_lines[0]
    assert "backend_contract::healthy" not in failing_lines[0]
    # The full capture is persisted, one file per stream.
    stdout_logs = sorted(tmp_path.glob("*.stdout.log"))
    stderr_logs = sorted(tmp_path.glob("*.stderr.log"))
    assert len(stdout_logs) == 1 and len(stderr_logs) == 1
    assert stdout_logs[0].name.startswith("01-")
    assert "running 2 tests" in stdout_logs[0].read_text(encoding="utf-8")
    assert "backend_contract::outage ... FAILED" in stdout_logs[0].read_text(
        encoding="utf-8"
    )
    assert "assertion failed: expected Err" in stderr_logs[0].read_text(encoding="utf-8")


def test_gate_runner_keeps_no_capture_for_a_passing_step(monkeypatch, tmp_path):
    """Only a failure's bytes are kept: a green step leaves no log behind."""
    import governor.__main__ as gov

    def fake_steps(quick=False):  # noqa: ARG001 - mirrors the real signature
        return [("green", [sys.executable, "-c", "print('ok')"], 60)]

    monkeypatch.setattr(gov, "architecture_gate_steps", fake_steps)
    monkeypatch.setenv(gov._ARCHITECTURE_GATE_LOG_ENV, str(tmp_path))

    class _Args:
        quick = False

    assert gov._run_architecture_gate(_Args()) == 0
    assert list(tmp_path.iterdir()) == []


def test_gate_never_mass_builds_examples_or_the_workspace():
    """Named example provisioning only: no `build --examples`, no `--workspace`."""
    steps = _gate_steps()
    offenders: list[str] = []
    for step in steps:
        for flag in _FORBIDDEN_WORKSPACE_FLAGS:
            if flag in step["flags"]:
                offenders.append(f"{step['name']}: {flag}")
        if step["sub"] == "build" and "--examples" in step["flags"]:
            offenders.append(f"{step['name']}: cargo build --examples")
    assert not offenders, (
        "gate must provision named targets only; mass builds found: " + "; ".join(offenders)
    )


def test_every_named_example_exists_and_its_required_features_are_enabled():
    """A named example whose feature is off fails the whole cargo step.

    A cargo command fails as a unit, so one unbuildable `--example` name
    silently drops every other example in the same step. The manifest is the
    source of truth for a name and for its `required-features`.
    """
    problems: list[str] = []
    for step in _gate_steps():
        if not step["examples"]:
            continue
        for crate in step["packages"]:
            manifest = _example_required_features(crate)
            for name in step["examples"]:
                if name not in manifest:
                    problems.append(f"{step['name']}: example `{name}` not in {crate} manifest")
                    continue
                missing = manifest[name] - step["features"]
                if missing:
                    problems.append(
                        f"{step['name']}: `{name}` needs {sorted(missing)} "
                        f"but step features are {sorted(step['features'])}"
                    )
    assert not problems, "\n".join(problems)


# ── coverage invariants ──────────────────────────────────────────────────────


def test_da_parse_integration_targets_are_executed_not_just_lib():
    """T03: `da-parse --tests` replaces the old `--lib`-only step."""
    steps = _gate_steps()
    executing = [s for s in steps if _step_executes_tests(s, "da-parse")]
    assert executing, "no gate step executes da-parse integration tests"
    assert not any("--lib" in s["flags"] and "--tests" not in s["flags"] for s in executing)

    files = _scan_crate_tests("da-parse")
    assert len(files) > 0
    inventory = build_inventory()
    not_executed = [
        f["file"] for f in inventory["files"]
        if f["crate"] == "da-parse" and not f["executed"]
    ]
    assert not not_executed, f"da-parse test targets compiled but not executed: {not_executed}"


def test_learning_evaluation_targets_are_executed_and_provisioned():
    """T03: the learning-evaluation test and both feature examples reach the gate."""
    steps = _gate_steps()
    executed_tests = any(
        _step_executes_tests(s, "da-cli") and "learning-evaluation" in s["features"]
        for s in steps
    )
    assert executed_tests, "no gate step runs `cargo test -p da-cli --features learning-evaluation --tests`"

    provisioned = {
        name
        for s in steps
        if "learning-evaluation" in s["features"] and s["sub"] == "build"
        for name in s["examples"]
    }
    required = {
        name
        for name, req in _example_required_features("da-cli").items()
        if req == {"learning-evaluation"}
    }
    assert required, "no learning-evaluation example found in the da-cli manifest"
    assert required <= provisioned, (
        f"learning-evaluation examples not provisioned by name: {sorted(required - provisioned)}"
    )

    inventory = build_inventory()
    state = inventory["feature_pairs"][("da-cli", "learning-evaluation")]
    assert state == {"compiled": True, "executed": True}


def test_scope_crates_have_no_unexecuted_test_target():
    """Every test file in the measurement plane is executed with its cfg satisfied."""
    inventory = build_inventory()
    gaps = [
        f"{f['crate']}/tests/{f['file']} (feature={f['file_feature']})"
        for f in inventory["files"]
        if f["crate"] in SCOPE_CRATES and not f["executed"]
    ]
    assert not gaps, "scoped test targets compiled but not executed: " + ", ".join(gaps)


def test_every_required_feature_plane_is_executed():
    inventory = build_inventory()
    for pair in sorted(REQUIRED_EXECUTED_FEATURES):
        state = inventory["feature_pairs"].get(pair)
        assert state is not None, f"required feature plane missing: {pair}"
        assert state["executed"], f"required feature plane not executed: {pair}"


def test_unexecuted_feature_targets_are_exactly_the_declared_gaps():
    """Silence is the failure mode: a new cfg target must be covered or declared."""
    inventory = build_inventory()
    unexecuted = {
        pair for pair, state in inventory["feature_pairs"].items() if not state["executed"]
    }
    declared = set(DECLARED_UNEXECUTED_FEATURES)
    undeclared = sorted(unexecuted - declared)
    stale = sorted(declared - unexecuted)
    assert not undeclared, (
        "feature-gated test targets neither executed nor declared: " + repr(undeclared)
    )
    assert not stale, (
        "declared gap no longer matches reality (remove it): " + repr(stale)
    )


def test_execution_implies_compilation_and_cfg_is_recorded():
    """A target cannot execute a feature it was not compiled with."""
    inventory = build_inventory()
    violated = [
        pair for pair, state in inventory["feature_pairs"].items()
        if state["executed"] and not state["compiled"]
    ]
    assert not violated, f"executed without compiled: {sorted(violated)}"

    # The cfg distinction must be recorded, not assumed: parse_batch_test is a
    # file with no file-level feature but `redb`-gated functions.
    parse_batch = [
        f for f in inventory["files"] if f["file"] == "parse_batch_test.rs"
    ]
    assert parse_batch, "parse_batch_test.rs missing from the inventory"
    assert parse_batch[0]["file_feature"] is None
    assert parse_batch[0]["fn_features"] == ["redb"]


def test_every_crate_with_tests_is_executed_or_declared_lib_only():
    """A crate's integration targets must be executed or the gap must be named."""
    inventory = build_inventory()
    crates_with_tests = {f["crate"] for f in inventory["files"]}
    executed_crates = {f["crate"] for f in inventory["files"] if f["executed"]}
    declared = set(DECLARED_LIB_ONLY_CRATES)

    undeclared = sorted(crates_with_tests - executed_crates - declared)
    stale = sorted(declared - (crates_with_tests - executed_crates))
    assert not undeclared, (
        "crates whose integration tests never execute and are not declared: "
        + repr(undeclared)
    )
    assert not stale, (
        "declared lib-only crate now executes its tests (remove the entry): "
        + repr(stale)
    )


def test_inventory_counts_are_nonzero(capsys):
    """Record the actual counts; a zero here means the scan silently emptied."""
    inventory = build_inventory()
    counts = inventory["counts"]
    assert counts["gate_steps"] > 0
    assert counts["test_files"] > 0
    assert counts["executed_test_files"] > 0
    assert counts["executed_feature_pairs"] > 0
    print(
        "architecture test inventory: "
        f"{counts['gate_steps']} gate steps, "
        f"{counts['executed_test_files']}/{counts['test_files']} test files executed, "
        f"{counts['executed_feature_pairs']}/{counts['feature_pairs']} feature planes executed, "
        f"{len(DECLARED_UNEXECUTED_FEATURES)} declared gaps"
    )
