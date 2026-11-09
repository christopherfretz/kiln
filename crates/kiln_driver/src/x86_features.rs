//! The x86-64 target features of the command line (WP-1000;
//! spec/driver.md "x86-64 target features"): the `-m<feature>` /
//! `-mno-<feature>` flags of clang's `m_x86_Features_Group`, the feature
//! map clang's `X86TargetInfo::initFeatureMap` builds from the `-march=`
//! CPU and those flags, and the feature macros
//! `X86TargetInfo::getTargetDefines` derives from the map.
//!
//! The implication graph is the generated clang 21 table
//! ([`crate::x86_targets::implications`]) with the clang 19 and clang 22
//! differences measured with `clang-N -Xclang -target-feature`: clang 19
//! lacks the AVX10.2, AMX-AVX512/FP8/MOVRS/TF32 and MOVRS features, its
//! `avx10.1-256`/`-512` also imply AES, PCLMUL, VAES and VPCLMULQDQ and
//! its `avx512fp16` also AVX512DQ and AVX512VL; clang 22 names the AVX10
//! features `avx10.1` / `avx10.2` (no 256/512 split) and has no `evex512`
//! and no `cf`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The feature flag names (`-m<name>` / `-mno-<name>`) both clang 19 and
/// clang 22 accept, sorted. `sse4`, `80387`, `apxf`, `avx10.1`, `3dnow`
/// and `3dnowa` are spelt differently from their features (see
/// [`flag_items`]).
const FLAGS: &[&str] = &[
    "3dnow", "3dnowa", "80387", "adx", "aes", "amx-bf16", "amx-complex", "amx-fp16",
    "amx-int8", "amx-tile", "apxf", "avx", "avx10.1", "avx2", "avx512bf16", "avx512bitalg",
    "avx512bw", "avx512cd", "avx512dq", "avx512f", "avx512fp16", "avx512ifma", "avx512vbmi",
    "avx512vbmi2", "avx512vl", "avx512vnni", "avx512vp2intersect", "avx512vpopcntdq",
    "avxifma", "avxneconvert", "avxvnni", "avxvnniint16", "avxvnniint8", "bmi", "bmi2",
    "cldemote", "clflushopt", "clwb", "clzero", "cmpccxadd", "crc32", "cx16", "enqcmd", "f16c",
    "fma", "fma4", "fsgsbase", "fxsr", "gfni", "hreset", "invpcid", "kl", "lwp", "lzcnt", "mmx",
    "movbe", "movdir64b", "movdiri", "mwaitx", "pclmul", "pconfig", "pku", "popcnt",
    "prefetchi", "prfchw", "ptwrite", "raoint", "rdpid", "rdpru", "rdrnd", "rdseed", "rtm",
    "sahf", "serialize", "sgx", "sha", "sha512", "shstk", "sm3", "sm4", "sse", "sse2", "sse3",
    "sse4", "sse4.1", "sse4.2", "sse4a", "ssse3", "tbm", "tsxldtrk", "uintr", "usermsr", "vaes",
    "vpclmulqdq", "vzeroupper", "waitpkg", "wbnoinvd", "widekl", "x87", "xop", "xsave",
    "xsavec", "xsaveopt", "xsaves",
];

/// The flag names only clang 19 accepts (clang 22: unknown argument).
const FLAGS_19: &[&str] = &["avx10.1-256", "avx10.1-512", "evex512"];

/// The flag names only clang 22 accepts (clang 19: unknown argument).
const FLAGS_22: &[&str] = &["amx-avx512", "amx-fp8", "amx-movrs", "amx-tf32", "avx10.2", "movrs"];

/// The table features clang 19 does not have.
const NOT_19: &[&str] = &[
    "amx-avx512", "amx-fp8", "amx-movrs", "amx-tf32", "amx-transpose", "avx10.2-256",
    "avx10.2-512", "movrs",
];

/// The table features clang 22 does not have.
const NOT_22: &[&str] =
    &["amx-transpose", "avx10.1-256", "avx10.1-512", "avx10.2-256", "avx10.2-512", "cf", "evex512"];

/// The APX features `-mapxf` enables, in clang's order, and whose all
/// being on defines `__APX_F__`.
const APX_19: &[&str] = &["egpr", "push2pop2", "ppx", "ndd", "ccmp", "nf", "cf", "zu"];
const APX_22: &[&str] = &["egpr", "ndd", "ccmp", "nf", "zu", "push2pop2", "ppx"];

/// The default `x86-64` CPU's features.
pub const X86_64: [&str; 7] = ["cmov", "cx8", "fxsr", "mmx", "sse", "sse2", "x87"];

/// The `-m<name>` / `-mno-<name>` split of `arg`: `(enable, name)`.
fn split(arg: &str) -> Option<(bool, &str)> {
    match arg.strip_prefix("-mno-") {
        Some(n) => Some((false, n)),
        None => arg.strip_prefix("-m").map(|n| (true, n)),
    }
}

/// The values `-mapx-features=` / `-mno-apx-features=` take.
const APX_VALUES: &[&str] = &["egpr", "push2pop2", "ppx", "ndd", "ccmp", "nf", "cf", "zu"];

/// clang's `-mapx-inline-asm-use-gpr32` (no `-mno-` form): the
/// `inline-asm-use-gpr32` feature.
const GPR32: &str = "-mapx-inline-asm-use-gpr32";

/// Whether `arg` is an x86 feature flag clang 19 or clang 22 knows,
/// `-m[no-]apx-features=<list>` and `-mapx-inline-asm-use-gpr32` included.
pub fn is_flag(arg: &str) -> bool {
    arg == GPR32
        || split(arg).is_some_and(|(_, n)| {
            n.starts_with("apx-features=")
                || FLAGS.binary_search(&n).is_ok()
                || FLAGS_19.contains(&n)
                || FLAGS_22.contains(&n)
        })
}

/// The first value of `-m[no-]apx-features=<list>` flag `arg` that names
/// no APX feature, as `(option, value)` for clang's "unsupported argument
/// '…' to option '…'" error.
pub fn bad_value(arg: &str) -> Option<(String, String)> {
    let (on, name) = split(arg)?;
    let list = name.strip_prefix("apx-features=")?;
    let bad = list.split(',').filter(|v| !v.is_empty()).find(|v| !APX_VALUES.contains(v))?;
    Some((format!("{}apx-features=", if on { "-m" } else { "-mno-" }), bad.to_owned()))
}

/// Whether flag `arg` ([`is_flag`]) is one the `-fclang-compat=` clang
/// lacks (an unknown argument there).
pub fn unknown_at(arg: &str, v22: bool) -> bool {
    split(arg).is_some_and(|(_, n)| (if v22 { FLAGS_19 } else { FLAGS_22 }).contains(&n))
}

/// Whether flag `arg` is `-m3dnow`/`-m3dnowa` (with or without `no-`):
/// clang accepts and drops them, warning on x86 that it does not support
/// the enabling ones.
pub fn is_3dnow(arg: &str) -> bool {
    split(arg).is_some_and(|(_, n)| n == "3dnow" || n == "3dnowa")
}

/// The `-target-feature` items (`+name` / `-name`) clang's driver turns
/// flag `arg` into: `-msse4` is `+sse4.2` and `-mno-sse4` `-sse4.1`,
/// `-m80387` is `x87`, `-mapxf` the APX features (`-mapx-features=` the
/// listed ones), clang 19's `-mavx10.1`
/// `avx10.1-256`, `-m3dnow`/`-m3dnowa` nothing.
fn flag_items(arg: &str, v22: bool) -> Vec<String> {
    if arg == GPR32 {
        return vec!["+inline-asm-use-gpr32".to_owned()];
    }
    let Some((on, name)) = split(arg) else { return Vec::new() };
    let sign = if on { '+' } else { '-' };
    let names: Vec<&str> = match name {
        "3dnow" | "3dnowa" => Vec::new(),
        "sse4" => vec![if on { "sse4.2" } else { "sse4.1" }],
        "80387" => vec!["x87"],
        "apxf" => (if v22 { APX_22 } else { APX_19 }).to_vec(),
        n if n.starts_with("apx-features=") => {
            let list = &n["apx-features=".len()..];
            list.split(',').filter(|v| APX_VALUES.contains(v)).collect()
        }
        "avx10.1" if !v22 => vec!["avx10.1-256"],
        n => vec![n],
    };
    names.into_iter().map(|n| format!("{sign}{n}")).collect()
}

/// How clang names flag `arg` in its "unsupported option '…' for target"
/// error on a non-x86 target, or `None` for the flags clang only warns
/// about there ("argument unused during compilation": `-msse4`,
/// `-msse4.2`, `-m[no-]3dnow[a]`).
pub fn non_x86_spelling(arg: &str, v22: bool) -> Option<String> {
    if arg == GPR32 {
        return Some(GPR32.to_owned());
    }
    let (on, name) = split(arg)?;
    let prefix = if on { "-m" } else { "-mno-" };
    let name = match name {
        "3dnow" | "3dnowa" => return None,
        "sse4" | "sse4.2" if on => return None,
        "sse4" => "sse4.1",
        "80387" => "x87",
        "apxf" if !v22 => "apx-features=",
        n if n.starts_with("apx-features=") => "apx-features=",
        "avx10.1" if !v22 => "avx10.1-256",
        n => n,
    };
    Some(format!("{prefix}{name}"))
}

/// clang's driver `unifyTargetFeatures`: the last item naming each
/// feature wins, the survivors keeping their order.
fn unify(items: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (i, f) in items.iter().enumerate() {
        let later = items[i + 1..].iter().any(|g| g.get(1..) == f.get(1..));
        if !later {
            out.push(f.clone());
        }
    }
    out
}

/// The `-target-feature` items of the x86 feature flags `flags` (as
/// written, in order) at `-fclang-compat=22` or `19`, after clang's
/// driver dedup. Flags that clang lacks are skipped (the caller reports
/// them).
pub fn driver_items(flags: &[String], v22: bool) -> Vec<String> {
    let items =
        flags.iter().filter(|f| !unknown_at(f, v22)).flat_map(|f| flag_items(f, v22)).collect();
    unify(items)
}

/// The implication graph of clang 19 (`v22` false) or clang 22: each
/// feature to the features enabling it enables, itself included.
fn graph(v22: bool) -> &'static BTreeMap<&'static str, Vec<&'static str>> {
    static G19: OnceLock<BTreeMap<&'static str, Vec<&'static str>>> = OnceLock::new();
    static G22: OnceLock<BTreeMap<&'static str, Vec<&'static str>>> = OnceLock::new();
    let build = || {
        let drop = if v22 { NOT_22 } else { NOT_19 };
        let rename = |f: &'static str| -> &'static str {
            match f {
                "avx10.1-256" | "avx10.1-512" if v22 => "avx10.1",
                "avx10.2-256" | "avx10.2-512" if v22 => "avx10.2",
                f => f,
            }
        };
        let mut g: BTreeMap<&'static str, Vec<&'static str>> = BTreeMap::new();
        for (name, list) in crate::x86_targets::implications() {
            let key = match name {
                "avx10.1-256" | "avx10.2-256" if v22 => rename(name),
                n if drop.contains(&n) => continue,
                n => n,
            };
            let mut l: Vec<&'static str> = Vec::new();
            for f in list.iter() {
                let f = rename(f);
                if !drop.contains(&f) && !l.contains(&f) {
                    l.push(f);
                }
            }
            if !v22 && (list.contains("avx10.1-256")) {
                l.extend(["aes", "pclmul", "vaes", "vpclmulqdq"]);
            }
            if !v22 && list.contains("avx512fp16") {
                l.extend(["avx512dq", "avx512vl"]);
            }
            l.sort_unstable();
            l.dedup();
            g.insert(key, l);
        }
        g
    };
    if v22 { G22.get_or_init(build) } else { G19.get_or_init(build) }
}

/// clang's `X86TargetInfo::setFeatureEnabled`: enabling `name` enables
/// what it implies; disabling it disables every feature implying it.
fn set_enabled(map: &mut BTreeMap<String, bool>, name: &str, on: bool, v22: bool) {
    let g = graph(v22);
    map.insert(name.to_owned(), on);
    if on {
        for f in g.get(name).into_iter().flatten() {
            map.insert((*f).to_owned(), true);
        }
    } else {
        for (f, l) in g {
            if l.contains(&name) {
                map.insert((*f).to_owned(), false);
            }
        }
    }
}

/// How clang 19's and clang 22's `-march=` CPU features differ from the
/// generated clang 21 rows ([`crate::x86_targets`]): (CPU, added, removed).
const CPU_19: &[(&str, &[&str], &[&str])] = &[
    ("alderlake", &["cldemote"], &[]),
    ("arrowlake", &["cldemote"], &[]),
    ("arrowlake-s", &["cldemote"], &[]),
    ("clearwaterforest", &["kl", "widekl"], &[]),
    ("gracemont", &["cldemote"], &[]),
    ("lunarlake", &["cldemote"], &[]),
    ("meteorlake", &["cldemote"], &[]),
    ("pantherlake", &["cldemote", "kl", "widekl"], &[]),
    ("raptorlake", &["cldemote"], &[]),
];
const CPU_22: &[(&str, &[&str], &[&str])] = &[("diamondrapids", &[], &["usermsr"]), ("pantherlake", &[], &["prefetchi"])];

/// The features of `-march=` CPU `cpu` as clang 19 (`v22` false) or clang
/// 22 has them: the clang 21 row with [`CPU_19`] / [`CPU_22`] applied.
pub fn cpu_features(cpu: crate::x86_targets::Cpu, v22: bool) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = cpu.features.iter().collect();
    let table = if v22 { CPU_22 } else { CPU_19 };
    if let Some((_, add, remove)) = table.iter().find(|(n, ..)| *n == cpu.name) {
        out.retain(|f| !remove.contains(f));
        out.extend_from_slice(add);
    }
    out
}

/// Linux `/proc/cpuinfo` `flags` names whose clang feature name differs
/// (the others are used as they are when they name a clang feature).
const CPUINFO: &[(&str, &str)] = &[
    ("abm", "lzcnt"),
    ("amx_bf16", "amx-bf16"),
    ("amx_complex", "amx-complex"),
    ("amx_fp16", "amx-fp16"),
    ("amx_int8", "amx-int8"),
    ("amx_tile", "amx-tile"),
    ("avx512_bf16", "avx512bf16"),
    ("avx512_bitalg", "avx512bitalg"),
    ("avx512_fp16", "avx512fp16"),
    ("avx512_vbmi2", "avx512vbmi2"),
    ("avx512_vnni", "avx512vnni"),
    ("avx512_vp2intersect", "avx512vp2intersect"),
    ("avx512_vpopcntdq", "avx512vpopcntdq"),
    ("avx_vnni", "avxvnni"),
    ("bmi1", "bmi"),
    ("fpu", "x87"),
    ("lahf_lm", "sahf"),
    ("pclmulqdq", "pclmul"),
    ("pni", "sse3"),
    ("prefetchw", "prfchw"),
    ("rdrand", "rdrnd"),
    ("sha_ni", "sha"),
    ("sse4_1", "sse4.1"),
    ("sse4_2", "sse4.2"),
    ("user_shstk", "shstk"),
];

/// The x86-64 features `-march=native` enables on a Linux x86-64 host
/// whose `/proc/cpuinfo` is `cpuinfo`: [`X86_64`] plus each feature of the
/// first `flags` line clang knows (spec/driver.md "x86-64 target
/// features"). Unknown flags are skipped.
pub fn native_features(cpuinfo: &str, v22: bool) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = X86_64.to_vec();
    let Some(line) = cpuinfo.lines().find_map(|l| l.strip_prefix("flags")) else { return out };
    let flags = line.trim_start().strip_prefix(':').unwrap_or("");
    let g = graph(v22);
    for flag in flags.split_whitespace() {
        let name = CPUINFO.iter().find(|(c, _)| *c == flag).map_or(flag, |(_, n)| n);
        if let Some((&n, _)) = g.get_key_value(name) {
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

/// The feature map of x86-64 CPU features `cpu` (the `-march=` CPU's, or
/// [`X86_64`]) and the driver's `-target-feature` items `items`,
/// as clang 19's or clang 22's
/// `X86TargetInfo::initFeatureMap` builds it:
///
/// - `sse2` (x86-64), then the CPU's features, are enabled.
/// - clang 19 holds the `evex512` items back and applies the AVX10 items
///   after the others; then `+evex512` follows when an AVX-512 feature is
///   on (`-evex512` when an item disabled it), else with an AVX10 feature
///   `evex512` follows `avx10.1-512`.
/// - Each item is applied with [`set_enabled`].
/// - Then `sse4.2` enables `popcnt` and `crc32`, `sse` `mmx` and `avx`
///   `xsave`, unless an item disabled that feature.
pub fn feature_map(cpu: &[&str], items: &[String], v22: bool) -> BTreeMap<String, bool> {
    let mut map: BTreeMap<String, bool> = BTreeMap::new();
    set_enabled(&mut map, "sse2", true, v22);
    let drop = if v22 { NOT_22 } else { NOT_19 };
    for f in cpu.iter().filter(|f| !drop.contains(f)) {
        set_enabled(&mut map, f, true, v22);
    }
    let items = if v22 { items.to_vec() } else { evex512_items(&map, items) };
    for f in &items {
        if let Some((sign, name)) = f.split_at_checked(1) {
            set_enabled(&mut map, name, sign == "+", v22);
        }
    }
    let disabled = |n: &str| items.iter().any(|f| f.strip_prefix('-') == Some(n));
    let adds = [("sse4.2", "popcnt"), ("sse", "mmx"), ("avx", "xsave"), ("sse4.2", "crc32")];
    for (cond, add) in adds {
        if map.get(cond) == Some(&true) && !disabled(add) {
            map.insert(add.to_owned(), true);
        }
    }
    map
}

/// clang 19's reordering of the items over the CPU's map `map`: the
/// `evex512` items held back, the AVX10 ones last, then the resulting
/// `evex512` item.
fn evex512_items(map: &BTreeMap<String, bool>, items: &[String]) -> Vec<String> {
    let on = |f: &str| map.get(f) == Some(&true);
    let mut avx512 = on("avx512f");
    let (mut avx10, mut avx10_512) = (on("avx10.1-256"), on("avx10.1-512"));
    let mut evex512 = None;
    let (mut out, mut avx10_items) = (Vec::new(), Vec::new());
    for f in items {
        if f.get(1..7) == Some("avx10.") {
            if f.starts_with('+') {
                avx10 = true;
                avx10_512 |= f.ends_with("512");
            } else if avx10 && f == "-avx10.1-256" {
                (avx10, avx10_512) = (false, false);
            } else if avx10_512 && f == "-avx10.1-512" {
                avx10_512 = false;
            }
            avx10_items.push(f.clone());
            continue;
        }
        if !avx512 && f.starts_with("+avx512") {
            avx512 = true;
        } else if avx512 && f == "-avx512f" {
            avx512 = false;
        } else if f == "+evex512" {
            evex512 = Some(true);
            continue;
        } else if f == "-evex512" {
            evex512 = Some(false);
            continue;
        }
        out.push(f.clone());
    }
    out.append(&mut avx10_items);
    if avx512 {
        out.push(if evex512 == Some(false) { "-evex512" } else { "+evex512" }.to_owned());
    } else if avx10 {
        out.push(if avx10_512 { "+evex512" } else { "-evex512" }.to_owned());
    }
    out
}

/// The `-msse2`-level `_Float16` macros clang defines with SSE2.
const FLT16: &[(&str, &str)] = &[
    ("__FLT16_DECIMAL_DIG__", "5"),
    ("__FLT16_DENORM_MIN__", "5.9604644775390625e-8F16"),
    ("__FLT16_DIG__", "3"),
    ("__FLT16_EPSILON__", "9.765625e-4F16"),
    ("__FLT16_HAS_DENORM__", "1"),
    ("__FLT16_HAS_INFINITY__", "1"),
    ("__FLT16_HAS_QUIET_NAN__", "1"),
    ("__FLT16_MANT_DIG__", "11"),
    ("__FLT16_MAX_10_EXP__", "4"),
    ("__FLT16_MAX_EXP__", "16"),
    ("__FLT16_MAX__", "6.5504e+4F16"),
    ("__FLT16_MIN_10_EXP__", "(-4)"),
    ("__FLT16_MIN_EXP__", "(-13)"),
    ("__FLT16_MIN__", "6.103515625e-5F16"),
    ("__FLT16_NORM_MAX__", "6.5504e+4F16"),
];

/// The features defining no macro.
const NO_MACRO: &[&str] = &[
    "cmov", "cx8", "x87", "evex512", "vzeroupper", "retpoline-indirect-calls", "retpoline-indirect-branches",
    "retpoline-external-thunk", "lvi-cfi", "seses", "inline-asm-use-gpr32",
];

/// The feature macros (name, value) of feature map `map` at clang 19 or
/// clang 22 (clang `X86TargetInfo::getTargetDefines`, measured per
/// feature): `__NAME__` 1 with `.` and `-` as `_`, except `sse` (also
/// `__SSE_MATH__`), `sse2` (`__SSE2_MATH__` and the `_Float16` macros),
/// `cx16` (`__GCC_HAVE_SYNC_COMPARE_AND_SWAP_16`), `sahf`
/// (`__LAHF_SAHF__`), the AVX10 ones (clang 19 `__AVX10_1__` /
/// `__AVX10_1_512__`; clang 22 also the `_512` one per level), clang 19's
/// `__EVEX512__` (`evex512`) and `__EVEX256__` (`avx512vl` or
/// `avx10.1-256`), and `__APX_F__` with every APX feature.
pub fn feature_macros(map: &BTreeMap<String, bool>, v22: bool) -> BTreeMap<String, String> {
    let on = |f: &str| map.get(f) == Some(&true);
    let mut m: BTreeMap<String, String> = BTreeMap::new();
    let mut def = |n: &str, v: &str| {
        m.insert(n.to_owned(), v.to_owned());
    };
    for (f, _) in map.iter().filter(|e| *e.1) {
        match f.as_str() {
            f if NO_MACRO.contains(&f) => {}
            "sse" => {
                def("__SSE__", "1");
                def("__SSE_MATH__", "1");
            }
            "sse2" => {
                def("__SSE2__", "1");
                def("__SSE2_MATH__", "1");
                for (n, v) in FLT16 {
                    def(n, v);
                }
            }
            "cx16" => def("__GCC_HAVE_SYNC_COMPARE_AND_SWAP_16", "1"),
            "sahf" => def("__LAHF_SAHF__", "1"),
            "avx10.1-256" => def("__AVX10_1__", "1"),
            "avx10.1-512" => def("__AVX10_1_512__", "1"),
            "avx10.1" | "avx10.2" => {
                let l = &f[6..7];
                def(&format!("__AVX10_{l}__"), "1");
                def(&format!("__AVX10_{l}_512__"), "1");
            }
            f => def(&format!("__{}__", f.to_ascii_uppercase().replace(['.', '-'], "_")), "1"),
        }
    }
    if !v22 {
        if on("evex512") {
            def("__EVEX512__", "1");
        }
        if on("avx512vl") || on("avx10.1-256") {
            def("__EVEX256__", "1");
        }
    }
    if (if v22 { APX_22 } else { APX_19 }).iter().all(|f| on(f)) {
        def("__APX_F__", "1");
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feats(cpu: &[&str], items: &[&str], v22: bool) -> String {
        let items: Vec<String> = items.iter().map(|&s| s.to_owned()).collect();
        let map = feature_map(cpu, &items, v22);
        let on = map.iter().filter(|e| *e.1).map(|e| format!("+{}", e.0));
        let off = map.iter().filter(|e| !*e.1).map(|e| format!("-{}", e.0));
        on.chain(off).collect::<Vec<_>>().join(",")
    }

    #[test]
    fn flags_sorted() {
        assert!(FLAGS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn driver_dedup() {
        let f = |s: &str| s.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(driver_items(&f("-mavx -mno-avx"), true), ["-avx"]);
        assert_eq!(driver_items(&f("-mavx2 -mno-avx -mavx2"), true), ["-avx", "+avx2"]);
        assert_eq!(driver_items(&f("-msse4 -mno-sse4"), true), ["+sse4.2", "-sse4.1"]);
        assert_eq!(driver_items(&f("-mavx10.1 -mevex512 -mmovrs"), false), ["+avx10.1-256", "+evex512"]);
        assert_eq!(driver_items(&f("-mavx10.1 -mevex512 -mmovrs"), true), ["+avx10.1", "+movrs"]);
    }

    /// clang 19's / 22's `"target-features"` (`-S -emit-llvm`).
    #[test]
    fn maps() {
        let base = "+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87";
        assert_eq!(feats(&X86_64, &[], true), base);
        let avx2 = "+avx,+avx2,+cmov,+crc32,+cx8,+fxsr,+mmx,+popcnt,+sse,+sse2,+sse3,+sse4.1,\
                    +sse4.2,+ssse3,+x87,+xsave";
        assert_eq!(feats(&X86_64, &["+avx2"], false), avx2);
        assert_eq!(feats(&X86_64, &["+avx2"], true), avx2);
        let f = feats(&X86_64, &["+avx512f"], false);
        assert!(f.contains("+evex512,+f16c,+fma"), "{f}");
        assert!(!feats(&X86_64, &["+avx512f"], true).contains("evex512"));
        let f = feats(&X86_64, &["-avx"], true);
        assert!(f.contains("-avx10.1,-avx10.2,-avx2,") && !f.contains("-avx10.1-256"), "{f}");
        let f = feats(&X86_64, &["-avx"], false);
        assert!(f.contains("-avx10.1-256,-avx10.1-512,-avx2,") && !f.contains("avx10.2"), "{f}");
    }

    #[test]
    fn macros() {
        let m = |items: &[&str], v22: bool| {
            let items: Vec<String> = items.iter().map(|&s| s.to_owned()).collect();
            feature_macros(&feature_map(&X86_64, &items, v22), v22)
        };
        let base = m(&[], true);
        assert!(base.contains_key("__SSE2_MATH__") && base.contains_key("__FLT16_MAX__"));
        assert!(base.contains_key("__MMX__") && !base.contains_key("__X87__"));
        let a = m(&["+avx512vl"], false);
        assert!(a.contains_key("__EVEX512__") && a.contains_key("__EVEX256__"));
        let a = m(&["+avx512vl"], true);
        assert!(!a.contains_key("__EVEX512__") && a.contains_key("__AVX512VL__"));
        let apx = ["+egpr", "+push2pop2", "+ppx", "+ndd", "+ccmp", "+nf", "+zu"];
        assert!(m(&apx, true).contains_key("__APX_F__"));
        assert!(!m(&apx, false).contains_key("__APX_F__"));
        assert!(m(&["+cx16", "+sahf"], true).contains_key("__LAHF_SAHF__"));
        let a = m(&["+avx10.2"], true);
        assert!(a.contains_key("__AVX10_2_512__") && a.contains_key("__AVX10_1__"));
    }

    /// spec/driver.md "x86-64 target features": `-march=native` maps the
    /// Linux `flags` names to clang's and skips the unknown ones.
    #[test]
    fn native() {
        let info = "processor\t: 0\nflags\t\t: fpu cx8 sse4_1 sse4_2 pni abm bmi1 sha_ni avx2 lm nx_bogus\nflags : avx512f\n";
        let f = native_features(info, true);
        assert_eq!(f, ["cmov", "cx8", "fxsr", "mmx", "sse", "sse2", "x87", "sse4.1", "sse4.2", "sse3", "lzcnt", "bmi", "sha", "avx2"]);
        let map = feature_map(&f, &[], true);
        assert_eq!(map.get("avx"), Some(&true));
        assert_eq!(map.get("avx512f"), None);
        assert_eq!(native_features("", false), X86_64);
    }

    #[test]
    fn spellings() {
        assert_eq!(non_x86_spelling("-mavx2", true).as_deref(), Some("-mavx2"));
        assert_eq!(non_x86_spelling("-mno-sse4", true).as_deref(), Some("-mno-sse4.1"));
        assert_eq!(non_x86_spelling("-m80387", true).as_deref(), Some("-mx87"));
        assert_eq!(non_x86_spelling("-mapxf", false).as_deref(), Some("-mapx-features="));
        assert_eq!(non_x86_spelling("-mapxf", true).as_deref(), Some("-mapxf"));
        assert_eq!(non_x86_spelling("-mavx10.1", false).as_deref(), Some("-mavx10.1-256"));
        assert_eq!(non_x86_spelling("-msse4.2", true), None);
        assert_eq!(non_x86_spelling("-mno-3dnow", true), None);
        assert!(is_flag("-mno-avx512vp2intersect") && !is_flag("-mfoo") && !is_flag("-m64"));
        assert!(unknown_at("-mevex512", true) && unknown_at("-mmovrs", false));
    }
}
