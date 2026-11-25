# repocli

Rust-замена `genhdlist2` — утилиты, которая генерирует метаданные репозитория
(`hdlist.cz`, `synthesis.hdlist.cz`, XML) для дистрибутивов на базе urpmi
(ROSA, ALT Linux, Mageia).

Пакеты читаются напрямую: заголовки RPM разбираются на чистом Rust без вызовов
`genhdlist2`, `librpm` и системной утилиты `rpm`.

## Возможности

- **Поддержка rpm3, rpm4, **rpm5** (основная цель) и rpm6** через реестр плагинов.
  Формат определяется по `RPMTAG_RPMVERSION` (тег 1064), а не по «замороженному»
  lead major=3, который есть почти у всех современных RPM.
- **`hdlist.cz`** пишется в контейнере MDV::Packdrakeng 1.14: gzip-блок с
  конкатенированными заголовками RPM, TOC и трейлер `cz[0 … 0]cz`.
  Проверено: файл читается эталонным `urpmq`.
- **`synthesis.hdlist.cz`** — порядок и кодировка полей соответствуют
  `URPM.xs::Pkg_build_info()` из URPM 4.62 (`@provides@`, `@requires@`,
  `@filesize@`, `@info@` и др.).
- **XML-метаданные** `info.xml`, `files.xml`, `changelog.xml` — та же структура,
  что у эталонной реализации.
- **Инкрементальное обновление:** существующий `hdlist.cz` переиспользуется
  через `old-rpms.lst` / `old-metadata.lst`; режим `--merge` объединяет новые и
  существующие метаданные и убирает дубликаты.
- **Обслуживание репозитория:** `MD5SUM`, версионированные симлинки, глобальная
  блокировка `UPDATING`, обработка ошибок `--no-bad-rpm`.
- **Производительность:** ~550 пакет/с на x86_64; экстраполяция на 5000 пакетов —
  около 9 с (цель ≤15 с).

## Сборка и тесты

```bash
cargo fmt --all
cargo build --release
cargo test --release
```

Поддерживается кросс-компиляция в CI: тест запуска `--help` пропускается, если
хост и целевая платформа различаются; проверка наличия бинарника выполняется
всегда.

## Использование

```bash
# Генерация метаданных репозитория
repocli /path/to/rpms

# Объединение новых метаданных с существующими
repocli --merge /path/to/rpms

# Пересборка с нуля без XML и MD5SUM
repocli --clean --xml-info false --no-md5sum /path/to/rpms
```

Основные опции:

| Опция | Назначение |
|---|---|
| `--clean` | Не использовать существующий hdlist, собрать заново |
| `--merge` | Объединить новые метаданные с существующими |
| `--no-bad-rpm` | Пропускать нечитаемые пакеты вместо аварийного выхода |
| `--no-md5sum` | Не создавать `MD5SUM` |
| `--no-hdlist` | Не создавать `hdlist.cz` |
| `--allow-empty-media` | Разрешить каталог без `.rpm`-файлов |
| `--file-deps <file>` | Пометить file-provides, необходимые другим медиа |
| `--synthesis-filter <suffix:cmd>` | Фильтр сжатия synthesis (по умолчанию `.cz:xz -1`) |
| `--xml-info <auto\|true\|false>` | Режим генерации XML (по умолчанию `auto`) |
| `--xml-info-filter <suffix:cmd>` | Фильтр сжатия XML (по умолчанию `.lzma:xz -1`) |
| `--versioned <auto\|true\|false>` | Версионированные симлинки (по умолчанию `auto`) |
| `--media_info-dir <dir>` | Каталог вывода (по умолчанию `<rpms>/media_info`) |
| `-q`, `-v` | Тихий / подробный режим (повторяемо) |

## Структура проекта

```
src/
  main.rs              CLI и оркестрация
  rpm/
    mod.rs             trait RpmReader и реестр плагинов
    detect.rs          детекция формата по RPMTAG_RPMVERSION
    header.rs          безопасный декодер заголовка по тегам
    universal.rs       общий reader метаданных и raw_header
    rpm3.rs … rpm6.rs  плагины форматов (тонкие адаптеры)
  packdrakeng.rs       writer/читатель контейнера hdlist.cz
  synthesis.rs         writer synthesis.hdlist
  xml_metadata.rs      writer'ы info/files/changelog XML
  incremental.rs, merge.rs, old_rpms.rs, old_metadata.rs,
  compression.rs, hdlist.rs, lock.rs, md5sum.rs,
  utils.rs, versioning.rs
tests/
  fixtures/            реальные RPM-пакеты (rpm4/rpm5/rpm6)
  integration.rs       smoke-тесты CLI
```

## Эталонные источники

Реализация следует оригинальным исходникам ROSA/urpmi, а не экспериментальному
Go-прототипу:

- `URPM-4.62/URPM.xs` — `open_archive()`, `Pkg_build_info()`, `pack_list()`,
  `print_list_entry()`, `pack_header()`, `build_header()`
- `urpmi-7.32.3` — эталонный вызов genhdlist2 и формат XML
- `urpm-tools-2.3.5`, spec- и patch-файлы в родительском каталоге
- формат контейнера MDV::Packdrakeng 1.14

## Документация

- [Архитектура](docs/architecture_ru.md) / [Architecture](docs/architecture.md)
- [Форматы файлов](docs/formats_ru.md) / [On-disk formats](docs/formats.md)
- [Руководство разработчика](docs/development_ru.md) / [Development guide](docs/development.md)
- [Руководство по эксплуатации](docs/operations_ru.md) / [Operations guide](docs/operations.md)
- Уровень проекта (родительский каталог): `TZ_repocli-migration.md`, `CHANGELOG.md`,
  `SUMMARY_genhdlist2-migration.md`, `SUMMARY_repocli-work.md`

## Лицензия

GPLv3
