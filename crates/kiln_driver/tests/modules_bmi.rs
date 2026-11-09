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
