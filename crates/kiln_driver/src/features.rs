//! The `AArch64` target features of `-march=`, `-mcpu=`,
//! `-msve-vector-bits=` and `-target-feature` (spec/driver.md "Target
//! features"), and the x86-64 ones of `-march=<cpu>` and `target("...")`
//! (spec/landed/driver/WP-630.md).
//!
//! `-march=<arch>[+ext...]` names an architecture level (`armv8-a` ..
//! `armv8.9-a`, `armv9-a` .. `armv9.5-a`, and `armv9.6-a`/`armv9.7-a` at
//! `-fclang-compat=22`; also spelt without the `-`, case-insensitively)
//! whose default features are on, then applies each `+ext` / `+noext` in
//! order as LLVM's `AArch64::ExtensionSet` does: enabling a feature
//! enables what it depends on, disabling an enabled one disables what
//! depends on it, and every feature the level or an extension touched is
//! listed (`+name` when finally on, `-name` when off).
//! `-mcpu=generic[+ext...]` is `armv8-a` plus the `ete` and `trbe`
//! features. `-march=` wins over `-mcpu=` (whose extensions are then
//! ignored, as clang's are); the `-target-feature` list applies after
//! both, the last naming a feature winning, with no dependency closure
//! (as clang's `-cc1`). Any other architecture, CPU or extension is an
//! unsupported value. The levels and the dependency graph are clang 19's
//! or clang 22's by `-fclang-compat=` (measured with
//! `clang-N -S -emit-llvm -march=...`).

use std::collections::{BTreeMap, BTreeSet};

use kiln_pp::ClangCompat;

use crate::args::{ArgError, Args};
use crate::toolchain::Target;
use crate::x86_features;

/// The resolved target features beyond the target's base set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TargetFeatures {
    /// `sve` is finally on: the SVE types are usable and function
    /// definitions carry `vscale_range(1,16)`.
    pub sve: bool,
    /// The `+name` / `-name` entries for the functions' `target-features`
    /// beyond the base `+fp-armv8,+neon,+v8a`, one per name (the last
    /// wins): the architecture level's and the extensions' features in
    /// name order, then the `-target-feature` ones in order.
    pub extra: Vec<String>,
    /// The architecture version `(major, minor)`: `(8, 0)` for `armv8-a`,
    /// `generic` and no option.
    pub arch: (u8, u8),
    /// On x86-64, `extra` is instead the `-march=<cpu>` CPU's features as
    /// `+name`, sorted (empty without `-march=`, for `native`, or for a
    /// CPU clang 21 does not know: the default `x86-64`'s; WP-630).
    /// `-msve-vector-bits=N` (128 .. 2048, a power of two); `None` for
    /// `scalable` or no option.
    pub sve_vector_bits: Option<u32>,
    /// The CPU the backend generates code for: `native` for
    /// `-march=native` / `-mcpu=native` (both targets; the backend's
    /// `-mcpu=native` detects the host, kiln does not), or the x86
    /// `-march=<cpu>`; `None` for the default (spec/driver.md "Codegen
    /// flags").
    pub cpu: Option<String>,
}

impl TargetFeatures {
    fn set(&mut self, name: &str, on: bool) {
        let entry = format!("{}{name}", if on { '+' } else { '-' });
        match self.extra.iter_mut().find(|e| &e[1..] == name) {
            Some(e) => *e = entry,
            None => self.extra.push(entry),
        }
        if name == "sve" {
            self.sve = on;
        }
    }

    /// Whether the x86-64 `-march=` CPU has `cx16`: clang
    /// (`X86TargetInfo::setMaxAtomicWidth`) then inlines 128-bit atomics
    /// (WP-630).
    pub fn x86_cx16(&self) -> bool {
        self.extra.iter().any(|f| f == "+cx16")
    }

    /// The x86-64 features finally enabled, sorted ([`TargetFeatures::extra`],
    /// or the default `x86-64` CPU's).
    pub fn x86_enabled(&self) -> Vec<String> {
        match self.x86_map() {
            Some(m) => m.into_iter().filter(|e| e.1).map(|e| e.0).collect(),
            None => X86_64_FEATURES.iter().map(|&f| f.to_owned()).collect(),
        }
    }

    /// The x86-64 feature map [`TargetFeatures::extra`] lists, if it lists
    /// one (a `-march=` CPU or feature flags; not only the
    /// `-mspeculative-load-hardening` retpoline feature).
    pub fn x86_map(&self) -> Option<BTreeMap<String, bool>> {
        let option = |e: &String| e == "+retpoline-indirect-calls";
        if self.extra.iter().all(option) {
            return None;
        }
        let entries = self.extra.iter().filter(|e| !option(e));
        Some(entries.filter_map(|e| e.split_at_checked(1)).map(|(s, n)| (n.to_owned(), s == "+")).collect())
    }

    /// The finally enabled feature names: the base `fp-armv8` and `neon`
    /// with [`TargetFeatures::extra`] applied (the `__ARM_*` predefines
    /// derive from them; spec/pp.md "Predefines").
    pub fn enabled(&self) -> Vec<String> {
        let mut on: BTreeSet<&str> = ["fp-armv8", "neon"].into_iter().collect();
        for e in &self.extra {
            match e.split_at(1) {
                ("+", n) => on.insert(n),
                (_, n) => on.remove(n),
            };
        }
        on.into_iter().map(str::to_owned).collect()
    }
}

/// The features `armv8.N-a` adds to `armv8.(N-1)-a` (the `v8.Na` name is
/// the level's own).
fn v8_level(n: u8, v22: bool) -> &'static [&'static str] {
    match n {
        1 => &["crc", "lse", "rdm", "v8.1a"],
        2 => &["ras", "v8.2a"],
        3 => &["ccidx", "complxnum", "jsconv", "pauth", "rcpc", "v8.3a"],
        4 => &["dit", "dotprod", "flagm", "v8.4a"],
        5 => &["bti", "predres", "sb", "ssbs", "v8.5a"],
        6 => &["bf16", "i8mm", "v8.6a"],
        7 if v22 => &["spe-eef", "v8.7a", "wfxt"],
        7 => &["v8.7a", "wfxt"],
        8 => &["hbc", "mops", "v8.8a"],
        9 => &["cssc", "rasv2", "specres2", "v8.9a"],
        _ => &[],
    }
}

/// The features `armv9.N-a` adds to `armv9.(N-1)-a` beyond the `armv8`
/// level it includes (`armv9.N-a` includes `armv8.(N+5)-a`, up to 8.9).
fn v9_level(n: u8, v22: bool) -> &'static [&'static str] {
    match n {
        0 => &["fullfp16", "sve", "sve2", "v9a"],
        1 => &["rme", "v9.1a"],
        2 => &["mec", "v9.2a"],
        3 => &["v9.3a"],
        4 if v22 => &["sve2p1", "v9.4a"],
        4 => &["v9.4a"],
        5 => &["cpa", "faminmax", "lut", "v9.5a"],
        6 => &["cmpbr", "lsui", "occmo", "v9.6a"],
        7 => &["f16f32dot", "fprcvt", "sve2p2", "sve2p3", "v9.7a"],
        _ => &[],
    }
}

/// The architecture level of a lower-case `-march=` base name.
fn parse_arch(s: &str, v22: bool) -> Option<(u8, u8)> {
    let s = s.strip_prefix("armv")?;
    let s = s.strip_suffix("-a").or_else(|| s.strip_suffix('a'))?;
    let (major, minor) = match s.split_once('.') {
        None => (s, 0),
        Some((m, n)) => match n.as_bytes() {
            [d @ b'1'..=b'9'] => (m, d - b'0'),
            _ => return None,
        },
    };
    match major {
        "8" => Some((8, minor)),
        "9" if minor <= if v22 { 7 } else { 5 } => Some((9, minor)),
        _ => None,
    }
}

/// The LLVM feature an `-march=` extension name enables or (as `no<name>`)
/// disables.
fn extension(name: &str, v22: bool) -> Option<&'static str> {
    Some(match name {
        "fp" => "fp-armv8",
        "simd" => "neon",
        "fp16" => "fullfp16",
        "rdm" | "rdma" => "rdm",
        "jscvt" => "jsconv",
        "fcma" => "complxnum",
        "memtag" => "mte",
        "rng" => "rand",
        "tme" if !v22 => "tme",
        "f16f32dot" | "fprcvt" | "sve-aes" | "sve-aes2" | "sve-b16b16" | "sve-bitperm"
        | "sve-f16f32mm" | "sve-sha3" | "sve-sm4" | "sve2p2" | "sve2p3"
            if v22 =>
        {
            *[
                "f16f32dot",
                "fprcvt",
                "sve-aes",
                "sve-aes2",
                "sve-b16b16",
                "sve-bitperm",
                "sve-f16f32mm",
                "sve-sha3",
                "sve-sm4",
                "sve2p2",
                "sve2p3",
            ]
            .iter()
            .find(|n| **n == name)?
        }
        _ => *[
            "aes",
            "bf16",
            "bti",
            "crc",
            "crypto",
            "cssc",
            "d128",
            "dotprod",
            "f32mm",
            "f64mm",
            "faminmax",
            "flagm",
            "fp16fml",
            "gcs",
            "i8mm",
            "ls64",
            "lse",
            "lse128",
            "lut",
            "mops",
            "pauth",
            "predres",
            "ras",
            "rcpc",
            "rcpc3",
            "sb",
            "sha2",
            "sha3",
            "sm4",
            "ssbs",
            "sve",
            "sve2",
            "sve2-aes",
            "sve2-bitperm",
            "sve2-sha3",
            "sve2-sm4",
            "sve2p1",
            "the",
        ]
        .iter()
        .find(|n| **n == name)?,
    })
}

/// Splits `base+ext+...` into the base and the `+`-separated extensions.
fn split(v: &str) -> (String, &str) {
    v.split_once('+').map_or((v.to_owned(), ""), |(b, e)| (b.to_owned(), e))
}

/// The features feature `f` depends on (LLVM `ExtensionDependencies`):
/// enabling `f` enables them, disabling one of them disables `f`.
fn deps(f: &str, v22: bool) -> &'static [&'static str] {
    match f {
        "neon" | "jsconv" | "fullfp16" | "fprcvt" => &["fp-armv8"],
        "complxnum" | "dotprod" | "rdm" | "aes" | "sha2" | "sm4" => &["neon"],
        "bf16" | "i8mm" | "lut" | "faminmax" if v22 => &["neon"],
        "fp16fml" | "f16f32dot" if v22 => &["fullfp16", "neon"],
        "sha3" => &["sha2"],
        "crypto" => &["aes", "sha2"],
        "sve" | "fp16fml" => &["fullfp16"],
        "sve2" | "f32mm" | "f64mm" | "sve-f16f32mm" => &["sve"],
        "sve2p2" => &["sve2p1"],
        "sve2p3" => &["sve2p2"],
        // clang 22 splits each `sve2-x` into `sve-x` and `sve2`.
        "sve-aes" => &["aes"],
        "sve-sha3" => &["sha3"],
        "sve-sm4" => &["sm4"],
        "sve2-aes" if v22 => &["sve-aes", "sve2"],
        "sve2-sha3" if v22 => &["sve-sha3", "sve2"],
        "sve2-sm4" if v22 => &["sve-sm4", "sve2"],
        "sve2-bitperm" if v22 => &["sve-bitperm", "sve2"],
        "sve2-aes" => &["aes", "sve2"],
        "sve2-sha3" => &["sha3", "sve2"],
        "sve2-sm4" => &["sm4", "sve2"],
        "sve2-bitperm" | "sve2p1" => &["sve2"],
        "rcpc-immo" => &["rcpc"],
        "rcpc3" => &["rcpc-immo"],
        "lse128" => &["lse"],
        "d128" => &["lse128"],
        "rasv2" => &["ras"],
        "specres2" => &["predres"],
        "gcs" => &["chk"],
        _ => &[],
    }
}

/// Every feature with a dependency (the candidates of a disable cascade).
const DEPENDENT: &[&str] = &[
    "aes",
    "bf16",
    "complxnum",
    "crypto",
    "d128",
    "dotprod",
    "f16f32dot",
    "f32mm",
    "f64mm",
    "faminmax",
    "fprcvt",
    "fp16fml",
    "fullfp16",
    "gcs",
    "i8mm",
    "jsconv",
    "lse128",
    "lut",
    "neon",
    "rasv2",
    "rcpc-immo",
    "rcpc3",
    "rdm",
    "sha2",
    "sha3",
    "sm4",
    "specres2",
    "sve",
    "sve-aes",
    "sve-f16f32mm",
    "sve-sha3",
    "sve-sm4",
    "sve2",
    "sve2-aes",
    "sve2-bitperm",
    "sve2-sha3",
    "sve2-sm4",
    "sve2p1",
    "sve2p2",
    "sve2p3",
];

/// LLVM's `AArch64::ExtensionSet` over one base architecture.
struct ExtensionSet {
    arch: (u8, u8),
    v22: bool,
    enabled: BTreeSet<&'static str>,
    touched: BTreeSet<&'static str>,
}

impl ExtensionSet {
    /// The set of architecture level `arch` with its default features on.
    fn new(arch: (u8, u8), v22: bool) -> ExtensionSet {
        let mut s = ExtensionSet { arch, v22, enabled: BTreeSet::new(), touched: BTreeSet::new() };
        let v8 = if arch.0 == 9 { (arch.1 + 5).min(9) } else { arch.1 };
        let mut level: Vec<&'static str> = vec!["fp-armv8", "neon"];
        level.extend((1..=v8).flat_map(|n| v8_level(n, v22)));
        if arch.0 == 9 {
            level.extend((0..=arch.1).flat_map(|n| v9_level(n, v22)));
        }
        for f in level {
            s.enable(f);
        }
        s
    }

    fn enable(&mut self, f: &'static str) {
        if !self.enabled.insert(f) {
            return;
        }
        self.touched.insert(f);
        for &d in deps(f, self.v22) {
            self.enable(d);
        }
        // `+crypto` from v8.4 on also enables `sha3` and `sm4`; `+fp16`
        // implies `fp16fml` on v8.4 .. v8.9.
        let v84 = self.arch >= (8, 4);
        if f == "crypto" && v84 {
            self.enable("sha3");
            self.enable("sm4");
        }
        if f == "fullfp16" && v84 && self.arch.0 == 8 {
            self.enable("fp16fml");
        }
    }

    fn disable(&mut self, f: &'static str) {
        // `-crypto` always disables `aes`, `sha2`, `sha3` and `sm4`; clang
        // 22's `-sve2-x` also disables its `sve-x` (and `-sve2-bitperm`
        // `sve2`).
        let also: &[&'static str] = match f {
            "crypto" => &["aes", "sha2", "sha3", "sm4"],
            "sve2-aes" if self.v22 => &["sve-aes"],
            "sve2-sha3" if self.v22 => &["sve-sha3"],
            "sve2-sm4" if self.v22 => &["sve-sm4"],
            "sve2-bitperm" if self.v22 => &["sve-bitperm", "sve2"],
            _ => &[],
        };
        for &a in also {
            self.disable(a);
        }
        if !self.enabled.remove(f) {
            return;
        }
        self.touched.insert(f);
        for &d in DEPENDENT {
            if deps(d, self.v22).contains(&f) {
                self.disable(d);
            }
        }
    }

    /// Applies the `+ext` list of `-march=`/`-mcpu=` (case-insensitive,
    /// as clang's), or returns `None` for an extension kiln does not know.
    fn extensions(&mut self, exts: &str) -> Option<()> {
        for e in exts.split('+').filter(|e| !e.is_empty()) {
            let e = e.to_ascii_lowercase();
            match e.strip_prefix("no").and_then(|n| extension(n, self.v22)) {
                Some(f) => self.disable(f),
                None => self.enable(extension(&e, self.v22)?),
            }
        }
        Some(())
    }

    /// The touched features as `+name` / `-name`, minus the base set.
    fn into_features(self, f: &mut TargetFeatures) {
        f.arch = self.arch;
        for &t in &self.touched {
            let on = self.enabled.contains(t);
            if !(on && ["fp-armv8", "neon", "v8a"].contains(&t)) {
                f.set(t, on);
            }
        }
    }
}

/// Resolves the target features of `args` for `target`. Off `AArch64`,
/// `-march=`/`-mcpu=` are unsupported values, `-msve-vector-bits=` is an
/// unsupported option and `-target-feature` is ignored. Every error is
/// returned; the features are those of the valid options.
pub fn resolve(args: &Args, target: &Target) -> (TargetFeatures, Vec<ArgError>) {
    let mut f = TargetFeatures { arch: (8, 0), ..TargetFeatures::default() };
    let mut errors = Vec::new();
    let unsupported = |opt: &str, v: &str| ArgError::UnsupportedValue(opt.to_owned(), v.to_owned());
    let native = |v: &Option<String>| v.as_deref() == Some("native");
    if !target.is_aarch64() {
        // x86: `-march=<cpu>` names one of clang's CPUs (`getX86TargetCPU`)
        // or `native`. A CPU clang 21 knows has its features
        // (`x86_targets::CPUS`, with clang 19's / 22's differences);
        // `native` on a Linux x86-64 host has the host's `/proc/cpuinfo`
        // features, elsewhere none (spec/driver.md "x86-64 target
        // features"). Any other value stays unsupported.
        let v22 = args.predef.clang_compat == ClangCompat::Clang22;
        let mut cpu_features: Option<Vec<&'static str>> = None;
        match args.march.as_deref() {
            Some("native") => {
                f.cpu = Some("native".to_owned());
                if std::env::consts::ARCH == "x86_64" {
                    cpu_features = std::fs::read_to_string("/proc/cpuinfo")
                        .ok()
                        .map(|c| x86_features::native_features(&c, v22));
                }
            }
            Some(v) if is_x86_cpu(v, v22) => {
                f.cpu = Some(v.to_owned());
                cpu_features = x86_cpu(v).map(|c| x86_features::cpu_features(c, v22));
            }
            Some(v) => errors.push(unsupported("-march=", v)),
            None => {}
        }
        // WP-1000: the `-m[no-]<feature>` flags this clang lacks are
        // unknown arguments; the others, deduplicated as clang's driver
        // does, build clang's `initFeatureMap` over the CPU's features
        // (spec/driver.md "x86-64 target features"). `-Xclang
        // -target-feature` stays ignored on x86-64.
        for a in &args.x86_feature_flags {
            if x86_features::unknown_at(a, v22) {
                errors.push(ArgError::Unknown(a.clone()));
            } else if let Some((opt, v)) = x86_features::bad_value(a) {
                errors.push(unsupported(&opt, &v));
            }
        }
        let items = x86_features::driver_items(&args.x86_feature_flags, v22);
        if cpu_features.is_some() || !items.is_empty() {
            let cpu = cpu_features.unwrap_or_else(|| X86_64_FEATURES.to_vec());
            let map = x86_features::feature_map(&cpu, &items, v22);
            f.extra = map.into_iter().map(|(n, on)| format!("{}{n}", if on { '+' } else { '-' })).collect();
        }
        // WP-813: clang's `getX86TargetFeatures` adds the retpoline call
        // thunks for `-mspeculative-load-hardening`.
        if args.codegen.speculative_load_hardening {
            f.extra.push("+retpoline-indirect-calls".to_owned());
        }
        if args.mcpu.is_some() {
            let t = args.target.as_deref().unwrap_or(target.triple);
            errors.push(ArgError::UnsupportedForTarget("-mcpu=".to_owned(), t.into()));
        }
        if args.sve_vector_bits.is_some() {
            let t = args.target.as_deref().unwrap_or(target.triple);
            errors.push(ArgError::UnsupportedForTarget("-msve-vector-bits=".to_owned(), t.into()));
        }
        // WP-846: clang's target-specific `-mbranch-protection=`.
        if args.codegen.mbranch_protection_given {
            let t = args.target.as_deref().unwrap_or(target.triple);
            errors
                .push(ArgError::UnsupportedForTarget("-mbranch-protection=".to_owned(), t.into()));
        }
        return (f, errors);
    }
    let v22 = args.predef.clang_compat == ClangCompat::Clang22;
    if native(&args.march) || (args.march.is_none() && native(&args.mcpu)) {
        f.cpu = Some("native".to_owned());
    } else if let Some(v) = &args.march {
        let (base, exts) = split(v);
        let set = parse_arch(&base.to_ascii_lowercase(), v22).and_then(|arch| {
            let mut s = ExtensionSet::new(arch, v22);
            s.extensions(exts).map(|()| s)
        });
        match set {
            Some(s) => s.into_features(&mut f),
            None => errors.push(unsupported("-march=", v)),
        }
    } else if let Some(v) = &args.mcpu {
        let (base, exts) = split(v);
        let mut s = ExtensionSet::new((8, 0), v22);
        if base.eq_ignore_ascii_case("generic") && s.extensions(exts).is_some() {
            // `generic` is clang's `-mcpu=` default CPU plus the `ete` and
            // `trbe` features it names explicitly.
            f.set("ete", true);
            f.set("trbe", true);
            s.into_features(&mut f);
        } else {
            errors.push(unsupported("-mcpu=", v));
        }
    }
    match args.sve_vector_bits.as_deref() {
        None | Some("scalable") => {}
        Some(v) => match v.parse::<u32>() {
            Ok(n) if (128..=2048).contains(&n) && n.is_power_of_two() => {
                f.sve_vector_bits = Some(n);
            }
            _ => errors.push(unsupported("-msve-vector-bits=", v)),
        },
    }
    // clang `SanitizerArgs::addArgs`: `-target-feature +tagged-globals`
    // for the hardware-address sanitizer on AArch64 (WP-675).
    if target.is_aarch64()
        && args
            .sanitize(false, target.triple)
            .0
            .opts
            .has(kiln_ast::sanitize::SanitizerSet::HWADDRESS)
    {
        f.set("tagged-globals", true);
    }
    for t in &args.target_features {
        match t.split_at_checked(1) {
            Some(("+", name)) if !name.is_empty() => f.set(name, true),
            Some(("-", name)) if !name.is_empty() => f.set(name, false),
            _ => errors.push(unsupported("-target-feature", t)),
        }
    }
    if let Err(e) = args.codegen.branch_protection(true) {
        errors.push(e);
    }
    // WP-961: clang's x86-only `-mbranches-within-32B-boundaries`.
    if args.codegen.branches_within_32b {
        let t = args.target.as_deref().unwrap_or(target.triple);
        errors.push(ArgError::UnsupportedForTarget(
            "-mbranches-within-32B-boundaries".to_owned(),
            t.into(),
        ));
    }
    // WP-1000: clang's x86 feature flags are target-specific options
    // (named as clang's driver spells them), unknown when this clang
    // lacks them; `-msse4`, `-msse4.2` and `-m[no-]3dnow[a]` are only
    // unused ([`x86_features::non_x86_spelling`]; the driver warns).
    for a in &args.x86_feature_flags {
        if x86_features::unknown_at(a, v22) {
            errors.push(ArgError::Unknown(a.clone()));
        } else if let Some(o) = x86_features::non_x86_spelling(a, v22) {
            let t = args.target.as_deref().unwrap_or(target.triple);
            errors.push(ArgError::UnsupportedForTarget(o, t.into()));
        }
    }
    (f, errors)
}

/// clang 19's x86-64 `-march=` CPUs (its "valid target CPU values"
/// note, `llvm::X86::fillValidCPUArchList`).
const X86_CPUS: &[&str] = &[
    "nocona",
    "core2",
    "penryn",
    "bonnell",
    "atom",
    "silvermont",
    "slm",
    "goldmont",
    "goldmont-plus",
    "tremont",
    "nehalem",
    "corei7",
    "westmere",
    "sandybridge",
    "corei7-avx",
    "ivybridge",
    "core-avx-i",
    "haswell",
    "core-avx2",
    "broadwell",
    "skylake",
    "skylake-avx512",
    "skx",
    "cascadelake",
    "cooperlake",
    "cannonlake",
    "icelake-client",
    "rocketlake",
    "icelake-server",
    "tigerlake",
    "sapphirerapids",
    "alderlake",
    "raptorlake",
    "meteorlake",
    "arrowlake",
    "arrowlake-s",
    "lunarlake",
    "gracemont",
    "pantherlake",
    "sierraforest",
    "grandridge",
    "graniterapids",
    "graniterapids-d",
    "emeraldrapids",
    "clearwaterforest",
    "knl",
    "knm",
    "k8",
    "athlon64",
    "athlon-fx",
    "opteron",
    "k8-sse3",
    "athlon64-sse3",
    "opteron-sse3",
    "amdfam10",
    "barcelona",
    "btver1",
    "btver2",
    "bdver1",
    "bdver2",
    "bdver3",
    "bdver4",
    "znver1",
    "znver2",
    "znver3",
    "znver4",
    "znver5",
    "x86-64",
    "x86-64-v2",
    "x86-64-v3",
    "x86-64-v4",
];

/// clang 22's further x86-64 `-march=` CPUs.
const X86_CPUS_22: &[&str] = &["wildcatlake", "novalake", "diamondrapids"];

/// Whether `-march=<v>` names an x86-64 CPU clang knows.
fn is_x86_cpu(v: &str, v22: bool) -> bool {
    X86_CPUS.contains(&v) || (v22 && X86_CPUS_22.contains(&v))
}

/// The default `x86-64` CPU's features.
const X86_64_FEATURES: [&str; 7] = x86_features::X86_64;

/// The generated row of x86-64 CPU `name`, if clang 21 knows it.
fn x86_cpu(name: &str) -> Option<crate::x86_targets::Cpu> {
    crate::x86_targets::cpu(name)
}

/// How the predefined macros of the x86-64 features `f` differ from the
/// default x86-64 ones: each macro defined or redefined with its value,
/// then each default one no longer defined with `None`. The CPU's own
/// macros (`__corei7__`, `__tune_corei7__`; the lowercase ones) come from
/// the `-march=` CPU's row (clang 21's `-dM -E`;
/// spec/landed/driver/WP-630.md), the feature macros from the final
/// feature map ([`x86_features::feature_macros`], WP-1000). Empty without
/// a feature map (no `-march=` CPU kiln expands and no feature flag).
pub fn x86_macros(f: &TargetFeatures, v22: bool) -> Vec<(String, Option<String>)> {
    let Some(map) = f.x86_map() else { return Vec::new() };
    let cpu = f.cpu.as_deref().and_then(x86_cpu);
    let own = |n: &&str| n.bytes().any(|b| b.is_ascii_lowercase());
    let base = x86_features::feature_macros(&x86_features::feature_map(&X86_64_FEATURES, &[], v22), v22);
    let new = x86_features::feature_macros(&map, v22);
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    for c in cpu.iter() {
        out.extend(c.define.iter().filter(|(n, _)| own(n)).map(|(n, v)| (n.to_owned(), Some(v.to_owned()))));
    }
    out.extend(new.iter().filter(|(n, v)| base.get(*n) != Some(v)).map(|(n, v)| (n.clone(), Some(v.clone()))));
    for c in cpu.iter() {
        out.extend(c.undef.iter().filter(own).map(|n| (n.to_owned(), None)));
    }
    out.extend(base.keys().filter(|n| !new.contains_key(*n)).map(|n| (n.clone(), None)));
    out
}

/// The features x86 feature `f` enables: itself and what it implies
/// (`x86_targets::IMPLIES`; nothing for a name the table lacks, whose
/// caller enables just the name itself).
fn x86_implies(f: &str) -> impl Iterator<Item = &'static str> {
    crate::x86_targets::implies(f).into_iter().flat_map(crate::x86_targets::List::iter)
}

/// The features of a GNU `target("...")` attribute string on x86-64 over
/// the module's features `module` (`+name`/`-name`), as clang 19 to 21's
/// `X86TargetInfo::initFeatureMap` computes a function's map
/// (spec/landed/driver/WP-630.md "target"):
///
/// - Each comma-separated item is a feature, `no-<f>` disabling it;
///   `fpmath=` items and empty ones are skipped. `arch=<cpu>` starts from
///   that CPU's features instead of the module's and makes it the
///   function's CPU; `tune=<cpu>` its tune CPU. An `arch=` or `tune=`
///   naming no x86-64 CPU makes clang ignore the attribute (with a warning
///   kiln does not give): the result is then the module's features.
/// - The `evex512` items are held back. With an AVX-512 feature in the
///   map or among the items (and no `avx10.1-512` in the map),
///   `+evex512` is applied last, or `-evex512` when an item disabled it;
///   otherwise with an AVX10 feature, `evex512` follows `avx10.1-512`.
///   The AVX10 items are applied after the others.
/// - Enabling a feature enables what it implies; disabling one disables
///   every feature that implies it.
/// - Then `sse` enables `mmx`, `sse4.2` enables `popcnt` and `crc32`, and
///   `avx` enables `xsave`, unless an item disabled that feature.
///
/// The result is the items' `+` features in order (an `always_inline`
/// callee requires those first), then every feature of the final map.
/// `_v22` is unused: clang 19 and 22 agree here as far as kiln measured.
pub fn x86_target_attr_features(
    attr: &str,
    module: &[String],
    _v22: bool,
) -> kiln_codegen::TargetAttrFeatures {
    let mut map: BTreeMap<String, bool> = BTreeMap::new();
    let (mut cpu, mut tune) = (None, None);
    for item in attr.split(',').map(str::trim) {
        let (slot, v) = match (item.strip_prefix("arch="), item.strip_prefix("tune=")) {
            (Some(v), _) => (&mut cpu, v),
            (_, Some(v)) => (&mut tune, v),
            _ => continue,
        };
        if !is_x86_cpu(v, true) {
            let features = module.to_vec();
            return kiln_codegen::TargetAttrFeatures { features, ..Default::default() };
        }
        *slot = Some(v.to_owned());
    }
    match cpu.as_deref() {
        Some(c) => match x86_cpu(c) {
            Some(c) => map.extend(c.features.iter().map(|f| (f.to_owned(), true))),
            None => map.extend(X86_64_FEATURES.iter().map(|&f| (f.to_owned(), true))),
        },
        None => {
            for m in module {
                if let Some((sign, name)) = m.split_at_checked(1) {
                    map.insert(name.to_owned(), sign == "+");
                }
            }
        }
    }
    let on = |map: &BTreeMap<String, bool>, f: &str| map.get(f) == Some(&true);
    let mut avx512 = on(&map, "avx512f");
    let (mut avx10, mut avx10_512) = (on(&map, "avx10.1-256"), on(&map, "avx10.1-512"));
    let mut evex512 = None;
    let (mut items, mut avx10_items): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for item in attr.split(',').map(str::trim) {
        if item.is_empty() || ["arch=", "tune=", "fpmath="].iter().any(|p| item.starts_with(p)) {
            continue;
        }
        let f = match item.strip_prefix("no-") {
            Some(f) => format!("-{f}"),
            None => format!("+{item}"),
        };
        if f.get(1..7) == Some("avx10.") {
            if f.starts_with('+') {
                avx10 = true;
                avx10_512 |= f.ends_with("512");
            } else if f == "-avx10.1-256" {
                (avx10, avx10_512) = (false, false);
            } else if f == "-avx10.1-512" {
                avx10_512 = false;
            }
            avx10_items.push(f);
            continue;
        }
        if f.starts_with("+avx512") {
            avx512 = true;
        }
        match f.as_str() {
            "+evex512" => evex512 = Some(true),
            "-evex512" => evex512 = Some(false),
            _ => items.push(f),
        }
    }
    items.append(&mut avx10_items);
    if avx512 && !avx10_512 {
        items.push(if evex512 == Some(false) { "-evex512" } else { "+evex512" }.to_owned());
    } else if avx10 {
        items.push(if avx10_512 { "+evex512" } else { "-evex512" }.to_owned());
    }
    for f in &items {
        let Some((sign, name)) = f.split_at_checked(1) else { continue };
        if sign == "+" {
            map.insert(name.to_owned(), true);
            for g in x86_implies(name) {
                map.insert(g.to_owned(), true);
            }
        } else {
            map.insert(name.to_owned(), false);
            for (g, imp) in crate::x86_targets::implications() {
                if imp.contains(name) {
                    map.insert(g.to_owned(), false);
                }
            }
        }
    }
    let disabled = |n: &str| items.iter().any(|f| f.strip_prefix('-') == Some(n));
    let adds = [("sse4.2", "popcnt"), ("sse", "mmx"), ("avx", "xsave"), ("sse4.2", "crc32")];
    for (cond, add) in adds {
        if on(&map, cond) && !disabled(add) {
            map.insert(add.to_owned(), true);
        }
    }
    let own = items.iter().filter(|f| f.starts_with('+')).cloned();
    let all = map.into_iter().map(|(n, v)| format!("{}{n}", if v { '+' } else { '-' }));
    kiln_codegen::TargetAttrFeatures { features: own.chain(all).collect(), cpu, tune }
}

/// The LLVM feature of an `AArch64` extension named in a `target("...")`
/// attribute: the `-march=` spelling, and in clang 19 also the LLVM
/// feature names `neon`, `fp-armv8`, `fullfp16`, `jsconv`, `complxnum`.
fn attr_extension(name: &str, v22: bool) -> Option<&'static str> {
    extension(name, v22).or_else(|| {
        let llvm: &[&'static str] =
            if v22 { &[] } else { &["neon", "fp-armv8", "fullfp16", "jsconv", "complxnum"] };
        llvm.iter().copied().find(|n| *n == name)
    })
}

/// Whether LLVM feature `f` is an extension's (LLVM
/// `targetFeatureToExtension`): the ones `-march=` extensions name.
fn is_extension_feature(f: &str, v22: bool) -> bool {
    ["fp-armv8", "neon", "fullfp16", "jsconv", "complxnum", "mte", "rand", "fmv"].contains(&f)
        || extension(f, v22) == Some(f)
}

/// The features of a GNU `target("...")` attribute string on `AArch64`
/// over the module's features `module` (`+name`/`-name`; clang
/// `AArch64TargetInfo::parseTargetAttr` + `getFunctionFeatureMap`;
/// spec/landed/codegen/WP-558.md "Target features"). The module's extension features
/// seed an extension set (LLVM `reconstructFromParsedFeatures`); per
/// comma-separated item, an extension name (`-march=` spelling) enables
/// it with its dependencies, `no<ext>`/`no-<ext>` disables it with its
/// dependents; otherwise `no-x` is `-x`, a sub-architecture `v8.Na`/`v9.Na`
/// is `+v8.Na` plus its level's features (clang 22 `setFeatureEnabled`;
/// clang 19 enables the extension ones in the set), and
/// an LLVM feature name is `+x`. Then the extension set's touched
/// features follow, so they win over a plain `+x`/`-x` naming one.
/// `arch=`, `cpu=`, `tune=`, `branch-protection=`, `+ext` items and
/// unknown names add nothing (kiln does not diagnose them yet).
pub fn target_attr_features(
    attr: &str,
    module: &[String],
    v22: bool,
) -> kiln_codegen::TargetAttrFeatures {
    const LLVM: &[&str] = &["fp-armv8", "neon", "fullfp16", "jsconv", "complxnum", "mte", "rand"];
    let mut bits =
        ExtensionSet { arch: (8, 0), v22, enabled: BTreeSet::new(), touched: BTreeSet::new() };
    for m in module {
        let (on, name) = match m.split_at_checked(1) {
            Some(("+", n)) => (true, n),
            Some((_, n)) => (false, n),
            None => continue,
        };
        let known = ["fp-armv8", "neon", "fullfp16", "jsconv", "complxnum", "mte", "rand", "fmv"]
            .into_iter()
            .chain(DEPENDENT.iter().copied())
            .find(|f| *f == name)
            .or_else(|| extension(name, v22).filter(|f| *f == name));
        if let Some(f) = known.filter(|f| is_extension_feature(f, v22)) {
            bits.touched.insert(f);
            if on {
                bits.enabled.insert(f);
            } else {
                bits.enabled.remove(f);
            }
        }
    }
    let mut features = Vec::new();
    for item in attr.split(',').map(str::trim) {
        if item.is_empty()
            || item.starts_with('+')
            || ["arch=", "cpu=", "tune=", "branch-protection="].iter().any(|p| item.starts_with(p))
        {
            continue;
        }
        let neg = item.strip_prefix("no-").or_else(|| item.strip_prefix("no"));
        if let Some(f) = neg.and_then(|n| extension(n, v22)) {
            bits.disable(f);
            continue;
        }
        if let Some(f) = attr_extension(item, v22) {
            bits.enable(f);
            continue;
        }
        if let Some(n) = item.strip_prefix("no-") {
            features.push(format!("-{n}"));
        } else if let Some(arch) = parse_arch(&format!("arm{item}"), v22) {
            // clang 19 enables the level's extensions in the extension set
            // (their dependencies too); clang 22 only sets them.
            for &f in ExtensionSet::new(arch, v22).enabled.iter().filter(|f| **f != item) {
                if !v22 && is_extension_feature(f, v22) {
                    bits.enable(f);
                } else {
                    features.push(format!("+{f}"));
                }
            }
            features.push(format!("+{item}"));
        } else if LLVM.contains(&item) {
            features.push(format!("+{item}"));
        }
    }
    for f in &bits.touched {
        features.push(format!("{}{f}", if bits.enabled.contains(f) { '+' } else { '-' }));
    }
    kiln_codegen::TargetAttrFeatures { features, ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn res(s: &str, aarch64: bool) -> (TargetFeatures, Vec<ArgError>) {
        let a = Args::parse(&s.split_whitespace().map(str::to_owned).collect::<Vec<_>>());
        assert_eq!(a.errors, []);
        let t = Target::parse(if aarch64 { "aarch64-linux-gnu" } else { "x86_64-linux-gnu" });
        resolve(&a, &t.unwrap())
    }

    /// `"<sve> <extra,...>"`.
    fn feats(s: &str) -> String {
        let (f, e) = res(s, true);
        assert_eq!(e, []);
        format!("{} {}", f.sve, f.extra.join(","))
    }

    #[test]
    fn march_sve() {
        assert_eq!(feats("a.cpp"), "false ");
        assert_eq!(feats("-march=armv8-a a.cpp"), "false ");
        assert_eq!(feats("-march=armv8-a+sve a.cpp"), "true +fullfp16,+sve");
        assert_eq!(feats("-march=ARMv8-A+SVE a.cpp"), "true +fullfp16,+sve");
        assert_eq!(feats("-march=armv8-a+nosve a.cpp"), "false ");
        assert_eq!(feats("-march=armv8-a+sve+nosve a.cpp"), "false +fullfp16,-sve");
        assert_eq!(feats("-march=armv8-a+sve2+nosve a.cpp"), "false +fullfp16,-sve,-sve2");
        // The last `-march=` wins.
        assert_eq!(feats("-march=armv8-a+sve -march=armv8-a a.cpp"), "false ");
    }

    #[test]
    fn mcpu_and_march() {
        assert_eq!(feats("-mcpu=generic a.cpp"), "false +ete,+trbe");
        assert_eq!(feats("-mcpu=generic+sve a.cpp"), "true +ete,+trbe,+fullfp16,+sve");
        // `-march=` wins and `-mcpu=`'s extensions are ignored.
        assert_eq!(feats("-mcpu=generic+sve -march=armv8-a a.cpp"), "false ");
        assert_eq!(feats("-march=armv8-a+sve -mcpu=generic a.cpp"), "true +fullfp16,+sve");
    }

    #[test]
    fn target_feature_last_wins() {
        assert_eq!(feats("-Xclang -target-feature -Xclang +sve a.cpp"), "true +sve");
        assert_eq!(feats("-target-feature -sve a.cpp"), "false -sve");
        assert_eq!(
            feats("-march=armv8-a+sve -Xclang -target-feature -Xclang -sve a.cpp"),
            "false +fullfp16,-sve"
        );
        assert_eq!(feats("-target-feature +sve -target-feature -sve a.cpp"), "false -sve");
    }

    #[test]
    fn levels() {
        assert_eq!(feats("-march=armv8.1-a a.cpp"), "false +crc,+lse,+rdm,+v8.1a");
        assert_eq!(feats("-march=armv8.1a a.cpp"), "false +crc,+lse,+rdm,+v8.1a");
        let (f, _) = res("-march=armv9.2-a a.cpp", true);
        assert_eq!((f.sve, f.arch), (true, (9, 2)));
        assert!(f.extra.contains(&"+v8.7a".to_owned()) && f.extra.contains(&"+mec".to_owned()));
        assert!(f.extra.contains(&"+spe-eef".to_owned()));
        let (f, _) = res("-fclang-compat=19 -march=armv9.2-a a.cpp", true);
        assert!(!f.extra.contains(&"+spe-eef".to_owned()));
        assert_eq!(res("-march=armv9.7-a a.cpp", true).1, []);
    }

    #[test]
    fn extension_closure() {
        assert_eq!(feats("-march=armv8-a+crypto a.cpp"), "false +aes,+crypto,+sha2");
        assert_eq!(feats("-march=armv8-a+crc+nocrc a.cpp"), "false -crc");
        assert_eq!(feats("-march=armv8-a+nocrc a.cpp"), "false ");
        assert_eq!(feats("-march=armv8-a+crypto+noaes a.cpp"), "false -aes,-crypto,-sha2");
        assert_eq!(feats("-march=armv8-a+nofp a.cpp"), "false -fp-armv8,-neon");
        assert_eq!(feats("-march=armv8-a+fp16fml+nofp16 a.cpp"), "false -fp16fml,-fullfp16");
        // `+fp16` implies `fp16fml` on v8.4 .. v8.9; `+crypto` adds `sha3`
        // and `sm4` from v8.4.
        let x = |s: &str| res(s, true).0.extra;
        assert!(x("-march=armv8.4-a+sve a.cpp").contains(&"+fp16fml".to_owned()));
        assert!(!x("-march=armv9-a+fp16 a.cpp").contains(&"+fp16fml".to_owned()));
        assert!(x("-march=armv9-a+crypto a.cpp").contains(&"+sm4".to_owned()));
        let nofp = x("-march=armv9-a+nofp a.cpp");
        for n in ["-complxnum", "-dotprod", "-fullfp16", "-jsconv", "-rdm", "-sve", "-sve2"] {
            assert!(nofp.contains(&n.to_owned()), "{n}");
        }
        assert_eq!(
            feats("-march=armv8-a+sve2-aes a.cpp"),
            "true +aes,+fullfp16,+sve,+sve-aes,+sve2,+sve2-aes"
        );
        assert_eq!(
            feats("-fclang-compat=19 -march=armv8-a+sve2-aes a.cpp"),
            "true +aes,+fullfp16,+sve,+sve2,+sve2-aes"
        );
        // clang 22's `sve-bitperm` stands alone, but disabling it (through
        // `sve2-bitperm`) also disables `sve2`.
        assert_eq!(feats("-march=armv8-a+sve-bitperm a.cpp"), "false +sve-bitperm");
        assert_eq!(
            feats("-march=armv8-a+sve2-bitperm+nosve-bitperm a.cpp"),
            "true +fullfp16,+sve,-sve-bitperm,-sve2,-sve2-bitperm"
        );
        assert_eq!(
            feats("-march=armv8-a+f16f32dot+nosimd a.cpp"),
            "false -f16f32dot,+fullfp16,-neon"
        );
        assert_eq!(res("-fclang-compat=19 -march=armv8-a+sve-bitperm a.cpp", true).1.len(), 1);
    }

    #[test]
    fn sve_vector_bits() {
        assert_eq!(res("-msve-vector-bits=256 a.cpp", true).0.sve_vector_bits, Some(256));
        assert_eq!(res("-msve-vector-bits=scalable a.cpp", true).0.sve_vector_bits, None);
        let u = |v: &str| vec![ArgError::UnsupportedValue("-msve-vector-bits=".into(), v.into())];
        assert_eq!(res("-msve-vector-bits=100 a.cpp", true).1, u("100"));
        assert_eq!(res("-msve-vector-bits=4096 a.cpp", true).1, u("4096"));
        let t =
            ArgError::UnsupportedForTarget("-msve-vector-bits=".into(), "x86_64-linux-gnu".into());
        assert_eq!(res("--target=x86_64-linux-gnu -msve-vector-bits=256 a.cpp", false).1, [t]);
        let t = ArgError::UnsupportedForTarget("-mcpu=".into(), "x86_64-linux-gnu".into());
        assert_eq!(res("--target=x86_64-linux-gnu -mcpu=cortex-a57 a.cpp", false).1, [t]);
    }

    #[test]
    fn unsupported_values() {
        let u = |o: &str, v: &str| ArgError::UnsupportedValue(o.into(), v.into());
        assert_eq!(res("-march=armv8-a+foo a.cpp", true).1, [u("-march=", "armv8-a+foo")]);
        assert_eq!(res("-march=armv7-a a.cpp", true).1, [u("-march=", "armv7-a")]);
        assert_eq!(res("-march=armv8.0-a a.cpp", true).1, [u("-march=", "armv8.0-a")]);
        assert_eq!(res("-march=armv8-a+tme a.cpp", true).1, [u("-march=", "armv8-a+tme")]);
        assert_eq!(res("-fclang-compat=19 -march=armv8-a+tme a.cpp", true).1, []);
        assert_eq!(
            res("-fclang-compat=19 -march=armv9.6-a a.cpp", true).1,
            [u("-march=", "armv9.6-a")]
        );
        assert_eq!(res("-mcpu=cortex-a57 a.cpp", true).1, [u("-mcpu=", "cortex-a57")]);
        assert_eq!(res("-target-feature sve a.cpp", true).1, [u("-target-feature", "sve")]);
        assert_eq!(res("-march=armv8-a+sve a.cpp", false).1, [u("-march=", "armv8-a+sve")]);
        // `-target-feature` is ignored on a non-`AArch64` target.
        assert_eq!(res("-target-feature +sve a.cpp", false).0.extra, Vec::<String>::new());
        assert_eq!(res("-march=armv8-a+foo a.cpp", true).0.extra, Vec::<String>::new());
    }

    /// spec/driver.md "Codegen flags" (WP-605): `native` on both targets
    /// and the x86-64 CPU names are the backend CPU.
    #[test]
    fn march_cpu() {
        let cpu = |s: &str, aarch64: bool| {
            let (f, e) = res(s, aarch64);
            assert_eq!(e, [], "{s}");
            f.cpu
        };
        assert_eq!(cpu("a.cpp", true), None);
        assert_eq!(cpu("a.cpp", false), None);
        assert_eq!(cpu("-march=native a.cpp", true).as_deref(), Some("native"));
        assert_eq!(cpu("-mcpu=native a.cpp", true).as_deref(), Some("native"));
        assert_eq!(cpu("-march=native a.cpp", false).as_deref(), Some("native"));
        assert_eq!(cpu("-march=haswell a.cpp", false).as_deref(), Some("haswell"));
        assert_eq!(cpu("-march=x86-64-v3 a.cpp", false).as_deref(), Some("x86-64-v3"));
        assert_eq!(cpu("-march=armv8.2-a a.cpp", true), None);
        // `-march=native` keeps the default features.
        assert_eq!(res("-march=native a.cpp", true).0.extra, Vec::<String>::new());
        let u = |o: &str, v: &str| ArgError::UnsupportedValue(o.into(), v.into());
        assert_eq!(res("-march=foo a.cpp", false).1, [u("-march=", "foo")]);
        assert_eq!(
            res("-fclang-compat=19 -march=novalake a.cpp", false).1,
            [u("-march=", "novalake")]
        );
        assert_eq!(cpu("-march=novalake a.cpp", false).as_deref(), Some("novalake"));
    }

    /// WP-630: an x86-64 `-march=<cpu>` has clang's features and
    /// predefined macros; the default keeps `x86-64`'s, as `native` does
    /// off an x86-64 host.
    #[test]
    fn x86_cpu_features() {
        let f = res("-march=westmere a.cpp", false).0;
        assert_eq!(
            f.extra.join(","),
            "+cmov,+crc32,+cx16,+cx8,+fxsr,+mmx,+pclmul,+popcnt,+sahf,+sse,+sse2,+sse3,+sse4.1,\
             +sse4.2,+ssse3,+x87"
        );
        assert_eq!(f.x86_enabled().len(), 16);
        let base = ["cmov", "cx8", "fxsr", "mmx", "sse", "sse2", "x87"];
        assert_eq!(res("a.cpp", false).0.x86_enabled(), base);
        if std::env::consts::ARCH != "x86_64" {
            // Off an x86-64 host `native` has no features to detect.
            assert_eq!(res("-march=native a.cpp", false).0.x86_enabled(), base);
            assert_eq!(x86_macros(&res("-march=native a.cpp", false).0, true), []);
        }
        let m = x86_macros(&res("-march=westmere a.cpp", false).0, true);
        assert!(m.contains(&("__SSE4_2__".to_owned(), Some("1".to_owned()))));
        assert!(m.contains(&("__tune_corei7__".to_owned(), Some("1".to_owned()))));
        assert!(m.contains(&("__k8".to_owned(), None)));
        assert!(!m.iter().any(|(n, _)| n == "__AES__"));
        assert_eq!(x86_macros(&res("-march=x86-64 a.cpp", false).0, true), []);
    }

    /// WP-1000: the feature-derived macros of every CPU reproduce its
    /// generated row (clang 21's `-dM -E`, which still has `evex512`, as
    /// clang 19), as a set. The CPUs whose clang 19 features differ from
    /// clang 21's are skipped (the corpus pins them against clang 19).
    #[test]
    fn x86_cpu_macros_from_features() {
        // clang 22's further CPUs are no `-fclang-compat=19` CPU.
        let same = |c: &crate::x86_targets::Cpu| {
            x86_features::cpu_features(*c, false) == c.features.iter().collect::<Vec<_>>()
        };
        for c in crate::x86_targets::cpus().filter(|c| !X86_CPUS_22.contains(&c.name) && same(c)) {
            let f = res(&format!("-march={} -fclang-compat=19 a.cpp", c.name), false).0;
            let mut got: Vec<_> = x86_macros(&f, false);
            let mut want: Vec<(String, Option<String>)> =
                c.define.iter().map(|(n, v)| (n.to_owned(), Some(v.to_owned()))).collect();
            want.extend(c.undef.iter().map(|n| (n.to_owned(), None)));
            got.sort();
            want.sort();
            assert_eq!(got, want, "{}", c.name);
        }
    }

    /// WP-630: x86 `target("...")` strings over the default module
    /// features: the function's map as clang 21's `target-features`
    /// prints it (enabled features sorted, then disabled ones).
    #[test]
    fn x86_target_attribute() {
        let module: Vec<String> =
            ["+cmov", "+cx8", "+fxsr", "+mmx", "+sse", "+sse2", "+x87"].map(String::from).into();
        let t = |s: &str| {
            let mut m = BTreeMap::new();
            for e in x86_target_attr_features(s, &module, true).features {
                m.insert(e[1..].to_owned(), e.starts_with('+'));
            }
            let on = m.iter().filter(|e| *e.1).map(|e| format!("+{}", e.0));
            let off = m.iter().filter(|e| !*e.1).map(|e| format!("-{}", e.0));
            on.chain(off).collect::<Vec<_>>().join(",")
        };
        let base = "+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87";
        assert_eq!(
            t("sse4.2"),
            "+cmov,+crc32,+cx8,+fxsr,+mmx,+popcnt,+sse,+sse2,+sse3,+sse4.1,+sse4.2,+ssse3,+x87"
        );
        // A lone `no-evex512` (clang 19's SSE headers) changes nothing.
        assert_eq!(t("sse2,no-evex512"), base);
        assert_eq!(
            t("avx512f,no-evex512"),
            "+avx,+avx2,+avx512f,+cmov,+crc32,+cx8,+f16c,+fma,+fxsr,+mmx,+popcnt,+sse,+sse2,\
             +sse3,+sse4.1,+sse4.2,+ssse3,+x87,+xsave,-amx-avx512,-avx10.1-512,-avx10.2-512,\
             -evex512"
        );
        assert_eq!(t("aes,pclmul"), "+aes,+cmov,+cx8,+fxsr,+mmx,+pclmul,+sse,+sse2,+x87");
        assert_eq!(t("tune=generic,ssse3"), base);
        let a = x86_target_attr_features("arch=haswell,tune=znver4", &module, true);
        assert_eq!((a.cpu.as_deref(), a.tune.as_deref()), (Some("haswell"), Some("znver4")));
        assert!(a.features.iter().any(|f| f == "+avx2"));
        // The attribute's own features come first.
        let own = x86_target_attr_features("popcnt,crc32", &module, true).features;
        assert_eq!(own[..2], ["+popcnt", "+crc32"]);
    }

    /// `target("...")` strings over the default module features (clang
    /// 19's and 22's `target-features` of such a function;
    /// spec/landed/codegen/WP-558.md "Target features").
    #[test]
    fn target_attribute() {
        let module: Vec<String> =
            ["+fp-armv8", "+neon", "+outline-atomics", "+v8a", "-fmv"].map(String::from).into();
        let t = |s: &str, v22: bool| target_attr_features(s, &module, v22).features.join(",");
        assert_eq!(t("bf16,neon", false), "+bf16,-fmv,+fp-armv8,+neon");
        assert_eq!(t("sha3", false), "-fmv,+fp-armv8,+neon,+sha2,+sha3");
        assert_eq!(t("fp16", true), "-fmv,+fp-armv8,+fullfp16,+neon");
        // clang 19 enables the level's extensions in the set, clang 22
        // sets them after the module's.
        assert_eq!(t("v8.1a", false), "+v8.1a,+crc,-fmv,+fp-armv8,+lse,+neon,+rdm");
        assert_eq!(t("v8.1a", true), "+crc,+fp-armv8,+lse,+neon,+rdm,+v8.1a,-fmv,+fp-armv8,+neon");
        // `neon` is an extension name in clang 19 only.
        assert_eq!(t("neon", true), "+neon,-fmv,+fp-armv8,+neon");
        // `no-neon` names the LLVM feature, not the extension (`simd`):
        // the module's `+neon` in the set wins; `nosha2` of an off
        // extension touches nothing.
        assert_eq!(
            t("no-neon,nosha2,arch=armv8-a,+crc,bogus", false),
            "-neon,-fmv,+fp-armv8,+neon"
        );
        assert_eq!(t("nosimd", true), "-fmv,+fp-armv8,-neon");
    }
}
