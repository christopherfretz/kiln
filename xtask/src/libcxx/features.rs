//! The lit feature set of `cargo xtask libcxx` (`spec/libcxx.md` "Feature
//! set").
//!
//! libc++'s lit configuration derives its features at startup by probing the
//! compiler (`libcxx/utils/libcxx/test/{params,features}.py`). This driver
//! does not run lit. What lit reads from the toolchain and is cheap to read
//! without a compiler is derived the same way here ([`detect`]):
//!
//! - the `__config_site` macros of the installed libc++ headers
//!   (`features.py` `macros`; [`config_site_features`]): ABI version,
//!   availability markup, hardening mode and the `no-*` carve-outs
//!   (`no-tzdb`, `no-localization`, `no-threads`, ...), in both the
//!   libc++ 19 spelling (`_LIBCPP_HAS_NO_X` defined) and the libc++ ≥ 20 one
//!   (`_LIBCPP_HAS_X` defined to `0`);
//! - `locale.<name>` for each locale `locale -a` lists ([`locale_features`]);
//! - `has-no-zdump` when `zdump --version` fails (the tzdata validation
//!   tests compare against `zdump`);
//! - from the target triple ([`target_triple`]: `$KILN_LIBCXX_TARGET`, else
//!   kiln's default target, for which the harness passes no `--target`) and
//!   the host ([`target_features`]):
//!   `target=<triple>` (`params.py` `target_triple`), `linux` (`__linux__`
//!   among the compiler's macros) and `buildhost=<os>` (`sys.platform`);
//! - the thread API `__config` settles on ([`thread_api_features`]):
//!   `libcpp-has-thread-api-pthread` / `-external` (`features.py` `macros`,
//!   libc++ 22's `true_false_macros`), which `__config_site` selects or
//!   `__config` computes from the target OS.
//!
//! The libc++ version (`_LIBCPP_VERSION` in `__config`, [`libcxx_version`])
//! is not a lit feature; the harness uses it to gate environment
//! exclusions that only a given release needs (`run.rs`
//! `TZDATA_Z_UNSUPPORTED`).
//!
//! The rest of the set is fixed ([`FEATURES`]): what that configuration
//! derives for libc++ 19.1.7 (Debian `libc++-19-dev`) on Linux with its
//! default parameters, with three deliberate differences:
//!
//! - no compiler-identity features (`clang`, `clang-19`, `gcc`,
//!   `apple-clang`, `msvc`, ...): kiln is none of them, so a
//!   `// XFAIL: clang` must not hide a kiln result, and a
//!   `// UNSUPPORTED: gcc` must not hide a test; `kiln` is added instead.
//!   `gcc-style-warnings` is not an identity: lit derives it for any
//!   compiler that understands `-Wno-<group>` (unlike `cl.exe`), which
//!   kiln does, and libc++ 19 uses it only to gate
//!   `ADDITIONAL_COMPILE_FLAGS(gcc-style-warnings): -Wno-...`;
//! - target capabilities that lit probes by compiling a snippet
//!   (`has-unix-headers`, `has-64-bit-atomics`) are taken as clang 19 sees
//!   this machine, not as kiln does today, so the set does not move when
//!   kiln changes. Probes of compiler *flags* (`has-fconstexpr-steps`,
//!   `-fsized-deallocation`, `verify-support`) are kiln's answers;
//! - `libcpp-hardening-mode=<mode>` is present although lit adds it only
//!   when the `hardening_mode` parameter is set: it is what the installed
//!   `__config_site` builds (`_LIBCPP_HARDENING_MODE_DEFAULT`; Debian:
//!   none), so `.assert.pass.cpp` death tests that lit gates with
//!   `UNSUPPORTED: libcpp-hardening-mode=none` are skipped instead of
//!   failing with "child did not die" (clang++-19 fails them the same way).

use std::path::Path;

/// The C++ standard the suite is run in, as a lit feature and a flag.
pub const STD: &str = "c++26";

/// The target triple the suite is compiled for (the `target=<triple>`
/// feature): `$KILN_LIBCXX_TARGET` ([`cross_target`]) when set, else
/// kiln's default target, `<host arch>-unknown-linux-gnu` (`kiln_driver`
/// `Target`, spec/driver.md "Toolchain"), for which the harness passes no
/// `--target`.
pub fn target_triple() -> String {
    cross_target().unwrap_or_else(host_triple)
}

/// kiln's default target on this host, `<host arch>-unknown-linux-gnu`.
pub fn host_triple() -> String {
    format!("{}-unknown-linux-gnu", std::env::consts::ARCH)
}

/// The environment variable naming the target the suite is compiled for,
/// when it is not the host's default: every compile and link of the run
/// gets `--target=<it>`, the `target=` and thread-API features follow it,
/// and the libc++ package tree's library directory is its
/// [`multiarch_dir`] (spec/libcxx.md "Cross targets").
pub const TARGET_ENV: &str = "KILN_LIBCXX_TARGET";

/// The environment variable naming the executor of a test binary
/// (`%{exec}`, lit's `executor`): whitespace-separated words put before
/// the binary, e.g. `qemu-x86_64 -L /usr/x86_64-linux-gnu`.
pub const EXEC_ENV: &str = "KILN_LIBCXX_EXEC";

/// The environment variable scaling both run-step limits (`--timeout` and
/// `--run-timeout-long`), for an executor that is slower than the host
/// (spec/libcxx.md "Cross targets").
pub const TIMEOUT_SCALE_ENV: &str = "KILN_LIBCXX_TIMEOUT_SCALE";

/// `$KILN_LIBCXX_TARGET` ([`parse_target`]).
pub fn cross_target() -> Option<String> {
    parse_target(std::env::var(TARGET_ENV).ok().as_deref())
}

/// A `KILN_LIBCXX_TARGET` value: `None` when unset, empty or blank, else
/// the value without surrounding whitespace.
pub fn parse_target(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned)
}

/// The words of `$KILN_LIBCXX_EXEC` ([`split_executor`]; empty: run the
/// binary directly).
pub fn executor() -> Vec<String> {
    split_executor(&std::env::var(EXEC_ENV).unwrap_or_default())
}

/// A `KILN_LIBCXX_EXEC` value split at whitespace. No shell quoting: a
/// word that needs a space needs a wrapper script.
pub fn split_executor(raw: &str) -> Vec<String> {
    raw.split_whitespace().map(str::to_owned).collect()
}

/// `$KILN_LIBCXX_TIMEOUT_SCALE` ([`parse_timeout_scale`]; unset: 1).
pub fn timeout_scale() -> Result<f64, String> {
    std::env::var(TIMEOUT_SCALE_ENV).map_or(Ok(1.0), |v| parse_timeout_scale(&v))
}

/// A `KILN_LIBCXX_TIMEOUT_SCALE` value: a finite number greater than 0
/// (blank: 1).
pub fn parse_timeout_scale(raw: &str) -> Result<f64, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(1.0);
    }
    match raw.parse::<f64>() {
        Ok(n) if n.is_finite() && n > 0.0 => Ok(n),
        _ => Err(format!("{TIMEOUT_SCALE_ENV}: expected a number greater than 0, got `{raw}`")),
    }
}

/// The Debian multiarch directory name (`usr/lib/<it>`) of a Linux target
/// triple: `<arch>-linux-<env>` without the vendor, the environment
/// defaulting to `gnu`, `i[3-6]86` spelled `i386` and `arm*` spelled
/// `arm` (`x86_64-unknown-linux-gnu` -> `x86_64-linux-gnu`,
/// `armv7-unknown-linux-gnueabihf` -> `arm-linux-gnueabihf`). `None` for
/// a triple that names no Linux OS.
pub fn multiarch_dir(triple: &str) -> Option<String> {
    let parts: Vec<&str> = triple.split('-').collect();
    let at = parts.iter().position(|p| p.starts_with("linux"))?;
    if at == 0 {
        return None;
    }
    let arch = match parts[0] {
        "i386" | "i486" | "i586" | "i686" => "i386",
        a if a.starts_with("arm") && !a.starts_with("arm64") => "arm",
        a => a,
    };
    let env = parts.get(at + 1).copied().unwrap_or("gnu");
    Some(format!("{arch}-linux-{env}"))
}

/// Whether `triple` needs another library directory than the host's
/// default target: its [`multiarch_dir`] differs from
/// [`host_triple`]'s. `KILN_LIBCXX_TARGET=<the host triple>` is not cross.
pub fn is_cross(triple: &str) -> bool {
    multiarch_dir(triple) != multiarch_dir(&host_triple())
}

/// Every static feature, sorted (see [`all`]).
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
    // libc++ 22's `std-at-least-<s>` for every standard up to `STD`
    // (`params.py` `std`: `_allStandards` with `s <= std`); libc++ 19
    // never names them.
    "std-at-least-c++03",
    "std-at-least-c++11",
    "std-at-least-c++14",
    "std-at-least-c++17",
    "std-at-least-c++20",
    "std-at-least-c++23",
    "std-at-least-c++26",
    // Flags kiln accepts (lit probes them with hasCompileFlag).
    "-fsized-deallocation",
    // `hasCompileFlag("-Wuser-defined-warnings")`: kiln evaluates
    // `diagnose_if` (spec/sema.md). lit's action adds the flag; its
    // warnings are on by default, so the harness need not.
    "diagnose-if-support",
    // kiln understands `-Wno-<group>` (features.py `_isAnyClangOrGCC`);
    // see the module doc.
    "gcc-style-warnings",
    "has-fconstexpr-steps",
    // `hasCompileFlag("-Xclang -verify-ignore-unexpected")` (spec/libcxx.md
    // ".verify.cpp").
    "verify-support",
    // Snippet probes of the target and host (see the module doc).
    "can-create-symlinks",
    "has-64-bit-atomics",
    "has-unix-headers",
    // This compiler.
    "kiln",
];

/// Compiler identities whose `ADDITIONAL_COMPILE_FLAGS(<identity>):` flags
/// apply although the identity is not in the feature set: kiln implements
/// clang's dialect and flags, so a flag libc++ adds for clang
/// (`simd_unary.pass.cpp`'s `-flax-vector-conversions=integer` against the
/// runner's `-flax-vector-conversions=none`, `-Wno-deprecated`) is one kiln
/// needs too. Only the exact name `clang`: not `clang-<N>`,
/// `apple-clang` or `clang-modules-build`. `REQUIRES:` / `UNSUPPORTED:` /
/// `XFAIL:` never see it (spec/libcxx.md "Directives", WP-980).
pub const DIALECT_FLAG_FEATURES: &[&str] = &["clang"];

/// The static feature set plus the [`target_features`] of
/// [`target_triple`], without the ones derived from the headers (see
/// [`detect`]).
pub fn all() -> Vec<String> {
    let mut v: Vec<String> = FEATURES.iter().map(|&f| f.to_owned()).collect();
    v.extend(target_features(&target_triple(), std::env::consts::OS));
    v.sort();
    v
}

/// The OS component of a target triple (`linux` in
/// `aarch64-unknown-linux-gnu`; `<arch>-<vendor>-<os>[-<env>]`, or
/// `<arch>-<os>[-<env>]` when the vendor is omitted).
fn triple_os(triple: &str) -> &str {
    let parts: Vec<&str> = triple.split('-').collect();
    match parts.as_slice() {
        [_, os] | [_, os, "gnu" | "musl" | "android" | "elf" | "eabi"] | [_, _, os, ..] => os,
        _ => "",
    }
}

/// The features lit derives from the target triple and the build host:
/// `target=<triple>` (`params.py` `target_triple`), `linux` for a Linux
/// target (`__linux__`, `features.py`) and `buildhost=<host_os>`
/// (`sys.platform`, which is `linux` on Linux).
pub fn target_features(triple: &str, host_os: &str) -> Vec<String> {
    let mut v = vec![format!("target={triple}"), format!("buildhost={host_os}")];
    if triple_os(triple).starts_with("linux") {
        v.push("linux".to_owned());
    }
    v.sort();
    v
}

/// Target OSes whose threads `__config` maps to pthreads when
/// `__config_site` selects no thread API (`__config` "Thread API":
/// `__linux__`, `__APPLE__`, `__FreeBSD__`, ...; Fuchsia too).
const PTHREAD_OSES: &[&str] = &[
    "linux",
    "darwin",
    "macos",
    "ios",
    "freebsd",
    "netbsd",
    "openbsd",
    "wasi",
    "nuttx",
    "hurd",
    "zos",
    "aix",
    "emscripten",
    "fuchsia",
];

/// The thread-API features of a `__config_site` text for `triple`, as
/// `__config` settles the API and lit reads it: nothing without threads
/// (`_LIBCPP_HAS_NO_THREADS` defined, or `_LIBCPP_HAS_THREADS 0`); the API
/// `__config_site` selects (libc++ 19: `_LIBCPP_HAS_THREAD_API_<X>`
/// defined; libc++ ≥ 20: defined to `1`); else pthreads on the
/// [`PTHREAD_OSES`]. lit maps `PTHREAD` to `libcpp-has-thread-api-pthread`
/// and `EXTERNAL` to `libcpp-has-thread-api-external`.
pub fn thread_api_features(site: &str, triple: &str) -> Vec<String> {
    let mut threads = true;
    let mut selected = None;
    for (name, value) in defines(site) {
        match name {
            "_LIBCPP_HAS_NO_THREADS" => threads = false,
            "_LIBCPP_HAS_THREADS" if value == "0" => threads = false,
            _ => {}
        }
        if let Some(api) = name.strip_prefix("_LIBCPP_HAS_THREAD_API_")
            && (value.is_empty() || value == "1")
        {
            selected.get_or_insert(api);
        }
    }
    if !threads {
        return Vec::new();
    }
    let os = triple_os(triple);
    let api =
        selected.or_else(|| PTHREAD_OSES.iter().any(|p| os.starts_with(p)).then_some("PTHREAD"));
    match api {
        Some("PTHREAD") => vec!["libcpp-has-thread-api-pthread".to_owned()],
        Some("EXTERNAL") => vec!["libcpp-has-thread-api-external".to_owned()],
        _ => Vec::new(),
    }
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

/// The feature set of this machine and of the libc++ headers in `include`
/// (the root the run uses: the system install or `.toolchain/libcxx-N-root`):
/// [`all`], the [`config_site_features`] and [`thread_api_features`] of
/// `include/__config_site`, `has-no-zdump` when `zdump --version` fails, and
/// `locale.<name>` for each of [`LOCALES`] that `locale -a` lists.
/// `locale -a` and `zdump` are the host's: with `$KILN_LIBCXX_EXEC` the
/// test binary is taken to see the host's file system (qemu user
/// emulation does), so its locales and time zones are the host's too.
pub fn detect(include: &Path) -> Vec<String> {
    let listed = std::process::Command::new("locale")
        .arg("-a")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut set = all();
    let site = std::fs::read_to_string(include.join("__config_site")).unwrap_or_default();
    set.extend(config_site_features(&site));
    set.extend(thread_api_features(&site, &target_triple()));
    let zdump = std::process::Command::new("zdump")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !zdump {
        set.push("has-no-zdump".to_owned());
    }
    with_locales(set, &listed)
}

/// `_LIBCPP_VERSION` of the libc++ headers in `include` (`190107` for
/// 19.1.7), from `include/__config`.
pub fn libcxx_version(include: &Path) -> Option<u32> {
    parse_libcxx_version(&std::fs::read_to_string(include.join("__config")).ok()?)
}

/// The `_LIBCPP_VERSION` a `__config` text defines.
pub fn parse_libcxx_version(config: &str) -> Option<u32> {
    defines(config).find(|(m, _)| *m == "_LIBCPP_VERSION").and_then(|(_, v)| v.parse().ok())
}

/// The libc++ major release of a `_LIBCPP_VERSION` (`190107` is 19).
pub fn libcxx_major(version: u32) -> u32 {
    version / 10000
}

/// The `#define NAME [VALUE]` lines of a header (any indentation after `#`).
fn defines(text: &str) -> impl Iterator<Item = (&str, &str)> {
    text.lines().filter_map(|l| {
        let rest = l.trim_start().strip_prefix('#')?.trim_start().strip_prefix("define")?;
        if !rest.starts_with([' ', '\t']) {
            return None;
        }
        let rest = rest.trim();
        let (name, value) = rest.split_once([' ', '\t']).unwrap_or((rest, ""));
        Some((name, value.split("//").next().unwrap_or("").trim()))
    })
}

/// `__config_site` macros whose presence is a lit feature: libc++ 19
/// defines `_LIBCPP_HAS_NO_<X>` (`features.py` `macros`); libc++ ≥ 20
/// defines `_LIBCPP_HAS_<X>` to `0` or `1` and lit adds the feature for
/// `0` (`inverted_macros`).
const CARVE_OUTS: &[(&str, &str, &str)] = &[
    ("_LIBCPP_HAS_NO_FILESYSTEM", "_LIBCPP_HAS_FILESYSTEM", "no-filesystem"),
    ("_LIBCPP_HAS_NO_LOCALIZATION", "_LIBCPP_HAS_LOCALIZATION", "no-localization"),
    ("_LIBCPP_HAS_NO_MONOTONIC_CLOCK", "_LIBCPP_HAS_MONOTONIC_CLOCK", "no-monotonic-clock"),
    ("_LIBCPP_HAS_NO_RANDOM_DEVICE", "_LIBCPP_HAS_RANDOM_DEVICE", "no-random-device"),
    ("_LIBCPP_HAS_NO_THREADS", "_LIBCPP_HAS_THREADS", "no-threads"),
    ("_LIBCPP_HAS_NO_TIME_ZONE_DATABASE", "_LIBCPP_HAS_TIME_ZONE_DATABASE", "no-tzdb"),
    ("_LIBCPP_HAS_NO_UNICODE", "_LIBCPP_HAS_UNICODE", "libcpp-has-no-unicode"),
    (
        "_LIBCPP_HAS_NO_VENDOR_AVAILABILITY_ANNOTATIONS",
        "_LIBCPP_HAS_VENDOR_AVAILABILITY_ANNOTATIONS",
        "libcpp-has-no-availability-markup",
    ),
    ("_LIBCPP_HAS_NO_WIDE_CHARACTERS", "_LIBCPP_HAS_WIDE_CHARACTERS", "no-wide-characters"),
];

/// The lit features a `__config_site` text implies: [`CARVE_OUTS`],
/// `libcpp-abi-version=<n>` (`_LIBCPP_ABI_VERSION`) and
/// `libcpp-hardening-mode=<mode>` (`_LIBCPP_HARDENING_MODE_DEFAULT`, the
/// `__config` values `1 << 1` none, `1 << 2` fast, `1 << 4` extensive,
/// `1 << 3` debug, as numbers or by name).
pub fn config_site_features(site: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (name, value) in defines(site) {
        if let Some((.., f)) =
            CARVE_OUTS.iter().find(|(old, new, _)| name == *old || (name == *new && value == "0"))
        {
            out.push((*f).to_owned());
        }
        match name {
            "_LIBCPP_ABI_VERSION" if !value.is_empty() => {
                out.push(format!("libcpp-abi-version={value}"));
            }
            "_LIBCPP_HARDENING_MODE_DEFAULT" => {
                let mode = match value {
                    "2" | "(1 << 1)" | "_LIBCPP_HARDENING_MODE_NONE" => "none",
                    "4" | "(1 << 2)" | "_LIBCPP_HARDENING_MODE_FAST" => "fast",
                    "16" | "(1 << 4)" | "_LIBCPP_HARDENING_MODE_EXTENSIVE" => "extensive",
                    "8" | "(1 << 3)" | "_LIBCPP_HARDENING_MODE_DEBUG" => "debug",
                    _ => continue,
                };
                out.push(format!("libcpp-hardening-mode={mode}"));
            }
            _ => {}
        }
    }
    out.sort();
    out.dedup();
    out
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
        assert!(all.contains(&format!("target={}", target_triple())));
        assert!(all.iter().any(|f| f == STD));
        assert!(all.iter().any(|f| f == "kiln"));
        assert!(all.iter().any(|f| f == "verify-support"));
        assert!(all.iter().any(|f| *f == format!("std-at-least-{STD}")));
        for f in &all {
            for bad in ["clang", "gcc", "msvc", "apple-clang"] {
                assert!(
                    f == "gcc-style-warnings" || (f != bad && !f.starts_with(&format!("{bad}-"))),
                    "compiler-identity feature `{f}` in the static set"
                );
            }
        }
        assert!(!all.iter().any(|f| f.starts_with("locale.")), "locales are detected");
        for derived in ["libcpp-abi-version", "libcpp-hardening-mode", "no-", "has-no-zdump"] {
            assert!(!all.iter().any(|f| f.starts_with(derived)), "{derived} is derived");
        }
    }

    /// libc++ 19.1.7's Debian `__config_site` (abridged) gives exactly what
    /// the static set held before the derivation.
    #[test]
    fn config_site_19() {
        let site = "#ifndef _LIBCPP___CONFIG_SITE\n#define _LIBCPP___CONFIG_SITE\n\n\
                    #define _LIBCPP_ABI_VERSION 1\n#define _LIBCPP_ABI_NAMESPACE __1\n\
                    /* #undef _LIBCPP_HAS_NO_THREADS */\n\
                    #define _LIBCPP_HAS_NO_VENDOR_AVAILABILITY_ANNOTATIONS\n\
                    /* #undef _LIBCPP_HAS_NO_TIME_ZONE_DATABASE */\n\
                    #define _LIBCPP_PSTL_BACKEND_STD_THREAD\n\
                    #define _LIBCPP_HARDENING_MODE_DEFAULT 2\n#endif\n";
        assert_eq!(
            config_site_features(site),
            [
                "libcpp-abi-version=1",
                "libcpp-hardening-mode=none",
                "libcpp-has-no-availability-markup"
            ]
        );
        let config = "#ifndef _LIBCPP___CONFIG\n#  define _LIBCPP_VERSION 190107\n";
        assert_eq!(parse_libcxx_version(config), Some(190_107));
        assert_eq!(libcxx_major(190_107), 19);
        assert_eq!(parse_libcxx_version("#define _LIBCPP_VERSIONX 1\n"), None);
    }

    #[test]
    fn target_features_from_the_triple() {
        assert_eq!(
            target_features("aarch64-unknown-linux-gnu", "linux"),
            ["buildhost=linux", "linux", "target=aarch64-unknown-linux-gnu"]
        );
        assert_eq!(
            target_features("x86_64-pc-linux-gnu", "linux"),
            ["buildhost=linux", "linux", "target=x86_64-pc-linux-gnu"]
        );
        assert_eq!(target_features("x86_64-linux-gnu", "linux")[1], "linux");
        assert_eq!(
            target_features("arm64-apple-macosx14.0", "darwin"),
            ["buildhost=darwin", "target=arm64-apple-macosx14.0"]
        );
        assert_eq!(
            target_features("x86_64-pc-windows-msvc", "linux"),
            ["buildhost=linux", "target=x86_64-pc-windows-msvc"]
        );
        let t = host_triple();
        assert!(t.ends_with("-unknown-linux-gnu") && t.starts_with(std::env::consts::ARCH));
    }

    /// `KILN_LIBCXX_TARGET`, `KILN_LIBCXX_EXEC` and
    /// `KILN_LIBCXX_TIMEOUT_SCALE` values (spec/libcxx.md "Cross targets").
    #[test]
    fn cross_target_environment() {
        assert_eq!(parse_target(None), None);
        assert_eq!(parse_target(Some("")), None);
        assert_eq!(parse_target(Some("  ")), None);
        assert_eq!(
            parse_target(Some(" x86_64-unknown-linux-gnu\n")).as_deref(),
            Some("x86_64-unknown-linux-gnu")
        );
        assert_eq!(split_executor(""), Vec::<String>::new());
        assert_eq!(split_executor(" \t"), Vec::<String>::new());
        assert_eq!(
            split_executor("qemu-x86_64  -L\t/usr/x86_64-linux-gnu "),
            ["qemu-x86_64", "-L", "/usr/x86_64-linux-gnu"]
        );
        assert_eq!(parse_timeout_scale(""), Ok(1.0));
        assert_eq!(parse_timeout_scale(" 10 "), Ok(10.0));
        assert_eq!(parse_timeout_scale("2.5"), Ok(2.5));
        for bad in ["0", "-1", "x", "inf", "NaN"] {
            assert!(parse_timeout_scale(bad).is_err(), "{bad}");
        }
    }

    /// Debian's multiarch directory of a target triple.
    #[test]
    fn multiarch_directories() {
        let m = |t: &str| multiarch_dir(t);
        assert_eq!(m("x86_64-unknown-linux-gnu").as_deref(), Some("x86_64-linux-gnu"));
        assert_eq!(m("aarch64-linux-gnu").as_deref(), Some("aarch64-linux-gnu"));
        assert_eq!(m("riscv64-unknown-linux-gnu").as_deref(), Some("riscv64-linux-gnu"));
        assert_eq!(m("i686-pc-linux-gnu").as_deref(), Some("i386-linux-gnu"));
        assert_eq!(m("armv7-unknown-linux-gnueabihf").as_deref(), Some("arm-linux-gnueabihf"));
        assert_eq!(m("x86_64-linux").as_deref(), Some("x86_64-linux-gnu"));
        assert_eq!(m("x86_64-unknown-linux-musl").as_deref(), Some("x86_64-linux-musl"));
        assert_eq!(m("arm64-apple-macosx14.0"), None);
        assert_eq!(m("linux-gnu"), None);
        assert_eq!(m(""), None);
        assert!(!is_cross(&host_triple()));
        assert!(!is_cross(&format!("{}-pc-linux-gnu", std::env::consts::ARCH)));
        let other = if std::env::consts::ARCH == "x86_64" { "aarch64" } else { "x86_64" };
        assert!(is_cross(&format!("{other}-unknown-linux-gnu")));
    }

    /// libc++ 19 leaves the thread API to `__config` (`/* #undef ... */`),
    /// which picks pthreads on Linux; libc++ 22 writes `0` for every API
    /// and `__config` picks again.
    #[test]
    fn thread_api_from_config_site_and_target() {
        let linux = "aarch64-unknown-linux-gnu";
        let site19 = "/* #undef _LIBCPP_HAS_NO_THREADS */\n\
                      /* #undef _LIBCPP_HAS_THREAD_API_PTHREAD */\n\
                      /* #undef _LIBCPP_HAS_THREAD_API_EXTERNAL */\n";
        assert_eq!(thread_api_features(site19, linux), ["libcpp-has-thread-api-pthread"]);
        let site22 = "#define _LIBCPP_HAS_THREADS 1\n\
                      #define _LIBCPP_HAS_THREAD_API_PTHREAD 0\n\
                      #define _LIBCPP_HAS_THREAD_API_EXTERNAL 0\n\
                      #define _LIBCPP_HAS_THREAD_API_WIN32 0\n\
                      #define _LIBCPP_HAS_THREAD_API_C11 0 // FIXME\n";
        assert_eq!(thread_api_features(site22, linux), ["libcpp-has-thread-api-pthread"]);
        assert_eq!(
            thread_api_features(site22, "x86_64-pc-linux-gnu"),
            ["libcpp-has-thread-api-pthread"]
        );
        // No threads, an explicit API, an OS `__config` gives no pthreads.
        assert_eq!(
            thread_api_features("#define _LIBCPP_HAS_NO_THREADS\n", linux),
            Vec::<String>::new()
        );
        assert_eq!(
            thread_api_features("#define _LIBCPP_HAS_THREADS 0\n", linux),
            Vec::<String>::new()
        );
        assert_eq!(
            thread_api_features("#define _LIBCPP_HAS_THREAD_API_EXTERNAL\n", linux),
            ["libcpp-has-thread-api-external"]
        );
        assert_eq!(
            thread_api_features("#define _LIBCPP_HAS_THREAD_API_EXTERNAL 1\n", linux),
            ["libcpp-has-thread-api-external"]
        );
        assert_eq!(thread_api_features("", "x86_64-pc-windows-msvc"), Vec::<String>::new());
    }

    /// libc++ ≥ 20 spells the carve-outs as `_LIBCPP_HAS_<X> 0|1`.
    #[test]
    fn config_site_20_spelling() {
        let site = "#define _LIBCPP_ABI_VERSION 2\n#define _LIBCPP_HAS_THREADS 1\n\
                    #define _LIBCPP_HAS_TIME_ZONE_DATABASE 0\n\
                    #define _LIBCPP_HAS_VENDOR_AVAILABILITY_ANNOTATIONS 0\n\
                    #define _LIBCPP_HAS_LOCALIZATION 1\n\
                    #define _LIBCPP_HARDENING_MODE_DEFAULT _LIBCPP_HARDENING_MODE_NONE\n";
        assert_eq!(
            config_site_features(site),
            [
                "libcpp-abi-version=2",
                "libcpp-hardening-mode=none",
                "libcpp-has-no-availability-markup",
                "no-tzdb"
            ]
        );
        assert_eq!(
            config_site_features("#define _LIBCPP_HAS_NO_LOCALIZATION\n"),
            ["no-localization"]
        );
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
