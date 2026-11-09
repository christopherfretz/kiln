//! The driver-level IR-diff corpus (spec/codegen.md §6, "IR-diff corpus").
//!
//! Every `tests/codegen/*.cpp` is compiled in-process by the kiln driver,
//! as `kiln++` (`kiln_driver::run`), with
//! `-S -emit-llvm -O0 -std=c++26 --target=<triple>` for each triple in
//! [`TRIPLES`], and by `clang++-N` with the same flags; kiln also gets
//! `-fclang-compat=N` (its codegen follows clang N, spec/codegen.md §6a). Both outputs go
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
//! Debug-information cases (`debug_*.cpp`, `//@ args: -g`; WP-814) are
//! compared the same way. Under oracle 19 that is byte-exact, metadata
//! included: kiln creates its metadata nodes in clang's order, so the
//! `!N` numbering after `opt-19` is clang's and no renumbering is needed;
//! [`ir_norm::base`] only blanks the `DICompileUnit`'s `producer:` (the
//! version string). Under oracle 22 [`ir_norm::structural`] drops the
//! `#dbg_*` records and `!dbg` attachments with the rest of the metadata.
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
    /// Nothing: kiln's IR is compared in the oracle's mode after
    /// `opt-N -passes=loop-simplify,lcssa` (which skip `optnone`
    /// functions) and [`ir_norm::unname_locals`], the canonicalization
    /// clang 19's `AArch64` `-O0` pipeline applies to its own output
    /// (spec/codegen.md "Deviations").
    LoopCanon,
}

/// `(case, triple, oracle, allow)`: pairs whose IR differs from clang
/// `oracle`'s by a documented deviation (spec/codegen.md "Deviations").
const DEVIATIONS: &[(&str, &str, &str, Allow)] = &[
    // clang 19's AArch64 -O0 pipeline runs `loop(loop-idiom-vectorize)`,
    // whose LoopSimplify/LCSSA canonicalization reshapes the array loops
    // of the (non-`optnone`) global initialization functions; kiln's IR
    // matches after the same two passes.
    ("global_init_array_eh", "aarch64-unknown-linux-gnu", "19", Allow::LoopCanon),
    // The same canonicalization of `__cxx_global_array_dtor`'s
    // partial-array EH cleanup loop (WP-205).
    ("eh_array_destroy_throwing_global", "aarch64-unknown-linux-gnu", "19", Allow::LoopCanon),
    // The same, for the EH cleanup of a static array temporary's
    // initialization (WP-216).
    ("array_temporary_global", "aarch64-unknown-linux-gnu", "19", Allow::LoopCanon),
    // kiln zeroes a padded `_Atomic` operand's padding and writes a failed
    // compare-exchange's old value back to `*expected` (C11 7.17.7.4p2) at
    // every compat level, as clang 22 does; clang 19 did neither.
    // An importer of a module whose global module fragment defines G's key
    // function: kiln declares `_ZTV1G` external (the interface unit emits
    // it with `_ZTI1G`/`_ZTS1G`, Itanium ABI §5.2.3); clang 22 omits them
    // from the interface and defines them, with G's deleting destructor,
    // strongly in every importer, so two importers fail to link
    // (spec/modules.md §10 deviation 7). The extra globals are in the ABI
    // slice.
    ("module_gmf_vtable", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("module_gmf_vtable", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    // clang 22 names members of an unnamed class `(unnamed struct)::N` in
    // IR type names (19: `(anonymous struct)`).
    ("lambda_names", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("lambda_names", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    // clang 22 lowers `modf` to the `llvm.modf` intrinsic (19: a `modff`
    // libcall).
    ("builtins_math", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("builtins_math", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    // `__builtin_thread_pointer()` calls `llvm.thread.pointer`, the name
    // the LLVM 19 backend kiln drives knows (`llvm_major`); clang 22's
    // LLVM overloads it as `llvm.thread.pointer.p0` (WP-782).
    ("builtins_thread_pointer", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("builtins_thread_pointer", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    // LLVM 20+ fast-math flags on `fpext`/`fptrunc` and on calls returning
    // a homogeneous FP struct (`__muldc3`): the LLVM 19 IR kiln emits for
    // its backend (`llvm_major`) cannot carry them (WP-806).
    ("fp_contract_fast", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_contract_fast", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_contract_honor", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_contract_honor", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_fast_math", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_fast_math", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_finite_math", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_finite_math", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_unsafe_math", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_unsafe_math", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_parts", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_parts", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_fast_math_undo", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_fast_math_undo", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_math_errno_O1", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_math_errno_O1", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_model_fast", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("fp_model_fast", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    // clang 22 instruction-selection changes that kiln has not followed:
    // `alloca` of the atomic flag / order temporaries, `i1` (19: `i8`) overflow-result
    // truncation, `select` for vector `?:`, and the block order of
    // `delete`'s null check.
    ("atomic_builtins", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("atomic_builtins", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("atomic_orders", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("atomic_orders", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("builtins_overflow", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("builtins_overflow", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("vector_cond", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("vector_cond", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("new_array_cookie", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("new_array_cookie", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("new_delete_replaced", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("new_delete_replaced", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    // LLVM 22's InstCombine rewrites the `delete[]` destruction loop's
    // element GEP to an `i8` GEP over the byte size; kiln's optimizer is
    // `opt-19` (spec/driver.md "IR optimization"), which keeps the
    // `%struct.N` GEP, so the type definition (ABI slice) differs too.
    ("delete_oz", "aarch64-unknown-linux-gnu", "22", Allow::Abi),
    ("delete_oz", "x86_64-unknown-linux-gnu", "22", Allow::Abi),
    // clang 22 gives a coroutine's `await_suspend` wrappers internal (19:
    // private) linkage, and LLVM 22's coroutine lowering reshapes the
    // resume/destroy functions; kiln's coroutine IR follows clang 19.
    ("coro_aligned_alloc", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("coro_aligned_alloc", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("coro_await_suspend", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("coro_await_suspend", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    ("coro_only_destroy_when_complete", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("coro_only_destroy_when_complete", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
    // `__builtin_coro_noop`'s `__NoopCoro_ResumeDestroy` is made by the
    // `opt-19` CoroEarly pass kiln runs (private; LLVM 22: internal).
    ("coro_builtins", "aarch64-unknown-linux-gnu", "22", Allow::Structure),
    ("coro_builtins", "x86_64-unknown-linux-gnu", "22", Allow::Structure),
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
///
/// `-ftrivial-auto-var-init=uninitialized`: kiln writes the C++26
/// erroneous value of an uninitialized automatic variable by default
/// (P2795R5, spec/codegen.md §7r), which the clang oracles do not
/// implement; `erroneous_init.cpp` compares kiln's fill with clang's
/// `-ftrivial-auto-var-init=zero` through its `//@ args:`.
fn pinned_flags(triple: &str) -> &'static [&'static str] {
    if triple.starts_with("aarch64") {
        &["-moutline-atomics", "-ftrivial-auto-var-init=uninitialized"]
    } else {
        &["-ftrivial-auto-var-init=uninitialized"]
    }
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

/// The LLVM installation kiln drives here: LLVM 19's, named explicitly
/// (kiln's own default is 22 since WP-1016, spec/driver.md "LLVM
/// discovery"), since the oracle-19 byte-exact comparison and the
/// [`DEVIATIONS`] that cite `llvm_major` assume LLVM 19 IR. Without it,
/// discovery picks.
const KILN_LLVM_DIR: &str = "/usr/lib/llvm-19";

/// Compiles `file` for `triple` with the in-process driver, at the
/// oracle's compat level (`kiln++` defaults to 22 since WP-394), driving
/// [`KILN_LLVM_DIR`].
fn kiln_ir(file: &Path, triple: &str) -> Result<String, String> {
    let out = SharedBuffer::new();
    let err = SharedBuffer::new();
    let io = Io {
        stdout: Box::new(out.clone()),
        stderr: SharedWriter::new(err.clone()),
        stdin: Box::new(std::io::empty()),
    };
    let compat = format!("-fclang-compat={}", oracle_version());
    let args: Vec<String> = [
        "-S",
        "-emit-llvm",
        "-O0",
        "-std=c++26",
        &format!("--target={triple}"),
        &compat,
        "-o",
        "-",
    ]
    .iter()
    .chain(pinned_flags(triple))
    .map(|s| (*s).to_owned())
    .chain(extra_flags(file))
    .chain(
        module_inputs(file).into_iter().map(|(n, p)| format!("-fmodule-file={n}={}", p.display())),
    )
    .chain(
        Path::new(KILN_LLVM_DIR).is_dir().then(|| format!("--llvm-dir={KILN_LLVM_DIR}")),
    )
    .chain(std::iter::once(file.display().to_string()))
    .collect();
    let code = run(&args, io);
    if code != 0 {
        return Err(format!("kiln exited {code}:\n{}", err.contents()));
    }
    // kiln writes its backend's lifetime-marker (WP-783) and unlowered
    // `llvm.coro.end` (WP-941) forms; compare in the oracle's.
    if oracle_version() == "22" {
        return Ok(ir_norm::coro_end_llvm22(&ir_norm::lifetime_markers_llvm22(&out.contents())));
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

/// [`Allow::LoopCanon`]: verified IR `ir` after LLVM's loop
/// canonicalization, renumbered and reprinted.
fn loop_canon(ir: &str) -> Result<String, String> {
    let canon = pipe(&opt_tool(), &["-S", "-passes=loop-simplify,lcssa", "-o", "-"], ir)
        .unwrap_or_else(|| panic!("{} present", opt_tool()))?;
    opt(&ir_norm::unname_locals(&canon)).unwrap_or_else(|| panic!("{} present", opt_tool()))
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

fn run_case(name: &str, file: &Path, triple: &str, canon: bool) -> Result<Option<Outcome>, String> {
    let ir = kiln_ir(file, triple)?;
    let Some(checked) = opt(&ir) else { return Ok(None) };
    let (opt_name, clang_name) = (opt_tool(), clang_tool());
    let mut ours = checked.map_err(|e| format!("{opt_name} rejects kiln IR:\n{e}\n{ir}"))?;
    if canon {
        ours = loop_canon(&ours).map_err(|e| format!("loop canonicalization failed: {e}"))?;
    }
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

/// `f` over `items` on up to `RUST_TEST_THREADS` (else the available
/// parallelism) worker threads, the results in `items` order.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::env::var("RUST_TEST_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .or_else(|| std::thread::available_parallelism().ok().map(usize::from))
        .unwrap_or(1)
        .clamp(1, items.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut out: Vec<Option<R>> = std::iter::repeat_with(|| None).take(items.len()).collect();
    std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break done };
                        done.push((i, f(item)));
                    }
                })
            })
            .collect();
        for w in workers {
            for (i, r) in w.join().expect("an IR-diff worker panicked") {
                out[i] = Some(r);
            }
        }
    });
    out.into_iter().map(|r| r.expect("every item ran")).collect()
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
    // Every (case, triple) pair, in corpus order; the pairs are independent
    // (kiln runs in-process, clang and opt as child processes), so they run
    // on worker threads and are then reported in this order.
    let mut jobs = Vec::new();
    for file in &files {
        let name = file.file_stem().and_then(|s| s.to_str()).expect("file name").to_owned();
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        for &triple in TRIPLES {
            let allow = DEVIATIONS
                .iter()
                .find(|d| d.0 == name && d.1 == triple && d.2 == oracle)
                .map(|d| d.3);
            jobs.push((name.clone(), file, triple, allow));
        }
    }
    let outcomes = par_map(&jobs, |(name, file, triple, allow)| {
        run_case(name, file, triple, *allow == Some(Allow::LoopCanon))
    });
    for ((name, _, triple, allow), outcome) in jobs.iter().zip(outcomes) {
        let (name, triple) = (name.as_str(), *triple);
        total += 1;
        let canon = *allow == Some(Allow::LoopCanon);
        let allow = allow.filter(|&a| a != Allow::LoopCanon);
        let o = match outcome {
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
        if canon && o.diff.is_none() {
            deviations.push(format!("{name} ({triple}): matches after loop canonicalization"));
        }
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
            (None | Some(Allow::Structure | Allow::LoopCanon), None, Some(d)) => failures
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
