# Права и упаковка модельных данных GROBID

Статус: **inventory complete, model redistribution blocked**  
Волна: M507 / S01 / T01  
Дата проверки: 2026-09-14

## Вывод

Пин baseline — `java-grobid-http@0.9.0-crf`, запускаемый как внешний Java HTTP
service. Репозиторий GROBID tag `0.9.0` содержит файл `LICENSE` с Apache License
2.0; это подтверждает `Apache-2.0` для самого GROBID upstream work. Использование
неизменённого сервиса допустимо (`reusable: true`).

Это **не** является разрешением копировать или распространять отдельные
модельные файлы. Для lexicon entries, feature templates и CRF weights в
проверенном upstream evidence не найден отдельный, однозначно связанный
provenance/license notice на каждый bundled asset. Поэтому inventory намеренно
ставит `redistributable: false` и `rights_status: blocked` для model assets.
Модельные байты в репозиторий не добавлялись.

## Первоисточники и упаковка

- Source repository: `https://github.com/grobidOrg/grobid`
- Pinned release tag: `0.9.0` (CRF distribution identity: `0.9.0-crf`)
- License evidence: upstream `LICENSE` at
  `https://raw.githubusercontent.com/grobidOrg/grobid/0.9.0/LICENSE`
- Packaging evidence: upstream `Dockerfile.crf` in tag `0.9.0`
- Resource-root evidence: GROBID configuration documentation states that
  `grobid-home/` contains runtime resources including models, lexicons and
  native libraries: `https://grobid.readthedocs.io/en/latest/Configuration/`

В репозитории проекта license evidence path — этот документ:
`doc/parse/grobid-model-rights.md`. Машинный inventory находится в
`data/parse/grobid_model_rights.yaml`.

## Инвентарь активов, нужных волне

| Asset | Upstream location | Нужен сейчас | Повторное использование | Redistribution |
|---|---|---:|---|---|
| Lexicon resources | `grobid-home/lexicon/` | Да, как источник ABI/каналов | Только за границей Java service | Blocked |
| CRF feature templates | `grobid-home/models/*/features*` | Да, как будущий ABI input | Только за границей Java service | Blocked |
| CRF model weights | `grobid-home/models/` | Нет, позже | Только внешний Java service | Blocked |
| pdfalto runtime | `grobid-home/pdfalto` | Нет, отдельный компонент | Не покрыт этим решением | Separately blocked |

Пути выше — upstream-relative, без локальных абсолютных путей. Ожидаемая
упаковка — ресурсы внутри audited CRF container/distribution, а не новый
репозиторный sidecar. До отдельной проверки нельзя извлекать их в
`data/parse/`, коммитить веса или превращать sidecar output в graph truth.

## Границы решения

Разрешено:

- использовать upstream GROBID 0.9.0 CRF через внешний HTTP boundary;
- исследовать формат и ABI в аудированной копии distribution без публикации
  model bytes;
- продолжить T02/T03 только с явно описанным fail-closed правовым статусом.

Не разрешено:

- распространять bundled lexicons, feature templates или weights на основании
  одной только Apache-лицензии репозитория;
- добавлять fake/JSONL/SQLite/redb authority или обходить Java route;
- писать graph, менять `import_eligible`, auto-promote или считать sidecar
  TEI/markdown graph truth.

Если будущая задача требует публикации model assets, сначала нужен отдельный
review каждого файла/набора: upstream provenance, applicable SPDX, notice/source
обязанности и reproducible checksum. При отсутствии любого доказательства
результат должен быть typed blocked/fail-closed, а не silent default.

## ADR-051 применимость

Этот T01 — feasibility/rights inventory, не runtime parser и не quality claim.
Он даёт artifact-level evidence для последующих Contract/Integration тестов;
Operational и UAT метрики модели здесь не измеряются. Java remains production
authority, native work remains candidate/shadow-only, graph remains frozen.
