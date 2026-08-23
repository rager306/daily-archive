# sources/ — иммутабельные исходные ассеты

После загрузки файлы не изменяются. Это read-only зона.

## Структура

```
sources/
├── arxiv/<category>/<paper_id>/     ← статьи arXiv
│   ├── identity.json                ← метаданные (id, title, authors, version)
│   ├── article.json                 ← совместимость (dual-name lookup)
│   ├── article.v<N>.json            ← версионированные метаданные
│   └── source/
│       ├── <paper_id>.pdf           ← основной PDF
│       ├── article.html             ← HTML версия (если есть)
│       └── abs.html                 ← страница абстракта
│
├── textbook/<textbook_id>/          ← учебники (по учебнику!)
│   ├── textbook_manifest.json       ← ОБЯЗАТЕЛЬНО: формат + parser profile
│   └── <chapter_or_section>/
│       ├── identity.json
│       └── source/chapter.html | *.pdf
│
├── blog/<blog_id>/<post_id>/        ← блог-посты
├── nature/<journal>/<article_id>/   ← Nature статьи
└── course/<course_id>/<lecture>/    ← учебные материалы
```

## Правила

1. Файлы после записи НЕ изменяются.
2. Категория — атрибут identity.json, не структура пути обработки.
3. Каждый учебник имеет textbook_manifest.json с описанием формата.
4. Service buckets (`mixed-source`, `unknown`) разрешены внутри arxiv/,
   но никогда не являются каноническими при разрешении источника.
5. Дубликаты категорий (одна статья в 2+ папках) запрещены.
