#[cfg(test)]
mod tests {
    use std::process::Command;

    // Cargo sets this to the built binary for the active profile (debug/release).
    const REPOCLI: &str = env!("CARGO_BIN_EXE_repocli");

    #[test]
    fn test_help() {
        let output = Command::new(REPOCLI).arg("--help").output().unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("repocli"));
    }

    #[test]
    fn test_binary_exists() {
        assert!(std::path::Path::new(REPOCLI).exists());
    }
}
