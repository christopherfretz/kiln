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
    *text = text.chars().map(|ch| if ch == '\n' { '\n' } else { ' ' }).collect();
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
    *text = text.chars().map(|ch| if ch == '\n' { '\n' } else { ' ' }).collect();
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
    *text = text.chars().map(|ch| if ch == '\n' { '\n' } else { ' ' }).collect();
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

/// WP-1096 (§11.11 "Part 2a", "ODR merging"): names interned only as
/// module-name components do not count as seen, so a namespace named
/// like its module (plain or dotted) is built from the image; a real
/// earlier declaration of an image's name still replays, as do a second
/// image declaring the same name and a global module fragment's
/// declaration of it.
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
    w("c2.cpp", "int g;\nimport m;\nint main() { return m::f() - 40 + g; }\n");
    assert_eq!(image_notes(&d, &["-fsyntax-only", m, "c2.cpp"]), replay("m", "g"));
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
}

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

/// A unit with a node the image does not carry is written without one,
/// and `-v` names the first such node (§11.11 "Part 2a", "Fallback").
#[test]
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
            "export module f;\n#define ONE 1\nexport constexpr int g() { return ONE; }\n",
            "in a macro expansion",
        ),
        (
            "export module f;\nexport struct S { virtual int f() const { return 1; } };\nexport constexpr int k = 1;\n",
            "a polymorphic class",
        ),
        (
            "export module f;\nexport struct B {};\nexport struct D : virtual B {};\nexport constexpr int k = 1;\n",
            "a virtual base class",
        ),
        (
            "export module f;\nexport struct S { int x; friend int g(S s) { return s.x; } };\nexport constexpr int k = 1;\n",
            "a friend declaration",
        ),
        (
            "export module f;\nexport struct S { union { int a; float b; }; };\nexport constexpr int k = 1;\n",
            "an anonymous union or struct member",
        ),
        (
            "export module f;\nexport inline int *g() { return new int(1); }\nexport constexpr int k = 1;\n",
            "a new or delete expression",
        ),
        (
            "export module f;\nexport template <class T> struct B { T v; };\n\
             template <> struct B<int> { int w; };\n",
            "an explicit or partial specialization",
        ),
        (
            "export module f;\nexport template <int N = 1> struct B { int f() { return N; } };\n",
            "a default template argument of a non-type parameter",
        ),
        (
            "export module f;\nexport template <class T> struct B { T v; B(T t) : v(t) {} };\n",
            "mem-initializers of a constructor in a class template",
        ),
        (
            "export module f;\nexport template <class T> struct B { template <class U> U m(U u) { return u; } };\n",
            "a member template or a template in a template",
        ),
        (
            "export module f;\nexport template <int N> struct B { static constexpr int v = N; };\n\
             export inline int g() { return B<2>::v; }\n",
            "a template argument the image does not carry",
        ),
        (
            "export module f;\nexport template <int N> struct B { int a[N]; };\n",
            "a type the image does not carry",
        ),
        (
            "export module f;\nexport template <class T> struct A { int f() { return 1; } };\n\
             export inline int h() { A<int> a; return a.f(); }\n",
            "an implicit member of a class template specialization",
        ),
        ("export module f;\nexport template <class T> constexpr T v = T(1);\n", "a template"),
        (
            "export module f;\nexport template <class... T> int n(T...) { return sizeof...(T); }\n",
            "a template parameter pack",
        ),
        (
            "export module f;\nexport template <class T> requires (sizeof(T) > 1) T id(T t) { return t; }\n",
            "a constrained template",
        ),
        (
            "export module f;\nexport struct S { template <class T> T m(T t) { return t; } };\n",
            "a member function template",
        ),
        (
            "export module f;\nexport template <class T> T e(T t) { return t; }\n\
             template <> int e<int>(int t) { return t + 1; }\n",
            "a template specialization",
        ),
        (
            "export module f;\nexport template <class T> T d(T t);\n\
             export template <class T> T d(T t) { return t; }\n",
            "a redeclared template",
        ),
        (
            "export module f;\nexport template <class T> T g(T t) { return t; }\n\
             export template <class T> T h(T t) { return g(t); }\n",
            "a template expression",
        ),
        (
            "export module f;\nexport namespace n { using I = int; }\nexport using n::I;\n",
            "a using-declaration",
        ),
        ("module;\nint h();\nexport module f;\nexport int g();\n", "a global module fragment"),
        (
            "export module f;\nstruct S { int x; bool operator==(const S &) const = default; };\nexport constexpr S v{3};\n",
            "a defaulted comparison",
        ),
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
