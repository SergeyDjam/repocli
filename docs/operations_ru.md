# Руководство по эксплуатации

## Типичные сценарии

### Сборка медиа с нуля

```bash
repocli --clean /srv/repo/media/main/release
```

В каталоге `media_info/` создаются:

- `hdlist.cz`
- `synthesis.hdlist.cz`
- `info.xml.lzma`, `files.xml.lzma`, `changelog.xml.lzma`
- `MD5SUM`
- версионированные симлинки (`YYYYMMDD-HHMMSS-<name>`), если предыдущие симлинки уже были

### Добавление новых пакетов в существующее медиа

```bash
repocli --merge /srv/repo/media/main/release
```

Утилита читает существующие `synthesis.hdlist.cz` и XML, удаляет устаревшие
записи из `old-metadata.lst` и добавляет только новые пакеты. Заголовки
неизменившихся RPM переиспользуются — это самый быстрый путь для регулярных
обновлений.

### Инкрементальное обновление

```bash
repocli /srv/repo/media/main/release
```

`old-rpms.lst` планирует удаление пакетов, которых больше нет на диске.
`--no-clean-old-rpms` сохраняет файлы RPM, но обновляет метаданные;
`--only-clean-old-rpms` выполняет только очистку.

### Пропуск повреждённых пакетов

```bash
repocli --no-bad-rpm /srv/repo/media/main/release
```

Нечитаемые пакеты сообщаются в stderr и пропускаются, а не прерывают весь запуск.

### Нестандартное сжатие

```bash
repocli --synthesis-filter '.cz:gzip -1' \
        --xml-info-filter '.lzma:xz -1' \
        /srv/repo/media/main/release
```

Формат фильтра — `suffix:command`. Пустая команда отключает сжатие.
По умолчанию: `.cz:xz -1` и `.lzma:xz -1`.

## Каталог media_info

| Файл | Назначение |
|---|---|
| `hdlist.cz` | контейнер Packdrakeng с сырыми заголовками RPM |
| `synthesis.hdlist.cz` | текстовый индекс зависимостей и описаний (сжатый) |
| `info.xml.lzma` | описания и URL источников |
| `files.xml.lzma` | списки файлов |
| `changelog.xml.lzma` | changelog'и |
| `MD5SUM` | контрольные суммы опубликованных файлов |
| `old-rpms.lst` | расписания `[Remove]` / `[Keep-in-hdlist]` |
| `old-metadata.lst` | устаревшие записи для режима merge |
| `UPDATING` | файл блокировки; удаляется при выходе |
| `*.tmp` | временные файлы, убираются обработчиком сигналов |

Версионированные симлинки создаются только если уже existed предыдущие или
передан `--versioned true`.

## Коды возврата

| Код | Значение |
|---:|---|
| 0 | Успех |
| 1 | Фатальная ошибка (нечитаемый RPM без `--no-bad-rpm`, ошибка блокировки, I/O) |

## Диагностика

| Симптом | Что проверить |
|---|---|
| `no *.rpm found` | Добавьте `--allow-empty-media` или укажите верный каталог |
| `another repocli already running` | Удалите устаревший `media_info/UPDATING` (только если процесс не активен) |
| `bad rpm <path>` | `cargo run --bin rpm_diag -- <path>`; `--no-bad-rpm` для продолжения |
| urpmi не видит пакеты | Сравните `hdlist.cz` с эталонным; проверьте трейлер (`cz[0` … `0]cz`) |
| Нет Version/Group/Size в `urpmq -ai` | Проверьте `synthesis.hdlist.cz`: `@info@` должен быть последним и содержать `nvra@epoch@size@group` |
| Медленная работа | `REPOCLI_TIMING=1` для поиска фазы; см. [development_ru.md](development_ru.md) |

## Ожидаемая производительность

На x86_64 с 97 пакетами: примерно 0,18–0,30 с в зависимости от фильтра сжатия;
325–550 пакет/с. Экстраполяция на 5000 пакетов — 9–15 с. `sys`-время должно быть
небольшим; большой `sys` указывает на регресс в работе с файлами.
