// src/bin/compare_headers.rs
use anyhow::Result;
use memmap2::Mmap;
use rpm::Package;
use std::fs::File;
use std::path::Path;

const LEAD_SIZE: usize = 96;
const HEADER_MAGIC: [u8; 4] = [0x8e, 0xad, 0xe8, 0x01];

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: compare_headers <rpm-file>");
        std::process::exit(1);
    }
    let path = Path::new(&args[1]);

    // 1. Get the exported header (reference) via the slow method
    let pkg = Package::open(path)?;
    let exported = pkg.header_bytes()?;
    println!("Exported header size: {} bytes", exported.len());

    // 2. Get the raw header (fast parser) - read the header from the file
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let file_len = mmap.len();
    if file_len < LEAD_SIZE {
        eprintln!("File too small");
        std::process::exit(1);
    }
    let lead = &mmap[0..LEAD_SIZE];
    if &lead[0..4] != b"\xed\xab\xee\xdb" {
        eprintln!("Invalid RPM magic");
        std::process::exit(1);
    }
    let mut pos = LEAD_SIZE;
    let sig_size = u32::from_le_bytes([lead[76], lead[77], lead[78], lead[79]]);
    const MAX_SANE_SIG_SIZE: u32 = 1 << 20;
    if sig_size > 0 && sig_size <= MAX_SANE_SIG_SIZE && pos + sig_size as usize <= file_len {
        pos += sig_size as usize;
    } else if sig_size > 0 {
        // Locate the header magic
        if let Some(header_start) = find_header_magic(&mmap[pos..]) {
            pos += header_start;
        } else {
            eprintln!("Cannot find header magic");
            std::process::exit(1);
        }
    }
    // Parse the header to determine its size
    let header = parse_header(&mmap[pos..])?;
    let header_size = 16 + header.index_count * 16 + header.data_size;
    let raw_header = &mmap[pos..pos + header_size];
    println!("Raw header size: {} bytes", raw_header.len());

    // 3. Compare
    if raw_header.len() == exported.len() {
        let diff: Vec<usize> = raw_header
            .iter()
            .zip(exported.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        if diff.is_empty() {
            println!("Headers are identical.");
        } else {
            println!("Headers differ at {} positions:", diff.len());
            for &i in diff.iter().take(20) {
                println!(
                    "  offset {}: raw={:02x} exported={:02x}",
                    i, raw_header[i], exported[i]
                );
            }
            if diff.len() > 20 {
                println!("  ... and {} more", diff.len() - 20);
            }
        }
    } else {
        println!(
            "Sizes differ: raw={}, exported={}",
            raw_header.len(),
            exported.len()
        );
        // Print the first 100 bytes for visual comparison
        println!("Raw header first 100 bytes:");
        print_hex(raw_header, 100);
        println!("Exported header first 100 bytes:");
        print_hex(&exported, 100);
    }

    Ok(())
}

fn parse_header(data: &[u8]) -> Result<ParsedHeader> {
    if data.len() < 16 {
        anyhow::bail!("Header too short");
    }
    let magic = &data[0..4];
    if magic != HEADER_MAGIC {
        anyhow::bail!("Invalid header magic");
    }
    let index_count = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
    let data_size = u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as usize;
    Ok(ParsedHeader {
        index_count,
        data_size,
    })
}

struct ParsedHeader {
    index_count: usize,
    data_size: usize,
}

fn find_header_magic(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|window| window == HEADER_MAGIC)
}

fn print_hex(data: &[u8], max: usize) {
    let len = data.len().min(max);
    for i in (0..len).step_by(16) {
        print!("{:04x}: ", i);
        for j in 0..16 {
            if i + j < len {
                print!("{:02x} ", data[i + j]);
            } else {
                print!("   ");
            }
        }
        println!();
    }
    if data.len() > max {
        println!("... and {} more bytes", data.len() - max);
    }
}
