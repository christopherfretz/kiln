//! End-to-end driver tests: the command line, exit codes, diagnostics,
//! `-v` against clang's search lists (`resource/include-paths`), `-M*`.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use kiln_diag::SharedBuffer;
use kiln_driver::{Io, SharedWriter, run};

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn kiln(args: &[&str], stdin: &str) -> Out {
    let out = SharedBuffer::new();
    let err = SharedBuffer::new();
    let io = Io {
        stdout: Box::new(out.clone()),
        stderr: SharedWriter::new(err.clone()),
        stdin: Box::new(std::io::Cursor::new(stdin.as_bytes().to_vec())),
    };
    let owned: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let code = run(&owned, io);
    Out { code, stdout: out.contents(), stderr: err.contents() }
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-driver-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn version_and_no_inputs() {
    let o = kiln(&["--version"], "");
    assert_eq!(o.code, 0);
    assert!(o.stdout.starts_with("kiln version "), "{}", o.stdout);
    let o = kiln(&[], "");
    assert_eq!((o.code, o.stderr.as_str()), (1, "kiln: error: no input files\n"));
}

#[test]
fn argument_errors() {
    let o = kiln(&["-E", "-fbogus", "-x", "pascal", "-stdlib=libfoo", "/nonexistent.cpp"], "");
    assert_eq!(o.code, 1);
    assert_eq!(
        o.stderr,
        "kiln: error: unknown argument: '-fbogus'\n\
         kiln: error: language not recognized: 'pascal'\n\
         kiln: error: invalid library name in argument '-stdlib=libfoo'\n\
         kiln: error: no such file or directory: '/nonexistent.cpp'\n\
         kiln: error: no input files\n"
    );
    let o = kiln(&["-E", "-o"], "");
    assert!(o.stderr.starts_with("kiln: error: argument to '-o' is missing (expected 1 value)\n"));
    let o = kiln(&["-E", "-std=c++27", "-"], "");
    assert_eq!((o.code, o.stderr.as_str()), (1, "error: invalid value 'c++27' in '-std=c++27'\n"));
    let o = kiln(&["-E", "-std=c11", "-"], "");
    assert_eq!(o.stderr, "error: invalid argument '-std=c11' not allowed with 'C++'\n");
    let o = kiln(&["-emit-llvm", "-"], "");
    assert_eq!(
        (o.code, o.stderr.as_str()),
        (1, "kiln: error: -emit-llvm cannot be used when linking\n")
    );
}

#[test]
fn syntax_only() {
    let o = kiln(&["-std=c++26", "-fsyntax-only", "-"], "int f();\n");
    assert_eq!((o.code, o.stdout.as_str(), o.stderr.as_str()), (0, "", ""));
    let o = kiln(&["-std=c++26", "-fsyntax-only", "-"], "int x = ;\n");
    assert_eq!(o.code, 1);
    assert!(o.stderr.ends_with("1 error generated.\n"), "{}", o.stderr);
    let o = kiln(
        &["-std=c++26", "-fsyntax-only", "-fdump-ast", "-fdump-ast-filter=keep", "-"],
        "int keep_me;\nint drop_me;\n",
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.contains("keep_me") && !o.stdout.contains("drop_me"), "{}", o.stdout);
    let o = kiln(&["-fsyntax-only", "-fdump-ast=bogus", "-"], "");
    assert_eq!(
        o.stderr,
        "error: invalid value 'bogus' in '-fdump-ast=bogus'\n1 error generated.\n"
    );
    let o = kiln(&["-E", "-fdump-ast", "-"], "");
    assert_eq!(o.stderr, "kiln: error: -fdump-ast without -fsyntax-only is not implemented yet\n");
    let o = kiln(&["-fsyntax-only", "-x", "c", "-"], "");
    assert_eq!(o.stderr, "error: -fsyntax-only for C is not implemented yet\n");
}

#[test]
fn preprocess_stdin() {
    let o = kiln(&["-E", "-P", "-DX=42", "-"], "int a = X;\n#ifdef __KILN__\nkiln\n#endif\n");
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
    assert_eq!(o.stdout, "int a = 42;\nkiln\n");
    let o = kiln(&["-E", "-"], "x\n");
    assert!(o.stdout.starts_with("# 1 \"<stdin>\"\n"), "{}", o.stdout);
}

#[test]
fn builtin_buffer_like_clang() {
    // clang 19 pushes the interference sizes, so `-D` redefines them
    // without a warning and `pop_macro` restores the original.
    let src = "int a = __GCC_DESTRUCTIVE_SIZE;\n#pragma pop_macro(\"__GCC_DESTRUCTIVE_SIZE\")\n\
               int b = __GCC_DESTRUCTIVE_SIZE;\n";
    let o = kiln(&["-E", "-P", "-D__GCC_DESTRUCTIVE_SIZE=128", "-"], src);
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
    assert_eq!(o.stdout, "int a = 128;\nint b = 64;\n");
    // Before `<command line>`, the move back to the marker's line in
    // `<built-in>` (clang's `EnterFile` include location).
    let o = kiln(&["-E", "-"], "x\n");
    let lines: Vec<&str> = o.stdout.lines().take(5).collect();
    assert_eq!(lines[..3], ["# 1 \"<stdin>\"", "# 1 \"<built-in>\" 1", "# 1 \"<built-in>\" 3"]);
    assert!(lines[3].starts_with("# ") && lines[3].ends_with(" \"<built-in>\" 3"), "{lines:?}");
    assert_eq!(lines[4], "# 1 \"<command line>\" 1");
}

#[test]
fn errors_exit_1_with_summary() {
    let o = kiln(&["-E", "-P", "-"], "#error boom\n#warning hm\nok\n");
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("<stdin>:1:2: error: boom"), "{}", o.stderr);
    assert!(o.stderr.ends_with("1 warning and 1 error generated.\n"), "{}", o.stderr);
    // The output is still written.
    assert_eq!(o.stdout, "ok\n");
    let o = kiln(&["-E", "-P", "-w", "-"], "#warning hm\n");
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
}

#[test]
fn macros_only() {
    let o = kiln(&["-E", "-dM", "-std=c++26", "-x", "c++", "/dev/null"], "");
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.contains("#define __cplusplus 202400L\n"));
    assert!(o.stdout.contains("#define __KILN__ 1\n"));
    assert!(!o.stdout.contains("#define linux 1\n"));
    let o = kiln(&["-E", "-dM", "-x", "c++", "/dev/null"], "");
    assert!(o.stdout.contains("#define linux 1\n"));
    assert!(o.stdout.contains("#define __cplusplus 201703L\n"));
    let o = kiln(&["-E", "-dM", "-std=c11", "-x", "c", "/dev/null"], "");
    assert!(!o.stdout.contains("__cplusplus"));
    assert!(o.stdout.contains("#define __STDC_VERSION__ 201112L\n"));
    assert!(o.stdout.contains("#define __STRICT_ANSI__ 1\n"));
}

#[test]
fn reproducible_time() {
    let o = kiln(&["-E", "-P", "-fkiln-reproducible-time", "-"], "__DATE__ __TIME__\n");
    assert_eq!(o.stdout, "\"Jan  1 1970\" \"00:00:00\"\n");
}

#[test]
fn dependencies() {
    let d = scratch("deps");
    std::fs::write(d.join("a b.h"), "").unwrap();
    std::fs::create_dir_all(d.join("sys")).unwrap();
    std::fs::write(d.join("sys/s.h"), "").unwrap();
    std::fs::write(d.join("t.cpp"), "#include \"a b.h\"\n#include <s.h>\n").unwrap();
    let sys = d.join("sys");
    let sys = sys.to_str().unwrap();
    let t = d.join("t.cpp");
    let t = t.to_str().unwrap();
    let o = kiln(&["-MM", "-MP", "-isystem", sys, t], "");
    assert_eq!(o.code, 0, "{}", o.stderr);
    let dir = d.to_str().unwrap();
    // (Long temporary paths wrap; the wrapping is unit-tested in `deps`.)
    let unwrap = |s: &str| s.replace(" \\\n  ", " ");
    assert_eq!(unwrap(&o.stdout), format!("t.o: {dir}/t.cpp {dir}/a\\ b.h\n{dir}/a\\ b.h:\n"));
    let o = kiln(&["-M", "-MT", "x y", "-MQ", "$z", "-isystem", sys, t], "");
    assert!(o.stdout.starts_with("x y $$z: "), "{}", o.stdout);
    assert!(o.stdout.contains("sys/s.h"), "{}", o.stdout);
    let dfile = d.join("out.d");
    let o = kiln(
        &["-E", "-P", "-MMD", "-MF", dfile.to_str().unwrap(), "-MT", "tgt", "-isystem", sys, t],
        "",
    );
    assert_eq!(o.code, 0);
    let written = std::fs::read_to_string(&dfile).unwrap();
    assert_eq!(unwrap(&written), format!("tgt: {dir}/t.cpp {dir}/a\\ b.h\n"));
    std::fs::remove_dir_all(&d).unwrap();
}

/// `-v` prints clang's search list for each `resource/include-paths` file
/// (generated from clang 19 on the reference container). Skipped where the
/// LLVM 19 installation the goldens describe is missing.
#[test]
fn verbose_search_lists_match_clang() {
    let clang_res = "/usr/lib/llvm-19/lib/clang/19";
    if !Path::new(clang_res).join("include").is_dir() {
        eprintln!("skipped: no {clang_res}");
        return;
    }
    let dir = repo().join("resource/include-paths");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap() {
        let path = e.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let cmd = text.lines().nth(1).unwrap();
        let target = cmd.split_whitespace().find_map(|w| w.strip_prefix("--target=")).unwrap();
        let stdlib = cmd.split_whitespace().find(|w| w.starts_with("-stdlib=")).unwrap();
        let want: String = text
            .lines()
            .filter_map(|l| {
                if let Some(m) = l.strip_prefix("# ignoring") {
                    Some(format!("ignoring{m}\n"))
                } else if l.starts_with("# ") {
                    None
                } else {
                    Some(format!("{l}\n"))
                }
            })
            .collect();
        // Driver -> cc1 lines end where the search block ends.
        let want = want.split("End of search list.\n").next().unwrap().to_owned()
            + "End of search list.\n";
        let tflag = format!("--target={target}");
        let o = kiln(
            &[&tflag, stdlib, "-resource-dir", clang_res, "-E", "-v", "-x", "c++", "/dev/null"],
            "",
        );
        assert_eq!(o.code, 0, "{}", o.stderr);
        let start = o.stderr.find("kiln -cc1 version").expect("cc1 line");
        let got = o.stderr[start..].split_once('\n').unwrap().1;
        let got =
            got.split("End of search list.\n").next().unwrap().to_owned() + "End of search list.\n";
        assert_eq!(got, want, "{}", path.display());
        n += 1;
    }
    assert!(n >= 4);
}

/// clang 19's stderr for `clang -E <dir>/a.cpp` with `<dir>` replaced by
/// `@`: nested and repeated inclusions, a macro-expanded header name, a
/// note (which prints no include stack of its own) and a diagnostic back
/// in the main file.
const INCLUDE_STACK_GOLDEN: &str = "\
In file included from @/a.cpp:1:
In file included from @/b.h:2:
@/c.h:1:9: warning: 'M' macro redefined [-Wmacro-redefined]
    1 | #define M 2
      |         ^
@/b.h:1:9: note: previous definition is here
    1 | #define M 1
      |         ^
In file included from @/a.cpp:1:
In file included from @/b.h:2:
@/c.h:2:2: warning: in c [-W#warnings]
    2 | #warning in c
      |  ^
In file included from @/a.cpp:1:
@/b.h:3:2: warning: in b [-W#warnings]
    3 | #warning in b
      |  ^
In file included from @/a.cpp:3:
@/c.h:2:2: warning: in c [-W#warnings]
    2 | #warning in c
      |  ^
@/a.cpp:4:2: error: main
    4 | #error main
      |  ^
4 warnings and 1 error generated.
";

// spec: diag.md §9.6 (include stack); [cpp.include] (each inclusion is a
// distinct location range, `SourceManager::add_inclusion`)
#[test]
fn include_stack_like_clang() {
    let d = scratch("include-stack");
    std::fs::write(
        d.join("a.cpp"),
        "#include \"b.h\"\n#define HDR \"c.h\"\n#include HDR\n#error main\n",
    )
    .unwrap();
    std::fs::write(d.join("b.h"), "#define M 1\n#include \"c.h\"\n#warning in b\n").unwrap();
    std::fs::write(d.join("c.h"), "#define M 2\n#warning in c\n").unwrap();
    let a = d.join("a.cpp");
    let o = kiln(&["-E", "-o", "/dev/null", a.to_str().unwrap()], "");
    assert_eq!(o.code, 1);
    let dir = d.to_str().unwrap();
    assert_eq!(o.stderr, INCLUDE_STACK_GOLDEN.replace('@', dir));
    // The line markers are unchanged (clang 19's).
    let o = kiln(&["-E", a.to_str().unwrap()], "");
    let markers: Vec<&str> =
        o.stdout.lines().filter(|l| l.starts_with("# ") && !l.contains('<')).collect();
    let want = [
        "# 1 \"@/a.cpp\"",
        "# 1 \"@/a.cpp\" 2",
        "# 1 \"@/b.h\" 1",
        "# 1 \"@/c.h\" 1",
        "# 3 \"@/b.h\" 2",
        "# 2 \"@/a.cpp\" 2",
        "# 1 \"@/c.h\" 1",
        "# 4 \"@/a.cpp\" 2",
    ];
    assert_eq!(markers, want.map(|m| m.replace('@', dir)));
    let _ = std::fs::remove_dir_all(&d);
}
