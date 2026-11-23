#[cfg(test)]
mod tests {
    use std::process::Command;

    // Cargo sets this to the built binary for the active profile (debug/release).
    const REPOCLI: &str = env!("CARGO_BIN_EXE_repocli");

    // Provided by build.rs (Cargo HOST / TARGET triples).
    const HOST: &str = env!("REPOCLI_HOST_TRIPLE");
    const TARGET: &str = env!("REPOCLI_TARGET_TRIPLE");

    /// In cross-compilation CI (e.g. HOST=x86_64, TARGET=riscv64gc) the built
    /// binary is not executable on the runner. Only spawn it on native builds.
    fn can_execute_target() -> bool {
        HOST == TARGET
    }

    #[test]
    fn test_help() {
        if !can_execute_target() {
            eprintln!("skipping --help execution test: cross-compiling {} on {}", TARGET, HOST);
            return;
        }
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
