"""M451 v5: reference layer builder — figures + external_resources from TEI."""
import importlib.util
import json
from pathlib import Path

_MOD = Path(__file__).resolve().parent.parent / "m451_reference_layers.py"
_spec = importlib.util.spec_from_file_location("m451_ref_layers", _MOD)
mod = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(mod)

TEI = """<TEI><body>
<figure xml:id="fig_0"><head>Figure 1.</head><label>1</label>
<figDesc>Caption text here.</figDesc><graphic url="f1.png"/></figure>
<figure type="table" xml:id="tab_0"><cell/></figure>
<ref type="figure" target="#fig_0">Fig</ref>
<ref type="figure" target="#fig_0">Fig</ref>
<ptr target="https://github.com/org/repo"/>
<ptr target="https://huggingface.co/model"/>
<ptr target="https://unknown.example/x"/>
</body></TEI>"""


def _article(tmp_path: Path) -> Path:
    d = tmp_path / "2501.00001"
    (d / "parsed").mkdir(parents=True)
    (d / "context").mkdir()
    (d / "extracted").mkdir()
    (d / "parsed" / "article.json").write_text(json.dumps({
        "paper_id": "2501.00001", "sections": [{"section_id": "s0", "title": "T", "text": "x", "level": 1, "path_id": "1.", "parent_section_id": None, "order_in_parent": 0}],
        "citations": [{"raw_text": "r", "doi": None, "arxiv_id": "2401.00001", "title": "T", "anchors": []}],
        "formulas": [{"formula_id": "formula_0", "section_path_id": "1.", "section_ordinal": 0, "label": "(1)", "representation": {"plain_text": "x"}, "source_digest": None, "missingness": None}],
        "tei_xml": TEI, "table_missingness": "no_table_elements",
    }))
    (d / "context" / "projection.json").write_text("{}")
    (d / "extracted" / "entities.json").write_text(json.dumps([{"label": "m", "entity_type": "Method", "section_ordinal": 0}]))
    (d / "manifest.json").write_text(json.dumps({
        "schema_version": "article-manifest.v2", "paper_id": "2501.00001", "created_at": "2026-08-23T00:00:00Z",
        "source": {"code": "arxiv", "identity_ref": "x"},
        "stages": {
            "parsed": {"status": "complete", "at": "2026-08-23T01:00:00Z", "output_sha256": "a", "sections_count": 1},
            "context": {"status": "complete", "at": "2026-08-23T02:00:00Z", "output_sha256": "b", "ast_nodes": 1},
            "extracted": {"status": "complete", "at": "2026-08-23T03:00:00Z", "output_sha256": "c", "entity_count": 1},
        }}))
    (d / "manifest.yaml").write_text(
        "schema_version: article-manifest.v1\n"
        "paper_id: 2501.00001\n"
        "source: {code: arxiv, identity_ref: x}\n"
        "lifecycle: extracted\n"
        "lifecycle_events:\n"
        "  - {stage: parsed, recorded_at: '2026-08-23T01:00:00Z', actor: grobid}\n"
        "  - {stage: structured, recorded_at: '2026-08-23T02:00:00Z', actor: dcp}\n"
        "  - {stage: extracted, recorded_at: '2026-08-23T03:00:00Z', actor: rbe}\n"
        "artifacts: {}\n"
    )
    return d.parent


def test_figures_and_external_resources_built(tmp_path):
    proc = _article(tmp_path)
    d = proc / "2501.00001"
    mod.build("2501.00001", root=proc)

    figs = json.loads((d / "figures" / "index.json").read_text())
    # only the untyped figure (tab_0 belongs to tables layer)
    assert len(figs) == 1
    f = figs[0]
    assert f["figure_id"] == "fig_0"
    assert f["ref_count"] == 2          # both in-text refs counted
    assert f["has_graphic"] is True
    assert f["caption"].startswith("Caption")

    ext = json.loads((d / "external_resources" / "index.json").read_text())
    by_dom = {e["domain"]: e for e in ext}
    assert by_dom["github.com"]["resource_type"] == "code"
    assert by_dom["huggingface.co"]["resource_type"] == "model_hub"
    assert by_dom["unknown.example"]["resource_type"] == "other"  # dictionary fallback

    import yaml
    m = yaml.safe_load((d / "manifest.yaml").read_text())
    assert m["artifacts"]["figures"]["stats"]["count"] == 1
    assert m["artifacts"]["external_resources"]["stats"]["unique_urls"] == 3
    assert m["artifacts"]["tables"]["missingness"] == "no_table_elements"

    # quality_signals: deterministic, policy-driven
    qs = json.loads((d / "quality_signals.json").read_text())
    by_id = {s["signal_id"]: s for s in qs}
    assert set(by_id) >= {"sections_text_completeness", "citation_anchor_coverage",
                          "figure_caption_coverage", "code_available"}
    assert all(s["verdict"] in ("ok", "warn", "alarm", "info") for s in qs)
    assert by_id["figure_caption_coverage"]["value"] == 1.0
    assert by_id["code_available"]["value"] == 1  # github.com in fixture TEI
    assert m["artifacts"]["quality_signals"]["stats"]["count"] == len(qs)
    assert "enriched" in m["artifacts"] and m["artifacts"]["enriched"]["not_reached"]
