"""Feature gated test coverage axis: ratio, zero-target names, negative cases.

The axis exists because the M515 defect was invisible: a contract test behind
``#![cfg(feature = "pdf-layout")]`` compiled to an empty target under the
default ``cargo test -p da-cli --tests`` step and executed zero tests while the
tree looked green. These tests pin the predicate that makes that target
nameable, using inline fixtures so they never depend on the real gate's
current membership.
"""

from __future__ import annotations

from pathlib import Path

from governor.adapters.feature_gated_coverage import (
    build_feature_gated_coverage,
    observe_feature_gated_coverage,
)

# ── fixtures ─────────────────────────────────────────────────────────────────


def _write(root: Path, rel: str, text: str) -> None:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _test_step(crate: str, features: str | None = None) -> tuple:
    argv = ["cargo", "test", "-p", crate]
    if features:
        argv += ["--features", features]
    argv += ["--tests"]
    return ("cargo test", argv, 60)


def _coverage(root: Path, steps):
    return build_feature_gated_coverage(root, steps)


# ── file-level gate ──────────────────────────────────────────────────────────


def test_file_gate_with_enabled_feature_executes(tmp_path: Path):
    """A gated file whose feature a `--tests` step enables is not empty."""
    _write(
        tmp_path,
        "crates/da-cli/tests/layout_contract.rs",
        '#![cfg(feature = "pdf-layout")]\n\n#[test]\nfn t() {}\n',
    )
    cov = _coverage(tmp_path, [_test_step("da-cli", "pdf-layout")])
    assert cov.gated_total == 1
    assert cov.executed_total == 1
    assert cov.ratio == 1.0
    assert cov.zero_executing == ()
    assert cov.targets[0].file_level is True


def test_file_gate_without_enabled_feature_is_named(tmp_path: Path):
    """The M515 defect: the target compiles empty and is named individually."""
    _write(
        tmp_path,
        "crates/da-cli/tests/rollback_contract.rs",
        '#![cfg(feature = "pdf-layout")]\n\n#[test]\nfn t() {}\n',
    )
    cov = _coverage(tmp_path, [_test_step("da-cli")])
    assert cov.gated_total == 1
    assert cov.executed_total == 0
    assert cov.ratio == 0.0
    named = [t.name for t in cov.zero_executing]
    assert named == ["da-cli/tests/rollback_contract.rs"]
    assert "EMPTY" in cov.zero_executing[0].detail
    # The rendered axis names the target, not just the aggregate.
    rendered = "\n".join(cov.render_lines())
    assert "0/1" in rendered
    assert "da-cli/tests/rollback_contract.rs" in rendered


# ── function-level gate ──────────────────────────────────────────────────────


def test_function_gate_all_tests_disabled_is_zero(tmp_path: Path):
    """A target whose ONLY tests are gated on a disabled feature runs none.

    This is the hole a crate-level ``--tests`` step alone cannot see: the step
    is green, but every test function compiled out.
    """
    _write(
        tmp_path,
        "crates/da-cli/tests/parse_batch_test.rs",
        '#[cfg(feature = "redb")]\n#[tokio::test]\nasync fn only_test() {}\n',
    )
    cov = _coverage(tmp_path, [_test_step("da-cli")])
    assert cov.executed_total == 0
    assert cov.zero_executing[0].file_level is False
    assert "zero runnable tests" in cov.zero_executing[0].detail


def test_function_gate_with_ungated_test_executes(tmp_path: Path):
    """One ungated test keeps the target non-zero even if others are gated."""
    _write(
        tmp_path,
        "crates/da-cli/tests/parse_batch_test.rs",
        '#[cfg(feature = "redb")]\n#[tokio::test]\nasync fn gated() {}\n\n'
        "#[tokio::test]\nasync fn ungated() {}\n",
    )
    cov = _coverage(tmp_path, [_test_step("da-cli")])
    assert cov.executed_total == 1
    assert cov.zero_executing == ()


def test_function_gate_enabled_by_a_step_executes(tmp_path: Path):
    """The gated test runs when a step enables its feature."""
    _write(
        tmp_path,
        "crates/da-cli/tests/parse_batch_test.rs",
        '#[cfg(feature = "redb")]\n#[tokio::test]\nasync fn only_test() {}\n',
    )
    cov = _coverage(tmp_path, [_test_step("da-cli", "redb")])
    assert cov.executed_total == 1


def test_not_feature_gate_runs_under_default_build(tmp_path: Path):
    """`#[cfg(not(feature = "redb"))]` describes a test that runs when OFF."""
    _write(
        tmp_path,
        "crates/da-cli/tests/parse_batch_test.rs",
        '#[cfg(not(feature = "redb"))]\n#[tokio::test]\nasync fn fails_closed_without_feature() {}\n',
    )
    cov = _coverage(tmp_path, [_test_step("da-cli")])
    assert cov.executed_total == 1
    assert cov.zero_executing == ()


def test_feature_gated_module_inherits_the_gate(tmp_path: Path):
    """Tests inside a `#[cfg(feature = "X")] mod` inherit the module gate."""
    _write(
        tmp_path,
        "crates/kg-storage/tests/contract.rs",
        '#[cfg(feature = "samyama")]\nmod inner {\n'
        "    #[tokio::test]\n    async fn contract() {}\n}\n",
    )
    cov = _coverage(tmp_path, [_test_step("kg-storage")])
    assert cov.executed_total == 0
    assert cov.zero_executing[0].gate_features == ("samyama",)


def test_crate_without_an_executing_step_is_zero(tmp_path: Path):
    """A fn-gated target in a crate the gate only builds with `--lib` is zero."""
    _write(
        tmp_path,
        "crates/kg-storage/tests/algo.rs",
        '#[cfg(feature = "algorithms")]\nmod inner {\n'
        "    #[tokio::test]\n    async fn a() {}\n}\n",
    )
    cov = _coverage(tmp_path, [("cargo test", ["cargo", "test", "-p", "kg-storage", "--lib"], 60)])
    assert cov.gated_total == 1
    assert cov.executed_total == 0
    assert "never executes" in cov.zero_executing[0].detail


def test_untouched_ungated_file_is_not_in_the_denominator(tmp_path: Path):
    """A file with no cfg feature gate is outside the axis entirely."""
    _write(tmp_path, "crates/da-cli/tests/plain.rs", "#[test]\nfn t() {}\n")
    _write(
        tmp_path,
        "crates/da-cli/tests/gated.rs",
        '#![cfg(feature = "pdf-layout")]\n#[test]\nfn t() {}\n',
    )
    cov = _coverage(tmp_path, [_test_step("da-cli", "pdf-layout")])
    assert cov.gated_total == 1
    assert [t.file for t in cov.targets] == ["gated.rs"]


def test_unparseable_gated_target_fails_closed(tmp_path: Path):
    """A gated file whose test items cannot be found is reported zero, not green."""
    _write(
        tmp_path,
        "crates/da-cli/tests/helpers.rs",
        '#[cfg(feature = "redb")]\nuse da_parse::ParseBatchItem;\n',
    )
    cov = _coverage(tmp_path, [_test_step("da-cli")])
    assert cov.gated_total == 1
    assert cov.executed_total == 0
    assert "zero-executing rather than assumed" in cov.zero_executing[0].detail


def test_missing_crates_dir_fails_loudly(tmp_path: Path):
    """A root with no crates/ must raise, never report a vacuous 0/0 green."""
    import pytest

    with pytest.raises(FileNotFoundError):
        _coverage(tmp_path, [])


# ── aggregate ────────────────────────────────────────────────────────────────


def test_ratio_over_mixed_targets(tmp_path: Path):
    """The axis value is executed / gated and the names are exactly the zeros."""
    _write(
        tmp_path,
        "crates/da-cli/tests/a.rs",
        '#![cfg(feature = "pdf-layout")]\n#[test]\nfn t() {}\n',
    )
    _write(
        tmp_path,
        "crates/da-cli/tests/b.rs",
        '#![cfg(feature = "ghost")]\n#[test]\nfn t() {}\n',
    )
    _write(
        tmp_path,
        "crates/da-adapters/tests/c.rs",
        '#![cfg(feature = "pdf-classifier")]\n#[test]\nfn t() {}\n',
    )
    cov = _coverage(tmp_path, [_test_step("da-cli", "pdf-layout")])
    assert cov.gated_total == 3
    assert cov.executed_total == 1
    assert cov.ratio == 1 / 3
    assert {t.name for t in cov.zero_executing} == {
        "da-cli/tests/b.rs",
        "da-adapters/tests/c.rs",
    }
    data = cov.to_dict()
    assert data["axis"] == "feature_gated_coverage"
    assert data["gated_targets"] == 3
    assert data["executed_targets"] == 1
    assert data["zero_executing_count"] == 2
    assert set(data["zero_executing_targets"]) == {
        "da-cli/tests/b.rs",
        "da-adapters/tests/c.rs",
    }


# ── real tree ────────────────────────────────────────────────────────────────


def test_real_tree_reports_a_ratio_and_names_zeros():
    """On the live tree the axis reports executed/gated and its zero names.

    Two named targets are stable enough to assert directly: `parse_batch_test`
    has ungated tests so it must NOT be flagged (the fn-level ungated case),
    while `contract_samyama` and the da-adapters gated targets have no
    executing step that enables their feature and must be flagged.
    """
    cov = observe_feature_gated_coverage()
    assert cov.gated_total > 0, "no gated test target found — scan silently emptied"
    assert 0.0 <= cov.ratio <= 1.0
    names = {t.name for t in cov.zero_executing}
    executed_names = {t.name for t in cov.targets if t.executes_tests}

    assert "da-cli/tests/parse_batch_test.rs" in executed_names
    assert "da-cli/tests/parse_batch_test.rs" not in names
    assert "kg-storage/tests/contract_samyama.rs" in names
    assert "da-adapters/tests/hub_ocr_probe.rs" in names

    assert cov.executed_total + len(cov.zero_executing) == cov.gated_total
    rendered = cov.render_lines()
    assert rendered and "feature-gated coverage:" in rendered[0]


def test_real_tree_uses_the_gate_data_not_a_copy():
    """Synthetic gate steps for the real root change the executed count.

    If the adapter carried its own step list, injected DATA would be ignored.
    """
    from governor.__main__ import architecture_gate_steps

    real = observe_feature_gated_coverage()
    # A gate step that enables pdf-layout for da-adapters moves its four
    # `#![cfg(feature = "pdf-layout")]` contract targets to executed.
    synthetic = observe_feature_gated_coverage(
        gate_steps=list(architecture_gate_steps()) + [_test_step("da-adapters", "pdf-layout")]
    )
    assert synthetic.executed_total >= real.executed_total
    assert synthetic.executed_total > real.executed_total
