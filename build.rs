use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/juna.ico");
    println!("cargo:rerun-if-changed=assets/juna.rc");
    println!("cargo:rerun-if-env-changed=WINDRES");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu") {
        panic!("Juna icon resources currently require the working Windows GNU toolchain");
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("juna-icon.o");
    let status = Command::new(env::var_os("WINDRES").unwrap_or_else(|| "windres".into()))
        .current_dir(&root)
        .args([
            "--input",
            "assets/juna.rc",
            "--output-format=coff",
            "--output",
        ])
        .arg(&output)
        .status()
        .expect("Windows GNU resource compiler windres must be on PATH (or set WINDRES)");
    assert!(status.success(), "Juna icon resource compilation failed");
    println!("cargo:rustc-link-arg-bin=juna={}", output.display());
}
