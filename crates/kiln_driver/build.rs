//! Puts `kiln++` next to the `kiln` binary: one driver, two names, as
//! clang and clang++ (spec/driver.md "Driver modes").
//!
//! Cargo cannot build a binary target named `kiln++` (a target name must be
//! a valid crate name), so the second name is a relative symlink
//! `kiln++ -> kiln` in the profile directory (`target/<profile>/`, three
//! levels above `OUT_DIR`), made once; until `kiln` is linked it dangles.
//! xtask's `build_kiln` makes it again if it is missing.
//!
//! On Linux the `kiln` binary is linked with `-z pack-relative-relocs`
//! (WP-653): its tens of thousands of `R_*_RELATIVE` relocations (every
//! `&str` and slice in a static table) are stored as a few KB of
//! `.relr.dyn` bitmaps instead of 24 bytes each in `.rela.dyn`, which the
//! loader reads in full before `main` (about 0.2 to 0.5 MB of peak RSS on
//! every compile; docs/status/perf-reference-workload.md "WP-653"). GNU ld
//! 2.38+ and lld 15+ accept it, glibc 2.36+ loads it.
#![forbid(unsafe_code)]

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // The target's, not the host's: a build script runs on the host.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-bin=kiln=-Wl,-z,pack-relative-relocs");
    }
    #[cfg(unix)]
    link_kilnpp();
}

#[cfg(unix)]
fn link_kilnpp() {
    let Some(out) = std::env::var_os("OUT_DIR") else { return };
    let out = std::path::PathBuf::from(out);
    // <target>/<profile>/build/kiln_driver-<hash>/out
    let Some(profile) = out.ancestors().nth(3) else { return };
    if !profile.join("build").is_dir() {
        return;
    }
    let link = profile.join("kiln++");
    if std::fs::symlink_metadata(&link).is_err() {
        // A parallel build may have made it first; either way it exists.
        let _ = std::os::unix::fs::symlink("kiln", &link);
    }
}
