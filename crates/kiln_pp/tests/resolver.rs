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
        PrintOptions { line_markers: markers, line_directives: false, ucn_as_utf8: false },
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

// --- Lookup caching (WP-820): each test counts the queries a `RealFs`
// would turn into system calls (`MemoryFs::queries`: open, stat, list).

/// A header-search over `fs` with the angled directories `dirs`.
fn searcher<F: HeaderFs>(mut fs: F, dirs: &[&str]) -> HeaderSearch<F> {
    let specs: Vec<SearchDirSpec> =
        dirs.iter().map(|d| SearchDirSpec::new(*d, DirGroup::Angled)).collect();
    let list = SearchList::build(&specs, true, &mut fs);
    HeaderSearch::new(fs, list)
}

/// `<name>` (`#include_next` from search position `next`, if given):
/// the path it was found by.
fn find<F: HeaderFs>(
    hs: &mut HeaderSearch<F>,
    sm: &SourceManager,
    name: &str,
    next: Option<u32>,
) -> Option<String> {
    use kiln_pp::{IncludeKind, IncludeRequest, IncludeResolver};
    let req = IncludeRequest {
        kind: if next.is_some() { IncludeKind::IncludeNext } else { IncludeKind::Include },
        name: name.to_owned(),
        angled: true,
        includer: None,
        lookup_from: next,
        name_span: kiln_source::Span::default(),
        directive_span: kiln_source::Span::default(),
    };
    hs.resolve(sm, &req).map(|r| r.path.expect("header search reports the path"))
}

/// The queries made since `before`.
fn since(hs: &HeaderSearch<MemoryFs>, before: (u32, u32, u32)) -> (u32, u32, u32) {
    let now = hs.fs().queries();
    (now.0 - before.0, now.1 - before.1, now.2 - before.2)
}

#[test]
fn a_repeated_lookup_makes_no_queries() {
    let sm = SourceManager::new();
    let fs = MemoryFs::new()
        .with_dir("/d1")
        .with_dir("/d2")
        .with_dir("/d3")
        .with_file("/d3/x.h", "")
        .with_file("/d2/n.h", "")
        .with_file("/d3/n.h", "");
    let mut hs = searcher(fs, &["/d1", "/d2", "/d3"]);
    let q = hs.fs().queries();
    assert_eq!(find(&mut hs, &sm, "x.h", None).as_deref(), Some("/d3/x.h"));
    assert_eq!(since(&hs, q), (3, 0, 0), "one open per directory");
    let q = hs.fs().queries();
    assert_eq!(find(&mut hs, &sm, "x.h", None).as_deref(), Some("/d3/x.h"));
    assert_eq!(find(&mut hs, &sm, "absent.h", None), None);
    assert_eq!(find(&mut hs, &sm, "absent.h", None), None);
    assert_eq!(since(&hs, q), (3, 0, 0), "only the first absent.h lookup opens");
    // `#include_next` from /d2 starts after it, whatever `n.h` found first.
    assert_eq!(find(&mut hs, &sm, "n.h", None).as_deref(), Some("/d2/n.h"));
    assert_eq!(find(&mut hs, &sm, "n.h", Some(1)).as_deref(), Some("/d3/n.h"));
    assert_eq!(find(&mut hs, &sm, "n.h", Some(2)), None);
    assert_eq!(find(&mut hs, &sm, "n.h", None).as_deref(), Some("/d2/n.h"));
}

#[test]
fn a_missing_subdirectory_rules_out_its_headers_with_one_stat() {
    let sm = SourceManager::new();
    let fs = MemoryFs::new()
        .with_dir("/d1")
        .with_file("/d2/sub/a.h", "")
        .with_file("/d2/sub/b.h", "")
        .with_file("/d2/sub/deep/c.h", "");
    let mut hs = searcher(fs, &["/d1", "/d2"]);
    let q = hs.fs().queries();
    assert_eq!(find(&mut hs, &sm, "sub/a.h", None).as_deref(), Some("/d2/sub/a.h"));
    assert_eq!(since(&hs, q), (1, 2, 0), "stat /d1/sub, stat /d2/sub, open");
    let q = hs.fs().queries();
    assert_eq!(find(&mut hs, &sm, "sub/b.h", None).as_deref(), Some("/d2/sub/b.h"));
    assert_eq!(since(&hs, q), (1, 0, 0), "both directories known");
    let q = hs.fs().queries();
    assert_eq!(find(&mut hs, &sm, "sub/deep/c.h", None).as_deref(), Some("/d2/sub/deep/c.h"));
    assert_eq!(since(&hs, q), (1, 1, 0), "stat /d2/sub/deep, open");
}

#[test]
fn a_directory_listing_answers_misses_after_four() {
    let sm = SourceManager::new();
    let mut fs = MemoryFs::new().with_file("/d1/hit.h", "");
    for i in 0..8 {
        fs = fs.with_file(&format!("/d2/m{i}.h"), "");
    }
    let mut hs = searcher(fs, &["/d1", "/d2"]);
    let q = hs.fs().queries();
    for i in 0..4 {
        assert_eq!(find(&mut hs, &sm, &format!("m{i}.h"), None), Some(format!("/d2/m{i}.h")));
    }
    assert_eq!(since(&hs, q), (8, 0, 1), "four misses in /d1 read its listing");
    let q = hs.fs().queries();
    for i in 4..8 {
        assert_eq!(find(&mut hs, &sm, &format!("m{i}.h"), None), Some(format!("/d2/m{i}.h")));
    }
    assert_eq!(since(&hs, q), (4, 0, 0), "/d1 misses come from the listing");
    // A name the listing has is opened; one neither directory has is a
    // miss in /d1 without a query.
    let q = hs.fs().queries();
    assert_eq!(find(&mut hs, &sm, "hit.h", None).as_deref(), Some("/d1/hit.h"));
    assert_eq!(find(&mut hs, &sm, "none.h", None), None);
    assert_eq!(since(&hs, q), (2, 0, 0), "open /d1/hit.h, open /d2/none.h");
    // A missing subdirectory of a listed directory needs no stat either.
    let q = hs.fs().queries();
    assert_eq!(find(&mut hs, &sm, "sub/x.h", None), None);
    assert_eq!(since(&hs, q), (0, 1, 0), "stat /d2/sub only");
}

/// A file system whose listings spell names in upper case, as a
/// case-insensitive one may: a listing never hides `hit.h` stored as
/// `HIT.H` (names are compared ignoring ASCII case).
struct UpperListing(MemoryFs);

impl HeaderFs for UpperListing {
    fn dir_identity(&mut self, path: &str) -> Option<String> {
        self.0.dir_identity(path)
    }
    fn list_dir(&mut self, path: &str) -> Option<Vec<String>> {
        Some(self.0.list_dir(path)?.iter().map(|n| n.to_ascii_uppercase()).collect())
    }
    fn open(&mut self, sm: &SourceManager, path: &str) -> Option<kiln_source::FileId> {
        self.0.open(sm, path)
    }
    fn read(&mut self, path: &str) -> Option<Vec<u8>> {
        self.0.read(path)
    }
}

#[test]
fn a_listing_compares_names_ignoring_ascii_case() {
    let sm = SourceManager::new();
    let mut fs = MemoryFs::new().with_file("/d1/hit.h", "");
    for i in 0..5 {
        fs = fs.with_file(&format!("/d2/m{i}.h"), "");
    }
    let mut hs = searcher(UpperListing(fs), &["/d1", "/d2"]);
    for i in 0..5 {
        assert!(find(&mut hs, &sm, &format!("m{i}.h"), None).is_some());
    }
    // Listed (after four misses) as `HIT.H`, still found.
    assert_eq!(hs.fs().0.queries(), (9, 2, 1));
    assert_eq!(find(&mut hs, &sm, "hit.h", None).as_deref(), Some("/d1/hit.h"));
}

#[test]
fn a_directory_with_a_non_ascii_name_is_not_answered_from_its_listing() {
    let sm = SourceManager::new();
    let mut fs = MemoryFs::new().with_file("/d1/\u{dc}n\u{ef}.h", "");
    for i in 0..6 {
        fs = fs.with_file(&format!("/d2/m{i}.h"), "");
    }
    let mut hs = searcher(fs, &["/d1", "/d2"]);
    let q = hs.fs().queries();
    for i in 0..6 {
        assert!(find(&mut hs, &sm, &format!("m{i}.h"), None).is_some());
    }
    assert_eq!(since(&hs, q), (12, 0, 1), "read once, then /d1 is still opened");
    assert_eq!(find(&mut hs, &sm, "\u{dc}n\u{ef}.h", None).as_deref(), Some("/d1/\u{dc}n\u{ef}.h"));
}

#[test]
fn a_header_is_found_by_its_spelled_path() {
    let sm = SourceManager::new();
    let fs = MemoryFs::new().with_file("/d/once.h", "").with_dir("/d/sub");
    let mut hs = searcher(fs, &["/d"]);
    assert_eq!(find(&mut hs, &sm, "sub/../once.h", None).as_deref(), Some("/d/sub/../once.h"));
    assert_eq!(find(&mut hs, &sm, "once.h", None).as_deref(), Some("/d/once.h"));
}
