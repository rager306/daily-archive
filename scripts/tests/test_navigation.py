"""M451 v10: PageIndex navigation — grammar, queries, symmetry."""
import importlib.util
import json
from pathlib import Path

_MOD = Path(__file__).resolve().parent.parent / "m451_navigation_build.py"
_spec = importlib.util.spec_from_file_location("m451_nav", _MOD)
nav = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(nav)


def test_parent_path_grammar():
    assert nav.parent_path("1.2.3") == "1.2"
    assert nav.parent_path("1.2") == "1"
    assert nav.parent_path("1") is None
    assert nav.parent_path("1.") is None  # header node is root-level metadata


def test_navigation_built_and_symmetric(tmp_path):
    # fixture article
    d = tmp_path / "2301.00001"
    (d / "context").mkdir(parents=True)
    (d / "context" / "projection.json").write_text(json.dumps({
        "paper_id": "2301.00001",
        "sections": [
            {"section_id": "2301.00001:0", "path_id": "1", "order": 0, "level": 1, "title": "Intro", "char_count": 500},
            {"section_id": "2301.00001:1", "path_id": "1.1", "order": 1, "level": 2, "title": "Setup", "char_count": 400},
            {"section_id": "2301.00001:2", "path_id": "2", "order": 2, "level": 1, "title": "Results", "char_count": 600},
        ]}))
    (d / "manifest.yaml").write_text(
        "schema_version: article-manifest.v1\npaper_id: 2301.00001\nlifecycle: structured\n"
        "lifecycle_events:\n  - {stage: structured, recorded_at: '1', actor: t}\n"
        "artifacts:\n  context: {file: context/projection.json, derived_from: [parsed]}\n")
    nav.build_navigation("2301.00001", tmp_path)

    nodes = json.loads((d / "context" / "navigation.json").read_text())
    by = {n["section_id"]: n for n in nodes}
    assert by["2301.00001:1"]["parent_section_id"] == "2301.00001:0"
    assert by["2301.00001:0"]["children_section_ids"] == ["2301.00001:1"]
    assert by["2301.00001:0"]["next_section_id"] == "2301.00001:1"
    assert by["2301.00001:1"]["prev_section_id"] == "2301.00001:0"
    assert by["2301.00001:2"]["parent_section_id"] is None

    import yaml
    m = yaml.safe_load((d / "manifest.yaml").read_text())
    assert m["artifacts"]["context"]["navigation"]["stats"]["nodes"] == 3
