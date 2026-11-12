//! Assembly inputs (`spec/driver.md` "Assembler inputs", `spec/pp.md`
//! "Assembler-with-cpp", WP-1331): `.s` is assembled, `.S` preprocessed
//! by kiln in assembler-with-cpp mode and then assembled, through every
//! phase, as clang does. Never an input dropped with exit status 0. The
//! assembly is target-neutral (directives and data symbols only), so the
//! tests run on aarch64 and x86-64 alike; those that assemble skip
//! without `llc-19` / `clang++-19`.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// A `.s`: one global data symbol.
const PLAIN: &str = "\t.data\n\t.globl plain_sym\nplain_sym:\n\t.long 42\n";

/// A `.S`: a `#` comment line (no directive), `__ASSEMBLER__`, a `'` in
/// a string, a `-D` macro and one from an included header.
const CPP: &str = "#include \"val.h\"\n\
                   # a comment, not a directive\n\
                   #define GLOBAL(n) .globl n; n:\n\
                   #ifndef __ASSEMBLER__\n\
                   #error not assembler-with-cpp\n\
                   #endif\n\
                   \t.data\n\
                   GLOBAL(cpp_sym)\n\
                   \t.long VAL + EXTRA // folded by the assembler\n\
                   \t.ascii \"it's\"\n";

const MAIN: &str = "extern int plain_sym, cpp_sym;\n\
                    int main(void) { return plain_sym + cpp_sym; }\n";

fn kiln() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_kiln"))
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-asm-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("plain.s"), PLAIN).unwrap();
    std::fs::write(d.join("cpp.S"), CPP).unwrap();
    std::fs::write(d.join("val.h"), "#define EXTRA 1\n").unwrap();
    std::fs::write(d.join("main.c"), MAIN).unwrap();
    d
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn tools() -> bool {
    let ok = have("llc-19") && have("clang++-19");
    if !ok {
        eprintln!("skipping: llc-19 / clang++-19 not found");
    }
    ok
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(kiln()).args(args).current_dir(dir).output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn is_object(p: &Path) -> bool {
    std::fs::read(p).is_ok_and(|b| b.starts_with(b"\x7fELF"))
}

/// `-c` assembles a `.s` and a `.S` beside a `.c`, each to its object;
/// one command compiles, assembles and links all three; `-c -o -` writes
/// the object to standard output.
#[test]
fn compile_and_link() {
    if !tools() {
        return;
    }
    let d = scratch("link");
    let o = run(&d, &["-c", "plain.s", "cpp.S", "main.c", "-DVAL=7"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    for f in ["plain.o", "cpp.o", "main.o"] {
        assert!(is_object(&d.join(f)), "{f}");
    }
    let o = run(&d, &["-c", "cpp.S", "-DVAL=7", "-o", "named.o"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    assert!(is_object(&d.join("named.o")));
    let o = run(&d, &["-c", "plain.s", "-o", "-"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(o.stdout.starts_with(b"\x7fELF"));
    // 42 + (7 + 1): the `.S` symbol's value comes from `-D` and the
    // header.
    let o = run(&d, &["main.c", "plain.s", "cpp.S", "-DVAL=7", "-o", "prog"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    let r = Command::new(d.join("prog")).output().unwrap();
    assert_eq!(r.status.code(), Some(50));
    // The link leaves no object behind.
    std::fs::remove_file(d.join("plain.o")).unwrap();
    let o = run(&d, &["main.c", "plain.s", "cpp.S", "-DVAL=0", "-o", "prog2"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(!d.join("plain.o").exists());
    // `-x assembler-with-cpp` / `-x assembler` name the type of any file.
    std::fs::copy(d.join("cpp.S"), d.join("cpp.txt")).unwrap();
    let o = run(&d, &["-c", "-x", "assembler-with-cpp", "cpp.txt", "-DVAL=1", "-o", "x.o"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    assert!(is_object(&d.join("x.o")));
    let o = run(&d, &["-c", "-x", "assembler", "plain.s", "-o", "y.o"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    assert!(is_object(&d.join("y.o")));
    // `-MD` on a `.S` writes its rule, the header included.
    let o = run(&d, &["-c", "-MD", "cpp.S", "-DVAL=1", "-o", "dep.o"]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(std::fs::read_to_string(d.join("dep.d")).unwrap(), "dep.o: cpp.S val.h\n");
}

/// clang's phases: `-E`, `-S` and `-fsyntax-only` write a `.S`'s
/// preprocessed text (checked against clang 19 and 22's `-E -P`); on a
/// `.s` they and `-M` only warn `'assembler' input unused`.
#[test]
fn phases() {
    let d = scratch("phases");
    let pre = " .data\n.globl cpp_sym; cpp_sym:\n .long 3 + 1\n .ascii \"it's\"\n";
    let o = run(&d, &["-E", "-P", "cpp.S", "-DVAL=3"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    assert_eq!(text(&o.stdout), format!("# a comment, not a directive\n{pre}"));
    // A `#` before a non-parameter is kept, `$` is no identifier
    // character (`$VAL` expands `VAL`), an unterminated `'` is text.
    std::fs::write(d.join("hash.S"), "#define IMM(x) #y x\nIMM(1) $VAL 'b\n").unwrap();
    let o = run(&d, &["-E", "-P", "hash.S", "-DVAL=3"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    assert_eq!(text(&o.stdout), "\n#y 1 $3 'b\n");
    let o = run(&d, &["-S", "-P", "cpp.S", "-DVAL=3"]);
    assert_eq!((o.status.code(), text(&o.stdout).contains(".long 3 + 1")), (Some(0), true));
    let o = run(&d, &["-fsyntax-only", "-P", "cpp.S", "-DVAL=3", "-o", "pre.s"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    assert!(std::fs::read_to_string(d.join("pre.s")).unwrap().contains(".long 3 + 1"));
    let o = run(&d, &["-M", "cpp.S"]);
    assert_eq!(text(&o.stdout), "cpp.o: cpp.S val.h\n");
    let o = run(&d, &["-dM", "-E", "cpp.S"]);
    assert!(text(&o.stdout).contains("#define __ASSEMBLER__ 1\n"));
    for phase in [&["-E"][..], &["-fsyntax-only"], &["-S"], &["-M"], &["-S", "-emit-llvm"]] {
        let mut args = phase.to_vec();
        args.push("plain.s");
        let o = run(&d, &args);
        assert_eq!(
            (o.status.code(), text(&o.stdout).as_str(), text(&o.stderr).as_str()),
            (
                Some(0),
                "",
                "kiln: warning: plain.s: 'assembler' input unused \
                 [-Wunused-command-line-argument]\n"
            ),
            "{phase:?}"
        );
        assert!(!d.join("plain.o").exists() && !d.join("plain.ll").exists());
    }
}

/// A failing assembly or preprocessing is an error with no object, and
/// the link does not run; bad `-Wa,` values are rejected before.
#[test]
fn errors() {
    if !tools() {
        return;
    }
    let d = scratch("errors");
    std::fs::write(d.join("bad.s"), "\t.long\n\t.bogus_directive 1\n").unwrap();
    let o = run(&d, &["-c", "bad.s"]);
    let err = text(&o.stderr);
    assert_eq!(o.status.code(), Some(1));
    assert!(err.contains("bad.s:2:"), "{err}");
    assert!(
        err.ends_with(
            "kiln: error: assembler command failed with exit code 1 (use -v to see invocation)\n"
        ),
        "{err}"
    );
    assert!(!d.join("bad.o").exists());
    // The preprocessed text's line markers point the assembler at the `.S`.
    std::fs::write(d.join("bad2.S"), "#define X 1\n\t.data\n\t.bogus_directive X\n").unwrap();
    let o = run(&d, &["-c", "bad2.S"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(text(&o.stderr).contains("bad2.S:3:"), "{}", text(&o.stderr));
    std::fs::write(d.join("err.S"), "#error stop\n").unwrap();
    let o = run(&d, &["main.c", "plain.s", "err.S", "-o", "prog"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(text(&o.stderr).contains("err.S:1:2: error: stop"), "{}", text(&o.stderr));
    assert!(!d.join("prog").exists() && !d.join("err.o").exists());
    let o = run(&d, &["-c", "plain.s", "-Wa,--bogus"]);
    assert_eq!(
        (o.status.code(), text(&o.stderr).as_str()),
        (Some(1), "kiln: error: unsupported argument '--bogus' to option '-Wa,'\n")
    );
    assert!(!d.join("plain.o").exists());
    // Two outputs with one `-o`.
    let o = run(&d, &["-c", "plain.s", "cpp.S", "-o", "one.o"]);
    assert_eq!(
        (o.status.code(), text(&o.stderr).as_str()),
        (Some(1), "kiln: error: cannot specify -o when generating multiple output files\n")
    );
}

/// `-###` shows the steps: the in-process preprocessor, then the
/// assembler with the target, `-I`, `-g`, and the `-Wa,` / `-Xassembler`
/// options as written. Without an LLVM installation assembling is the
/// missing-installation error, never a silent success.
#[test]
fn commands_and_missing_tools() {
    let d = scratch("hash");
    let o = run(
        &d,
        &[
            "-###",
            "-c",
            "-g",
            "-Iinc",
            "-Wa,--noexecstack",
            "-Xassembler",
            "--fatal-warnings",
            "cpp.S",
            "-o",
            "c.o",
        ],
    );
    let err = text(&o.stderr);
    assert_eq!(o.status.code(), Some(0), "{err}");
    assert!(err.contains(" (in-process) preprocessor \"cpp.S\" -> \""), "{err}");
    let asm = err.lines().find(|l| l.contains("\"assembler\"")).unwrap_or_default();
    for a in [
        "clang++\"",
        "\"-Iinc\"",
        "\"-g\" \"-gdwarf-5\"",
        "\"-Wa,--noexecstack\" \"-Xassembler\" \"--fatal-warnings\"",
        "\"-c\" \"-x\" \"assembler\"",
        "\"-o\" \"c.o\"",
    ] {
        assert!(asm.contains(a), "{a} in {asm}");
    }
    assert!(asm.contains("\"--target="), "{asm}");
    let o = Command::new(kiln())
        .args(["-c", "plain.s"])
        .current_dir(&d)
        .env("KILN_LLVM_DIR", d.join("no-llvm"))
        .env("PATH", d.join("no-bin"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(text(&o.stderr).contains("is not a usable LLVM installation"), "{}", text(&o.stderr));
    assert!(!d.join("plain.o").exists());
}
