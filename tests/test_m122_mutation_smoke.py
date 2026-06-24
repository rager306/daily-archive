from __future__ import annotations

from scripts import run_m122_mutation_smoke


def test_mutation_smoke_specs_are_unique_and_targets_exist() -> None:
    specs = run_m122_mutation_smoke.mutation_specs()

    assert len(specs) == 4
    assert len({spec.name for spec in specs}) == len(specs)
    for spec in specs:
        assert spec.path.exists()
        assert spec.old in spec.path.read_text(encoding="utf-8")
        assert spec.old != spec.new
        assert spec.tests
        assert all(test.startswith("tests/") for test in spec.tests)
