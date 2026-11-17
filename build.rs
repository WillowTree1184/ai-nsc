//! Captures the LLVM version and host triple at build time so that
//! `ai-nsc --version` reports the toolchain it was actually linked against.

use std::path::PathBuf;
use std::process::Command;

fn llvm_config() -> PathBuf {
    match std::env::var_os("LLVM_SYS_181_PREFIX") {
        Some(prefix) => PathBuf::from(prefix).join("bin").join("llvm-config"),
        None => PathBuf::from("llvm-config"),
    }
}

fn query(args: &[&str]) -> Option<String> {
    let output = Command::new(llvm_config()).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn main() {
    println!("cargo:rerun-if-env-changed=LLVM_SYS_181_PREFIX");
    println!("cargo:rerun-if-changed=build.rs");

    let version = query(&["--version"]).unwrap_or_else(|| "unknown".to_string());
    let host_target = query(&["--host-target"]).unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=AINSC_LLVM_VERSION={version}");
    println!("cargo:rustc-env=AINSC_HOST_TRIPLE={host_target}");
}
