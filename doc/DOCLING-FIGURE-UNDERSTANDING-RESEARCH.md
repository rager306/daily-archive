# Docling: глубокое исследование figure understanding (code-level)

**Дата:** 2026-08-13
**Источники:** официальная документация docling-project.github.io (v1.21.0 docling-serve), HuggingFace model cards, сторонние бенчмарки. См. раздел «Источники».
**Связанные артефакты:** ADR-060 (Scientific Figure Understanding Pipeline), D237 (no Python in product).

> ⚠️ **Главное исправление к ADR-060:** `PictureItem.meta.tabular_chart` и
> chart-to-table extraction в Docling **не существуют**. Это roadmap/миф, попавший
> в исследовательские заметки. Docling не делает chart→numeric table. См. §4.

---

## Сводка зрелости (maturity table)

| Возможность | Зрелость | Что реально есть |
|---|---|---|
| Извлечение figures (`PictureItem`, `get_image`, bbox) | ✅ **Production** | layout-детекция RT-DETR (Heron/Egret), `generate_picture_images`, `images_scale` |
| `PictureItem.meta` (caption, provenance) | ✅ **Production** | `prov` с bbox, `caption_text`, `meta.description`/`classification` |
| DocumentFigureClassifier (26 классов) | ✅ **Production** | EfficientNet-B0, v2.5, Acc 90.7%, ONNX+CPU |
| Picture description (VLM captioning) | ✅ **Production** | SmolVLM-256M (default), Granite-Vision, Pixtral, Qwen2.5-VL; local + remote API |
| docling-serve HTTP API | ✅ **Production** | FastAPI, `/v1/convert/{source,file}`, async+WebSocket, Docker, v1.21.0 |
| Table extraction (TableFormer) | ✅ **Production** | cell-level, fast/accurate; превосходит GROBID в бенчмарках |
| Granite-Docling-258M (full-page VLM, DocTags) | ✅ **Production-ready** | 258M, CPU-runnable, Apache-2.0, single-shot layout+OCR+table+formula |
| **Chart-to-table (tabular_chart)** | ❌ **НЕ существует** | нет в model catalog; классификация типа chart есть, извлечения данных — нет |
| Compound figure / subpanel decomposition | ❌ **НЕ существует** | не задокументировано; subpanels нужно делать самим (crop+recurse) |

---

## 1. Figure extraction — Python API (production)

### Точные опции pipeline

```python
from docling.datamodel.pipeline_options import PdfPipelineOptions

pipeline_options = PdfPipelineOptions()
pipeline_options.images_scale = 2.0                       # scale=1 ≈ 72 DPI; 2.0 ≈ 144 DPI
pipeline_options.generate_page_images = True              # рендер каждой страницы → page.image
pipeline_options.generate_picture_images = True           # рендер каждого PictureItem → picture.image
pipeline_options.generate_table_images = True             # (есть и для таблиц)
```

- `images_scale` единый множитель разрешения для ВСЕХ генерируемых картинок
  (page/picture/table). `scale=2.0` рекомендуется для подачи в VLM.
- `generate_*` — тумблеры «какие элементы снабдить картинкой».

### Структура `PictureItem` (docling_core.types.doc)

Из рабочего примера `export_figures.py`:

```python
from docling_core.types.doc import PictureItem, TableItem, ImageRefMode

for element, _level in conv_res.document.iterate_items():
    if isinstance(element, PictureItem):
        img = element.get_image(conv_res.document)   # PIL.Image — рендер bbox фигуры
        img.save(path, "PNG")

# Доступные поля/методы PictureItem (наблюдается в examples):
pic.self_ref            # "#/pictures/0"  — стабильная ссылка в DoclingDocument
pic.image               # ImageRefMeta: .uri, .pil_image, .mimetype, .dpi
pic.image.pil_image     # сам PIL.Image (после generate_picture_images)
pic.prov                # ProvenanceItem[]: bbox + page_no + page_size (координаты PDF-точки)
pic.caption_text(doc)   # связанный caption текст (Figure 1: ...)
pic.meta                # PictureItemData | None
pic.meta.description    # PictureDescriptionData: .text, .created_by, .usage(...)
pic.meta.classification # ClassificationData: .predicted_class, .confidence
```

**Координаты (`prov`):** bbox в PDF-точках (origin top-left или bottom-left зависит от
backend; Docling нормализует). `get_image()` рендерит по этому bbox с учётом
`images_scale`. Это **pixel-precise crop** — ключевое отличие от GROBID (см. §6).

### Как Docling извлекает фигуры vs GROBID

- **Docling:** layout-модель (RT-DETR, `docling-layout-heron`) детектирует `PICTURE`
  bbox на полной странице → рендер crop по bbox → `PictureItem`. Это
  **визуальная детекция**, работает даже когда figure не размечена в PDF-структуре.
- **GROBID:** разбор TEI-структуры; `teiCoordinates` даёт bbox для figure/table/note,
  но детекция опирается на cues текстовой разметки и менее устойчива к
  compound/embedded figures.
- **Compound figures:** ни Docling, ни GROBID не разбивают много-панельные фигуры
  на subpanels автоматически (см. §4). Subpanel decomposition — ответственность
  нашего pipeline (ADR-060 Stage 2d: crop+recurse).

---

## 2. DocumentFigureClassifier — РЕАЛЕН (production)

Это **реальный класс/модель**, не концепция. Модель:
[`docling-project/DocumentFigureClassifier-v2.5`](https://huggingface.co/docling-project/DocumentFigureClassifier-v2.5).

- **Архитектура:** Google **EfficientNet-B0**, fine-tuned. NOT RT-DETR, NOT DocLayNet.
  Image-classification head. ONNX + Safetensors.
- **Точность:** Accuracy **90.7%**, Weighted F1 **0.907**, Macro F1 **0.689** (held-out finepdfs).
- **Датасет:** 10× больший, чем v2.0 (huggingface/finepdfs subset).
- **Поддержка устройств:** CPU, CUDA, MPS, XPU (через Transformers/ViT).
- **Где вызывается:** enrichment-стадия `do_picture_classification` поверх уже
  извлечённых `PictureItem`. Заполняет `pic.meta.classification`.

### API

```python
pipeline_options = PdfPipelineOptions()
pipeline_options.generate_picture_images = True
pipeline_options.images_scale = 2
pipeline_options.do_picture_classification = True       # <-- включает DocumentFigureClassifier

doc = converter.convert(pdf).document
for pic in doc.pictures:
    print(pic.meta.classification.predicted_class,      # e.g. "bar_chart"
          pic.meta.classification.confidence)
```

CLI: `docling --enrich-picture-classes FILE`

### 26 классов (важно для per-type routing в ADR-060)

Категории chart: **line_chart, bar_chart, pie_chart, scatter_plot, box_plot**.
Прочие: flow_chart, photograph, logo, icon, signature, stamp, qr_code, bar_code,
chemistry_structure, engineering_drawing, geographical_map, topographical_map,
screenshot_from_computer, screenshot_from_manual, page_thumbnail, full_page_image,
table, music, calendar, crossword_puzzle, other.

> Per-label recall ключевых chart-типов v2.5: bar_chart 0.93, pie_chart 0.94,
> line_chart 0.92, scatter_plot 0.67 (слабее), box_plot 0.64 (слабее, мало данных).

---

## 3. Picture description (`do_picture_description`) — production

Enrichment-стадия captioning. Включается `do_picture_description = True`, модель
выбирается через `picture_description_options`.

### API signature

```python
from docling.datamodel.pipeline_options import (
    granite_picture_description,   # пресет: ibm-granite/granite-vision-3.1-2b-preview
    smolvlm_picture_description,   # пресет: HuggingFaceTB/SmolVLM-256M-Instruct
    PictureDescriptionVlmOptions,  # любой VLM с HF Hub (локально)
    PictureDescriptionApiOptions,  # удалённый OpenAI-compatible endpoint
)

pipeline_options.do_picture_description = True
pipeline_options.picture_description_options = granite_picture_description
pipeline_options.picture_description_options.prompt = (
    "Describe the image in three sentences. Be concise and accurate."
)
pipeline_options.images_scale = 2.0
pipeline_options.generate_picture_images = True
```

### VLM-бэкенды

| Класс | Что | Куда уходят данные |
|---|---|---|
| `granite_picture_description` (пресет) | Granite-Vision-3.1-2b-preview | **локально**, в процессе |
| `smolvlm_picture_description` (пресет) | SmolVLM-256M-Instruct | **локально** |
| `PictureDescriptionVlmOptions(repo_id=...)` | любой HF VLM | **локально** |
| `PictureDescriptionApiOptions(url=...)` | vLLM / Ollama / watsonx / Azure | **на удалённый сервис** (требует `enable_remote_services=True`) |

В model catalog для picture description перечислены: **SmolVLM-256M (default ⭐),
Granite-Vision-3.3-2B, Pixtral-12B, Qwen2.5-VL-3B**.

### Контекст: ТОЛЬКО изображение, не caption+surrounding

Подтверждено примером `pictures_description`: промт — «Describe the image in three
sentences». VLM получает **только crop картинки**. Caption и surrounding text в
контекст встроенными НЕ передаются — текст аннотации отдельный от caption_text.
> Это значит OmniScience-паттерн (figure+caption+citing) Docling НЕ делает сам —
> его реализует наш `FigureSemanticContext` (ADR-060). Docling даёт только
> image-only captioning как candidate.

### Выходной формат

**Свободный текст**, не structured JSON:

```python
pic.meta.description          # PictureDescriptionData
pic.meta.description.text     # "In this image we can see a pie chart..."
pic.meta.description.created_by  # "ibm-granite/granite-vision-3.1-2b-preview"
```

Структурированный вывод требует свой промт + парсинг. API-опция умеет сохранять
usage-метрики: `pic.meta.description.get_custom_part()["docling__usage"]`.

---

## 4. Chart understanding — КРИТИЧЕСКОЕ ИСПРАВЛЕНИЕ

> **`PictureItem.meta.tabular_chart` НЕ существует.** Chart-to-table extraction в
> Docling отсутствует. Model catalog не содержит ChartClassifier / DePlot /
> ChartLlama / TATR-для-chart. Поиск `do_chart_enrichment`, `chart_to_table`,
> `tabular_chart` по актуальным докам и коду — 0 результатов.

### Что Docling РЕАЛЬНО умеет с charts

1. **Классификация типа** chart (§2): bar/line/pie/scatter/box — через
   DocumentFigureClassifier. Это **label**, не данные.
2. **Описание VLM** (§3): свободный текст вида «bar chart with values CPU, Max, GPU».
   Это **не** structured {x_axis, legend, data_points}.

### Флаг `chart` в REST API docling-serve

В таблице conversion options REST API среди enrichment flags указан `chart`. На
уровне model catalog этому соответствует **picture classification** (детекция, что
элемент — chart) — НЕ извлечение данных. ADR-060 трактовал это как chart-to-table —
**ошибочно**.

### Вывод для ADR-060 Stage 2a (chart-to-table)

Chart→{axes, legend, key_data_points} Docling **не предоставит**. Эту стадию
нужно реализовывать нашим VLM-вызовом (Qwen3-VL) по schema `chart_extraction` из
ADR-060, либо подключать отдельную модель (DePlot/ChartLlama) как ещё один
sidecar/remote API. Docling здесь — только классификатор типа + опциональный
caption-candidate.

---

## 5. docling-serve как HTTP sidecar (production) — точно как GROBID

**Реален.** FastAPI-сервер, версия **v1.21.0**, официальный Docker-образ.
Полный аналог паттерна GROBID `/api/processDocument` —
[github.com/docling-project/docling-serve](https://github.com/docling-project/docling-serve).

### Endpoints (порт по умолчанию 5001)

| Endpoint | Method | Назначение |
|---|---|---|
| `/v1/convert/source` | POST | конвертация из URL/base64 (синхр.) |
| `/v1/convert/file` | POST | upload multipart (синхр.) |
| `/v1/convert/source/async`, `/v1/convert/file/async` | POST | асинхр. задача |
| `/v1/status/poll/{task_id}` | GET | опрос статуса |
| `/v1/status/ws/{task_id}` | WebSocket | push-обновления |
| `/v1/result/{task_id}` | GET | результат |
| `/docs` | GET | **живой OpenAPI/Swagger** (авторитетная схема) |
| `/ui` | GET | Gradio demo (опц., `DOCLING_SERVE_ENABLE_UI=1`) |

### Аутентификация

Если на сервере задан `DOCLING_SERVE_API_KEY` — каждый запрос несёт
`-H "X-Api-Key: <KEY>"`.

### Запуск (локально, БЕЗ внешних API)

```bash
pip install "docling-serve[ui]"
docling-serve run --enable-ui     # prod-стиль: 0.0.0.0, без reload
# docling-serve dev               # dev: 127.0.0.1, reload, UI

# Docker — CPU, one-liner (аналог нашего GROBID compose):
podman run -p 5001:5001 -e DOCLING_SERVE_ENABLE_UI=1 \
  quay.io/docling-project/docling-serve
```

Может работать **полностью локально** (модели скачиваются с HF, кэшируются в
`DOCLING_SERVE_ARTIFACTS_PATH`). Для air-gapped: предзаполнить директорию моделей
и указать `DOCLING_SERVE_ARTIFACTS_PATH`. Remote-services (внешние API) включаются
только флагом `DOCLING_SERVE_ENABLE_REMOTE_SERVICES=true` — по умолчанию OFF.

### Curl-пример (мультимодельный upload)

```bash
curl -X POST "http://localhost:5001/v1/convert/file" \
  -H "Content-Type: multipart/form-data" \
  -F "files=@2507.19457.pdf;type=application/pdf" \
  -F "to_formats=md" \
  -F "do_ocr=true" \
  -F "image_export_mode=embedded" \
  -F "table_mode=fast"
```

### Python-клиент (но нам не подходит — D237; нужен Rust HTTP)

```python
from docling.service_client import DoclingServiceClient
from docling.datamodel.service.options import ConvertDocumentsOptions

with DoclingServiceClient(url="http://localhost:5001") as client:
    result = client.convert(
        source="https://arxiv.org/pdf/2501.17887",
        options=ConvertDocumentsOptions(to_formats=["md"]),
    )
```

> **Для daily-archive:** клиент — НЕ Python (D237). Реализуем Rust-адаптер
> `DoclingHttpAdapter` (reqwest), как уже есть `GrobidParser` для GROBID.

### Ключевые env-vars

`UVICORN_HOST/PORT` (0.0.0.0/5001), `UVICORN_WORKERS`, `DOCLING_SERVE_API_KEY`,
`DOCLING_SERVE_ENG_KIND` (`local`|`rq` — in-process vs Redis+workers),
`DOCLING_SERVE_ARTIFACTS_PATH` (кэш моделей), `DOCLING_DEVICE` (cpu/cuda/mps),
`DOCLING_SERVE_MAX_NUM_PAGES` / `DOCLING_SERVE_MAX_FILE_SIZE`.

---

## 6. Docling vs GROBID — practical comparison (figures & tables)

| Аспект | GROBID | Docling |
|---|---|---|
| Подход к figure | TEI-структура + `teiCoordinates` | RT-DETR layout-детекция → pixel crop |
| Координаты | PDF points (TEI `coords`) | PDF points в `prov` + pixel crop в `get_image()` |
| Compound/subpanel | ❌ нет | ❌ нет |
| Caption grounding | TEI `<figure>`/`<figDesc>` связи; хорошо для классических статей | `caption_text(doc)` через DoclingDocument-граф; устойчивее к нестандартной разметке |
| Picture F1 | умеренный | ADR-060 ссылается на 0.63 — **НО для таблиц Docling СИЛЬНЕЕ** (см. ниже) |
| Скорость scholarly PDF | **быстрее** (discussion #622: GROBID стабильно быстрее Docling) | медленнее (layout-модель + опц. enrichment-модели) |
| References/metadata | **сильная сторона** (CROSSREF-консолидация, TEI-схема научных статей) | слабее; нет сферической модели научных ссылок |
| Table extraction | средне-хорошо | **лучше** (бенчмарк ниже) |

### Бенчмарк таблиц (Senellart et al. 2025, 3 датасета)

Ранжирование по content/table-discovery F1:
- **Table-BRGM:** Grobid ≼ LVLM ≼ PDFPlumber ≼ Camelot ≼ PyMuPDF ≼ **Docling**
- **PubTables, Table-arXiv:** PDFPlumber ≼ Camelot ≼ LVLM ≼ PyMuPDF ≼ Grobid ≼ **Docling**

→ **Docling превосходит GROBID на извлечении таблиц** (включая topology), но
**уступает в скорости** и в качестве extraction научной библиографии/метаданных.

### Что GROBID ловит, а Docling — нет (и наоборот)

- **GROBID лучше:** metadata/заголовок, ссылки/референсы (CROSSREF), affiliation,
  speed на больших scholarly PDF. Это наш текущий primary grounder — оставляем.
- **Docling лучше:** pixel-precise figure crop (grounded для VLM), table cell-level,
  picture classification (chart-type label), picture captioning, image-only pages
  / scans (OCR-engine agnostic), нестандартная вёрстка.

### Вывод: комплементарно, не взаимоисключающе

GROBID — primary structural grounder (текущий). Docling — secondary source для
**figures + tables + picture classification/captioning candidates** (D127: candidate-only).

---

## 7. Granite-Docling / SmolDocling (258M VLM) — production-ready

Это **full-page single-shot VLM**, отдельная ветка pipeline (`pipeline=vlm`),
альтернатива ансамблю (layout+OCR+TableFormer).

- **Granite-Docling-258M** — production-ready преемник SmolDocling-256M.
  258M параметров, Apache-2.0.
  - Backbone: Granite 165M LM + **SigLIP2** (vision, base, patch16-512) +
    Idefics3-style pixel-shuffle connector (nanoVLM training framework).
  - **Запускается локально**: CPU (batch 32, ~0.64s/doc DocLayNet), MPS, CUDA.
  - Выход: **DocTags** (структурированный промежуточный формат Docling), НЕ Markdown.
- **SmolDocling-256M** — предшественник, тоже DocTags, тоже локально-runnable.

### Бенчмарки (в т.ч. arXiv:2605.19866)

| Модель | BLEU | F1 | Precision | Recall | Meteor | Edit-Dist |
|---|---|---|---|---|---|---|
| smol-docling | 0.59 | 0.80 | 0.89 | 0.78 | 0.67 | 0.48 |
| **granite-docling-258M** | **0.65** | **0.84** | **0.91** | 0.83 | 0.72 | 0.29 |
| granite-docling-2stage-258M | 0.70 | 0.85 | 0.92 | 0.83 | 0.79 | 0.27 |

> granite-docling-2stage-258M (layout-сначала, потом OCR) — стабильно лучше
> single-stage, особенно на сложных документах.

### vs Qwen3-VL (для figure understanding)

- **Granite-Docling** = whole-page parser → DocTags. Сильный в **structure-preserving
  conversion** (layout+OCR+table+formula одним проходом), дешёвый, локальный.
  НЕ специализирован на semantic reasoning над отдельным figure-crop.
- **Qwen3-VL (235B)** = large general VLM. Сильнее в **semantic reasoning** над
  grounded crop (chart extraction, diagram component analysis, structured JSON по
  schema) — это то, что нужно ADR-060 Stage 2a/2b.
- Вывод: они **комплементарны**. Granite-Docling заменяет ансамбль для конверсии
  страниц (если нужна single-shot альтернатива GROBID+layout), Qwen3-VL остаётся
  для targeted figure reasoning. Для figure pipeline Qwen3-VL не заменять.

### Вывод JSON vs DocTags

Granite-Docling выдаёт **DocTags** (структурированный текстовый формат), не
произвольный JSON. Для structured JSON-схем (chart_extraction из ADR-060) модель
нужна другая (Qwen3-VL / Granite-Vision-3.3).

---

## 8. Table extraction (TableFormer) — production, дополняет figure pipeline

- **TableFormer** (docling-ibm-models): распознаёт rows/columns/cells и связи.
  Режимы `fast` / `accurate` (параметр `table_mode`). Devices: CPU, CUDA, XPU
  (MPS временно отключён из-за производительности).
- **VLM-альтернатива:** `granite-vision-4.1-4b` выдаёт структуру таблиц через
  **OTSL** (Open Table Structure Language).
- **Cell-level данные:** да, TableFormer даёт ячейки с текстом + структурные
  отношения (rowspan/colspan). Не «только структура».
- **Бенчмарк:** Docling ≥ GROBID на content+topology F1 во всех 3 датасетах (§6).

```python
for element, _ in doc.iterate_items():
    if isinstance(element, TableItem):
        tbl = element.data             # TableData: grid ячеек
        # tbl.grid[c][r].text, .row_span, .col_span, .bbox
        element.export_to_dataframe()  # → pandas DataFrame
        element.get_image(doc)         # PNG рендер таблицы
```

> Для daily-archive: таблицы — самостоятельный high-value канал (Metric/Data nodes).
  Docling tables превосходят GROBID — кандидат на отдельный enrichment-source.

---

## Практический план интеграции в daily-archive (Rust, D237, sidecar-паттерн)

Принципы: (1) D237 — **никакого Python в runtime-пути**, Docling только как HTTP
sidecar; (2) D127 — все выводы Docling = **candidate-only**, `authoritative=false`;
(3) GROBID остаётся primary grounder; (4) Samyama — единственный store (ADR-040).

### Этап 0 — sidecar-инфраструктура (аналог GROBID)

```yaml
# .docker/docker-compose.yml — добавить сервис docling рядом с grobid
docling:
  image: quay.io/docling-project/docling-serve
  ports: ["5001:5001"]
  environment:
    - DOCLING_SERVE_API_KEY=${DOCLING_API_KEY}   # опц.
    - DOCLING_SERVE_ARTIFACTS_PATH=/models        # кэш моделей (volume)
    - DOCLING_DEVICE=cpu                          # или cuda
  volumes:
    - docling-models:/models
```

Health-check: `GET /docs` (200) → аналог `GET /api/isalive` у GROBID.

### Этап 1 — Rust-адаптер `DoclingHttpAdapter` (da-adapters)

Зеркало существующего `GrobidParser` (D204 profile-паттерн):

```rust
// crates/da-adapters/src/docling_client.rs (feature-gated: `docling`)
pub struct DoclingHttpAdapter { client: reqwest::Client, base_url: Url, api_key: Option<String> }

#[async_trait]
impl ParserPort for DoclingHttpAdapter {
    async fn is_alive(&self) -> bool { /* GET /docs */ }
    async fn parse_pdf(&self, pdf: &Path, paper_id: &str) -> Result<ParsedDocument> {
        // POST /v1/convert/file  multipart:
        //   files=@<pdf>, to_formats=md, image_export_mode=referenced,
        //   do_picture_classification=true, table_mode=accurate
        // → парсим DoclingDocument JSON: pictures[].prov/bbox, .meta.classification,
        //   .meta.description; tables[].data
    }
}
```

- Profile-паттерн как у GROBID (`structural`/`rich`): например `figure_only`
  (classification+description on) vs `tables_only`.
- Fingerprint профиля входит в cache-key (урок из D204/GROBID: не смешивать профили
  под одним ключом).

### Этап 2 — порт-контракт `DoclingFigure` в da-ports / da-domain

```rust
// crates/da-domain/src/docling_types.rs
pub struct DoclingFigure {
    pub self_ref: String,
    pub bbox: Bbox,                 // PDF points из prov (→ нормализуем к GROBID-системе)
    pub page_no: u32,
    pub predicted_class: Option<String>,   // 26 классов DocumentFigureClassifier
    pub classification_confidence: Option<f32>,
    pub description: Option<String>,       // VLM caption (candidate)
    pub description_model: Option<String>,
    // НЕТ tabular_chart — такого поля нет
}
```

### Этап 3 — wiring в application (candidate-only, second opinion)

- `FigureContextStage`: принимает и GROBID bbox, и Docling bbox → `FigureSemanticContext`
  несёт оба мнения; primary = GROBID.
- `DocumentFigureClassifier`-label (chart/diagram/photo) → **вход в Stage 1 routing**
  ADR-060 (дешёвый сигнал, thinking OFF).
- Picture description → candidate-caption (D127 false), enrichment кандидатом,
  не заменяет VLM-reasoning Stage 2.
- **Chart-to-table (Stage 2a):** Docling НЕ даёт; реализуем своим Qwen3-VL вызовом
  по schema `chart_extraction`. Docling-классификатор лишь подтверждает chart-type.

### Этап 4 — контракты/тестирование (ADR-051)

- Contract suite: `DoclingHttpAdapter` проходит тот же `ParserPort` контракт, что
  `GrobidParser` (wiremock на `/v1/convert/file`).
- Семантический graph-task suite (Contour 6 §8.1): durable `.sgsnap` с фигурами,
  задача «найти все chart-фигуры» — PASS/EMPTY/ERROR.
- Интеграционный тест с реальным docling-serve (опц., как GROBID testcontainer).

### Поправки к ADR-060 (исходя из находок)

1. **Удалить `PictureItem.meta.tabular_chart`** из «Что работает (SOTA)» §Context и
   из Docling-bullet «Decision». Docling chart-to-table не существует.
2. Stage 2a (chart-to-table) — собственный VLM-вызов (Qwen3-VL), Docling не источник.
3. Stage 1 classifier: DocumentFigureClassifier-лейблы (`bar_chart` и др.) —
   **прямой, дешёвый, локальный** routing-сигнал (thinking OFF не требуется — это
   EfficientNet, не reasoning-модель). Можно как первичный router без VLM-вызова.
4. Picture description из Docling = image-only captioning (без surrounding text) —
   OmniScience-контекст (`FigureSemanticContext`) по-прежнему наш.

---

## Источники

1. Docling — Export figures example (код `export_figures.py`).
   https://docling-project.github.io/docling/_generated/examples/export_figures/
2. Docling — Pictures description example (Granite Vision / SmolVLM).
   https://docling-project.github.io/docling/_generated/examples/pictures_description/
3. Docling — Enrichment features (Picture classification / Picture description).
   https://docling-project.github.io/docling/usage/enrichments/
4. Docling — Model catalog (stages, layout/TableFormer/VLM/picture-classifier).
   https://docling-project.github.io/docling/usage/model_catalog/
5. Docling — REST API (docling-serve v1.21.0, endpoints, options, picture description).
   https://docling-project.github.io/docling/usage/api_server/rest_api/
6. Docling — Deployment (docling-serve: command, Docker Compose, engines, env vars).
   https://docling-project.github.io/docling/usage/api_server/deployment/
7. DocumentFigureClassifier-v2.5 model card (EfficientNet-B0, 26 классов, метрики).
   https://huggingface.co/docling-project/DocumentFigureClassifier-v2.5
8. IBM — Granite-Docling: end-to-end document understanding (анонс, 258M, DocTags).
   https://www.ibm.com/new/announcements/granite-docling-end-to-end-document-conversion
9. MarkTechPost — Granite-Docling-258M обзор (SigLIP2, backbone, Apache-2.0).
   https://www.marktechpost.com/2025/09/17/ibm-ai-releases-granite-docling-258m-an-open-source-enterprise-ready-document-ai-model
10. arXiv:2605.19866 — Structured Layout Priors (бенчмарки granite-docling vs smol-docling).
    https://arxiv.org/html/2605.19866v1
11. Senellart et al. 2025 — Benchmarking Table Extraction (GROBID vs Docling F1).
    https://pierre.senellart.com/publications/soric2025benchmarking.pdf
12. docling-project/docling Discussion #622 — Docling vs GROBID (скорость, scholarly).
    https://github.com/docling-project/docling/discussions/622
13. docling-project/docling-serve — репозиторий HTTP-сервера (v1.21.0).
    https://github.com/docling-project/docling-serve
14. UbiOps — Deploy Docling with Docker (подтверждение FastAPI + endpoints).
    https://ubiops.com/docs/ubiops_tutorials/docling-tutorial/docling-tutorial
