//! The oracle toolchain: the one place that names the clang/LLVM release the
//! harness compares kiln with (`spec/harness.md` "Oracle toolchain").
//!
//! `KILN_ORACLE` selects the release: `22` (the default since WP-355) or
//! `19` (kiln's codegen and the M3 libc++ gate still mirror it; CI runs the
//! quick suite under both). Every tool name (`clang++-N`, `opt-N`, `llc-N`, `FileCheck-N`),
//! the LLVM directory `/usr/lib/llvm-N` (exported to kiln++ as
//! `KILN_LLVM_DIR`) and the libc++ header directory come from here; the
//! commands' `--oracle`/`--compiler` flags still override the compiler.

use std::path::PathBuf;

/// The environment variable that selects the oracle release.
pub const ENV: &str = "KILN_ORACLE";

/// The releases `KILN_ORACLE` accepts, oldest first.
pub const SUPPORTED: &[u32] = &[19, 22];

/// The release an unset (or empty) `KILN_ORACLE` selects.
pub const DEFAULT: u32 = 22;

/// One clang/LLVM release installed side by side with the others
/// (Debian's `-N` suffixed tools and `/usr/lib/llvm-N`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Oracle {
    /// The LLVM major version, one of [`SUPPORTED`].
    pub version: u32,
}

impl Default for Oracle {
    fn default() -> Self {
        Self { version: DEFAULT }
    }
}

impl Oracle {
    /// The release `KILN_ORACLE` names; unset (or empty) is the default.
    pub fn from_env() -> Result<Self, String> {
        Self::parse(std::env::var(ENV).ok().as_deref())
    }

    /// Parses a `KILN_ORACLE` value; `None` and `""` are the default.
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        let Some(v) = value.map(str::trim).filter(|v| !v.is_empty()) else {
            return Ok(Self::default());
        };
        match v.parse::<u32>() {
            Ok(n) if SUPPORTED.contains(&n) => Ok(Self { version: n }),
            _ => {
                let known: Vec<String> = SUPPORTED.iter().map(u32::to_string).collect();
                Err(format!(
                    "{ENV}=`{v}` is not a known oracle release (accepted: {}; unset means {})",
                    known.join(" or "),
                    DEFAULT
                ))
            }
        }
    }

    fn tool(self, name: &str) -> String {
        format!("{name}-{}", self.version)
    }

    /// The C++ compiler, `clang++-N`.
    pub fn clang(self) -> String {
        self.tool("clang++")
    }

    /// The IR optimizer/verifier, `opt-N`.
    #[allow(dead_code)] // the IR-diff corpus names its tools itself (kiln_codegen tests)
    pub fn opt(self) -> String {
        self.tool("opt")
    }

    /// The static compiler, `llc-N`.
    #[allow(dead_code)] // the IR-diff corpus names its tools itself (kiln_codegen tests)
    pub fn llc(self) -> String {
        self.tool("llc")
    }

    /// The test matcher, `FileCheck-N`.
    #[allow(dead_code)] // the IR-diff corpus names its tools itself (kiln_codegen tests)
    pub fn filecheck(self) -> String {
        self.tool("FileCheck")
    }

    /// The LLVM installation, `/usr/lib/llvm-N`; kiln++ gets it as
    /// `KILN_LLVM_DIR` (headers, `llc`, `opt`, the link driver).
    pub fn llvm_dir(self) -> PathBuf {
        PathBuf::from(format!("/usr/lib/llvm-{}", self.version))
    }

    /// The libc++ header directory, `/usr/lib/llvm-N/include/c++/v1`.
    pub fn libcxx_include(self) -> PathBuf {
        self.llvm_dir().join("include").join("c++").join("v1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_and_empty_are_22() {
        assert_eq!(Oracle::parse(None), Ok(Oracle { version: 22 }));
        assert_eq!(Oracle::parse(Some("")), Ok(Oracle { version: 22 }));
        assert_eq!(Oracle::default().version, 22);
        assert!(SUPPORTED.contains(&DEFAULT));
    }

    #[test]
    fn both_releases_parse() {
        let o19 = Oracle::parse(Some("19")).unwrap();
        assert_eq!(o19.clang(), "clang++-19");
        assert_eq!(o19.opt(), "opt-19");
        assert_eq!(o19.llc(), "llc-19");
        assert_eq!(o19.filecheck(), "FileCheck-19");
        assert_eq!(o19.llvm_dir(), PathBuf::from("/usr/lib/llvm-19"));
        assert_eq!(o19.libcxx_include(), PathBuf::from("/usr/lib/llvm-19/include/c++/v1"));
        let o22 = Oracle::parse(Some(" 22 ")).unwrap();
        assert_eq!(o22.clang(), "clang++-22");
        assert_eq!(o22.opt(), "opt-22");
        assert_eq!(o22.llc(), "llc-22");
        assert_eq!(o22.filecheck(), "FileCheck-22");
        assert_eq!(o22.llvm_dir(), PathBuf::from("/usr/lib/llvm-22"));
        assert_eq!(o22.libcxx_include(), PathBuf::from("/usr/lib/llvm-22/include/c++/v1"));
    }

    #[test]
    fn garbage_names_the_accepted_values() {
        for bad in ["banana", "20", "clang++-22", "-19", "19.1"] {
            let e = Oracle::parse(Some(bad)).unwrap_err();
            assert!(e.contains("KILN_ORACLE") && e.contains(bad), "{e}");
            assert!(e.contains("19 or 22"), "{e}");
        }
    }
}
