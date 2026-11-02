//! The driver-level IR-diff corpus (spec/codegen.md §6, "IR-diff corpus").
//!
//! Every `tests/codegen/*.cpp` is compiled in-process by the kiln driver,
//! as `kiln++` (`kiln_driver::run`), with
//! `-S -emit-llvm -O0 -std=c++26 --target=<triple>` for each triple in
//! [`TRIPLES`], and by `clang++-N` with the same flags. Both outputs go
//! through `opt-N -S -passes=verify` and are compared in the oracle's
//! comparison mode ([`Mode`], spec/codegen.md §6 "Comparison modes"):
//! byte-identical after [`ir_norm::base`] for oracle 19 (kiln's IR is
//! clang 19's), and under oracle 22 equal after [`ir_norm::structural`]
//! (attributes, `align`, GEP/arithmetic flags, metadata and datalayout
//! stripped) with the strict [`ir_norm::abi_slice`] (calling convention,
//! signatures, `byval`/`sret`/`inreg`, named types, datalayout, mangled
//! names) compared verbatim. A `(case, triple, oracle)` listed in
//! [`DEVIATIONS`] lowers the requirement to what its [`Allow`] says; kiln's
//! IR must still verify. Without `opt-N` / `clang++-N` the comparison is
//! skipped.
//!
//! N is the oracle release, `KILN_ORACLE` (`22`, the default, or `19`;
//! spec/harness.md "Oracle toolchain"), read here directly since a test
//! crate cannot use xtask's `oracle` module.
//!
//! `KILN_CASE=<name>` (the file stem) runs one case and prints both IR
//! texts for each triple. A `//@ args: <opts>` line in a case adds
//! `<opts>` (whitespace-separated) to both command lines, after the
//! [`pinned_flags`].
//!
//! A `//@ module: <name>=<path>` line (path relative to the case) names a
//! module interface unit the case imports: kiln builds it in-process from
//! `-fmodule-file=<name>=<path>`; clang first precompiles it
//! (`--precompile`, same flags, with the earlier lines' modules) into a
//! scratch BMI and gets `-fmodule-file=<name>=<bmi>`. List the lines in
//! dependency order (spec/codegen.md §6, "IR-diff corpus").
//!
//! A `//@ min-oracle: <N>` line compares the case only under oracle
//! release N or later; under an older oracle kiln's IR must still verify
//! (the case follows a newer clang where the older one differs).

mod ir_norm;

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kiln_diag::SharedBuffer;
use kiln_driver::{Io, SharedWriter, run};

const TRIPLES: &[&str] = &["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"];

/// What a listed `(case, triple, oracle)` may differ in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Allow {
    /// The instructions (the oracle's required mode fails); the ABI slice
    /// must still match.
    Structure,
    /// Everything but verification, the ABI slice included.
    Abi,
    /// The oracle's own IR fails `opt-N -passes=verify` (a clang defect);
    /// only kiln's IR is verified.
    OracleInvalid,
}

/// `(case, triple, oracle, allow)`: pairs whose IR differs from clang
/// `oracle`'s by a documented deviation (spec/codegen.md "Deviations").
const DEVIATIONS: &[(&str, &str, &str, Allow)] = &[
    // clang 19's AArch64 -O0 pipeline runs `loop(loop-idiom-vectorize)`,
    // whose LoopSimplify/LCSSA canonicalization reshapes the array loops
    // of the (non-`optnone`) global initialization functions.
    ("global_init_array_eh", "aarch64-unknown-linux-gnu", "19", Allow::Structure),
    // The same canonicalization of `__cxx_global_array_dtor`'s
    // partial-array EH cleanup loop (WP-205).
    ("eh_array_destroy_throwing_global", "aarch64-unknown-linux-gnu", "19", Allow::Structure),
    // The same, for the EH cleanup of a static array temporary's
    // initialization (WP-216).
    ("array_temporary_global", "aarch64-unknown-linux-gnu", "19", Allow::Structure),
    // kiln zeroes a padded `_Atomic` operand's padding and writes a failed
    // compare-exchange's old value back to `*expected` (C11 7.17.7.4p2) at
    // every compat level, as clang 22 does; clang 19 did neither.
    ("atomic_padded", "aarch64-unknown-linux-gnu", "19", Allow::Structure),
    ("atomic_padded", "x86_64-unknown-linux-gnu", "19", Allow::Structure),
    // clang 20+ passes an AArch64 empty class by value as `i64` (GCC's
    // convention; `-fclang-abi-compat=19` keeps the old `i8`). kiln follows
    // clang 19 until codegen honours `-fclang-compat=22` (WP-184 residual).
    ("abi_decls", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("assign_rhs_first", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("dependent_signature", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("empty_base_copy", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("empty_copy_std_builtin_constexpr", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("global_init_count_new", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("global_init_static_local", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("namespace_reopen_inline", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("template_out_of_line", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("udc_conversions", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("virtual_template_out_of_line", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    // LLVM 20+ passes x86-64 `__int128` as `i128` instead of `{ i64, i64 }`
    // / two `i64`s (the same registers and stack slots: SysV §3.2.3).
    ("abi_decls", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    ("atomic_builtins", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    ("atomic_lv_libcall", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    ("atomic_wide", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    ("builtins_bits", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    ("builtins_overflow", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    ("int128", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    ("int128_ops", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    ("new_placement_nothrow", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    ("va_arg", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    // clang 20+ coerces an AArch64 aggregate of pointers to `ptr` /
    // `[N x ptr]` instead of `i64` / `[N x i64]` (the same registers).
    ("srcloc_immediate", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("struct_pointers", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    // clang 22 names members of an unnamed class `(unnamed struct)::N` in
    // IR type names (19: `(anonymous struct)`).
    ("lambda_names", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("lambda_names", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    // clang 20+ implements CWG2137 ([over.match.list], [over.ics.list]):
    // `B{b}` picks `B(std::initializer_list<B>)` where clang 19 and kiln
    // copy-construct (CWG1467 alone). kiln is the one to change (sema;
    // WP-184 residual).
    ("dr1467_single_element", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("dr1467_single_element", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    // clang 22 lowers `modf` to the `llvm.modf` intrinsic (19: a `modff`
    // libcall).
    ("builtins_math", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("builtins_math", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    // clang 22 instruction-selection changes that kiln has not followed:
    // `alloca` of the atomic flag / order temporaries, P0528 padding
    // clearing (`memset`) of atomic objects, `i1` (19: `i8`) overflow-result
    // truncation, `undef` (19: `poison`) ext-vector widening shuffles,
    // `select` for vector `?:`, constant-folded vector reductions, and the
    // block order of `delete`'s null check.
    ("atomic_builtins", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("atomic_orders", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("atomic_orders", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("atomic_padded", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("atomic_padded", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("builtins_overflow", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("builtins_shufflevector", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("builtins_shufflevector", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("ext_vector_element", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("ext_vector_element", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("vector_abi", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("vector_abi", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("vector_cond", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("vector_cond", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("vector_const", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("vector_const", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("new_array_cookie", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("new_array_cookie", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("new_delete_replaced", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("new_delete_replaced", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    // clang 22 emits `insertelement <8 x i1> .., i8 1, ..` for a bool
    // vector element store, which opt-22 rejects.
    ("vector_ops", "aarch64-unknown-linux-gnu", "22", Allow::OracleInvalid),
    ("vector_ops", "x86_64-unknown-linux-gnu", "22", Allow::OracleInvalid),
];

/// Flags given to both compilers ahead of a case's own, to pin target
/// defaults that clang derives from the host rather than the triple.
///
/// clang 19 turns `AArch64`'s `+outline-atomics` on by default only when it
/// finds a GCC >= 9.3.1 installation for the target (or compiler-rt):
/// natively on an `AArch64` host it does, cross-compiling from x86-64 (the
/// GitHub runner) it does not. kiln defaults to on; the explicit flag
/// makes the comparison host-independent.
fn pinned_flags(triple: &str) -> &'static [&'static str] {
    if triple.starts_with("aarch64") { &["-moutline-atomics"] } else { &[] }
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/codegen")
}

/// The case's extra options, from its `//@ args:` lines (split on whitespace).
fn extra_flags(file: &Path) -> Vec<String> {
    let src = std::fs::read_to_string(file).unwrap_or_default();
    src.lines()
        .filter_map(|l| l.strip_prefix("//@ args:"))
        .flat_map(str::split_whitespace)
        .map(str::to_owned)
        .collect()
}

/// The case's `//@ module: <name>=<path>` lines, paths resolved against
/// the case's directory, in order.
fn module_inputs(file: &Path) -> Vec<(String, PathBuf)> {
    let src = std::fs::read_to_string(file).unwrap_or_default();
    let dir = file.parent().unwrap_or(Path::new("."));
    src.lines()
        .filter_map(|l| l.strip_prefix("//@ module:"))
        .filter_map(|m| m.trim().split_once('='))
        .map(|(n, p)| (n.trim().to_owned(), dir.join(p.trim())))
        .collect()
}

/// The case's `//@ min-oracle: <N>` release, if any.
fn min_oracle(file: &Path) -> Option<u32> {
    let src = std::fs::read_to_string(file).unwrap_or_default();
    src.lines().find_map(|l| l.strip_prefix("//@ min-oracle:")).and_then(|v| v.trim().parse().ok())
}

/// Compiles `file` for `triple` with the in-process driver.
fn kiln_ir(file: &Path, triple: &str) -> Result<String, String> {
    let out = SharedBuffer::new();
    let err = SharedBuffer::new();
    let io = Io {
        stdout: Box::new(out.clone()),
        stderr: SharedWriter::new(err.clone()),
        stdin: Box::new(std::io::empty()),
    };
    let args: Vec<String> =
        ["-S", "-emit-llvm", "-O0", "-std=c++26", &format!("--target={triple}"), "-o", "-"]
            .iter()
            .chain(pinned_flags(triple))
            .map(|s| (*s).to_owned())
            .chain(extra_flags(file))
            .chain(
                module_inputs(file)
                    .into_iter()
                    .map(|(n, p)| format!("-fmodule-file={n}={}", p.display())),
            )
            .chain(std::iter::once(file.display().to_string()))
            .collect();
    let code = run(&args, io);
    if code != 0 {
        return Err(format!("kiln exited {code}:\n{}", err.contents()));
    }
    Ok(out.contents())
}

/// Runs `prog args` with `input` on stdin; `None` when `prog` is missing.
fn pipe(prog: &str, args: &[&str], input: &str) -> Option<Result<String, String>> {
    let Ok(mut child) = Command::new(prog)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return None;
    };
    {
        let mut stdin = child.stdin.take().expect("stdin");
        let _ = stdin.write_all(input.as_bytes());
    }
    let out = child.wait_with_output().expect("wait");
    if out.status.success() {
        Some(Ok(String::from_utf8_lossy(&out.stdout).into_owned()))
    } else {
        Some(Err(String::from_utf8_lossy(&out.stderr).into_owned()))
    }
}

/// The oracle release: `KILN_ORACLE`, `22` when unset or empty.
fn oracle_version() -> &'static str {
    match std::env::var("KILN_ORACLE").as_deref().map(str::trim) {
        Err(_) | Ok("" | "22") => "22",
        Ok("19") => "19",
        Ok(v) => panic!("KILN_ORACLE=`{v}` is not a known oracle release (accepted: 19 or 22)"),
    }
}

/// `opt-N` of the oracle release.
fn opt_tool() -> String {
    format!("opt-{}", oracle_version())
}

/// `clang++-N` of the oracle release.
fn clang_tool() -> String {
    format!("clang++-{}", oracle_version())
}

fn opt(ir: &str) -> Option<Result<String, String>> {
    pipe(&opt_tool(), &["-S", "-passes=verify", "-o", "-"], ir)
}

fn clang_ir(file: &Path, triple: &str) -> Option<Result<String, String>> {
    let target = format!("--target={triple}");
    let path = file.display().to_string();
    let extra = extra_flags(file);
    let mut base = vec!["-O0", "-std=c++26", target.as_str()];
    base.extend(pinned_flags(triple));
    base.extend(extra.iter().map(String::as_str));
    let modules = module_inputs(file);
    let scratch = std::env::temp_dir().join(format!(
        "kiln-ir-vs-clang-{}-{}-{triple}",
        std::process::id(),
        file.file_stem().and_then(|s| s.to_str()).unwrap_or("case")
    ));
    let mut mflags: Vec<String> = Vec::new();
    let r = (|| {
        if !modules.is_empty() && std::fs::create_dir_all(&scratch).is_err() {
            return Some(Err(format!("cannot create {}", scratch.display())));
        }
        for (name, src) in &modules {
            let bmi = scratch.join(format!("{name}.pcm")).display().to_string();
            let src = src.display().to_string();
            let mut args = base.clone();
            args.extend(mflags.iter().map(String::as_str));
            args.extend(["--precompile", "-x", "c++-module", src.as_str(), "-o", bmi.as_str()]);
            match pipe(&clang_tool(), &args, "")? {
                Ok(_) => mflags.push(format!("-fmodule-file={name}={bmi}")),
                Err(e) => return Some(Err(format!("precompiling {src}: {e}"))),
            }
        }
        let mut args = vec!["-S", "-emit-llvm", "-o", "-"];
        args.extend(base.iter().copied());
        args.extend(mflags.iter().map(String::as_str));
        args.push(path.as_str());
        pipe(&clang_tool(), &args, "")
    })();
    if !modules.is_empty() {
        let _ = std::fs::remove_dir_all(&scratch);
    }
    r
}

/// How a pair is compared under an oracle (spec/codegen.md §6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// Byte-identical after [`ir_norm::base`] (oracle 19).
    Exact,
    /// [`ir_norm::structural`] equal and [`ir_norm::abi_slice`] equal
    /// (oracle 22).
    Structural,
}

fn mode() -> Mode {
    if oracle_version() == "19" { Mode::Exact } else { Mode::Structural }
}

/// A minimal line diff: the first differing region with context.
fn diff(a: &str, b: &str) -> String {
    let al: Vec<&str> = a.lines().collect();
    let bl: Vec<&str> = b.lines().collect();
    let mut i = 0;
    while i < al.len() && i < bl.len() && al[i] == bl[i] {
        i += 1;
    }
    let lo = i.saturating_sub(3);
    let mut s = String::new();
    for l in &al[lo..al.len().min(i + 8)] {
        let _ = writeln!(s, "kiln : {l}");
    }
    for l in &bl[lo..bl.len().min(i + 8)] {
        let _ = writeln!(s, "clang: {l}");
    }
    s
}

/// What a compared pair matched in.
struct Outcome {
    exact: bool,
    structural: bool,
    /// `None` when the slices match, else their diff.
    abi: Option<String>,
    /// The diff in the oracle's mode, when it does not match.
    diff: Option<String>,
}

fn run_case(name: &str, file: &Path, triple: &str) -> Result<Option<Outcome>, String> {
    let ir = kiln_ir(file, triple)?;
    let Some(checked) = opt(&ir) else { return Ok(None) };
    let (opt_name, clang_name) = (opt_tool(), clang_tool());
    let ours = checked.map_err(|e| format!("{opt_name} rejects kiln IR:\n{e}\n{ir}"))?;
    if min_oracle(file).is_some_and(|n| oracle_version().parse::<u32>().is_ok_and(|o| o < n)) {
        return Ok(None);
    }
    let Some(clang) = clang_ir(file, triple) else { return Ok(None) };
    let clang = clang.map_err(|e| format!("{clang_name} failed: {e}"))?;
    let theirs = opt(&clang)
        .unwrap_or_else(|| panic!("{opt_name} present"))
        .map_err(|e| format!("{opt_name} rejects clang IR: {e}"))?;
    let (a, b) = (ir_norm::base(&ours), ir_norm::base(&theirs));
    let (sa, sb) = (ir_norm::structural(&ours), ir_norm::structural(&theirs));
    let (aa, ab) = (ir_norm::abi_slice(&ours), ir_norm::abi_slice(&theirs));
    if std::env::var("KILN_CASE").is_ok_and(|c| c == name) {
        let (k, o) = if mode() == Mode::Exact { (&a, &b) } else { (&sa, &sb) };
        eprintln!("==== kiln ({triple}) ====\n{k}\n==== clang ({triple}) ====\n{o}");
    }
    let exact = a == b;
    let structural = sa == sb;
    let abi = (aa != ab).then(|| diff(&aa, &ab));
    let diff = match mode() {
        Mode::Exact if !exact => Some(diff(&a, &b)),
        Mode::Structural if !structural => Some(diff(&sa, &sb)),
        _ => None,
    };
    Ok(Some(Outcome { exact, structural, abi, diff }))
}

#[test]
fn ir_matches_clang() {
    let only = std::env::var("KILN_CASE").ok();
    let mut files: Vec<PathBuf> = std::fs::read_dir(corpus_dir())
        .expect("tests/codegen")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "cpp"))
        .collect();
    files.sort();
    assert!(only.is_some() || files.len() >= 40, "the corpus has {} cases", files.len());
    let oracle = oracle_version();
    let (mut exact, mut structural, mut abi_ok, mut total, mut skipped) = (0, 0, 0, 0, 0);
    let mut failures = Vec::new();
    let mut deviations = Vec::new();
    for file in &files {
        let name = file.file_stem().and_then(|s| s.to_str()).expect("file name").to_owned();
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        for &triple in TRIPLES {
            total += 1;
            let allow = DEVIATIONS
                .iter()
                .find(|d| d.0 == name && d.1 == triple && d.2 == oracle)
                .map(|d| d.3);
            let o = match run_case(&name, file, triple) {
                Ok(Some(o)) => o,
                Ok(None) => {
                    skipped += 1;
                    continue;
                }
                Err(e) if allow == Some(Allow::OracleInvalid) && e.contains("rejects clang IR") => {
                    deviations.push(format!("{name} ({triple}): oracle IR invalid"));
                    continue;
                }
                Err(e) => {
                    failures.push(format!("{name} ({triple}): {e}"));
                    continue;
                }
            };
            exact += usize::from(o.exact);
            structural += usize::from(o.structural);
            abi_ok += usize::from(o.abi.is_none());
            match (allow, &o.diff, &o.abi) {
                (None, None, None) => {}
                (None, Some(d), abi) => failures.push(format!(
                    "{name} ({triple}): differs from clang {oracle} ({:?} mode)\n{d}{}",
                    mode(),
                    abi.as_ref().map_or(String::new(), |a| format!("the ABI slice too:\n{a}"))
                )),
                (None | Some(Allow::Structure), None, Some(d)) => failures
                    .push(format!("{name} ({triple}): ABI slice differs from clang {oracle}\n{d}")),
                (Some(Allow::Structure), Some(_), Some(d)) => failures.push(format!(
                    "{name} ({triple}): ABI slice differs from clang {oracle} (only the \
                     structure is a listed deviation)\n{d}"
                )),
                (Some(Allow::OracleInvalid), _, _) => failures.push(format!(
                    "{name} ({triple}): clang {oracle}'s IR now verifies; drop the deviation"
                )),
                (Some(_), None, None) => {
                    deviations.push(format!("{name} ({triple}): now matches; drop the deviation"));
                }
                (Some(a), Some(d), _) => {
                    deviations.push(format!("{name} ({triple}): {a:?} deviation\n{d}"));
                }
                (Some(Allow::Abi), None, Some(d)) => {
                    deviations.push(format!("{name} ({triple}): Abi deviation\n{d}"));
                }
            }
        }
    }
    eprintln!(
        "ir_vs_clang: oracle {oracle}, {:?} mode: {exact}/{total} exact, {structural}/{total} \
         structural, {abi_ok}/{total} ABI slices equal, {skipped} skipped (no {}/{}, or \
         min-oracle)",
        mode(),
        opt_tool(),
        clang_tool(),
    );
    for d in &deviations {
        eprintln!("  {d}");
    }
    assert!(failures.is_empty(), "{} failing case(s):\n{}", failures.len(), failures.join("\n"));
}
