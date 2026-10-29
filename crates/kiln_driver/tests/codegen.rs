//! End-to-end tests of the driver's codegen path (`spec/driver.md` "Code
//! generation"): `-S -emit-llvm`, `-S`, `-c`, linking, `-###`, `-v`,
//! output naming, the unused-input warnings and the external-command
//! diagnostics. The `kiln` binary runs in a scratch directory; tests that
//! need `llc-19` / `clang++-19` skip without them.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const MAIN: &str = "extern \"C\" int printf(const char*, ...);\n\
                    int add(int, int);\n\
                    int main() { printf(\"sum %d\\n\", add(2, 3)); return 4; }\n";
const ADD: &str = "int add(int a, int b) { return a + b; }\n";

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-codegen-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
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

fn kiln_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kiln")).args(args).current_dir(dir).output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn setup(name: &str) -> PathBuf {
    let d = scratch(name);
    std::fs::write(d.join("main.cpp"), MAIN).unwrap();
    std::fs::write(d.join("add.cpp"), ADD).unwrap();
    d
}

#[test]
fn emit_llvm_text() {
    let d = setup("ll");
    let o = kiln_in(&d, &["-S", "-emit-llvm", "add.cpp"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    let ll = std::fs::read_to_string(d.join("add.ll")).unwrap();
    assert!(ll.contains("define dso_local noundef i32 @_Z3addii(i32 noundef %0, i32 noundef %1)"));
    assert!(ll.contains("!{!\"kiln version "), "{ll}");
    let o = kiln_in(
        &d,
        &["-S", "-emit-llvm", "-o", "-", "--target=x86_64-unknown-linux-gnu", "add.cpp"],
    );
    assert!(text(&o.stdout).contains("target triple = \"x86_64-unknown-linux-gnu\""));
    // Errors: no output file, exit 1.
    std::fs::write(d.join("bad.cpp"), "int x = ;\n").unwrap();
    let o = kiln_in(&d, &["-S", "-emit-llvm", "bad.cpp"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(text(&o.stderr).ends_with("1 error generated.\n"));
    assert!(!d.join("bad.ll").exists());
}

#[test]
fn compile_only_outputs() {
    if !tools() {
        return;
    }
    let d = setup("c");
    let o = kiln_in(&d, &["-c", "add.cpp", "main.cpp"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    for f in ["add.o", "main.o"] {
        let b = std::fs::read(d.join(f)).unwrap();
        assert_eq!(&b[..4], b"\x7fELF", "{f}");
    }
    let o = kiln_in(&d, &["-S", "add.cpp", "-o", "x.s"]);
    assert_eq!(o.status.code(), Some(0));
    let s = std::fs::read_to_string(d.join("x.s")).unwrap();
    assert!(s.contains("_Z3addii:"), "{s}");
    // `-S` wins over `-c`.
    let o = kiln_in(&d, &["-c", "-S", "add.cpp", "-o", "-"]);
    assert!(text(&o.stdout).contains("_Z3addii:"));
    // Link the objects made above.
    let o = kiln_in(&d, &["add.o", "main.o", "-o", "prog"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    let r = Command::new(d.join("prog")).output().unwrap();
    assert_eq!((r.status.code(), text(&r.stdout).as_str()), (Some(4), "sum 5\n"));
}

#[test]
fn compile_and_link() {
    if !tools() {
        return;
    }
    let d = setup("link");
    let o = kiln_in(&d, &["main.cpp", "add.cpp", "-O2", "-g", "-lm"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    let r = Command::new(d.join("a.out")).output().unwrap();
    assert_eq!((r.status.code(), text(&r.stdout).as_str()), (Some(4), "sum 5\n"));
    // No object files are left behind in the current directory.
    assert!(!d.join("main.o").exists());
    // A link failure is a driver error.
    let o = kiln_in(&d, &["main.cpp", "-o", "p2"]);
    assert_eq!(o.status.code(), Some(1));
    let err = text(&o.stderr);
    assert!(err.contains("undefined reference to `add(int, int)'"), "{err}");
    assert!(
        err.ends_with(
            "kiln: error: linker command failed with exit code 1 (use -v to see invocation)\n"
        ),
        "{err}"
    );
}

#[test]
fn driver_errors_and_warnings() {
    let d = setup("diag");
    std::fs::write(d.join("lib.o"), "").unwrap();
    let o = kiln_in(&d, &["-c", "add.cpp", "main.cpp", "-o", "x.o"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        text(&o.stderr),
        "kiln: error: cannot specify -o when generating multiple output files\n"
    );
    let o = kiln_in(&d, &["-emit-llvm", "add.cpp"]);
    assert_eq!(text(&o.stderr), "kiln: error: -emit-llvm cannot be used when linking\n");
    let args = [
        "-S",
        "-emit-llvm",
        "-lm",
        "add.cpp",
        "-L/x",
        "lib.o",
        "-Wl,-z,now",
        "-fuse-ld=lld",
        "-static",
    ];
    let o = kiln_in(&d, &args);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(
        text(&o.stderr),
        "kiln: warning: -lm: 'linker' input unused [-Wunused-command-line-argument]\n\
         kiln: warning: argument unused during compilation: '-L/x' [-Wunused-command-line-argument]\n\
         kiln: warning: lib.o: 'linker' input unused [-Wunused-command-line-argument]\n\
         kiln: warning: -Wl,-z,now: 'linker' input unused [-Wunused-command-line-argument]\n\
         kiln: warning: argument unused during compilation: '-fuse-ld=lld' [-Wunused-command-line-argument]\n"
    );
    let o =
        kiln_in(&d, &["-S", "-emit-llvm", "-Wno-unused-command-line-argument", "-lm", "add.cpp"]);
    assert_eq!(text(&o.stderr), "");
    let o = kiln_in(&d, &["-S", "-emit-llvm", "-Werror", "-lm", "add.cpp"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        text(&o.stderr),
        "kiln: error: -lm: 'linker' input unused [-Werror,-Wunused-command-line-argument]\n"
    );
    std::fs::write(d.join("c.c"), "int f(void) { return 0; }\n").unwrap();
    let o = kiln_in(&d, &["-S", "-emit-llvm", "c.c"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(text(&o.stderr), "error: compiling C is not implemented yet\n");
}

#[test]
fn print_commands() {
    let d = setup("hash");
    std::fs::write(d.join("add.o"), "").unwrap();
    let o = kiln_in(&d, &["-###", "main.cpp", "add.o", "-lm", "-O2"]);
    assert_eq!(o.status.code(), Some(0));
    let err = text(&o.stderr);
    assert!(err.starts_with("kiln version "), "{err}");
    assert!(err.contains(" (in-process) frontend \"main.cpp\" -> \"|\"\n"), "{err}");
    assert!(err.contains("\"-O2\" \"-relocation-model=pic\" \"-filetype=obj\" \"-o\""), "{err}");
    assert!(err.contains("-main.o\" \"add.o\" \"-lm\" \"-o\" \"a.out\"\n"), "{err}");
    assert!(!d.join("a.out").exists());
    let o = kiln_in(&d, &["-###", "-S", "-fno-pie", "main.cpp"]);
    let err = text(&o.stderr);
    assert!(
        err.contains("\"-O0\" \"-relocation-model=static\" \"-o\" \"main.s\" \"-\"\n"),
        "{err}"
    );
    let o = kiln_in(&d, &["-###", "-c", "-emit-llvm", "main.cpp"]);
    assert!(text(&o.stderr).contains("\"-o\" \"main.bc\" \"-\"\n"));
}

#[test]
fn verbose_prints_commands() {
    if !tools() {
        return;
    }
    let d = setup("v");
    let o = kiln_in(&d, &["-v", "-c", "add.cpp"]);
    assert_eq!(o.status.code(), Some(0));
    let err = text(&o.stderr);
    assert!(err.contains(" -O0 -relocation-model=pic -filetype=obj -o add.o -\n"), "{err}");
}

#[test]
fn missing_tools() {
    let d = setup("missing");
    let o = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(["-c", "add.cpp"])
        .current_dir(&d)
        .env("KILN_LLVM_DIR", d.join("no-llvm"))
        .env("PATH", d.join("no-bin"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        text(&o.stderr),
        "kiln: error: unable to execute command: Executable \"llc-19\" doesn't exist!\n"
    );
    assert!(!d.join("add.o").exists());
}
