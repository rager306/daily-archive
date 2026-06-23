from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from types import ModuleType

RUNNER = Path(__file__).resolve().parents[1] / "scripts" / "run_m122_mutation_smoke.py"


def _load_runner() -> ModuleType:
    spec = importlib.util.spec_from_file_location("run_m122_mutation_smoke", RUNNER)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_mutation_smoke_specs_are_unique_and_targets_exist() -> None:
    runner = _load_runner()

    specs = runner.mutation_specs()

    assert len(specs) == 4
    assert len({spec.name for spec in specs}) == len(specs)
    for spec in specs:
        assert spec.path.exists()
        assert spec.old in spec.path.read_text(encoding="utf-8")
        assert spec.old != spec.new
        assert spec.tests
        assert all(test.startswith("tests/") for test in spec.tests)
