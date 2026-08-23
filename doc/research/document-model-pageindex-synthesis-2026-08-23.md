# Document Model Research — PageIndex Ideas Synthesis (2026-08-23, M451)

Синтез четырёх исследований структурности документов для статей, учебников
и баз знаний. Источники: (1) собственный Python-архив (волны 04/06, M025),
(2) Semantica (vendor), (3) Docling + RAPTOR + chunking-бенчмарки (внешние),
(4) VectifyAI/PageIndex (внешний, индексирован в codebase-memory: 7956 узлов).

## 1. Эволюция идеи PageIndex в нашем собственном архиве

| Поколение | Артефакт | Суть | Что у нас сейчас |
|---|---|---|---|
| Gen 1 nav-tree | wave-04 `indexing/page_index.py` + `navigation.py` | PageIndexNode: parent/children/next/path, **text в узле**, материализованный path, `navigation_anchors`, `validate_navigation()` встроен | ✅ v10 navigation.json (28 988 узлов), selfcheck; 🔶 anchors как отдельная сущность — нет |
| Gen 2 route-chunks | wave-06 `structure_aware_chunking.py` («replaces PageIndex/SemanticChunk») | Чанк типизирован: **ChunkRoute** (claim/method/entity/relation/table/citation/metadata/retrieval_only/exclude), **ChunkType** (12: definition_candidate, table_row_group, figure_caption_context…), **ConfidenceClass** (deterministic/heuristic/requires_review), **GraphReadinessState** (ok_for_graph/retrieval_only/repair/reject), redacted spans | ❌ не реализовано — главное упущение |
| Gen 3 Layer A | ADR-053 | path_id из TEI, структурная валидация | ✅ v10 + html_structure_honesty Governor-контракт (M398) |

Ключевой вывод: идея PageIndex в нашем архиве — не «дерево», а
**структура → маршрутизация экстракции → честность состояний**.

## 2. Внешние подходы

| Источник | Идея | Оценка для нас |
|---|---|---|
| **Docling DoclingDocument** (IBM) | Единое дерево (секции+группы); **furniture-дискриминация** (колонтитулы ≠ body); bounding-boxes на всё; reading order | Принять: furniture-роли (наши 55% path-дублей — furniture «Under review.», «Fig. 1.»). Координаты — позже (GROBID coords null) |
| **RAPTOR / ST-Raptor** | Рекурсивная кластеризация+суммаризация → дерево абстракций, retrieval на уровнях | Принять мини-версию: topic-группы секций (боль #9 big-picture); у нас уже 2 уровня summaries |
| **Chunking-бенчмарк 2026** (36 методов) | Paragraph-group/structure-aware бьют fixed-size ×2 (nDCG 0.46 vs 0.24); доменная чувствительность | Наши секции = готовые structure-aware чанки; для math/bio не только секции |
| **Late chunking** | Сначала длинный контекст, потом границы чанков | Принять: секционные эмбеддинги с path-префиксом (path_to_root) — контекст не теряется |
| **VectifyAI/PageIndex** | (а) **Page-range узлы** {title,start,end} + embedded_toc (PDF-bookmarks = дерево); (б) **tree_cost**: формальная worst-case стоимость поиска (frontier, residual — непокрытые детьми страницы), optimize/merge по стоимости; (в) **Flash** heading-detection без LLM: клики секционных ключевых слов×font-подпись, sentence_like-фильтр, NumberingTrie, forward/reverse курсоры; (г) **vectorless tree-search**: similarity≠relevance, LLM reasoning по структуре; MCP-контракт: next_steps в каждом ответе, JSON-envelope без исключений, char-бюджет; (д) **prompt-injection hardening** входов | Принять: tree_cost как quality-signal + критерий чистки мусора; embedded_toc для монографий; sentence_like в furniture-фильтр; MCP-паттерны для agent-tools W-F; hardening для LLM-стадий. НЕ принимать целиком vectorless (гибрид: структура-first + вектора дополнением) и page-range как единственную гранулярность (статьям нужен text-span) |
| **Semantica** | context bundles с `as_of` (bi-temporal) | Уже отражено (ADR-053 Layer H); parse у них не глубже нашего |

## 3. Целевая универсальная документ-модель (синтез)

Пять измерений; статья — частный случай, учебник/KB — профили.

```
DocumentModel
1. IDENTITY      section_id — единственная идентичность (path_id не уникален,
                 55% статей имеют дубли; корпус: 0 дублей section_id)
2. GRAMMAR       профили как DATA (article_schema):
                 • arxiv_pdf_dot    '1.2.3' (567 статей; '~'-рестарты не родители)
                 • html_headings    h1>h2>h3 — 28 gnn-глав (сегодня BLOCKED M398)
                 • latex_env        theorem/definition/exercise — монографии
                 • page_outline     PDF-bookmarks (embedded_toc) — trench
                 • kb_page          будущие базы знаний
3. TYPING        role: body|furniture|header_metadata (Docling)
                 structural_type: theorem, definition, exercise, table_row_group,
                   figure_caption, reference_entry, administrative (Gen-2)
                 route_hint: какой extraction-route (Gen-2 ChunkRoute)
                 confidence: deterministic|heuristic|requires_review
4. ANCHORS       доменно: text-span (статьи) | page-range (монографии) |
                 element-id (HTML) — объявлено грамматикой
5. QUALITY       инварианты (next/prev симметрия, dangling parent, path-дубль
                 требует отличной role) + tree_cost-сигнал (frontier/residual)
                 + GraphReadinessState per-chunk (ok_for_graph|retrieval_only|
                   repair_required|reject — родня наших missingness/not_claimed)
```

## 4. Retrieval-стратегия (синтез Vectify-тезиса с нашей гибридностью)

- **Structure-first reasoning**: агент ходит по navigation.json (vectorless
  walk, их MCP-контракт: next_steps, envelope, бюджет) — W-F agent read-plane
- **Вектора — дополнением**: late-chunking секционные эмбеддинги с
  path-префиксом (не similarity-первичность)
- **Summaries = уровни абстракции** (мини-RAPTOR): paper → section →
  topic-group (будущее, лениво)
- **RouteChunking**: extractor берёт route_hint вместо эвристик (таблица =
  table_row_group-чанк, не «где-то в секции»)

## 5. Безопасность и честность (сквозное)

- Prompt-injection hardening перед всеми LLM-стадиями (Vectify classic)
- ConfidenceClass ↔ наш identity_method; GraphReadinessState ↔ missingness/
  not_claimed (M398 fail-closed философия совпадает)
- ACAP-lineage везде: cache key = pid+model+prompt_sha (+sections_sha)
- D127/D136-замки не затрагиваются

## 6. Дорожная карта (цена/эффект)

| Волна | Что | Закрывает |
|---|---|---|
| v11 | furniture-роли + структурные инварианты + tree_cost-сигнал + injection-hardening | чистка 55% мусора, измеримость навигации, безопасность LLM |
| v12 | html_headings-профиль → 28 gnn-глав в пайплайне | учебники-HTML (M398-BLOCKED), боль #5 |
| v13 | page_outline (trench bookmarks) + latex_env (theorem/definition) | монографии, глоссарный вход Layer J |
| v14 | route_hint+readiness на чанках | extractor-маршрутизация, Datapoint-подготовка |
| W-F | agent read-tools по navigation (next_steps/envelope) | agent brain, vectorless walk |

## 7. Что сознательно НЕ принимаем

1. Полный vectorless RAG — наш корпус и графи знаний требуют гибрида
   (structure-first + вектора + граф), не замены
2. Page-range как единственная гранулярность — статьям нужна span-точность
3. Flash font-клики сейчас — нет font-метрик в GROBID-выхлопе (вернуться при
   layout-источнике)
4. Chunk-пайплайн «поверх всего» (Gen-2 redacted-контракт) — у нас слои+
   манифесты дают ту же честность без дублирующей модели чанков; берём
   типизацию и маршрутизацию, не транспорт

## 8. Карта «идея → источник → куда»

| Идея | Источник | Куда |
|---|---|---|
| furniture-роли | Docling | v11 role-поле navigation |
| sentence_like-фильтр | Vectify flash | v11 furniture-детектор |
| tree_cost/frontier | Vectify | v11 quality_signal navigation_efficiency |
| embedded_toc/bookmarks | Vectify flash | v13 page_outline |
| нумерационные последовательности | Vectify NumberingTrie | v11/v12 grammar-валидатор |
| next_steps/envelope MCP | Vectify | W-F agent tools |
| prompt-injection hardening | Vectify classic | summaries/per_section входы |
| topic-группы | RAPTOR | ленивая фоновая задача (после v12) |
| late-chunking path-префикс | Jina/бенчмарк | embedded-стадия (кэш-инвалидация!) |
| route_hint | наш Gen-2 | v14 |
| definition_candidate | наш Gen-2 | Layer J терминология (боль #5) |
