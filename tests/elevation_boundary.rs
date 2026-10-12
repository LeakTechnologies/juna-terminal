//! Guards the privilege boundary defined in `ARCHITECTURE.md` 3.8.
//!
//! **What this does and does not prove.** It is a *source-level* guard. It reads Juna's
//! own Rust sources, manifest, and build inputs and fails if any of them ask for
//! elevation or reach for a token API. It does **not** parse the PE resource
//! section of a built binary, so it cannot detect an elevation manifest injected
//! by the linker, by a build script, or by an external `.rc` file it does not know
//! about. It prevents the mistake from being made in the code we own; it is not a
//! proof about the shipped `.exe`.

use std::path::{Path, PathBuf};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Windows primitives that would let Juna raise or lower a child's privilege.
/// `conpty-oxide` exposes none of these, which is precisely why the boundary holds.
const TOKEN_APIS: &[&str] = &[
    "CreateProcessWithTokenW",
    "CreateProcessAsUserW",
    "AdjustTokenPrivileges",
    "ImpersonateLoggedOnUser",
    "ImpersonateNamedPipeClient",
    "DuplicateTokenEx",
    "SetTokenInformation",
    "PROC_THREAD_ATTRIBUTE_TOKEN",
    "ShellExecuteExW",
];

/// Manifest attributes that would make *every* launch elevated. Elevation is opt-in
/// per launch (`scripts/run-elevated.ps1`), so none of these may appear in code or
/// build inputs. Documentation prose is excluded by scoping the scan below.
const ELEVATION_MANIFEST_ATTRS: &[&str] = &["requireAdministrator", "highestAvailable", "uiAccess"];

fn read(rel: &str) -> String {
    let path = Path::new(ROOT).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Every `.rs` file under `src/`, recursively.
fn rust_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{:?}: {e}", dir));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    let mut out = Vec::new();
    walk(&Path::new(ROOT).join("src"), &mut out);
    out.sort();
    assert!(!out.is_empty(), "no sources found under src/ -- scan is vacuous");
    out
}

/// Build inputs that can influence what is linked into the binary.
fn build_inputs() -> Vec<PathBuf> {
    ["Cargo.toml", "build.rs", "juna.rc", "juna.manifest"]
        .iter()
        .map(PathBuf::from)
        .filter(|p| Path::new(ROOT).join(p).exists())
        .collect()
}

#[test]
fn no_source_reaches_for_a_token_api() {
    let violations: Vec<String> = rust_sources()
        .into_iter()
        .flat_map(|path| {
            let text = std::fs::read_to_string(&path).unwrap();
            let rel = path.strip_prefix(ROOT).unwrap_or(&path).display().to_string();
            TOKEN_APIS
                .iter()
                .filter(move |api| text.contains(**api))
                .map(move |api| format!("{} references {api}", rel))
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        violations.is_empty(),
        "ARCHITECTURE.md 3.8 forbids privilege manipulation in Juna's own code. \
         A child inherits the host token; the transport must not request elevation. \
         Offending references:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn no_build_input_requests_elevated_execution() {
    let mut violations = Vec::new();
    for path in build_inputs() {
        let text = std::fs::read_to_string(&path).unwrap();
        for attr in ELEVATION_MANIFEST_ATTRS {
            if text.contains(attr) {
                violations.push(format!(
                    "{} contains {attr}",
                    path.strip_prefix(ROOT).unwrap_or(&path).display()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "ARCHITECTURE.md 3.8: the manifest must not request elevation, so casual \
         launches stay unelevated. Offending inputs:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn the_supported_elevation_path_exists() {
    // The boundary is only meaningful if the opt-in path actually works. Removing
    // or renaming the launcher would silently leave no way to run elevated.
    let script = read("scripts/run-elevated.ps1");
    assert!(
        script.contains("-Verb RunAs"),
        "scripts/run-elevated.ps1 must launch via the 'runas' verb; see ARCHITECTURE.md 3.8"
    );
}

#[test]
fn the_transport_layer_is_free_of_elevation_intent() {
    // Narrower than no_source_reaches_for_a_token_api, and deliberately so: this one
    // is the assertion that actually encodes the boundary. The transport moves bytes
    // and nothing else.
    let text = read("src/transport.rs");
    for api in TOKEN_APIS {
        assert!(
            !text.contains(api),
            "src/transport.rs references {api}; the transport must move bytes only"
        );
    }
    for term in ["elevation", "elevated", "runas", "RunAs", "token"] {
        assert!(
            !text.contains(term),
            "src/transport.rs mentions {term:?}; privilege is a property of the host \
             process, never of a transport session (ARCHITECTURE.md 3.8)"
        );
    }
}