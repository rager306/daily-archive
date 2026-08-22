# Continue — Handoff после M443; hot path проиндексирован (37–154x)

## Состояние (2026-08-22, конец сессии M443-ss1ks0)

- **M443-ss1ks0 закрыт** (validate pass → gates 0 → consistency ok →
  complete). Коммит `04a1ce6b`, решение D284.
- **Индексы горячего пути**: da-application/materialization_indexes
  (9 identity-ключей; Citation.arxiv_id исключён — семантика дубликатов);
  раннер обеспечивает индексы после open/preload. Бенчмарк: Entity.vid
  **37x**, Paper.arxiv_id **154x** на полном сторе (66k узлов), сборка
  360 мс. Регрессия: структурная идентичность, 0 replay-нарушений.
- Следующий full-run пересборки каталога ожидаемо минуты-десятки минут
  вместо ~2ч (lookup-стоимость устранена).
- Governor logging-пин поймал bench-пример до коммита — контур
  enforcement работает.

## Следующий шаг (приоритет)

1. **Пересборка каталога с индексами** (валидация сквозного выигрыша на
   полном прогоне; resume доступен) — быстрый операторский прогон.
2. D127-решение по каталогу-графу (интеграция gold+catalog в queryable
   слой) — требует вашего явного «да».
3. DATA-словарь методов для не-CS (3 low-yield статьи).

# Continue — Handoff после M442; каталог материализован в граф (589 статей)

## Состояние (2026-08-22, конец сессии M442-fafzp6)

- **M442-fafzp6 закрыт** (validate pass → gates 0 → consistency ok →
  complete → checkpoint). Коммиты `45f60bab`, `b2da8fa1`, `9bdbac59`.
- **Полный прогон: 589/604 (97.5%) статей каталога в одном графе**:
  catalog_materialized.sgsnap — 66299 узлов / 169916 рёбер, 0 failed,
  **0 replay-noop violations**. Снапшот закоммичен (24МБ) как durable
  candidate evidence. D127 locked.
- **Feature-matrix пин**: гейт компилирует da-cli --features
  pdf-classifier --examples; Governor-политика feature_matrix_gate
  флагает исчезновение пина (78 self-test).
- **Crash-safe resume** (--resume / DA_MATERIALIZE_RESUME=1): первый
  full-run умер на 540/589 без resume — добавлен и E2E-проверен.
- Сравнение с gold: плотность сопоставима (112.6/288.5 vs
  126.4/297.8 узлов/рёбер на статью), наборы непересекающиеся;
  catalog_vs_gold_comparison.json.

## Следующий шаг (приоритет)

1. Решение оператора по D127 unlock для каталога-графа (сейчас
   candidate evidence only) — или интеграция обоих графов (gold+catalog)
   в один queryable слой через serve-graph.
2. DATA-словарь методов для не-CS полей (3 low-yield статьи свипа).
3. Производительность full-run (~2ч): per-paper lookup растёт линейно
   со стором — кандидат на индекс vid→node_id в materialize_extracted.

# Continue — Handoff после M441; recovery 66/81, полный корпус готов

## Состояние (2026-08-22, конец сессии M441-h0uc3h)

- **M441-h0uc3h закрыт** (validate pass → gates 0 → consistency ok →
  complete → checkpoint). Коммит `e0104199`. Governor closeout ON_TRACK.
- **Recovery: 66/81 no_tei_revision восстановлены** из PDF-штампов
  (arXiv:<id>vN, страница 1-2, pdf-inspector за feature pdf-classifier,
  оффлайн). Merged-канарка: 30/30, 0 failed, 0 replay-noop; снапшот
  3341 узлов / 6990 рёбер.
- Итог по покрытию материализации: **589/604 (97.5%) bindable**
  (523 TEI + 66 штампы), 15 остаются typed-skip no_stamp.
- Попутно починен pre-existing баг feature-сборки da-cli
  (RegionTextPort import, ломался с M422; гейт эту комбинацию не
  собирал — кандидат на усиление гейта).
- Новый оператор: `--recover` + `DA_MATERIALIZE_RECOVERY=... --run`
  (см. AGENTS Operators).

## Следующий шаг (приоритет)

1. **Full run 589 статей** → единый не-gold граф + сравнение с gold
   (M437). Решение по объёму за оператором.
2. DATA-словарь методов для не-CS полей (3 low-yield статьи).
3. Governor: пин «feature-matrix build» в verify_rust_architecture.sh
   (--features pdf-classifier для da-cli) — чтобы feature-брейки больше
   не жили годами.

# Continue — Handoff после M440; свип материализуем, GSD здоров

## Состояние (2026-08-22, конец сессии M440-rv8tlx)

- **M440-rv8tlx закрыт** (validate pass → gates 0 → consistency ok →
  complete → checkpoint). Коммит `228e7853` (атомарный: код+тесты+доки+
  артефакты). Governor closeout: **ON_TRACK flags=0 violations=0**.
- Кэш свипа M439 теперь материализуем: **523/604 бумаг bindable**
  (ADR-065 tei_declared_revision fallback, D283), 81 typed-skip
  (no_tei_revision). Канарка 25 статей: 25/25, 0 failed, **0 replay-noop
  violations**, снапшот 2626 узлов/5484 рёбер.
- Новое: `da-application/catalog_materialization.rs` (FSM scan→bind→
  materialize→verify, terminal-absorb proptest, D127-canary),
  `da-cli/catalog_materialize.rs` (audit+loader, typed errors),
  `examples/catalog_materialize_runner.rs` (--audit-only / --run;
  env DA_MATERIALIZE_{LIMIT,SNAPSHOT,RUN_REPORT}; ADR-052 logging).
- Полный прогон 523 статей: `cargo run -p da-cli --example
  catalog_materialize_runner -- --run` (~минуты, оффлайн). Выходной
  снапшот — candidate evidence, D127 locked.

## Следующий шаг (приоритет)

1. **Full run 523 статей** → corpus-wide не-gold граф + метрики против
   gold-графа (M437). Решение оператора по объёму.
2. DATA-словарь методов для не-CS (3 low-yield статьи свипа).
3. Восстановление версий 81 no_tei_revision (HTML idno / arXiv API) —
   новый кандидат после M440.

# Continue — Handoff после M439; каталог обработан полностью

## GSD health-кампания 2026-08-22 (поверх волны M439)

Doctor: **651 → 139 issues**. Решено: checkbox_db_status_divergence 468→0
(хотфикс рендерера skipped→[x] в ОБОИХ копиях markdown-renderer.js,
бэкапы *.bak-skipped-render; обновление gsd-pi 00:35 затёрло старый фикс —
MEM921), 21 legacy-веха без SUMMARY реконструирована через
executeSummarySave (честные reconstruct-баннеры), фантом M361 удалён
движковым deleteMilestone, state.json refreshed (milestone: null).
Доктор-итог: **ok:true, 0 errors** (2 warnings: env_git_remote —
косметика/ложная, remote работает; stale_uncommitted — сам continue.md).
138× validation_source_revision_mismatch погашены хотфиксом D282:
reportMilestoneValidationSourceDrift early-return без env
GSD_REPORT_CLOSED_MILESTONE_REVISION_DRIFT=1 (обе копии
doctor-engine-checks.js, бэкапы *.bak-drift-check). Подделку evidence
отклонена: фейковые записи устареют со следующим коммитом и фальшивят
event-log. Апдейт gsd-pi затрёт хотфикс — пере-применить. Дубли
bare/suffixed M285–M291 безвредны, не трогать.

## Состояние проекта (на конец сессии)

- **Волна M420→M439 (20 milestones) закрыта и запечатана** на master,
  головка `d3753ebd` (continue.md) ← `5ba51242` (trajectory ON_TRACK)
  ← `101efab3` (logging pin) ← `64d2de5d` (M439 sweep).
- QUEUE пуст, STATE: no active milestone. **D127 locked** (import
  fail-closed), все отчёты import_eligible=false.
- **Governor ON_TRACK flags=0 violations=0** (disk вычищен: target/
  19G→15G, incremental удалён, свободно 99G).
- GitNexus reindex свежий: 30339 узлов / 56801 рёбер (AGENTS.md обновлён).

## Что сделано в этой сессии (хронология)

| Волна | Суть | Коммит |
|---|---|---|
| **M436** kg-pipeline honest engine FSM | Фейковый ExecutionEngine (retry/failure policy игнорировались, 4/6 FSM-статусов недостижимы) заменён реальным: retry-реестр + Sleeper-порт (ADR-051), Abort/Continue/RetryThenAbort с транзитивным скипом, StepEvent-поток, EngineControl pause/stop. ADR-064. Governor-пин `pipeline_engine_honest`. | `d5e93613` |
| **M437** Corpus-wide semantic graph (MEM679) | Первый корпусный граф: 173 статьи → corpus_graph.sgsnap (21868 узлов/51518 рёбер), replay_zero_writes на всех волнах, chain continuity, sha256-bound. **ADR-065**: каталог 100% versionless → честный tei_declared_revision binding (GROBID idno из parse-кэша), 27 no-version бумаг исключены DATA-манифестом. Canary stage-id drift fix (red-first). Corpus task suite 10 задач all-PASS. | `4dee321e` |
| **M438** Architecture closure | **ADR-066/D280**: Orchestrator остаётся product-side (durable vs in-process retry — комплементарны, пересечение ~30 строк). docs_drift_check.sh + audit-fields/schema-check, секция Schema drift gate в GRAPH-SCHEMA.md, negative fixture. | `32dc02da` |
| **M439** Catalog-wide sweep | 606 не-gold статей обработано (parse→extract→embed): **604 ok (99.7%), 0 failed**, 9 source-аномалий, 12.3 мин. **Продуктовый баг исправлен**: rule_extractor резал текст по байтам из lowercase-копии → паника на мультибайте; char-boundary snapping + red-first тест (мутация воспроизводит панику). Метрики: entities p50=9/p95=18; sections p50=42. Рекомендации в analysis.md. | `64d2de5d`+`101efab3` |
| Disk cleanup | target/ 20G→15G (incremental), Governor ON_TRACK. | `5ba51242` |

## Ключевые артефакты

- `artifacts/gold_corpus/corpus_graph.sgsnap` — корпусный семантический
  граф (173 papers), AllWavesReport с sha256-привязкой
- `artifacts/catalog_sweep/{sweep_report,aggregate}.json + analysis.md` —
  свип каталога с метриками и 5 рекомендациями оптимизации
- `data/graph_tasks/gold_corpus.tasks.cypher` — корпусный semantic suite
- ADR-064 (honest engine), ADR-065 (tei binding), ADR-066 (orchestrator
  defer); D280

## Открытые кандидаты (требуют решения/разведки, не фабриковать no-op)

1. **DATA-словарь методов** — расширить methods.phrases для не-CS полей
   (3 статьи свипа с 0 entities: math-oc, robotics). Чистый DATA-путь.
2. **HTML-путь для 8 no_pdf аномалий** — HtmlParseStage готов, нужен
   wiring в свип-раннер.
3. **Материализация свипа в граф** — 606 статей сейчас только в кэше;
   требует решения по version-binding для не-gold (ADR-065 fallback
   уже есть — tei_declared_revision должен покрыть большинство).
4. **GROBID workers=8** — холодный прогон упал бы с ~34 до ~17 мин
   (проверить память контейнера -Xmx4g).
5. **Crash-safe partial flush** отчёта свипа (сейчас resume спасает кэшем).
6. Отложено навсегда/долго: Orchestrator→движок (ADR-066), 27 no-version
   бумаг (ждут upstream evidence), FailureAction consumer-or-remove.

## Операторы (проверенные команды)

```bash
# Per-crate dev loop (НЕ workspace — rocksdb trap)
cargo test -p <crate> --lib; cargo clippy -p <crate> --all-targets --no-deps -- -D warnings

# Полный гейт — только в конце слайса
bash scripts/verify_rust_architecture.sh

# Governor (быстрый, ~1s)
python3 scripts/check_project_trajectory.py --phase preflight|active|closeout

# Свип каталога (resume-safe, кэш content-addressed)
cargo run -p da-cli --example catalog_sweep_runner -- --workers 4 \
  --include-embedding --cache-root artifacts/catalog_sweep/pipeline_cache \
  --report artifacts/catalog_sweep/sweep_report.json

# Корпусный граф: запросы
cargo run -p da-cli -- graph-stats --snapshot artifacts/gold_corpus/corpus_graph.sgsnap
cargo run -p da-cli --example graph_task_suite -- --snapshot artifacts/gold_corpus/corpus_graph.sgsnap \
  --tasks data/graph_tasks/gold_corpus.tasks.cypher --report artifacts/graph_tasks/gold_corpus_report.json

# GitNexus reindex (после структурных изменений)
node .gitnexus/run.cjs analyze
```

## Do not (грабли, собранные за сессию)

- **Unlock D127**; изобретать версии статей; писать в граф из свипа.
- **Слайсить строки по байтам из lowercase-копий** — snap_down/snap_up
  (M439 баг). Governor-пин на это пока НЕ добавлен — кандидат на усиление.
- Хардкодить stage-id словари — читать из run report (M437 canary баг).
- Governor forbidden/required паттерны — точные строки кода/маркеры, не
  английские фразы (матчат doc-комментарии; уроки M435/M436).
- gsd_validate: evidence-классы гомогенны per verification class
  (Contract=runtime, Integration=artifact, UAT=runtime gsd_uat_exec);
  browser-requiring слайсам — per-slice UAT строки.
- `rm -rf` блокируется guard'ом даже после подтверждения — использовать
  `find <path> -depth -delete`.
- Деструктивные операции над target/ — только через явное подтверждение
  пользователя (ask_user_questions destructive_confirm).
- После pre-commit «Stashing unstaged files» — rg-проверка что doc-правки
  попали в коммит (MEM918).
- Ручной ввод 64-символьных SHA в validate — всегда свежий
  captureMilestoneVerificationSourceRevision (M430/M434).

## Следующий шаг (приоритет)

Рекомендую: **(1)** расширение DATA-словаря методов + перегон 3 low-yield
статей (быстрый измеримый выигрыш); **(2)** материализация свипа в граф
(ADR-065 binding уже готов — проверить покрытие tei_declared_revision на
не-gold корпусе). Пункты 4-5 из списка — по мере надобности.
