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
//! `-mcpu=<cpu>[+ext...]` names a CPU of clang's `AArch64TargetParser`
//! ([`crate::aarch64_cpus`], by `-fclang-compat=`; WP-1151) whose
//! extensions are on, the modifiers applying over them alike. `-march=`
//! wins over `-mcpu=`'s features (the CPU stays the `target-cpu`, as
//! clang's); the `-target-feature` list applies after
//! both, the last naming a feature winning, with no dependency closure
//! (as clang's `-cc1`). Any other architecture, CPU or extension is an
//! unsupported value. The levels are clang 19's or clang 22's by
//! `-fclang-compat=` (measured with `clang-N -S -emit-llvm -march=...`);
//! the modifier names and the dependency graph are the generated
//! [`crate::aarch64_extensions`] (`scripts/gen-aarch64-cpus.py`; WP-1167).

use std::collections::{BTreeMap, BTreeSet};

use kiln_pp::ClangCompat;

use crate::aarch64_cpus;
use crate::aarch64_extensions;
use crate::aarch64_levels;
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
    /// x86-64: the `-march=` CPU's features (empty: the default
    /// `x86-64`'s), before the `-m<feature>` items (WP-1000). On x86-64
    /// `extra` is instead the whole feature map as `+name`/`-name`,
    /// sorted (empty: the default `x86-64`'s; WP-630, WP-1000).
    pub x86_cpu_features: Vec<String>,
    /// x86-64: the command line's feature items as written, clang's
    /// `FeaturesAsWritten` (the deduplicated `-m<feature>` items, then
    /// `+retpoline-indirect-calls`).
    pub x86_written: Vec<String>,
    /// `-msve-vector-bits=N` (128 .. 2048, a power of two); `None` for
    /// `scalable` or no option.
    pub sve_vector_bits: Option<u32>,
    /// The CPU the backend generates code for: `native` for
    /// `-march=native` / `-mcpu=native` (both targets; the backend's
    /// `-mcpu=native` detects the host, kiln does not), or the x86
    /// `-march=<cpu>`; `None` for the default (spec/driver.md "Codegen
    /// flags").
    pub cpu: Option<String>,
    /// `AArch64`: clang's `hasTargetFeatureMTE`, the memtag sanitizers'
    /// hardware support: `-march=`/`-mcpu=` enable `mte`, or (clang 22)
    /// an `-Xclang -target-feature +mte` is given (WP-1151).
    pub memtag_hw: bool,
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
        Some(
            entries
                .filter_map(|e| e.split_at_checked(1))
                .map(|(s, n)| (n.to_owned(), s == "+"))
                .collect(),
        )
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

/// The features architecture level `major.minor` adds (the generated
/// [`aarch64_levels`] table of clang 19 or clang 22; WP-1187): `armv8.N-a`
/// adds them to `armv8.(N-1)-a`, `armv9.N-a` to `armv9.(N-1)-a` beyond the
/// `armv8.(N+5)-a` (up to 8.9) it includes. `None` for a level the
/// clang version does not know.
fn level(major: u8, minor: u8, v22: bool) -> Option<impl Iterator<Item = &'static str>> {
    let table = if v22 { aarch64_levels::LEVELS_22 } else { aarch64_levels::LEVELS_19 };
    let key = format!("{major}.{minor}");
    table.lines().find_map(|l| l.split_once(' ').filter(|(k, _)| *k == key)).map(|(_, f)| list(f))
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
    let major = match major {
        "8" => 8,
        "9" => 9,
        _ => return None,
    };
    level(major, minor, v22).map(|_| (major, minor))
}

/// The rows `(feature, names, deps)` of the generated modifier table of
/// clang 19 or clang 22 ([`aarch64_extensions`], scripts/gen-aarch64-cpus.py
/// documents the columns; WP-1167).
fn extension_rows(v22: bool) -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
    let table =
        if v22 { aarch64_extensions::EXTENSIONS_22 } else { aarch64_extensions::EXTENSIONS_19 };
    table.lines().filter_map(|l| {
        let mut w = l.split(' ');
        Some((w.next()?, w.next()?, w.next()?))
    })
}

/// The LLVM feature an `-march=` extension name (lower case) enables or
/// (as `no<name>`) disables: a modifier of clang's
/// `--print-supported-extensions`, or the alias `rdma`.
fn extension(name: &str, v22: bool) -> Option<&'static str> {
    extension_rows(v22).find(|(_, names, _)| list(names).any(|n| n == name)).map(|(f, _, _)| f)
}

/// The table row of LLVM feature `f`, if it is in LLVM's extension graph.
fn extension_feature(f: &str, v22: bool) -> Option<&'static str> {
    extension_rows(v22).find(|(g, _, _)| *g == f).map(|(g, _, _)| g)
}

/// Splits `base+ext+...` into the base and the `+`-separated extensions.
fn split(v: &str) -> (String, &str) {
    v.split_once('+').map_or((v.to_owned(), ""), |(b, e)| (b.to_owned(), e))
}

/// The features feature `f` directly depends on (LLVM
/// `ExtensionDependencies`, from the generated table): enabling `f`
/// enables them, disabling one of them disables `f`.
fn deps(f: &str, v22: bool) -> impl Iterator<Item = &'static str> {
    list(extension_rows(v22).find(|(g, _, _)| *g == f).map_or("-", |(_, _, d)| d))
}

/// The features directly depending on `f` (the next step of a disable
/// cascade).
fn dependents(f: &str, v22: bool) -> impl Iterator<Item = &'static str> {
    extension_rows(v22).filter(move |(_, _, d)| list(d).any(|x| x == f)).map(|(g, _, _)| g)
}

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
        let mut defaults: Vec<&'static str> = vec!["fp-armv8", "neon"];
        defaults.extend((1..=v8).filter_map(|n| level(8, n, v22)).flatten());
        if arch.0 == 9 {
            defaults.extend((0..=arch.1).filter_map(|n| level(9, n, v22)).flatten());
        }
        for f in defaults {
            s.enable(f);
        }
        s
    }

    fn enable(&mut self, f: &'static str) {
        if !self.enabled.insert(f) {
            return;
        }
        self.touched.insert(f);
        for d in deps(f, self.v22) {
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
        for d in dependents(f, self.v22) {
            self.disable(d);
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

/// A row of the `-mcpu=` CPU table (`aarch64_cpus`; the columns are
/// scripts/gen-aarch64-cpus.py's).
struct CpuRow {
    canonical: &'static str,
    arch: &'static str,
    driver: &'static str,
    closure: &'static str,
    keep: &'static str,
}

/// The items of a table list (`-` is the empty list).
fn list(l: &'static str) -> impl Iterator<Item = &'static str> {
    l.split(',').filter(|f| *f != "-")
}

/// The CPU table row of `name` (case-insensitive, as clang's `parseCpu`
/// after the driver lower-cases `-mcpu=`), for clang 19 or clang 22.
/// The R-profile CPUs (`v8r`: `cortex-r82`, `cortex-r82ae`) are left out,
/// as `-march=armv8-r` is (kiln has no R-profile predefines).
fn cpu_row(name: &str, v22: bool) -> Option<CpuRow> {
    let table = if v22 { aarch64_cpus::CPUS_22 } else { aarch64_cpus::CPUS_19 };
    table.lines().find_map(|l| {
        let mut w = l.split(' ');
        if !w.next()?.eq_ignore_ascii_case(name) {
            return None;
        }
        let row = CpuRow {
            canonical: w.next()?,
            arch: w.next()?,
            driver: w.next()?,
            closure: w.next()?,
            keep: w.next()?,
        };
        (row.arch != "v8r").then_some(row)
    })
}

/// The `(major, minor)` of an architecture feature (`v8a`, `v8.2a`, `v9a`).
fn arch_of(feature: &str) -> Option<(u8, u8)> {
    let v = feature.strip_prefix('v')?.strip_suffix('a')?;
    let (major, minor) = v.split_once('.').unwrap_or((v, "0"));
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// The CPU of `-mcpu=<cpu>[+ext...]` and its features as clang's
/// `ExtensionSet` after `addCPUDefaults` and the `+ext` items, or `None`
/// for an unknown CPU or extension.
fn cpu_set(v: &str, v22: bool) -> Option<(CpuRow, ExtensionSet)> {
    let (base, exts) = split(v);
    let row = cpu_row(&base, v22)?;
    let arch = arch_of(row.arch)?;
    let on: BTreeSet<&'static str> = list(row.driver).chain([row.arch]).collect();
    let mut s = ExtensionSet { arch, v22, enabled: on.clone(), touched: on };
    s.extensions(exts)?;
    Some((row, s))
}

/// [`resolve`] on x86-64 (spec/driver.md "x86-64 target features").
fn resolve_x86(args: &Args, target: &Target, f: &mut TargetFeatures, errors: &mut Vec<ArgError>) {
    let unsupported = |opt: &str, v: &str| ArgError::UnsupportedValue(opt.to_owned(), v.to_owned());
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
    f.x86_cpu_features = cpu_features.iter().flatten().map(|&c| c.to_owned()).collect();
    if cpu_features.is_some() || !items.is_empty() {
        let cpu = cpu_features.unwrap_or_else(|| X86_64_FEATURES.to_vec());
        let map = x86_features::feature_map(&cpu, &items, v22);
        f.extra =
            map.into_iter().map(|(n, on)| format!("{}{n}", if on { '+' } else { '-' })).collect();
    }
    f.x86_written = items;
    // WP-813: clang's `getX86TargetFeatures` adds the retpoline call
    // thunks for `-mspeculative-load-hardening`.
    if args.codegen.speculative_load_hardening {
        f.extra.push("+retpoline-indirect-calls".to_owned());
        f.x86_written.push("+retpoline-indirect-calls".to_owned());
    }
    if args.mcpu.is_some() {
        let t = args.target.as_deref().unwrap_or(target.triple);
        errors.push(ArgError::UnsupportedForTarget("-mcpu=".to_owned(), t.into()));
    }
    if args.sve_vector_bits.is_some() {
        let t = args.target.as_deref().unwrap_or(target.triple);
        errors.push(ArgError::UnsupportedForTarget("-msve-vector-bits=".to_owned(), t.into()));
    }
    // WP-1151: `-ffixed-x18` is an `AArch64` (and RISC-V) option.
    if args.fixed_x18 {
        let t = args.target.as_deref().unwrap_or(target.triple);
        errors.push(ArgError::UnsupportedForTarget("-ffixed-x18".to_owned(), t.into()));
    }
    // WP-846: clang's target-specific `-mbranch-protection=`.
    if args.codegen.mbranch_protection_given {
        let t = args.target.as_deref().unwrap_or(target.triple);
        errors.push(ArgError::UnsupportedForTarget("-mbranch-protection=".to_owned(), t.into()));
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
        resolve_x86(args, target, &mut f, &mut errors);
        return (f, errors);
    }
    let v22 = args.predef.clang_compat == ClangCompat::Clang22;
    if native(&args.march) || (args.march.is_none() && native(&args.mcpu)) {
        f.cpu = Some("native".to_owned());
    } else if let Some(v) = &args.march {
        // WP-1151: `-march=` wins over the `-mcpu=` CPU's features, but the
        // CPU is still checked and still the `target-cpu` (clang's
        // `getAArch64TargetFeatures` parses both; `getAArch64TargetCPU`).
        let cpu = args.mcpu.as_deref().filter(|c| *c != "native").map(|c| (c, cpu_set(c, v22)));
        if let Some((c, None)) = cpu {
            errors.push(unsupported("-mcpu=", c));
        }
        let (base, exts) = split(v);
        let set = parse_arch(&base.to_ascii_lowercase(), v22).and_then(|arch| {
            let mut s = ExtensionSet::new(arch, v22);
            s.extensions(exts).map(|()| s)
        });
        match set {
            Some(s) => {
                if let Some((_, Some((row, _)))) = &cpu {
                    // clang 19's `+zcm,+zcz` of an Apple CPU stay.
                    for k in list(row.keep) {
                        f.set(k, true);
                    }
                    f.cpu = Some(row.canonical.to_owned()).filter(|c| c != "generic");
                }
                s.into_features(&mut f);
            }
            None => errors.push(unsupported("-march=", v)),
        }
    } else if let Some(v) = &args.mcpu {
        // WP-1151: a CPU of clang's `AArch64TargetParser` (by
        // `-fclang-compat=`) has its extensions (`aarch64_cpus`); the
        // function's `target-features` are its architecture feature's
        // closure overridden by the driver's list.
        match cpu_set(v, v22) {
            Some((row, s)) => {
                for c in list(row.closure) {
                    f.set(c, true);
                }
                f.cpu = Some(row.canonical.to_owned()).filter(|c| c != "generic");
                s.into_features(&mut f);
            }
            None => errors.push(unsupported("-mcpu=", v)),
        }
    }
    // WP-1151: clang's `hasTargetFeatureMTE` (the memtag sanitizers'
    // hardware check) sees the driver's own `-target-feature` list, and at
    // `-fclang-compat=22` also the `-Xclang -target-feature` items (any
    // `+mte`, no last-wins); clang 19 checks before appending them.
    f.memtag_hw = f.extra.iter().any(|e| e == "+mte")
        || v22 && args.target_features.iter().any(|t| t == "+mte");
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
    // WP-1151: clang's `getAArch64TargetFeatures` reserves x18.
    if args.fixed_x18 {
        f.set("reserve-x18", true);
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
    let base =
        x86_features::feature_macros(&x86_features::feature_map(&X86_64_FEATURES, &[], v22), v22);
    let new = x86_features::feature_macros(&map, v22);
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    if let Some(c) = &cpu {
        out.extend(
            c.define
                .iter()
                .filter(|(n, _)| own(n))
                .map(|(n, v)| (n.to_owned(), Some(v.to_owned()))),
        );
    }
    out.extend(
        new.iter()
            .filter(|(n, v)| base.get(*n) != Some(v))
            .map(|(n, v)| (n.clone(), Some(v.clone()))),
    );
    if let Some(c) = &cpu {
        out.extend(c.undef.iter().filter(own).map(|n| (n.to_owned(), None)));
    }
    out.extend(base.keys().filter(|n| !new.contains_key(*n)).map(|n| (n.clone(), None)));
    out
}

/// The features of a GNU `target("...")` attribute string on x86-64 over
/// the module `m` (its CPU's features, else the default `x86-64`'s), as clang's
/// `getFunctionFeatureMap` and `X86TargetInfo::initFeatureMap` compute a
/// function's map (spec/landed/driver/WP-630.md "target", WP-1000):
///
/// - Each comma-separated item is a feature, `no-<f>` disabling it;
///   `fpmath=` items and empty ones are skipped. `arch=<cpu>` starts from
///   that CPU's features (as `-march=<cpu>` at this compat level) instead
///   of the module's and makes it the function's CPU; `tune=<cpu>` its
///   tune CPU. An `arch=` or `tune=` naming no x86-64 CPU makes clang
///   ignore the attribute (with a warning kiln does not give): the result
///   is then the module's features.
/// - The command line's items as written (`m.written`, clang
///   `FeaturesAsWritten`) come before the attribute's, with or without
///   `arch=`.
/// - The items are applied as the `-m<feature>` items of `-march=`
///   ([`x86_features::feature_map`]): clang 19 holds the `evex512` items
///   back and reorders the AVX10 ones; enabling a feature enables what it
///   implies and disabling one disables every feature that implies it;
///   then `sse` enables `mmx`, `sse4.2` `popcnt` and `crc32`, and `avx`
///   `xsave`, unless an item disabled that feature.
/// - At `-fclang-compat=22` the `evex512` / `no-evex512` items are
///   dropped: clang 22 has no such feature, and kiln's builtin headers
///   (clang 19's) name it.
///
/// The result is the items' `+` features in order (an `always_inline`
/// callee requires those first), then every feature of the final map.
pub fn x86_target_attr_features(
    attr: &str,
    m: &kiln_codegen::TargetAttrModule<'_>,
) -> kiln_codegen::TargetAttrFeatures {
    let (module, v22) = (m.features, m.v22);
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
    let mut map: BTreeMap<String, bool> = match cpu.as_deref() {
        Some(c) => match x86_cpu(c) {
            Some(c) => x86_features::cpu_map(&x86_features::cpu_features(c, v22), v22),
            None => x86_features::cpu_map(&X86_64_FEATURES, v22),
        },
        None if m.cpu_features.is_empty() => x86_features::cpu_map(&X86_64_FEATURES, v22),
        None => {
            let c: Vec<&str> = m.cpu_features.iter().map(String::as_str).collect();
            x86_features::cpu_map(&c, v22)
        }
    };
    let own: Vec<String> = attr
        .split(',')
        .map(str::trim)
        .filter(|i| {
            !i.is_empty() && !["arch=", "tune=", "fpmath="].iter().any(|p| i.starts_with(p))
        })
        .map(|i| match i.strip_prefix("no-") {
            Some(f) => format!("-{f}"),
            None => format!("+{i}"),
        })
        // kiln's builtin headers are clang 19's, whose SSE functions say
        // `target("sse2,no-evex512")`; clang 22 has no `evex512` (its
        // headers do not name it), so its items are dropped there.
        .filter(|f: &String| !(v22 && f[1..] == *"evex512"))
        .collect();
    let items: Vec<String> = m.written.iter().chain(&own).cloned().collect();
    x86_features::apply_items(&mut map, &items, v22);
    let own = own.into_iter().filter(|f| f.starts_with('+'));
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
/// `targetFeatureToExtension`): `fmv`, or one a `-march=` modifier names.
fn is_extension_feature(f: &str, v22: bool) -> bool {
    f == "fmv" || extension_rows(v22).any(|(g, names, _)| g == f && names != "-")
}

/// [`target_attr_features`] as a [`kiln_codegen::TargetAttrParser`].
pub fn target_attr_parser(
    attr: &str,
    m: &kiln_codegen::TargetAttrModule<'_>,
) -> kiln_codegen::TargetAttrFeatures {
    target_attr_features(attr, m.features, m.v22)
}

/// The CPU table row of a `target("cpu=<name>")` / `tune=<name>` CPU:
/// case-sensitive (clang's `AArch64::parseCpu` on the attribute's text,
/// an alias resolving to its CPU), the R-profile CPUs included.
fn attr_cpu_row(name: &str, v22: bool) -> Option<CpuRow> {
    let table = if v22 { aarch64_cpus::CPUS_22 } else { aarch64_cpus::CPUS_19 };
    table.lines().find_map(|l| {
        let mut w = l.split(' ');
        if w.next()? != name {
            return None;
        }
        Some(CpuRow {
            canonical: w.next()?,
            arch: w.next()?,
            driver: w.next()?,
            closure: w.next()?,
            keep: w.next()?,
        })
    })
}

/// Whether `name` is a CPU of clang 19's / clang 22's `AArch64`
/// `isValidCPUName` (case-sensitive): the check of a `target("cpu=…")`
/// and `tune=…` CPU (clang `Sema::checkTargetAttr`, WP-1187).
#[must_use]
pub fn aarch64_attr_cpu_valid(name: &str, v22: bool) -> bool {
    attr_cpu_row(name, v22).is_some()
}

/// The `+ext+...` modifiers of a `target("...")` item (clang
/// `parseTargetAttr`'s `SplitAndAddFeatures`): an extension (`no<ext>`
/// disables it) goes to the extension set; anything else passes through
/// as `-name` for `no<name>`, else `+name`.
fn attr_modifiers(exts: &str, bits: &mut ExtensionSet, features: &mut Vec<String>) {
    for e in exts.split('+').filter(|e| !e.is_empty()) {
        match e.strip_prefix("no") {
            Some(n) => match extension(n, bits.v22) {
                Some(f) => bits.disable(f),
                None => features.push(format!("-{n}")),
            },
            None => match attr_extension(e, bits.v22) {
                Some(f) => bits.enable(f),
                None => features.push(format!("+{e}")),
            },
        }
    }
}

/// The features of a GNU `target("...")` attribute string on `AArch64`
/// over the module's features `module` (`+name`/`-name`), and its CPUs
/// (clang `AArch64TargetInfo::parseTargetAttr` + `getFunctionFeatureMap`
/// + `GetCPUAndFeaturesAttributes`; spec/driver.md "Target attribute",
/// WP-1187). The module's extension features seed an extension set (LLVM
/// `reconstructFromParsedFeatures`). Per comma-separated item:
/// - `arch=<arch>[+ext...]` (case-sensitive; unknown: nothing): the
///   architecture becomes the base and its default extensions are
///   enabled, then the `+ext` modifiers apply;
/// - `cpu=<cpu>[+ext...]` (case-sensitive): the function's `target-cpu`;
///   a known CPU becomes the base with its extensions, then its `+ext`;
///   a second `cpu=` is ignored;
/// - `tune=<cpu>`: the function's `tune-cpu` (a known CPU);
/// - `+ext+...`: modifiers; `fpmath=` and `branch-protection=`: nothing;
/// - otherwise an extension name (`-march=` spelling) enables it with its
///   dependencies, `no<ext>`/`no-<ext>` disables it with its dependents,
///   a sub-architecture `v8.Na`/`v9.Na` is `+v8.Na` plus its level's
///   features (clang 22 `setFeatureEnabled`; clang 19 enables the
///   extension ones in the set), and any other name passes through as
///   `-x` for `no-x`, else `+x` (raw LLVM features such as `spe`).
///
/// Then the base architecture's feature with the features it implies
/// (`setFeatureEnabled`) and the extension set's touched features follow,
/// so they win over a plain `+x`/`-x` naming one.
pub fn target_attr_features(
    attr: &str,
    module: &[String],
    v22: bool,
) -> kiln_codegen::TargetAttrFeatures {
    let mut bits =
        ExtensionSet { arch: (8, 0), v22, enabled: BTreeSet::new(), touched: BTreeSet::new() };
    for m in module {
        let (on, name) = match m.split_at_checked(1) {
            Some(("+", n)) => (true, n),
            Some((_, n)) => (false, n),
            None => continue,
        };
        let known = extension_feature(name, v22).or((name == "fmv").then_some("fmv"));
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
    // The base architecture's feature and the features it implies.
    let mut base: Option<(&'static str, Vec<&'static str>)> = None;
    let (mut cpu, mut tune) = (None::<String>, None::<String>);
    let mut cpu_written = false;
    for item in attr.split(',').map(str::trim) {
        if item.is_empty() || item.starts_with("fpmath=") || item.starts_with("branch-protection=")
        {
            continue;
        }
        if let Some(v) = item.strip_prefix("arch=") {
            let (name, exts) = v.trim().split_once('+').unwrap_or((v.trim(), ""));
            let Some(arch) = parse_arch(name, v22) else { continue };
            let defaults = ExtensionSet::new(arch, v22);
            let feature = defaults.enabled.iter().copied().find(|f| arch_of(f) == Some(arch));
            bits.arch = arch;
            // LLVM extensions without a modifier (`ccidx`) are touched too,
            // so they survive a later `cpu=`.
            for &f in &defaults.enabled {
                if is_extension_feature(f, v22) {
                    bits.enable(f);
                } else if arch_of(f).is_none() {
                    features.push(format!("+{f}"));
                }
            }
            let implied = defaults.enabled.into_iter().filter(|f| Some(*f) != feature).collect();
            base = Some((feature.unwrap_or("v8a"), implied));
            attr_modifiers(exts, &mut bits, &mut features);
            continue;
        }
        if let Some(v) = item.strip_prefix("cpu=") {
            let (name, exts) = v.trim().split_once('+').unwrap_or((v.trim(), ""));
            if cpu_written {
                continue;
            }
            cpu_written = !name.is_empty();
            if let Some(row) = attr_cpu_row(name, v22) {
                cpu = Some(name.to_owned());
                bits.arch = arch_of(row.arch).unwrap_or((8, 0));
                for f in list(row.driver) {
                    match extension_feature(f, v22).filter(|f| is_extension_feature(f, v22)) {
                        Some(f) => bits.enable(f),
                        None => features.push(format!("+{f}")),
                    }
                }
                base = Some((row.arch, list(row.closure).collect()));
                attr_modifiers(exts, &mut bits, &mut features);
            }
            continue;
        }
        if let Some(v) = item.strip_prefix("tune=") {
            if tune.as_deref().is_none_or(str::is_empty) {
                tune = Some(v.trim().to_owned());
            }
            continue;
        }
        if item.starts_with('+') {
            attr_modifiers(item, &mut bits, &mut features);
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
        } else {
            features.push(format!("+{item}"));
        }
    }
    if let Some((feature, implied)) = base {
        features.extend(implied.into_iter().map(|f| format!("+{f}")));
        features.push(format!("+{feature}"));
    }
    for f in &bits.touched {
        features.push(format!("{}{f}", if bits.enabled.contains(f) { '+' } else { '-' }));
    }
    // clang `GetCPUAndFeaturesAttributes`: a valid `tune=` CPU is the
    // `tune-cpu` (a valid `cpu=` clears the command line's).
    let tune = tune.filter(|t| attr_cpu_row(t, v22).is_some());
    kiln_codegen::TargetAttrFeatures { features, cpu, tune }
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
        // clang's `-###` order: the `ExtensionSet` features sorted.
        assert_eq!(feats("-mcpu=generic+sve a.cpp"), "true +ete,+fullfp16,+sve,+trbe");
        // `-march=` wins and `-mcpu=`'s extensions are ignored.
        assert_eq!(feats("-mcpu=generic+sve -march=armv8-a a.cpp"), "false ");
        assert_eq!(feats("-march=armv8-a+sve -mcpu=generic a.cpp"), "true +fullfp16,+sve");
    }

    /// spec/driver.md "Target features" (WP-1151): a CPU of clang's
    /// `AArch64TargetParser` has its extensions (`-mcpu=cortex-x2` has
    /// `+mte`), its modifiers apply over them with their dependencies, and
    /// an alias names its canonical CPU.
    #[test]
    fn mcpu_cpu_table() {
        let cpu = |s: &str| {
            let (f, e) = res(s, true);
            assert_eq!(e, [], "{s}");
            f
        };
        let has = |s: &str, x: &str| cpu(s).extra.iter().any(|e| e == x);
        for v in ["19", "22"] {
            let x2 = cpu(&format!("-fclang-compat={v} -mcpu=cortex-x2 a.cpp"));
            assert_eq!(x2.cpu.as_deref(), Some("cortex-x2"));
            assert!(x2.sve && x2.memtag_hw && x2.extra.iter().any(|e| e == "+v9a"));
            let s = format!("-fclang-compat={v} -mcpu=cortex-x2+nomemtag a.cpp");
            assert!(!cpu(&s).memtag_hw && has(&s, "-mte"));
            // `-march=` wins over the features, not the `target-cpu`.
            let m = cpu(&format!("-fclang-compat={v} -march=armv8-a -mcpu=cortex-x2 a.cpp"));
            assert_eq!(m.cpu.as_deref(), Some("cortex-x2"));
            assert!(!m.memtag_hw && !m.sve);
            assert_eq!(cpu(&format!("-fclang-compat={v} -mcpu=CORTEX-A57 a.cpp")).arch, (8, 0));
        }
        // An alias: clang 22's `apple-a18` is `apple-m4`.
        let a = cpu("-fclang-compat=22 -mcpu=apple-a18 a.cpp");
        assert_eq!(a.cpu.as_deref(), Some("apple-m4"));
        // The SME dependencies: clang 22's `sme` needs `fullfp16`, clang
        // 19's does not.
        assert!(has("-fclang-compat=22 -mcpu=apple-m4+nofp16 a.cpp", "-sme"));
        assert!(!has("-fclang-compat=19 -mcpu=apple-m4+nofp16 a.cpp", "-sme"));
        assert!(has("-fclang-compat=19 -mcpu=apple-m4+nobf16 a.cpp", "-sme"));
    }

    /// spec/driver.md "Target features" (WP-1151): clang's
    /// `hasTargetFeatureMTE` sees `-Xclang -target-feature +mte` at
    /// `-fclang-compat=22` only, and any `+mte` (no last-wins).
    #[test]
    fn xclang_mte_by_compat() {
        let mte = |s: &str| res(s, true).0.memtag_hw;
        let x = "-Xclang -target-feature -Xclang";
        assert!(!mte(&format!("-fclang-compat=19 {x} +mte a.cpp")));
        assert!(mte(&format!("-fclang-compat=22 {x} +mte a.cpp")));
        assert!(mte(&format!("-fclang-compat=22 {x} +mte {x} -mte a.cpp")));
        assert!(mte(&format!("-fclang-compat=19 -mcpu=cortex-x2 {x} -mte a.cpp")));
        assert!(!mte("-fclang-compat=22 -march=armv8.4-a a.cpp"));
    }

    /// spec/driver.md "Target features" (WP-1151): `-ffixed-x18` is
    /// `+reserve-x18` on `AArch64` and unsupported on x86-64.
    #[test]
    fn fixed_x18() {
        assert!(res("-ffixed-x18 a.cpp", true).0.extra.iter().any(|e| e == "+reserve-x18"));
        let (f, e) = res("--target=x86_64-linux-gnu -ffixed-x18 a.cpp", false);
        assert!(f.extra.is_empty());
        assert_eq!(
            e,
            [ArgError::UnsupportedForTarget("-ffixed-x18".into(), "x86_64-linux-gnu".into())]
        );
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
        assert_eq!(res("-mcpu=cortex-a1 a.cpp", true).1, [u("-mcpu=", "cortex-a1")]);
        // WP-1151: `-march=` does not hide an unknown CPU; the Armv8-R
        // CPUs are unsupported values, as `-march=armv8-r` is.
        assert_eq!(res("-march=armv8-a -mcpu=cortex-a1 a.cpp", true).1, [u("-mcpu=", "cortex-a1")]);
        assert_eq!(res("-mcpu=cortex-r82 a.cpp", true).1, [u("-mcpu=", "cortex-r82")]);
        assert_eq!(
            res("-fclang-compat=19 -mcpu=apple-a18 a.cpp", true).1,
            [u("-mcpu=", "apple-a18")]
        );
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

    /// WP-630, WP-1000: x86 `target("...")` strings over the default
    /// module features: the function's map as clang 19's (and clang 22's)
    /// `target-features` prints it (enabled features sorted, then disabled
    /// ones).
    fn md(module: &[String], v22: bool) -> kiln_codegen::TargetAttrModule<'_> {
        kiln_codegen::TargetAttrModule { features: module, cpu_features: &[], written: &[], v22 }
    }

    #[test]
    fn x86_target_attribute() {
        let module: Vec<String> =
            ["+cmov", "+cx8", "+fxsr", "+mmx", "+sse", "+sse2", "+x87"].map(String::from).into();
        let tv = |s: &str, v22: bool| {
            let mut m = BTreeMap::new();
            for e in x86_target_attr_features(s, &md(&module, v22)).features {
                m.insert(e[1..].to_owned(), e.starts_with('+'));
            }
            let on = m.iter().filter(|e| *e.1).map(|e| format!("+{}", e.0));
            let off = m.iter().filter(|e| !*e.1).map(|e| format!("-{}", e.0));
            on.chain(off).collect::<Vec<_>>().join(",")
        };
        let t = |s: &str| tv(s, false);
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
             +sse3,+sse4.1,+sse4.2,+ssse3,+x87,+xsave,-avx10.1-512,-evex512"
        );
        // clang 22: no `evex512` (WP-1000); its headers do not name it.
        assert_eq!(tv("sse2,no-evex512", true), base);
        assert_eq!(
            tv("avx512f,no-evex512", true),
            "+avx,+avx2,+avx512f,+cmov,+crc32,+cx8,+f16c,+fma,+fxsr,+mmx,+popcnt,+sse,+sse2,\
             +sse3,+sse4.1,+sse4.2,+ssse3,+x87,+xsave"
        );
        assert_eq!(t("aes,pclmul"), "+aes,+cmov,+cx8,+fxsr,+mmx,+pclmul,+sse,+sse2,+x87");
        assert_eq!(t("tune=generic,ssse3"), base);
        let a = x86_target_attr_features("arch=haswell,tune=znver4", &md(&module, true));
        assert_eq!((a.cpu.as_deref(), a.tune.as_deref()), (Some("haswell"), Some("znver4")));
        assert!(a.features.iter().any(|f| f == "+avx2"));
        // The attribute's own features come first.
        let own = x86_target_attr_features("popcnt,crc32", &md(&module, true)).features;
        assert_eq!(own[..2], ["+popcnt", "+crc32"]);
        // clang's `FeaturesAsWritten` come first: `-march=x86-64-v3
        // -mno-crc32` keeps crc32 off in a `target("sha")` function, and
        // an `arch=` CPU still gets them.
        let v3 = x86_features::cpu_features(x86_cpu("x86-64-v3").unwrap(), true);
        let cpu: Vec<String> = v3.iter().map(|&c| c.to_owned()).collect();
        let written = vec!["-crc32".to_owned()];
        let m = kiln_codegen::TargetAttrModule {
            features: &module,
            cpu_features: &cpu,
            written: &written,
            v22: true,
        };
        let has = |a: &str, f: &str| {
            x86_target_attr_features(a, &m)
                .features
                .iter()
                .rev()
                .any(|e| e[1..] == *f && e.starts_with('+'))
        };
        assert!(has("sha", "sha") && has("sha", "avx2") && !has("sha", "crc32"));
        assert!(!has("arch=haswell", "crc32") && has("arch=haswell", "movbe"));
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
        // extension touches nothing; `bogus` passes through (WP-1187).
        assert_eq!(
            t("no-neon,nosha2,arch=armv8-a,+crc,bogus", false),
            "-neon,+bogus,+fp-armv8,+neon,+v8a,+crc,-fmv,+fp-armv8,+neon"
        );
        assert_eq!(t("nosimd", true), "-fmv,+fp-armv8,-neon");
        // WP-1187: `arch=` is the base with its defaults, then `+ext`.
        assert_eq!(
            t("arch=armv8.2-a+sve", true),
            "+crc,+fp-armv8,+lse,+neon,+ras,+rdm,+v8.1a,+v8.2a,\
             +crc,-fmv,+fp-armv8,+fullfp16,+lse,+neon,+ras,+rdm,+sve"
        );
        // `cpu=` replaces the base; `arch=`'s touched extensions stay.
        let a = target_attr_features("arch=armv8.4-a,cpu=cortex-a57", &module, true);
        assert_eq!(a.cpu.as_deref(), Some("cortex-a57"));
        assert_eq!(
            a.features.join(","),
            "+ccidx,+v8a,+aes,+complxnum,+crc,+dit,+dotprod,+flagm,-fmv,+fp-armv8,\
             +jsconv,+lse,+neon,+pauth,+perfmon,+ras,+rcpc,+rdm,+sha2"
        );
        let a = target_attr_features("cpu=cortex-a57,tune=neoverse-v1,tune=x", &module, true);
        assert_eq!(
            (a.cpu.as_deref(), a.tune.as_deref()),
            (Some("cortex-a57"), Some("neoverse-v1"))
        );
        // Raw LLVM features pass through; `+ext` modifiers; an unknown
        // (case-mismatched) level is nothing.
        assert_eq!(t("spe,no-perfmon", true), "+spe,-perfmon,-fmv,+fp-armv8,+neon");
        assert_eq!(t("+sve2+nofp16", true), "-fmv,+fp-armv8,-fullfp16,+neon,-sve,-sve2");
        assert_eq!(t("arch=ARMv8.2-a", true), "-fmv,+fp-armv8,+neon");
        assert!(aarch64_attr_cpu_valid("cortex-r82", false));
        assert!(!aarch64_attr_cpu_valid("Neoverse-V1", true));
    }
}
