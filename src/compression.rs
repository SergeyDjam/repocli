use anyhow::{anyhow, Result};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;

pub trait Compressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>>;
}

pub struct ExternalFilter {
    compress_cmd: String,
    decompress_cmd: String,
    _suffix: String,
}

impl ExternalFilter {
    pub fn new(spec: &str) -> Result<Self> {
        let (suffix, cmd) = if let Some((s, c)) = spec.split_once(':') {
            (s.to_string(), c.to_string())
        } else {
            (spec.to_string(), String::new())
        };
        if cmd.is_empty() {
            return Ok(ExternalFilter {
                compress_cmd: String::new(),
                decompress_cmd: String::new(),
                _suffix: suffix,
            });
        }
        let compress_cmd = cmd.clone();
        let decompress_cmd = format!("{} -d", cmd);
        Ok(ExternalFilter {
            compress_cmd,
            decompress_cmd,
            _suffix: suffix,
        })
    }

    fn run_filter(cmd: &str, input: &[u8]) -> Result<Vec<u8>> {
        if cmd.is_empty() {
            return Ok(input.to_vec());
        }
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(input.to_vec());
        }
        let mut child = Command::new(parts[0])
            .args(&parts[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut stdin = child.stdin.take().unwrap();
        let input = input.to_vec();
        let handle = thread::spawn(move || {
            stdin.write_all(&input)?;
            Ok::<_, anyhow::Error>(())
        });
        let mut output = Vec::new();
        child.stdout.take().unwrap().read_to_end(&mut output)?;
        handle.join().unwrap()?;
        let status = child.wait()?;
        if status.success() {
            Ok(output)
        } else {
            Err(anyhow!(
                "Filter command '{}' failed with status {}",
                cmd,
                status
            ))
        }
    }
}

impl Compressor for ExternalFilter {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        Self::run_filter(&self.compress_cmd, data)
    }
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        Self::run_filter(&self.decompress_cmd, data)
    }
}

pub struct NoCompressor;

impl Compressor for NoCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }
}
