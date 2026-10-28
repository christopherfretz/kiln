//! Header search ([`HeaderSearch`], [`SearchList`]) on a virtual file
//! system: clang 19's search order, `#include_next`, `__has_include`,
//! `#pragma once` across paths, the depth limit, system / extern "C"
//! classification and `#embed` lookup.
//!
//! Hand-written against clang 19's `InitHeaderSearch` /
//! `HeaderSearch::LookupFile` behavior (expected outputs checked with
//! `clang -E` on equivalent trees).

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, Std};
use kiln_pp::{
    DirGroup, HeaderFs, HeaderSearch, MemoryFs, PpOptions, Preprocessor, PrintOptions,
    SearchDirSpec, SearchList, print_preprocessed,
};
use kiln_source::SourceManager;

fn specs(list: &[(&str, DirGroup)]) -> Vec<SearchDirSpec> {
    list.iter().map(|&(p, g)| SearchDirSpec::new(p, g)).collect()
}

/// Preprocesses `main` (a file of `fs`) with search dirs `dirs`; returns
/// the output (with line markers if `markers`) and the diagnostics.
fn run_with(
    mut fs: MemoryFs,
    main: &str,
    dirs: &[(&str, DirGroup)],
    markers: bool,
) -> (String, Vec<String>) {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let list = SearchList::build(&specs(dirs), true, &mut fs);
    let main = fs.open(&sm, main).expect("main file");
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let mut pp = Preprocessor::new(&sm, &engine, &interner, PpOptions::new(Std::Cxx26));
    pp.set_include_resolver(Box::new(
        HeaderSearch::new(fs, list).with_embed_dirs(vec!["res".into()]),
    ));
    pp.enter_main_file(main);
    let mut out = String::new();
    print_preprocessed(
        &mut pp,
        PrintOptions { line_markers: markers, line_directives: false },
        &mut out,
    );
    drop(pp);
    (out, sink.take().iter().map(|d| d.message.clone()).collect())
}

fn run(fs: MemoryFs, main: &str, dirs: &[(&str, DirGroup)]) -> String {
    let (out, diags) = run_with(fs, main, dirs, false);
    assert!(diags.is_empty(), "{diags:?}");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The file-entry line markers (`# 1 "name" 1...`) of a run.
fn entries(out: &str) -> Vec<String> {
    out.lines()
        .filter(|l| l.starts_with("# 1 \"") && l.contains("\" 1") && !l.contains('<'))
        .map(str::to_owned)
        .collect()
}

#[test]
fn search_list_order_and_dedup() {
    let mut fs = MemoryFs::new()
        .with_dir("/q")
        .with_dir("/u")
        .with_dir("/s")
        .with_dir("/c")
        .with_dir("/a")
        .with_dir("/cxx")
        .with_dir("/conly");
    let l = SearchList::build(
        &specs(&[
            ("/a", DirGroup::After),
            ("/u", DirGroup::Angled),
            ("/missing", DirGroup::Angled),
            ("/q", DirGroup::Quoted),
            ("/c", DirGroup::ExternCSystem),
            ("/s", DirGroup::System),
            ("/cxx", DirGroup::CxxSystem),
            ("/conly", DirGroup::CSystem),
            ("/q", DirGroup::Quoted),
            ("/u/.", DirGroup::Angled),
        ]),
        true,
        &mut fs,
    );
    assert_eq!(
        l.verbose_text(),
        "ignoring nonexistent directory \"/missing\"\n\
         ignoring duplicate directory \"/q\"\n\
         ignoring duplicate directory \"/u/.\"\n\
         #include \"...\" search starts here:\n /q\n\
         #include <...> search starts here:\n /u\n /c\n /s\n /cxx\n /a\n\
         End of search list.\n"
    );
    assert_eq!(l.angled_start(), 1);
}

#[test]
fn user_dir_duplicating_system_dir_is_dropped() {
    let mut fs = MemoryFs::new().with_dir("/x").with_dir("/y");
    let l = SearchList::build(
        &specs(&[("/x", DirGroup::Angled), ("/y", DirGroup::Angled), ("/x", DirGroup::System)]),
        true,
        &mut fs,
    );
    assert_eq!(
        l.verbose_text(),
        "ignoring duplicate directory \"/x\"\n  as it is a non-system directory that duplicates a system directory\n\
         #include \"...\" search starts here:\n#include <...> search starts here:\n /y\n /x\n\
         End of search list.\n"
    );
    // A system dir duplicated by a later user dir: the user one goes.
    let l = SearchList::build(
        &specs(&[("/x", DirGroup::System), ("/x", DirGroup::After)]),
        false,
        &mut fs,
    );
    assert_eq!(l.dirs().len(), 1);
}

#[test]
fn quoted_finds_includer_dir_first() {
    let fs = MemoryFs::new()
        .with_file("t.cpp", "#include \"sub/a.h\"\n")
        .with_file("sub/a.h", "#include \"b.h\"\n")
        .with_file("sub/b.h", "in_sub_b\n")
        .with_file("inc/b.h", "in_inc_b\n");
    assert_eq!(run(fs.clone(), "t.cpp", &[("inc", DirGroup::Angled)]), "in_sub_b");
    let (out, _) = run_with(fs, "t.cpp", &[("inc", DirGroup::Angled)], true);
    assert_eq!(entries(&out), ["# 1 \"./sub/a.h\" 1", "# 1 \"./sub/b.h\" 1"]);
}

#[test]
fn angled_skips_includer_dir_and_quote_dirs() {
    let fs = MemoryFs::new()
        .with_file("t.cpp", "#include <b.h>\n#include \"c.h\"\n")
        .with_file("b.h", "local_b\n")
        .with_file("q/b.h", "quote_b\n")
        .with_file("q/c.h", "quote_c\n")
        .with_file("i/b.h", "angled_b\n");
    assert_eq!(
        run(fs, "t.cpp", &[("q", DirGroup::Quoted), ("i", DirGroup::Angled)]),
        "angled_b quote_c"
    );
}

#[test]
fn search_order_across_groups() {
    let fs = MemoryFs::new()
        .with_file("t.cpp", "#include <h1.h>\n#include <h2.h>\n#include <h3.h>\n")
        .with_file("after/h1.h", "after1\n")
        .with_file("sys/h1.h", "sys1\n")
        .with_file("user/h1.h", "user1\n")
        .with_file("after/h2.h", "after2\n")
        .with_file("sys/h2.h", "sys2\n")
        .with_file("after/h3.h", "after3\n");
    let dirs = [("after", DirGroup::After), ("sys", DirGroup::System), ("user", DirGroup::Angled)];
    assert_eq!(run(fs, "t.cpp", &dirs), "user1 sys2 after3");
}

#[test]
fn include_next_continues_after_found_dir() {
    let fs = MemoryFs::new()
        .with_file("t.cpp", "#include <w.h>\n")
        .with_file("a/w.h", "a_w\n#include_next <w.h>\n")
        .with_file("b/w.h", "b_w\n#include_next \"w.h\"\n")
        .with_file("c/w.h", "c_w\n");
    let dirs = [("a", DirGroup::Angled), ("b", DirGroup::System), ("c", DirGroup::After)];
    assert_eq!(run(fs.clone(), "t.cpp", &dirs), "a_w b_w c_w");
    let (out, _) = run_with(fs, "t.cpp", &dirs, true);
    assert_eq!(entries(&out), ["# 1 \"a/w.h\" 1", "# 1 \"b/w.h\" 1 3", "# 1 \"c/w.h\" 1 3"]);
}

#[test]
fn include_next_in_main_file_searches_from_start() {
    let fs =
        MemoryFs::new().with_file("t.cpp", "#include_next <w.h>\n").with_file("a/w.h", "a_w\n");
    let (out, diags) = run_with(fs, "t.cpp", &[("a", DirGroup::Angled)], false);
    assert_eq!(out.trim(), "a_w");
    assert_eq!(
        diags,
        ["#include_next in primary source file; will search from start of include path"]
    );
}

#[test]
fn include_next_from_relatively_found_header() {
    let fs = MemoryFs::new()
        .with_file("t.cpp", "#include \"r.h\"\n")
        .with_file("r.h", "#include_next <x.h>\n")
        .with_file("a/x.h", "a_x\n");
    let (out, diags) = run_with(fs, "t.cpp", &[("a", DirGroup::Angled)], false);
    assert_eq!(out.trim(), "a_x");
    assert_eq!(
        diags,
        [
            "#include_next in file found relative to primary source file or found by absolute path; will search from start of include path"
        ]
    );
}

#[test]
fn has_include_and_has_include_next() {
    let fs = MemoryFs::new()
        .with_file(
            "t.cpp",
            "#include <h.h>\n\
             #if __has_include(<h.h>) && __has_include(\"loc.h\") && !__has_include(<loc.h>)\nyes\n#endif\n\
             #if __has_include(<nope.h>)\nbad\n#endif\n",
        )
        .with_file("loc.h", "")
        .with_file("a/h.h", "#if __has_include_next(<h.h>)\nnext_ok\n#endif\n#if __has_include_next(<only_a.h>)\nbad\n#endif\n")
        .with_file("a/only_a.h", "")
        .with_file("b/h.h", "");
    assert_eq!(
        run(fs, "t.cpp", &[("a", DirGroup::Angled), ("b", DirGroup::Angled)]),
        "next_ok yes"
    );
}

#[test]
fn pragma_once_by_file_identity() {
    let fs = MemoryFs::new()
        .with_file(
            "t.cpp",
            "#include \"inc/o.h\"\n#include <o.h>\n#include \"inc/../inc/o.h\"\nend\n",
        )
        .with_file("inc/o.h", "#pragma once\nonce\n");
    assert_eq!(run(fs, "t.cpp", &[("inc", DirGroup::Angled)]), "once end");
}

#[test]
fn depth_limit() {
    let fs = MemoryFs::new()
        .with_file("t.cpp", "#include \"r.h\"\n")
        .with_file("r.h", "#include \"r.h\"\n");
    let (_, diags) = run_with(fs, "t.cpp", &[], false);
    assert_eq!(diags, ["#include nested too deeply"]);
}

#[test]
fn not_found_and_absolute() {
    let fs = MemoryFs::new()
        .with_file("t.cpp", "#include \"/abs/x.h\"\n#include <missing.h>\nafter\n")
        .with_file("/abs/x.h", "abs_x\n");
    let (out, diags) = run_with(fs, "t.cpp", &[], false);
    assert_eq!(diags, ["'missing.h' file not found"]);
    assert!(out.contains("abs_x"));
}

#[test]
fn system_and_extern_c_flags() {
    let fs = MemoryFs::new()
        .with_file("t.cpp", "#include <s.h>\n#include <c.h>\n#include \"u.h\"\n")
        .with_file("sys/s.h", "#include \"rel.h\"\n")
        .with_file("sys/rel.h", "rel\n")
        .with_file("ext/c.h", "#include <s2.h>\n")
        .with_file("sys/s2.h", "s2\n")
        .with_file("u.h", "u\n");
    let (out, _) =
        run_with(fs, "t.cpp", &[("sys", DirGroup::System), ("ext", DirGroup::ExternCSystem)], true);
    assert_eq!(
        entries(&out),
        [
            "# 1 \"sys/s.h\" 1 3",
            "# 1 \"sys/rel.h\" 1 3",
            "# 1 \"ext/c.h\" 1 3 4",
            "# 1 \"sys/s2.h\" 1 3 4",
            "# 1 \"./u.h\" 1",
        ]
    );
    // Returning to an extern "C" header keeps flags 3 4.
    assert!(out.contains("# 2 \"ext/c.h\" 2 3 4"), "{out}");
}

#[test]
fn embed_lookup() {
    let mut fs = MemoryFs::new()
        .with_file(
            "src/t.cpp",
            "#embed \"d.bin\"\n;\n#embed <r.bin>\n;\n#if __has_embed(<d.bin>) == 0\nno_angled\n#endif\n",
        );
    fs.add_file("src/d.bin", b"AB");
    fs.add_file("res/r.bin", b"C");
    fs.add_file("res/../d.bin", b"Z");
    let (out, diags) = run_with(fs, "src/t.cpp", &[], false);
    assert_eq!(diags, ["#embed is a Clang extension", "#embed is a Clang extension"]);
    assert_eq!(out.split_whitespace().collect::<String>(), "65,66;67;no_angled");
}
