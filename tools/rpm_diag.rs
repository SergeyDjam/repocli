// src/bin/rpm_diag.rs
use std::env;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: rpm_diag <rpm-file>");
        std::process::exit(1);
    }
    let path = Path::new(&args[1]);
    if let Err(e) = diagnose(path) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn diagnose(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    println!("File: {}", path.display());
    println!("Size: {} bytes", file_len);

    // ----- Lead (96 bytes) -----
    let mut lead = [0u8; 96];
    file.read_exact(&mut lead)?;
    println!("\n--- Lead (96 bytes) ---");
    print_hex(&lead, 96);
    let magic = &lead[0..4];
    println!(
        "Magic: {:02x?} {}",
        magic,
        if magic == b"\xed\xab\xee\xdb" {
            "(OK)"
        } else {
            "(INVALID)"
        }
    );
    let version = u8::from_le_bytes([lead[4]]);
    let type_ = u8::from_le_bytes([lead[5]]);
    let arch = u16::from_le_bytes([lead[6], lead[7]]);
    let name = String::from_utf8_lossy(&lead[8..72])
        .trim_end_matches('\0')
        .to_string();
    let os = u16::from_le_bytes([lead[72], lead[73]]);
    let sig_type = u16::from_le_bytes([lead[74], lead[75]]);
    let sig_size = u32::from_le_bytes([lead[76], lead[77], lead[78], lead[79]]);
    println!("Version: {}", version);
    println!("Type: {}", type_);
    println!("Arch: {}", arch);
    println!("Name: '{}'", name);
    println!("OS: {}", os);
    println!("Signature type: {}", sig_type);
    println!("Signature size: {} bytes", sig_size);

    // ----- Signature (if any) -----
    if sig_size > 0 {
        if 96 + sig_size as u64 > file_len {
            println!(
                "\nWARNING: Signature size {} exceeds file length {}",
                sig_size, file_len
            );
            println!("Attempting to locate header magic by scanning...");
        } else {
            println!("\n--- Signature ({} bytes) ---", sig_size);
            let mut sig_buf = vec![0u8; sig_size as usize];
            file.read_exact(&mut sig_buf)?;
            print_hex(&sig_buf, 64);
            // Optional: parse the signature (more involved)
        }
    } else {
        println!("\nNo signature.");
    }

    // ----- Locate header magic (8e ad e8 01) -----
    // If the signature looks suspicious or cannot be read, scan for header magic from offset 96.
    let header_magic = b"\x8e\xad\xe8\x01";
    let mut header_pos = None;
    file.seek(SeekFrom::Start(96))?;
    let mut buf = vec![0u8; 4096];
    let mut pos = 96;
    while let Ok(n) = file.read(&mut buf) {
        if n == 0 {
            break;
        }
        if let Some(offset) = buf[..n].windows(4).position(|w| w == header_magic) {
            header_pos = Some(pos + offset as u64);
            break;
        }
        pos += n as u64;
        if pos > file_len {
            break;
        }
        // rewind 3 bytes to catch magic spanning a buffer boundary
        if n >= 4 {
            file.seek(SeekFrom::Current(-3))?;
            pos -= 3;
        }
    }
    let header_pos = header_pos.ok_or("Header magic not found")?;
    println!(
        "\nHeader magic found at offset: {} (0x{:x})",
        header_pos, header_pos
    );
    file.seek(SeekFrom::Start(header_pos))?;

    // ----- Header structure -----
    let mut magic_buf = [0u8; 4];
    file.read_exact(&mut magic_buf)?;
    let mut reserved = [0u8; 4];
    file.read_exact(&mut reserved)?;
    let mut index_count_buf = [0u8; 4];
    file.read_exact(&mut index_count_buf)?;
    let index_count = u32::from_be_bytes(index_count_buf);
    let mut data_size_buf = [0u8; 4];
    file.read_exact(&mut data_size_buf)?;
    let data_size = u32::from_be_bytes(data_size_buf);
    println!("\n--- Header ---");
    println!("Magic: {:02x?} (should be 8e ad e8 01)", magic_buf);
    println!("Reserved: {:02x?}", reserved);
    println!("Index count: {}", index_count);
    println!("Data size: {} bytes", data_size);

    let index_size = index_count as usize * 16;
    let mut index = vec![0u8; index_size];
    file.read_exact(&mut index)?;
    let mut data = vec![0u8; data_size as usize];
    file.read_exact(&mut data)?;

    println!("\n--- Index entries (first 20) ---");
    let mut entries = Vec::new();
    for i in 0..index_count.min(20) {
        let base = i as usize * 16;
        let tag = u32::from_be_bytes([
            index[base],
            index[base + 1],
            index[base + 2],
            index[base + 3],
        ]);
        let typ = u32::from_be_bytes([
            index[base + 4],
            index[base + 5],
            index[base + 6],
            index[base + 7],
        ]);
        let offset = u32::from_be_bytes([
            index[base + 8],
            index[base + 9],
            index[base + 10],
            index[base + 11],
        ]);
        let count = u32::from_be_bytes([
            index[base + 12],
            index[base + 13],
            index[base + 14],
            index[base + 15],
        ]);
        entries.push((tag, typ, offset, count));
        println!(
            "  tag={:4} type={:2} offset={:4} count={}",
            tag, typ, offset, count
        );
    }
    if index_count > 20 {
        println!("  ... and {} more", index_count - 20);
    }

    // Try to extract the name (tag=1000)
    if let Some((_, typ, offset, count)) = entries.iter().find(|(t, _, _, _)| *t == 1000) {
        if *typ == 6 && *count == 1 {
            let start = *offset as usize;
            let mut end = start;
            while end < data.len() && data[end] != 0 {
                end += 1;
            }
            if end <= data.len() {
                let name = String::from_utf8_lossy(&data[start..end]);
                println!("\nPackage name: '{}'", name);
            }
        } else {
            println!(
                "\nPackage name tag found but type/count unexpected: type={} count={}",
                typ, count
            );
        }
    } else {
        println!("\nPackage name tag (1000) not found in header.");
    }

    Ok(())
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
        print!(" |");
        for j in 0..16 {
            if i + j < len {
                let c = data[i + j];
                if (0x20..=0x7e).contains(&c) {
                    print!("{}", c as char);
                } else {
                    print!(".");
                }
            }
        }
        println!("|");
    }
    if data.len() > max {
        println!("... and {} more bytes", data.len() - max);
    }
}
