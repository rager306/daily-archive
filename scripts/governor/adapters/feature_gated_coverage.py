"""Adapter: feature gated test coverage axis.

The architecture gate (``governor architecture``) is the project's full
verification surface. A test target that is *compiled* is not necessarily
*executed*, and a target that is executed is not necessarily executing any
tests:

* a file-level ``#![cfg(feature = "X")]`` gate compiles the whole target to an
  EMPTY binary unless a gate step enables ``X``;
* a function-level ``#[cfg(feature = "X")]`` gate compiles only the gated test
  functions out; a target whose *only* tests are gated on a feature the gate
  never enables still executes zero tests while the crate's ``--tests`` step
  reports success.

The M515 defect is exactly this class: a contract test behind
``#![cfg(feature = "pdf-layout")]`` compiled to an empty target under the
default ``cargo test -p da-cli --tests`` step and executed zero tests, while
the tree looked green. This axis makes such a target *nameable* instead of
letting it hide behind a green aggregate.

What this adapter reports
-------------------------
For every integration test target (``crates/<crate>/tests/*.rs``) that carries
a cfg feature gate, it reports whether the architecture gate executes a
non-zero number of tests from it, and names each zero-executing target
individually. The axis value is ``executed / gated`` — a regression is a
decrease, and the individual names make the number actionable.

The gate step list is read as DATA (``architecture_gate_steps``), so the axis
measures "under the feature set the repository actually builds" rather than a
copy. A target counts as executing only when a ``cargo test ... --tests`` step
for its crate enables the feature its file gate requires (file-level) or when
at least one of its test functions is runnable under some executing step's
feature set (function-level).

Relationship to ``scripts/tests/test_absorption_test_inventory.py``
-------------------------------------------------------------------
That test owns the structural invariant ("a new cfg target must be covered or
declared"). This adapter owns the *ratio* and the *zero-target names*. The two
scan the same test sources with the same regex vocabulary; this module is
parameterised by repository root so it can be exercised against fixtures,
which the test script (hard-wired to the real repository) cannot be.
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence

__all__ = [
    "FeatureGatedCoverage",
    "GatedTarget",
    "build_feature_gated_coverage",
    "observe_feature_gated_coverage",
]

#: A cargo argv is ``("name", ["cargo", <sub>, ...], timeout)`` — the shape
#: ``governor.__main__.architecture_gate_steps()`` returns.
GateStep = tuple[str, Sequence[str], int]


# ── gate-step parsing ────────────────────────────────────────────────────────


def _parse_gate_step(name: str, argv: Sequence[str], timeout_s: int) -> dict:
    """Parse a cargo argv into the fields the coverage model needs.

    ``features`` is exactly what the step's ``--features`` flag enables; the
    crate's ``default`` feature set is added separately at evaluation time so a
    default-on feature is not mistaken for a disabled one.
    """
    packages: list[str] = []
    features: set[str] = set()
    flags: set[str] = set()
    sub = argv[1] if len(argv) > 1 else ""
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
        if tok.startswith("--features="):
            features.update(part for part in tok.split("=", 1)[1].split(",") if part)
        elif tok.startswith("--"):
            flags.add(tok)
        i += 1
    return {
        "name": name,
        "sub": sub,
        "packages": packages,
        "features": features,
        "flags": flags,
        "timeout_s": timeout_s,
    }


def _parse_gate_steps(gate_steps: Iterable[GateStep]) -> list[dict]:
    return [_parse_gate_step(name, argv, timeout_s) for name, argv, timeout_s in gate_steps]


def _executing_steps(steps: Sequence[dict], crate: str) -> list[dict]:
    """Steps that RUN integration tests for ``crate`` (``cargo test --tests``)."""
    return [
        s
        for s in steps
        if s["sub"] == "test" and crate in s["packages"] and "--tests" in s["flags"]
    ]


# ── manifest: default feature set ────────────────────────────────────────────

_FEATURES_SECTION_RE = re.compile(r"^\[features\]\s*$", re.MULTILINE)
_DEFAULT_LINE_RE = re.compile(r"^\s*default\s*=\s*\[(?P<body>.*?)\]", re.DOTALL | re.MULTILINE)
_QUOTED_RE = re.compile(r'"([^"]+)"')


def _crate_default_features(root: Path, crate: str) -> frozenset[str]:
    """The crate's ``[features] default = [...]`` names, minus dependency syntax.

    A token like ``dep:pdf-inspector`` or ``da-adapters/pdf-layout`` names a
    dependency feature, never a ``cfg(feature = ...)`` label, so it is dropped.
    A missing manifest (fixtures) is an empty default set, which is the
    conservative reading: nothing is enabled unless a step says so.
    """
    manifest = root / "crates" / crate / "Cargo.toml"
    if not manifest.is_file():
        return frozenset()
    text = manifest.read_text(encoding="utf-8")
    section = _FEATURES_SECTION_RE.search(text)
    if section is None:
        return frozenset()
    # Bound the search to the [features] section so a `default = [...]`
    # elsewhere in the manifest cannot leak in.
    body = text[section.end():]
    next_section = re.search(r"^\[", body, re.MULTILINE)
    if next_section is not None:
        body = body[: next_section.start()]
    match = _DEFAULT_LINE_RE.search(body)
    if match is None:
        return frozenset()
    names = set()
    for token in _QUOTED_RE.findall(match.group("body")):
        if token.startswith("dep:") or "/" in token:
            continue
        names.add(token)
    return frozenset(names)


# ── test source scanning ─────────────────────────────────────────────────────

# File-level inner gate: `#![cfg(feature = "X")]`.
_INNER_FEATURE_RE = re.compile(r'^\s*#!\[cfg\(\s*feature\s*=\s*"([^"]+)"\s*\)\s*\]', re.MULTILINE)
# Any line-anchored `#[cfg(... feature = "X" ...)]`. `not(feature = "X")` is a
# feature gate too (a test that runs when X is OFF), so the "carries a gate"
# scan must see it; the positive/negative distinction is made later by
# `_chain_requirement` and `_satisfiable`.
_CFG_ATTR_RE = re.compile(r"^\s*#\[cfg\((?P<body>.*)\)\]\s*$", re.MULTILINE)

_ATTR_LINE_RE = re.compile(r"^\s*#!?\[")
_TEST_ATTR_RE = re.compile(r"^\s*#\[(?:tokio::)?test\b")
_FEATURE_TOKEN_RE = re.compile(r'(?P<neg>not\s*\(\s*)?feature\s*=\s*"(?P<feat>[^"]+)"')
_MOD_RE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+")


def _all_cfg_feature_names(text: str) -> set[str]:
    """Every feature named by a line-anchored ``#[cfg(...feature = "X"...)]``."""
    names: set[str] = set()
    for match in _CFG_ATTR_RE.finditer(text):
        for token in _FEATURE_TOKEN_RE.finditer(match.group("body")):
            names.add(token.group("feat"))
    return names


def _read_test_file(path: Path) -> dict | None:
    text = path.read_text(encoding="utf-8")
    inner = _INNER_FEATURE_RE.search(text)
    fn_features = sorted(_all_cfg_feature_names(text))
    if inner is None and not fn_features:
        return None
    return {
        "file_feature": inner.group(1) if inner else None,
        "fn_features": fn_features,
        "text": text,
    }


def _scan_crate_tests(root: Path) -> dict[str, dict[str, dict]]:
    """Per crate, the gated test targets: ``crate -> filename -> gate info``."""
    crates_dir = root / "crates"
    if not crates_dir.is_dir():
        return {}
    out: dict[str, dict[str, dict]] = {}
    for crate_dir in sorted(p for p in crates_dir.iterdir() if p.is_dir()):
        tests_dir = crate_dir / "tests"
        if not tests_dir.is_dir():
            continue
        targets: dict[str, dict] = {}
        for path in sorted(tests_dir.glob("*.rs")):
            info = _read_test_file(path)
            if info is not None:
                targets[path.name] = info
        if targets:
            out[crate_dir.name] = targets
    return out


# ── literal/comment masking (so brace depth ignores strings) ──────────────────


def _mask_literals_and_comments(text: str) -> str:
    """Replace string/char literals and comments with spaces, keeping newlines.

    Brace counting is only used to attribute test items to a feature-gated
    ``mod`` body, so masking strings (which routinely contain ``{`` in test
    fixtures) is what keeps the depth honest.
    """
    out: list[str] = []
    i = 0
    n = len(text)
    mode = None  # None | "line" | "block" | "str" | "raw" | "char"
    raw_hashes = 0
    while i < n:
        c = text[i]
        if mode is None:
            if c == "/" and i + 1 < n and text[i + 1] == "/":
                mode = "line"
                i += 2
                out.append("  ")
                continue
            if c == "/" and i + 1 < n and text[i + 1] == "*":
                mode = "block"
                i += 2
                out.append("  ")
                continue
            if c == '"':
                mode = "str"
                i += 1
                out.append(" ")
                continue
            if c == "r":
                j = i + 1
                h = 0
                while j < n and text[j] == "#":
                    h += 1
                    j += 1
                if j < n and text[j] == '"':
                    mode = "raw"
                    raw_hashes = h
                    i = j + 1
                    out.append(" ")
                    continue
            if c == "'":
                # A char literal ('a', '\n', '{', '}') is masked; a lifetime
                # ('static) is not. Detect the closing quote two chars out.
                if i + 2 < n and (text[i + 2] == "'" or (text[i + 1] == "\\" and n > i + 3)):
                    mode = "char"
                    i += 1
                    out.append(" ")
                    continue
            out.append(c)
            i += 1
            continue
        if mode == "line":
            if c == "\n":
                mode = None
                out.append("\n")
                i += 1
                continue
            out.append(" ")
            i += 1
            continue
        if mode == "block":
            if c == "*" and i + 1 < n and text[i + 1] == "/":
                mode = None
                i += 2
                out.append("  ")
                continue
            out.append("\n" if c == "\n" else " ")
            i += 1
            continue
        if mode == "str":
            if c == "\\":
                out.append("  ")
                i += 2
                continue
            if c == '"':
                mode = None
                i += 1
                out.append(" ")
                continue
            out.append("\n" if c == "\n" else " ")
            i += 1
            continue
        if mode == "raw":
            if c == '"':
                j = i + 1
                h = 0
                while j < n and text[j] == "#":
                    h += 1
                    j += 1
                if h >= raw_hashes:
                    mode = None
                    i = j
                    out.append(" ")
                    continue
            out.append("\n" if c == "\n" else " ")
            i += 1
            continue
        if mode == "char":
            if c == "\\":
                out.append("  ")
                i += 2
                continue
            if c == "'":
                mode = None
                i += 1
                out.append(" ")
                continue
            out.append(" ")
            i += 1
            continue
    return "".join(out)


def _brace_depths_per_line(masked: str) -> list[int]:
    """Brace depth at the START of each line of ``masked`` text."""
    depths: list[int] = []
    depth = 0
    for line in masked.split("\n"):
        depths.append(depth)
        depth += line.count("{") - line.count("}")
    return depths


def _chain_requirement(chain: Sequence[str]) -> tuple[set[str], set[str]]:
    """(positive, negative) feature requirements from an attribute chain."""
    pos: set[str] = set()
    neg: set[str] = set()
    for attr in chain:
        m = _CFG_ATTR_RE.match(attr)
        if not m:
            continue
        for token in _FEATURE_TOKEN_RE.finditer(m.group("body")):
            if token.group("neg"):
                neg.add(token.group("feat"))
            else:
                pos.add(token.group("feat"))
    return pos, neg


def _test_item_requirements(text: str) -> list[tuple[frozenset[str], frozenset[str]]]:
    """Per test item, the (positive, negative) feature requirements.

    Positive: the named feature must be enabled for the test to run. Negative
    (``#[cfg(not(feature = "X"))]``): the named feature must be DISABLED. An
    item with no requirements runs under any feature set. Items inside a
    feature-gated ``mod`` inherit that module's requirement.
    """
    masked = _mask_literals_and_comments(text)
    depths = _brace_depths_per_line(masked)
    lines = text.splitlines()
    items: list[tuple[frozenset[str], frozenset[str]]] = []
    module_gates: list[tuple[int, frozenset[str], frozenset[str]]] = []
    chain: list[str] = []
    for idx, line in enumerate(lines):
        if _ATTR_LINE_RE.match(line):
            chain.append(line)
            continue
        if chain:
            pos, neg = _chain_requirement(chain)
            is_test = any(_TEST_ATTR_RE.match(c) for c in chain)
            is_mod = bool(_MOD_RE.match(line)) and "{" in line
            if is_mod and (pos or neg):
                module_gates.append((depths[idx] + 1, frozenset(pos), frozenset(neg)))
            if is_test:
                item_pos, item_neg = set(pos), set(neg)
                for body_depth, gate_pos, gate_neg in module_gates:
                    if depths[idx] >= body_depth:
                        item_pos |= gate_pos
                        item_neg |= gate_neg
                items.append((frozenset(item_pos), frozenset(item_neg)))
            chain = []
    return items


def _satisfiable(pos: frozenset[str], neg: frozenset[str], enabled: frozenset[str]) -> bool:
    """Can a test with these requirements run under this enabled feature set?"""
    return pos <= enabled and not (neg & enabled)


# ── the axis ─────────────────────────────────────────────────────────────────


@dataclass(frozen=True)
class GatedTarget:
    """One test target that carries a cfg feature gate."""

    crate: str
    file: str
    file_level: bool
    gate_features: tuple[str, ...]
    executes_tests: bool
    detail: str

    @property
    def name(self) -> str:
        return f"{self.crate}/tests/{self.file}"

    def to_dict(self) -> dict:
        return {
            "target": self.name,
            "crate": self.crate,
            "file": self.file,
            "file_level": self.file_level,
            "gate_features": list(self.gate_features),
            "executes_tests": self.executes_tests,
            "zero_executing": not self.executes_tests,
            "detail": self.detail,
        }


@dataclass(frozen=True)
class FeatureGatedCoverage:
    """The feature-gated coverage axis: executed gated targets over all gated."""

    root: str
    targets: tuple[GatedTarget, ...]

    @property
    def gated_total(self) -> int:
        return len(self.targets)

    @property
    def executed_total(self) -> int:
        return sum(1 for t in self.targets if t.executes_tests)

    @property
    def zero_executing(self) -> tuple[GatedTarget, ...]:
        return tuple(t for t in self.targets if not t.executes_tests)

    @property
    def ratio(self) -> float:
        if not self.targets:
            return 1.0
        return self.executed_total / self.gated_total

    def to_dict(self) -> dict:
        return {
            "axis": "feature_gated_coverage",
            "definition": (
                "gated test targets that execute a non-zero number of tests over all "
                "test targets carrying a cfg feature gate, under the feature sets the "
                "architecture gate actually builds"
            ),
            "root": self.root,
            "gated_targets": self.gated_total,
            "executed_targets": self.executed_total,
            "ratio": self.ratio,
            "zero_executing_count": len(self.zero_executing),
            "zero_executing_targets": [t.name for t in self.zero_executing],
            "targets": [t.to_dict() for t in self.targets],
        }

    def render_lines(self) -> list[str]:
        """The axis as the lines a session reads first."""
        pct = self.ratio * 100.0
        zero = self.zero_executing
        first = (
            f"feature-gated coverage: {self.executed_total}/{self.gated_total} "
            f"({pct:.1f}%) gated test targets execute tests"
        )
        if not zero:
            return [first, "  zero-executing: none"]
        names = ", ".join(f"{t.name} [{','.join(t.gate_features)}]" for t in zero)
        return [first, f"  zero-executing ({len(zero)}): {names}"]


def _target_gate_features(info: dict) -> tuple[str, ...]:
    if info["file_feature"] is not None:
        return (info["file_feature"],)
    return tuple(info["fn_features"])


def _evaluate_target(
    crate: str,
    filename: str,
    info: dict,
    steps: Sequence[dict],
    default_features: frozenset[str],
) -> GatedTarget:
    file_level = info["file_feature"] is not None
    gate_features = _target_gate_features(info)
    exec_steps = _executing_steps(steps, crate)
    enabled_sets = [frozenset(s["features"]) | default_features for s in exec_steps]

    if file_level:
        feature = info["file_feature"]
        executes = any(feature in enabled for enabled in enabled_sets)
        if executes:
            detail = (
                f"file gate #![cfg(feature = \"{feature}\")]: "
                "a `--tests` step enables the feature, so the target is not empty"
            )
        else:
            detail = (
                f"file gate #![cfg(feature = \"{feature}\")]: no `--tests` step "
                "enables the feature, so the target compiles EMPTY and executes zero tests"
            )
        return GatedTarget(crate, filename, True, gate_features, executes, detail)

    if not exec_steps:
        return GatedTarget(
            crate,
            filename,
            False,
            gate_features,
            False,
            "no gate step runs `cargo test --tests` for this crate, so the target "
            "never executes",
        )

    items = _test_item_requirements(info["text"])
    if not items:
        # Fail closed: a target carrying a feature gate whose test items could
        # not be parsed is reported as zero-executing rather than assumed green.
        return GatedTarget(
            crate,
            filename,
            False,
            gate_features,
            False,
            "no test item could be parsed but the file carries a feature gate; "
            "reported zero-executing rather than assumed",
        )

    executes = any(
        _satisfiable(pos, neg, enabled) for enabled in enabled_sets for pos, neg in items
    )
    if executes:
        detail = (
            "at least one test item is runnable under an executing gate step "
            "(ungated test, or a gate feature the step enables)"
        )
    else:
        detail = (
            "every test item is gated on a feature no `--tests` step enables, so the "
            "target compiles with zero runnable tests"
        )
    return GatedTarget(crate, filename, False, gate_features, executes, detail)


def build_feature_gated_coverage(
    root: Path,
    gate_steps: Iterable[GateStep],
) -> FeatureGatedCoverage:
    """Compute the axis for ``root`` against the given gate step DATA.

    ``gate_steps`` is injected rather than imported so the axis can be measured
    against a fixture repository. Pass ``governor.__main__.architecture_gate_steps()``
    for the real tree (see :func:`observe_feature_gated_coverage`).

    Raises ``FileNotFoundError`` when ``root`` has no ``crates/`` directory: an
    empty scan would otherwise report a vacuous ``0/0`` green and hide a bad
    root. Individual source-read errors are left to propagate.
    """
    root = Path(root)
    if not (root / "crates").is_dir():
        raise FileNotFoundError(
            f"{root} has no crates/ directory; refusing to report a vacuous green axis"
        )
    steps = _parse_gate_steps(gate_steps)
    scanned = _scan_crate_tests(root)
    targets: list[GatedTarget] = []
    for crate in sorted(scanned):
        default_features = _crate_default_features(root, crate)
        for filename in sorted(scanned[crate]):
            targets.append(
                _evaluate_target(crate, filename, scanned[crate][filename], steps, default_features)
            )
    return FeatureGatedCoverage(root=str(root), targets=tuple(targets))


def _default_gate_steps() -> list[GateStep]:
    """The real gate DATA, without re-importing ``governor.__main__``.

    When running as ``python3 -m governor`` the composition root is loaded as
    ``__main__``; importing ``governor.__main__`` again would create a second
    module object. Prefer whichever is already loaded.
    """
    for module_name in ("governor.__main__", "__main__"):
        module = sys.modules.get(module_name)
        if module is not None and hasattr(module, "architecture_gate_steps"):
            return list(module.architecture_gate_steps())
    from governor.__main__ import architecture_gate_steps  # local import avoids a cycle

    return list(architecture_gate_steps())


def observe_feature_gated_coverage(
    root: Path | None = None,
    gate_steps: Iterable[GateStep] | None = None,
) -> FeatureGatedCoverage:
    """Observe the axis on the real tree (or a given root) against the real gate."""
    if root is None:
        root = Path(__file__).resolve().parent.parent.parent.parent
    if gate_steps is None:
        gate_steps = _default_gate_steps()
    return build_feature_gated_coverage(Path(root), gate_steps)
