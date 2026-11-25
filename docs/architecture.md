# Architecture

## Purpose

`repocli` generates urpmi repository metadata from a directory of RPM packages.
It is a Rust reimplementation of `genhdlist2` and deliberately does **not** call
the reference tool, `librpm`, or the system `rpm` binary.

## Data flow

```
RPM directory
    │
    ├─ find_rpms()            walkdir, *.rpm filter
    ├─ old-rpms.lst           drop packages scheduled for removal
    │
    ▼
RpmReaderRegistry::read()  ── per package ────────────────┐
    │                                                      │
    ├─ detect()          RPM lead + header chain           │
    │                    RPMTAG_RPMVERSION (tag 1064)      │
    │                    → Rpm3 | Rpm4 | Rpm5 | Rpm6       │
    │                                                      │
    └─ RpmReader plugin → universal::read_mapped()         │
                           metadata + raw_header           │
                           single open + mmap per file     │
                                                            │
    ┌───────────────────────────────────────────────────────┘
    ▼
RpmInfo[] (name, version, release, arch, epoch, size, group,
           dependencies, files, changelog, raw_header)
    │
    ├─ merge::run() / incremental::run()
    │     ├─ packdrakeng::write_archive() → hdlist.cz
    │     ├─ synthesis::write_synthesis() → synthesis.hdlist.cz (compressed)
    │     └─ xml_metadata::write_*()      → info/files/changelog.xml (compressed)
    │
    └─ versioning + md5sum + old-rpms.lst bookkeeping
```

## Key design decisions

### Detection from the header, not the lead

Every modern RPM — including rpm6 — carries a frozen `3.0` lead. Classification
therefore reads `RPMTAG_RPMVERSION` (tag 1064) from the main header. The lead is
kept in `Detection` only as diagnostic data. If the tag is absent (very old
packages), classification falls back to the lead with `Confidence::Probable`.

### One parser for all formats

The header layout (magic, 16-byte index entries, big-endian data store) and the
tag numbers used by repocli are identical across rpm3–rpm6. Differences are in
payload handling (gzip / xz / zstd), which repocli never needs. All four plugins
are thin adapters over `rpm/universal.rs`; the plugin boundary exists so that
future version-specific behaviour has a home.

### Single open per package

Metadata and the raw header used for `hdlist.cz` are extracted in the same
`mmap` pass. The previous implementation re-opened every RPM through `rpm-rs`
a second time, which produced large `sys` time.

### Output formats follow authoritative sources

- `hdlist.cz`: MDV::Packdrakeng 1.14 container, per `MDV::Packdrakeng.pm`.
- `synthesis.hdlist.cz`: `URPM.xs::Pkg_build_info()` field order and encoding.
- XML: `genhdlist2::add_pkg()` structure.
- Dependency encoding: `URPM.xs::print_list_entry()` (`name[op evr]`, joined by `@`).

## Module map

| Module | Responsibility |
|---|---|
| `main.rs` | CLI (`clap`), orchestration, phase timing (`REPOCLI_TIMING=1`) |
| `rpm/mod.rs` | `RpmReader` trait, registry, detection test matrix |
| `rpm/detect.rs` | header chain scan, `RpmFormat`, `Confidence` |
| `rpm/header.rs` | safe tag decoder (STRING, I18NSTRING, STRING_ARRAY, INT32) |
| `rpm/universal.rs` | metadata and raw header extraction |
| `rpm/rpm3..rpm6.rs` | format plugins |
| `packdrakeng.rs` | hdlist container writer and test reader |
| `synthesis.rs` | synthesis writer |
| `xml_metadata.rs` | XML writers |
| `incremental.rs`, `merge.rs` | update strategies |
| `old_rpms.rs`, `old_metadata.rs` | inclusion/exclusion lists |
| `compression.rs` | external filter abstraction (`suffix:command`) |
| `lock.rs`, `md5sum.rs`, `versioning.rs`, `utils.rs` | housekeeping |

For on-disk layouts, see [formats.md](formats.md). For extension rules, see
[development.md](development.md).
