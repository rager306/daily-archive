"""Unit tests for structured 9router extract path (mocked chat, no network)."""

from __future__ import annotations

from dataclasses import dataclass

from research_graph.infrastructure.llm.ninerouter_json_extract import (
    NineRouterJsonExtractClient,
)


@dataclass
class _FakeChatResult:
    ok: bool
    text: str
    error: str | None = None


class _FakeChat:
    def __init__(self, responses: list[str]) -> None:
        self.responses = list(responses)
        self.calls = 0
        self.messages_log: list[list[dict]] = []

    def chat(self, **kwargs):  # noqa: ANN003
        self.messages_log.append(list(kwargs.get("messages") or []))
        idx = min(self.calls, len(self.responses) - 1)
        text = self.responses[idx]
        self.calls += 1
        return _FakeChatResult(ok=True, text=text)


def test_structured_mode_uses_structured_prompt_and_followup() -> None:
    body = (
        "# Neural Machine Translation BY JOINTLY LEARNING TO ALIGN AND TRANSLATE\n\n"
        "## Abstract\n"
        "Neural Machine Translation with Align and Translate attention.\n\n"
        "## Method\n"
        "We jointly learn to Align and Translate.\n\n"
        "## Experiments\n"
        "BLEU results on WMT.\n"
    )
    fake = _FakeChat(
        [
            # first: request more section (title-ish id; resolver is fuzzy)
            '{"entities":[],"relations":[],"need_sections":["Method"]}',
            # second: final
            (
                '{"entities":[{"type":"Method","label":"Neural Machine Translation"},'
                '{"type":"Task","label":"Align and Translate"}],'
                '"relations":[{"type":"APPLIED_TO","source_label":"Neural Machine Translation",'
                '"target_label":"Align and Translate"}],'
                '"need_sections":[]}'
            ),
        ]
    )
    client = NineRouterJsonExtractClient(
        chat_client=fake,  # type: ignore[arg-type]
        model="test/model",
        use_structured_context=True,
        max_followup_rounds=1,
    )
    out = client.extract_case(body, "case:train:1409.0473", paper_id="1409.0473")
    assert out["json_valid"] is True
    assert len(out["entities"]) == 2
    assert client.last_diagnostics["mode"] == "structured_context"
    assert client.last_diagnostics["followup_rounds"] >= 1
    # first user prompt must be structured
    user0 = fake.messages_log[0][1]["content"]
    assert "--- OUTLINE ---" in user0
    assert "--- GROUNDED CANDIDATES ---" in user0
    assert "--- PAPER TEXT ---" not in user0


def test_raw_mode_still_available() -> None:
    fake = _FakeChat(
        [
            '{"entities":[{"type":"Method","label":"Foo Bar"}],"relations":[]}',
        ]
    )
    client = NineRouterJsonExtractClient(
        chat_client=fake,  # type: ignore[arg-type]
        use_structured_context=False,
    )
    out = client.extract_case("Foo Bar paper body", "case:x")
    assert out["json_valid"] is True
    assert client.last_diagnostics["mode"] == "raw_body"
    assert "--- PAPER TEXT ---" in fake.messages_log[0][1]["content"]
