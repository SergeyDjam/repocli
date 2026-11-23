use crate::rpm_ffi::RpmInfo;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::writer::Writer;
use std::io::Write;

pub fn write_info_xml<W: Write>(
    writer: &mut W,
    packages: &[RpmInfo],
) -> Result<(), quick_xml::Error> {
    let mut xml_writer = Writer::new(writer);
    xml_writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
    xml_writer.write_event(Event::Start(BytesStart::new("media_info")))?;

    for pkg in packages {
        let fullname = pkg.fullname();
        let mut elem = BytesStart::new("info");
        elem.push_attribute(("fn", fullname.as_str()));
        elem.push_attribute(("sourcerpm", pkg.sourcerpm.as_str()));
        elem.push_attribute(("url", pkg.url.as_str()));
        elem.push_attribute(("license", pkg.license.as_str()));
        xml_writer.write_event(Event::Start(elem))?;
        let desc = BytesText::new(&pkg.description);
        xml_writer.write_event(Event::Text(desc))?;
        xml_writer.write_event(Event::End(BytesEnd::new("info")))?;
    }
    xml_writer.write_event(Event::End(BytesEnd::new("media_info")))?;
    Ok(())
}

pub fn write_files_xml<W: Write>(
    writer: &mut W,
    packages: &[RpmInfo],
) -> Result<(), quick_xml::Error> {
    let mut xml_writer = Writer::new(writer);
    xml_writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
    xml_writer.write_event(Event::Start(BytesStart::new("media_info")))?;

    for pkg in packages {
        let fullname = pkg.fullname();
        let mut elem = BytesStart::new("files");
        elem.push_attribute(("fn", fullname.as_str()));
        xml_writer.write_event(Event::Start(elem))?;
        for file in &pkg.files {
            let line = format!("{}\n", file);
            let text = BytesText::new(&line);
            xml_writer.write_event(Event::Text(text))?;
        }
        xml_writer.write_event(Event::End(BytesEnd::new("files")))?;
    }
    xml_writer.write_event(Event::End(BytesEnd::new("media_info")))?;
    Ok(())
}

pub fn write_changelog_xml<W: Write>(
    writer: &mut W,
    packages: &[RpmInfo],
) -> Result<(), quick_xml::Error> {
    let mut xml_writer = Writer::new(writer);
    xml_writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
    xml_writer.write_event(Event::Start(BytesStart::new("media_info")))?;

    for pkg in packages {
        if pkg.changelog_time.is_empty() {
            continue;
        }
        let fullname = pkg.fullname();
        let mut elem = BytesStart::new("changelogs");
        elem.push_attribute(("fn", fullname.as_str()));
        xml_writer.write_event(Event::Start(elem))?;
        for i in 0..pkg.changelog_time.len() {
            let mut log_elem = BytesStart::new("log");
            log_elem.push_attribute(("time", pkg.changelog_time[i].to_string().as_str()));
            xml_writer.write_event(Event::Start(log_elem))?;
            let name = BytesText::new(&pkg.changelog_name[i]);
            xml_writer.write_event(Event::Start(BytesStart::new("log_name")))?;
            xml_writer.write_event(Event::Text(name))?;
            xml_writer.write_event(Event::End(BytesEnd::new("log_name")))?;
            let text = BytesText::new(&pkg.changelog_text[i]);
            xml_writer.write_event(Event::Start(BytesStart::new("log_text")))?;
            xml_writer.write_event(Event::Text(text))?;
            xml_writer.write_event(Event::End(BytesEnd::new("log_text")))?;
            xml_writer.write_event(Event::End(BytesEnd::new("log")))?;
        }
        xml_writer.write_event(Event::End(BytesEnd::new("changelogs")))?;
    }
    xml_writer.write_event(Event::End(BytesEnd::new("media_info")))?;
    Ok(())
}
