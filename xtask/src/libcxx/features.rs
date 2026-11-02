//! The lit feature set of `cargo xtask libcxx` (`spec/libcxx.md` "Feature
//! set").
//!
//! libc++'s lit configuration derives its features at startup by probing the
//! compiler (`libcxx/utils/libcxx/test/{params,features}.py`). This driver
//! does not run lit, so most of the set is fixed here (the `locale.*`
//! features are detected, [`detect`]): what that configuration
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

/// The static feature set, including `target=<triple>` and without the
/// detected `locale.*` features (see [`detect`]).
pub fn all() -> Vec<String> {
    let mut v: Vec<String> = FEATURES.iter().map(|&f| f.to_owned()).collect();
    v.push(format!("target={TARGET}"));
    v.sort();
    v
}

/// The locales libc++'s tests ask for, by the name in their `locale.<name>`
/// feature (`features.py` `locales`). `scripts/provision-locales.sh`
/// generates them.
pub const LOCALES: &[&str] = &[
    "cs_CZ.ISO8859-2",
    "en_US.UTF-8",
    "fr_CA.ISO8859-1",
    "fr_FR.UTF-8",
    "ja_JP.UTF-8",
    "ru_RU.UTF-8",
    "zh_CN.UTF-8",
];

/// The feature set of this machine: [`all`] plus `locale.<name>` for each
/// of [`LOCALES`] that `locale -a` lists.
pub fn detect() -> Vec<String> {
    let listed = std::process::Command::new("locale")
        .arg("-a")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    with_locales(all(), &listed)
}

/// `features` plus the `locale.*` features of `locale_a` (`locale -a`
/// output), sorted.
pub fn with_locales(mut features: Vec<String>, locale_a: &str) -> Vec<String> {
    features.extend(locale_features(locale_a));
    features.sort();
    features.dedup();
    features
}

/// `locale.<name>` for each of [`LOCALES`] that `locale_a` lists under any
/// spelling glibc's `setlocale` accepts for it: `locale -a` prints the
/// normalized codeset (`en_US.utf8`, `cs_CZ.iso88592`).
pub fn locale_features(locale_a: &str) -> Vec<String> {
    let have: Vec<String> = locale_a.lines().map(|l| normalize_locale(l.trim())).collect();
    LOCALES
        .iter()
        .filter(|l| have.contains(&normalize_locale(l)))
        .map(|l| format!("locale.{l}"))
        .collect()
}

/// glibc's locale-name normalization (`_nl_normalize_codeset`): the part
/// after `.` (up to any `@modifier`) keeps only letters (lowercased) and
/// digits, and gets an `iso` prefix when it is all digits. So `UTF-8` and
/// `utf8` are both `utf8`, `ISO8859-2` and `iso88592` both `iso88592`.
pub fn normalize_locale(name: &str) -> String {
    let Some((lang, rest)) = name.split_once('.') else { return name.to_owned() };
    let (codeset, modifier) = rest.split_once('@').map_or((rest, None), |(c, m)| (c, Some(m)));
    let mut norm: String = codeset
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if !norm.is_empty() && norm.chars().all(|c| c.is_ascii_digit()) {
        norm.insert_str(0, "iso");
    }
    let mut out = format!("{lang}.{norm}");
    if let Some(m) = modifier {
        out.push('@');
        out.push_str(m);
    }
    out
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
        assert!(!all.iter().any(|f| f.starts_with("locale.")), "locales are detected");
    }

    #[test]
    fn locale_names_normalize_like_glibc() {
        for (name, want) in [
            ("en_US.UTF-8", "en_US.utf8"),
            ("en_US.utf8", "en_US.utf8"),
            ("cs_CZ.ISO8859-2", "cs_CZ.iso88592"),
            ("cs_CZ.ISO-8859-2", "cs_CZ.iso88592"),
            ("fr_CA.iso88591", "fr_CA.iso88591"),
            ("ja_JP.eucJP", "ja_JP.eucjp"),
            ("sr_RS.UTF-8@latin", "sr_RS.utf8@latin"),
            ("C", "C"),
            ("POSIX", "POSIX"),
        ] {
            assert_eq!(normalize_locale(name), want, "{name}");
        }
        let mut sorted = LOCALES.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, LOCALES);
    }

    #[test]
    fn locale_features_from_locale_a() {
        // A container without locales: only C, C.utf8 and POSIX.
        assert_eq!(locale_features("C\nC.utf8\nPOSIX\n"), Vec::<String>::new());
        // After scripts/provision-locales.sh (Debian's `locale -a`).
        let provisioned = "C\nC.utf8\nPOSIX\ncs_CZ.iso88592\nczech\nen_US.utf8\n\
                           fr_CA.iso88591\nfr_FR.utf8\nja_JP.utf8\nru_RU.utf8\nzh_CN.utf8\n";
        let got = locale_features(provisioned);
        assert_eq!(got.len(), LOCALES.len());
        assert!(got.contains(&"locale.cs_CZ.ISO8859-2".to_owned()));
        assert!(got.contains(&"locale.en_US.UTF-8".to_owned()));
        // A bare `fr_CA` (no codeset) is not `fr_CA.ISO8859-1`, and an
        // unrelated codeset does not count.
        assert_eq!(locale_features("fr_CA\nja_JP.eucjp\nen_US.UTF-8\n"), ["locale.en_US.UTF-8"]);
        // Merged into the static set, sorted and unique.
        let set = with_locales(all(), provisioned);
        let mut sorted = set.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(set, sorted);
        assert_eq!(set.len(), all().len() + LOCALES.len());
    }
}
