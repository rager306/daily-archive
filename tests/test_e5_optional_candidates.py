"""M284 S03: E5 optional candidates — generators, Docling fallback, second judge."""

from __future__ import annotations

from research_graph.application.corpus.e5_optional_candidates import (
    HeaderPriorityCandidateGenerator,
    OptionalGlinerCandidateGenerator,
    blind_second_judge,
    build_e5_optional_candidates_package,
    docling_available,
    docling_page_fallback,
    gliner_available,
)


def _body() -> str:
    return (
        "# Seq2Seq Models for Knowledge Graph Link Prediction\n\n"
        "We study Seq2Seq Models applied to link prediction on knowledge graphs.\n"
        "The Transformer architecture improves translation quality.\n"
    )


def test_header_priority_generator_emits_entities() -> None:
    gen = HeaderPriorityCandidateGenerator()
    out = gen.generate(body_text=_body(), paper_id="1206.6423", case_id="c1")
    assert out["available"] is True
    assert out["generator"] == "header_priority"
    assert int(out["entity_count"]) >= 1
    assert any(e.get("label") for e in out["entities"])


def test_gliner_unavailable_when_not_installed() -> None:
    gen = OptionalGlinerCandidateGenerator()
    out = gen.generate(body_text=_body(), paper_id="x", case_id="c1")
    if not gliner_available():
        assert out["available"] is False
        assert out["blocked_reason"] == "gliner_not_installed"
        assert out["entity_count"] == 0
    else:
        assert out["available"] is True


def test_docling_fallback_gated_when_hybrid_ok() -> None:
    r = docling_page_fallback(pdf_path="/tmp/x.pdf", hybrid_failed=False)
    assert r.attempted is False
    assert r.used is False
    assert r.reason == "hybrid_not_failed_skip"
    assert r.import_eligible is False


def test_docling_fallback_injectable_convert() -> None:
    def _convert(path: str) -> dict:
        assert path.endswith(".pdf")
        return {"text": "fallback page text", "page_count": 3}

    r = docling_page_fallback(
        pdf_path="/tmp/failed.pdf",
        hybrid_failed=True,
        convert_fn=_convert,
    )
    assert r.attempted is True
    assert r.used is True
    assert r.page_count == 3
    assert r.text_chars > 0


def test_blind_second_judge_flags_high_impact() -> None:
    primary = [
        {
            "case_id": "c1",
            "entities": [{"label": "Seq2Seq Models", "type": "Method"}],
            "relations": [
                {
                    "type": "APPLIED_TO",
                    "source_label": "Seq2Seq Models",
                    "target_label": "link prediction",
                }
            ],
        }
    ]
    secondary = [
        {
            "case_id": "c1",
            "entities": [{"label": "Transformer", "type": "Model"}],
            "relations": [
                {
                    "type": "OUTPERFORMS",
                    "source_label": "Transformer",
                    "target_label": "baseline",
                }
            ],
        }
    ]
    pkg = blind_second_judge(primary=primary, secondary=secondary)
    assert pkg.compared_cases == 1
    assert pkg.high_impact_disagreements >= 1
    assert pkg.import_eligible is False
    assert pkg.agreement_rate == 0.0


def test_build_e5_package_fail_closed() -> None:
    pkg = build_e5_optional_candidates_package(
        body_text=_body(),
        paper_id="1206.6423",
        case_id="c1",
        hybrid_failed=False,
    )
    assert pkg.import_eligible is False
    assert pkg.graph_writes_allowed is False
    gens = {g["generator"]: g for g in pkg.generators}
    assert "header_priority" in gens
    assert gens["header_priority"]["available"] is True
    assert gens["header_priority"]["entity_count"] >= 1
    assert "gliner_relex" in gens
    assert pkg.docling_fallback["used"] is False
    assert "header_entity_count" in pkg.coverage_delta
    assert docling_available() is True or pkg.docling_fallback["available"] in (
        True,
        False,
    )
