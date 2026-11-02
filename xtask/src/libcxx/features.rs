//! The static lit feature set of `cargo xtask libcxx` (`spec/libcxx.md`
//! "Feature set").
//!
//! libc++'s lit configuration derives its features at startup by probing the
//! compiler (`libcxx/utils/libcxx/test/{params,features}.py`). This driver
//! does not run lit, so the set is fixed here: what that configuration
//! derives for libc++ 19.1.7 (Debian `libc++-19-dev`) on
//! aarch64-unknown-linux-gnu with its default parameters, with three
//! deliberate differences:
//!
//! - no compiler-identity features (`clang`, `clang-19`, `gcc`,
//!   `apple-clang`, `msvc`, `gcc-style-warnings`, ...): kiln is none of
//!   them, so a `// XFAIL: clang` must not hide a kiln result, and a
//!   `// UNSUPPORTED: gcc` must not hide a test; `kiln` is added instead;
//! - target capabilities that lit probes by compiling a snippet
//!   (`has-unix-headers`, `has-64-bit-atomics`) are taken as clang 19 sees
//!   this machine, not as kiln does today, so the set does not move when
//!   kiln changes. Probes of compiler *flags* (`has-fconstexpr-steps`,
//!   `-fsized-deallocation`, `verify-support`) are kiln's answers;
//! - `libcpp-hardening-mode=none` is present although lit adds it only when
//!   the `hardening_mode` parameter is set: it is what the installed
//!   `__config_site` builds (`_LIBCPP_HARDENING_MODE_DEFAULT` is
//!   `_LIBCPP_HARDENING_MODE_NONE`), so `.assert.pass.cpp` death tests that
//!   lit gates with `UNSUPPORTED: libcpp-hardening-mode=none` are skipped
//!   instead of failing with "child did not die" (clang++-19 fails them the
//!   same way).

/// The C++ standard the suite is run in, as a lit feature and a flag.
pub const STD: &str = "c++26";

/// The target triple (`target=<triple>` feature).
pub const TARGET: &str = "aarch64-unknown-linux-gnu";

/// Every feature, sorted, without the `target=` one (see [`all`]).
const FEATURES: &[&str] = &[
    // Parameters (params.py defaults): std, stdlib, optimization,
    // enable_experimental, long_tests, large_tests.
    "c++26",
    "c++experimental",
    "large_tests",
    "long_tests",
    "optimization=none",
    "stdlib=libc++",
    "stdlib=llvm-libc++",
    // Flags kiln accepts (lit probes them with hasCompileFlag).
    "-fsized-deallocation",
    "has-fconstexpr-steps",
    // `hasCompileFlag("-Xclang -verify-ignore-unexpected")` (spec/libcxx.md
    // ".verify.cpp").
    "verify-support",
    // __config / __config_site macros (features.py `macros`).
    "libcpp-abi-version=1",
    "libcpp-has-no-availability-markup",
    "libcpp-has-thread-api-pthread",
    // `_LIBCPP_HARDENING_MODE_DEFAULT` in the installed `__config_site`
    // (Debian: none); see the module doc.
    "libcpp-hardening-mode=none",
    // Target and host.
    "buildhost=linux",
    "can-create-symlinks",
    "has-64-bit-atomics",
    "has-unix-headers",
    "linux",
    // This compiler.
    "kiln",
];

/// The full feature set, including `target=<triple>`.
pub fn all() -> Vec<String> {
    let mut v: Vec<String> = FEATURES.iter().map(|&f| f.to_owned()).collect();
    v.push(format!("target={TARGET}"));
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_is_sorted_unique_and_has_no_compiler_identity() {
        let all = all();
        let mut sorted = all.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(all, sorted);
        assert!(all.contains(&format!("target={TARGET}")));
        assert!(all.iter().any(|f| f == STD));
        assert!(all.iter().any(|f| f == "kiln"));
        assert!(all.iter().any(|f| f == "verify-support"));
        assert!(all.iter().any(|f| f == "libcpp-hardening-mode=none"));
        for f in &all {
            for bad in ["clang", "gcc", "msvc", "apple-clang"] {
                assert!(
                    f != bad && !f.starts_with(&format!("{bad}-")),
                    "compiler-identity feature `{f}` in the static set"
                );
            }
        }
    }
}
