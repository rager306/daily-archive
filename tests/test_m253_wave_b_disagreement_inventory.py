"""M253: Reviewed disagreement inventory via extraction harness."""

from __future__ import annotations

from research_graph.application.corpus.wave_b_disagreement_inventory import (
    WaveBDisagreementInventoryPackage,
    inventory_reviewed_extraction_disagreements,
)


def test_inventory_scores_m072_fixtures() -> None:
    pkg = inventory_reviewed_extraction_disagreements()
    assert pkg.import_eligible is False
    assert pkg.dspy_optimizer_enabled is False
    assert pkg.train_case_count >= 1
    assert pkg.validation_case_count >= 1
    assert pkg.leakage_clean is True
    assert pkg.train_entity_f1 >= 0.0
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["wave"] == "B"
    assert "disagreement_kind_counts" in d


def test_rejects_dspy_true() -> None:
    import pytest

    with pytest.raises(ValueError):
        WaveBDisagreementInventoryPackage(
            schema_version="x",
            train_case_count=0,
            validation_case_count=0,
            train_disagreement_count=0,
            validation_disagreement_count=0,
            disagreement_kind_counts={},
            train_entity_f1=0.0,
            validation_entity_f1=0.0,
            leakage_clean=True,
            samples=(),
            diagnostics=(),
            dspy_optimizer_enabled=True,
        )
