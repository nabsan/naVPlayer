use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Duration, Utc};

fn main() {
    println!("cargo:rerun-if-changed=libmpv-2.dll");
    println!("cargo:rerun-if-changed=build.rs");

    let jst_now = Utc::now() + Duration::hours(9);
    let version = format!("ver.{}", jst_now.format("%Y%m%d.%H.%M"));
    println!("cargo:rustc-env=NAVPLAYER_VERSION={version}");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR missing"));
    let source = manifest_dir.join("libmpv-2.dll");
    if !source.exists() {
        println!("cargo:warning=libmpv-2.dll not found in {}", manifest_dir.display());
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR missing"));
    let Some(profile_dir) = target_profile_dir(&out_dir) else {
        println!("cargo:warning=unable to determine target profile directory from {}", out_dir.display());
        return;
    };

    let destination = profile_dir.join("libmpv-2.dll");
    if let Err(err) = fs::create_dir_all(&profile_dir) {
        println!("cargo:warning=failed to create {}: {}", profile_dir.display(), err);
        return;
    }
    if let Err(err) = fs::copy(&source, &destination) {
        println!("cargo:warning=failed to copy {} to {}: {}", source.display(), destination.display(), err);
        return;
    }

    println!("cargo:warning=copied {} to {}", source.display(), destination.display());
}

fn target_profile_dir(out_dir: &Path) -> Option<PathBuf> {
    out_dir.ancestors().nth(3).map(Path::to_path_buf)
}
