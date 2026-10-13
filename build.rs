use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-env-changed=JUNA_BUILD_COMMIT");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/master");
    let commit = env::var("JUNA_BUILD_COMMIT")
        .ok()
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
        })
        .unwrap_or_else(|| "unknown".into());
    let commit = commit.trim();
    assert!(
        commit == "unknown"
            || (commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit())),
        "Invalid build commit"
    );
    println!("cargo:rustc-env=JUNA_BUILD_COMMIT={commit}");
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
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let numeric = format!(
        "{},{},{},0",
        env::var("CARGO_PKG_VERSION_MAJOR").unwrap(),
        env::var("CARGO_PKG_VERSION_MINOR").unwrap(),
        env::var("CARGO_PKG_VERSION_PATCH").unwrap()
    );
    let resource = output.with_extension("rc");
    let mut text = std::fs::read_to_string(root.join("assets/juna.rc")).unwrap();
    text.push_str(&format!(
        r#"
#include <winver.h>
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEFLAGSMASK 0x3fL
FILEFLAGS {}
FILEOS VOS_NT_WINDOWS32
FILETYPE VFT_APP
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "100904b0"
  BEGIN
   VALUE "CompanyName", "Leak Technologies\0"
   VALUE "FileDescription", "Juna Terminal\0"
   VALUE "FileVersion", "{version}\0"
   VALUE "ProductName", "Juna Terminal\0"
   VALUE "ProductVersion", "{version}\0"
   VALUE "OriginalFilename", "juna.exe\0"
   VALUE "SourceCommit", "{commit}\0"
   VALUE "Comments", "Source commit: {commit}\0"
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x1009, 1200
 END
END
"#,
        if version.contains('-') {
            "VS_FF_PRERELEASE"
        } else {
            "0"
        }
    ));
    std::fs::write(&resource, text).unwrap();
    let status = Command::new(env::var_os("WINDRES").unwrap_or_else(|| "windres".into()))
        .current_dir(&root)
        .args(["--input"])
        .arg(&resource)
        .args(["--output-format=coff", "--output"])
        .arg(&output)
        .status()
        .expect("Windows GNU resource compiler windres must be on PATH (or set WINDRES)");
    assert!(status.success(), "Juna icon resource compilation failed");
    println!("cargo:rustc-link-arg-bin=juna={}", output.display());
}
