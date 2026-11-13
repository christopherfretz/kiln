//! The lookup log (WP-1403, spec/pp.md "Lookup log", spec/driver.md
//! "Lookup-tolerant search path"): header search records every lookup a
//! prefix makes, and [`LookupLog::check`] repeats them over another
//! search list, naming the first whose result changed.
#![allow(clippy::many_single_char_names, reason = "f(ile system), l(ist), o/s/n (query counts)")]

use std::fmt::Write as _;

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, Std};
use kiln_pp::{
    DirGroup, HeaderFs, HeaderSearch, LookupLog, MemoryFs, PpOptions, Preprocessor, PrintOptions,
    SearchDirSpec, SearchList, print_preprocessed,
};
use kiln_source::SourceManager;

fn list(fs: &mut MemoryFs, dirs: &[(&str, DirGroup)]) -> SearchList {
    let specs: Vec<SearchDirSpec> = dirs.iter().map(|&(p, g)| SearchDirSpec::new(p, g)).collect();
    SearchList::build(&specs, true, fs)
}

/// Preprocesses `main` of `fs` over `dirs`, recording; the log.
fn record(mut fs: MemoryFs, main: &str, dirs: &[(&str, DirGroup)]) -> LookupLog {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let l = list(&mut fs, dirs);
    let main = fs.open(&sm, main).expect("main file");
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let mut pp = Preprocessor::new(&sm, &engine, &interner, PpOptions::new(Std::Cxx26));
    let mut hs = HeaderSearch::new(fs, l).with_embed_dirs(vec!["/res".into()]);
    hs.record_lookups();
    pp.set_include_resolver(Box::new(hs));
    pp.enter_main_file(main);
    let mut out = String::new();
    print_preprocessed(
        &mut pp,
        PrintOptions { line_markers: false, line_directives: false, ucn_as_utf8: false },
        &mut out,
    );
    let log = pp.take_lookup_log().expect("recorded");
    drop(pp);
    let diags: Vec<String> = sink.take().iter().map(|d| d.message.clone()).collect();
    assert!(diags.iter().all(|d| d.contains("Clang extension")), "{diags:?}");
    log
}

/// Repeats `log` over `dirs` on `fs`.
fn check(log: &LookupLog, fs: &mut MemoryFs, dirs: &[(&str, DirGroup)]) -> Result<(), String> {
    let l = list(fs, dirs);
    log.check(&l, &["/res".to_owned()], fs, &|_| false, true)
}

const A: DirGroup = DirGroup::Angled;
const S: DirGroup = DirGroup::System;

/// A prefix that includes `<v.h>` (found in `/a`), `"q.h"` (beside it),
/// asks `__has_include(<opt.h>)` (missing) and `#include_next`s from
/// `/a/n.h` into `/b/n.h`.
fn tree() -> MemoryFs {
    MemoryFs::new()
        .with_file(
            "/p/h.h",
            "#include <v.h>\n#include \"q.h\"\n#if __has_include(<opt.h>)\n#endif\n#include <n.h>\n",
        )
        .with_file("/p/q.h", "int q;\n")
        .with_file("/a/v.h", "int v;\n")
        .with_file("/a/n.h", "#include_next <n.h>\n")
        .with_file("/b/n.h", "int n;\n")
        .with_dir("/extra")
        .with_dir("/shadow")
}

const DIRS: &[(&str, DirGroup)] = &[("/a", A), ("/b", A)];

#[test]
fn records_each_lookup_once() {
    let log = record(tree(), "/p/h.h", DIRS);
    let names: Vec<&str> = log.lookups.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["v.h", "q.h", "opt.h", "n.h", "n.h"]);
    assert!(log.absolute());
    // The same list repeats cleanly.
    assert_eq!(check(&log, &mut tree(), DIRS), Ok(()));
}

#[test]
fn added_and_reordered_directories_restore() {
    let log = record(tree(), "/p/h.h", DIRS);
    // An unrelated directory, before or after.
    assert_eq!(check(&log, &mut tree(), &[("/extra", A), ("/a", A), ("/b", A)]), Ok(()));
    assert_eq!(check(&log, &mut tree(), &[("/a", A), ("/b", A), ("/extra", A)]), Ok(()));
    // A harmless reorder: `/extra` and `/shadow` hold nothing the prefix
    // asks for.
    let mut fs = tree();
    assert_eq!(
        check(&log, &mut fs, &[("/shadow", A), ("/a", A), ("/extra", A), ("/b", A)]),
        Ok(())
    );
}

#[test]
fn shadowing_refuses() {
    let log = record(tree(), "/p/h.h", DIRS);
    let mut fs = tree().with_file("/shadow/v.h", "int w;\n");
    assert_eq!(
        check(&log, &mut fs, &[("/shadow", A), ("/a", A), ("/b", A)]),
        Err("lookup of <v.h> now finds '/shadow/v.h' (was '/a/v.h')".to_owned())
    );
    // Swapping the two directories of the `#include_next` pair changes
    // what `<n.h>` finds.
    assert_eq!(
        check(&log, &mut tree(), &[("/b", A), ("/a", A)]),
        Err("lookup of <n.h> now finds '/b/n.h' (was '/a/n.h')".to_owned())
    );
}

#[test]
fn has_include_that_now_succeeds_refuses() {
    let log = record(tree(), "/p/h.h", DIRS);
    let mut fs = tree().with_file("/extra/opt.h", "\n");
    assert_eq!(
        check(&log, &mut fs, &[("/a", A), ("/b", A), ("/extra", A)]),
        Err("lookup of <opt.h> now finds '/extra/opt.h' (was not found)".to_owned())
    );
}

#[test]
fn isystem_against_i_refuses() {
    let log = record(tree(), "/p/h.h", DIRS);
    assert_eq!(
        check(&log, &mut tree(), &[("/a", S), ("/b", A)]),
        Err("lookup of <v.h> now finds '/a/v.h' as a system header (was a user header)".to_owned())
    );
}

#[test]
fn include_next_needs_its_directory() {
    let log = record(tree(), "/p/h.h", DIRS);
    // Without `/b` the first `<n.h>` still finds `/a/n.h`; its
    // `#include_next` now finds nothing.
    assert_eq!(
        check(&log, &mut tree(), &[("/a", A)]),
        Err("lookup of #include_next <n.h> now finds nothing (was '/b/n.h')".to_owned())
    );
}

#[test]
fn quoted_and_after_groups() {
    let fs = MemoryFs::new()
        .with_file("/p/h.h", "#include \"cfg.h\"\n#include <late.h>\n")
        .with_file("/q/cfg.h", "\n")
        .with_file("/late/late.h", "\n")
        .with_dir("/extra");
    let dirs = &[("/q", DirGroup::Quoted), ("/late", DirGroup::After)];
    let log = record(fs.clone(), "/p/h.h", dirs);
    let mut f = fs.clone();
    assert_eq!(
        check(&log, &mut f, &[("/extra", A), ("/q", DirGroup::Quoted), ("/late", DirGroup::After)]),
        Ok(())
    );
    // `-iquote` dropped: `"cfg.h"` is not found.
    let mut f = fs.clone();
    assert_eq!(
        check(&log, &mut f, &[("/late", DirGroup::After)]),
        Err("lookup of \"cfg.h\" now finds nothing (was '/q/cfg.h')".to_owned())
    );
    // `-idirafter` turned into `-I`: the same file, now a user header.
    let mut f = fs;
    assert_eq!(
        check(&log, &mut f, &[("/q", DirGroup::Quoted), ("/late", A)]),
        Err("lookup of <late.h> now finds '/late/late.h' as a user header (was a system header)"
            .to_owned())
    );
}

#[test]
fn embed_lookups_are_repeated() {
    let fs = MemoryFs::new()
        .with_file("/p/h.h", "const char d[] = {\n#embed <d.bin>\n};\n")
        .with_file("/res/d.bin", "Z")
        .with_dir("/extra");
    let log = record(fs.clone(), "/p/h.h", &[]);
    let mut f = fs.clone();
    assert_eq!(check(&log, &mut f, &[("/extra", A)]), Ok(()));
    let mut f = fs;
    let l = list(&mut f, &[]);
    f.add_file("/other/d.bin", b"Y");
    assert_eq!(
        log.check(&l, &["/other".to_owned(), "/res".to_owned()], &mut f, &|_| false, true),
        Err("lookup of #embed <d.bin> now finds '/other/d.bin' (was '/res/d.bin')".to_owned())
    );
}

#[test]
fn absolute_names_are_not_recorded() {
    let fs = MemoryFs::new().with_file("/p/h.h", "#include \"/a/v.h\"\n").with_file("/a/v.h", "\n");
    let log = record(fs, "/p/h.h", &[]);
    assert!(log.lookups.is_empty());
}

/// Known files and repeated names cost no queries; many misses in one
/// directory read it once (header search's listing).
#[test]
fn queries_are_bounded() {
    let mut fs = MemoryFs::new().with_file("/p/h.h", "").with_dir("/a");
    let mut text = String::new();
    for i in 0..40 {
        let name = format!("f{i}.h");
        fs.add_file(&format!("/a/{name}"), b"\n");
        let _ = writeln!(text, "#include <{name}>");
    }
    fs.add_file("/p/h.h", text.as_bytes());
    let empty: Vec<String> = (0..20).map(|i| format!("/e{i}")).collect();
    let mut f = fs.clone();
    for d in &empty {
        f = f.with_dir(d);
    }
    let log = record(fs, "/p/h.h", &[("/a", A)]);
    let mut dirs: Vec<(&str, DirGroup)> = empty.iter().map(|d| (d.as_str(), A)).collect();
    dirs.push(("/a", A));
    let l = list(&mut f, &dirs);
    let before = f.queries();
    // Every hit known: only the 20 empty directories are asked, each
    // read once after its first misses.
    let known = |p: &str| p.starts_with("/a/");
    assert_eq!(log.check(&l, &[], &mut f, &known, true), Ok(()));
    let (o, s, n) = f.queries();
    let (o, s, n) = (o - before.0, s - before.1, n - before.2);
    assert!(o + s <= 20 * 4 + 1, "({o}, {s}, {n})");
    assert!(n <= 20, "({o}, {s}, {n})");
}

/// A directory the producer's lookup already searched (before its
/// result) is not asked again: one new directory costs only its own
/// misses, however long the producer's list.
#[test]
fn producer_misses_are_not_asked_again() {
    let mut fs = MemoryFs::new().with_dir("/a").with_dir("/extra");
    let mut text = String::new();
    let mut dirs: Vec<String> = (0..10).map(|i| format!("/s{i}")).collect();
    for d in &dirs {
        fs = fs.with_dir(d);
    }
    for i in 0..40 {
        fs.add_file(&format!("/a/f{i}.h"), b"\n");
        let _ = writeln!(text, "#include <f{i}.h>\n#if __has_include(<g{i}.h>)\n#endif");
    }
    fs.add_file("/p/h.h", text.as_bytes());
    dirs.push("/a".to_owned());
    let specs: Vec<(&str, DirGroup)> = dirs.iter().map(|d| (d.as_str(), A)).collect();
    let log = record(fs.clone(), "/p/h.h", &specs);
    let mut consumer = vec![("/extra", A)];
    consumer.extend(specs.iter().copied());
    let l = list(&mut fs, &consumer);
    let before = fs.queries();
    let known = |p: &str| p.starts_with("/a/");
    assert_eq!(log.check(&l, &[], &mut fs, &known, true), Ok(()));
    let (o, s, n) = fs.queries();
    let (o, s, n) = (o - before.0, s - before.1, n - before.2);
    // `/extra` misses four times and is then listed; the `__has_include`
    // misses in `/a` (searched by the producer to the end) are not asked.
    assert!(o + s <= 4, "({o}, {s}, {n})");
    assert!(n <= 1, "({o}, {s}, {n})");
    // From another working directory a relative list is asked again;
    // an absolute one is not.
    let before = fs.queries();
    assert_eq!(log.check(&l, &[], &mut fs, &known, false), Ok(()));
    let (o, s, _) = fs.queries();
    assert!(o + s - before.0 - before.1 <= 4);
}
