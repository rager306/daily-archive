# Инвентарь локального корпуса и лицензий, граница внешних байтов (M514-1lss2s/S02/T04)

Статус: инвентарь снят на живом дереве. Это **не** релиз, **не** заявление о
правах и **не** заявление о качестве. Измеренные факты отделены от неизвестного
(§2 и §3), границы D491 названы в §4, а исполняемой проверкой запись держит
`scripts/tests/test_absorption_progress_corpus_inventory.py`.

Носители ниже — **read-only**: `data/parse/holdout_manifests.yaml`,
`artifacts/parse_compat/external_holdout_release_probe.json`,
`data/parse/absorption_progress.schema.yaml`, `data/parse/license_inventory.yaml`,
`data/parse/research_authorization.yaml` не редактировались.

## 1. Что и откуда (носители инвентаря)

| Роль | Путь | sha256 на этой ревизии |
|---|---|---|
| оценка: назначенная популяция 610 | `data/parse/holdout_manifests.yaml` | `ef91d3c25c7d91dda2654471140fd41fcae8e1ed87e7094ad24c31fef2b64a39` |
| оценка: замороженный probe (байт-пины, census присутствия, внешняя полоса) | `artifacts/parse_compat/external_holdout_release_probe.json` | `a937da95ef987dffdcde46b998e98549fe247999e50986e1b0627a6f485d8275` |
| контракт scope/корпуса/сплита (DATA) | `data/parse/absorption_progress.schema.yaml` | `4ed78e34cb5863cf0fcbf6c09fed1351e1bb722659c73b881a36b1a16317641a` |
| компонентные лицензии (DATA, §4) | `data/parse/license_inventory.yaml` | `4f860c17a7963570b7233988034a7005cde3ed0f3cde6785b96d23fb53caba66` |
| активное исследовательское разрешение D491 (DATA, §4) | `data/parse/research_authorization.yaml` | `caa0c5c3baf6e4156375f74cb2d3c691530b01cb2567ce698e431e07be7a7a41` |

Тюнинговый корпус: 200 файлов по glob `data/gold_standard/*.gold.json`.

## 2. Измеренные факты

### 2.1 Оценочный корпус: 610 локальных документов

- В манифесте 610 статей, 610 **различных** `article_id` (дубликат раздул бы
  перепись, поэтому различие проверяется, а не предполагается).
- Все 610 несут `status: bytes_present`; значение совпадает с
  `assigned_population.manifest.present_value` контракта.
- Каждая статья несёт байт-пин: `sha256: <64 hex>` (хеш PDF) и `size_bytes > 0`.
- Локальное измерение дерева (T04): все 610 `pdf_path` существуют и их
  фактический размер совпадает с `size_bytes` — **0 расхождений**, суммарно
  **2.78 ГБ**. Файлы корпуса лежат под `data/sources/arxiv/` и **gitignored**,
  поэтому это измерение зафиксировано здесь как факт прогона, а не как
  CI-стабильная проверка (см. §5).
- `reference_population` probe: `documents = 610`, `measurable_documents = 531`,
  `excluded_documents = 79`, `role: identity_only`,
  `identity_sha256 = sha256:eee622ae57286faff567b1793f31ee54c5df3118e544a4bb8edf1c7376955c6d`,
  причём `identity_digest_is_byte_hash: false` — это дайджест идентичности, не
  хеш байтов.

### 2.2 Байт-пины probe называют те же 610 идентичностей

`reference_population.byte_pins` — список из **610** записей
`{document_id, identity_sha256, size_bytes}`. Множество `document_id` в нём
совпадает с множеством `article_id` манифеста. `identity_sha256` хранится
**без** префикса `sha256:` (голые 64 hex) — в отличие от per-article `sha256`
манифеста, где префикс есть; это дайджест идентичности
(`identity_digest_is_byte_hash: false`), а не хеш PDF-байтов. Это второй,
независимо сгенерированный замороженный источник для той же популяции —
совпадение идентичностей не выводится из одного носителя.

### 2.3 Связь probe с живыми байтами манифеста

`reference_population.source_manifest = data/parse/holdout_manifests.yaml` и
`source_manifest_sha256` равен sha256 живого манифеста (§1). Замена или
усечение манифеста поэтому видны немедленно.

### 2.4 Производственный каталог: пересечение 0

`production_catalog_overlap` в probe (тип переписи —
`filesystem_presence_of_the_holdout_identities`): `documents = 610`,
`declared_pdf_present_count = 610`, `absent_from_every_locator_count = 0`,
`canonical_catalog_present_count = 0`, `gold_standard_present_count = 0`.
Контракт объявляет `independent_corpus.disjointness.production_catalog_overlap: 0`.

Дополнительно пере-выведено на живом дереве по правилу probe
(`<root>/<категория>/<id>/source/<id>.pdf` при root
`data/article_catalog/article_catalog/arxiv`): **0 совпадений**. Корень канонного
каталога сейчас отсутствует, поэтому ни один из 610 документов не является
продуктовым материалом. Если holdout-идентичность позже попадёт в канонный
каталог, объявленный `0` станет устаревшим — проверка падает намеренно, требуя
осознанного пересмотра, а не тихого перекрытия.

### 2.5 Тюнинговый корпус: 200 документов gold

`data/gold_standard/*.gold.json` — **200** файлов, 200 различных идентичностей;
у каждого файла `paper_id` равен идентичности, выведенной из имени файла
(правило `independent_corpus.tuning.identity_rule`), `corpus_id =
gold-standard-200`.

### 2.6 Дизъюнктность по arXiv identity: 0

Пересечение множеств `article_id` манифеста и идентичностей gold — **пустое
(0)**. Дизъюнктность объявлена по `document_identity` / `arxiv_id`;
`prompt_hash_forbidden: true` и `split_contract.prompt_hash_disjointness_forbidden:
true`: хеш промпта — это lineage, а не независимость. `evaluation_never_tuning_input:
true`, `reuse_forbidden: true`.

### 2.7 Внешняя полоса: `bytes_absent`, не доказательство

Probe `external_holdout_band`: `external_bytes = bytes_absent`,
`execution_status = not_executed`, `external_rows_are_evidence = false`,
`citation_without_release_decision = forbidden`.

Контракт `independent_corpus.external` согласован с полосой: `band =
bytes_absent`, `execution_status = not_executed`, `not_evidence: true`,
`download_forbidden: true`, `materialize_forbidden: true`,
`authorization: separate_authorization`; правила внешнего пакета принадлежат
`scripts/governor/checks/external_holdout_package.yaml` и здесь не
пересказываются.

## 3. Неизвестное и не записанное

### 3.1 Правовой статус на уровне документа не записан

У 610 оценок (arXiv-препринты) и у 200 gold **нет** записи о лицензии на
уровне документа: в манифесте нет ключей `license`/`rights`/`copyright`, а
per-article права этим репозиторием не собирались. Это **неизвестно**, а не
«verified»: запись не вправе называть права установленными, и тест утверждает
отсутствие такого заявления, а не его наличие.

Единственная принадлежащая репозиторию позиция по этим двум корпусам —
компонентная запись `data/parse/license_inventory.yaml`:

- `corpus.internal_gold`: `license: mixed_per_article`, `license_status: blocked`,
  `redistribution: blocked`, `blocked_reason: per_article_license_check_required`;
- `corpus.external_holdout`: `license: CC-BY`, `license_status: blocked`,
  `redistribution: blocked`, `blocked_reason: rights_not_established`.

Словарь статусов закрыт: `[verified, blocked]`; ни один из этих двух компонентов
не является `verified`.

### 3.2 Внешние байты не загружены

Внешняя полоса — `bytes_absent`, прогон `not_executed`; внешние строки не
являются доказательством. Права внешнего датасета принадлежат поверхности
приёмки (`data/parse/acceptance_manifest.yaml`) и проверке
`external_holdout_package.yaml`; этот инвентарь их не устанавливает.

### 3.3 Что D491 не даёт

D491 снимает бумажную часть C3 как исследовательский блокер, но **ничего не
дарует** от третьих лиц: `not_granted.third_party_legal_rights/license_clearance/
redistribution/graph_write/import_eligible/quality_pass` — все `false`
(§4).

## 4. Границы D491 (принадлежат DATA, не этому документу)

Границы не пересказываются здесь как источник: они читаются из
`data/parse/research_authorization.yaml` и `data/parse/license_inventory.yaml`.
Инвентарь лишь называет их и подтверждает, что запись им не противоречит.

**Не распространение, не запись в граф, не импорт:**

| Граница | Где | Значение |
|---|---|---|
| `new_distribution` | `research_authorization.retained_locks` | `forbidden` |
| `graph_write` | `research_authorization.retained_locks` | `forbidden` |
| `import_eligible` | `research_authorization.retained_locks` | `false` |
| `new_distribution` | `license_inventory.redistribution_policy` | `forbidden` |
| `graph_write` | `license_inventory.redistribution_policy` | `forbidden` |
| `import_eligible` | `license_inventory.redistribution_policy` | `false` |
| `auto_promote` | оба | `forbidden` |
| `unknown_rights` | `license_inventory.redistribution_policy` | `blocked` |

Разрешение опознаётся как `owner_directive` / `authorized` / `recorded_decision:
D491`; `scope.c3_process_gates: waived` относится только к отсутствию
release-решения, а `not_granted.quality_pass: false` — к тому, что разрешение
никогда не читается как pass. Контракт указывает на этот носитель
(`authorization.authority = data/parse/research_authorization.yaml`,
`decision: D491`) и держит покрытие отдельно от разрешения
(`product_coverage_effect: none`, `locks_stay_in_force: true`).

## 5. Что проверяет тест — и чего он не проверяет

`scripts/tests/test_absorption_progress_corpus_inventory.py` пере-выводит §2
и §4 из носителей на каждом прогоне: 610 идентичностей и их байт-пины, 200 gold,
дизъюнктность, отсутствие перекрытия с канонным каталогом, полосу `bytes_absent`,
компонентные блокировки и границы D491. Правка любого носителя меняет то, что
запись обязана утверждать; устаревшая прозаическая строка обнаруживается
читателем, а не регуляркой по тексту, поэтому тест **намеренно не разбирает**
этот документ (проверяется только его существование).

Границы проверки:

- Байты корпуса (`data/sources/arxiv/`) **gitignored**, поэтому исполняемая
  проверка привязывает *объявленные* факты присутствия: `status: bytes_present`
  для всех 610, байт-пин каждой статьи и census присутствия probe
  (`declared_pdf_present_count = 610`, `absent_from_every_locator_count = 0`).
  Измерение байтов на локальном дереве (§2.1) воспроизводимо только там, где
  корпус выложен; оно не может быть CI-стабильным и не притворяется им.
- Пересечение с производственным каталогом пере-выводится обходом живого дерева
  по правилу probe. Обход опирается на то, что канонный каталог отсутствует;
  появление в нём holdout-идентичности — сигнал дрейфа, а не ложная тревога.
- E3: это consistency-проверка над ревьюируемыми DATA плюс обход файловой
  системы, **не** tamper-evident attestation. Актор, переписывающий носитель и
  тест одновременно, её удовлетворит; ценность в том, что отсутствие, подмена,
  перекрытие и не-типизированное заявление о правах перестают проходить как
  соответствие.

## 6. Воспроизведение

```bash
PYTHONPATH=scripts python3 -m pytest scripts/tests/test_absorption_progress_corpus_inventory.py -q
```

Локальное измерение байтов (там, где корпус выложен; не CI-проверка):

```bash
python3 - <<'EOF'
import os, yaml
arts = yaml.safe_load(open('data/parse/holdout_manifests.yaml'))['articles']
bad = [a['article_id'] for a in arts
       if not os.path.isfile(a['pdf_path']) or os.path.getsize(a['pdf_path']) != a['size_bytes']]
print(len(arts), 'declared;', len(bad), 'existence/size mismatches;',
      round(sum(a['size_bytes'] for a in arts) / 1e9, 2), 'GB')
EOF
```
