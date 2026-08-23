"""W4 (M451): manifest.json (M450 stage ledger) -> manifest.yaml (ADR-068 article artifact).

Normative definition: data/schemas/article_schema.yaml (single definition site).
The migrator is a data transform, not a pipeline stage.
"""
import json
from pathlib import Path
import yaml


def _make_article(root: Path, paper_id: str = "1203.2295") -> Path:
    d = root / "processed" / paper_id
    (d / "parsed").mkdir(parents=True)
    (d / "context").mkdir()
    (d / "extracted").mkdir()
    (d / "parsed" / "article.json").write_text("{}")
    (d / "context" / "projection.json").write_text("{}")
    (d / "extracted" / "entities.json").write_text("[]")
    (d / "manifest.json").write_text(json.dumps({
        "schema_version": "article-manifest.v2",
        "paper_id": paper_id,
        "created_at": "2026-08-23T05:49:26.886584+00:00",
        "source": {"code": "arxiv", "identity_ref": f"sources/arxiv/math-oc/{paper_id}/identity.json"},
        "stages": {
            "parsed": {"status": "complete", "at": "2026-08-23T05:49:26.889927+00:00",
                       "output_sha256": "a" * 64, "sections_count": 10},
            "context": {"status": "complete", "at": "2026-08-23T05:49:26.890328+00:00",
                        "output_sha256": "b" * 64, "ast_nodes": 10},
            "extracted": {"status": "complete", "at": "2026-08-23T05:49:26.892469+00:00",
                          "output_sha256": "c" * 64, "entity_count": 2,
                          "entity_types": {"Method": 2}},
            "enriched": {"status": "pending"},
            "materialized": {"status": "pending"},
            "verified": {"status": "pending"},
            "embedded": {"status": "complete", "cache_key": "e" * 64},
        },
    }))
    return root / "processed"


def test_migrate_produces_derived_lifecycle_and_events(tmp_path):
    from importlib import util
    mod_path = Path(__file__).resolve().parent.parent / "m451_migrate_article_manifests.py"
    spec = util.spec_from_file_location("m451_migrate", mod_path)
    mod = util.module_from_spec(spec)
    spec.loader.exec_module(mod)

    proc = _make_article(tmp_path)
    out = mod.migrate_article(proc / "1203.2295")
    m = yaml.safe_load((proc / "1203.2295" / "manifest.yaml").read_text())

    # lifecycle DERIVED from events, graph stages (embedded/materialized/verified) excluded
    assert m["lifecycle"] == "extracted"
    stages = [e["stage"] for e in m["lifecycle_events"]]
    assert stages == ["parsed", "structured", "extracted"]
    assert all("observed_at" not in e for e in m["lifecycle_events"])  # no external anchor in M450
    assert m["lifecycle_events"][0]["actor"] == "grobid"

    # artifact graph with derived_from + stats
    assert m["artifacts"]["context"]["derived_from"] == ["parsed"]
    assert m["artifacts"]["parsed"]["stats"] == {"sections": 10}
    assert m["artifacts"]["context"]["stats"] == {"ast_nodes": 10}
    assert m["artifacts"]["extracted"]["stats"]["entities"] == 2

    # source locator preserved
    assert m["source"]["identity_ref"].endswith("identity.json")
    assert out.endswith("manifest.yaml")
