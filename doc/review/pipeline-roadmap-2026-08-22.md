# Pipeline Frontier Roadmap — порядок волн, цели, контрольные точки (2026-08-22)

Основа: аудит M445 (pipeline-stage-audit-2026-08-22.md). Принцип порядка:
(1) сначала независимые быстрые победы, (2) затем фундамент словарей,
который разблокирует сразу три направления, (3) потом потребители словарей,
(4) инфраструктурные волны в конце — они дороги и не блокируют остальных.
Каждая волна закрывается через GSD + regression-check (M444) + гейт.

## Порядок волн

| Приоритет | Волна | Цель (что получаем) | Задачи | Зависимости | Контрольные точки (измеримо) | Размер |
|---|---|---|---|---|---|---|
| **W-A** | Quick wins | Погасить точечные алармы и гигиену | A1: methods.phrases для math-oc/robotics (3 zero-mention); A2: unify edge_contracts.yaml (один канонический путь); A3: alias-fix document_context в ledger (Q4#4); A4: governor-пин «волна пересборки прикладывает regression_report.json» | нет | • drift_stats: zero_mention_alarms = 0<br>• rg-проверка: один edge_contracts<br>• governor self-test зелёный с новым пином | S |
| **W-B** | Словари цитирования (фундамент) | Единая DATA-база для ссылок/сущностей; убить единственный хардкод | B1: doi_prefix_map.yaml (source_code→префикс DOI); B2: citation_styles.yaml (формы arXiv/bibTeX/DOI/URL); B3: venue_registry.yaml (канонизация журналов, алиасы); B4: author_identity.yaml (канонизация авторов) | нет (A2 желательно первым) | • hardcode-scan = 0 находок (OpenAlex берёт префикс из DATA)<br>• парсер citation_styles: ≥95% эталонных строк из корпуса парсятся типизированно (фикстуры)<br>• venue/author: дедуп-тесты на алиасах (ACM TOCL≡Trans. Graph…) | M |
| **W-C** | Сущности-цитаты | Авторы/журналы как узлы графа; связи «статья→автор→журнал» | C1: extraction авторов/журналов через B3/B4; C2: материализация Author/Venue + рёбра WROTE/PUBLISHED_IN (vid по канон-ключу); C3: расширить regression-harness на новые семейства узлов | **W-B** | • в графе 589 статей: ≥90% авторов сгруппированы без коллизий (выборочная проверка 20)<br>• replay-idempotent: повторная материализация = 0 новых узлов<br>• regression-check зелёный на расширенных отчётах | M |
| **W-D** | Мультисорс-аквизиция | Источники кроме arXiv; DOI-first вход | D1: rust-native fetch за SourceAcquisitionPort (W10 ADR-056); D2: DOI-first резолюция (doi.org) → источник+версия; D3: адаптер PubMed/bioRxiv по тому же порту; D4: сущность-словарь source_codes пополнен | **W-B** (doi_prefix_map) | • одна не-arXiv статья (DOI-first) проходит acquire→checkpoint с типизированным LoadReport<br>• SourceIdentity различает pdf/html/sha без смешения (контракт-тесты)<br>• сеть только в Nightly-контуре (ADR-051), оффлайн-моки зелёные | L |
| **W-E** | Постраничный AST | Уровень страница→колонка→блок под document_context; кросс-страничные окна | E1: модель PageAnchor в DocumentContextProjection; E2: координаты GROBID → привязка блоков; E3: кросс-страничные quote-окна (фигуры); E4: governor-контракт расширен | желательно после C (сущности поверх стабильного AST) | • фикстуры: каждый блок имеет page_id+path_id; рестарт-правила как в §1 аудита<br>• кросс-страничная фигура даёт единое окно (фикстура)<br>• html_structure_honestity расширен и зелёный | M-L |
| **W-F** | Агентская готовность | Мозг агента + полный стор | F1: FSM-состояния REVIEW/OUTPUT (ADR-039) в домене+read-plane; F2: подключение RuVector (оценка, потом wiring); F3: P1 durable Samyama server (remote GraphStore); F4: пересмотр GraphWrite (D278) после F3 | частично W-C/E (контекст богаче) | • agent-run FSM покрывает REVIEW/OUTPUT (тесты переходов)<br>• решение-ADR по RuVector: wiring или замена<br>• DA_GRAPH_MODE=remote открывает полный стор (не NotConfigured)<br>• D127-разблокировка — ТОЛЬКО по явному «да» владельца | L |
| **W-G** | Научная глубина | Со-цитирование, citation-context, Crossref-fallback | G1: co-citation рёбра (через C); G2: citation-context за GROBID-маркерами (через E); G3: Crossref-fallback обогащения (через B/D) | **W-C, W-E, W-D** | • co-citation граф на 589 статьях: связность/степени в отчёте<br>• ≥1 citation-context извлечён нетривиально (фикстура)<br>• enrichment-fallback срабатывает при отсутствии OpenAlex | M |

## Цепочка зависимостей (текстом)

```
W-A (независимо) ─────────────────────────────► Quick wins закрыты
W-B (словари) ──► W-C (сущности-цитаты) ──► W-G (co-citation, контекст)
W-B ──► W-D (мультисорс, DOI-first)
                W-E (постраничный AST) ──► W-G2
W-C+W-E ──(богатый контекст)──► W-F (агенты: FSM, мозг, P1-стор) ──► D127-решение
```

## Контрольные точки волны (сквозные, обязательные после каждой)

1. Полный гейт `scripts/verify_rust_architecture.sh` — PASS (обе feature-сборки).
2. Governor trajectory — ON_TRACK (flags=0 violations=0, кроме бытовых).
3. `corpus_regression_check` на пересборке — 0 findings (или осознанный диф с ревью).
4. drift_stats — без новых zero-mention алармов.
5. GSD-валидация волны: pass + runtime-UAT evidence.

## Что НЕ делать (замки)

- D127-разблокировка без явного «да» владельца.
- DSPy/GRPO admission до замороженного eval (D136).
- GraphWrite executor до durable-стора (D278).
- Физическое удаление дубликатов каталога (память: only lookup-surface unification).

## Первый следующий шаг

**W-A + W-B одной сессией**: quick-wins (S) + словарный фундамент (M) —
после них сразу три направления (C/D/G) разблокированы, а единственный
хардкод в проде исчезает. Приоритет внутри W-B: B1 (doi_prefix_map) → B2
(citation_styles) → B3/B4 (venue/author).
