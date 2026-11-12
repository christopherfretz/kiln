//! WP-917: module files (spec/modules.md §11) end to end through `kiln++`:
//! `--precompile`, `-fmodule-output`, the three ways an importer finds a
//! module file, determinism, and the mismatch and corrupt-file
//! diagnostics (a damaged module file is always a diagnostic, never a
//! crash).
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-bmi-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Out {
    code: i32,
    stderr: String,
}

fn kiln(dir: &Path, args: &[&str]) -> Out {
    let o: Output = Command::new(kilnpp())
        .arg("-std=c++20")
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    Out { code: o.status.code().unwrap_or(-1), stderr }
}

fn ok(dir: &Path, args: &[&str]) {
    let o = kiln(dir, args);
    assert_eq!(o.code, 0, "kiln++ {args:?}:\n{}", o.stderr);
}

const M: &str = "export module m;\nexport int f() { return 41; }\n\
                 export inline int g() { return 1; }\n";
const USE: &str = "import m;\nint main() { return f() + g() == 42 ? 0 : 1; }\n";

/// A directory holding `m.cppm`, `u.cpp` and `m.pcm` (`--precompile`).
fn setup(name: &str) -> PathBuf {
    let d = scratch(name);
    std::fs::write(d.join("m.cppm"), M).unwrap();
    std::fs::write(d.join("u.cpp"), USE).unwrap();
    ok(&d, &["--precompile", "m.cppm", "-o", "m.pcm"]);
    d
}

#[test]
fn precompile_is_deterministic_and_importable() {
    let d = setup("precompile");
    let a = std::fs::read(d.join("m.pcm")).unwrap();
    assert!(a.starts_with(b"KILNBMI\0"), "magic");
    ok(&d, &["--precompile", "m.cppm", "-o", "m2.pcm"]);
    assert_eq!(a, std::fs::read(d.join("m2.pcm")).unwrap(), "same input, same bytes");
    // Without -o: `<stem>.pcm` in the working directory. (The capsule
    // records the source's path as spelled, so it differs from m.pcm.)
    std::fs::create_dir_all(d.join("sub")).unwrap();
    ok(&d.join("sub"), &["--precompile", "../m.cppm"]);
    assert!(d.join("sub/m.pcm").is_file());
    ok(&d, &["-fsyntax-only", "-fmodule-file=m=m.pcm", "u.cpp"]);
    ok(&d, &["-fsyntax-only", "-fprebuilt-module-path=sub", "u.cpp"]);
    // Part A: a module source given for the module still works.
    ok(&d, &["-fsyntax-only", "-fmodule-file=m=m.cppm", "u.cpp"]);
    // The nameless form reads the name from the file, with clang's warning.
    let o = kiln(&d, &["-fsyntax-only", "-fmodule-file=m.pcm", "u.cpp"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(
        o.stderr.lines().next().unwrap_or(""),
        "warning: the form '-fmodule-file=<BMI-path>' is deprecated for standard C++ named \
         modules;consider to use '-fmodule-file=<module-name>=<BMI-path>' instead \
         [-Weager-load-cxx-named-modules]"
    );
}

#[test]
fn module_output_object_and_link() {
    let d = setup("output");
    std::fs::create_dir_all(d.join("x")).unwrap();
    // `-fmodule-output` with -c -o: the object's path with `.pcm`.
    ok(&d, &["-c", "m.cppm", "-fmodule-output", "-o", "x/m.o"]);
    assert_eq!(
        std::fs::read(d.join("m.pcm")).unwrap(),
        std::fs::read(d.join("x/m.pcm")).unwrap(),
        "-fmodule-output writes what --precompile writes"
    );
    ok(&d, &["-c", "m.cppm", "-fmodule-output=x/other.pcm", "-o", "x/m2.o"]);
    assert!(d.join("x/other.pcm").is_file());
    // A module file is a compile input: its object is the interface's.
    ok(&d, &["-c", "m.pcm", "-o", "x/from-pcm.o"]);
    ok(&d, &["-c", "-fmodule-file=m=x/m.pcm", "u.cpp", "-o", "x/u.o"]);
    ok(&d, &["x/from-pcm.o", "x/u.o", "-o", "x/a.out"]);
    let st = Command::new(d.join("x/a.out")).status().unwrap();
    assert!(st.success(), "two-step program: {st}");
}

#[test]
fn non_module_inputs() {
    let d = scratch("nonmodule");
    std::fs::write(d.join("p.cpp"), "int f() { return 0; }\n").unwrap();
    let o = kiln(&d, &["-c", "p.cpp", "-fmodule-output", "-o", "p.o"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(
        o.stderr.contains("warning: argument unused during compilation: '-fmodule-output'"),
        "{}",
        o.stderr
    );
    assert!(!d.join("p.pcm").exists());
    let o = kiln(&d, &["--precompile", "p.cpp", "-o", "p.pcm"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(
        o.stderr.contains("error: --precompile requires a module interface or partition unit"),
        "{}",
        o.stderr
    );
    // A `.pcm` that is not a module file.
    std::fs::write(d.join("fake.pcm"), "int x;\n").unwrap();
    let o = kiln(&d, &["-c", "fake.pcm", "-o", "fake.o"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stderr.contains("fatal error: 'fake.pcm' is not a kiln module file"), "{}", o.stderr);
}

#[test]
fn mismatch_diagnostics() {
    let d = setup("mismatch");
    let o = kiln(&d, &["-std=c++23", "-fsyntax-only", "-fmodule-file=m=m.pcm", "u.cpp"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(
        o.stderr.contains(
            "error: module file 'm.pcm' was built with '-std=c++20' but the current \
             compilation uses '-std=c++23'"
        ),
        "{}",
        o.stderr
    );
    assert!(
        o.stderr.contains(
            "fatal error: module file 'm.pcm' cannot be loaded due to a configuration \
             mismatch with the current compilation"
        ),
        "{}",
        o.stderr
    );
    let o = kiln(&d, &["-fno-exceptions", "-fsyntax-only", "-fmodule-file=m=m.pcm", "u.cpp"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stderr.contains("'-fexceptions' but the current compilation uses '-fno-exceptions'"));
    let o = kiln(&d, &["-fsyntax-only", "-fmodule-file=m=missing.pcm", "u.cpp"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stderr.contains("fatal error: module file 'missing.pcm' not found"), "{}", o.stderr);
    std::fs::write(d.join("n.cpp"), "import n;\n").unwrap();
    let o = kiln(&d, &["-fsyntax-only", "-fmodule-file=n=m.pcm", "n.cpp"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(
        o.stderr.contains("fatal error: module file 'm.pcm' holds module 'm', not 'n'"),
        "{}",
        o.stderr
    );
    // A clang module file is recognised and refused.
    let clang = Command::new("clang++-19")
        .args(["-std=c++20", "--precompile", "m.cppm", "-o", "clang.pcm"])
        .current_dir(&d)
        .stderr(Stdio::null())
        .status();
    if clang.is_ok_and(|s| s.success()) {
        let o = kiln(&d, &["-fsyntax-only", "-fmodule-file=m=clang.pcm", "u.cpp"]);
        assert_eq!(o.code, 1, "{}", o.stderr);
        assert!(o.stderr.contains("'clang.pcm' was built by clang"), "{}", o.stderr);
    }
}

#[test]
fn damaged_module_files_are_diagnosed() {
    let d = setup("damaged");
    let good = std::fs::read(d.join("m.pcm")).unwrap();
    let n = good.len();
    let mut cases: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..16 {
        let cut = [0, 1, 7, 8, 12, 13, n / 2, n - 1].get(i).copied().unwrap_or(i * n / 16);
        cases.push((format!("truncated to {cut}"), good[..cut].to_vec()));
    }
    for i in 0..24 {
        let at = if i < 8 { i * 2 } else { i * n / 24 };
        let mut b = good.clone();
        b[at] ^= 0x5a;
        cases.push((format!("byte {at} flipped"), b));
    }
    for (what, bytes) in cases {
        std::fs::write(d.join("bad.pcm"), &bytes).unwrap();
        let o = kiln(&d, &["-fsyntax-only", "-fmodule-file=m=bad.pcm", "u.cpp"]);
        assert_eq!(o.code, 1, "{what}: {}", o.stderr);
        assert!(o.stderr.contains("fatal error: "), "{what}: {}", o.stderr);
    }
}

// --- WP-985: kind 2 (spec/modules.md §11.11) ---------------------------

/// A module whose constants take real work to evaluate.
const CM: &str = "export module cm;\n\
                  constexpr int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }\n\
                  export constexpr int F = fib(21);\n\
                  static_assert(fib(19) == 4181);\n\
                  export struct P { int a; double b; };\n\
                  export constexpr P p{3, 2.5};\n\
                  export constexpr bool B = F > 10;\n\
                  export enum class E { a = 3, b };\n\
                  export constexpr E e = E::b;\n\
                  export constexpr const int *np = nullptr;\n";
const CUSE: &str = "import cm;\n\
                    static_assert(F == 10946 && p.a == 3 && p.b == 2.5 && B);\n\
                    static_assert(e == E::b && np == nullptr);\n\
                    int main() { return F == 10946 ? 0 : 1; }\n";

/// The `"loc"` and `"steps"` of each evaluation in a `-fconstexpr-report`.
fn report(path: &Path) -> Vec<(String, u64)> {
    let text = std::fs::read_to_string(path).unwrap();
    let field = |l: &str, k: &str| -> Option<String> {
        let at = l.find(&format!("\"{k}\":"))? + k.len() + 3;
        let rest = &l[at..];
        let end = rest.find([',', '}'])?;
        Some(rest[..end].trim_matches('"').to_owned())
    };
    text.lines()
        .filter_map(|l| Some((field(l, "loc")?, field(l, "steps")?.parse().ok()?)))
        .collect()
}

fn setup_cm(name: &str) -> PathBuf {
    let d = scratch(name);
    std::fs::write(d.join("cm.cppm"), CM).unwrap();
    std::fs::write(d.join("u.cpp"), CUSE).unwrap();
    d
}

/// Kind 2 carries the module's constant results: an importer of the
/// kind-2 file runs none of the module's evaluations (no report line
/// for `cm.cppm`), one of the kind-1 file runs them all again. Both
/// files are deterministic and `-v` says which kind was written.
#[test]
fn kind2_carries_constant_results() {
    let d = setup_cm("kind2");
    let o = kiln(&d, &["-v", "--precompile", "cm.cppm", "-o", "cm2.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(
        o.stderr.contains("note: wrote module file 'cm2.pcm' as kind 2, carrying "),
        "{}",
        o.stderr
    );
    assert!(!o.stderr.contains("warning"), "{}", o.stderr);
    let o = kiln(&d, &["-v", "-fkiln-bmi-kind=1", "--precompile", "cm.cppm", "-o", "cm1.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("as kind 1, as -fkiln-bmi-kind=1 requests"), "{}", o.stderr);
    let (one, two) =
        (std::fs::read(d.join("cm1.pcm")).unwrap(), std::fs::read(d.join("cm2.pcm")).unwrap());
    assert_eq!((one[12], two[12]), (1, 2), "kind bytes");
    ok(&d, &["--precompile", "cm.cppm", "-o", "cm2b.pcm"]);
    assert_eq!(two, std::fs::read(d.join("cm2b.pcm")).unwrap(), "kind 2 is deterministic");

    for k in ["1", "2"] {
        let r = format!("r{k}.json");
        let m = format!("-fmodule-file=cm=cm{k}.pcm");
        ok(&d, &["-fsyntax-only", &m, &format!("-fconstexpr-report={r}"), "u.cpp"]);
        let lines = report(&d.join(&r));
        let module: Vec<_> = lines.iter().filter(|(l, _)| l.starts_with("cm.cppm:")).collect();
        let steps: u64 = module.iter().map(|(_, s)| s).sum();
        if k == "1" {
            assert!(module.len() >= 6 && steps > 10_000, "kind 1 re-evaluates: {lines:?}");
        } else {
            assert!(module.is_empty(), "kind 2 re-evaluated: {lines:?}");
        }
    }
    // The program built against each kind runs.
    for k in ["1", "2"] {
        let m = format!("-fmodule-file=cm=cm{k}.pcm");
        let exe = format!("u{k}");
        ok(&d, &[&m, "-c", "cm.cppm", "-o", "cm.o"]);
        ok(&d, &[&m, "u.cpp", "cm.o", "-o", &exe]);
        let st = Command::new(d.join(&exe)).status().unwrap();
        assert!(st.success(), "kind {k} program");
    }
    // A `.pcm` main input of kind 2 compiles too.
    ok(&d, &["-c", "cm2.pcm", "-o", "cm2.o"]);
}

/// A module with nothing to carry is written as kind 1, with the reason.
#[test]
fn kind2_falls_back_to_kind1() {
    let d = scratch("kind2-fallback");
    std::fs::write(d.join("m.cppm"), M).unwrap();
    let o = kiln(&d, &["-v", "--precompile", "m.cppm", "-o", "m.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("as kind 1: no constant result to carry"), "{}", o.stderr);
    assert_eq!(std::fs::read(d.join("m.pcm")).unwrap()[12], 1);
    let o = kiln(&d, &["-fkiln-bmi-kind=3", "--precompile", "m.cppm", "-o", "m.pcm"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
}

/// A damaged kind-2 file is a diagnostic, never a crash: truncations and
/// flips (caught by the checksum), and a damaged carried-constants
/// section behind a valid checksum (caught by its own reader).
#[test]
fn kind2_damaged_files_are_diagnosed() {
    use kiln_pp::bmi;
    let d = setup_cm("kind2-damaged");
    ok(&d, &["--precompile", "cm.cppm", "-o", "cm.pcm"]);
    let good = std::fs::read(d.join("cm.pcm")).unwrap();
    let n = good.len();
    let mut cases: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..8 {
        let cut = n - 1 - i * 7;
        cases.push((format!("truncated to {cut}"), good[..cut].to_vec()));
        let mut b = good.clone();
        b[cut] ^= 0x5a;
        cases.push((format!("byte {cut} flipped"), b));
    }
    let capsule = bmi::read(&good).unwrap();
    let sec = capsule.section(bmi::SECTION_CONSTS).unwrap().to_vec();
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    for i in 0..24 {
        let mut c = capsule.clone();
        let s = c.sections.iter_mut().find(|s| s.tag == bmi::SECTION_CONSTS).unwrap();
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        match i % 3 {
            0 => s.bytes.truncate((seed % sec.len() as u64) as usize),
            1 => s.bytes[(seed % sec.len() as u64) as usize] ^= 0xff,
            _ => s.bytes.push(0),
        }
        cases.push((format!("section damage {i}"), bmi::write(&c)));
    }
    for (what, bytes) in cases {
        std::fs::write(d.join("bad.pcm"), &bytes).unwrap();
        let o = kiln(&d, &["-fsyntax-only", "-fmodule-file=cm=bad.pcm", "u.cpp"]);
        // A flipped value byte may still decode to a well-formed entry
        // (then checked against its type, or simply a different constant
        // the static_asserts reject); never a panic.
        assert!(o.code == 0 || o.code == 1, "{what}: {}", o.stderr);
        if what.starts_with("truncated") || what.starts_with("byte") {
            assert!(o.stderr.contains("fatal error: "), "{what}: {}", o.stderr);
        }
    }
}

// --- WP-1046: results inside instantiations (§11.11 Part 2) -----------

const CT: &str = include_str!("../../../tests/conformance/module.interface/Inputs/ct.cppm");
const CT_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/carried-instantiations-run.cpp");
const CT_ERR: &str =
    include_str!("../../../tests/conformance/module.interface/carried-instantiations-errors.cpp");

/// Kind 2 carries the results of evaluations inside the module's own
/// template instantiations (static data members, `static_assert`s, local
/// constexpr variables, `if constexpr`, array bounds, non-type
/// arguments, and a body instantiated at the unit's end): a kind-2
/// importer re-runs none of the expensive ones, and its diagnostics and
/// IR are those of kind 1.
#[test]
fn kind2_carries_instantiation_results() {
    let d = scratch("kind2-inst");
    std::fs::write(d.join("ct.cppm"), CT).unwrap();
    std::fs::write(d.join("run.cpp"), CT_RUN).unwrap();
    std::fs::write(d.join("err.cpp"), CT_ERR).unwrap();
    let o = kiln(&d, &["-v", "--precompile", "ct.cppm", "-o", "ct2.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains(" inside template instantiations"), "{}", o.stderr);
    ok(&d, &["-fkiln-bmi-kind=1", "--precompile", "ct.cppm", "-o", "ct1.pcm"]);
    ok(&d, &["--precompile", "ct.cppm", "-o", "ct2b.pcm"]);
    let two = std::fs::read(d.join("ct2.pcm")).unwrap();
    assert_eq!(two, std::fs::read(d.join("ct2b.pcm")).unwrap(), "kind 2 is deterministic");

    let mut seen = Vec::new();
    for k in ["1", "2"] {
        // The same file name for both kinds, so the outputs compare.
        std::fs::copy(d.join(format!("ct{k}.pcm")), d.join("ct.pcm")).unwrap();
        let r = format!("r{k}.json");
        let rep = format!("-fconstexpr-report={r}");
        ok(&d, &["-fsyntax-only", "-fmodule-file=ct=ct.pcm", &rep, "run.cpp"]);
        let lines = report(&d.join(&r));
        let module: Vec<_> = lines.iter().filter(|(l, _)| l.starts_with("ct.cppm:")).collect();
        let steps: u64 = module.iter().map(|(_, s)| s).sum();
        if k == "1" {
            assert!(module.len() >= 15 && steps > 20_000, "kind 1 re-evaluates: {lines:?}");
        } else {
            // Left: the importer's own instantiations (`Fib<5>`, `big`
            // of `Fib<19>`/`Fib<20>`, `Fib<10>`), a few steps each.
            assert!(steps < 1_000, "kind 2 re-evaluated: {lines:?}");
            // Neither `twice`'s local constexpr variable nor that of
            // `runtime_twice`, which the importer instantiates at its own
            // end of TU, after the replay.
            assert!(
                !module
                    .iter()
                    .any(|(l, _)| l.starts_with("ct.cppm:13:") || l.starts_with("ct.cppm:35:")),
                "a local constexpr variable re-evaluated: {lines:?}"
            );
        }
        let e = kiln(&d, &["-fsyntax-only", "-fmodule-file=ct=ct.pcm", "err.cpp"]);
        assert_eq!(e.code, 1, "{}", e.stderr);
        ok(&d, &["-S", "-emit-llvm", "-fmodule-file=ct=ct.pcm", "run.cpp", "-o", "run.ll"]);
        let ir = std::fs::read_to_string(d.join("run.ll")).unwrap();
        seen.push((e.stderr, ir));
    }
    assert!(seen[0].0.contains("'6765 == 6764'"), "{}", seen[0].0);
    assert_eq!(seen[0].0, seen[1].0, "diagnostics differ between kinds");
    assert!(seen[0].1 == seen[1].1, "IR differs between kinds");

    // A damaged section with instantiation keys behind a valid checksum
    // is a diagnostic or a fallback, never a crash.
    let capsule = kiln_pp::bmi::read(&two).unwrap();
    let len = capsule.section(kiln_pp::bmi::SECTION_CONSTS).unwrap().len() as u64;
    let mut rng = 0x9e37_79b9_7f4a_7c15u64;
    for i in 0..24 {
        let mut c = capsule.clone();
        let s = c.sections.iter_mut().find(|s| s.tag == kiln_pp::bmi::SECTION_CONSTS).unwrap();
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        match i % 2 {
            0 => s.bytes.truncate((rng % len) as usize),
            _ => s.bytes[(rng % len) as usize] ^= 0xff,
        }
        std::fs::write(d.join("bad.pcm"), kiln_pp::bmi::write(&c)).unwrap();
        let o = kiln(&d, &["-fsyntax-only", "-fmodule-file=ct=bad.pcm", "run.cpp"]);
        assert!(o.code == 0 || o.code == 1, "damage {i}: {}", o.stderr);
    }
}

const CV: &str = include_str!("../../../tests/conformance/module.interface/Inputs/cv.cppm");
const CV_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/carried-pointers-run.cpp");
const CR: &str = include_str!("../../../tests/conformance/module.interface/Inputs/cr.cppm");
const CR_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/carried-reflections-run.cpp");
const CL: &str = include_str!("../../../tests/conformance/module.interface/Inputs/cl.cppm");
const CL_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/carried-literals-run.cpp");
const CX: &str = include_str!("../../../tests/conformance/module.interface/Inputs/cx.cppm");
const CX_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/carried-reflections-more-run.cpp");

/// WP-1047: kind 2 carries pointer, reference, member-pointer and
/// reflection values and the results inside specializations keyed on
/// them (`X<&g>`, `HW<h>`, `Wrap<^^S>`): a kind-2 importer re-evaluates
/// none of the module's initializers, and its IR (the specializations'
/// mangled names and addresses) is that of kind 1. WP-1160: so do
/// pointers to string literals, temporaries and labels (`cl`) and
/// reflections of alias template specializations, data member
/// specifications and annotations (`cx`); `-v` names the first value not
/// carried also when others are, and a damaged section of them is a
/// fallback or a diagnostic, never a crash.
#[test]
fn kind2_carries_pointer_and_reflection_values() {
    let d = scratch("kind2-values");
    let support = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/support");
    for (m, src, run, extra) in [
        ("cv", CV, CV_RUN, false),
        ("cr", CR, CR_RUN, true),
        ("cl", CL, CL_RUN, false),
        ("cx", CX, CX_RUN, true),
    ] {
        std::fs::write(d.join(format!("{m}.cppm")), src).unwrap();
        std::fs::write(d.join(format!("{m}-run.cpp")), run).unwrap();
        let mut base = vec!["-std=c++26"];
        if extra {
            base.extend(["-freflection", "-I", support]);
        }
        let args = |more: &[&str]| -> Vec<String> {
            base.iter().chain(more).map(|s| (*s).to_owned()).collect()
        };
        let kiln =
            |d: &Path, a: Vec<String>| kiln(d, &a.iter().map(String::as_str).collect::<Vec<_>>());
        let ok = |d: &Path, a: Vec<String>| {
            let o = kiln(d, a);
            assert_eq!(o.code, 0, "{}", o.stderr);
        };
        let cppm = format!("{m}.cppm");
        let o = kiln(&d, args(&["-v", "--precompile", &cppm, "-o", &format!("{m}2.pcm")]));
        assert_eq!(o.code, 0, "{}", o.stderr);
        if m != "cl" {
            assert!(o.stderr.contains(" inside template instantiations"), "{}", o.stderr);
        }
        // `cv` and `cl` carry every result; the trial evaluations of
        // `bases_of(...)` and `annotations_of(...)` in `cr` and `cx` fail,
        // and `-v` says so beside the count.
        let why = match m {
            "cv" | "cl" => None,
            "cx" => Some(
                "constant results, 2 inside template instantiations (first not carried: an evaluation that failed)",
            ),
            _ => Some(" (first not carried: "),
        };
        match why {
            Some(w) => assert!(o.stderr.contains(w), "{}", o.stderr),
            None => assert!(!o.stderr.contains("first not carried"), "{}", o.stderr),
        }
        ok(&d, args(&["-fkiln-bmi-kind=1", "--precompile", &cppm, "-o", &format!("{m}1.pcm")]));
        // A trial evaluation of `bases_of(...)` / `parameters_of(...)` on
        // its own fails (its vector is not a constant) and is not carried.
        let failed: Vec<String> = src
            .lines()
            .enumerate()
            .filter(|(_, l)| l.contains("_of(^^"))
            .map(|(i, _)| format!("{m}.cppm:{}:", i + 1))
            .collect();
        let mut irs = Vec::new();
        for k in ["1", "2"] {
            std::fs::copy(d.join(format!("{m}{k}.pcm")), d.join(format!("{m}.pcm"))).unwrap();
            let file = format!("-fmodule-file={m}={m}.pcm");
            let rep = format!("-fconstexpr-report={m}{k}.json");
            let run = format!("{m}-run.cpp");
            ok(&d, args(&["-fsyntax-only", &file, &rep, &run]));
            let lines = report(&d.join(format!("{m}{k}.json")));
            let module: Vec<_> = lines
                .iter()
                .filter(|(l, _)| l.starts_with(&format!("{m}.cppm:")))
                .filter(|(l, _)| !failed.iter().any(|f| l.starts_with(f.as_str())))
                .collect();
            if k == "1" {
                assert!(module.len() >= 13, "kind 1 re-evaluates: {lines:?}");
            } else {
                assert!(module.is_empty(), "kind 2 re-evaluated: {module:?}");
            }
            let ll = format!("{m}{k}.ll");
            ok(&d, args(&["-S", "-emit-llvm", &file, &run, "-o", &ll]));
            irs.push(std::fs::read_to_string(d.join(&ll)).unwrap());
        }
        assert!(irs[0] == irs[1], "{m}: IR differs between kinds");
        if m == "cl" || m == "cx" {
            damage_consts(
                &d,
                &format!("{m}2.pcm"),
                &args(&[
                    "-fsyntax-only",
                    &format!("-fmodule-file={m}=bad.pcm"),
                    &format!("{m}-run.cpp"),
                ]),
            );
        }
    }
}

/// Damages the carried-constants section of `pcm` (behind a valid
/// checksum) and compiles `args` against each damaged copy as `bad.pcm`:
/// a diagnostic or a fallback, never a crash.
fn damage_consts(d: &Path, pcm: &str, args: &[String]) {
    let capsule = kiln_pp::bmi::read(&std::fs::read(d.join(pcm)).unwrap()).unwrap();
    let len = capsule.section(kiln_pp::bmi::SECTION_CONSTS).unwrap().len() as u64;
    let mut rng = 0x2545_f491_4f6c_dd1du64;
    for i in 0..16 {
        let mut c = capsule.clone();
        let s = c.sections.iter_mut().find(|s| s.tag == kiln_pp::bmi::SECTION_CONSTS).unwrap();
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        match i % 2 {
            0 => s.bytes.truncate((rng % len) as usize),
            _ => s.bytes[(rng % len) as usize] ^= 0xff,
        }
        std::fs::write(d.join("bad.pcm"), kiln_pp::bmi::write(&c)).unwrap();
        let o = kiln(d, &args.iter().map(String::as_str).collect::<Vec<_>>());
        assert!(o.code == 0 || o.code == 1, "{pcm} damage {i}: {}", o.stderr);
    }
}

/// An interface the declaration image carries entirely (spec/modules.md
/// §11.11 "Part 2a").
const IM: &str = "export module im;\n\
                  namespace ns {\n\
                  export int add(int a, int b);\n\
                  int add(int a, int b) { return a + b; }\n\
                  export enum class E : short { a, b = 7 };\n\
                  export using P = const int *;\n\
                  }\n\
                  export long twice(long x) { return 2 * x; }\n";
const IUSE: &str = "import im;\n\
                    static_assert(static_cast<int>(ns::E::b) == 7);\n\
                    int main() { ns::P p = nullptr; return ns::add(1, 2) + twice(2) == 7 && !p ? 0 : 1; }\n";

fn setup_im(name: &str) -> PathBuf {
    let d = scratch(name);
    std::fs::write(d.join("im.cppm"), IM).unwrap();
    std::fs::write(d.join("u.cpp"), IUSE).unwrap();
    d
}

/// `-fkiln-bmi-image` writes the declaration image (deterministically,
/// with a `-v` note), an importer with the flag builds the unit from it
/// without parsing the unit's source (a capsule whose source is blanked
/// still imports), and one without the flag ignores it.
#[test]
fn kind2_declaration_image() {
    use kiln_pp::bmi;
    let d = setup_im("image");
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "im.cppm", "-o", "im.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of 12 declarations"), "{}", o.stderr);
    ok(&d, &["-fkiln-bmi-image", "--precompile", "im.cppm", "-o", "im2.pcm"]);
    let good = std::fs::read(d.join("im.pcm")).unwrap();
    assert_eq!(good, std::fs::read(d.join("im2.pcm")).unwrap(), "the image is deterministic");
    assert!(bmi::read(&good).unwrap().section(bmi::SECTION_IMAGE).is_some());
    ok(&d, &["--precompile", "im.cppm", "-o", "plain.pcm"]);
    let plain = bmi::read(&std::fs::read(d.join("plain.pcm")).unwrap()).unwrap();
    assert!(plain.section(bmi::SECTION_IMAGE).is_none(), "the image is off by default");

    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        ok(&d, &[flag, "-fsyntax-only", "-fmodule-file=im=im.pcm", "u.cpp"]);
        ok(&d, &["-c", "im.pcm", "-o", "im.o"]);
        ok(&d, &[flag, "-fmodule-file=im=im.pcm", "u.cpp", "im.o", "-o", "u"]);
        assert!(Command::new(d.join("u")).status().unwrap().success(), "{flag} program");
    }

    // Blank the unit's source in the capsule (same length): only an
    // importer that does not replay it still compiles.
    let mut c = bmi::read(&good).unwrap();
    let text = &mut c.files[0].text;
    *text = text.bytes().map(|b| if b == b'\n' { '\n' } else { ' ' }).collect();
    std::fs::write(d.join("blank.pcm"), bmi::write(&c)).unwrap();
    ok(&d, &["-fkiln-bmi-image", "-fsyntax-only", "-fmodule-file=im=blank.pcm", "u.cpp"]);
    let o = kiln(&d, &["-fsyntax-only", "-fmodule-file=im=blank.pcm", "u.cpp"]);
    assert_eq!(o.code, 1, "replaying a blank source fails: {}", o.stderr);
}

/// Stage S2 (§11.11 "Part 2a", WP-1064): inline, constexpr and consteval
/// bodies and variable initializers come from the image alone. With the
/// unit's source blanked in the capsule, an importer still evaluates
/// and calls them.
#[test]
fn kind2_declaration_image_bodies() {
    use kiln_pp::bmi;
    let d = scratch("image-bodies");
    let src = "export module b;\n\
               export constexpr int fact(int n) { int r = 1; for (int i = 2; i <= n; ++i) r *= i; return r; }\n\
               export consteval int sq(int x) { return x * x; }\n\
               export inline int pick(int x, int d = 4) { switch (x) { case 0: return d; default: return x; } }\n\
               export constexpr int kFact = fact(4);\n\
               export const int kBound = 3;\n\
               export inline int counter = sq(2);\n";
    std::fs::write(d.join("b.cppm"), src).unwrap();
    std::fs::write(
        d.join("u.cpp"),
        "int r = 0, i = 0; // names of `fact`'s locals: not gated\n\
         import b;\n\
         static_assert(fact(5) == 120 && sq(3) == 9 && kFact == 24 && kBound == 3);\n\
         int arr[kBound];\n\
         int main() { return pick(0) + pick(2) + counter + fact(3) - (int)sizeof(arr) / (int)sizeof(int) == 13 ? 0 : 1; }\n",
    )
    .unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "b.cppm", "-o", "b.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    let good = std::fs::read(d.join("b.pcm")).unwrap();
    ok(&d, &["-c", "b.pcm", "-o", "b.o"]);
    let mut c = bmi::read(&good).unwrap();
    let text = &mut c.files[0].text;
    *text = text.bytes().map(|b| if b == b'\n' { '\n' } else { ' ' }).collect();
    std::fs::write(d.join("blank.pcm"), bmi::write(&c)).unwrap();
    ok(&d, &["-fkiln-bmi-image", "-fmodule-file=b=blank.pcm", "u.cpp", "b.o", "-o", "u"]);
    assert!(Command::new(d.join("u")).status().unwrap().success(), "program from the image");
}

/// `pcm` with its unit's source blanked (same length): only an importer
/// that builds the unit from its declaration image still compiles.
fn blank_source(d: &Path, pcm: &str, out: &str) {
    use kiln_pp::bmi;
    let mut c = bmi::read(&std::fs::read(d.join(pcm)).unwrap()).unwrap();
    let text = &mut c.files[0].text;
    *text = text.bytes().map(|b| if b == b'\n' { '\n' } else { ' ' }).collect();
    std::fs::write(d.join(out), bmi::write(&c)).unwrap();
}

/// [`blank_source`] for a unit whose image has a preamble (S6): keeps
/// the preamble, which the importer replays, and blanks the rest.
fn blank_after_preamble(d: &Path, pcm: &str, out: &str) {
    use kiln_pp::bmi;
    let mut c = bmi::read(&std::fs::read(d.join(pcm)).unwrap()).unwrap();
    let img = c.sections.iter().find(|s| s.tag == bmi::SECTION_IMAGE).expect("an image");
    let p = bmi::image_header(&img.bytes).unwrap().preamble.expect("a preamble") as usize;
    let text = &mut c.files[0].text;
    let rest: String = text[p..].bytes().map(|b| if b == b'\n' { '\n' } else { ' ' }).collect();
    text.truncate(p);
    text.push_str(&rest);
    std::fs::write(d.join(out), bmi::write(&c)).unwrap();
}

/// The import notes of an image-enabled `-v` compile of `args`, which
/// succeeds.
fn image_notes(d: &Path, args: &[&str]) -> Vec<String> {
    let o = kiln(d, &[&["-v", "-fkiln-bmi-image"], args].concat());
    assert_eq!(o.code, 0, "{args:?}: {}", o.stderr);
    o.stderr
        .lines()
        .filter(|l| l.starts_with("note: imported '") || l.starts_with("note: replayed '"))
        .map(str::to_owned)
        .collect()
}

const IMGT: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-template.cppm");
const IMGT_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-template-run.cpp");

/// Stage S4 (§11.11 "Templates as patterns", WP-1066): function templates
/// come from the image as patterns. With the unit's source blanked, an
/// importer still instantiates them (deduced and explicit, type and
/// non-type arguments), its IR is the replay's, the interface's own
/// instantiation keeps its carried constant, and an ill-formed
/// instantiation is diagnosed exactly as after a replay.
#[test]
fn kind2_declaration_image_templates() {
    let d = scratch("image-templates");
    std::fs::write(d.join("t.cppm"), IMGT).unwrap();
    std::fs::write(d.join("u.cpp"), IMGT_RUN).unwrap();
    std::fs::write(
        d.join("err.cpp"),
        "import imgt;\nstruct S {};\nint main() { S s; it::add(s, s); return it::scale<int, 2>(s); }\n",
    )
    .unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "t.cppm", "-o", "t.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of 43 declarations"), "{}", o.stderr);
    ok(&d, &["-c", "t.pcm", "-o", "t.o"]);
    blank_source(&d, "t.pcm", "blank.pcm");

    let notes = image_notes(&d, &["-fmodule-file=imgt=blank.pcm", "u.cpp", "t.o", "-o", "u"]);
    assert_eq!(notes, ["note: imported 'imgt' from its declaration image"]);
    let out = Command::new(d.join("u")).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "5 3.5 12 10 25 42 7 8 2 9 15 6766 6765 | 4 1\n"
    );

    // The same IR from the image (blank source) as from the replay.
    let mut irs = Vec::new();
    for (flag, pcm) in [("-fkiln-bmi-image", "blank.pcm"), ("-fno-kiln-bmi-image", "t.pcm")] {
        std::fs::copy(d.join(pcm), d.join("m.pcm")).unwrap();
        let m = "-fmodule-file=imgt=m.pcm";
        ok(&d, &[flag, m, "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]);
        irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
        // `heavy<int>`'s `k` is the interface's carried constant: the
        // importer evaluates nothing of the module's.
        ok(&d, &[flag, m, "-fsyntax-only", "-fconstexpr-report=r.json", "u.cpp"]);
        let fib: u64 = report(&d.join("r.json"))
            .iter()
            .filter(|(l, _)| l.starts_with("t.cppm:"))
            .map(|(_, s)| s)
            .sum();
        assert!(fib < 1_000, "{flag}: re-evaluated fib(20): {fib} steps");
    }
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");

    // Instantiation diagnostics, with the real source: identical.
    let mut errs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        let e = kiln(&d, &[flag, "-fsyntax-only", "-fmodule-file=imgt=t.pcm", "err.cpp"]);
        assert_eq!(e.code, 1, "{}", e.stderr);
        errs.push(e.stderr);
    }
    assert!(
        errs[0].contains("in instantiation of function template specialization"),
        "{}",
        errs[0]
    );
    assert_eq!(errs[0], errs[1], "diagnostics differ between the image and the replay");
}

const IMGPK: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-pack.cppm");
const IMGPK_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-pack-run.cpp");

/// WP-1279 (§11.11 "Templates as patterns"): template parameter packs,
/// function parameter packs, `sizeof...`, fold-expressions, `auto` in
/// patterns and non-type default template arguments come from the image.
/// With the unit's source blanked, an importer still instantiates them,
/// its IR is the replay's, and an ill-formed instantiation (an error at a
/// fold's operator, a rejected default) is diagnosed exactly as after a
/// replay.
#[test]
fn kind2_declaration_image_packs() {
    let d = scratch("image-packs");
    std::fs::write(d.join("p.cppm"), IMGPK).unwrap();
    std::fs::write(d.join("u.cpp"), IMGPK_RUN).unwrap();
    std::fs::write(
        d.join("err.cpp"),
        "import imgp;\nstruct S {};\n\
         int main() { S s; ip::sum(s, s); return ip::sz<S, 1, 2>() + ip::def<s>() + ip::all(s); }\n",
    )
    .unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "p.cppm", "-o", "p.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    ok(&d, &["-c", "p.pcm", "-o", "p.o"]);
    blank_source(&d, "p.pcm", "blank.pcm");

    let notes = image_notes(&d, &["-fmodule-file=imgp=blank.pcm", "u.cpp", "p.o", "-o", "u"]);
    assert_eq!(notes, ["note: imported 'imgp' from its declaration image"]);
    let out = Command::new(d.join("u")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "1 3 10 11 1 7 0 6 2 3 9 | 4 4 8 5\n");

    let mut irs = Vec::new();
    for (flag, pcm) in [("-fkiln-bmi-image", "blank.pcm"), ("-fno-kiln-bmi-image", "p.pcm")] {
        std::fs::copy(d.join(pcm), d.join("m.pcm")).unwrap();
        ok(&d, &[flag, "-fmodule-file=imgp=m.pcm", "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]);
        irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
    }
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");

    let mut errs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        let e = kiln(&d, &[flag, "-fsyntax-only", "-fmodule-file=imgp=p.pcm", "err.cpp"]);
        assert_eq!(e.code, 1, "{}", e.stderr);
        errs.push(e.stderr);
    }
    assert!(errs[0].contains("p.cppm:10:"), "{}", errs[0]);
    assert!(
        errs[0].contains("in instantiation of function template specialization"),
        "{}",
        errs[0]
    );
    assert_eq!(errs[0], errs[1], "diagnostics differ between the image and the replay");
}

const IMGCS: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-cspec.cppm");
const IMGCS_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-class-spec-run.cpp");

/// WP-1283 (§11.11 "Templates as patterns"): explicit and partial class
/// template specializations come from the image. With the unit's source
/// blanked, an importer still matches and instantiates them, its IR is
/// the replay's, and an error inside an imported partial
/// specialization's pattern is diagnosed exactly as after a replay, at
/// the array declarator's `[` in the module.
#[test]
fn kind2_declaration_image_class_specs() {
    let d = scratch("image-class-specs");
    std::fs::write(d.join("c.cppm"), IMGCS).unwrap();
    std::fs::write(d.join("u.cpp"), IMGCS_RUN).unwrap();
    std::fs::write(
        d.join("err.cpp"),
        "import imgcs;\ncs::E<void *> e;\ncs::B<void> b;\ncs::B<int> c;\n",
    )
    .unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "c.cppm", "-o", "c.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    ok(&d, &["-c", "c.pcm", "-o", "c.o"]);
    blank_source(&d, "c.pcm", "blank.pcm");

    let notes = image_notes(&d, &["-fmodule-file=imgcs=blank.pcm", "u.cpp", "c.o", "-o", "u"]);
    assert_eq!(notes, ["note: imported 'imgcs' from its declaration image"]);
    let out = Command::new(d.join("u")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "9 40 5 3 2 0 8\n");

    let mut irs = Vec::new();
    for (flag, pcm) in [("-fkiln-bmi-image", "blank.pcm"), ("-fno-kiln-bmi-image", "c.pcm")] {
        std::fs::copy(d.join(pcm), d.join("m.pcm")).unwrap();
        ok(&d, &[flag, "-fmodule-file=imgcs=m.pcm", "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]);
        irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
    }
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");

    let mut errs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        let e = kiln(&d, &[flag, "-fsyntax-only", "-fmodule-file=imgcs=c.pcm", "err.cpp"]);
        assert_eq!(e.code, 1, "{}", e.stderr);
        errs.push(e.stderr);
    }
    assert!(
        errs[0].contains("c.cppm:30:41: error: array has incomplete element type"),
        "{}",
        errs[0]
    );
    assert!(errs[0].contains("in instantiation of template class 'cs::E<void *>'"), "{}", errs[0]);
    assert_eq!(errs[0], errs[1], "diagnostics differ between the image and the replay");
}

const IMGMT: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-membertempl.cppm");
const IMGMT_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-member-template-run.cpp");

/// WP-1292 (§11.11 "Member templates"): member function and class
/// templates, their out-of-line definitions (nested heads included) and
/// templates whose definition inherits a declaration's default arguments
/// come from the image. With the unit's source blanked, an importer still
/// instantiates them, its IR is the replay's, and an error inside an
/// imported member template's pattern is diagnosed exactly as after a
/// replay.
#[test]
fn kind2_declaration_image_member_templates() {
    let d = scratch("image-member-templates");
    std::fs::write(d.join("m.cppm"), IMGMT).unwrap();
    std::fs::write(d.join("u.cpp"), IMGMT_RUN).unwrap();
    std::fs::write(d.join("err.cpp"), "import imgmt;\nint x = mt::Bad<int>::h(0);\n").unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "m.cppm", "-o", "m.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    ok(&d, &["-c", "m.pcm", "-o", "m.o"]);
    blank_source(&d, "m.pcm", "blank.pcm");

    let notes = image_notes(&d, &["-fmodule-file=imgmt=blank.pcm", "u.cpp", "m.o", "-o", "u"]);
    assert_eq!(notes, ["note: imported 'imgmt' from its declaration image"]);
    let out = Command::new(d.join("u")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "1 8 7 2 17 7 26 9 5 12 6 1\n");

    let mut irs = Vec::new();
    for (flag, pcm) in [("-fkiln-bmi-image", "blank.pcm"), ("-fno-kiln-bmi-image", "m.pcm")] {
        std::fs::copy(d.join(pcm), d.join("x.pcm")).unwrap();
        ok(&d, &[flag, "-fmodule-file=imgmt=x.pcm", "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]);
        irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
    }
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");

    let mut errs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        let e = kiln(&d, &[flag, "-fsyntax-only", "-fmodule-file=imgmt=m.pcm", "err.cpp"]);
        assert_eq!(e.code, 1, "{}", e.stderr);
        errs.push(e.stderr);
    }
    assert!(
        errs[0]
            .contains("m.cppm:57:50: error: member reference base type 'int' is not a structure"),
        "{}",
        errs[0]
    );
    assert!(
        errs[0].contains(
            "in instantiation of function template specialization 'mt::Bad<int>::h<int>'"
        ),
        "{}",
        errs[0]
    );
    assert_eq!(errs[0], errs[1], "diagnostics differ between the image and the replay");
}

/// WP-1292 (§11.11 "Polymorphic class templates and pack bases",
/// "Friends", "Implicit members"): with the unit's source blanked, an
/// importer still instantiates polymorphic class templates and
/// pack-expansion bases, finds hidden friends by argument-dependent
/// lookup, lets befriended classes read private members and declares the
/// implicit members that need overload resolution, and its IR is the
/// replay's.
#[test]
fn kind2_declaration_image_polymorphic_templates_friends_and_implicit_members() {
    for (name, module, user, stdout) in [
        (
            "imgpoly",
            include_str!("../../../tests/conformance/module.interface/Inputs/img-polytempl.cppm"),
            include_str!("../../../tests/conformance/module.interface/image-poly-template-run.cpp"),
            "9 4 11 13 17 2 3\n",
        ),
        (
            "imgfr",
            include_str!("../../../tests/conformance/module.interface/Inputs/img-friend.cppm"),
            include_str!("../../../tests/conformance/module.interface/image-friend-run.cpp"),
            "1 0 12 105 6 5 0\n",
        ),
        // WP-1307: friend templates and friends of class templates.
        (
            "imgft",
            include_str!(
                "../../../tests/conformance/module.interface/Inputs/img-friend-template.cppm"
            ),
            include_str!(
                "../../../tests/conformance/module.interface/image-friend-template-run.cpp"
            ),
            "6 18 105 9 | 1 97 0 0 5 9 15\n",
        ),
        // WP-1307: enumerator values of typedef and `bool` types.
        (
            "imgev",
            include_str!("../../../tests/conformance/module.interface/Inputs/img-enum-values.cppm"),
            include_str!("../../../tests/conformance/module.interface/image-enum-values-run.cpp"),
            "1 255 -300 7 3 256 1\n",
        ),
        // WP-1307: explicit instantiation declarations and definitions.
        (
            "imgei",
            include_str!(
                "../../../tests/conformance/module.interface/Inputs/img-explicit-inst.cppm"
            ),
            include_str!("../../../tests/conformance/module.interface/image-explicit-inst-run.cpp"),
            "4 5 400 500 12 97 15 2\n",
        ),
        // WP-1307: static assertions in class templates.
        (
            "imgsat",
            include_str!(
                "../../../tests/conformance/module.interface/Inputs/img-static-assert.cppm"
            ),
            include_str!("../../../tests/conformance/module.interface/image-static-assert-run.cpp"),
            "7 5 2\n",
        ),
        // WP-1307: variable template partial and explicit specializations.
        (
            "imgvs",
            include_str!("../../../tests/conformance/module.interface/Inputs/img-var-spec.cppm"),
            include_str!("../../../tests/conformance/module.interface/image-var-spec-run.cpp"),
            "7 1 1 2\n",
        ),
        // WP-1307: out-of-line members of a partial specialization.
        (
            "imgop",
            include_str!(
                "../../../tests/conformance/module.interface/Inputs/img-partial-member.cppm"
            ),
            include_str!(
                "../../../tests/conformance/module.interface/image-partial-member-run.cpp"
            ),
            "10 5 2\n",
        ),
        // WP-1307: a specialization named before its template's definition.
        (
            "aft",
            include_str!(
                "../../../tests/conformance/module.unit/Inputs/alias-forward-template.cppm"
            ),
            include_str!("../../../tests/conformance/module.unit/alias-forward-template.cpp"),
            "",
        ),
        (
            "imgim",
            include_str!("../../../tests/conformance/module.interface/Inputs/img-implicit.cppm"),
            include_str!(
                "../../../tests/conformance/module.interface/image-implicit-members-run.cpp"
            ),
            "3 3 4 5 1\n",
        ),
    ] {
        let d = scratch(&format!("image-{name}"));
        std::fs::write(d.join("m.cppm"), module).unwrap();
        std::fs::write(d.join("u.cpp"), user).unwrap();
        let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "m.cppm", "-o", "m.pcm"]);
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
        ok(&d, &["-c", "m.pcm", "-o", "m.o"]);
        blank_source(&d, "m.pcm", "blank.pcm");

        let file = format!("-fmodule-file={name}=blank.pcm");
        let notes = image_notes(&d, &[&file, "u.cpp", "m.o", "-o", "u"]);
        assert_eq!(notes, [format!("note: imported '{name}' from its declaration image")]);
        let out = Command::new(d.join("u")).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), stdout);

        let mut irs = Vec::new();
        for (flag, pcm) in [("-fkiln-bmi-image", "blank.pcm"), ("-fno-kiln-bmi-image", "m.pcm")] {
            std::fs::copy(d.join(pcm), d.join("x.pcm")).unwrap();
            let file = format!("-fmodule-file={name}=x.pcm");
            ok(&d, &[flag, &file, "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]);
            irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
        }
        assert!(irs[0] == irs[1], "{name}: IR differs between the image and the replay");
    }
}

/// WP-1307 (§11.11 "Friends"): the unit of friend templates and friends
/// of class templates is written with an image, and an importer's
/// access and lookup errors against it (private members named outside
/// the friends, hidden friends named by qualified names) are the
/// replay's.
#[test]
fn kind2_declaration_image_friend_templates_errors() {
    let d = scratch("image-friend-templates-errors");
    std::fs::write(
        d.join("m.cppm"),
        include_str!("../../../tests/conformance/module.interface/Inputs/img-friend-template.cppm"),
    )
    .unwrap();
    std::fs::write(
        d.join("u.cpp"),
        include_str!(
            "../../../tests/conformance/module.interface/image-friend-template-errors.cpp"
        ),
    )
    .unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "m.cppm", "-o", "m.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    let mut errs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        let e = kiln(&d, &[flag, "-fsyntax-only", "-fmodule-file=imgft=m.pcm", "u.cpp"]);
        assert_eq!(e.code, 1, "{}", e.stderr);
        errs.push(e.stderr);
    }
    assert!(errs[0].contains("no member named 'peek' in namespace 'ft'"), "{}", errs[0]);
    assert!(errs[0].contains("'p' is a private member of '"), "{}", errs[0]);
    assert_eq!(errs[0], errs[1], "diagnostics differ between the image and the replay");
}

/// WP-1283, WP-1307: a static assertion in a class template is checked
/// again by each instantiation; the image carries it as a member, and an
/// importer rejects a failing instantiation exactly as after a replay
/// (the image once dropped the assertion silently).
#[test]
fn kind2_declaration_image_static_assert_in_template() {
    let d = scratch("image-static-assert");
    std::fs::write(
        d.join("s.cppm"),
        "export module imgsa;\nexport namespace sa {\n\
         template <class T> struct P { static_assert(sizeof(T) > 1, \"P needs a wide T\"); };\n\
         template <class T> struct E { };\n\
         template <class T> struct E<T *> { static_assert(sizeof(T) > 1, \"E needs a wide T\"); int x; };\n\
         }\n",
    )
    .unwrap();
    std::fs::write(
        d.join("u.cpp"),
        "import imgsa;\nsa::P<short> a;\nsa::P<char> p;\nsa::E<char *> e;\nsa::E<long *> b;\n",
    )
    .unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "s.cppm", "-o", "s.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    let mut errs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        let e = kiln(&d, &[flag, "-v", "-fsyntax-only", "-fmodule-file=imgsa=s.pcm", "u.cpp"]);
        assert_eq!(e.code, 1, "{}", e.stderr);
        errs.push(e.stderr);
    }
    assert!(errs[0].contains("s.cppm:3:45: error: static assertion failed"), "{}", errs[0]);
    assert!(errs[0].contains("E needs a wide T"), "{}", errs[0]);
    assert!(errs[0].contains("note: imported 'imgsa' from its declaration image"), "{}", errs[0]);
    let diags = |s: &str| -> Vec<String> {
        s.lines().filter(|l| !l.starts_with("note: ")).map(String::from).collect()
    };
    assert_eq!(
        diags(&errs[0]),
        diags(&errs[1]),
        "diagnostics differ between the image and the replay"
    );
}

const IMGA: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-alias.cppm");
const IMGA_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-alias-run.cpp");

/// WP-1279 (§11.11 "Templates as patterns"): alias templates come from
/// the image, with their `using` keyword. With the unit's source blanked,
/// an importer still substitutes and deduces through them, its IR is the
/// replay's, and an error in an alias's substitution is diagnosed exactly
/// as after a replay, at the alias's `using` in the module.
#[test]
fn kind2_declaration_image_alias_templates() {
    let d = scratch("image-alias");
    std::fs::write(d.join("a.cppm"), IMGA).unwrap();
    std::fs::write(d.join("u.cpp"), IMGA_RUN).unwrap();
    std::fs::write(
        d.join("err.cpp"),
        "import imga;\nia::Ptr<int &> p;\nia::Arr3<void> a;\n\
         int x = ia::unbox(3);\nia::Fn<int, int> f;\n",
    )
    .unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "a.cppm", "-o", "a.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    ok(&d, &["-c", "a.pcm", "-o", "a.o"]);
    blank_source(&d, "a.pcm", "blank.pcm");

    let notes = image_notes(&d, &["-fmodule-file=imga=blank.pcm", "u.cpp", "a.o", "-o", "u"]);
    assert_eq!(notes, ["note: imported 'imga' from its declaration image"]);
    let out = Command::new(d.join("u")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "7 5 9 8 3\n");

    let mut irs = Vec::new();
    for (flag, pcm) in [("-fkiln-bmi-image", "blank.pcm"), ("-fno-kiln-bmi-image", "a.pcm")] {
        std::fs::copy(d.join(pcm), d.join("m.pcm")).unwrap();
        ok(&d, &[flag, "-fmodule-file=imga=m.pcm", "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]);
        irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
    }
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");

    let mut errs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        let e = kiln(&d, &[flag, "-fsyntax-only", "-fmodule-file=imga=a.pcm", "err.cpp"]);
        assert_eq!(e.code, 1, "{}", e.stderr);
        errs.push(e.stderr);
    }
    for want in ["a.cppm:4:20: error", "a.cppm:6:20: error", "a.cppm:11:20: note"] {
        assert!(errs[0].contains(want), "{want}: {}", errs[0]);
    }
    assert_eq!(errs[0], errs[1], "diagnostics differ between the image and the replay");
}

const IMGV: &str = include_str!("../../../tests/conformance/module.interface/Inputs/img-var.cppm");
const IMGV_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-var-run.cpp");

/// WP-1279 (§11.11 "Templates as patterns"): variable templates come
/// from the image. With the unit's source blanked, an importer still
/// instantiates them, its IR is the replay's, and an error in an
/// instantiated initializer is diagnosed exactly as after a replay, in
/// the module, under the importer's instantiation note.
#[test]
fn kind2_declaration_image_variable_templates() {
    let d = scratch("image-var");
    std::fs::write(d.join("v.cppm"), IMGV).unwrap();
    std::fs::write(d.join("u.cpp"), IMGV_RUN).unwrap();
    std::fs::write(
        d.join("err.cpp"),
        "import imgv;\nint a = iv::size<void>;\nstruct S {};\nS b = iv::pi<S>;\n\
         int c = iv::sq<1, 2>;\nint d = iv::twice<S>;\n",
    )
    .unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "v.cppm", "-o", "v.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    ok(&d, &["-c", "v.pcm", "-o", "v.o"]);
    blank_source(&d, "v.pcm", "blank.pcm");

    let notes = image_notes(&d, &["-fmodule-file=imgv=blank.pcm", "u.cpp", "v.o", "-o", "u"]);
    assert_eq!(notes, ["note: imported 'imgv' from its declaration image"]);
    let out = Command::new(d.join("u")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "1 314 25 0 0 5\n");

    let mut irs = Vec::new();
    for (flag, pcm) in [("-fkiln-bmi-image", "blank.pcm"), ("-fno-kiln-bmi-image", "v.pcm")] {
        std::fs::copy(d.join(pcm), d.join("m.pcm")).unwrap();
        ok(&d, &[flag, "-fmodule-file=imgv=m.pcm", "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]);
        irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
    }
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");

    let mut errs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        let e = kiln(&d, &[flag, "-fsyntax-only", "-fmodule-file=imgv=v.pcm", "err.cpp"]);
        assert_eq!(e.code, 1, "{}", e.stderr);
        errs.push(e.stderr);
    }
    for want in [
        "v.cppm:3:41: error",
        "in instantiation of variable template specialization 'iv::pi<S>'",
        "v.cppm:7:32: note",
    ] {
        assert!(errs[0].contains(want), "{want}: {}", errs[0]);
    }
    assert_eq!(errs[0], errs[1], "diagnostics differ between the image and the replay");
}

const IMGL: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-lookup.cppm");
const IMGL_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-lookup-run.cpp");

/// WP-1279: patterns calling unresolved names (overload sets, qualified
/// names, explicit template arguments, variable templates, operators,
/// argument-dependent lookup, folds) are carried by the image. With the
/// unit's source blanked, an importer still instantiates and runs them,
/// its IR is the replay's, and errors inside an instantiated pattern
/// point into the module with the replay's instantiation notes.
#[test]
fn kind2_declaration_image_unresolved_names() {
    let d = scratch("image-lookup");
    std::fs::write(d.join("l.cppm"), IMGL).unwrap();
    std::fs::write(d.join("u.cpp"), IMGL_RUN).unwrap();
    std::fs::write(
        d.join("err.cpp"),
        "import imgu;\nstruct V {};\nint a = iu::call(nullptr);\nint b = iu::adl(V{});\n\
         int c = iu::expl<V>(V{});\n",
    )
    .unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "l.cppm", "-o", "l.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    ok(&d, &["-c", "l.pcm", "-o", "l.o"]);
    blank_source(&d, "l.pcm", "blank.pcm");

    let notes = image_notes(&d, &["-fmodule-file=imgu=blank.pcm", "u.cpp", "l.o", "-o", "u"]);
    assert_eq!(notes, ["note: imported 'imgu' from its declaration image"]);
    let out = Command::new(d.join("u")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "2 3 6 8 3 5 6 70 5 10\n");

    let mut irs = Vec::new();
    for (flag, pcm) in [("-fkiln-bmi-image", "blank.pcm"), ("-fno-kiln-bmi-image", "l.pcm")] {
        std::fs::copy(d.join(pcm), d.join("m.pcm")).unwrap();
        ok(&d, &[flag, "-fmodule-file=imgu=m.pcm", "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]);
        irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
    }
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");

    let mut errs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        let e = kiln(&d, &[flag, "-fsyntax-only", "-fmodule-file=imgu=l.pcm", "err.cpp"]);
        assert_eq!(e.code, 1, "{}", e.stderr);
        errs.push(e.stderr);
    }
    for want in [
        "l.cppm:7:54: error: no matching function for call to 'f'",
        "l.cppm:16:52: error: no matching function for call to 'h'",
        "l.cppm:5:50: error",
        "in instantiation of function template specialization 'iu::call<std::nullptr_t>'",
        "err.cpp:5:5: error: no viable conversion from 'V' to 'int'",
    ] {
        assert!(errs[0].contains(want), "{want}: {}", errs[0]);
    }
    assert_eq!(errs[0], errs[1], "diagnostics differ between the image and the replay");
}

const IMGG: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-generic.cppm");
const IMGG_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-generic-run.cpp");

/// WP-1132: bodies using `_Generic` (expression and type-name controls,
/// type and `default` associations, dependent controls, association
/// types and selections) and `__builtin_choose_expr` (constant and
/// dependent conditions, lvalue arms) are carried by the image, with no
/// fallback reason. With the unit's source blanked, an importer still
/// evaluates, instantiates and runs them, and its IR is the replay's.
#[test]
fn kind2_declaration_image_generic_and_choose() {
    let d = scratch("image-generic");
    std::fs::write(d.join("g.cppm"), IMGG).unwrap();
    std::fs::write(d.join("u.cpp"), IMGG_RUN).unwrap();
    // The type-name control form (kiln only: clang 19 crashes on it in a
    // module interface, so the corpus test leaves it out).
    std::fs::write(
        d.join("t.cppm"),
        "export module gt;\n\
         export constexpr int qual_tag() { return _Generic(const int, const int: 4, int: 0, default: 9); }\n\
         export template <class T> constexpr int ttag() { return _Generic(T, int: 10, default: 20); }\n\
         export template <class T> constexpr int ctag(T v) { return _Generic(v, T: 1, default: 2); }\n",
    )
    .unwrap();
    std::fs::write(
        d.join("tu.cpp"),
        "import gt;\n\
         static_assert(qual_tag() == 4 && ttag<int>() == 10 && ttag<char>() == 20 && ctag(1) == 1);\n\
         int main() { return ttag<long>() + qual_tag() + ctag('a') == 25 ? 0 : 1; }\n",
    )
    .unwrap();
    for (src, pcm, decls) in [("g.cppm", "g.pcm", 27), ("t.cppm", "t.pcm", 10)] {
        let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", src, "-o", pcm]);
        assert_eq!(o.code, 0, "{}", o.stderr);
        let carried = format!("with a declaration image of {decls} declarations");
        assert!(o.stderr.contains(&carried), "{src}: {}", o.stderr);
        assert!(!o.stderr.contains("no declaration image"), "{src}: {}", o.stderr);
    }
    ok(&d, &["-c", "g.pcm", "-o", "g.o"]);
    ok(&d, &["-c", "t.pcm", "-o", "t.o"]);
    blank_source(&d, "g.pcm", "gblank.pcm");
    blank_source(&d, "t.pcm", "tblank.pcm");

    let notes = image_notes(&d, &["-fmodule-file=imgg=gblank.pcm", "u.cpp", "g.o", "-o", "u"]);
    assert_eq!(notes, ["note: imported 'imgg' from its declaration image"]);
    let out = Command::new(d.join("u")).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "1 3 14 | 9 7 | 1 2 3 0 1 0 | 5 | 6 60 6 | 8 | 100 100\n"
    );
    let notes = image_notes(&d, &["-fmodule-file=gt=tblank.pcm", "tu.cpp", "t.o", "-o", "tu"]);
    assert_eq!(notes, ["note: imported 'gt' from its declaration image"]);
    assert!(Command::new(d.join("tu")).status().unwrap().success(), "type-name controls");

    // The same IR from the image (blank source) as from the replay.
    for (m, user, blank, real) in
        [("imgg", "u.cpp", "gblank.pcm", "g.pcm"), ("gt", "tu.cpp", "tblank.pcm", "t.pcm")]
    {
        let mut irs = Vec::new();
        for (flag, pcm) in [("-fkiln-bmi-image", blank), ("-fno-kiln-bmi-image", real)] {
            std::fs::copy(d.join(pcm), d.join("m.pcm")).unwrap();
            let mf = format!("-fmodule-file={m}=m.pcm");
            ok(&d, &[flag, &mf, "-S", "-emit-llvm", user, "-o", "u.ll"]);
            irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
        }
        assert!(irs[0] == irs[1], "{m}: IR differs between the image and the replay");
    }
}

const IMGCT: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-class-template.cppm");
const IMGCT_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-class-template-run.cpp");

/// Stage S4 (§11.11 "Class template patterns", WP-1139): class templates
/// come from the image as patterns, with members defined in the class and
/// out of line, static data members, nested types, default arguments and
/// dependent bases. With the unit's source blanked, an importer still
/// instantiates them, its IR is the replay's, the interface's own
/// specializations keep their carried constants, and an ill-formed
/// member instantiation is diagnosed exactly as after a replay.
#[test]
fn kind2_declaration_image_class_templates() {
    let d = scratch("image-class-templates");
    std::fs::write(d.join("t.cppm"), IMGCT).unwrap();
    std::fs::write(d.join("u.cpp"), IMGCT_RUN).unwrap();
    std::fs::write(
        d.join("err.cpp"),
        "import imgct;\nstruct S {};\n\
         int main() { ct::Box<S> b{}; b.twice(); return ct::Derived<S>{}.dy(); }\n",
    )
    .unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "t.cppm", "-o", "t.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of 73 declarations"), "{}", o.stderr);
    ok(&d, &["-c", "t.pcm", "-o", "t.o"]);
    blank_source(&d, "t.pcm", "blank.pcm");

    let notes = image_notes(&d, &["-fmodule-file=imgct=blank.pcm", "u.cpp", "t.o", "-o", "u"]);
    assert_eq!(notes, ["note: imported 'imgct' from its declaration image"]);
    let out = Command::new(d.join("u")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "8 7 4 3 9 9 1 6 12 6766 6765 | 8 2.5 4\n");

    // The same IR from the image (blank source) as from the replay.
    let mut irs = Vec::new();
    for (flag, pcm) in [("-fkiln-bmi-image", "blank.pcm"), ("-fno-kiln-bmi-image", "t.pcm")] {
        std::fs::copy(d.join(pcm), d.join("m.pcm")).unwrap();
        let m = "-fmodule-file=imgct=m.pcm";
        ok(&d, &[flag, m, "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]);
        irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
        // `Heavy<int>::get`'s `k` is the interface's carried constant:
        // the importer evaluates nothing of the module's.
        ok(&d, &[flag, m, "-fsyntax-only", "-fconstexpr-report=r.json", "u.cpp"]);
        let fib: u64 = report(&d.join("r.json"))
            .iter()
            .filter(|(l, _)| l.starts_with("t.cppm:"))
            .map(|(_, s)| s)
            .sum();
        assert!(fib < 1_000, "{flag}: re-evaluated fib(20): {fib} steps");
    }
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");

    // Member instantiation diagnostics, with the real source: identical.
    let mut errs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        let e = kiln(&d, &[flag, "-fsyntax-only", "-fmodule-file=imgct=t.pcm", "err.cpp"]);
        assert_eq!(e.code, 1, "{}", e.stderr);
        errs.push(e.stderr);
    }
    assert!(errs[0].contains("in instantiation of member function"), "{}", errs[0]);
    assert_eq!(errs[0], errs[1], "diagnostics differ between the image and the replay");
}

/// WP-1096, WP-1162 (§11.11 "Part 2a", "ODR merging"): the gate counts
/// only identifiers that reached the parser at global namespace scope,
/// against only the image's global-scope names. So a namespace named like
/// its module (plain or dotted) is built from the image, as is a unit
/// whose names the importer spelled only in a macro, as members, in its
/// own namespace or as locals, or declared in another scope; a real
/// earlier declaration of an image's global-scope name still replays (also
/// inside a linkage specification, an enumeration or an inline
/// namespace), as do a second image declaring the same name and a global
/// module fragment's declaration of it.
#[test]
fn kind2_image_gate_module_names() {
    let d = scratch("image-gate");
    let w = |n: &str, t: &str| std::fs::write(d.join(n), t).unwrap();
    w(
        "m.cppm",
        "export module m;\n\
         export namespace m { constexpr int f() { return 40; } int g() { return 2; } }\n",
    );
    w(
        "ab.cppm",
        "export module a.b;\n\
         export namespace a { namespace b { consteval int h() { return 2; } } }\n",
    );
    w("n.cppm", "export module n;\nexport namespace m { inline int k() { return 1; } }\n");
    w("hm.h", "namespace m { inline int z() { return 1; } }\n");
    for (src, pcm) in [("m.cppm", "m.pcm"), ("ab.cppm", "ab.pcm"), ("n.cppm", "n.pcm")] {
        ok(&d, &["-fkiln-bmi-image", "--precompile", src, "-o", pcm]);
    }
    blank_source(&d, "m.pcm", "mb.pcm");
    blank_source(&d, "ab.pcm", "abb.pcm");
    let (m, mb, abb, n) = (
        "-fmodule-file=m=m.pcm",
        "-fmodule-file=m=mb.pcm",
        "-fmodule-file=a.b=abb.pcm",
        "-fmodule-file=n=n.pcm",
    );
    let image = |k: &str| vec![format!("note: imported '{k}' from its declaration image")];
    let replay = |k: &str, name: &str| {
        vec![format!("note: replayed '{k}': its image declares '{name}', already seen")]
    };

    // The module's own namespace: the image alone builds the program.
    w(
        "u.cpp",
        "import m;\nstatic_assert(m::f() == 40);\n\
         int main() { return m::f() + m::g() == 42 ? 0 : 1; }\n",
    );
    assert_eq!(image_notes(&d, &["-fsyntax-only", mb, "u.cpp"]), image("m"));
    ok(&d, &["-c", "m.pcm", "-o", "m.o"]);
    ok(&d, &["-fkiln-bmi-image", mb, "u.cpp", "m.o", "-o", "u"]);
    assert!(Command::new(d.join("u")).status().unwrap().success(), "program from the image");
    // A dotted name: both components name namespaces of the unit.
    w("uab.cpp", "import a.b;\nstatic_assert(a::b::h() == 2);\n");
    assert_eq!(image_notes(&d, &["-fsyntax-only", abb, "uab.cpp"]), image("a.b"));
    // An implementation unit's implicit import of its interface.
    w("impl.cpp", "module m;\nnamespace m { int k() { return f() + g(); } }\n");
    assert_eq!(image_notes(&d, &["-fsyntax-only", mb, "impl.cpp"]), image("m"));

    // True clashes replay: the importer's namespace, its variable,
    // another image's namespace, a global module fragment's namespace.
    w("c1.cpp", "namespace m { int mine(); }\nimport m;\nint main() { return m::f() - 40; }\n");
    assert_eq!(image_notes(&d, &["-fsyntax-only", m, "c1.cpp"]), replay("m", "m"));
    // `::g` is no redeclaration of the image's `m::g`.
    w("c2.cpp", "int g;\nimport m;\nint main() { return m::f() - 40 + g; }\n");
    assert_eq!(image_notes(&d, &["-fsyntax-only", m, "c2.cpp"]), image("m"));
    w("c3.cpp", "import m;\nimport n;\nint main() { return m::f() + m::k() - 41; }\n");
    let notes = image_notes(&d, &["-fsyntax-only", m, n, "c3.cpp"]);
    assert_eq!(notes, [image("m"), replay("n", "m")].concat());
    w(
        "c4.cppm",
        "module;\n#include \"hm.h\"\nexport module x;\nimport m;\n\
         export int xx() { return m::z() + m::f(); }\n",
    );
    let notes = image_notes(&d, &["--precompile", m, "c4.cppm", "-o", "x.pcm"]);
    assert_eq!(notes, replay("m", "m"));

    // WP-1162: `m` spelled without a global-scope declaration of it.
    let tail = "import m;\nint main() { return m::f() - 40; }\n";
    for (idx, head) in [
        "#define M m::f\n",
        "struct S { int m; };\n",
        "namespace o { namespace m { int k; } }\n",
        "void z() { int m = 0; (void)m; }\n",
        "int arr[] = { [] { int m = 1; return m; }() };\n",
    ]
    .into_iter()
    .enumerate()
    {
        let file = format!("s{idx}.cpp");
        w(&file, &format!("{head}{tail}"));
        let notes = image_notes(&d, &["-fsyntax-only", m, &file]);
        assert_eq!(notes, image("m"), "{head}");
    }
    // ... and with one at global scope, however it is nested.
    for (idx, head) in [
        "extern \"C++\" { namespace m { int k; } }\n",
        "inline namespace v { namespace m { int k; } }\n",
        "namespace { namespace m { int k; } }\n",
        "enum { m };\n",
        "struct T {};\nusing m = T;\n",
    ]
    .into_iter()
    .enumerate()
    {
        let file = format!("r{idx}.cpp");
        w(&file, &format!("{head}{tail}"));
        let out = kiln(&d, &["-v", "-fkiln-bmi-image", "-fsyntax-only", m, &file]);
        let notes: Vec<&str> =
            out.stderr.lines().filter(|l| l.starts_with("note: replayed '")).collect();
        assert_eq!(notes, replay("m", "m"), "{head}: {}", out.stderr);
    }
}

/// WP-1162 (§11.11 "Part 2a", "ODR merging"): the corpus pair over
/// `img-gate-scope.cppm`. Names spelled before the import without a
/// global-scope declaration use the image; a global overload and a
/// reopened namespace replay.
#[test]
fn kind2_image_gate_scopes() {
    let d = scratch("image-gate-scopes");
    std::fs::write(d.join("s.cppm"), IMGS).unwrap();
    std::fs::write(d.join("scope.cpp"), IMGS_SCOPE).unwrap();
    std::fs::write(d.join("over.cpp"), IMGS_OVER).unwrap();
    ok(&d, &["-fkiln-bmi-image", "--precompile", "s.cppm", "-o", "s.pcm"]);
    let m = "-fmodule-file=imgs=s.pcm";
    let notes = image_notes(&d, &["-fsyntax-only", m, "scope.cpp"]);
    assert_eq!(notes, ["note: imported 'imgs' from its declaration image"]);
    let notes = image_notes(&d, &["-fsyntax-only", m, "over.cpp"]);
    assert_eq!(notes, ["note: replayed 'imgs': its image declares 'twice', already seen"]);
}

const IMGS: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-gate-scope.cppm");
const IMGS_SCOPE: &str =
    include_str!("../../../tests/conformance/module.interface/image-gate-scope-run.cpp");
const IMGS_OVER: &str =
    include_str!("../../../tests/conformance/module.interface/image-gate-overload-run.cpp");

/// Stage S3 (§11.11 "Part 2a", WP-1065): non-template classes, their
/// members, bases, constructors, destructor and implicit special members
/// come from the image alone. With the unit's source blanked in the
/// capsule, an importer still derives from, constant-evaluates and runs
/// them.
#[test]
fn kind2_declaration_image_classes() {
    let d = scratch("image-classes");
    let src = "export module c;\n\
               namespace n {\n\
               export struct P { int x = 1, y = 2; constexpr P() = default;\n\
                 constexpr P(int a, int b) : x(a), y(b) {}\n\
                 constexpr P operator+(const P &o) const { return {x + o.x, y + o.y}; }\n\
                 static constexpr int kDims = 2; static int made; };\n\
               int P::made = 0;\n\
               export class C { public: struct Step { long by = 1; };\n\
                 constexpr C &tick(Step s) { v += s.by; return *this; }\n\
                 constexpr C &tick() { return tick(Step{}); }\n\
                 constexpr long get() const { return v; } int twice() const;\n\
                 private: long v = 0; };\n\
               int C::twice() const { return int(v) * 2; }\n\
               export struct B1 { int b = 3; };\n\
               export struct F final : B1 {};\n\
               export struct D : B1 { P p; constexpr int sum() const { return b + p.x + p.y; } };\n\
               export long trace = 0;\n\
               export struct T { int id; explicit T(int i) : id(i) {} ~T() { trace = trace * 10 + id; } };\n\
               export constexpr P kP = P(3, 4) + P();\n\
               export constexpr long kTicks = C().tick().tick(C::Step{5}).get();\n\
               }\n";
    std::fs::write(d.join("c.cppm"), src).unwrap();
    std::fs::write(
        d.join("u.cpp"),
        "import c;\n\
         using namespace n;\n\
         static_assert(kP.x == 4 && kP.y == 6 && kTicks == 6 && P::kDims == 2 && D().sum() == 6);\n\
         struct E : D { constexpr int all() const { return sum() + 1; } };\n\
         static_assert(E().all() == 7 && sizeof(E) == 3 * sizeof(int) && F().b == 3);\n\
         int main() { { T t(4); (void)T(5).id; } C c; c.tick(); P::made += 1;\n\
           return trace == 54 && c.twice() == 2 && P::made == 1 && E().all() == 7 ? 0 : 1; }\n",
    )
    .unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "c.cppm", "-o", "c.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    ok(&d, &["-c", "c.pcm", "-o", "c.o"]);
    blank_source(&d, "c.pcm", "blank.pcm");
    let notes = image_notes(&d, &["-fsyntax-only", "-fmodule-file=c=blank.pcm", "u.cpp"]);
    assert_eq!(notes, ["note: imported 'c' from its declaration image"]);
    ok(&d, &["-fkiln-bmi-image", "-fmodule-file=c=blank.pcm", "u.cpp", "c.o", "-o", "u"]);
    assert!(Command::new(d.join("u")).status().unwrap().success(), "program from the image");
    let o = kiln(&d, &["-fsyntax-only", "-fmodule-file=c=blank.pcm", "u.cpp"]);
    assert_eq!(o.code, 1, "replaying a blank source fails: {}", o.stderr);
}

const IMGP: &str = include_str!("../../../tests/conformance/module.interface/Inputs/img-poly.cppm");
const IMGP_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-poly-run.cpp");

/// The vtable, VTT, construction-vtable, RTTI and thunk lines of `ir`
/// (their symbols and linkage), sorted.
fn vtable_lines(ir: &str) -> Vec<&str> {
    let mut v: Vec<&str> = ir
        .lines()
        .filter(|l| {
            ["@_ZTV", "@_ZTT", "@_ZTC", "@_ZTI", "@_ZTS"].iter().any(|p| l.starts_with(p))
                || (l.starts_with("define ") || l.starts_with("declare "))
                    && (l.contains("@_ZTh") || l.contains("@_ZTv"))
        })
        .collect();
    v.sort_unstable();
    v
}

/// Stage S3 (§11.11 "Classes (S3)", WP-1120): polymorphic classes come
/// from the image: virtual and pure virtual functions, overriders,
/// `final`, virtual destructors, a key function defined in the unit, a
/// diamond of virtual bases and an inline-only polymorphic class. With
/// the unit's source blanked, the importer runs them, and its IR (vtables,
/// VTTs, RTTI and thunks with their linkage, [class.virtual], Itanium
/// key-function placement) is the replay's and a kind-1 import's.
#[test]
fn kind2_declaration_image_polymorphic() {
    let d = scratch("image-polymorphic");
    std::fs::write(d.join("p.cppm"), IMGP).unwrap();
    std::fs::write(d.join("u.cpp"), IMGP_RUN).unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "p.cppm", "-o", "p.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    assert!(!o.stderr.contains("no declaration image"), "{}", o.stderr);
    ok(&d, &["-c", "p.pcm", "-o", "p.o"]);
    ok(&d, &["-fkiln-bmi-kind=1", "--precompile", "p.cppm", "-o", "k1.pcm"]);
    blank_source(&d, "p.pcm", "blank.pcm");

    let notes = image_notes(&d, &["-fmodule-file=imgp=blank.pcm", "u.cpp", "p.o", "-o", "u"]);
    assert_eq!(notes, ["note: imported 'imgp' from its declaration image"]);
    let out = Command::new(d.join("u")).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "animal 0 0, dog 4 8, animal 2 4, puppy 4 8, 1211321 | 9 18 | 117 117 117 21 7 | 6 2\n"
    );

    // The same IR from the image (blank source), the replay and kind 1.
    let mut irs = Vec::new();
    for (flag, pcm) in [
        ("-fkiln-bmi-image", "blank.pcm"),
        ("-fno-kiln-bmi-image", "p.pcm"),
        ("-fno-kiln-bmi-image", "k1.pcm"),
    ] {
        std::fs::copy(d.join(pcm), d.join("m.pcm")).unwrap();
        ok(&d, &[flag, "-fmodule-file=imgp=m.pcm", "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]);
        irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
    }
    let vt = vtable_lines(&irs[0]);
    assert_eq!(vt, vtable_lines(&irs[2]), "vtables and RTTI differ from a kind-1 import");
    // The key function is in the unit: the vtable is the unit's; an
    // importer's own class gets a vague-linkage one.
    assert!(vt.iter().any(|l| l.starts_with("@_ZTVN2ipW4imgp6AnimalE = external ")), "{vt:#?}");
    assert!(vt.iter().any(|l| l.starts_with("@_ZTV5Puppy = linkonce_odr ")), "{vt:#?}");
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");
    assert!(irs[0] == irs[2], "IR differs between the image and a kind-1 import");
}

const IMGU: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-using.cppm");
const IMGU_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-using-run.cpp");
const IMGU_HIDDEN: &str =
    include_str!("../../../tests/conformance/module.interface/image-using-hidden.cpp");

/// Stage S6 (§11.11 "Imports, partitions and using-declarations",
/// WP-1068): using-declarations, a using-enum-declaration, namespace
/// aliases and a using-directive come from the image. With the unit's
/// source blanked the importer still finds names through them (and ADL
/// through a class they name), runs, emits the replay's IR, and a
/// non-exported using-declaration stays invisible.
#[test]
fn kind2_declaration_image_using() {
    let d = scratch("image-using");
    std::fs::write(d.join("u.cppm"), IMGU).unwrap();
    std::fs::write(d.join("r.cpp"), IMGU_RUN).unwrap();
    std::fs::write(d.join("h.cpp"), IMGU_HIDDEN).unwrap();
    let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "u.cppm", "-o", "u.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    ok(&d, &["-c", "u.pcm", "-o", "u.o"]);
    blank_source(&d, "u.pcm", "blank.pcm");

    let notes = image_notes(&d, &["-fmodule-file=imgu=blank.pcm", "r.cpp", "u.o", "-o", "r"]);
    assert_eq!(notes, ["note: imported 'imgu' from its declaration image"]);
    let out = Command::new(d.join("r")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "3 11 2 2 11 | 5 16 42 9\n");

    let mut irs = Vec::new();
    for (flag, pcm) in [("-fkiln-bmi-image", "blank.pcm"), ("-fno-kiln-bmi-image", "u.pcm")] {
        std::fs::copy(d.join(pcm), d.join("m.pcm")).unwrap();
        ok(&d, &[flag, "-fmodule-file=imgu=m.pcm", "-S", "-emit-llvm", "r.cpp", "-o", "r.ll"]);
        irs.push(std::fs::read_to_string(d.join("r.ll")).unwrap());
    }
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");

    // The same three errors, built from the image or replayed.
    let mut errs = Vec::new();
    for (flag, pcm) in [("-fkiln-bmi-image", "blank.pcm"), ("-fno-kiln-bmi-image", "u.pcm")] {
        let o = kiln(&d, &[flag, &format!("-fmodule-file=imgu={pcm}"), "-fsyntax-only", "h.cpp"]);
        assert_ne!(o.code, 0, "{}", o.stderr);
        let e: Vec<String> = o
            .stderr
            .lines()
            .filter(|l| l.contains("error: "))
            .map(|l| l[l.find("h.cpp").unwrap_or(0)..].to_owned())
            .collect();
        errs.push(e);
    }
    assert_eq!(errs[0].len(), 3, "{:#?}", errs[0]);
    assert_eq!(errs[0], errs[1]);
}

const IMGP_A: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-part-a.cppm");
const IMGP_B: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-part-b.cppm");
const IMGPART: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-part.cppm");
const IMGP_IMPL: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-part-impl.cpp");
const IMGR: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-part-reexp.cppm");
const IMGPART_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-part-run.cpp");
const IMGP_HIDDEN: &str =
    include_str!("../../../tests/conformance/module.interface/image-part-hidden.cpp");

/// Stage S6 (§11.11 "Imports, partitions and using-declarations",
/// WP-1068): interface and implementation partitions, a primary that
/// re-exports one and imports the other, and a module that imports one
/// module and re-exports another all carry an image after a replayed
/// preamble. With everything after each preamble blanked, the consumer
/// and an implementation unit build, the program runs, the IR is the
/// replay's and the visibility errors are the same. A preamble ending in
/// an open conditional replays the rest of its unit.
#[test]
#[allow(clippy::too_many_lines)] // one module project, built both ways
fn kind2_declaration_image_imports() {
    let d = scratch("image-imports");
    let units = [
        ("imgu", "u.cppm", IMGU),
        ("imgp:a", "pa.cppm", IMGP_A),
        ("imgp:b", "pb.cppm", IMGP_B),
        ("imgp", "p.cppm", IMGPART),
        ("imgr", "r.cppm", IMGR),
    ];
    std::fs::write(d.join("impl.cpp"), IMGP_IMPL).unwrap();
    std::fs::write(d.join("run.cpp"), IMGPART_RUN).unwrap();
    std::fs::write(d.join("h.cpp"), IMGP_HIDDEN).unwrap();
    let mut mf: Vec<String> = Vec::new();
    let mut blank: Vec<String> = Vec::new();
    for (i, (name, src, text)) in units.iter().enumerate() {
        std::fs::write(d.join(src), text).unwrap();
        let pcm = format!("m{i}.pcm");
        let mut args: Vec<&str> = vec!["-v", "-fkiln-bmi-image"];
        args.extend(mf.iter().map(String::as_str));
        args.extend(["--precompile", src, "-o", &pcm]);
        let o = kiln(&d, &args);
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert!(o.stderr.contains("with a declaration image of "), "{name}: {}", o.stderr);
        let mut args: Vec<&str> = mf.iter().map(String::as_str).collect();
        let obj = format!("m{i}.o");
        args.extend(["-c", &pcm, "-o", &obj]);
        ok(&d, &args);
        let b = format!("b{i}.pcm");
        if i == 0 {
            blank_source(&d, &pcm, &b);
        } else {
            blank_after_preamble(&d, &pcm, &b);
        }
        mf.push(format!("-fmodule-file={name}={pcm}"));
        blank.push(format!("-fmodule-file={name}={b}"));
    }
    let objs = ["m0.o", "m1.o", "m2.o", "m3.o", "m4.o"];

    let mut args: Vec<&str> = blank.iter().map(String::as_str).collect();
    args.extend(["-c", "impl.cpp", "-o", "impl.o"]);
    let notes = image_notes(&d, &args);
    assert_eq!(notes.len(), 3, "{notes:#?}");
    assert!(notes.iter().all(|n| n.ends_with("from its declaration image")), "{notes:#?}");
    let mut args: Vec<&str> = blank.iter().map(String::as_str).collect();
    args.extend(["run.cpp", "impl.o", "-o", "run"]);
    args.extend(objs);
    let notes = image_notes(&d, &args);
    assert_eq!(notes.len(), 5, "{notes:#?}");
    assert!(notes.iter().all(|n| n.ends_with("from its declaration image")), "{notes:#?}");
    let out = Command::new(d.join("run")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "25 42 9 7 14\n");

    let mut irs = Vec::new();
    for (flag, m) in [("-fkiln-bmi-image", &blank), ("-fno-kiln-bmi-image", &mf)] {
        let mut args: Vec<&str> = vec![flag];
        args.extend(m.iter().map(String::as_str));
        args.extend(["-S", "-emit-llvm", "run.cpp", "-o", "run.ll"]);
        ok(&d, &args);
        irs.push(std::fs::read_to_string(d.join("run.ll")).unwrap());
    }
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");

    let mut errs = Vec::new();
    for (flag, m) in [("-fkiln-bmi-image", &blank), ("-fno-kiln-bmi-image", &mf)] {
        let mut args: Vec<&str> = vec![flag];
        args.extend(m.iter().map(String::as_str));
        args.extend(["-fsyntax-only", "h.cpp"]);
        let o = kiln(&d, &args);
        assert_ne!(o.code, 0, "{}", o.stderr);
        let e: Vec<String> = o
            .stderr
            .lines()
            .filter(|l| l.contains("error: "))
            .map(|l| l[l.find("h.cpp").unwrap_or(0)..].to_owned())
            .collect();
        errs.push(e);
    }
    assert_eq!(errs[0].len(), 3, "{:#?}", errs[0]);
    assert_eq!(errs[0], errs[1]);

    // The stop inside an open `#if`: the rest of the unit is replayed.
    std::fs::write(
        d.join("c.cppm"),
        "export module imgc;\n#if 1\nimport imgp;\n#endif\nexport constexpr int cv() { return 3; }\n",
    )
    .unwrap();
    let mut args: Vec<&str> = vec!["-v", "-fkiln-bmi-image"];
    args.extend(mf[1..4].iter().map(String::as_str));
    args.extend(["--precompile", "c.cppm", "-o", "c.pcm"]);
    let o = kiln(&d, &args);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    std::fs::write(d.join("cu.cpp"), "import imgc;\nstatic_assert(cv() == 3);\n").unwrap();
    let mut args: Vec<&str> = mf[1..4].iter().map(String::as_str).collect();
    args.extend(["-fmodule-file=imgc=c.pcm", "-fsyntax-only", "cu.cpp"]);
    let notes = image_notes(&d, &args);
    assert!(
        notes.contains(&"note: replayed 'imgc': its preamble ends in a conditional".to_owned()),
        "{notes:#?}"
    );

    // What refers to an imported declaration, and an import in an
    // included file, still fall back.
    std::fs::write(d.join("i.h"), "import imgp;\n").unwrap();
    for (src, why) in [
        (
            "export module f;\nimport imgp;\nexport inline int g() { return top::answer(); }\n",
            "a body referring to a declaration outside the image",
        ),
        (
            "export module f;\nimport imgp;\nexport int g(pa::Pt);\n",
            "a qualifier naming a namespace outside the image",
        ),
        (
            "export module f;\nimport imgp;\nexport struct D : top::Wrap {};\n",
            "a type declared outside the image",
        ),
        (
            "export module f;\nimport imgp;\nexport namespace q = pa;\n",
            "a using-directive or alias naming a namespace outside the image",
        ),
        (
            "export module f;\n#include \"i.h\"\nexport int g();\n",
            "an import outside the module's main file, or code after its `;`",
        ),
        (
            "export module f;\nimport imgp; export int g();\n",
            "an import outside the module's main file, or code after its `;`",
        ),
    ] {
        std::fs::write(d.join("f.cppm"), src).unwrap();
        let mut args: Vec<&str> = vec!["-v", "-fkiln-bmi-image"];
        args.extend(mf[1..4].iter().map(String::as_str));
        args.extend(["--precompile", "f.cppm", "-o", "f.pcm"]);
        let o = kiln(&d, &args);
        assert_eq!(o.code, 0, "{src}: {}", o.stderr);
        assert!(
            o.stderr.contains(&format!("(no declaration image: {why})")),
            "{src}: {}",
            o.stderr
        );
    }
    // A `//` comment after the `;` still leaves the preamble at a line
    // start ([cpp.module], [cpp.import]: the rest of the line is text).
    std::fs::write(d.join("f.cppm"), "export module f;\nimport imgp; // c\nexport int g();\n")
        .unwrap();
    let mut args: Vec<&str> = vec!["-v", "-fkiln-bmi-image"];
    args.extend(mf[1..4].iter().map(String::as_str));
    args.extend(["--precompile", "f.cppm", "-o", "f.pcm"]);
    let o = kiln(&d, &args);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
}

/// A unit with a node the image does not carry is written without one,
/// and `-v` names the first such node (§11.11 "Part 2a", "Fallback").
#[test]
#[allow(clippy::too_many_lines)] // a table of (unit, reason) cases
fn kind2_declaration_image_fallback() {
    let d = scratch("image-fallback");
    for (src, why) in [
        (
            "export module f;\nexport inline int g() { return [] { return 1; }(); }\n",
            "a local declaration the image does not carry",
        ),
        (
            "export module f;\nexport inline int g() { try { return 1; } catch (...) { return 0; } }\n",
            "a try block",
        ),
        (
            "export module f;\n#line 40 \"other.h\"\nexport constexpr int g() { return 1; }\n",
            "a file with #line directives or line markers",
        ),
        (
            "export module f;\ntemplate <class T> int h(T) { return 1; }\nexport template <class T> struct S { friend int h<>(S); };\nexport int k = h(S<int>{});\n",
            "a friend specialization or member of a class template",
        ),
        (
            "export module f;\nexport struct S { union { int a; float b; }; };\nexport constexpr int k = 1;\n",
            "an anonymous union or struct member",
        ),
        (
            "export module f;\nexport inline void g(int *p) { delete p; }\nexport constexpr int k = 1;\n",
            "a new or delete expression",
        ),
        (
            "export module f;\nexport template <class T> struct B { T v; };\n\
             template <class T> struct B<T *> { T v; };\ntemplate struct B<int *>;\n",
            "an explicit instantiation of a partial specialization",
        ),
        (
            "export module f;\nexport template <class T> struct B {};\n\
             template <class T> struct B<T *>;\ntemplate <class T> struct B<T *> { T v; };\n",
            "a redeclared partial specialization",
        ),
        (
            "export module f;\nexport template <template <class> class TT> struct W {};\n",
            "a template",
        ),
        (
            "export module f;\nexport template <class T> struct A { int f() { return 1; } };\n\
             export inline int h() { A<int> a; return a.f(); }\n",
            "an implicit member of a class template specialization",
        ),
        (
            "export module f;\nexport struct B { int f(); };\nexport struct S { friend int B::f(); };\n",
            "a friend member function",
        ),
        (
            "export module f;\nexport struct O {\n  template <class T> struct I { I(T) {} };\n  \
             template <class T> I(T*) -> I<T>;\n};\n",
            "a member deduction guide",
        ),
        (
            "export module f;\nexport template <class T> requires (sizeof(T) > 1) struct A {};\n\
             export inline int g() { return sizeof(A<long>); }\n",
            "a specialization of a constrained template",
        ),
        (
            "export module f;\nexport template <int N> struct F { int a : N; int b = 4; };\n\
             export inline int g() { F<3> x{}; x.a = 7; return x.a + x.b; }\n",
            "a specialization of a class template with a bit-field or a default member initializer",
        ),
        (
            "export module f;\nexport template <class T> T e(T t) { return t; }\n\
             template <> int e<int>(int t) { return t + 1; }\n",
            "a template specialization",
        ),
        (
            "export module f;\nexport struct B { int f() { return 1; } };\n\
             export struct D : B { using B::f; };\n",
            "a using-declaration in a class",
        ),
        (
            "export module f;\nexport template <class T> struct B { int f(); };\n\
             export template <class T> struct D : B<T> { using B<T>::f; };\n",
            "a dependent using-declaration",
        ),
        (
            "export module f;\nexport struct B { B(int) {} };\nexport struct D : B { using B::B; };\n",
            "an inherited constructor or assignment operator",
        ),
        ("module;\nint h();\nexport module f;\nexport int g();\n", "a global module fragment"),
        // WP-1314: a defaulted `==` is carried now (image-defaulted-cmp-run.cpp).
    ] {
        std::fs::write(d.join("f.cppm"), src).unwrap();
        let o = kiln(&d, &["-v", "-fkiln-bmi-image", "--precompile", "f.cppm", "-o", "f.pcm"]);
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert!(
            o.stderr.contains("(no declaration image: ") && o.stderr.contains(why),
            "{src}: {}",
            o.stderr
        );
    }
}

/// A damaged image section behind a valid checksum is the corrupt-file
/// error (or, for a flip that still decodes, at worst a different
/// module), never a panic.
#[test]
fn kind2_damaged_image_is_diagnosed() {
    use kiln_pp::bmi;
    let d = setup_im("image-damaged");
    ok(&d, &["-fkiln-bmi-image", "--precompile", "im.cppm", "-o", "im.pcm"]);
    let capsule = bmi::read(&std::fs::read(d.join("im.pcm")).unwrap()).unwrap();
    let sec = capsule.section(bmi::SECTION_IMAGE).unwrap().to_vec();
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for i in 0..30 {
        let mut c = capsule.clone();
        let s = c.sections.iter_mut().find(|s| s.tag == bmi::SECTION_IMAGE).unwrap();
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let at = (seed % sec.len() as u64) as usize;
        match i % 3 {
            0 => s.bytes.truncate(at),
            1 => s.bytes[at] ^= 0xff,
            _ => s.bytes.push(0),
        }
        std::fs::write(d.join("bad.pcm"), bmi::write(&c)).unwrap();
        let o =
            kiln(&d, &["-fkiln-bmi-image", "-fsyntax-only", "-fmodule-file=im=bad.pcm", "u.cpp"]);
        assert!(o.code == 0 || o.code == 1, "damage {i}: {}", o.stderr);
        if i % 3 != 1 {
            assert!(
                o.stderr.contains("fatal error: ") && o.stderr.contains("declaration image"),
                "damage {i}: {}",
                o.stderr
            );
        }
    }
}

/// The header of the S7 tests: functions, classes and templates, some of
/// them made by macros, #included in the purview of `m` (HM).
const HH: &str = "#define DECL_FN(name, v) inline int name() { return v; }\n\
                  #define CALL_NOPE(o) o.nope()\n\
                  #define TWICE(x) ((x) + (x))\n\
                  namespace hh {\n\
                  inline int add(int a, int b) { return a + b; }\n\
                  struct P { int x; int get() const { return x; } };\n\
                  template <class T> T twice(T t) { return TWICE(t.w); }\n\
                  template <class T> struct Box { T v; void bad() { CALL_NOPE(v); } };\n\
                  DECL_FN(seven, 7)\n\
                  }\n";
const HM: &str = "export module m;\nexport {\n#include \"h.h\"\n}\n\
                  export constexpr int sq(int x) { return x * x; }\n";
/// Errors located in the header: two instantiations whose bodies are
/// ill-formed there (through macros), and a call with a note there.
const HUSE: &str = "import m;\nint main() {\n  hh::Box<int> b{1};\n  b.bad();\n\
                    return hh::add(1) + hh::twice(3) + hh::seven() + hh::P{4}.get() + sq(2);\n}\n";

fn setup_hm(name: &str) -> PathBuf {
    let d = scratch(name);
    std::fs::write(d.join("h.h"), HH).unwrap();
    std::fs::write(d.join("m.cppm"), HM).unwrap();
    std::fs::write(d.join("u.cpp"), HUSE).unwrap();
    ok(&d, &["-fkiln-bmi-image", "--precompile", "m.cppm", "-o", "m.pcm"]);
    d
}

/// The failing compile of `u.cpp` against `m.pcm`, with `flag`.
fn header_errors(d: &Path, flag: &str) -> String {
    let o = kiln(d, &[flag, "-fsyntax-only", "-fmodule-file=m=m.pcm", "u.cpp"]);
    assert_eq!(o.code, 1, "{flag}: {}", o.stderr);
    o.stderr
}

/// Whether a `-v` compile of `u.cpp` built `m` from its image, and its
/// lines naming `h.h` or the import.
fn header_notes(d: &Path) -> (bool, Vec<String>) {
    let o = kiln(d, &["-v", "-fkiln-bmi-image", "-fsyntax-only", "-fmodule-file=m=m.pcm", "u.cpp"]);
    let notes: Vec<String> = o
        .stderr
        .lines()
        .filter(|l| l.starts_with("note: imported '") || l.starts_with("note: replayed '"))
        .chain(o.stderr.lines().filter(|l| l.starts_with("note: './h.h'")))
        .map(str::to_owned)
        .collect();
    (notes.iter().any(|l| l == "note: imported 'm' from its declaration image"), notes)
}

/// Stage S7 (§11.11 "File table", "Expansion records", WP-1070): a
/// purview that #includes a header is carried by the image, and a
/// diagnostic in an imported header declaration shows the same caret,
/// include stack, macro notes and instantiation notes as after a replay.
#[test]
fn kind2_image_header_locations() {
    let d = setup_hm("image-header");
    let (image, notes) = header_notes(&d);
    assert!(image, "{notes:?}");
    let img = header_errors(&d, "-fkiln-bmi-image");
    for want in [
        "./h.h:5:12: note: 'add' declared here\n    5 | inline int add(int a, int b) { return a + b; }\n      |            ^\n",
        "./h.h:8:51: error: member reference base type 'int' is not a structure or union\n    8 | template <class T> struct Box { T v; void bad() { CALL_NOPE(v); } };\n      |                                                   ^\n",
        "./h.h:2:23: note: expanded from macro 'CALL_NOPE'\n    2 | #define CALL_NOPE(o) o.nope()\n      |                       ^\n",
        "u.cpp:4:5: note: in instantiation of member function 'hh::Box<int>::bad' requested here",
        "In file included from m.cppm:3:\n./h.h:7:49: error: member reference base type 'int' is not a structure or union",
    ] {
        assert!(img.contains(want), "{want}\n{img}");
    }
    assert_eq!(img, header_errors(&d, "-fno-kiln-bmi-image"), "image and replay agree");
}

/// A header edited after its module file was built never gives a wrong
/// caret: with the same size the image is used and the module file's
/// recorded text is shown (with a `-v` note); with another size, or
/// missing, the unit replays from its recorded text, with the reason.
#[test]
fn kind2_image_edited_header() {
    let d = setup_hm("image-edited-header");
    let before = header_errors(&d, "-fno-kiln-bmi-image");
    std::fs::write(d.join("h.h"), HH.replace("nope()", "nopf()")).unwrap();
    let (image, notes) = header_notes(&d);
    assert!(image, "{notes:?}");
    assert!(
        notes.iter().any(|l| l
            == "note: './h.h' changed since its module file was built; its recorded text is shown"),
        "{notes:?}"
    );
    assert_eq!(header_errors(&d, "-fkiln-bmi-image"), before, "same size: the recorded text");
    // Without a diagnostic in it, the header's text is never read.
    std::fs::write(d.join("ok.cpp"), "import m;\nint main() { return hh::add(1, 2) - 3; }\n")
        .unwrap();
    let o =
        kiln(&d, &["-v", "-fkiln-bmi-image", "-fsyntax-only", "-fmodule-file=m=m.pcm", "ok.cpp"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(
        o.stderr.contains("note: imported 'm' from its declaration image")
            && !o.stderr.contains("changed since"),
        "{}",
        o.stderr
    );
    for (edit, why) in [
        (
            Some(format!("{HH}// one more line\n")),
            "'./h.h' changed since the module file was built",
        ),
        (None, "'./h.h' is missing"),
    ] {
        match &edit {
            Some(t) => std::fs::write(d.join("h.h"), t).unwrap(),
            None => std::fs::remove_file(d.join("h.h")).unwrap(),
        }
        let (image, notes) = header_notes(&d);
        assert!(
            !image && notes.iter().any(|l| l.starts_with("note: replayed 'm'") && l.contains(why)),
            "{notes:?}"
        );
        assert_eq!(header_errors(&d, "-fkiln-bmi-image"), before, "{why}: the replay's carets");
    }
}

/// `-fkiln-bmi-image-census` (§11.11 "Fallback census") counts the first
/// fallback reason of each top-level declaration, with an example, and
/// writes the same module file as without it.
#[test]
fn kind2_image_census() {
    let d = scratch("image-census");
    std::fs::write(
        d.join("c.cppm"),
        "export module c;\n\
         export inline int g() { try { return 1; } catch (...) { return 0; } }\n\
         export inline int h() { try { return 2; } catch (...) { return 0; } }\n\
         export inline int k() { return [] { return 1; }(); }\n\
         export constexpr int sq(int x) { return x * x; }\n\
         export struct S { int a; };\n",
    )
    .unwrap();
    std::fs::write(d.join("p.cpp"), "int main() {}\n").unwrap();
    ok(&d, &["-fkiln-bmi-image", "--precompile", "c.cppm", "-o", "c.pcm"]);
    let o = kiln(
        &d,
        &["-fkiln-bmi-image", "-fkiln-bmi-image-census", "--precompile", "c.cppm", "-o", "c2.pcm"],
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    let lines: Vec<&str> = o.stderr.lines().collect();
    assert_eq!(
        lines,
        [
            "image census for 'c': 5 top-level declarations, 2 carried, 3 not",
            "      2  a try block  (first: c.cppm:2:19)",
            "      1  a local declaration the image does not carry  (first: c.cppm:4:19)",
        ],
        "{}",
        o.stderr
    );
    let (a, b) =
        (std::fs::read(d.join("c.pcm")).unwrap(), std::fs::read(d.join("c2.pcm")).unwrap());
    assert!(a == b, "the census leaves the module file unchanged");
    let o = kiln(&d, &["-fkiln-bmi-image-census", "-fsyntax-only", "p.cpp"]);
    assert_eq!((o.code, o.stderr.as_str()), (0, "image census: not a module unit\n"));
}

const IMGC: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-constraints.cppm");
const IMGC_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-constraints-run.cpp");
const IMGC_ERR: &str =
    include_str!("../../../tests/conformance/module.interface/image-constraints-errors.cpp");
const IMGD: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-deptypes.cppm");
const IMGD_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-deptypes-run.cpp");
const IMGD_ERR: &str =
    include_str!("../../../tests/conformance/module.interface/image-deptypes-errors.cpp");

/// Builds module `name` from `src` with a declaration image, then checks
/// that with the unit's source blanked `run` still links and prints
/// `stdout`, that its IR is the replay's, and that `err`'s diagnostics
/// are the replay's; returns them.
fn image_round_trip(
    dir: &str,
    name: &str,
    src: &str,
    run: &str,
    err: &str,
    stdout: &str,
) -> String {
    let d = scratch(dir);
    std::fs::write(d.join("m.cppm"), src).unwrap();
    std::fs::write(d.join("u.cpp"), run).unwrap();
    std::fs::write(d.join("err.cpp"), err).unwrap();
    let std = "-std=c++20";
    let o = kiln(&d, &[std, "-v", "-fkiln-bmi-image", "--precompile", "m.cppm", "-o", "a.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("with a declaration image of "), "{}", o.stderr);
    ok(&d, &[std, "-c", "a.pcm", "-o", "a.o"]);
    blank_source(&d, "a.pcm", "blank.pcm");

    let mf = format!("-fmodule-file={name}=blank.pcm");
    let notes = image_notes(&d, &[std, &mf, "u.cpp", "a.o", "-o", "u"]);
    assert_eq!(notes, [format!("note: imported '{name}' from its declaration image")]);
    let out = Command::new(d.join("u")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), stdout);

    let mf = format!("-fmodule-file={name}=m.pcm");
    let mut irs = Vec::new();
    for (flag, pcm) in [("-fkiln-bmi-image", "blank.pcm"), ("-fno-kiln-bmi-image", "a.pcm")] {
        std::fs::copy(d.join(pcm), d.join("m.pcm")).unwrap();
        ok(&d, &[std, flag, &mf, "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"]);
        irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
    }
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");

    let mf = format!("-fmodule-file={name}=a.pcm");
    let mut errs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        let e = kiln(&d, &[std, flag, "-fsyntax-only", &mf, "err.cpp"]);
        assert_eq!(e.code, 1, "{}", e.stderr);
        errs.push(e.stderr);
    }
    assert_eq!(errs[0], errs[1], "diagnostics differ between the image and the replay");
    errs.swap_remove(0)
}

/// WP-1284 (§11.11 "Constraints"): concepts, constrained templates and
/// requires-expressions come from the image. With the unit's source
/// blanked an importer still checks satisfaction and runs them, its IR
/// is the replay's, and an unsatisfied constraint is explained with the
/// replay's notes, in the module ([temp.constr.constr]).
#[test]
fn kind2_declaration_image_constraints() {
    let errs = image_round_trip(
        "image-cons",
        "imgc",
        IMGC,
        IMGC_RUN,
        IMGC_ERR,
        "1 2 3 4 2 5 6 7 1 17 0 0 8 1 1 1\n",
    );
    for want in [
        "m.cppm:6:11: note: because 'Three' does not satisfy 'Small'",
        "m.cppm:3:36: note: because 'sizeof(Three) <= 2' (3 <= 2) evaluated to false",
        "m.cppm:12:7: note: candidate function not viable: constraints not satisfied",
    ] {
        assert!(errs.contains(want), "{want}: {errs}");
    }
}

/// WP-1284 (§11.11 "Dependent types", "Deduction guides"): dependent
/// types and names, member pointers, vectors and class template argument
/// deduction (implicit guides re-formed, explicit ones carried) come from
/// the image. An error inside an instantiated pattern sits at the written
/// component of the dependent name, as after a replay ([temp.dep]).
#[test]
fn kind2_declaration_image_dependent_types() {
    let errs = image_round_trip(
        "image-dep",
        "imgd",
        IMGD,
        IMGD_RUN,
        IMGD_ERR,
        "0 1 5 3 4 5 6 1 7 5 | 7 1 3 8 1 5 8\n",
    );
    for want in [
        "m.cppm:15:51: error: no member named 'inner' in 'B'",
        "m.cppm:14:53: error: no member named 'value' in 'C'",
        "note: implicit deduction guide declared as 'template <class T> Wrap(T *p) -> id::Wrap<T>'",
    ] {
        assert!(errs.contains(want), "{want}: {errs}");
    }
}

const IMGPI: &str =
    include_str!("../../../tests/conformance/module.interface/Inputs/img-pattern-inits.cppm");
const IMGPI_RUN: &str =
    include_str!("../../../tests/conformance/module.interface/image-pattern-inits-run.cpp");
const IMGPI_ERR: &str =
    include_str!("../../../tests/conformance/module.interface/image-pattern-inits-errors.cpp");

/// WP-1310 (§11.11 "Pattern mem-initializers"): the mem-initializers of
/// class template constructors, as parsed, and attributed locals come
/// from the image. With the unit's source blanked an importer still
/// instantiates them, its IR is the replay's, and a mem-initializer that
/// does not instantiate is diagnosed in the module, as after a replay
/// ([class.base.init], [temp.inst]/4).
#[test]
fn kind2_declaration_image_pattern_inits() {
    let errs = image_round_trip(
        "image-pi",
        "imgpi",
        IMGPI,
        IMGPI_RUN,
        IMGPI_ERR,
        "4 1 5 3 2 7 10 11 6 7\n",
    );
    for want in [
        "m.cppm:29:43: error: invalid operands to binary expression ('NC' and 'int')",
        "m.cppm:18:23: error: no matching conversion for functional-style cast from 'int' to 'NC'",
    ] {
        assert!(errs.contains(want), "{want}: {errs}");
    }
}
