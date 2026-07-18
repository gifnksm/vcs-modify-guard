//! This build script injects compile-time configuration flags.

use std::env;

const GIT_FEATURES: &[&str] = &["git-libgit2"];
const CFG_VCS_BACKEND_ENABLED: &str = "vcs_backend_enabled";
const CFG_GIT_BACKEND_ENABLED: &str = "git_backend_enabled";

fn is_feature_enabled(name: &str) -> bool {
    let name = name.replace('-', "_").to_uppercase();
    env::var(format!("CARGO_FEATURE_{name}")).is_ok()
}

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg({CFG_GIT_BACKEND_ENABLED})");
    println!("cargo::rustc-check-cfg=cfg({CFG_VCS_BACKEND_ENABLED})");

    let git_backend_enabled = GIT_FEATURES
        .iter()
        .any(|&feature| is_feature_enabled(feature));
    if git_backend_enabled {
        println!("cargo::rustc-cfg={CFG_GIT_BACKEND_ENABLED}");
    }

    let vcs_backend_enabled = git_backend_enabled;
    if vcs_backend_enabled {
        println!("cargo::rustc-cfg={CFG_VCS_BACKEND_ENABLED}");
    }
}
