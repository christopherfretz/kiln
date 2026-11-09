//! End-to-end driver tests: the command line, exit codes, diagnostics,
//! `-v` against clang's search lists (`resource/include-paths`), `-M*`.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use kiln_diag::SharedBuffer;
use kiln_driver::{Io, SharedWriter, run, run_as};

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn kiln(args: &[&str], stdin: &str) -> Out {
    kiln_named(None, args, stdin)
}

/// Runs the driver as `argv0` (`None`: [`run`], i.e. `kiln++`).
fn kiln_named(argv0: Option<&str>, args: &[&str], stdin: &str) -> Out {
    let out = SharedBuffer::new();
    let err = SharedBuffer::new();
    let io = Io {
        stdout: Box::new(out.clone()),
        stderr: SharedWriter::new(err.clone()),
        stdin: Box::new(std::io::Cursor::new(stdin.as_bytes().to_vec())),
    };
    let owned: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let code = match argv0 {
        None => run(&owned, io),
        Some(a) => run_as(a, &owned, io),
    };
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
    assert_eq!((o.code, o.stderr.as_str()), (1, "kiln++: error: no input files\n"));
}

/// spec/driver.md "`-v`, `--version`, `--help`": `-print-resource-dir`
/// (and `--print-resource-dir`) prints the resource directory and exits
/// 0 whatever else is given, as clang does; `CMake`'s compiler probe asks
/// for it (WP-667).
#[test]
fn print_resource_dir() {
    for flag in ["-print-resource-dir", "--print-resource-dir"] {
        let o = kiln(&[flag, "-resource-dir", "/some/res"], "");
        assert_eq!((o.code, o.stdout.as_str(), o.stderr.as_str()), (0, "/some/res\n", ""));
    }
    let o = kiln(&["-print-resource-dir", "-c", "/nonexistent.cpp"], "");
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(Path::new(o.stdout.trim_end()).join("include").is_dir(), "{}", o.stdout);
}

/// spec/driver.md "Toolchain probes" (WP-920): `-dumpversion` prints the
/// clang compatibility version, the one `__clang_major__`,
/// `__clang_minor__` and `__clang_patchlevel__` spell (configure probes
/// compare it with clang thresholds), not kiln's own version.
#[test]
fn dumpversion_is_the_clang_compat_version() {
    for (compat, want) in [(None, "22.1.8\n"), (Some("-fclang-compat=19"), "19.1.7\n")] {
        let mut args = vec!["-dumpversion"];
        args.extend(compat);
        let o = kiln(&args, "");
        assert_eq!((o.code, o.stdout.as_str(), o.stderr.as_str()), (0, want, ""));
        let mut args = vec!["-E", "-P", "-"];
        args.extend(compat);
        let o = kiln(&args, "__clang_major__.__clang_minor__.__clang_patchlevel__\n");
        assert_eq!(o.stdout.replace(' ', ""), want);
    }
}

/// spec/driver.md "Toolchain probes": the target probes answer from the
/// target kiln compiles for, without a compile; the first probe in clang's
/// order (`HandleImmediateArgs`) wins.
#[test]
fn target_probes() {
    let t = "--target=x86_64-linux-gnu";
    let cases: [(&[&str], &str); 7] = [
        (&["-dumpmachine", t], "x86_64-unknown-linux-gnu\n"),
        (&["-print-target-triple", t], "x86_64-unknown-linux-gnu\n"),
        (&["-print-multiarch", t], "x86_64-linux-gnu\n"),
        (&["-print-sysroot"], "\n"),
        (&["-print-sysroot", "--sysroot=/some/root"], "/some/root\n"),
        (&["-dumpversion", "-dumpmachine", t], "x86_64-unknown-linux-gnu\n"),
        (&["-print-sysroot", "-dumpversion"], "22.1.8\n"),
    ];
    for (args, want) in cases {
        let o = kiln(args, "");
        assert_eq!((o.code, o.stdout.as_str(), o.stderr.as_str()), (0, want, ""), "{args:?}");
    }
}

/// spec/driver.md "Toolchain probes": `-print-file-name=` answers from the
/// resource directory first, and that, `-print-prog-name=`,
/// `-print-libgcc-file-name` and `-print-search-dirs` otherwise come from
/// the backing clang++ (the one kiln links with), which knows the GCC
/// installation and the runtime libraries.
#[test]
fn delegated_probes() {
    let d = scratch("probes");
    std::fs::create_dir_all(d.join("include")).unwrap();
    let res = d.to_str().unwrap();
    let o = kiln(&["-resource-dir", res, "-print-file-name=include"], "");
    assert_eq!((o.code, o.stdout.clone()), (0, format!("{res}/include\n")), "{}", o.stderr);
    let o = kiln(&["-print-file-name=kiln-no-such-file.zz"], "");
    assert_eq!((o.code, o.stdout.as_str()), (0, "kiln-no-such-file.zz\n"), "{}", o.stderr);
    let o = kiln(&["-print-prog-name=ld"], "");
    assert!(o.code == 0 && o.stdout.ends_with("ld\n"), "{} {}", o.stdout, o.stderr);
    let o = kiln(&["-print-libgcc-file-name"], "");
    assert!(
        o.code == 0 && Path::new(o.stdout.trim_end()).extension().is_some_and(|e| e == "a"),
        "{} {}",
        o.stdout,
        o.stderr
    );
    let o = kiln(&["-print-search-dirs"], "");
    assert!(o.code == 0 && o.stdout.starts_with("programs: ="), "{} {}", o.stdout, o.stderr);
    assert!(o.stdout.contains("\nlibraries: ="), "{}", o.stdout);
    std::fs::remove_dir_all(&d).unwrap();
}

/// spec/driver.md "Identity" (operator decision Q3, WP-920): `__VERSION__`
/// names kiln; the clang macros stay so headers take their clang branches.
#[test]
fn version_macro_names_kiln() {
    let o = kiln(&["-E", "-P", "-"], "__VERSION__ __clang__\n");
    assert_eq!(o.stdout, format!("\"kiln {}\" 1\n", kiln_driver::version::VERSION));
    let o = kiln(&["--version"], "");
    assert!(!o.stdout.contains("clang version"), "{}", o.stdout);
}

/// spec/driver.md "Reflection" (operator decision Q7, WP-920): reflection
/// is on at C++26 by default and the last `-f[no-]reflection` wins; below
/// C++26 `__cpp_impl_reflection` stays undefined whatever the flag.
#[test]
fn reflection_default_follows_the_standard() {
    let cases: [(&[&str], &str); 6] = [
        (&["-std=c++26"], "202603L\n"),
        (&["-std=gnu++26"], "202603L\n"),
        (&["-std=c++26", "-fno-reflection"], "__cpp_impl_reflection\n"),
        (&["-std=c++26", "-fno-reflection", "-freflection"], "202603L\n"),
        (&["-std=c++23"], "__cpp_impl_reflection\n"),
        (&["-std=c++23", "-freflection"], "__cpp_impl_reflection\n"),
    ];
    for (std, want) in cases {
        let mut args = vec!["-E", "-P", "-"];
        args.extend_from_slice(std);
        let o = kiln(&args, "__cpp_impl_reflection\n");
        assert_eq!((o.code, o.stdout.as_str()), (0, want), "{std:?}: {}", o.stderr);
    }
}

/// spec/driver.md "`-dD`, `-dI`": `-dD` keeps every `#define`/`#undef` in
/// the output (the predefines first), `-dI` every `#include` as a comment
/// line, as clang's.
#[test]
fn dump_defines_and_includes() {
    let d = scratch("dd");
    std::fs::write(d.join("x.h"), "int h;\n").unwrap();
    std::fs::write(d.join("t.cpp"), "#define A 1\nA\n#include \"x.h\"\n#undef A\nA\n").unwrap();
    let t = d.join("t.cpp");
    let t = t.to_str().unwrap();
    let o = kiln(&["-E", "-dD", "-P", t], "");
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.contains("#define __cplusplus "), "{}", o.stdout);
    assert!(o.stdout.ends_with("\n#define A 1\n1\nint h;\n#undef A\nA\n"), "{}", o.stdout);
    let o = kiln(&["-E", "-dI", "-P", t], "");
    assert_eq!(o.stdout, "\n1\n#include \"x.h\" /* clang -E -dI */\nint h;\n\nA\n");
    std::fs::remove_dir_all(&d).unwrap();
}

/// spec/driver.md "Dependency files" (WP-596, WP-920): `-MG` lists a
/// missing header as a dependency (a generated one) under `-M`/`-MM`, and
/// needs one of them.
#[test]
fn missing_headers_as_generated() {
    let d = scratch("mg");
    std::fs::write(d.join("t.cpp"), "#include \"gen.h\"\n#include <gen2.h>\n").unwrap();
    let t = d.join("t.cpp");
    let t = t.to_str().unwrap();
    let o = kiln(&["-MM", "-MG", "-MP", t], "");
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(
        o.stdout.replace(" \\\n  ", " "),
        format!("t.o: {t} gen.h gen2.h\ngen.h:\ngen2.h:\n")
    );
    let o = kiln(&["-MM", t], "");
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("'gen.h' file not found"), "{}", o.stderr);
    for action in ["-E", "-c"] {
        let o = kiln(&[action, "-MG", t], "");
        assert_eq!(o.code, 1);
        assert_eq!(o.stderr, "kiln++: error: option '-MG' requires '-M' or '-MM'\n");
    }
    std::fs::remove_dir_all(&d).unwrap();
}

/// spec/driver.md "`-MJ`" (WP-920): one JSON compilation-database line per
/// frontend job, clang's format; `-###` writes none.
#[test]
fn compilation_database_fragment() {
    let d = scratch("mj");
    std::fs::write(d.join("t.cpp"), "int x;\n").unwrap();
    let t = d.join("t.cpp");
    let t = t.to_str().unwrap();
    let j = d.join("t.json");
    let j = j.to_str().unwrap();
    let o = kiln(&["-###", "-c", t, "-o", "t.o", "-MJ", j], "");
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(!d.join("t.json").exists());
    let obj = d.join("t.o");
    let obj = obj.to_str().unwrap();
    let o = kiln(&["-c", t, "-o", obj, "-DX=1", "-MJ", j, "--target=x86_64-linux-gnu"], "");
    assert_eq!(o.code, 0, "{}", o.stderr);
    let text = std::fs::read_to_string(j).unwrap();
    let cwd = std::env::current_dir().unwrap();
    let head = format!(
        "{{ \"directory\": \"{}\", \"file\": \"{t}\", \"output\": \"{obj}\", \"arguments\": [\"",
        cwd.display()
    );
    assert!(text.starts_with(&head), "{text}");
    let tail = format!(
        "\"-xc++\", \"{t}\", \"-o\", \"{obj}\", \"--driver-mode=g++\", \"-c\", \"-D\", \"X=1\", \"--target=x86_64-linux-gnu\", \"--target=x86_64-unknown-linux-gnu\"]}},\n"
    );
    assert!(text.ends_with(&tail), "{text}");
    let o = kiln(&["-fsyntax-only", t, "-MJ", j], "");
    assert_eq!(o.code, 0, "{}", o.stderr);
    let text = std::fs::read_to_string(j).unwrap();
    assert!(text.contains(&format!("\"file\": \"{t}\", \"arguments\": [")), "{text}");
    let o = kiln(&["-fsyntax-only", t, "-MJ", "/kiln-no-such-dir/t.json"], "");
    assert_eq!(o.code, 1);
    assert_eq!(
        o.stderr,
        "kiln++: error: compilation database '/kiln-no-such-dir/t.json' could not be opened: No such file or directory\n"
    );
    std::fs::remove_dir_all(&d).unwrap();
}

/// spec/driver.md "Inputs": linker inputs alone are inputs, as clang's
/// (`-Wl,--version` runs only the linker: Meson's linker probe, WP-920).
#[test]
fn linker_inputs_alone_are_inputs() {
    let o = kiln(&["-c", "-Wl,--version"], "");
    assert_eq!(o.code, 0);
    assert_eq!(
        o.stderr,
        "kiln++: warning: -Wl,--version: 'linker' input unused [-Wunused-command-line-argument]\n"
    );
}

/// spec/driver.md "Time trace": with `-fsyntax-only` clang runs no
/// compiler job that writes a trace, so each spelling of the family is
/// `argument unused during compilation` (silenced by
/// `-Qunused-arguments`) and nothing is written; a granularity that is not
/// an unsigned integer is an error whenever a trace would be written
/// (WP-1115; the compiling cases are in `tests/time_trace.rs`).
#[test]
fn time_trace_syntax_only() {
    let d = scratch("tt");
    let file = d.join("x.json");
    let file = file.to_str().unwrap();
    let eq = format!("-ftime-trace={file}");
    let flags =
        ["-ftime-trace", eq.as_str(), "-ftime-trace-granularity=10", "-ftime-trace-verbose"];
    let mut args = vec!["-std=c++26", "-fsyntax-only"];
    args.extend(flags);
    args.push("-");
    let o = kiln(&args, "int f();\n");
    assert_eq!(o.code, 0, "{}", o.stderr);
    let want: String = flags
        .iter()
        .map(|f| {
            "kiln++: warning: argument unused during compilation: '".to_owned()
                + f
                + "' [-Wunused-command-line-argument]\n"
        })
        .collect();
    assert_eq!(o.stderr, want);
    assert!(!Path::new(file).exists());
    args.insert(0, "-Qunused-arguments");
    let o = kiln(&args, "int f();\n");
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
    let o = kiln(&["-E", "-ftime-trace", "-ftime-trace-granularity=abc", "-"], "");
    assert_eq!(
        (o.code, o.stderr.as_str()),
        (1, "kiln++: error: invalid integral value 'abc' in '-ftime-trace-granularity=abc'\n")
    );
}

#[test]
fn argument_errors() {
    let o = kiln(&["-E", "-fbogus", "-x", "pascal", "-stdlib=libfoo", "/nonexistent.cpp"], "");
    assert_eq!(o.code, 1);
    assert_eq!(
        o.stderr,
        "kiln++: error: unknown argument: '-fbogus'\n\
         kiln++: error: language not recognized: 'pascal'\n\
         kiln++: error: invalid library name in argument '-stdlib=libfoo'\n\
         kiln++: error: no such file or directory: '/nonexistent.cpp'\n\
         kiln++: error: no input files\n"
    );
    let o = kiln(&["-E", "-o"], "");
    assert!(
        o.stderr.starts_with("kiln++: error: argument to '-o' is missing (expected 1 value)\n")
    );
    let o = kiln(&["-E", "-std=c++27", "-"], "");
    assert_eq!((o.code, o.stderr.as_str()), (1, "error: invalid value 'c++27' in '-std=c++27'\n"));
    let o = kiln(&["-E", "-std=c11", "-"], "");
    assert_eq!(o.stderr, "error: invalid argument '-std=c11' not allowed with 'C++'\n");
    let o = kiln(&["-emit-llvm", "-"], "");
    assert_eq!(
        (o.code, o.stderr.as_str()),
        (1, "kiln++: error: -emit-llvm cannot be used when linking\n")
    );
}

/// The visibility and interposition flags (spec/driver.md "Options",
/// "Visibility"): all accepted silently, as by clang; an unknown
/// `-fvisibility=` value is clang's per-input `-cc1` error, unless a later
/// valid `-fvisibility=` (clang forwards only the last) replaces it.
#[test]
fn visibility_flags() {
    let ok = [
        "-fvisibility=default",
        "-fvisibility=hidden",
        "-fvisibility=protected",
        "-fvisibility=internal",
        "-fvisibility-ms-compat",
        "-fvisibility-inlines-hidden",
        "-fno-visibility-inlines-hidden",
        "-fsemantic-interposition",
        "-fno-semantic-interposition",
        "-fdirect-access-external-data",
        "-fno-direct-access-external-data",
    ];
    for f in ok {
        let o = kiln(&["-fsyntax-only", f, "-x", "c++", "-"], "int x;\n");
        assert_eq!((o.code, o.stderr.as_str()), (0, ""), "{f}");
    }
    let bad = "error: invalid value 'foo' in '-fvisibility=foo'\n";
    for mode in ["-E", "-fsyntax-only"] {
        let o = kiln(&[mode, "-fvisibility=foo", "-x", "c++", "-"], "int x;\n");
        assert_eq!((o.code, o.stderr.as_str()), (1, bad), "{mode}");
    }
    let o =
        kiln(&["-fsyntax-only", "-fvisibility=foo", "-fvisibility=hidden", "-x", "c++", "-"], "");
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
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
    assert_eq!(
        o.stderr,
        "kiln++: error: -fdump-ast without -fsyntax-only is not implemented yet\n"
    );
    let o = kiln(&["-fsyntax-only", "-x", "c", "-"], "");
    assert_eq!(o.stderr, "error: -fsyntax-only for C is not implemented yet\n");
}

/// `-flax-vector-conversions[=none|integer|all]`, `-fno-lax-vector-conversions`
/// (clang's driver flags; GNU vector extension, spec/sema.md §5.8): which
/// same-size vector bitcasts are implicit. clang is the oracle.
#[test]
fn lax_vector_conversions_like_clang() {
    let src = "typedef int i4 __attribute__((vector_size(16)));\n\
               typedef unsigned u4 __attribute__((vector_size(16)));\n\
               typedef float f4 __attribute__((vector_size(16)));\n\
               void f(u4 u, f4 b) { i4 x = u; i4 z = b; }\n";
    let int_err = "<stdin>:4:25: error: cannot initialize a variable of type 'i4' (vector of 4 \
                   'int' values) with an lvalue of type 'u4' (vector of 4 'unsigned int' values)\n";
    let float_err = "<stdin>:4:35: error: cannot initialize a variable of type 'i4' (vector of 4 \
                     'int' values) with an lvalue of type 'f4' (vector of 4 'float' values)\n";
    let run = |flag: &[&str]| {
        let mut args = vec!["-std=c++26", "-fsyntax-only", "-fno-caret-diagnostics"];
        args.extend_from_slice(flag);
        args.push("-");
        kiln(&args, src)
    };
    for f in [&[][..], &["-flax-vector-conversions=all"]] {
        let o = run(f);
        assert_eq!((o.code, o.stderr.as_str()), (0, ""), "{f:?}");
    }
    for f in [&["-flax-vector-conversions"][..], &["-flax-vector-conversions=integer"]] {
        let o = run(f);
        assert_eq!(o.stderr, format!("{float_err}1 error generated.\n"), "{f:?}");
    }
    for f in [&["-fno-lax-vector-conversions"][..], &["-flax-vector-conversions=none"]] {
        let o = run(f);
        assert_eq!(o.stderr, format!("{int_err}{float_err}2 errors generated.\n"), "{f:?}");
    }
    // clang reports this from cc1 without a program prefix; kiln's driver
    // errors all carry `kiln++: `.
    let o = run(&["-flax-vector-conversions=bogus"]);
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.ends_with("error: invalid value 'bogus' in '-flax-vector-conversions=bogus'\n"),
        "{}",
        o.stderr
    );
}

/// `-finput-charset=` / `-fexec-charset=` accept UTF-8 in any case and
/// nothing else, as clang's driver (folly passes `-finput-charset=UTF-8`).
#[test]
fn charset_flags_accept_utf8_only() {
    for f in ["-finput-charset=UTF-8", "-finput-charset=utf-8", "-fexec-charset=Utf-8"] {
        let o = kiln(&["-fsyntax-only", f, "-"], "int x;\n");
        assert_eq!((o.code, o.stderr.as_str()), (0, ""), "{f}");
    }
    for (f, v) in [("-finput-charset=latin1", "latin1"), ("-fexec-charset=UTF8", "UTF8")] {
        let o = kiln(&["-fsyntax-only", f, "-"], "int x;\n");
        assert_eq!(o.code, 1, "{f}");
        assert!(o.stderr.contains(&format!("error: invalid value '{v}' in '{f}'")), "{}", o.stderr);
    }
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
    // without a warning and `pop_macro` restores the original (64; clang
    // 22's original is target-dependent, so the level is pinned).
    let src = "int a = __GCC_DESTRUCTIVE_SIZE;\n#pragma pop_macro(\"__GCC_DESTRUCTIVE_SIZE\")\n\
               int b = __GCC_DESTRUCTIVE_SIZE;\n";
    let o = kiln(&["-E", "-P", "-fclang-compat=19", "-D__GCC_DESTRUCTIVE_SIZE=128", "-"], src);
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

/// The search block of a `-v` run (`ignoring ...` lines, both
/// `#include ... search starts here:` headers and the directories): from the
/// line after `<name> -cc1 version` through `End of search list.`.
fn search_block(stderr: &str, cc1: &str) -> String {
    let start = stderr.find(cc1).unwrap_or_else(|| panic!("no `{cc1}` line in\n{stderr}"));
    let rest = stderr[start..].split_once('\n').unwrap().1;
    rest.split("End of search list.\n").next().unwrap().to_owned() + "End of search list.\n"
}

/// `-v` prints clang's search list for each (target, stdlib) pair that has a
/// `resource/include-paths` file. The lists depend on the host's GCC
/// installations, so kiln is compared with clang++-19 run live on this host;
/// the files themselves are the reference container's (WP-228). Skipped
/// where clang++-19 or its resource directory is missing.
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
        let tflag = format!("--target={target}");
        let args =
            [&tflag, stdlib, "-resource-dir", clang_res, "-E", "-v", "-x", "c++", "/dev/null"];
        let Ok(clang) = std::process::Command::new("clang++-19").args(args).output() else {
            eprintln!("skipped: no clang++-19");
            return;
        };
        assert!(clang.status.success(), "clang++-19 {args:?} failed");
        let want = search_block(&String::from_utf8_lossy(&clang.stderr), "clang -cc1 version");
        // libc++'s headers are the LLVM installation's: clang 19's here
        // (kiln's default is LLVM 22, spec/driver.md "LLVM discovery").
        let mut kargs = args.to_vec();
        kargs.push("--llvm-dir=/usr/lib/llvm-19");
        let o = kiln(&kargs, "");
        assert_eq!(o.code, 0, "{}", o.stderr);
        let got = search_block(&o.stderr, "kiln -cc1 version");
        assert_eq!(got, want, "{target} {stdlib} ({})", path.display());
        n += 1;
    }
    assert!(n >= 4);
}

/// The GCC lines of a `-v` run (`Found candidate`/`Selected GCC
/// installation`).
fn gcc_lines(stderr: &str) -> Vec<&str> {
    stderr.lines().filter(|l| l.contains("GCC installation")).collect()
}

// spec: driver.md "Toolchain" (WP-525): `--gcc-toolchain=<dir>` makes
// `<dir>` the only prefix of the GCC installation search (clang 19/22
// `GCCInstallationDetector::init`), `--gcc-install-dir=<dir>` names the
// installation; the libstdc++ directories derive from it. A fake tree
// (two installations with a `crtbegin.o` and headers, one with neither)
// against clang++-19
// run live; skipped without it.
#[test]
fn gcc_toolchain_matches_clang() {
    let clang_res = "/usr/lib/llvm-19/lib/clang/19";
    if !Path::new(clang_res).join("include").is_dir() {
        eprintln!("skipped: no {clang_res}");
        return;
    }
    let d = scratch("gcc-toolchain");
    let tc = d.join("tc");
    for triple in ["aarch64-linux-gnu", "x86_64-linux-gnu"] {
        for v in ["15", "16.1.0", "17"] {
            let g = tc.join("lib/gcc").join(triple).join(v);
            std::fs::create_dir_all(&g).unwrap();
            if v != "17" {
                std::fs::write(g.join("crtbegin.o"), "").unwrap();
            }
        }
        let inc = tc.join("include/c++/16.1.0");
        std::fs::create_dir_all(inc.join(triple).join("bits")).unwrap();
        std::fs::write(inc.join(triple).join("bits/c++config.h"), "").unwrap();
    }
    std::fs::create_dir_all(tc.join("include/c++/16.1.0/backward")).unwrap();
    std::fs::create_dir_all(tc.join("include/c++/15")).unwrap();
    std::fs::write(tc.join("include/c++/16.1.0/vector"), "").unwrap();
    let tcs = tc.display().to_string();
    let mut n = 0;
    for target in ["--target=aarch64-linux-gnu", "--target=x86_64-linux-gnu"] {
        let triple = &target["--target=".len()..];
        let install = format!("{tcs}/lib/gcc/{triple}/15/");
        for gcc in [format!("--gcc-toolchain={tcs}/"), format!("--gcc-install-dir={install}")] {
            let args =
                [target, &gcc, "-resource-dir", clang_res, "-E", "-v", "-x", "c++", "/dev/null"];
            let Ok(clang) = std::process::Command::new("clang++-19").args(args).output() else {
                eprintln!("skipped: no clang++-19");
                return;
            };
            assert!(clang.status.success(), "clang++-19 {args:?} failed");
            let cerr = String::from_utf8_lossy(&clang.stderr);
            let o = kiln(&args, "");
            assert_eq!(o.code, 0, "{}", o.stderr);
            assert_eq!(gcc_lines(&o.stderr), gcc_lines(&cerr), "{args:?}");
            assert_eq!(
                search_block(&o.stderr, "kiln -cc1 version"),
                search_block(&cerr, "clang -cc1 version"),
                "{args:?}"
            );
            n += 1;
        }
    }
    assert_eq!(n, 4);
    let bad = format!("--gcc-install-dir={tcs}/lib/gcc/aarch64-linux-gnu/17");
    let o = kiln(&[&bad, "-fsyntax-only", "-x", "c++", "/dev/null"], "");
    assert_eq!(
        (o.code, o.stderr),
        (
            1,
            format!(
                "kiln++: error: '{tcs}/lib/gcc/aarch64-linux-gnu/17' does not contain a GCC installation\n"
            )
        )
    );
    std::fs::remove_dir_all(&d).unwrap();
}

// spec: driver.md "Target features" (WP-808): `+outline-atomics` is the
// AArch64 default only with a GCC installation of 9.3.1 or later (clang
// 19/22 `Linux::IsAArch64OutlineAtomicsDefault`), so a cross compile with
// no AArch64 GCC gets inline atomics; `-m[no-]outline-atomics` decides
// regardless. Fake installations are named, so the host's GCC does not
// matter; clang++-19, when present, must agree.
#[test]
fn aarch64_outline_atomics_follow_the_gcc_installation() {
    let d = scratch("outline-atomics");
    let mut cases = Vec::new();
    for (v, want) in [("9.3.0", false), ("9.3.1", true), ("14", true)] {
        let g = d.join(v).join("lib/gcc/aarch64-linux-gnu").join(v);
        std::fs::create_dir_all(&g).unwrap();
        std::fs::write(g.join("crtbegin.o"), "").unwrap();
        cases.push((format!("--gcc-toolchain={}", d.join(v).display()), None, want));
    }
    let none = d.join("none");
    std::fs::create_dir_all(&none).unwrap();
    let no_gcc = format!("--gcc-toolchain={}", none.display());
    cases.push((no_gcc.clone(), None, false));
    cases.push((no_gcc.clone(), Some("-moutline-atomics"), true));
    cases.push((cases[2].0.clone(), Some("-mno-outline-atomics"), false));
    let src = "int f(int *p) { return __atomic_fetch_add(p, 1, 5); }\n";
    for (gcc, flag, want) in &cases {
        let mut args = vec!["--target=aarch64-linux-gnu", gcc.as_str(), "-S", "-emit-llvm"];
        args.extend(flag.iter().copied());
        args.extend(["-o", "-", "-x", "c++", "-"]);
        let o = kiln(&args, src);
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert_eq!(o.stdout.contains("+outline-atomics"), *want, "{args:?}");
        // `-outline-atomics` only when written, as clang.
        assert_eq!(o.stdout.contains("-outline-atomics"), *flag == Some("-mno-outline-atomics"));
        if let Ok(c) = std::process::Command::new("clang++-19")
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write as _;
                c.stdin.take().unwrap().write_all(src.as_bytes())?;
                c.wait_with_output()
            })
        {
            let cout = String::from_utf8_lossy(&c.stdout);
            if c.status.success() {
                let f = |s: &str| (s.contains("+outline-atomics"), s.contains("-outline-atomics"));
                assert_eq!(f(&cout), f(&o.stdout), "clang++-19 {args:?}");
            }
        }
    }
    std::fs::remove_dir_all(&d).unwrap();
}

// spec: driver.md "Floating-point flags" (WP-806): only
// `-fdenormal-fp-math=` gives `denormal-fp-math`. `-ffast-math` on x86-64
// with a GCC installation that has `crtfastmath.o` adds none, as clang 19
// and 22 (the link still adds `crtfastmath.o`). The host's own GCC must
// not matter, so a fake installation is named.
#[test]
fn fast_math_x86_64_crtfastmath_keeps_ieee_denormals() {
    let d = scratch("crtfastmath");
    let g = d.join("tc/lib/gcc/x86_64-linux-gnu/14");
    std::fs::create_dir_all(&g).unwrap();
    std::fs::write(g.join("crtbegin.o"), "").unwrap();
    std::fs::write(g.join("crtfastmath.o"), "").unwrap();
    let gcc = format!("--gcc-toolchain={}", d.join("tc").display());
    let src = "float f(float a, float b) { return a * b; }\n";
    for fast in ["-ffast-math", "-Ofast", "-funsafe-math-optimizations"] {
        let args = [
            "--target=x86_64-linux-gnu",
            &gcc,
            fast,
            "-S",
            "-emit-llvm",
            "-o",
            "-",
            "-x",
            "c++",
            "-",
        ];
        let o = kiln(&args, src);
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert!(o.stdout.contains("\"no-signed-zeros-fp-math\"=\"true\""), "{fast}: {}", o.stdout);
        assert!(!o.stdout.contains("denormal-fp-math"), "{fast}: {}", o.stdout);
    }
    let o = kiln(
        &[
            "--target=x86_64-linux-gnu",
            &gcc,
            "-ffast-math",
            "-fdenormal-fp-math=preserve-sign",
            "-fno-fast-math",
            "-S",
            "-emit-llvm",
            "-o",
            "-",
            "-x",
            "c++",
            "-",
        ],
        src,
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.contains("\"denormal-fp-math\"=\"preserve-sign,preserve-sign\""));
    std::fs::remove_dir_all(&d).unwrap();
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

// clang 19's stderr for the files of `system_header_warnings_like_clang`
// (`@` is the scratch directory), without and with `-Wsystem-headers`.
const SYSTEM_HEADER_GOLDEN: &str = r"In file included from @/a.cpp:1:
@/sys/s.h:1:2: warning: in s [-W#warnings]
    1 | #warning in s
      |  ^
In file included from @/a.cpp:2:
@/u.h:1:10: warning: multi-character character constant [-Wmultichar]
    1 | int u1 = 'ef';
      |          ^
@/a.cpp:3:9: warning: multi-character character constant [-Wmultichar]
    3 | int m = M;
      |         ^
@/sys/s.h:2:11: note: expanded from macro 'M'
    2 | #define M 'ab'
      |           ^
3 warnings generated.
";
const SYSTEM_HEADER_ALL_GOLDEN: &str = r"In file included from @/a.cpp:1:
@/sys/s.h:1:2: warning: in s [-W#warnings]
    1 | #warning in s
      |  ^
@/sys/s.h:3:9: warning: multi-character character constant [-Wmultichar]
    3 | int s = 'cd';
      |         ^
In file included from @/a.cpp:2:
@/u.h:1:10: warning: multi-character character constant [-Wmultichar]
    1 | int u1 = 'ef';
      |          ^
@/u.h:3:10: warning: multi-character character constant [-Wmultichar]
    3 | int u2 = 'gh';
      |          ^
@/a.cpp:3:9: warning: multi-character character constant [-Wmultichar]
    3 | int m = M;
      |         ^
@/sys/s.h:2:11: note: expanded from macro 'M'
    2 | #define M 'ab'
      |           ^
5 warnings generated.
";

// spec: diag.md §5.4 (warnings in system headers); spec/source.md "File
// kinds"; spec/pp.md (`#pragma GCC system_header`, `-isystem`)
#[test]
fn system_header_warnings_like_clang() {
    let d = scratch("system-headers");
    std::fs::create_dir_all(d.join("sys")).unwrap();
    std::fs::write(d.join("a.cpp"), "#include <s.h>\n#include \"u.h\"\nint m = M;\n").unwrap();
    std::fs::write(d.join("sys/s.h"), "#warning in s\n#define M 'ab'\nint s = 'cd';\n").unwrap();
    std::fs::write(d.join("u.h"), "int u1 = 'ef';\n#pragma GCC system_header\nint u2 = 'gh';\n")
        .unwrap();
    let (a, sys) = (d.join("a.cpp"), d.join("sys"));
    let (a, sys) = (a.to_str().unwrap(), sys.to_str().unwrap());
    let dir = d.to_str().unwrap();
    let o = kiln(&["-fsyntax-only", "-isystem", sys, a], "");
    assert_eq!(o.code, 0);
    assert_eq!(o.stderr, SYSTEM_HEADER_GOLDEN.replace('@', dir));
    let o = kiln(&["-fsyntax-only", "-Wsystem-headers", "-isystem", sys, a], "");
    assert_eq!(o.code, 0);
    assert_eq!(o.stderr, SYSTEM_HEADER_ALL_GOLDEN.replace('@', dir));
    let _ = std::fs::remove_dir_all(&d);
}

/// `-m[no-]outline-atomics` on x86-64: clang 19's `-Woption-ignored` text,
/// naming the last spelling, in every mode; `-Werror` makes it an error
/// (no job runs), `-w` silences it, and `AArch64` takes the flag (WP-228).
#[test]
fn outline_atomics_ignored_on_x86_64() {
    let x86 = "--target=x86_64-unknown-linux-gnu";
    let warn = |flag: &str| {
        format!(
            "kiln++: warning: 'x86_64' does not support '{flag}'; flag ignored [-Woption-ignored]\n"
        )
    };
    let o = kiln(&[x86, "-moutline-atomics", "-fsyntax-only", "-x", "c++", "-"], "int x;\n");
    assert_eq!((o.code, o.stderr.as_str()), (0, warn("-moutline-atomics").as_str()));
    let o = kiln(
        &[x86, "-moutline-atomics", "-mno-outline-atomics", "-E", "-x", "c++", "-"],
        "int x;\n",
    );
    assert_eq!((o.code, o.stderr.as_str()), (0, warn("-mno-outline-atomics").as_str()));
    let o = kiln(&[x86, "-mno-outline-atomics", "-Werror", "-E", "-x", "c++", "-"], "int x;\n");
    assert_eq!(o.code, 1);
    assert_eq!(
        o.stderr,
        "kiln++: error: 'x86_64' does not support '-mno-outline-atomics'; flag ignored \
         [-Werror,-Woption-ignored]\n"
    );
    assert_eq!(o.stdout, "");
    for quiet in ["-w", "-Wno-option-ignored"] {
        let o = kiln(&[x86, "-moutline-atomics", quiet, "-fsyntax-only", "-x", "c++", "-"], "");
        assert_eq!((o.code, o.stderr.as_str()), (0, ""), "{quiet}");
    }
    let a64 = "--target=aarch64-unknown-linux-gnu";
    let o = kiln(&[a64, "-mno-outline-atomics", "-fsyntax-only", "-x", "c++", "-"], "int x;\n");
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
}

/// One driver, two names (spec/driver.md "Driver modes", "Input
/// languages"): the program name picks the mode and the diagnostics'
/// prefix; `kiln++` compiles `.c`/`.h`/`.i` as C++ with clang++'s
/// deprecated warning once per input (`-Werror` stops the run, `-w` and
/// `-Wno-deprecated` silence it); `kiln` takes them, and standard input,
/// as C; `-x` beats both; `--driver-mode=` overrides the name, the last
/// one winning.
#[test]
fn driver_modes() {
    let dir = scratch("modes");
    for (f, src) in [("a.c", "int a = __cplusplus;\n"), ("b.h", "int b;\n"), ("c.i", "int c;\n")] {
        std::fs::write(dir.join(f), src).unwrap();
    }
    let path = |f: &str| dir.join(f).display().to_string();
    let (c_src, h_src, i_src) = (path("a.c"), path("b.h"), path("c.i"));
    let warn = |from: &str, to: &str| {
        format!(
            "kiln++: warning: treating '{from}' input as '{to}' when in C++ mode, this behavior \
             is deprecated [-Wdeprecated]\n"
        )
    };
    let o = kiln(&["-E", "-P", &c_src, &h_src, &i_src], "");
    assert_eq!(o.code, 0);
    assert_eq!(
        o.stderr,
        warn("c", "c++") + &warn("c-header", "c++-header") + &warn("cpp-output", "c++-cpp-output")
    );
    assert!(o.stdout.starts_with("int a = 201703L;"), "{}", o.stdout);
    let o = kiln_named(Some("/usr/local/bin/kiln++"), &["-fsyntax-only", &c_src], "");
    assert_eq!((o.code, o.stderr), (0, warn("c", "c++")));
    let o = kiln(&["-fsyntax-only", "-Werror", &c_src], "");
    assert_eq!(o.code, 1);
    assert_eq!(
        o.stderr,
        "kiln++: error: treating 'c' input as 'c++' when in C++ mode, this behavior is \
         deprecated [-Werror,-Wdeprecated]\n"
    );
    for quiet in ["-w", "-Wno-deprecated"] {
        let o = kiln(&["-fsyntax-only", quiet, &c_src], "");
        assert_eq!((o.code, o.stderr.as_str()), (0, ""), "{quiet}");
    }
    // `-x` beats the mode: no promotion, C.
    let o = kiln(&["-E", "-P", "-x", "c", &c_src], "");
    assert_eq!((o.code, o.stderr.as_str(), o.stdout.as_str()), (0, "", "int a = __cplusplus;\n"));

    // `kiln`: C by extension (C is preprocessed, not compiled yet), C++
    // otherwise, standard input C.
    let o = kiln_named(Some("kiln"), &["-E", "-P", &c_src], "");
    assert_eq!((o.code, o.stderr.as_str(), o.stdout.as_str()), (0, "", "int a = __cplusplus;\n"));
    let o = kiln_named(Some("kiln"), &["-fsyntax-only", &h_src], "");
    assert_eq!(
        (o.code, o.stderr.as_str()),
        (1, "error: -fsyntax-only for C is not implemented yet\n")
    );
    let o = kiln_named(Some("kiln"), &["-E", "-P", "-"], "__cplusplus __STDC_VERSION__\n");
    assert_eq!((o.code, o.stdout.as_str()), (0, "__cplusplus 201710L\n"));
    let o = kiln_named(Some("kiln"), &["-E", "-P", "-x", "c++", &c_src], "");
    assert!(o.stdout.starts_with("int a = 201703L;"), "{}", o.stdout);
    let o = kiln_named(Some("kiln"), &[], "");
    assert_eq!((o.code, o.stderr.as_str()), (1, "kiln: error: no input files\n"));

    // `--driver-mode=`: the last wins; the prefix stays the program's.
    let o = kiln_named(Some("kiln"), &["--driver-mode=g++", "-fsyntax-only", &c_src], "");
    assert_eq!((o.code, o.stderr), (0, warn("c", "c++").replace("kiln++:", "kiln:")));
    let o = kiln(&["--driver-mode=gcc", "-E", "-P", &c_src, "--driver-mode=gcc"], "");
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
    let o = kiln(&["--driver-mode=cl", &c_src], "");
    assert_eq!(
        (o.code, o.stderr.as_str()),
        (1, "kiln++: error: unsupported argument 'cl' to option '--driver-mode='\n")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// [except.spec]/5, [class.virtual]/2, [dcl.init.general]/16.9: clang 19's
/// stderr for `clang++ -std=c++26 -fsyntax-only -x c++ -` on
/// `EXC_SPEC_SOURCE`, fix-its after cv- and ref-qualifiers and the
/// initializer's range included (the corpus harness ignores both).
const EXC_SPEC_SOURCE: &str = r"struct A {
  void f() const noexcept;
  void g() & noexcept;
  void h() const volatile && noexcept;
  void i() volatile const noexcept;
};
void A::f() const {}
void A::g() & {}
void A::h() const volatile && {}
void A::i() volatile const {}
struct B { virtual void m() noexcept; };
struct C : B { void m(); };
void (*p)() noexcept = (void (*)())nullptr;
";
const EXC_SPEC_GOLDEN: &str = r"<stdin>:7:9: error: 'f' is missing exception specification 'noexcept'
    7 | void A::f() const {}
      |         ^
      |                   noexcept
<stdin>:2:8: note: previous declaration is here
    2 |   void f() const noexcept;
      |        ^
<stdin>:8:9: error: 'g' is missing exception specification 'noexcept'
    8 | void A::g() & {}
      |         ^
      |               noexcept
<stdin>:3:8: note: previous declaration is here
    3 |   void g() & noexcept;
      |        ^
<stdin>:9:9: error: 'h' is missing exception specification 'noexcept'
    9 | void A::h() const volatile && {}
      |         ^
      |                               noexcept
<stdin>:4:8: note: previous declaration is here
    4 |   void h() const volatile && noexcept;
      |        ^
<stdin>:10:9: error: 'i' is missing exception specification 'noexcept'
   10 | void A::i() volatile const {}
      |         ^
      |                            noexcept
<stdin>:5:8: note: previous declaration is here
    5 |   void i() volatile const noexcept;
      |        ^
<stdin>:12:21: error: exception specification of overriding function is more lax than base version
   12 | struct C : B { void m(); };
      |                     ^
<stdin>:11:25: note: overridden virtual function is here
   11 | struct B { virtual void m() noexcept; };
      |                         ^
<stdin>:13:8: error: cannot initialize a variable of type 'void (*)() noexcept' with an rvalue of type 'void (*)()': different exception specifications
   13 | void (*p)() noexcept = (void (*)())nullptr;
      |        ^               ~~~~~~~~~~~~~~~~~~~
6 errors generated.
";

#[test]
fn exception_spec_fixits_and_ranges_like_clang() {
    let o = kiln(&["-std=c++26", "-fsyntax-only", "-"], EXC_SPEC_SOURCE);
    assert_eq!((o.code, o.stderr.as_str()), (1, EXC_SPEC_GOLDEN));
}

/// [class.bit]/1, /3, [dcl.pre]/12: bit-field diagnostics underline the
/// width where clang does (zero width, non-integral field type, a
/// non-integral width, a static member or typedef with a width), and a
/// failed `static_assert` comparison's note underlines the whole
/// comparison (clang `DiagnoseStaticAssertDetails`) (WP-320).
const BIT_WIDTH_SOURCE: &str = r"struct A { int c : 0; int d : (0); float f : 3; int g : 2.0; };
struct C { static int s : 3; typedef int td : 2; };
constexpr struct D { unsigned u : 3; int s : 2; } d{5, 1};
static_assert(d.u == 4);
static_assert(d.s + 1 == 3);
";

/// clang 19's `-fsyntax-only` output for `BIT_WIDTH_SOURCE`.
const BIT_WIDTH_GOLDEN: &str = r"<stdin>:1:16: error: named bit-field 'c' has zero width
    1 | struct A { int c : 0; int d : (0); float f : 3; int g : 2.0; };
      |                ^   ~
<stdin>:1:27: error: named bit-field 'd' has zero width
    1 | struct A { int c : 0; int d : (0); float f : 3; int g : 2.0; };
      |                           ^   ~~~
<stdin>:1:42: error: bit-field 'f' has non-integral type 'float'
    1 | struct A { int c : 0; int d : (0); float f : 3; int g : 2.0; };
      |                                          ^   ~
<stdin>:1:57: error: integral constant expression must have integral or unscoped enumeration type, not 'double'
    1 | struct A { int c : 0; int d : (0); float f : 3; int g : 2.0; };
      |                                                         ^~~
<stdin>:2:23: error: static member 's' cannot be a bit-field
    2 | struct C { static int s : 3; typedef int td : 2; };
      |                       ^   ~
<stdin>:2:42: error: typedef member 'td' cannot be a bit-field
    2 | struct C { static int s : 3; typedef int td : 2; };
      |                                          ^    ~
<stdin>:4:15: error: static assertion failed due to requirement 'd.u == 4'
    4 | static_assert(d.u == 4);
      |               ^~~~~~~~
<stdin>:4:19: note: expression evaluates to '5 == 4'
    4 | static_assert(d.u == 4);
      |               ~~~~^~~~
<stdin>:5:15: error: static assertion failed due to requirement 'd.s + 1 == 3'
    5 | static_assert(d.s + 1 == 3);
      |               ^~~~~~~~~~~~
<stdin>:5:23: note: expression evaluates to '2 == 3'
    5 | static_assert(d.s + 1 == 3);
      |               ~~~~~~~~^~~~
8 errors generated.
";

#[test]
fn bit_width_and_static_assert_ranges_like_clang() {
    let o = kiln(&["-std=c++26", "-fsyntax-only", "-"], BIT_WIDTH_SOURCE);
    assert_eq!((o.code, o.stderr.as_str()), (1, BIT_WIDTH_GOLDEN));
}

/// [dcl.init.ref]/5: a reference binding that drops qualifiers underlines
/// the initializer, as clang streams its source range into
/// `err_reference_bind_drops_quals` (WP-818).
const DROPS_QUALS_SOURCE: &str = r"const int ci = 0;
volatile int vi = 0;
int &r1 = ci;
const int &r2 = (vi);
struct S { const volatile int m = 0; } s;
int &r3 = s.m;
int *const icp = nullptr;
const int *&r4 = icp;
";

/// clang 19's `-fsyntax-only` output for `DROPS_QUALS_SOURCE`.
const DROPS_QUALS_GOLDEN: &str = r"<stdin>:3:6: error: binding reference of type 'int' to value of type 'const int' drops 'const' qualifier
    3 | int &r1 = ci;
      |      ^    ~~
<stdin>:4:12: error: binding reference of type 'const int' to value of type 'volatile int' drops 'volatile' qualifier
    4 | const int &r2 = (vi);
      |            ^    ~~~~
<stdin>:6:6: error: binding reference of type 'int' to value of type 'const volatile int' drops 'const volatile' qualifiers
    6 | int &r3 = s.m;
      |      ^    ~~~
<stdin>:8:13: error: binding reference of type 'const int *' to value of type 'int *const' drops 'const' qualifier
    8 | const int *&r4 = icp;
      |             ^    ~~~
4 errors generated.
";

#[test]
fn reference_bind_drops_quals_ranges_like_clang() {
    let o = kiln(&["-std=c++26", "-fsyntax-only", "-"], DROPS_QUALS_SOURCE);
    assert_eq!((o.code, o.stderr.as_str()), (1, DROPS_QUALS_GOLDEN));
}

/// P2996 [expr.prim.splice]: an expression splice in a requires-clause
/// mangles as GCC 16's `DS <expression> [<template-args>] E`, not as an
/// error (WP-818; the reflection literal before it is still kiln's own).
#[test]
fn splice_in_requires_clause_mangles_like_gcc() {
    let src = "using info = decltype(^^int);
template <class T> constexpr bool Small = sizeof(T) <= 4;
template <info R, class T> requires (template [: R :]<T>) int pick() { return 1; }
constexpr int one = 1;
template <info R> requires ([: R :] == 1) int pick2() { return 2; }
int use() { return pick<^^Small, int>() + pick2<^^one>(); }
";
    let o = kiln(&["-std=c++26", "-freflection", "-S", "-emit-llvm", "-o", "-", "-"], src);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.contains("EiQDST_IT0_EEEiv("), "{}", o.stdout);
    assert!(o.stdout.contains("EQeqDST_ELi1EEiv("), "{}", o.stdout);
}

/// [conv.bool], [dcl.pre]/12: a -Wliteral-conversion range underlines a
/// whole `&&` or `||` operator, and a range whose ends come from a macro
/// argument covers its last token (clang `Lexer::getLocForEndOfToken`
/// on the spelling); a `static_assert` condition with cleanups (a
/// `__builtin_bit_cast` operand) still gets its "expression evaluates
/// to" note (WP-410).
const MACRO_RANGE_SOURCE: &str = r"bool f(bool x) { return x && 5.0; }
bool g(bool x) { return 5.0 || x; }
#define P(x) (x)
static_assert(P(2 + 1 == 4));
static_assert(__builtin_bit_cast(unsigned, 1.0f) == 0);
";

/// clang 19's and clang 22's `-fsyntax-only` output for
/// `MACRO_RANGE_SOURCE`.
const MACRO_RANGE_GOLDEN: &str = r"<stdin>:1:30: warning: implicit conversion from 'double' to 'bool' changes value from 5 to true [-Wliteral-conversion]
    1 | bool f(bool x) { return x && 5.0; }
      |                           ~~ ^~~
<stdin>:2:25: warning: implicit conversion from 'double' to 'bool' changes value from 5 to true [-Wliteral-conversion]
    2 | bool g(bool x) { return 5.0 || x; }
      |                         ^~~ ~~
<stdin>:4:17: error: static assertion failed due to requirement '2 + 1 == 4'
    4 | static_assert(P(2 + 1 == 4));
      |                 ^~~~~~~~~~
<stdin>:3:15: note: expanded from macro 'P'
    3 | #define P(x) (x)
      |               ^
<stdin>:4:23: note: expression evaluates to '3 == 4'
    4 | static_assert(P(2 + 1 == 4));
      |                 ~~~~~~^~~~
<stdin>:3:15: note: expanded from macro 'P'
    3 | #define P(x) (x)
      |               ^
<stdin>:5:15: error: static assertion failed due to requirement '__builtin_bit_cast(unsigned int, 1.F) == 0'
    5 | static_assert(__builtin_bit_cast(unsigned, 1.0f) == 0);
      |               ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
<stdin>:5:50: note: expression evaluates to '1065353216 == 0'
    5 | static_assert(__builtin_bit_cast(unsigned, 1.0f) == 0);
      |               ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~^~~~
2 warnings and 2 errors generated.
";

#[test]
fn literal_conversion_and_macro_argument_ranges_like_clang() {
    let o = kiln(&["-std=c++26", "-fsyntax-only", "-"], MACRO_RANGE_SOURCE);
    assert_eq!((o.code, o.stderr.as_str()), (1, MACRO_RANGE_GOLDEN));
}

/// [expr.log.and], [stmt.if], [cpp.replace]: fix-its are file edits
/// (clang `Lexer::makeFileCharRange`, `edit::Commit`): a location that
/// begins or ends every macro expansion it is in maps to the invocation,
/// a macro argument to where it is written; otherwise the removal of
/// `-Wconstant-logical-operand`'s constant (`G(5)`) and both
/// `-Wunreachable-code` silencing insertions (`X 1`) are dropped while
/// the notes stay (WP-419).
const MACRO_FIXIT_SOURCE: &str = r"void g();
bool h();
#define LHS a
#define ID(x) x
#define G(x) x; (void)0
#define NOT !
#define X h() && !
bool f1(int a) { return LHS && 5; }
bool f2(int a) { return ID(a) && ID(5); }
bool f3(int a) { return a && G(5); }
void u1() { if (NOT 1) g(); }
void u2() { if (X 1) g(); }
";

/// clang 19's and clang 22's `-fsyntax-only -Wunreachable-code` output
/// for `MACRO_FIXIT_SOURCE`.
const MACRO_FIXIT_GOLDEN: &str = r"<stdin>:8:29: warning: use of logical '&&' with constant operand [-Wconstant-logical-operand]
    8 | bool f1(int a) { return LHS && 5; }
      |                             ^  ~
<stdin>:8:29: note: use '&' for a bitwise operation
    8 | bool f1(int a) { return LHS && 5; }
      |                             ^~
      |                             &
<stdin>:8:29: note: remove constant to silence this warning
    8 | bool f1(int a) { return LHS && 5; }
      |                             ^~~~
<stdin>:9:31: warning: use of logical '&&' with constant operand [-Wconstant-logical-operand]
    9 | bool f2(int a) { return ID(a) && ID(5); }
      |                               ^     ~
<stdin>:9:31: note: use '&' for a bitwise operation
    9 | bool f2(int a) { return ID(a) && ID(5); }
      |                               ^~
      |                               &
<stdin>:9:31: note: remove constant to silence this warning
    9 | bool f2(int a) { return ID(a) && ID(5); }
      |                               ^~~~~~~~
<stdin>:10:27: warning: use of logical '&&' with constant operand [-Wconstant-logical-operand]
   10 | bool f3(int a) { return a && G(5); }
      |                           ^    ~
<stdin>:10:27: note: use '&' for a bitwise operation
   10 | bool f3(int a) { return a && G(5); }
      |                           ^~
      |                           &
<stdin>:10:27: note: remove constant to silence this warning
<stdin>:11:24: warning: code will never be executed [-Wunreachable-code]
   11 | void u1() { if (NOT 1) g(); }
      |                        ^
<stdin>:11:17: note: silence by adding parentheses to mark code as explicitly dead
   11 | void u1() { if (NOT 1) g(); }
      |                 ^    
      |                 /* DISABLES CODE */ ( )
<stdin>:6:13: note: expanded from macro 'NOT'
    6 | #define NOT !
      |             ^
<stdin>:12:22: warning: code will never be executed [-Wunreachable-code]
   12 | void u2() { if (X 1) g(); }
      |                      ^
<stdin>:12:17: note: silence by adding parentheses to mark code as explicitly dead
   12 | void u2() { if (X 1) g(); }
      |                 ^
<stdin>:7:18: note: expanded from macro 'X'
    7 | #define X h() && !
      |                  ^
5 warnings generated.
";

#[test]
fn fixits_in_macro_expansions_like_clang() {
    for compat in ["-fclang-compat=19", "-fclang-compat=22"] {
        let args = ["-std=c++26", "-fsyntax-only", compat, "-Wunreachable-code", "-"];
        let o = kiln(&args, MACRO_FIXIT_SOURCE);
        assert_eq!((o.code, o.stderr.as_str()), (0, MACRO_FIXIT_GOLDEN), "{compat}");
    }
}

/// [basic.indet]: the uninitialized-use warnings' fix-its (clang
/// `SuggestInitializationFixit` and the sometimes-uninitialized branch
/// notes, WP-862): ` = 0`, ` = nullptr`, ` = 0.0`, ` = false`, ` = '\0'`
/// after the declarator (after a function pointer's `)`), and the
/// branch-removal replacements for `if`, `&&`, `||`, `?:`, loops and
/// `switch` defaults.
const UNINIT_FIXIT_SOURCE: &str = r"int f1() { int i; return i; }
int *f2() { int *p; return p; }
double f3() { double d; return d; }
bool f4() { bool b; return b; }
char f5() { char c; return c; }
struct S { int m; };
int S::*f6() { int S::*mp; return mp; }
int f7(bool c) { int x; if (c) x = 1; return x; }
int f8(bool c) { int x; if (c) {} else x = 1; return x; }
int f9(bool c) { int x; if (c && (x = 1)) {} return x; }
int f10(bool c) { int x; if (c || (x = 1)) {} return x; }
int f11(bool c) { int x; c ? x = 1 : 0; return x; }
int f12(int n) { int x; for (int i = 0; i < n; ++i) x = i; return x; }
int f13(int n) { int x; while (n) { x = n; --n; } return x; }
int f14(int k) { int x; switch (k) { case 0: x = 1; break; default: break; } return x; }
int f15() { int x, y = 0; return x + y; }
int f16() { int (*fp)(int); return fp(0); }
";

/// clang 19's and clang 22's `-fsyntax-only -Wuninitialized` output for
/// `UNINIT_FIXIT_SOURCE`.
const UNINIT_FIXIT_GOLDEN: &str = r"<stdin>:1:26: warning: variable 'i' is uninitialized when used here [-Wuninitialized]
    1 | int f1() { int i; return i; }
      |                          ^
<stdin>:1:17: note: initialize the variable 'i' to silence this warning
    1 | int f1() { int i; return i; }
      |                 ^
      |                  = 0
<stdin>:2:28: warning: variable 'p' is uninitialized when used here [-Wuninitialized]
    2 | int *f2() { int *p; return p; }
      |                            ^
<stdin>:2:19: note: initialize the variable 'p' to silence this warning
    2 | int *f2() { int *p; return p; }
      |                   ^
      |                    = nullptr
<stdin>:3:32: warning: variable 'd' is uninitialized when used here [-Wuninitialized]
    3 | double f3() { double d; return d; }
      |                                ^
<stdin>:3:23: note: initialize the variable 'd' to silence this warning
    3 | double f3() { double d; return d; }
      |                       ^
      |                        = 0.0
<stdin>:4:28: warning: variable 'b' is uninitialized when used here [-Wuninitialized]
    4 | bool f4() { bool b; return b; }
      |                            ^
<stdin>:4:19: note: initialize the variable 'b' to silence this warning
    4 | bool f4() { bool b; return b; }
      |                   ^
      |                    = false
<stdin>:5:28: warning: variable 'c' is uninitialized when used here [-Wuninitialized]
    5 | char f5() { char c; return c; }
      |                            ^
<stdin>:5:19: note: initialize the variable 'c' to silence this warning
    5 | char f5() { char c; return c; }
      |                   ^
      |                    = '\0'
<stdin>:7:35: warning: variable 'mp' is uninitialized when used here [-Wuninitialized]
    7 | int S::*f6() { int S::*mp; return mp; }
      |                                   ^~
<stdin>:7:26: note: initialize the variable 'mp' to silence this warning
    7 | int S::*f6() { int S::*mp; return mp; }
      |                          ^
      |                           = nullptr
<stdin>:8:29: warning: variable 'x' is used uninitialized whenever 'if' condition is false [-Wsometimes-uninitialized]
    8 | int f7(bool c) { int x; if (c) x = 1; return x; }
      |                             ^
<stdin>:8:46: note: uninitialized use occurs here
    8 | int f7(bool c) { int x; if (c) x = 1; return x; }
      |                                              ^
<stdin>:8:25: note: remove the 'if' if its condition is always true
    8 | int f7(bool c) { int x; if (c) x = 1; return x; }
      |                         ^~~~~~
<stdin>:8:23: note: initialize the variable 'x' to silence this warning
    8 | int f7(bool c) { int x; if (c) x = 1; return x; }
      |                       ^
      |                        = 0
<stdin>:9:29: warning: variable 'x' is used uninitialized whenever 'if' condition is true [-Wsometimes-uninitialized]
    9 | int f8(bool c) { int x; if (c) {} else x = 1; return x; }
      |                             ^
<stdin>:9:54: note: uninitialized use occurs here
    9 | int f8(bool c) { int x; if (c) {} else x = 1; return x; }
      |                                                      ^
<stdin>:9:25: note: remove the 'if' if its condition is always false
    9 | int f8(bool c) { int x; if (c) {} else x = 1; return x; }
      |                         ^~~~~~~~~~~~~~
<stdin>:9:23: note: initialize the variable 'x' to silence this warning
    9 | int f8(bool c) { int x; if (c) {} else x = 1; return x; }
      |                       ^
      |                        = 0
<stdin>:10:29: warning: variable 'x' is used uninitialized whenever '&&' condition is false [-Wsometimes-uninitialized]
   10 | int f9(bool c) { int x; if (c && (x = 1)) {} return x; }
      |                             ^
<stdin>:10:53: note: uninitialized use occurs here
   10 | int f9(bool c) { int x; if (c && (x = 1)) {} return x; }
      |                                                     ^
<stdin>:10:29: note: remove the '&&' if its condition is always true
   10 | int f9(bool c) { int x; if (c && (x = 1)) {} return x; }
      |                             ^~~~
<stdin>:10:23: note: initialize the variable 'x' to silence this warning
   10 | int f9(bool c) { int x; if (c && (x = 1)) {} return x; }
      |                       ^
      |                        = 0
<stdin>:11:30: warning: variable 'x' is used uninitialized whenever '||' condition is true [-Wsometimes-uninitialized]
   11 | int f10(bool c) { int x; if (c || (x = 1)) {} return x; }
      |                              ^
<stdin>:11:54: note: uninitialized use occurs here
   11 | int f10(bool c) { int x; if (c || (x = 1)) {} return x; }
      |                                                      ^
<stdin>:11:30: note: remove the '||' if its condition is always false
   11 | int f10(bool c) { int x; if (c || (x = 1)) {} return x; }
      |                              ^~~~
<stdin>:11:24: note: initialize the variable 'x' to silence this warning
   11 | int f10(bool c) { int x; if (c || (x = 1)) {} return x; }
      |                        ^
      |                         = 0
<stdin>:12:26: warning: variable 'x' is used uninitialized whenever '?:' condition is false [-Wsometimes-uninitialized]
   12 | int f11(bool c) { int x; c ? x = 1 : 0; return x; }
      |                          ^
<stdin>:12:48: note: uninitialized use occurs here
   12 | int f11(bool c) { int x; c ? x = 1 : 0; return x; }
      |                                                ^
<stdin>:12:26: note: remove the '?:' if its condition is always true
   12 | int f11(bool c) { int x; c ? x = 1 : 0; return x; }
      |                          ^~~       ~~~
<stdin>:12:24: note: initialize the variable 'x' to silence this warning
   12 | int f11(bool c) { int x; c ? x = 1 : 0; return x; }
      |                        ^
      |                         = 0
<stdin>:15:60: warning: variable 'x' is used uninitialized whenever switch default is taken [-Wsometimes-uninitialized]
   15 | int f14(int k) { int x; switch (k) { case 0: x = 1; break; default: break; } return x; }
      |                                                            ^~~~~~~
<stdin>:15:85: note: uninitialized use occurs here
   15 | int f14(int k) { int x; switch (k) { case 0: x = 1; break; default: break; } return x; }
      |                                                                                     ^
<stdin>:15:23: note: initialize the variable 'x' to silence this warning
   15 | int f14(int k) { int x; switch (k) { case 0: x = 1; break; default: break; } return x; }
      |                       ^
      |                        = 0
<stdin>:16:34: warning: variable 'x' is uninitialized when used here [-Wuninitialized]
   16 | int f15() { int x, y = 0; return x + y; }
      |                                  ^
<stdin>:16:18: note: initialize the variable 'x' to silence this warning
   16 | int f15() { int x, y = 0; return x + y; }
      |                  ^
      |                   = 0
<stdin>:17:36: warning: variable 'fp' is uninitialized when used here [-Wuninitialized]
   17 | int f16() { int (*fp)(int); return fp(0); }
      |                                    ^~
<stdin>:17:27: note: initialize the variable 'fp' to silence this warning
   17 | int f16() { int (*fp)(int); return fp(0); }
      |                           ^
      |                            = nullptr
14 warnings generated.
";

#[test]
fn uninit_fixits_like_clang() {
    for compat in ["-fclang-compat=19", "-fclang-compat=22"] {
        let args = ["-std=c++26", "-fsyntax-only", compat, "-Wuninitialized", "-"];
        let o = kiln(&args, UNINIT_FIXIT_SOURCE);
        assert_eq!((o.code, o.stderr.as_str()), (0, UNINIT_FIXIT_GOLDEN), "{compat}");
    }
}

#[test]
fn module_missing_import_notes() {
    // [module.reach]: each missing-import error of
    // tests/conformance/module.reach/missing-import.cpp carries clang 22's
    // note at the declaration in the module source; the corpus matches
    // only notes located in the test file, so they are checked here.
    let dir = repo().join("tests/conformance/module.reach");
    let input = |f: &str| dir.join("Inputs").join(f).display().to_string();
    let args = [
        "-std=c++20".to_owned(),
        "-fsyntax-only".to_owned(),
        format!("-fmodule-file=mr.a={}", input("mr-a.cppm")),
        format!("-fmodule-file=mr.b={}", input("mr-b.cppm")),
        format!("-fmodule-file=mr.b:part={}", input("mr-p.cppm")),
        dir.join("missing-import.cpp").display().to_string(),
    ];
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let o = kiln(&args, "");
    assert_eq!(o.code, 1, "{}", o.stderr);
    let notes: Vec<&str> = o
        .stderr
        .lines()
        .filter(|l| l.contains(": note: "))
        .map(|l| l.rsplit('/').next().unwrap_or(l))
        .collect();
    let at = |l: &str| format!("{l}: note: declaration here is not visible");
    let want: Vec<String> = [
        "mr-a.cppm:5:12",
        "mr-p.cppm:4:12",
        "mr-b.cppm:6:5",
        "mr-b.cppm:8:5",
        "mr-b.cppm:8:5",
        "mr-b.cppm:18:5",
        "mr-b.cppm:21:5",
        "mr-b.cppm:9:8",
        "mr-b.cppm:19:8",
    ]
    .into_iter()
    .map(at)
    .collect();
    assert_eq!(notes, want, "{}", o.stderr);
}

#[test]
fn module_adl_example() {
    // [basic.lookup.argdep]/4-5: in the standard example's instantiation
    // of `apply` (tests/conformance/basic.lookup.argdep/module-example.cpp)
    // ADL does not find R::g, internal to module N's unit; clang 22
    // reports it in N's source with the instantiation note, which the
    // corpus cannot match (its primary is in Inputs/).
    let dir = repo().join("tests/conformance/basic.lookup.argdep");
    let input = |f: &str| dir.join("Inputs").join(f).display().to_string();
    let args = [
        "-std=c++20".to_owned(),
        "-fsyntax-only".to_owned(),
        format!("-fmodule-file=M={}", input("adl-m.cppm")),
        format!("-fmodule-file=N={}", input("adl-n.cppm")),
        format!("-fmodule-file=Q={}", input("adl-q.cppm")),
        dir.join("module-example.cpp").display().to_string(),
    ];
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let o = kiln(&args, "");
    assert_eq!(o.code, 1, "{}", o.stderr);
    let lines: Vec<&str> = o
        .stderr
        .lines()
        .filter(|l| l.contains(": error: ") || l.contains(": note: "))
        .map(|l| l.rsplit('/').next().unwrap_or(l))
        .collect();
    let want = [
        "module-example.cpp:27:6: error: no type named 'f' in namespace 'R'",
        "module-example.cpp:29:3: error: no matching function for call to 'f'",
        "adl-m.cppm:5:15: note: candidate function not viable: requires 1 argument, but 2 were provided",
        "adl-n.cppm:8:3: error: use of undeclared identifier 'g'",
        "module-example.cpp:31:3: note: in instantiation of function template specialization 'apply<R::X, S::Z>' requested here",
    ];
    assert_eq!(lines, want, "{}", o.stderr);
}

/// WP-469, spec/driver.md §Target features: clang 22's driver errors for
/// `-march=` / `-msve-vector-bits=` values it rejects, and the
/// `__ARM_FEATURE_SVE_BITS` they set when accepted.
#[test]
fn aarch64_target_feature_options() {
    let a64 = "--target=aarch64-unknown-linux-gnu";
    let src = "int x = __ARM_FEATURE_SVE_BITS;\n";
    let run = |args: &[&str]| {
        let o = kiln(&[&["-E", "-P", "-x", "c++", "-"], args].concat(), src);
        (o.code, o.stderr, o.stdout)
    };
    let err = |m: &str| (1, format!("kiln++: error: {m}\n"), String::new());
    assert_eq!(
        run(&["--target=x86_64-unknown-linux-gnu", "-msve-vector-bits=256"]),
        err("unsupported option '-msve-vector-bits=' for target 'x86_64-unknown-linux-gnu'")
    );
    assert_eq!(
        run(&[a64, "-msve-vector-bits=100"]),
        err("unsupported argument '100' to option '-msve-vector-bits='")
    );
    assert_eq!(
        run(&[a64, "-march=armv8-a+bogus"]),
        err("unsupported argument 'armv8-a+bogus' to option '-march='")
    );
    assert_eq!(
        run(&[a64, "-march=armv8-a+sve", "-msve-vector-bits=512"]),
        (0, String::new(), "int x = 512;\n".to_owned())
    );
}

/// `-fkiln-memory-report` (spec/driver.md): after the diagnostics, one
/// `memory:` line per producer row, then the total. The rows come from
/// the AST, the type context, sema and the Oven; no numbers are pinned.
#[test]
fn memory_report_rows() {
    let src = "template <class T> constexpr T sq(T x) { return x * x; }\n\
               static_assert(sq(3) == 9);\n\
               int f();\nint f();\nint g() { return sq<int>(f()); }\n";
    for mode in [&["-fsyntax-only"][..], &["-S", "-emit-llvm", "-o", "-"][..]] {
        let mut args = vec!["-std=c++26", "-fkiln-memory-report"];
        args.extend_from_slice(mode);
        args.extend_from_slice(&["-x", "c++", "-"]);
        let o = kiln(&args, src);
        assert_eq!(o.code, 0, "{mode:?}: {}", o.stderr);
        let lines: Vec<&str> = o.stderr.lines().collect();
        assert!(lines.iter().all(|l| l.starts_with("memory: ")), "{mode:?}: {}", o.stderr);
        let row = |name: &str| {
            let p = format!("memory: {name} count=");
            lines
                .iter()
                .any(|l| l.starts_with(&p) && l.contains(" live=") && l.contains(" reserved="))
        };
        for name in [
            "sema.redecl_index",
            "sema.explicit_memo",
            "journal.lookup",
            "instantiation.retained",
            "oven.code",
        ] {
            assert!(row(name), "{mode:?}: missing {name}: {}", o.stderr);
        }
        let prefixed = |p: &str| lines.iter().any(|l| l.starts_with(&format!("memory: {p}")));
        assert!(prefixed("types."), "{mode:?}: no type row: {}", o.stderr);
        assert!(prefixed("decls"), "{mode:?}: no AST row: {}", o.stderr);
        let last = lines.last().copied().unwrap_or_default();
        assert!(last.starts_with("memory: total live=") && last.contains(" reserved="), "{last}");
    }
    // Without the flag nothing is printed.
    let o = kiln(&["-std=c++26", "-fsyntax-only", "-x", "c++", "-"], src);
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
}

/// spec/oven.md "Trial folds" (WP-798): a failed outermost trial fold
/// gives its thunk's code back, so the `-Winteger-overflow` scan
/// ([expr.pre]/4) of many non-constant operands keeps no thunk per
/// operand alive (`oven.thunks` stays small; it held one per `+` before).
#[test]
fn failed_trial_folds_keep_no_thunks() {
    let mut src = String::from("int f(int);\nint g(int);\nint use(int x) {\n  int r = 0;\n");
    for _ in 0..200 {
        src.push_str("  r += f(x) + g(x);\n");
    }
    src.push_str("  return r;\n}\n");
    let o = kiln(&["-std=c++26", "-fkiln-memory-report", "-fsyntax-only", "-x", "c++", "-"], &src);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let count = o
        .stderr
        .lines()
        .find_map(|l| l.strip_prefix("memory: oven.thunks count="))
        .and_then(|r| r.split_whitespace().next())
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or_else(|| panic!("no oven.thunks row: {}", o.stderr));
    assert!(count < 20, "{count} thunks kept for 200 failed folds");
}

/// spec/sema.md "-Winteger-overflow" (WP-798): an operand that calls a
/// non-`constexpr` member function on every evaluation ([expr.const]/10.2)
/// is not folded at all, so the scan of a body made of them compiles no
/// thunk; an overflow beside one is still diagnosed.
#[test]
fn overflow_scan_skips_folds_that_cannot_succeed() {
    let mut src = String::from(
        "struct A { int m; int get() const { return m; } };\n\
         struct B { A *a; A *operator->() const { return a; } };\n\
         struct C { B b; B operator->() const { return b; } };\n\
         int use(const C &p) {\n  int r = 0;\n",
    );
    for _ in 0..50 {
        src.push_str("  r += p->m + p->get();\n");
    }
    src.push_str("  return r + (2147483647 + 1);\n}\n");
    let o = kiln(&["-std=c++26", "-fkiln-memory-report", "-fsyntax-only", "-x", "c++", "-"], &src);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let warns = o.stderr.matches("warning: overflow in expression; result is").count();
    assert_eq!(warns, 1, "{}", o.stderr);
    assert!(o.stderr.contains("memory: oven.thunks count=0 "), "{}", o.stderr);
}

/// spec/driver.md "Output, diagnostics, exit status" (WP-682): a compile
/// thread that cannot be created (its stack is reserved up front; here one
/// larger than any address space) is reported, not a silent exit 70.
#[test]
fn compile_thread_spawn_failure_is_diagnosed() {
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(["-fkiln-stack-size=1000000000g", "-fsyntax-only", "-x", "c++", "-"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(70), "{err}");
    assert!(
        err.starts_with(
            "kiln: error: cannot create the compile thread with a 1073741824000000000-byte stack: "
        ),
        "{err}"
    );
    assert!(err.ends_with("(lower it with -fkiln-stack-size=)\n"), "{err}");
}

/// The debug information flags end to end (spec/driver.md "Debug
/// information flags"): the last level wins, `-gdwarf-N` enables `-g` and
/// sets the version, `-fdebug-prefix-map=` rewrites the directory, and an
/// unimplemented flag is an error rather than a silent drop.
#[test]
fn debug_info_flags_reach_the_ir() {
    let src = "int f(int a) { int b = a + 1; return b; }\n";
    let ir = |flags: &[&str]| {
        let mut args = vec!["-S", "-emit-llvm", "-o", "-", "-x", "c++"];
        args.extend_from_slice(flags);
        args.push("-");
        let o = kiln(&args, src);
        assert_eq!((o.code, o.stderr.as_str()), (0, ""), "{flags:?}");
        o.stdout
    };
    let full = ir(&["-g"]);
    assert!(full.contains("emissionKind: FullDebug"), "{full}");
    assert!(full.contains("!DILocalVariable(name: \"b\""), "{full}");
    assert!(full.contains("#dbg_declare("), "{full}");
    assert!(full.contains("!{i32 7, !\"Dwarf Version\", i32 5}"), "{full}");
    for off in [&["-g", "-g0"][..], &[]] {
        let o = ir(off);
        assert!(!o.contains("!llvm.dbg.cu") && !o.contains("!dbg"), "{off:?}: {o}");
    }
    let lines = ir(&["-g", "-gline-tables-only"]);
    assert!(lines.contains("emissionKind: LineTablesOnly"), "{lines}");
    assert!(!lines.contains("DILocalVariable"), "{lines}");
    let v4 = ir(&["-gdwarf-4"]);
    assert!(v4.contains("emissionKind: FullDebug"), "{v4}");
    assert!(v4.contains("!{i32 7, !\"Dwarf Version\", i32 4}"), "{v4}");
    let mapped = ir(&["-g", "-fdebug-prefix-map=/=/mapped/"]);
    assert!(mapped.contains("directory: \"/mapped/"), "{mapped}");
    let o = kiln(&["-g", "-gcodeview", "-S", "-emit-llvm", "-o", "-", "-x", "c++", "-"], src);
    assert_eq!(o.code, 1);
    assert_eq!(o.stderr, "kiln++: error: '-gcodeview' is not supported by kiln\n");
}
