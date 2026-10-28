//! The `-E` printer ([`kiln_pp::print`]): line-marker mode and `-dM`.
//!
//! Goldens: `clang++-19 -std=c++26 -E` (hand-checked: clang's
//! `<built-in>` / `<command line>` markers and the `-I` directory prefix
//! of header names are left out; see `spec/pp.md`).

mod common;

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, Std};
use kiln_pp::{MemoryResolver, PpOptions, Preprocessor, PrintOptions, print_macros};
use kiln_source::SourceManager;

const MARKERS: PrintOptions = PrintOptions { line_markers: true, line_directives: false };

fn headers() -> MemoryResolver {
    MemoryResolver::new()
        .with_header("a.h", "h1\n#include \"b.h\"\nh2\n")
        .with_header("b.h", "b\n")
        .with_system_header("s.h", "sys\n")
}

const SRC: &str = "#include \"a.h\"\nmain1\n\n\n\n\n\n\n\n\n\nmain2\n#include <s.h>\n\
#line 100 \"z.cpp\"\nz\n# 7 \"q.h\" 1\nq\n";

/// [cpp.include]/1, [cpp.line]/3
///
/// Entering and leaving headers (flags 1 / 2, 3 for system headers), a
/// jump of more than 8 lines, `#line` and a GNU line marker.
#[test]
fn line_markers() {
    common::expect(
        &common::run_with(SRC, headers(), MARKERS, |_| {}),
        r#"# 1 "t.cpp"
# 1 "<built-in>" 1
# 1 "t.cpp" 2
# 1 "a.h" 1
h1
# 1 "b.h" 1
b
# 3 "a.h" 2
h2
# 2 "t.cpp" 2
main1
# 12 "t.cpp"
main2
# 1 "s.h" 1 3
sys
# 14 "t.cpp" 2
# 100 "z.cpp"
z
# 7 "q.h" 1
q
"#,
        &[],
    );
}

/// [cpp.line]/3
///
/// GNU line-marker flags are reprinted: 1 (enter), 2 (exit), 3 (system).
#[test]
fn line_marker_flags() {
    let o = common::run_with(
        "x\n# 7 \"q.h\" 1\nq\n# 9 \"t.cpp\" 2\nr\n# 20 \"s.h\" 3\ns\n",
        MemoryResolver::new(),
        MARKERS,
        |_| {},
    );
    common::expect(
        &o,
        "# 1 \"t.cpp\"\n# 1 \"<built-in>\" 1\n# 1 \"t.cpp\" 2\nx\n# 7 \"q.h\" 1\nq\n# 9 \"t.cpp\" 2\nr\n# 20 \"s.h\" 3\ns\n",
        &[],
    );
}

/// [cpp.line]/3
///
/// clang `-fuse-line-directives`.
#[test]
fn line_directives() {
    let o = common::run_with(
        "a\n#include \"b.h\"\nc\n",
        headers(),
        PrintOptions { line_markers: true, line_directives: true },
        |_| {},
    );
    common::expect(
        &o,
        "#line 1 \"t.cpp\"\n#line 1 \"<built-in>\"\n#line 1 \"t.cpp\"\na\n#line 1 \"b.h\"\nb\n#line 3 \"t.cpp\"\nc\n",
        &[],
    );
}

/// [cpp.pre]/1
///
/// Short line gaps are printed as blank lines, long ones as a marker.
#[test]
fn line_gaps() {
    let o =
        common::run_with("a\n\n\nb\n\n\n\n\n\n\n\n\n\nc\n", MemoryResolver::new(), MARKERS, |_| {});
    common::expect(
        &o,
        "# 1 \"t.cpp\"\n# 1 \"<built-in>\" 1\n# 1 \"t.cpp\" 2\na\n\n\nb\n# 14 \"t.cpp\"\nc\n",
        &[],
    );
}

/// `-dM`: every macro still defined, builtins excluded, sorted.
fn dump_macros(src: &str) -> String {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let mut pp = Preprocessor::new(&sm, &engine, &interner, PpOptions::new(Std::Cxx26));
    pp.enter_main_file(main);
    let mut out = String::new();
    print_macros(&mut pp, &mut out);
    out
}

/// [cpp.replace.general]
#[test]
fn print_macros_dm() {
    let out = dump_macros(
        "#define B(x, ...) x + __VA_ARGS__\n#define A  1   +  2\n#define C\n#undef C\n",
    );
    let mine: Vec<_> =
        out.lines().filter(|l| l.starts_with("#define A") || l.starts_with("#define B")).collect();
    assert_eq!(mine, ["#define A 1 + 2", "#define B(x,...) x + __VA_ARGS__"]);
    assert!(!out.contains("#define C"), "{out}");
    assert!(!out.contains("__LINE__"), "builtins are not printed:\n{out}");
    let mut sorted: Vec<_> = out.lines().collect();
    sorted.sort_unstable();
    assert_eq!(sorted, out.lines().collect::<Vec<_>>());
}
