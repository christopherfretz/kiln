//! kiln's version: the one source every version string, number and macro
//! reads (spec/driver.md "Version").
//!
//! The version is the workspace's `[workspace.package] version`
//! (`Cargo.toml`; every crate inherits it, so this crate's
//! `CARGO_PKG_VERSION*` is that version). `__KILN_MAJOR__`,
//! `__KILN_MINOR__` and `__KILN_PATCH__` are its three components, the
//! install layout's resource directory is `lib/kiln/<MAJOR>`
//! ([`crate::toolchain::INSTALL_RESOURCE_SUBDIR`]), and `--version` /
//! `-dumpversion` print [`VERSION`]. [`GIT_SHA`] is the short commit the
//! binary was built from, when the build script could tell
//! (`crates/kiln_driver/build.rs`).

/// The version, `MAJOR.MINOR.PATCH[-PRE]` (semantic versioning;
/// docs/STABILITY.md).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The major version (`__KILN_MAJOR__`).
pub const MAJOR: u32 = component(env!("CARGO_PKG_VERSION_MAJOR"));

/// The minor version (`__KILN_MINOR__`).
pub const MINOR: u32 = component(env!("CARGO_PKG_VERSION_MINOR"));

/// The patch version (`__KILN_PATCH__`).
pub const PATCH: u32 = component(env!("CARGO_PKG_VERSION_PATCH"));

/// The short (12 hex digit) git commit kiln was built from: the build
/// environment's `KILN_GIT_SHA` if set, else `git rev-parse` of the
/// checkout when the crate is built inside its own git work tree. `None`
/// for a build from a source archive without either.
pub const GIT_SHA: Option<&str> = option_env!("KILN_BUILD_GIT_SHA");

/// `(major, minor, patch)`.
pub const fn triple() -> (u32, u32, u32) {
    (MAJOR, MINOR, PATCH)
}

/// [`VERSION`], followed by ` (<sha>)` when [`GIT_SHA`] is known.
pub fn long_version() -> String {
    with_sha(VERSION, GIT_SHA)
}

fn with_sha(version: &str, sha: Option<&str>) -> String {
    match sha {
        Some(s) if !s.is_empty() => format!("{version} ({s})"),
        _ => version.to_owned(),
    }
}

/// A decimal version component; a build error if Cargo ever hands over
/// anything else.
const fn component(s: &str) -> u32 {
    match u32::from_str_radix(s, 10) {
        Ok(v) => v,
        Err(_) => panic!("CARGO_PKG_VERSION_* is not a decimal number"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The components are the version's, so the macros, the resource
    /// directory and `--version` cannot disagree.
    #[test]
    fn components_match_the_version() {
        let core = VERSION.split(['-', '+']).next().unwrap();
        assert_eq!(core, format!("{MAJOR}.{MINOR}.{PATCH}"));
        assert_eq!(triple(), (MAJOR, MINOR, PATCH));
    }

    #[test]
    fn long_version_appends_the_sha() {
        assert_eq!(with_sha("1.2.3", Some("0123456789ab")), "1.2.3 (0123456789ab)");
        assert_eq!(with_sha("1.2.3", Some("")), "1.2.3");
        assert_eq!(with_sha("1.2.3", None), "1.2.3");
        assert!(long_version().starts_with(VERSION));
        if let Some(s) = GIT_SHA {
            assert!(s.len() >= 7 && s.bytes().all(|b| b.is_ascii_hexdigit()), "{s}");
        }
    }
}
