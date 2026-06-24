from __future__ import annotations

from pathlib import Path

from scripts import repair_m042_linked_metadata as repair


def _write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    repair.write_json(path, payload)


def _manifest(tmp_path: Path, *, article_key: str = "2401.00001") -> Path:
    manifest_path = tmp_path / "artifacts" / "m041" / "manifest.json"
    _write_json(
        manifest_path,
        {
            "article_count": 20,
            "articles": [
                {
                    "article_key": article_key,
                    "article_ref": f"artifact:data/article_catalog/article_catalog/arxiv/cs-ai/{article_key}/article.json",
                    "catalog_path": f"arxiv/cs-ai/{article_key}",
                    "linked_from": [
                        "data/article_catalog/article_catalog/arxiv/cs-ai/source/source/article.html"
                    ],
                    "m041_category": "reference_linked",
                }
            ],
            "safety_flags": {
                "graph_write_allowed": False,
                "import_eligible": False,
                "production_import_attempted": False,
                "promotion_allowed": False,
            },
        },
    )
    return manifest_path


def _article(tmp_path: Path, article_key: str, *, fetched: bool) -> Path:
    article_path = (
        tmp_path
        / "data"
        / "article_catalog"
        / "article_catalog"
        / "arxiv"
        / "cs-ai"
        / article_key
        / "article.json"
    )
    identity = {"arxiv_id": article_key}
    if fetched:
        identity.update(
            {
                "canonical_url": f"https://arxiv.org/abs/{article_key}",
                "metadata_status": "fetched",
                "pdf_url": f"https://arxiv.org/pdf/{article_key}",
                "published": "2024-01-01T00:00:00Z",
                "title": "Already fetched title",
                "updated": "2024-01-01T00:00:00Z",
            }
        )
    _write_json(
        article_path,
        {
            "article_key": article_key,
            "connectivity_smoke": {
                "category": "reference_linked",
                "linked_from": [
                    "data/article_catalog/article_catalog/arxiv/cs-ai/source/source/article.html"
                ],
                "metadata_only": True,
                "metadata_status": "fetched" if fetched else "deferred",
            },
            "identity": identity,
            "safety_flags": {
                "trusted_kg_imported": False,
                "raw_text_embedded_in_metadata": False,
            },
        },
    )
    return article_path


def _atom(article_key: str) -> bytes:
    return f"""
    <feed xmlns="http://www.w3.org/2005/Atom" xmlns:arxiv="http://arxiv.org/schemas/atom">
      <entry>
        <id>http://arxiv.org/abs/{article_key}</id>
        <updated>2024-02-02T00:00:00Z</updated>
        <published>2024-02-01T00:00:00Z</published>
        <title> Repaired Metadata Title </title>
        <summary> Repaired summary. </summary>
        <arxiv:primary_category term="cs.AI" />
        <link href="https://arxiv.org/abs/{article_key}" rel="alternate" type="text/html" />
        <link href="https://arxiv.org/pdf/{article_key}" title="pdf" />
      </entry>
    </feed>
    """.encode()


def test_repair_linked_metadata_updates_deferred_record(tmp_path, monkeypatch):
    article_key = "2401.00001"
    manifest_path = _manifest(tmp_path, article_key=article_key)
    article_path = _article(tmp_path, article_key, fetched=False)
    monkeypatch.setattr(repair, "ROOT", tmp_path)

    report = repair.repair_linked_metadata(
        manifest_path=manifest_path,
        output_dir=tmp_path / "out",
        fetcher=lambda _url: _atom(article_key),
    )

    updated = repair.load_json(article_path)
    assert updated["identity"]["metadata_status"] == "fetched"
    assert updated["identity"]["title"] == "Repaired Metadata Title"
    assert updated["connectivity_smoke"]["linked_from"]
    assert report["status_counts"] == {"fetched": 1}
    assert report["records"][0]["action"] == "repaired"
    assert report["graph_write_allowed"] is False
    assert (tmp_path / "out" / "repair-report.json").exists()
    assert (tmp_path / "out" / "repair-report.md").exists()


def test_repair_linked_metadata_defers_when_network_disabled(tmp_path, monkeypatch):
    article_key = "2401.00002"
    manifest_path = _manifest(tmp_path, article_key=article_key)
    article_path = _article(tmp_path, article_key, fetched=False)
    monkeypatch.setattr(repair, "ROOT", tmp_path)

    report = repair.repair_linked_metadata(
        manifest_path=manifest_path,
        output_dir=tmp_path / "out",
        no_network=True,
        fetcher=lambda _url: (_ for _ in ()).throw(AssertionError("network must not be called")),
    )

    unchanged = repair.load_json(article_path)
    assert unchanged["identity"].get("metadata_status") is None
    assert report["status_counts"] == {"deferred": 1}
    assert report["records"][0]["action"] == "deferred"
    assert report["records"][0]["deferred_reason"] == "network_disabled"


def test_repair_linked_metadata_skips_already_fetched_without_network(tmp_path, monkeypatch):
    article_key = "2401.00003"
    manifest_path = _manifest(tmp_path, article_key=article_key)
    _article(tmp_path, article_key, fetched=True)
    monkeypatch.setattr(repair, "ROOT", tmp_path)

    report = repair.repair_linked_metadata(
        manifest_path=manifest_path,
        output_dir=tmp_path / "out",
        fetcher=lambda _url: (_ for _ in ()).throw(AssertionError("network must not be called")),
    )

    assert report["status_counts"] == {"fetched": 1}
    assert report["records"][0]["action"] == "already_fetched"


def test_repair_linked_metadata_rejects_enabled_safety_flag(tmp_path, monkeypatch):
    article_key = "2401.00004"
    manifest_path = _manifest(tmp_path, article_key=article_key)
    manifest = repair.load_json(manifest_path)
    manifest["safety_flags"]["graph_write_allowed"] = True
    _write_json(manifest_path, manifest)
    _article(tmp_path, article_key, fetched=True)
    monkeypatch.setattr(repair, "ROOT", tmp_path)

    try:
        repair.repair_linked_metadata(manifest_path=manifest_path, output_dir=tmp_path / "out")
    except ValueError as exc:
        assert "safety flags" in str(exc)
    else:  # pragma: no cover - defensive failure branch
        raise AssertionError("expected ValueError")
