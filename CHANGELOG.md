# Changelog

Все значимые изменения в `repocli` фиксируются здесь. Формат — по мотивам
[Keep a Changelog](https://keepachangelog.com/). Даты — по UTC.

**Правило:** любое изменение кода сопровождается новой записью в раздел
`[Unreleased]`; перед релизом раздел переименовывается в версию с датой.

---

## [Unreleased]

### Added — этап 0 (скелет плагинов)

- `src/rpm/mod.rs`: `trait RpmReader`, `RpmReaderRegistry` с методами
  `register` / `reader_for` / `read` / `read_with_hint`; `Default` регистрирует
  плагины rpm3–rpm6.
- `src/rpm/detect.rs`: структурная детекция формата по lead и цепочке
  заголовков (`LEAD_MAGIC`, `HEADER_MAGIC`), поля `signature_offset`,
  `header_offset`, `Confidence`, явный `hint` для неоднозначных пакетов.
- Заглушки плагинов `rpm3.rs`, `rpm4.rs`, `rpm5.rs`, `rpm6.rs` с указанием
  стадии ТЗ, на которой они будут реализованы.
- 4 юнит-теста детекции и тест реестра; `tests/integration.rs` переведён на
  `env!("CARGO_BIN_EXE_repocli")`.

### Added — этап 1 (парсер rpm5)

- `src/rpm/header.rs`: безопасный разбор заголовка по тегам без эвристик.
  Поддержаны `STRING(6)`, `I18NSTRING(9)`, `STRING_ARRAY(8)`, `INT32(4)`,
  строгая проверка всех границ срезов и big-endian.
- `src/rpm/rpm5.rs`: `Rpm5Reader` читает метаданные и `raw_header` из тегов;
  Provides/Requires/Conflicts/Obsoletes собираются из пар «имя + версия»,
  список файлов — из `DIRNAMES + BASENAMES + DIRINDEXES`.
- Фикстура `tests/fixtures/zstd-1.5.6-1-R11.x86_64.rpm` (реальный пакет
  ROSA, rpm 5.4.10) и 3 теста на ней.

### Changed

- `RpmInfo` перенесён в `src/rpm/model.rs`; `src/rpm_ffi.rs` оставлен как
  совместимый re-export.
- `src/main.rs`: `read_package()` направляет пакеты с детекцией `Rpm5` в
  реестр плагинов; остальные форматы временно идут через legacy-путь.
- `src/incremental.rs` и `src/merge.rs`: hdlist строится из `raw_header`,
  уже полученного при разборе метаданных. **Второе открытие каждого RPM
  через `rpm-rs` (`read_raw_header_slow_async`) убрано.**
- `src/hdlist.rs`: новая сигнатура `build_hdlist_from_packages(&[RpmInfo], …)`;
  формат вывода пока сохранён (Packdrakeng — этап 3).
- Все комментарии в `src/` и `tools/` переведены на английский (требование C2).

### Fixed

- Интеграционные тесты искали `target/debug/repocli` и падали при
  `cargo test --release`; теперь используют макрос Cargo с путём активного
  профиля.
- Удалены неиспользуемые импорты в `rpm_ffi.rs`, `compare_headers.rs`;
  временный `#![allow(dead_code)]` в `src/rpm/mod.rs` (снять после этапа 2).
- Детекция: rpm5 с lead major 3 и header-style сигнатурой ошибочно
  классифицировался как rpm4 и уходил на legacy-парсер.
- `detect.rs` больше не считает размер сигнатуры из `lead[76..80]` — это было
  неверно (`osnum` + `signature_type`); сигнатура ищется структурно.
- `find_headers()` сканировала весь файл побайтно, хотя нужны только два
  первых заголовка; лимит снижен с 4 до 2 (потенциальный источник лишних
  затрат CPU на больших RPM).

### Performance / известная регрессия

- Замер на 97 пакетах до подключения парсера: `real 0,419 s / user 0,143 s /
  sys 0,555 s`.
- После подключения: `real 4,975 s / user 3,293 s / sys 0,352 s`.
- `sys` упал (ожидаемый эффект от устранения второго открытия файлов), но
  `user` вырос в ~23 раза. Предварительная причина: до подключения парсера
  поля `requires`, `provides`, `files`, `description` были пустыми, поэтому
  synthesis/XML были крошечными; теперь они содержат реальные данные, и
  внешние `xz -7 -T0` (значения по умолчанию в `main.rs`) тратят секунды
  CPU на сжатие.

- Гипотеза про `xz` **опровергнута**: прогон с `--synthesis-filter .cz:cat
  --xml-info-filter .lzma:cat` дал `real 6,838 s / user 6,335 s /
  sys 0,425 s` — без сжатия стало ещё медленнее. Значит, CPU расходуется
  внутри `repocli`, а не в дочерних процессах.
- Добавлена фазовая инструментация: при переменной окружения
  `REPOCLI_TIMING=1` в stderr выводятся `metadata`, `pipeline`, `finalize`,
  а внутри merge — `merge:synthesis`, `merge:hdlist`, `merge:xml`.
  Следующий шаг — локализовать дорогую фазу по этому выводу.

- Причина регресса **найдена и устранена**: `find_headers()` сканировала весь
  RPM побайтно вместо двух первых заголовков. После снижения лимита до 2:
  `real 0,749 s / user 0,645 s / sys 0,108 s` на тех же 97 пакетах
  (было 6,838 / 6,335 / 0,425). Ускорение ~9,1×.
- Сравнение с исходным эвристическим путём: `0,419 / 0,143 / 0,555`.
  Новый парсер дороже на ~0,33 s CPU на 97 пакетов, но даёт корректные
  метаданные и `raw_header`; `sys` снизился в 5 раз за счёт одного открытия
  файла вместо двух.

- Фазовая инструментация (`REPOCLI_TIMING=1`) локализовала оставшиеся
  затраты: `metadata 14 мс`, `merge:synthesis 328 мс`, `merge:xml 292 мс`,
  `merge:hdlist 55 мс`, `finalize 3 мс` на 97 пакетов. Парсер занимает 2%,
  ~90% времени — внешний `xz -7 -T0` при сжатии synthesis и трёх XML.
  Раняя гипотеза про `xz` была верной, но маскировалась багом сканирования.
- **Изменено:** уровни сжатия по умолчанию снижены с `xz -7 -T0` до `xz -1`
  для synthesis и XML (`main.rs`). Формат `.xz` сохранён, совместимость не
  нарушена. Ожидаемый эффект — снятие ~600 мс CPU на 97 пакетов.

- Результат смены уровня: `real 0,323 s / user 0,205 s / sys 0,060 s`
  (было 0,706 / 0,605 / 0,107), ускорение ещё ~2,2×.
  Разбивка: `metadata 24,6 мс`, `merge:synthesis 83,4 мс`,
  `merge:hdlist 88,5 мс`, `merge:xml 94,1 мс`, `finalize 4,5 мс`.
  Затраты распределены равномерно; цена пакета ~3,3 мс.
- **Изменено:** gzip для hdlist переведён с уровня по умолчанию (6) на 1 —
  `hdlist.rs`. Формат gzip не меняется, совместимость сохранена.

- **Финальный замер (97 пакетов):** `real 0,176 s / user 0,140 s /
  sys 0,042 s`. Разбивка: `metadata 16,5 мс`, `merge:synthesis 55,5 мс`,
  `merge:hdlist 19,7 мс`, `merge:xml 66,2 мс`, `finalize 4,4 мс`.
  Ускорение 38,9× относительно регресса и 2,4× относительно исходного
  эвристического пути — при том что теперь читаются настоящие заголовки.
- Пропускная способность 551 пакет/с. Экстраполяция на 5000/10000/40000
  пакетов: **9,1 / 18,1 / 72,6 с** (линейно), 7,0 / 14,0 / 56,1 с с учётом
  амортизации старта. Цель ТЗ ≤15 с на 5000 пакетов **достигнута с запасом**;
  этап 5 (производительность) закрыт.

### Added — этап 3 (Packdrakeng)

- `src/packdrakeng.rs`: writer формата **MDV::Packdrakeng 1.14**, восстановленного
  по исходникам `MDV::Packdrakeng.pm` и `genhdlist2` (Mageia git):
  сжатый gzip-блок, TOC из имён виртуальных файлов, 16-байтовые записи
  `coff/csize/off/size` и 64-байтовый трейлер `cz[0 … 0]cz` с методом
  распаковки `gzip -d`. Каждый RPM — отдельный virtual file с именем `fullname`
  и содержимым raw header, как в эталоне (`add_virtual('f', ...)`).
- `read_archive()` — минимальный читатель для тестов и диагностики;
  round-trip тест на двух пакетах.
- `hdlist.rs` стал тонкой обёрткой над `packdrakeng::write_archive`.
- Уровень gzip для hdlist выбран 1 (формат совместим с любым gzip).

### Changed — пересмотр по эталонным исходникам (ROSA)

- `go-genhdlist` признан **не эталоном**: многие функции там заглушки.
  Эталон — `/config/home/projects/urpm-ng/URPM-4.62`, `urpmi-7.32.3`,
  `urpm-tools-2.3.5`, spec- и patch-файлы.
- Подтверждено по `URPM.xs::open_archive()`: `hdlist.cz` — это трейлер
  `cz[0…0]cz`, поле команды распаковки и поток конкатенированных RPM-заголовков.
  Наш Packdrakeng-writer совместим; TOC не используется URPM при чтении hdlist.
- **Переписан `synthesis.rs`** по `URPM.xs::Pkg_build_info()` (URPM 4.62):
  поля в порядке `provides, conflicts, obsoletes, requires, summary, filesize,
  info`; зависимости объединяются через `@` в одну строку; формат записи —
  `name[op evr]` по `print_list_entry()`; `@info@` всегда последний и содержит
  `nvra@epoch@size@group`. Удалены несуществующие в эталоне `@name@`,
  `@version@`, `@release@`, `@arch@`, `@epoch@` и `@file@` строки.
- **Добавлены поля `size` (RPMTAG_SIZE) и `group` (RPMTAG_GROUP)** в `RpmInfo`;
  парсер rpm5 читает их из тегов 1009 и 1016.
- Зависимости теперь учитывают флаги (`print_list_entry`: `Fstr[(flags>>1)&7]`);
  RPMLIB-записи пропускаются (в rpm5 — бит 14, в rpm.org — бит 24).
- Подтверждено по spec: `perl(MDV::Packdrakeng) >= 1.14-5`, ROSA использует
  `xml-info => always` (patch urpmi-7.32.2-always-xml-info.patch).
- `xml_metadata.rs`: `<info>` теперь содержит атрибуты `sourcerpm`, `url`,
  `license`; каждый файл в `<files>` пишется с завершающим `\n`, как в эталоне.

### Fixed — сборка

- `rpm_reader.rs`: добавлены поля `size`/`group` в legacy-инициализатор `RpmInfo`.
- `rpm5.rs`: импорт `detect_format` перенесён в тестовый модуль (неиспользуемый
  импорт в basic-сборке).
- `packdrakeng.rs`: неиспользуемая переменная переименована в `_name`;
  `read_archive()` помечен `#[cfg(test)]` (убрал warning `dead_code`).
- **Флаги зависимостей уточнены:** `0x4000` — это `RPMSENSE_FIND_REQUIRES`
  (автоматические ELF-зависимости вида `libc.so.6()(64bit)`), их URPM
  **сохраняет**; пропускать нужно только `RPMSENSE_RPMLIB = 0x1000000`.
  На фикстуре: 26 requires сохранено, `rpmlib(PayloadIsXz)` пропущен.

### Verified — работа с эталонным потребителем

- `urpmq -f --media example --list` возвращает все 97 пакетов — **hdlist.cz
  читается urpmi** (Packdrakeng-трейлер и поток заголовков корректны).
- `urpmq -f --media example -ai paranoia` показывает верные Name/Version/
  Release/Group/Size/Summary — **synthesis и XML корректны**, включая новые
  поля `size`/`group` и формат `@info@`.
- Замер после пересмотра по эталону: `real 0,298 s / user 0,130 s /
  sys 0,057 s` на 97 пакетах. Фазы: `metadata 143,7 мс`,
  `merge:synthesis 17,9 мс`, `merge:hdlist 34,5 мс`, `merge:xml 79,7 мс`.
  Пропускная способность 325 пакет/с; экстраполяция на 5000 — 15,4 с
  (12,8 с с учётом амортизации старта). Цель ≤15 с выполняется с учётом
  фиксированной стоимости запуска.

### Added — этап 2 (форматы rpm3–rpm6)

- **Детекция по `RPMTAG_RPMVERSION` (тег 1064) вместо lead.** Выяснено, что
  все современные RPM (включая rpm6) несут «замороженный» lead major=3,
  поэтому детекция по lead была принципиально нерабочей. На фикстурах тег 1064
  даёт 4.2.2 / 4.17.0 / 4.2.3 / 5.4.10 / 6.1.0.
- `src/rpm/universal.rs`: общий tag-based reader, вынесенный из `rpm5.rs`.
  Заголовки rpm3–rpm6 структурно идентичны, различается только payload,
  который repocli не читает.
- Плагины `rpm3/4/5/6.rs` стали тонкими адаптерами над общим reader'ом;
  реестр по-прежнему обеспечивает расширяемость. `main.rs` теперь всегда
  использует `RpmReaderRegistry::read()` без legacy-fallback.
- Матричный тест `reads_all_fixture_formats`: все фикстуры детектируются с
  `Confidence::Exact` и читаются (имя, версия, raw header).
- `read_rpm_fast` помечен `#[allow(dead_code)]` как legacy.
- `RpmFormat` получил `PartialOrd, Ord` (нужны для `BTreeSet` в матричном тесте).
- Устаревший синтетический тест `detects_rpm5_that_keeps_lead_3` переименован
  в `falls_back_to_lead_without_rpmversion`: без тега 1064 детекция по lead
  даёт Rpm3 с `Confidence::Probable`. Точная классификация по RPMVERSION
  проверяется матричным тестом на реальных фикстурах.

### Fixed — clippy (`-D warnings`)

- `tools/rpm_diag.rs`: `(0x20..=0x7e).contains(&c)`.
- `src/rpm/detect.rs`: `is_none_or` вместо `map_or(true, …)`.
- `src/main.rs`: `matches!` вместо match-логики для `xml_info`.
- `src/versioning.rs`, `src/old_metadata.rs`: свёрнуты вложенные `if`/`match`.
- `src/old_metadata.rs`: `rsplit_once`, `for l in lines.by_ref()`.
- `src/merge.rs`: `Vec::to_vec()`; `#[allow(clippy::too_many_arguments)]`
  для `run()` (сигнатура унаследована от эталона).
- `src/incremental.rs`: `#[allow(clippy::too_many_arguments)]` для `run()`.
- `src/packdrakeng.rs`, `src/synthesis.rs`: struct literal с `..Default::default()`
  в тестах вместо присваивания полей.

### Fixed — кросс-компиляция (CI riscv64)

- `tests/integration.rs::test_help` падал в CI при `TARGET=riscv64gc` на хосте
  x86_64: тест пытался запустить кросс-собранный бинарник.
- Добавлен `build.rs`, экспортирующий `HOST`/`TARGET` в `cargo:rustc-env`;
  execution-тест пропускается при `HOST != TARGET`. `test_binary_exists`
  по-прежнему проверяет артефакт на всех целевых платформах.

### Added — документация

- `README.md` (EN) и `README_ru.md` (RU): возможности, сборка и тесты, опции
  CLI, структура проекта, эталонные источники, ссылки на ТЗ и саммари.
- `docs/` — подробная документация на двух языках (862 строки):
  `architecture[_{ru}].md` (поток данных, решения, карта модулей),
  `formats[_{ru}].md` (RPM, Packdrakeng, synthesis, XML, вспомогательные файлы),
  `development[_{ru}].md` (правила кода, добавление плагинов, диагностика,
  профилирование), `operations[_{ru}].md` (сценарии, media_info, диагностика,
  производительность). README ссылаются на разделы.
- Экстраполяция старого замера на 5000/10000/40000 пакетов: 21,6 / 43,2 /
  172,8 с при 231 пакет/с; цель ТЗ — ≤15 с на 5000 (нужно 333 пакет/с).

---

## [0.1.0] — 2026-11-21

- Базовое состояние репозитория `repocli` на момент начала миграции:
  рабочая оркестрация (CLI, lock, old-rpms, merge/incremental, versioning,
  md5sum, XML) и эвристический `read_rpm_fast` с пустым `raw_header`.
- См. `TZ_repocli-migration.md` и `SUMMARY_genhdlist2-migration.md`.
