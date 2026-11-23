use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub fn filter_out_old_metadata(
    media_info_dir: &Path,
    old_metadata_file: &Path,
    synthesis_path: &Path,
    xml_files: &[&str],
) -> Result<()> {
    let to_remove = fs::read_to_string(old_metadata_file)
        .ok()
        .map(|s| {
            s.lines()
                .map(|l| l.trim_end_matches(".rpm").to_string())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    if to_remove.is_empty() {
        return Ok(());
    }

    for &name in xml_files {
        let path = media_info_dir.join(format!("{}.xml", name));
        let tmp_path = media_info_dir.join(format!("{}.xml.tmp", name));
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        let mut reader = Reader::from_str(&content);
        reader.config_mut().trim_text(true);
        let mut writer = Writer::new(Vec::new());
        let mut in_entry = false;
        let mut entry_fn = String::new();
        let mut skip_entry = false;
        loop {
            match reader.read_event()? {
                Event::Eof => break,
                Event::Start(e) => {
                    let tag = std::str::from_utf8(e.name().into_inner()).unwrap_or("");
                    if tag == name {
                        in_entry = true;
                        entry_fn.clear();
                        for attr in e.attributes() {
                            let attr = attr?;
                            if attr.key == QName(b"fn") {
                                entry_fn = String::from_utf8(attr.value.into_owned())?;
                                break;
                            }
                        }
                        skip_entry = to_remove.contains(&entry_fn);
                        if !skip_entry {
                            writer.write_event(Event::Start(e))?;
                        }
                    } else {
                        writer.write_event(Event::Start(e))?;
                    }
                }
                Event::End(e) => {
                    let tag = std::str::from_utf8(e.name().into_inner()).unwrap_or("");
                    if tag == name && in_entry {
                        in_entry = false;
                        if !skip_entry {
                            writer.write_event(Event::End(e))?;
                        }
                    } else {
                        writer.write_event(Event::End(e))?;
                    }
                }
                Event::Empty(e) => writer.write_event(Event::Empty(e))?,
                Event::Text(e) if !skip_entry => writer.write_event(Event::Text(e))?,
                _ => {}
            }
        }
        let result = writer.into_inner();
        fs::write(&tmp_path, result)?;
        fs::rename(&tmp_path, &path)?;
    }

    let tmp_synthesis = synthesis_path.with_extension("hdlist.tmp");
    let content = fs::read_to_string(synthesis_path)?;
    let mut out = String::new();
    let mut lines = content.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with("@info@") {
            let fullname = line.trim_start_matches("@info@").trim_end_matches('@');
            let pkg_name = fullname.rsplit_once('-').map(|x| x.0).unwrap_or(fullname);
            let skip = to_remove.contains(pkg_name);
            if !skip {
                out.push_str(line);
                out.push('\n');
                for l in lines.by_ref() {
                    out.push_str(l);
                    out.push('\n');
                    if l == "@@" {
                        break;
                    }
                }
            } else {
                for l in lines.by_ref() {
                    if l == "@@" {
                        break;
                    }
                }
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    fs::write(&tmp_synthesis, out)?;
    fs::rename(&tmp_synthesis, synthesis_path)?;
    Ok(())
}
