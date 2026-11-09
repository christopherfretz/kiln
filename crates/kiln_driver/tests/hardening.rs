//! WP-813: the hardening and instrumentation attributes and flags end to
//! end (spec/driver.md "Codegen flags", spec/landed/codegen/WP-813.md).
//! The objects and assembly kiln++ produces for both triples are compared
//! with clang++-19's: the `patchable_function_entry` NOP sled through
//! `llvm-objdump-19`, `no_stack_protector` under `-fstack-protector-all`
//! (no canary) and `zero_call_used_regs` (the zeroing before `ret`). The
//! flags' target errors follow clang 19's driver and `-cc1`. Tests that
//! need `llc-19` / `clang++-19` / `llvm-objdump-19` skip without them.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const TRIPLES: [&str; 2] = ["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"];

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-hardening-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn tools(extra: &[&str]) -> bool {
    let ok = ["llc-19", "clang++-19"].iter().chain(extra).all(|t| have(t));
    if !ok {
        eprintln!("skipping: llc-19 / clang++-19 / {extra:?} not found");
    }
    ok
}

fn run(prog: &Path, dir: &Path, args: &[&str]) -> Output {
    Command::new(prog).args(args).current_dir(dir).output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// Compiles `src` with kiln++ (`-fclang-compat=19`) and clang++-19 with
/// `args`; returns both outputs' paths (kiln's, clang's).
fn both(d: &Path, src: &str, out: &str, args: &[&str]) -> (PathBuf, PathBuf) {
    let kiln_out = d.join(format!("k-{out}"));
    let clang_out = d.join(format!("c-{out}"));
    for (prog, path, compat) in [
        (kilnpp(), &kiln_out, Some("-fclang-compat=19")),
        (PathBuf::from("clang++-19"), &clang_out, None),
    ] {
        let mut cmd: Vec<&str> = args.to_vec();
        cmd.extend(compat);
        cmd.extend([src, "-o", path.to_str().unwrap()]);
        let r = run(&prog, d, &cmd);
        let shown = prog.display();
        assert_eq!((r.status.code(), text(&r.stderr).as_str()), (Some(0), ""), "{shown} {cmd:?}");
    }
    (kiln_out, clang_out)
}

/// The instruction lines of function `name` in assembly `s`, comments and
/// directives dropped.
fn body(s: &str, name: &str) -> Vec<String> {
    let start = format!("{name}:");
    let lines = s.lines().skip_while(|l| !l.starts_with(&start)).skip(1);
    lines
        .take_while(|l| !l.starts_with(".Lfunc_end"))
        .map(|l| {
            let l = l.split(" //").next().unwrap_or(l);
            // x86 comments; not aarch64 immediates (`, #0`).
            l.split("  #").next().unwrap_or(l).trim_end().to_owned()
        })
        .filter(|l| !l.is_empty() && !l.trim_start().starts_with('.') && !l.starts_with("//"))
        .filter(|l| !l.starts_with('#'))
        .collect()
}

/// `patchable_function_entry(2, 1)` and `-fpatchable-function-entry=3,2`:
/// the disassembly (NOPs before and after the symbol) is clang 19's, and
/// the entry is recorded in `__patchable_function_entries`.
#[test]
fn patchable_nop_sled() {
    if !tools(&["llvm-objdump-19"]) {
        return;
    }
    let d = scratch("pfe");
    std::fs::write(
        d.join("p.cpp"),
        "__attribute__((patchable_function_entry(2, 1))) int sled(int x) { return x + 1; }\n\
         __attribute__((patchable_function_entry(0))) int none(int x) { return x; }\n\
         int dflt(int x) { return x * 2; }\n",
    )
    .unwrap();
    let objdump = |o: &Path, what: &str| {
        let r = run(
            Path::new("llvm-objdump-19"),
            &d,
            &[what, "--no-show-raw-insn", o.to_str().unwrap()],
        );
        // The first lines name the file.
        text(&r.stdout).lines().skip(2).collect::<Vec<_>>().join("\n")
    };
    for t in TRIPLES {
        for flag in [None, Some("-fpatchable-function-entry=3,2")] {
            let target = format!("--target={t}");
            let mut args = vec![target.as_str(), "-c"];
            args.extend(flag);
            let (k, c) = both(&d, "p.cpp", "p.o", &args);
            let dis = objdump(&k, "-d");
            assert_eq!(dis, objdump(&c, "-d"), "{t} {flag:?}");
            // One NOP before `sled` (its own `.Ltmp` label), one at its entry.
            let sled = dis.split("<_Z4sledi>:").nth(1).expect(&dis);
            assert!(sled.lines().nth(1).is_some_and(|l| l.trim_end().ends_with("nop")), "{dis}");
            // The section (its size: one entry per patchable function).
            let section = |o: &Path| {
                let h = objdump(o, "-h");
                h.lines().find(|l| l.contains(" __patchable_function_entries ")).map(str::to_owned)
            };
            assert!(section(&k).is_some(), "{t} {flag:?}");
            assert_eq!(section(&k), section(&c), "{t} {flag:?}");
        }
    }
}

/// WP-961: `-mbranches-within-32B-boundaries` (x86; projects add it for
/// Intel's JCC erratum, `daw_json_link` among them) pads the object's
/// branches as clang 19's does: `llc -x86-branches-within-32B-boundaries`.
#[test]
fn branches_within_32b_boundaries() {
    if !tools(&["llvm-objdump-19"]) {
        return;
    }
    let d = scratch("jcc");
    std::fs::write(
        d.join("b.cpp"),
        "int classify(const int *p, int n) {\n\
           int r = 0;\n\
           for (int i = 0; i < n; ++i) {\n\
             if (p[i] > 7) r += 3;\n\
             else if (p[i] < -5) r -= 2;\n\
             else if (p[i] == 2) r ^= 9;\n\
             else if (p[i] & 1) r += p[i];\n\
             else r -= 1;\n\
           }\n\
           return r;\n\
         }\n",
    )
    .unwrap();
    let objdump = |o: &Path| {
        let r = run(
            Path::new("llvm-objdump-19"),
            &d,
            &["-d", "--no-show-raw-insn", o.to_str().unwrap()],
        );
        text(&r.stdout).lines().skip(2).collect::<Vec<_>>().join("\n")
    };
    let x86 = "--target=x86_64-unknown-linux-gnu";
    let (plain, _) = both(&d, "b.cpp", "plain.o", &[x86, "-O2", "-c"]);
    let (k, c) =
        both(&d, "b.cpp", "jcc.o", &[x86, "-O2", "-c", "-mbranches-within-32B-boundaries"]);
    let dis = objdump(&k);
    assert_eq!(dis, objdump(&c));
    // The padding is there: the same function, longer.
    assert_ne!(dis, objdump(&plain));
    assert!(dis.matches("nop").count() > objdump(&plain).matches("nop").count(), "{dis}");
}

/// `no_stack_protector` under `-fstack-protector-all`: the guarded
/// function checks the canary, the opted-out ones do not; each body is
/// clang 19's.
#[test]
fn no_stack_protector_has_no_canary() {
    if !tools(&[]) {
        return;
    }
    let d = scratch("nsp");
    std::fs::write(
        d.join("s.cpp"),
        "void use(char*);\n\
         void guarded() { char b[16]; use(b); }\n\
         __attribute__((no_stack_protector)) void bare() { char b[16]; use(b); }\n\
         [[gnu::no_stack_protector]] void bare2() { char b[16]; use(b); }\n",
    )
    .unwrap();
    for t in TRIPLES {
        let target = format!("--target={t}");
        let (k, c) = both(&d, "s.cpp", "s.s", &[&target, "-S", "-fstack-protector-all"]);
        let (k, c) = (std::fs::read_to_string(k).unwrap(), std::fs::read_to_string(c).unwrap());
        let canary =
            |b: &[String]| b.iter().any(|l| l.contains("__stack_chk") || l.contains("%fs:40"));
        for f in ["_Z7guardedv", "_Z4barev", "_Z5bare2v"] {
            let b = body(&k, f);
            assert_eq!(b, body(&c, f), "{t} {f}");
            assert_eq!(canary(&b), f == "_Z7guardedv", "{t} {f}: {b:?}");
        }
    }
}

/// `zero_call_used_regs`: the used (or all) registers are zeroed before
/// `ret`, as clang 19 does, and `-fzero-call-used-regs=` sets the default.
#[test]
fn zero_call_used_regs_asm() {
    if !tools(&[]) {
        return;
    }
    let d = scratch("zcur");
    std::fs::write(
        d.join("z.cpp"),
        "__attribute__((zero_call_used_regs(\"used-gpr\"))) int z(int a, int b) { return a * b; }\n\
         __attribute__((zero_call_used_regs(\"all\"))) int za(int a, int b) { return a + b; }\n\
         __attribute__((zero_call_used_regs(\"skip\"))) int zs(int a, int b) { return a - b; }\n\
         int plain(int a, int b) { return a ^ b; }\n",
    )
    .unwrap();
    for t in TRIPLES {
        let target = format!("--target={t}");
        for flag in [None, Some("-fzero-call-used-regs=used-gpr-arg")] {
            let mut args = vec![target.as_str(), "-S"];
            args.extend(flag);
            let (k, c) = both(&d, "z.cpp", "z.s", &args);
            let (k, c) = (std::fs::read_to_string(k).unwrap(), std::fs::read_to_string(c).unwrap());
            for f in ["_Z1zii", "_Z2zaii", "_Z2zsii", "_Z5plainii"] {
                assert_eq!(body(&k, f), body(&c, f), "{t} {flag:?} {f}");
            }
            let zeroes = |f: &str| {
                let b = body(&k, f);
                let ret = b.iter().position(|l| l.contains("ret")).expect(f);
                // The zeroing sits just before the return.
                b[..ret]
                    .iter()
                    .rev()
                    .take_while(|l| l.ends_with(", #0") || l.contains("xor") || l.contains("movi"))
                    .count()
            };
            let n = (zeroes("_Z1zii"), zeroes("_Z2zaii"));
            assert!(n.0 > 0 && n.1 > n.0, "{t} {n:?} {:?}", body(&k, "_Z1zii"));
            assert_eq!(zeroes("_Z2zsii"), 0, "{t}");
            assert_eq!(zeroes("_Z5plainii") > 0, flag.is_some(), "{t} {flag:?}");
        }
    }
}

/// The flags' errors, as clang 19 prints them: the target's `-cc1`
/// errors and the driver's own; no output is written.
#[test]
fn flag_errors() {
    let d = scratch("errs");
    std::fs::write(d.join("m.cpp"), "int main(){}\n").unwrap();
    let a64 = "--target=aarch64-unknown-linux-gnu";
    let x86 = "--target=x86_64-unknown-linux-gnu";
    let cases: &[(&[&str], &str)] = &[
        (
            &[a64, "-fcf-protection"],
            "error: option 'cf-protection=return' cannot be specified on this target\n\
             error: option 'cf-protection=branch' cannot be specified on this target\n\
             2 errors generated.\n",
        ),
        (
            &[a64, "-fcf-protection=branch"],
            "error: option 'cf-protection=branch' cannot be specified on this target\n\
             1 error generated.\n",
        ),
        (
            &[a64, "-fcf-protection=bogus"],
            "error: invalid value 'bogus' in '-fcf-protection=bogus'\n",
        ),
        (
            &[a64, "-mfunction-return=thunk-extern"],
            "error: invalid argument '-mfunction-return=' not allowed with 'aarch64-unknown-linux-gnu'\n",
        ),
        (
            &[a64, "-fsyntax-only", "-mfunction-return=keep"],
            "error: invalid argument '-mfunction-return=' not allowed with 'aarch64-unknown-linux-gnu'\n",
        ),
        (
            &[x86, "-mfunction-return=bogus"],
            "error: invalid value 'bogus' in '-mfunction-return=bogus'\n\
             error: invalid value 'bogus' in '-mfunction-return=bogus'\n",
        ),
        (
            &[a64, "-fcf-protection=bogus", "-mfunction-return=bogus", "-fzero-call-used-regs=bad"],
            "error: invalid value 'bogus' in '-mfunction-return=bogus'\n\
             error: invalid value 'bad' in '-fzero-call-used-regs=bad'\n\
             error: invalid value 'bogus' in '-fcf-protection=bogus'\n\
             error: invalid argument '-mfunction-return=' not allowed with 'aarch64-unknown-linux-gnu'\n",
        ),
        (
            &[a64, "-mbranches-within-32B-boundaries"],
            "kiln++: error: unsupported option '-mbranches-within-32B-boundaries' for target 'aarch64-unknown-linux-gnu'\n",
        ),
        (
            &["-fpatchable-function-entry=x"],
            "kiln++: error: invalid argument 'x' to -fpatchable-function-entry=\n",
        ),
        (
            &["-fpatchable-function-entry=1,2"],
            "kiln++: error: the second argument of '-fpatchable-function-entry' must be smaller than the first argument\n",
        ),
        (
            &["-fxray-instrument", "-fpatchable-function-entry=1"],
            "kiln++: error: invalid argument '-fxray-instrument' not allowed with '-fpatchable-function-entry='\n",
        ),
        (
            &["-fxray-instrument", "-fxray-instruction-threshold=abc"],
            "kiln++: error: invalid value 'abc' in '-fxray-instruction-threshold=abc'\n",
        ),
        (
            &["-fxray-instrument", "-fxray-modes=xray-basic"],
            "kiln++: error: '-fxray-modes=xray-basic' is not supported by kiln\n",
        ),
    ];
    for (flags, want) in cases {
        let mut args = flags.to_vec();
        args.extend(["-c", "m.cpp", "-o", "e.o"]);
        let o = run(&kilnpp(), &d, &args);
        assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(1), *want), "{flags:?}");
        assert!(!d.join("e.o").exists(), "{flags:?}");
    }
    // XRay's options without `-fxray-instrument` are unused, not errors.
    let o = run(
        &kilnpp(),
        &d,
        &["-fsyntax-only", "-fxray-instruction-threshold=abc", "-fxray-ignore-loops", "m.cpp"],
    );
    assert_eq!(
        (o.status.code(), text(&o.stderr).as_str()),
        (
            Some(0),
            "kiln++: warning: argument unused during compilation: '-fxray-instruction-threshold=abc' [-Wunused-command-line-argument]\n\
             kiln++: warning: argument unused during compilation: '-fxray-ignore-loops' [-Wunused-command-line-argument]\n"
        )
    );
}

/// The instrumentation flags reach the IR as clang 19's attributes, and
/// the `-O0` pipeline turns `-finstrument-functions` into calls.
#[test]
fn instrumentation_ir() {
    if !tools(&[]) {
        return;
    }
    let d = scratch("instr");
    std::fs::write(d.join("i.cpp"), "int f(int x) { return x; }\n").unwrap();
    for (flag, want) in [
        ("-finstrument-functions", "call void @__cyg_profile_func_enter("),
        (
            "-finstrument-function-entry-bare",
            "\"instrument-function-entry-inlined\"=\"__cyg_profile_func_enter_bare\"",
        ),
        ("-finstrument-functions-after-inlining", "\"instrument-function-entry-inlined\"="),
        ("-fxray-instrument", "\"xray-instruction-threshold\"=\"200\""),
        ("-fsplit-stack", "\"split-stack\""),
        ("-mspeculative-load-hardening", "speculative_load_hardening"),
    ] {
        let (k, c) = both(&d, "i.cpp", "i.ll", &["-S", "-emit-llvm", flag]);
        let k = std::fs::read_to_string(k).unwrap();
        assert!(k.contains(want), "{flag}: {k}");
        let c = std::fs::read_to_string(c).unwrap();
        let defs = |s: &str| {
            s.lines()
                .filter(|l| l.starts_with("define ") || l.starts_with("attributes "))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        assert_eq!(defs(&k), defs(&c), "{flag}");
    }
}

/// WP-846: `-mbranch-protection=` / `-msign-return-address=` are checked as
/// clang 19's driver checks them (`CollectARMPACBTIOptions`): the last of
/// the two wins on `AArch64`, a bad item is an unsupported argument (`<empty>`
/// for an empty one), x86 rejects `-mbranch-protection=` and ignores
/// `-msign-return-address=` as unused. Compared with clang++-19's stderr,
/// the program name dropped.
#[test]
fn branch_protection_flags() {
    if !have("clang++-19") {
        eprintln!("skipping: clang++-19 not found");
        return;
    }
    let d = scratch("bp");
    std::fs::write(d.join("m.cpp"), "int main(){}\n").unwrap();
    let a64 = "--target=aarch64-unknown-linux-gnu";
    let x86 = "--target=x86_64-unknown-linux-gnu";
    let cases: &[&[&str]] = &[
        &[a64, "-mbranch-protection=x"],
        &[a64, "-mbranch-protection=bti+"],
        &[a64, "-mbranch-protection=pac-ret+leaf+bogus"],
        &[a64, "-msign-return-address=bad"],
        &[a64, "-mbranch-protection=x", "-msign-return-address=all"],
        &[a64, "-msign-return-address=all", "-mbranch-protection=standard"],
        &[x86, "-mbranch-protection=bti"],
        &[x86, "-fsyntax-only", "-mbranch-protection=x"],
        &[x86, "-msign-return-address=all"],
    ];
    let strip = |s: String| s.replace("clang++-19: ", "").replace("kiln++: ", "");
    for flags in cases {
        let mut args = flags.to_vec();
        args.extend(["-c", "m.cpp", "-o", "e.o"]);
        let k = run(&kilnpp(), &d, &args);
        let c = run(Path::new("clang++-19"), &d, &args);
        assert_eq!(
            (k.status.code(), strip(text(&k.stderr))),
            (c.status.code(), strip(text(&c.stderr))),
            "{flags:?}"
        );
    }
}
