//! Expose the Cargo host/target triples to tests at compile time.
//!
//! In cross-compilation CI the produced binary cannot always be executed by
//! the test runner; tests that spawn it need to know this.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let host = std::env::var("HOST").unwrap_or_default();
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rustc-env=REPOCLI_HOST_TRIPLE={}", host);
    println!("cargo:rustc-env=REPOCLI_TARGET_TRIPLE={}", target);
}
