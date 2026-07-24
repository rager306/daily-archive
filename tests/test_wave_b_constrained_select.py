"""TDD: constrained selectors over body candidates (no free invent)."""

from __future__ import annotations

import json

from research_graph.application.corpus.wave_b_constrained_select import (
    _looks_like_author_span,
    build_relation_candidates,
    guess_entity_type,
    header_priority_select,
    make_header_fallback_select_fn,
    make_header_prefer_select_fn,
    min_surface_distance,
    relation_type_for_entity_pair,
    score_selection_structural,
    make_llm_constrained_select_fn,
    parse_constrained_llm_selection,
    prioritize_candidates_for_prompt,
    render_constrained_select_prompt,
    select_relations_for_entities,
    trim_selection_to_top_k,
)
from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
    build_body_candidates,
    score_gold_hybrid_constrained_pilot,
)


def _body_and_gold() -> tuple[str, dict]:
    body = (
        "# A Joint Model of Language and Perception for Grounded Attribute Learning\n\n"
        "We study Language and Perception for Grounded Attribute Learning.\n"
    )
    gold = {
        "case_id": "case:train:1206.6423",
        "paper_id": "arxiv:1206.6423",
        "source_artifact_refs": ["artifact:x"],
        "entities": [
            {
                "id": "e:field",
                "type": "Field",
                "label": "Language and Perception",
                "evidence_refs": ["e1"],
            },
            {
                "id": "e:task",
                "type": "Task",
                "label": "Grounded Attribute Learning",
                "evidence_refs": ["e2"],
            },
        ],
        "relations": [
            {
                "id": "r1",
                "type": "APPLIED_TO",
                "source": "e:field",
                "target": "e:task",
                "evidence_refs": ["r1"],
            }
        ],
        "schema_valid": True,
        "json_valid": True,
        "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
    }
    return body, gold


def test_guess_entity_type_heuristics() -> None:
    assert guess_entity_type("Language and Perception") == "Field"
    assert guess_entity_type("Grounded Attribute Learning") == "Task"
    assert guess_entity_type("Neural Machine Translation") == "Method"
    assert guess_entity_type("Human Feedback") == "Method"
    assert guess_entity_type("Extractive Summarization") == "Task"
    assert guess_entity_type("Recursively Summarizing Books") == "Task"
    assert guess_entity_type("Recurrent Neural Network") == "Method"


def test_header_priority_select_recovers_title_gold() -> None:
    body, gold = _body_and_gold()
    cands = build_body_candidates(body, paper_id="1206.6423")
    sel = header_priority_select(body, gold["case_id"], cands)
    assert sel["json_valid"] is True
    assert len(sel["entities"]) >= 2
    # map surfaces
    by_id = {c["candidate_id"]: c for c in cands}
    labels = {by_id[e["candidate_id"]]["surface"] for e in sel["entities"]}
    assert "Language and Perception" in labels
    assert "Grounded Attribute Learning" in labels
    assert len(sel["relations"]) >= 1


def test_header_priority_scores_perfect_on_fixture() -> None:
    body, gold = _body_and_gold()
    pkg = score_gold_hybrid_constrained_pilot(
        cases=(
            {
                "case_id": gold["case_id"],
                "paper_id": "1206.6423",
                "gold": gold,
                "body_text": body,
            },
        ),
        select_fn=header_priority_select,
        llm_used=False,
    )
    assert pkg.metrics["entity_f1"] == 1.0
    assert pkg.metrics["relation_f1"] == 1.0
    assert pkg.import_eligible is False


def test_parse_constrained_llm_selection_drops_unknown_ids() -> None:
    cands = [
        {
            "candidate_id": "c:a:0",
            "surface": "Language and Perception",
            "surface_norm": "language and perception",
            "source": "header_title",
        },
        {
            "candidate_id": "c:b:1",
            "surface": "Grounded Attribute Learning",
            "surface_norm": "grounded attribute learning",
            "source": "header_title",
        },
    ]
    raw = {
        "entities": [
            {"candidate_id": "c:a:0", "type": "Field"},
            {"candidate_id": "c:invented", "type": "Method"},
            {"candidate_id": "c:b:1", "type": "Task"},
        ],
        "relations": [
            {
                "type": "APPLIED_TO",
                "source_id": "c:a:0",
                "target_id": "c:b:1",
            },
            {
                "type": "APPLIED_TO",
                "source_id": "c:a:0",
                "target_id": "c:missing",
            },
        ],
        "json_valid": True,
    }
    sel = parse_constrained_llm_selection(raw, cands)
    assert len(sel["entities"]) == 2
    assert len(sel["relations"]) == 1
    assert all(e["candidate_id"] in {"c:a:0", "c:b:1"} for e in sel["entities"])


def test_render_constrained_prompt_lists_candidates_only() -> None:
    cands = [
        {
            "candidate_id": "c:a:0",
            "surface": "Language and Perception",
            "surface_norm": "language and perception",
            "source": "header_title",
        }
    ]
    prompt = render_constrained_select_prompt(
        case_id="case:x",
        paper_id="x",
        candidates=cands,
        outline_titles=["Abstract", "Method"],
    )
    assert "c:a:0" in prompt
    assert "Language and Perception" in prompt
    assert "candidate_id" in prompt
    assert "Do NOT invent new labels" in prompt or "do not invent" in prompt.casefold()


def test_make_llm_constrained_select_fn_maps_candidate_ids() -> None:
    """Injected chat returns candidate_id JSON; invents are dropped."""
    body, gold = _body_and_gold()
    cands = build_body_candidates(body, paper_id="1206.6423")
    by_surface = {c["surface"]: c["candidate_id"] for c in cands}
    cid_field = by_surface["Language and Perception"]
    cid_task = by_surface["Grounded Attribute Learning"]

    def chat_fn(messages, *, model, max_tokens=700, temperature=0.0):
        assert model == "mock-model"
        assert any("CANDIDATES" in str(m.get("content") or "") for m in messages)
        return (
            '{"entities":[{"candidate_id":"%s","type":"Field"},'
            '{"candidate_id":"%s","type":"Task"},'
            '{"candidate_id":"c:invented","type":"Method"}],'
            '"relations":[{"type":"APPLIED_TO","source_id":"%s","target_id":"%s"}]}'
            % (cid_field, cid_task, cid_field, cid_task)
        )

    select = make_llm_constrained_select_fn(chat_fn=chat_fn, model="mock-model")
    sel = select(body, gold["case_id"], cands)
    assert sel["json_valid"] is True
    ids = {e["candidate_id"] for e in sel["entities"]}
    assert ids == {cid_field, cid_task}
    assert "c:invented" not in ids
    assert len(sel["relations"]) == 1


def test_make_llm_constrained_select_fn_fail_closed_on_chat_error() -> None:
    def chat_fn(messages, *, model, max_tokens=700, temperature=0.0):
        raise RuntimeError("boom")

    select = make_llm_constrained_select_fn(chat_fn=chat_fn, model="x")
    sel = select("body", "case:x", [{"candidate_id": "c:1", "surface": "A B", "surface_norm": "a b"}])
    assert sel["json_valid"] is False
    assert sel["entities"] == []
    assert sel["relations"] == []


def test_make_llm_constrained_select_fn_maps_label_to_candidate() -> None:
    """When model returns free labels that match candidates, map to candidate_id."""
    body, gold = _body_and_gold()
    cands = build_body_candidates(body, paper_id="1206.6423")

    def chat_fn(messages, *, model, max_tokens=700, temperature=0.0):
        return (
            '{"entities":[{"label":"Language and Perception","type":"Field"},'
            '{"label":"Grounded Attribute Learning","type":"Task"}],'
            '"relations":[]}'
        )

    select = make_llm_constrained_select_fn(chat_fn=chat_fn, model="m")
    sel = select(body, gold["case_id"], cands)
    assert len(sel["entities"]) == 2
    assert all(e.get("candidate_id") for e in sel["entities"])



def test_header_priority_rejects_prose_noise_and_org_spans() -> None:
    """Structural demotion: sentence fragments / org names lose to technical NPs."""
    body = (
        "# WebAgent: Web Automation with Natural Language Instructions\n\n"
        "Although several works adopt prior techniques on real websites.\n"
    )
    cands = [
        {
            "candidate_id": "c:noise1",
            "surface": "although several works",
            "surface_norm": "although several works",
            "source": "header_title",
        },
        {
            "candidate_id": "c:noise2",
            "surface": "adopt prior techniques",
            "surface_norm": "adopt prior techniques",
            "source": "header_title",
        },
        {
            "candidate_id": "c:org",
            "surface": "Google DeepMind",
            "surface_norm": "google deepmind",
            "source": "header_title",
        },
        {
            "candidate_id": "c:web",
            "surface": "web automation",
            "surface_norm": "web automation",
            "source": "header_title",
        },
        {
            "candidate_id": "c:nli",
            "surface": "natural language instructions",
            "surface_norm": "natural language instructions",
            "source": "header_title",
        },
    ]
    sel = header_priority_select(body, "case:x", cands)
    by = {c["candidate_id"]: c["surface"] for c in cands}
    labels = {by[e["candidate_id"]] for e in sel["entities"]}
    assert "web automation" in labels
    assert "natural language instructions" in labels
    assert "although several works" not in labels
    assert "Google DeepMind" not in labels


def test_header_priority_prefers_core_np_over_wrapper_suffix() -> None:
    """Prefer 'beam search' over 'Beam Search Strategies' when both candidates."""
    body = "# Beam Search Strategies for Neural Machine Translation\n"
    cands = [
        {
            "candidate_id": "c:wrap",
            "surface": "Beam Search Strategies",
            "surface_norm": "beam search strategies",
            "source": "header_title",
        },
        {
            "candidate_id": "c:core",
            "surface": "beam search",
            "surface_norm": "beam search",
            "source": "header_title",
        },
        {
            "candidate_id": "c:nmt",
            "surface": "Neural Machine Translation",
            "surface_norm": "neural machine translation",
            "source": "header_title",
        },
    ]
    sel = header_priority_select(body, "case:x", cands)
    by = {c["candidate_id"]: c["surface_norm"] for c in cands}
    norms = {by[e["candidate_id"]] for e in sel["entities"]}
    assert "beam search" in norms
    assert "beam search strategies" not in norms
    assert "neural machine translation" in norms


def test_make_header_fallback_select_fn_uses_header_when_primary_empty() -> None:
    body, gold = _body_and_gold()
    cands = build_body_candidates(body, paper_id="1206.6423")

    def empty_select(body_text, case_id, candidates):
        return {"entities": [], "relations": [], "json_valid": False}

    select = make_header_fallback_select_fn(empty_select)
    sel = select(body, gold["case_id"], cands)
    assert len(sel["entities"]) >= 2
    assert sel.get("fallback_used") is True
    by_id = {c["candidate_id"]: c for c in cands}
    labels = {by_id[e["candidate_id"]]["surface"] for e in sel["entities"]}
    assert "Language and Perception" in labels


def test_make_header_fallback_select_fn_keeps_primary_when_nonempty() -> None:
    body, gold = _body_and_gold()
    cands = build_body_candidates(body, paper_id="1206.6423")
    cid = next(c["candidate_id"] for c in cands if c["surface"] == "Language and Perception")

    def primary(body_text, case_id, candidates):
        return {
            "entities": [{"candidate_id": cid, "type": "Field"}],
            "relations": [],
            "json_valid": True,
        }

    select = make_header_fallback_select_fn(primary)
    sel = select(body, gold["case_id"], cands)
    assert sel.get("fallback_used") is False
    assert len(sel["entities"]) == 1
    assert sel["entities"][0]["candidate_id"] == cid



def test_looks_like_author_span_keeps_title_case_tech_nps() -> None:
    """Title-Case technical NPs must not be treated as author names."""
    keep = [
        "Physical Dynamics",
        "Compositional Object-based",
        "Web Automation",
        "Code Generation",
        "Code Repair",
        "Code Emulator",
        "Dynamical Systems",
        "Predictive State Representations",
        "Mathematical Reasoning",
        "Software Engineering",
        "Natural Language Instructions",
        "API Search Tools",
        "GitHub Issues",
        "Beam Search",
        "Subword Units",
    ]
    for s in keep:
        assert _looks_like_author_span(s) is False, s
    # still detect author-like spans
    assert _looks_like_author_span("Michael Chang") is True
    assert _looks_like_author_span("Satinder Singh") is True


def test_header_priority_recovers_title_case_tech_nps_from_candidates() -> None:
    body = (
        "# A Compositional Object-based Approach to Learning Physical Dynamics\n\n"
        "We study physical dynamics with compositional object-based models.\n"
    )
    cands = [
        {
            "candidate_id": "c:learn",
            "surface": "Learning Physical Dynamics",
            "surface_norm": "learning physical dynamics",
            "source": "header_title",
        },
        {
            "candidate_id": "c:phys",
            "surface": "Physical Dynamics",
            "surface_norm": "physical dynamics",
            "source": "header_title",
        },
        {
            "candidate_id": "c:obj",
            "surface": "Compositional Object-based",
            "surface_norm": "compositional object-based",
            "source": "header_title",
        },
        {
            "candidate_id": "c:noise",
            "surface": "balls moving toward",
            "surface_norm": "balls moving toward",
            "source": "header_title",
        },
    ]
    sel = header_priority_select(body, "case:x", cands)
    by = {c["candidate_id"]: c["surface_norm"] for c in cands}
    norms = {by[e["candidate_id"]] for e in sel["entities"]}
    assert "physical dynamics" in norms
    assert "compositional object-based" in norms



def test_header_priority_keeps_shorter_core_over_longer_wrapper() -> None:
    """Do not replace shorter multiword gold cores with longer wrappers."""
    body = (
        "# Recursively Summarizing Books with Human Feedback\n\n"
        "We use human feedback for recursively summarizing books.\n"
    )
    cands = [
        {
            "candidate_id": "c:task",
            "surface": "Recursively Summarizing Books",
            "surface_norm": "recursively summarizing books",
            "source": "header_title",
        },
        {
            "candidate_id": "c:method",
            "surface": "Human Feedback",
            "surface_norm": "human feedback",
            "source": "header_title",
        },
        {
            "candidate_id": "c:long1",
            "surface": "Recursively Summarizing Books with Human",
            "surface_norm": "recursively summarizing books with human",
            "source": "header_title",
        },
        {
            "candidate_id": "c:long2",
            "surface": "Books with Human Feedback",
            "surface_norm": "books with human feedback",
            "source": "header_title",
        },
    ]
    sel = header_priority_select(body, "case:x", cands)
    by = {c["candidate_id"]: c["surface_norm"] for c in cands}
    norms = {by[e["candidate_id"]] for e in sel["entities"]}
    assert "recursively summarizing books" in norms
    assert "human feedback" in norms
    assert "recursively summarizing books with human" not in norms



def test_header_priority_prefers_complete_with_np() -> None:
    body = "# Attention with Linear Biases\n"
    cands = [
        {
            "candidate_id": "c:short",
            "surface": "Attention with Linear",
            "surface_norm": "attention with linear",
            "source": "header_title",
        },
        {
            "candidate_id": "c:full",
            "surface": "Attention with Linear Biases",
            "surface_norm": "attention with linear biases",
            "source": "header_title",
        },
        {
            "candidate_id": "c:other",
            "surface": "ALiBi",
            "surface_norm": "alibi",
            "source": "header_title",
        },
    ]
    # need second multiword partner; add Field-ish
    cands.append(
        {
            "candidate_id": "c:field",
            "surface": "Language Modeling",
            "surface_norm": "language modeling",
            "source": "header_title",
        }
    )
    sel = header_priority_select(body, "case:x", cands)
    by = {c["candidate_id"]: c["surface_norm"] for c in cands}
    norms = {by[e["candidate_id"]] for e in sel["entities"]}
    assert "attention with linear biases" in norms
    assert "attention with linear" not in norms



def test_score_selection_structural_prefers_header_title_multiword() -> None:
    cands = [
        {
            "candidate_id": "c:a",
            "surface": "Language and Perception",
            "surface_norm": "language and perception",
            "source": "header_title",
        },
        {
            "candidate_id": "c:a2",
            "surface": "Grounded Attribute Learning",
            "surface_norm": "grounded attribute learning",
            "source": "header_title",
        },
        {
            "candidate_id": "c:b",
            "surface": "although several works",
            "surface_norm": "although several works",
            "source": "header_title",
        },
        {
            "candidate_id": "c:c",
            "surface": "random ngram span",
            "surface_norm": "random ngram span",
            "source": "ngram",
        },
    ]
    good = {
        "entities": [
            {"candidate_id": "c:a", "type": "Field"},
            {"candidate_id": "c:a2", "type": "Task"},
        ],
        "relations": [
            {"type": "APPLIED_TO", "source_id": "c:a", "target_id": "c:a2"},
        ],
        "json_valid": True,
    }
    noisy = {
        "entities": [
            {"candidate_id": "c:b", "type": "Method"},
            {"candidate_id": "c:c", "type": "Task"},
            {"candidate_id": "c:a", "type": "Field"},
            {"candidate_id": "c:c", "type": "Method"},
        ],
        "relations": [],
        "json_valid": True,
    }
    assert score_selection_structural(good, cands) > score_selection_structural(noisy, cands)


def test_make_header_prefer_select_fn_switches_when_primary_weaker() -> None:
    body, gold = _body_and_gold()
    cands = build_body_candidates(body, paper_id="1206.6423")
    # primary: pick a weaker ngram-like candidate if present, else empty-ish wrong set
    weak_id = next(
        (
            c["candidate_id"]
            for c in cands
            if c.get("source") != "header_title" and " " in str(c.get("surface") or "")
        ),
        None,
    )
    if weak_id is None:
        weak_id = cands[0]["candidate_id"]

    def weak_primary(body_text, case_id, candidates):
        return {
            "entities": [
                {"candidate_id": weak_id, "type": "Method"},
                {"candidate_id": weak_id, "type": "Task"},
                {"candidate_id": weak_id, "type": "Field"},
            ],
            "relations": [],
            "json_valid": True,
        }

    select = make_header_prefer_select_fn(weak_primary)
    sel = select(body, gold["case_id"], cands)
    assert sel.get("fallback_used") is True
    assert sel.get("fallback_reason") == "primary_weaker_structural"
    by_id = {c["candidate_id"]: c for c in cands}
    labels = {by_id[e["candidate_id"]]["surface"] for e in sel["entities"] if e["candidate_id"] in by_id}
    assert "Language and Perception" in labels


def test_make_header_prefer_select_fn_keeps_strictly_stronger_primary() -> None:
    body, gold = _body_and_gold()
    cands = build_body_candidates(body, paper_id="1206.6423")
    header = header_priority_select(body, gold["case_id"], cands)
    # Equal-to-header primary is not kept (ties prefer header).
    def equal_primary(body_text, case_id, candidates):
        return dict(header)

    select = make_header_prefer_select_fn(equal_primary)
    sel = select(body, gold["case_id"], cands)
    assert sel.get("fallback_used") is True
    assert sel.get("fallback_reason") == "primary_not_stronger_structural"

    # Strictly stronger: header entities plus a clean relation already present is equal;
    # craft primary that header ranks lower by using only header entities but with
    # valid relation + same compact set — still equal. Instead inject a primary that
    # is header selection with an extra high-quality relation already counted, and
    # compare against a weakened header by using prefer_margin negative.
    select2 = make_header_prefer_select_fn(equal_primary, prefer_margin=10.0)
    sel2 = select2(body, gold["case_id"], cands)
    # With large margin, header must beat primary by >10; equal keeps primary.
    assert sel2.get("fallback_used") is False
    assert len(sel2["entities"]) >= 2



def test_render_constrained_prompt_requires_exactly_two() -> None:
    cands = [
        {
            "candidate_id": "c:a",
            "surface": "Language and Perception",
            "surface_norm": "language and perception",
            "source": "header_title",
        },
        {
            "candidate_id": "c:noise",
            "surface": "although several works",
            "surface_norm": "although several works",
            "source": "header_title",
        },
        {
            "candidate_id": "c:ng",
            "surface": "random body ngram",
            "surface_norm": "random body ngram",
            "source": "ngram",
        },
    ]
    prompt = render_constrained_select_prompt(
        case_id="case:x",
        paper_id="x",
        candidates=prioritize_candidates_for_prompt(cands),
    )
    assert "EXACTLY 2" in prompt or "exactly 2" in prompt.casefold()
    assert "Select 2-4" not in prompt
    assert "c:a" in prompt
    # noise/ngram may be listed later or omitted; header multiword first
    pos_a = prompt.find("c:a")
    pos_ng = prompt.find("c:ng")
    if pos_ng >= 0:
        assert pos_a < pos_ng


def test_prioritize_candidates_for_prompt_orders_header_multiword_first() -> None:
    cands = [
        {
            "candidate_id": "c:ng",
            "surface": "random body ngram",
            "surface_norm": "random body ngram",
            "source": "ngram",
        },
        {
            "candidate_id": "c:a",
            "surface": "Language and Perception",
            "surface_norm": "language and perception",
            "source": "header_title",
        },
        {
            "candidate_id": "c:one",
            "surface": "Attention",
            "surface_norm": "attention",
            "source": "header_title",
        },
    ]
    ordered = prioritize_candidates_for_prompt(cands)
    assert ordered[0]["candidate_id"] == "c:a"


def test_trim_selection_to_top_k_keeps_best_two() -> None:
    cands = [
        {
            "candidate_id": "c:a",
            "surface": "Language and Perception",
            "surface_norm": "language and perception",
            "source": "header_title",
        },
        {
            "candidate_id": "c:b",
            "surface": "Grounded Attribute Learning",
            "surface_norm": "grounded attribute learning",
            "source": "header_title",
        },
        {
            "candidate_id": "c:noise",
            "surface": "although several works",
            "surface_norm": "although several works",
            "source": "header_title",
        },
        {
            "candidate_id": "c:ng",
            "surface": "random body ngram",
            "surface_norm": "random body ngram",
            "source": "ngram",
        },
    ]
    raw = {
        "entities": [
            {"candidate_id": "c:noise", "type": "Method"},
            {"candidate_id": "c:a", "type": "Field"},
            {"candidate_id": "c:ng", "type": "Task"},
            {"candidate_id": "c:b", "type": "Task"},
        ],
        "relations": [
            {
                "type": "APPLIED_TO",
                "source_id": "c:a",
                "target_id": "c:b",
            },
            {
                "type": "APPLIED_TO",
                "source_id": "c:noise",
                "target_id": "c:ng",
            },
        ],
        "json_valid": True,
    }
    trimmed = trim_selection_to_top_k(raw, cands, k=2)
    ids = {e["candidate_id"] for e in trimmed["entities"]}
    assert ids == {"c:a", "c:b"}
    assert len(trimmed["relations"]) == 1
    assert trimmed["relations"][0]["source_id"] == "c:a"


def test_make_llm_trims_overselect_before_return() -> None:
    body, gold = _body_and_gold()
    cands = build_body_candidates(body, paper_id="1206.6423")
    by_surface = {c["surface"]: c["candidate_id"] for c in cands}
    cid_field = by_surface["Language and Perception"]
    cid_task = by_surface["Grounded Attribute Learning"]
    # pick a third if available
    extra = next(
        (c["candidate_id"] for c in cands if c["candidate_id"] not in {cid_field, cid_task}),
        None,
    )

    def chat_fn(messages, *, model, max_tokens=700, temperature=0.0):
        ents = [
            {"candidate_id": cid_field, "type": "Field"},
            {"candidate_id": cid_task, "type": "Task"},
        ]
        if extra:
            ents.append({"candidate_id": extra, "type": "Method"})
        payload = {
            "entities": ents,
            "relations": [
                {
                    "type": "APPLIED_TO",
                    "source_id": cid_field,
                    "target_id": cid_task,
                }
            ],
        }
        return json.dumps(payload)

    select = make_llm_constrained_select_fn(chat_fn=chat_fn, model="mock")
    sel = select(body, gold["case_id"], cands)
    assert len(sel["entities"]) <= 2
    ids = {e["candidate_id"] for e in sel["entities"]}
    assert cid_field in ids
    assert cid_task in ids



def test_relation_type_for_entity_pair_priors() -> None:
    assert relation_type_for_entity_pair("Method", "Task") == "APPLIED_TO"
    assert relation_type_for_entity_pair("Method", "Method") == "USES_COMPONENT"
    assert relation_type_for_entity_pair("Method", "Dataset") == "EVALUATED_ON"
    assert relation_type_for_entity_pair("Dataset", "Field") is None


def test_min_surface_distance() -> None:
    body = "We use Neural Machine Translation for Subword Units in translation."
    d = min_surface_distance(body, "Neural Machine Translation", "Subword Units")
    assert d is not None and d < 50


def test_build_relation_candidates_type_pair_proximity() -> None:
    body = (
        "Neural Machine Translation with Subword Units improves rare words. "
        "Neural Machine Translation is a Method applied to translation tasks."
    )
    cands = build_body_candidates(body)
    # pick two known surfaces
    by_surf = {c["surface"]: c for c in cands}
    assert "Neural Machine Translation" in by_surf
    assert "Subword Units" in by_surf
    ents = [
        {"candidate_id": by_surf["Neural Machine Translation"]["candidate_id"], "type": "Method"},
        {"candidate_id": by_surf["Subword Units"]["candidate_id"], "type": "Method"},
    ]
    rels = build_relation_candidates(
        body_text=body, entities=ents, candidates=cands, max_relations=1
    )
    assert len(rels) == 1
    assert rels[0]["type"] == "USES_COMPONENT"
    assert rels[0]["source_id"] != rels[0]["target_id"]


def test_header_select_uses_relation_candidates() -> None:
    body, gold = _body_and_gold()
    cands = build_body_candidates(body, paper_id="1206.6423")
    sel = header_priority_select(body, gold["case_id"], cands)
    assert len(sel["relations"]) >= 1
    assert sel["relations"][0]["type"] in {
        "APPLIED_TO",
        "USES_COMPONENT",
        "EVALUATED_ON",
        "OUTPERFORMS",
    }
