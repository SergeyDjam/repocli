use crate::compression::{Compressor, ExternalFilter, NoCompressor};
use crate::old_metadata;
use crate::rpm::RpmInfo;
use crate::synthesis;
use crate::xml_metadata;
use crate::Args;
use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[allow(clippy::too_many_arguments)]
pub async fn run(
    media_info_dir: &Path,
    new_packages: &[RpmInfo],
    args: &Args,
    synthesis_suffix: &str,
    synthesis_cmd: &str,
    xml_suffix: &str,
    xml_cmd: &str,
    xml_types: &[&str],
    file_deps: Option<&HashSet<String>>,
) -> Result<()> {
    let old_metadata_file = media_info_dir.join("old-metadata.lst");
    let synthesis_path = media_info_dir.join(format!("synthesis.hdlist{}", synthesis_suffix));
    let tmp_synth = media_info_dir.join("synthesis.hdlist.tmp");

    if synthesis_path.exists() {
        let compressor: Box<dyn Compressor> = if synthesis_cmd.is_empty() {
            Box::new(NoCompressor)
        } else {
            Box::new(ExternalFilter::new(&format!(
                "{}:{}",
                synthesis_suffix, synthesis_cmd
            ))?)
        };
        let compressed = fs::read(&synthesis_path)?;
        let decompressed = compressor.decompress(&compressed)?;
        fs::write(&tmp_synth, decompressed)?;
    } else {
        fs::write(&tmp_synth, b"")?;
    }

    for &typ in xml_types {
        let path = media_info_dir.join(format!("{}.xml{}", typ, xml_suffix));
        let tmp_path = media_info_dir.join(format!("{}.xml.tmp", typ));
        if path.exists() {
            let compressor: Box<dyn Compressor> = if xml_cmd.is_empty() {
                Box::new(NoCompressor)
            } else {
                Box::new(ExternalFilter::new(&format!("{}:{}", xml_suffix, xml_cmd))?)
            };
            let compressed = fs::read(&path)?;
            let decompressed = compressor.decompress(&compressed)?;
            fs::write(&tmp_path, decompressed)?;
        } else {
            fs::write(
                &tmp_path,
                b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<media_info></media_info>",
            )?;
        }
    }

    if old_metadata_file.exists() {
        let xml_names: Vec<&str> = xml_types.to_vec();
        old_metadata::filter_out_old_metadata(
            media_info_dir,
            &old_metadata_file,
            &tmp_synth,
            &xml_names,
        )?;
    }

    let mut all_packages = Vec::new();
    all_packages.extend_from_slice(new_packages);

    let t_synth = std::time::Instant::now();
    {
        let mut out = Vec::new();
        synthesis::write_synthesis(&mut out, &all_packages, file_deps)?;
        let compressor: Box<dyn Compressor> = if synthesis_cmd.is_empty() {
            Box::new(NoCompressor)
        } else {
            Box::new(ExternalFilter::new(&format!(
                "{}:{}",
                synthesis_suffix, synthesis_cmd
            ))?)
        };
        let compressed = compressor.compress(&out)?;
        fs::write(&tmp_synth, compressed)?;
    }
    fs::rename(&tmp_synth, &synthesis_path)?;
    crate::report_time("merge:synthesis", t_synth);

    let t_hdlist = std::time::Instant::now();
    // Hdlist from the raw headers collected during metadata parsing:
    // each RPM is opened only once.
    if !args.no_hdlist {
        let hdlist_path = media_info_dir.join("hdlist.cz");
        let tmp_hdlist = media_info_dir.join("hdlist.cz.tmp");
        crate::hdlist::build_hdlist_from_packages(&all_packages, &tmp_hdlist)?;
        fs::rename(&tmp_hdlist, &hdlist_path)?;
    }
    crate::report_time("merge:hdlist", t_hdlist);

    let t_xml = std::time::Instant::now();
    let compressor: Box<dyn Compressor> = if xml_cmd.is_empty() {
        Box::new(NoCompressor)
    } else {
        Box::new(ExternalFilter::new(&format!("{}:{}", xml_suffix, xml_cmd))?)
    };
    for &typ in xml_types {
        let tmp_path = media_info_dir.join(format!("{}.xml.tmp", typ));
        let mut buf = Vec::new();
        match typ {
            "info" => xml_metadata::write_info_xml(&mut buf, &all_packages)?,
            "files" => xml_metadata::write_files_xml(&mut buf, &all_packages)?,
            "changelog" => xml_metadata::write_changelog_xml(&mut buf, &all_packages)?,
            _ => {}
        }
        let compressed = compressor.compress(&buf)?;
        fs::write(&tmp_path, compressed)?;
        let final_path = media_info_dir.join(format!("{}.xml{}", typ, xml_suffix));
        fs::rename(&tmp_path, &final_path)?;
    }
    crate::report_time("merge:xml", t_xml);

    Ok(())
}
