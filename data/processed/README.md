# processed/ — обработанные статьи по стадиям конвейера

Каждая статья = одна папка `<paper_id>/`. Пересчитываемо из sources/.

## Структура одной статьи

```
<paper_id>/
├── manifest.json                  ← паспорт: статус каждой стадии
├── parsed/article.json            ← ParsedArticle (полный текст)
├── context/projection.json        ← DocumentContextProjection AST
└── extracted/entities.json        ← ExtractedEntity[]
```

## manifest.json — паспорт статьи

Отслеживает каждую стадию: статус, timestamp, sha256 входа/выхода,
конфигурацию (словарь, fingerprint), счётчики (sections, entities).

## Debug

Для отладки конкретной статьи смотри manifest.json — там видно
на какой стадии что произошло. В будущем: `governor debug <paper_id>`.

## Не редактировать вручную

Пересчитывается из sources/ скриптом `scripts/m450_build_processed.py`.
