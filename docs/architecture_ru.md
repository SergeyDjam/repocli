# Архитектура

## Назначение

`repocli` генерирует метаданные репозитория urpmi из каталога RPM-пакетов. Это
Rust-реализация `genhdlist2`, которая намеренно **не вызывает** ни эталонную
утилиту, ни `librpm`, ни системный `rpm`.

## Поток данных

```
Каталог с RPM
    │
    ├─ find_rpms()            обход каталога, фильтр *.rpm
    ├─ old-rpms.lst           исключение пакетов, помеченных на удаление
    │
    ▼
RpmReaderRegistry::read()  ── для каждого пакета ──────────┐
    │                                                       │
    ├─ detect()          lead + цепочка заголовков          │
    │                    RPMTAG_RPMVERSION (тег 1064)       │
    │                    → Rpm3 | Rpm4 | Rpm5 | Rpm6        │
    │                                                       │
    └─ плагин RpmReader → universal::read_mapped()          │
                           метаданные + raw_header          │
                           одно открытие + mmap на файл     │
                                                             │
    ┌────────────────────────────────────────────────────────┘
    ▼
RpmInfo[] (имя, версия, release, arch, epoch, size, group,
           зависимости, файлы, changelog, raw_header)
    │
    ├─ merge::run() / incremental::run()
    │     ├─ packdrakeng::write_archive() → hdlist.cz
    │     ├─ synthesis::write_synthesis() → synthesis.hdlist.cz (сжатие)
    │     └─ xml_metadata::write_*()      → info/files/changelog.xml (сжатие)
    │
    └─ версионирование, MD5SUM, ведение old-rpms.lst
```

## Ключевые решения

### Детекция по заголовку, а не по lead

Все современные RPM, включая rpm6, несут «замороженный» lead major=3. Поэтому
классификация читает `RPMTAG_RPMVERSION` (тег 1064) из основного заголовка.
lead остаётся в `Detection` только для диагностики. Если тега нет (очень старые
пакеты), используется lead с `Confidence::Probable`.

### Один парсер для всех форматов

Структура заголовка (магия, 16-байтовые записи индекса, big-endian хранилище
данных) и номера используемых тегов одинаковы для rpm3–rpm6. Различия — в
обработке payload (gzip / xz / zstd), которая repocli не читает. Все четыре
плагина — тонкие адаптеры над `rpm/universal.rs`; граница плагина нужна, чтобы
в будущем можно было добавить специфика версии.

### Одно открытие файла

Метаданные и raw header для `hdlist.cz` извлекаются за один проход `mmap`.
Предыдущая реализация открывала каждый RPM второй раз через `rpm-rs`, что давало
огромное `sys`-время.

### Форматы вывода по эталонным источникам

- `hdlist.cz`: контейнер MDV::Packdrakeng 1.14, по `MDV::Packdrakeng.pm`.
- `synthesis.hdlist.cz`: порядок и кодировка полей из `URPM.xs::Pkg_build_info()`.
- XML: структура `genhdlist2::add_pkg()`.
- Зависимости: `URPM.xs::print_list_entry()` (`name[op evr]`, объединение через `@`).

## Карта модулей

| Модуль | Ответственность |
|---|---|
| `main.rs` | CLI (`clap`), оркестрация, таймеры фаз (`REPOCLI_TIMING=1`) |
| `rpm/mod.rs` | trait `RpmReader`, реестр, матричный тест детекции |
| `rpm/detect.rs` | сканирование цепочки заголовков, `RpmFormat`, `Confidence` |
| `rpm/header.rs` | безопасный декодер тегов (STRING, I18NSTRING, STRING_ARRAY, INT32) |
| `rpm/universal.rs` | извлечение метаданных и raw header |
| `rpm/rpm3..rpm6.rs` | плагины форматов |
| `packdrakeng.rs` | writer контейнера hdlist и читатель для тестов |
| `synthesis.rs` | writer synthesis |
| `xml_metadata.rs` | writer'ы XML |
| `incremental.rs`, `merge.rs` | стратегии обновления |
| `old_rpms.rs`, `old_metadata.rs` | списки включения/исключения |
| `compression.rs` | абстракция внешнего фильтра (`suffix:command`) |
| `lock.rs`, `md5sum.rs`, `versioning.rs`, `utils.rs` | обслуживание |

Форматы файлов — в [formats_ru.md](formats_ru.md), правила расширения — в
[development_ru.md](development_ru.md).
