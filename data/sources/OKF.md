# sources/ — иммутабельные исходные ассеты

```
sources/
├── arxiv/<category>/<paper_id>/
│   ├── identity.json             ← метаданные статьи
│   ├── article.v<N>.json         ← версионированные метаданные
│   └── source/<paper_id>.pdf     ← PDF + HTML
│
├── textbook/<textbook_id>/       ← учебники (по учебнику)
│   ├── textbook_manifest.json    ← ОБЯЗАТЕЛЬНО: формат + parser profile
│   └── <chapter>/<section>/      ← главы/разделы
│       ├── identity.json
│       └── source/chapter.html | *.pdf
│
├── blog/<blog_id>/<post_id>/     ← блог-посты
├── nature/<journal>/<id>/        ← Nature
└── course/<course_id>/<lecture>/ ← курсы

Служебные категории: mixed-source, unknown (не канонические).
Дубликаты категорий запрещены.
