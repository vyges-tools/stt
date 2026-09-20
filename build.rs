// SPDX-License-Identifier: Apache-2.0
//! Fetch the FLUTE lookup tables at build time.
//!
//! ⛔ **The tables are not vendored.** `POWV9.dat` + `POST9.dat` are 9.4 MB of BSD-3 licensed
//! third-party DATA. Fetching them keeps this Apache-2.0 repo free of it and makes the provenance
//! a commit rather than a copy. See `flute-tables.yaml`.

use std::path::{Path, PathBuf};

fn pinned_sha() -> String {
    let pin = std::fs::read_to_string("flute-tables.yaml").expect("read flute-tables.yaml");
    pin.lines()
        .find_map(|l| {
            let l = l.trim();
            l.strip_prefix("commit:")
                .map(|v| v.split('#').next().unwrap_or("").trim().to_string())
        })
        .expect("flute-tables.yaml has no `commit:`")
}

/// The SHA a checkout is actually at, or None when it is not a git tree.
fn tree_sha(dir: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["-C", dir.to_str()?, "rev-parse", "HEAD"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn fetch(dest: &Path, sha: &str) {
    let src = "https://github.com/The-OpenROAD-Project/OpenROAD.git";
    println!("cargo:warning=vyges-stt: fetching the FLUTE tables @ {sha}");
    let run = |args: &[&str], cwd: Option<&Path>| {
        let mut c = std::process::Command::new("git");
        c.args(args);
        if let Some(d) = cwd {
            c.current_dir(d);
        }
        assert!(c.status().expect("git not found").success(), "git {args:?} failed");
    };
    if !dest.join(".git").exists() {
        std::fs::create_dir_all(dest).unwrap();
        run(&["clone", "--quiet", "--filter=blob:none", "--no-checkout", src,
              dest.to_str().unwrap()], None);
    }
    // ⚠️ A clone taken at an earlier pin holds no objects for a newer commit, so `checkout` alone
    // fails on a re-pin. Fetch the exact SHA first; on a fresh clone this is a no-op.
    run(&["fetch", "--quiet", "--filter=blob:none", "origin", sha], Some(dest));
    run(&["sparse-checkout", "set", "--cone", "src/stt/src/flt/etc"], Some(dest));
    run(&["checkout", "--quiet", sha], Some(dest));
}

fn main() {
    println!("cargo:rerun-if-changed=flute-tables.yaml");
    println!("cargo:rerun-if-changed=build.rs");

    // ⛔ The pin the CLI reports comes from HERE, not from a copy in `main.rs`. It was a copy,
    // and nothing compared the two — the correlation harness reads the binary's `--describe` pin
    // to refuse a stale engine, so a `main.rs` left behind at a bump would have made the harness
    // certify the wrong reference. One source, checked by the compiler.
    println!("cargo:rustc-env=VYGES_STT_OPENROAD_PIN={}", pinned_sha());

    // ⚠️ The commit this binary was built from, for `--version`. A released binary and a local
    // build of the same version are different artifacts, and a correlation run has to say which.
    // `unknown` when the source is not a git tree — a tarball build is legitimate, and refusing
    // to build over a missing SHA would break it.
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=VYGES_STT_GIT_SHA={sha}");

    // An explicit override, for an air-gapped build or a tree already on disk.
    if let Ok(dir) = std::env::var("VYGES_STT_FLUTE_DIR") {
        println!("cargo:rustc-env=VYGES_STT_FLUTE_DIR={dir}");
        return;
    }

    let sha = pinned_sha();
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let dest = out.join("OpenROAD");
    let etc = dest.join("src/stt/src/flt/etc");

    // ⛔ **EXISTENCE IS NOT FRESHNESS.** Fetching only when the files are ABSENT leaves a tree
    // from the previous commit in place, and the build then reads tables that do not match what
    // this engine was validated against. Keying a cache on presence rather than identity has
    // cost us a day before now, on a machine where it could not be reproduced.
    let stale = etc.join("POWV9.dat").exists() && tree_sha(&dest).as_deref() != Some(sha.as_str());
    if stale {
        println!("cargo:warning=vyges-stt: the cached table tree is not at {sha}; re-fetching");
    }
    if !etc.join("POWV9.dat").exists() || stale {
        fetch(&dest, &sha);
    }

    for f in ["POWV9.dat", "POST9.dat"] {
        assert!(etc.join(f).exists(), "{f} missing after fetch at {sha}");
    }
    println!("cargo:rustc-env=VYGES_STT_FLUTE_DIR={}", etc.display());
    println!("cargo:rustc-env=VYGES_STT_OPENROAD_PIN={sha}");
}