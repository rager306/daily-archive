"""M451 W4: article manifest content checker — negative fixtures."""
import json
from pathlib import Path
import yaml


def _repo(tmp_path: Path) -> Path:
    root = tmp_path / "repo"
    (root / "scripts" / "governor" / "checks").mkdir(parents=True)
    (root / "scripts" / "governor" / "checks" / "article_manifest.yaml").write_text(
        "schema_version: article-manifest-check.v1\n"
        "dimension: article_manifest\nseverity: medium\n"
        "norm: data/schemas/article_schema.yaml\n"
        "processed_root: data/processed\n"
        "manifest_name: manifest.yaml\n"
        "invariants: [lifecycle_is_derived, transaction_time_monotonic, "
        "derived_from_exists, ast_covers_sections, no_orphan_entities, artifact_files_exist]\n"
        "sample_limit: 0\n"
    )
    (root / "data" / "schemas").mkdir(parents=True)
    (root / "data" / "schemas" / "article_schema.yaml").write_text(
        "schema_version: article-schema.v1\n"
        "lifecycle_order: [raw, parsed, structured, extracted]\n"
        "layers:\n"
        "  - {id: source, authority: A0}\n"
        "  - {id: parsed, authority: A1}\n"
        "  - {id: context, authority: A2}\n"
        "  - {id: extracted, authority: A3}\n"
    )
    return root


def _article(root: Path, paper: str, **overrides) -> Path:
    d = root / "data" / "processed" / paper
    (d / "parsed").mkdir(parents=True, exist_ok=True)
    (d / "context").mkdir(exist_ok=True)
    (d / "extracted").mkdir(exist_ok=True)
    (d / "parsed" / "article.json").write_text("{}")
    (d / "context" / "projection.json").write_text("{}")
    (d / "extracted" / "entities.json").write_text(json.dumps(
        [{"label": "x", "entity_type": "Method", "section_ordinal": 0}]
    ))
    m = {
        "schema_version": "article-manifest.v1",
        "paper_id": paper,
        "lifecycle": "extracted",
        "lifecycle_events": [
            {"stage": "parsed", "recorded_at": "2026-08-23T01:00:00Z", "actor": "grobid"},
            {"stage": "structured", "recorded_at": "2026-08-23T02:00:00Z", "actor": "dcp"},
            {"stage": "extracted", "recorded_at": "2026-08-23T03:00:00Z", "actor": "rbe"},
        ],
        "artifacts": {
            "parsed": {"file": "parsed/article.json", "derived_from": ["source"], "stats": {"sections": 3}},
            "context": {"file": "context/projection.json", "derived_from": ["parsed"], "stats": {"ast_nodes": 3}},
            "extracted": {"file": "extracted/entities.json", "derived_from": ["parsed"], "stats": {"entities": 1}},
        },
    }
    m.update(overrides)
    (d / "manifest.yaml").write_text(yaml.safe_dump(m, sort_keys=False))
    return d


def _flags(root: Path):
    from governor.adapters.article_manifest_checker import check_article_manifests
    return check_article_manifests(root)


def test_clean_article_passes(tmp_path):
    root = _repo(tmp_path)
    _article(root, "2501.00001")
    assert _flags(root) == []


def test_handset_lifecycle_flagged(tmp_path):
    root = _repo(tmp_path)
    _article(root, "2501.00002", lifecycle="validated")  # not derived from last event
    assert any(f.flag == "lifecycle_not_derived" for f in _flags(root))


def test_ast_mismatch_flagged(tmp_path):
    root = _repo(tmp_path)
    d = _article(root, "2501.00003")
    m = yaml.safe_load((d / "manifest.yaml").read_text())
    m["artifacts"]["context"]["stats"]["ast_nodes"] = 9
    (d / "manifest.yaml").write_text(yaml.safe_dump(m, sort_keys=False))
    assert any(f.flag == "ast_covers_sections" for f in _flags(root))


def test_orphan_entity_flagged(tmp_path):
    root = _repo(tmp_path)
    d = _article(root, "2501.00004")
    (d / "extracted" / "entities.json").write_text(json.dumps(
        [{"label": "orphan", "entity_type": "Method"}]  # no section_ordinal
    ))
    assert any(f.flag == "orphan_entities" for f in _flags(root))


def test_missing_artifact_file_flagged(tmp_path):
    root = _repo(tmp_path)
    d = _article(root, "2501.00005")
    (d / "context" / "projection.json").unlink()  # FS = truth: file gone -> flag
    assert any(f.flag == "artifact_file_missing" for f in _flags(root))


def test_unknown_stage_flagged(tmp_path):
    root = _repo(tmp_path)
    _article(root, "2501.00006", lifecycle_events=[
        {"stage": "teleported", "recorded_at": "2026-08-23T01:00:00Z", "actor": "?"},
    ])
    assert any(f.flag == "stage_not_in_norm" for f in _flags(root))
