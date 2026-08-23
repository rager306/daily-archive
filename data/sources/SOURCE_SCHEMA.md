# Source Schema — единая схема источника (meta-prompt принцип)

Все источники описываются через **общую схему** + **манифест конкретного источника**.
Код не содержит хардкодов — всё в YAML/DATA.

---

## 1. Общая схема источника

Каждый источник имеет **три обязательных компонента**:

```
sources/<source_type>/<source_id>/
├── source_manifest.json    ← ОБЯЗАТЕЛЬНО: манифест источника
├── identity.json           ← метаданные статьи/главы
└── source/                 ← физические файлы (PDF, HTML, …)
```

### 1.1 source_manifest.json — паспорт ИСТОЧНИКА

Описывает сам источник: что это, какой формат, как парсить.

```json
{
  "schema_version": "source-manifest.v1",
  "source_id": "arxiv",
  "title": "Human-readable name",
  "description": "Что это за источник",
  "reliability_tier": 2,
  
  "locator": {
    "type": "arxiv_id | pmid | doi | url | local_path",
    "pattern": "<regex для валидации локатора>",
    "examples": ["2507.19457", "10.48550/arxiv.2507.19457"]
  },
  
  "acquisition": {
    "method": "api_fetch | direct_download | catalog_hit | local_drop | manual",
    "base_url": "https://...",
    "auth": "none | api_key | oauth",
    "rate_limit_rps": 1.0,
    "retry_policy": {"max_retries": 3, "backoff_seconds": 30}
  },
  
  "assets_policy": {
    "primary_format": "pdf | html | jats_xml | markdown",
    "sidecar_formats": ["html", "latex"],
    "requires_fulltext_license": false,
    "versioning_model": "v_suffix | none | doi_version"
  },
  
  "parser_profile": "grobid_structural | html_navigation | pdf_native | jats_xml",
  
  "identity_mapping": {
    "paper_key_field": "arxiv_id",
    "category_field": "coarse_topic_code",
    "title_field": "title",
    "authors_field": "authors"
  }
}
```

### 1.2 identity.json — паспорт СТАТЬИ

Метаданные конкретной статьи из этого источника.

```json
{
  "schema_version": "article-identity.v2",
  "paper_key": "2507.19457",
  "source_code": "arxiv",
  "source_manifest_ref": "../source_manifest.json",
  "primary_category": "cs-cl",
  "title": "...",
  "authors": [...],
  "version": "v2",
  "sha256": "abc...",
  "acquired_at": "2026-08-20T..."
}
```

### 1.3 Стадийный состав

| Стадия | Вход | Выход | Где |
|---|---|---|---|
| acquire | locator → assets | source/*.pdf/html | sources/<id>/source/ |
| identity | assets → metadata | identity.json | sources/<id>/ |
| parse | PDF/HTML → ParsedArticle | processed/<id>/parsed/article.json | processed/ |
| context | ParsedArticle → AST | processed/<id>/context/projection.json | processed/ |
| extract | ParsedArticle → entities | processed/<id>/extracted/entities.json | processed/ |
| enrich | paper_id → OpenAlex/Crossref | processed/<id>/enriched/*.json | processed/ |
| materialize | parsed+extracted+context → graph nodes | knowledge/graphs/*.sgsnap | knowledge/ |
| verify | replay → ACAP evidence | verified: PASS/FAIL | manifest |

## 2. Реестр источников

### 2.1 Зарегистрированные (есть в source_codes.yaml)

| Код | Название | Tier | Статус реализации | Манифест |
|---|---|---|---|---|
| `arxiv` | arXiv preprint server | 2 | ✅ полный (801 статья) | неявный (в коде) → нужен явный JSON |
| `gnn_textbook` | GNN Textbook HTML chapters | 1 | ✅ полный (28 глав) | ✅ textbook_manifest.json |
| `textbook` | Textbook (curated) | 1 | ✅ (Trench Real Analysis) | ✅ textbook_manifest.json |
| `stanford` | Stanford course materials | 2 | ⚠️ частично (1 курс) | ❌ нет |
| `company_blog` | Company engineering blog | 3 | ⚠️ частично (1 пост) | ❌ нет |
| `nature` | Nature family (curated) | 2 | ⚠️ частично (1 статья) | ❌ нет |
| `openalex` | OpenAlex metadata | 1 | ⚠️ enrichment only | ❌ нет |
| `crossref` | Crossref DOI metadata | 1 | ❌ не реализован | ❌ нет |
| `semantic_scholar` | Semantic Scholar | 2 | ❌ не реализован | ❌ нет |
| `local` | Operator local drop | 3 | ❌ не реализован | ❌ нет |

### 2.2 Планируемые по ADR-056 (нет в source_codes.yaml)

| Семейство | Предлагаемые коды | Формат | Приоритет |
|---|---|---|---|
| Biomedical | `pubmed`, `europe_pmc` | HTML/JATS или PDF | HIGH — extension_domains da.medicine |
| DOI-first | `doi` | metadata only | HIGH — закрывает хардкод |
| RU academic | `elibrary`, `cyberleninka` | PDF/HTML policy-gated | LOW |
| CN academic | `cnki`, `wanfang` | policy-gated | LOW |

## 3. Требования к каждому источнику

Для добавления нового источника необходимо:

1. **source_codes.yaml**: добавить `{code, name, reliability_tier}`
2. **source_manifest.json**: заполнить по схеме §1.1
   - locator pattern
   - acquisition method
   - assets policy
   - parser profile
   - identity mapping
3. **identity.json**: для каждой загруженной статьи
4. **SourceAcquisitionPort adapter**: реализовать интерфейс
5. **Тесты**: contract suite (ADR-051 port contour)

## 4. Governor контроль

```yaml
# data_integrity.yaml additions:
rules:
  - id: source_manifest_required
    description: "Every sources/<type>/<id>/ must have source_manifest.json or inherit from parent"
    
  - id: source_identity_schema_valid
    description: "identity.json must conform to article-identity.v2 schema"
    
  - id: no_orphan_sources
    description: "Every dir in sources/ with identity.json must be in index.json"
```

## 5. Текущие пробелы (action items)

| # | Что делать | Для какого источника | Блокирует |
|---|---|---|---|
| 1 | Создать явные source_manifest.json для arxiv | arxiv | W-D |
| 2 | Создать source_manifest.json для stanford/blog/nature | curated sources | — |
| 3 | Зарегистрировать pubmed/europe_pmc в source_codes.yaml | biomedical | W-D |
| 4 | Зарегистрировать doi в source_codes.yaml | DOI-first | W-B/W-D |
| 5 | Реализовать SourceAcquisitionPort для каждого нового | все новые | W-D |
| 6 | Добавить Governor правила (см. §4) | все | сразу |

## 6. Связь с extension_domains.yaml

extension_domains.yaml определяет НАУЧНЫЕ ДОМЕНЫ (не источники!).
Домен = тематическая классификация (medicine, genetics…).
Источник = откуда пришёл документ (pubmed, arxiv…).

Связь: identity.json.primary_category может быть как arXiv кодом,
так и da.* доменом. Источник НЕ определяет домен — домен определяется
содержимым статьи.


---

## 7. Реализованные манифесты (текущий статус)

| Источник | Манифест | Путь |
|---|---|---|
| arxiv | ✅ source_manifest.json | data/sources/arxiv/source_manifest.json |
| gnn_textbook | ✅ textbook_manifest.json | data/sources/textbook/gnn-textbook/textbook_manifest.json |
| textbook | ✅ textbook_manifest.json | data/sources/textbook/trench-real-analysis/textbook_manifest.json |
| stanford | ✅ source_manifest.json | data/sources/course/source_manifest.json |
| company_blog | ✅ source_manifest.json | data/sources/blog/source_manifest.json |
| nature | ✅ source_manifest.json | data/sources/nature/source_manifest.json |
| semantic_scholar | ❌ не реализован | — |
| crossref | ❌ не реализован | — |
| pubmed/europe_pmc | ❌ не реализован, не зарегистрирован | — |
| local | ❌ не реализован, не зарегистрирован | — |
