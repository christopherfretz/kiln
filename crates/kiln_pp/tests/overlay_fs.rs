//! `OverlayFs` (spec/source.md "Overlay", WP-2391): header search's view
//! of a tree as an overlay changes it, compared query by query with
//! `RealFs` over a copy of the tree in which the edit is applied (created
//! files written, their directories made, removed files unlinked,
//! replaced files rewritten). Paths include `..` (out of created
//! directories and out of the disk's), `.`, and symbolic links.
#![forbid(unsafe_code)]
#![cfg(unix)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use kiln_pp::{
    Characteristic, DirGroup, HeaderFs, HeaderSearch, IncludeKind, IncludeRequest, IncludeResolver,
    LookupLog, OverlayFs, RealFs, SearchDirSpec, SearchList, Trust,
};
use kiln_source::{Overlay, SourceManager, Span, WorkDir};

struct Tree(PathBuf);

impl Tree {
    fn new(tag: &str) -> Tree {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("kiln_pp-overlay-fs-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Tree(dir)
    }

    fn write(&self, name: &str, bytes: &[u8]) {
        let p = self.0.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    fn work_dir(&self) -> WorkDir {
        WorkDir::at(&self.0).unwrap()
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The edit, applied to an overlay over `t` and to the tree `c`.
enum Edit {
    Create(&'static str, &'static [u8]),
    Replace(&'static str, &'static [u8]),
    Remove(&'static str),
}

/// Builds the base tree in `t` (and `c`, its copy).
fn base(t: &Tree) {
    t.write("a.h", b"a\n");
    t.write("inc/b.h", b"b\n");
    t.write("inc/sub/c.h", b"c\n");
    t.write("other/d.h", b"d\n");
    for i in 0..6 {
        t.write(&format!("many/f{i}.h"), b"f\n");
    }
    std::fs::create_dir_all(t.0.join("empty")).unwrap();
    std::os::unix::fs::symlink("inc", t.0.join("alias")).unwrap();
}

const EDITS: &[Edit] = &[
    Edit::Create("inc/new.h", b"new\n"),
    Edit::Create("newdir/x.h", b"x\n"),
    Edit::Create("newdir/deep/y.h", b""),
    Edit::Create("inc/sub2/z.h", b"z\n"),
    Edit::Create("many/late.h", b"late\n"),
    Edit::Create("many/latedir/w.h", b"w\n"),
    Edit::Remove("inc/b.h"),
    Edit::Replace("other/d.h", b"d2\n"),
];

/// The overlay tree `t` and the edited copy `c`.
fn trees(tag: &str) -> (Tree, Tree, Arc<Overlay>) {
    let (t, c) = (Tree::new(tag), Tree::new(&format!("{tag}-copy")));
    base(&t);
    base(&c);
    let mut o = Overlay::new();
    for e in EDITS {
        match *e {
            Edit::Create(name, bytes) => {
                o.create(&t.0.join(name), Arc::from(bytes)).unwrap();
                c.write(name, bytes);
            }
            Edit::Replace(name, bytes) => {
                o.replace(&t.0.join(name), Arc::from(bytes)).unwrap();
                c.write(name, bytes);
            }
            Edit::Remove(n) => {
                o.remove(&t.0.join(n)).unwrap();
                std::fs::remove_file(c.0.join(n)).unwrap();
            }
        }
    }
    (t, c, Arc::new(o))
}

const PATHS: &[&str] = &[
    ".",
    "a.h",
    "inc",
    "inc/",
    "inc/b.h",
    "inc/new.h",
    "inc/./new.h",
    "alias/new.h",
    "alias/b.h",
    "alias/sub2/z.h",
    "alias/sub2/../new.h",
    "alias/../newdir/x.h",
    "inc/sub",
    "inc/sub/c.h",
    "inc/sub/../new.h",
    "inc/sub2",
    "inc/sub2/z.h",
    "inc/sub2/../b.h",
    "inc/sub2/../new.h",
    "inc/sub2/../sub/c.h",
    "inc/sub2/../../a.h",
    "newdir",
    "newdir/",
    "./newdir",
    "newdir/x.h",
    "./newdir/./x.h",
    "newdir/deep",
    "newdir/deep/y.h",
    "newdir/deep/..",
    "newdir/deep/../x.h",
    "newdir/deep/../../a.h",
    "newdir/../a.h",
    "newdir/../inc/new.h",
    "newdir/../newdir/deep/y.h",
    "newdir/x.h/../x.h",
    "newdir/missing.h",
    "newdir/deep/missing/y.h",
    "nodir/../a.h",
    "nodir/x.h",
    "missing.h",
    "inc/missing.h",
    "other/d.h",
    "empty",
    "empty/x.h",
    "many",
    "many/late.h",
    "many/latedir",
    "many/latedir/w.h",
    "a.h/x.h",
];

/// The names of `listing` (of the directory `dir`) that `fs` says
/// exist, sorted. An overlay's listing keeps the name of a file it
/// removes (a listed name only permits the `open`, which answers), so
/// listings are compared on the names that exist, and the raw one must
/// hold every one of them.
fn existing(fs: &mut dyn HeaderFs, dir: &str, listing: Option<Vec<String>>) -> Option<Vec<String>> {
    let mut names: Vec<String> = listing?
        .into_iter()
        .filter(|n| {
            let p = kiln_pp::join_path(dir, n);
            fs.is_file(&p) || fs.is_dir(&p)
        })
        .collect();
    names.sort();
    Some(names)
}

/// Every query of `fs` on every path, with directory identities replaced
/// by the index of the first path that had the same one.
fn answers(fs: &mut dyn HeaderFs, sm: &SourceManager) -> Vec<String> {
    let mut idents: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();
    for (i, p) in PATHS.iter().enumerate() {
        let ident = fs.dir_identity(p).map(|id| *idents.entry(id).or_insert(i));
        let listing = fs.list_dir(p);
        let listing = existing(fs, p, listing);
        let opened = fs.open(sm, p).map(|f| sm.text(f).to_owned());
        out.push(format!(
            "{p}: ident {ident:?} dir {} file {} list {listing:?} read {:?} open {opened:?}",
            fs.is_dir(p),
            fs.is_file(p),
            fs.read(p).map(|b| String::from_utf8_lossy(&b).into_owned()),
        ));
    }
    let threaded = fs.list_dirs(PATHS);
    for (l, p) in threaded.into_iter().zip(PATHS) {
        let l = existing(fs, p, l);
        out.push(format!("{p}: listed {l:?}"));
    }
    out
}

/// [cpp.include]: each of `HeaderFs`'s seven queries over the overlay
/// answers what `RealFs` answers over the edited copy, path by path.
#[test]
fn every_query_matches_the_edited_tree() {
    let (t, c, o) = trees("queries");
    let sm = SourceManager::new();
    sm.set_work_dir(t.work_dir());
    sm.set_overlay(Arc::clone(&o));
    let mut ofs = OverlayFs::new(t.work_dir(), Arc::clone(&o));
    let with = answers(&mut ofs, &sm);
    let csm = SourceManager::new();
    csm.set_work_dir(c.work_dir());
    let reference = answers(&mut RealFs::in_dir(c.work_dir()), &csm);
    for (w, r) in with.iter().zip(&reference) {
        assert_eq!(w, r);
    }
    assert_eq!(with.len(), reference.len());
}

/// The same queries with absolute paths (the working directory unused).
#[test]
fn absolute_paths_match_the_edited_tree() {
    let (t, c, o) = trees("absolute");
    for (p, q) in [
        ("newdir/deep/y.h", true),
        ("inc/new.h", true),
        ("inc/b.h", false),
        ("newdir/../a.h", true),
        ("nodir/x.h", false),
    ] {
        let mut ofs = OverlayFs::new(WorkDir::process(), Arc::clone(&o));
        let mut real = RealFs::new();
        let (tp, cp) = (t.0.join(p), c.0.join(p));
        let (tp, cp) = (tp.to_str().unwrap(), cp.to_str().unwrap());
        assert_eq!(ofs.is_file(tp), q, "{p}");
        assert_eq!(real.is_file(cp), q, "{p}");
        assert_eq!(ofs.read(tp), real.read(cp), "{p}");
    }
}

fn include(name: &str, angled: bool) -> IncludeRequest {
    IncludeRequest {
        kind: IncludeKind::Include,
        name: name.to_owned(),
        angled,
        includer: None,
        lookup_from: None,
        name_span: Span::INVALID,
        directive_span: Span::INVALID,
    }
}

/// The lookups `names` make through header search over `fs`, `-I`
/// directories `dirs`: each name's result path, or `None`.
fn lookups<F: HeaderFs>(
    fs: F,
    sm: &SourceManager,
    dirs: &[&str],
    names: &[&str],
) -> (Vec<Option<String>>, Vec<String>) {
    let specs: Vec<SearchDirSpec> = dirs
        .iter()
        .map(|d| SearchDirSpec { path: (*d).to_owned(), group: DirGroup::Angled })
        .collect();
    let mut fs = fs;
    let list = SearchList::build(&specs, true, &mut fs);
    let messages = list.messages().to_vec();
    let mut hs = HeaderSearch::new(fs, list);
    let found =
        names.iter().map(|n| hs.resolve(sm, &include(n, true)).and_then(|r| r.path)).collect();
    (found, messages)
}

/// Header search's caches (spec/pp.md "Header search"): after enough
/// misses in a directory its listing answers further misses with no
/// system call, so a listing without the overlay's created names would
/// miss them; a directory only the overlay makes exist is kept in the
/// search list, `-I newdir -I ./newdir` being one directory; a removed
/// header lets the next directory's copy win.
#[test]
fn header_search_over_the_overlay_matches_the_edited_tree() {
    let (t, c, o) = trees("search");
    t.write("second/b.h", b"b2\n");
    c.write("second/b.h", b"b2\n");
    let dirs = ["many", "newdir", "./newdir", "inc", "second", "nodir"];
    let names = [
        "m0.h",
        "m1.h",
        "m2.h",
        "m3.h",
        "m4.h",
        "m5.h",
        "late.h",
        "latedir/w.h",
        "x.h",
        "deep/y.h",
        "b.h",
        "new.h",
        "sub2/z.h",
        "absent.h",
    ];
    let sm = SourceManager::new();
    sm.set_work_dir(t.work_dir());
    sm.set_overlay(Arc::clone(&o));
    let with = lookups(OverlayFs::new(t.work_dir(), Arc::clone(&o)), &sm, &dirs, &names);
    let csm = SourceManager::new();
    csm.set_work_dir(c.work_dir());
    let reference = lookups(RealFs::in_dir(c.work_dir()), &csm, &dirs, &names);
    assert_eq!(with, reference);
    assert_eq!(with.0[6].as_deref(), Some("many/late.h"));
    assert_eq!(with.0[10].as_deref(), Some("second/b.h"));
    assert!(with.1.iter().any(|m| m.contains("duplicate directory \"./newdir\"")), "{:?}", with.1);
    assert_eq!(Characteristic::default(), Characteristic::User);
}

/// A path the disk does not have and whose last component no created
/// path has is a miss with no system call beyond the disk's own.
#[test]
fn a_miss_the_overlay_cannot_have_is_answered_at_once() {
    let (t, _c, o) = trees("miss");
    let mut ofs = OverlayFs::new(t.work_dir(), o);
    assert!(!ofs.is_file("inc/unrelated.h"));
    assert!(!ofs.is_dir("nodir/deeper"));
    assert!(ofs.read("nodir/none.bin").is_none());
    assert!(Path::new(&t.0).join("inc").is_dir());
}

/// The search list of `-Iinc1 -Iinc2` over `fs`.
fn two_dirs(fs: &mut dyn HeaderFs) -> SearchList {
    let specs =
        ["inc1", "inc2"].map(|d| SearchDirSpec { path: d.to_owned(), group: DirGroup::Angled });
    SearchList::build(&specs, true, fs)
}

/// The log of `#include <h.h>` and `__has_include(<opt.h>)` over `fs`.
fn record<F: HeaderFs>(mut fs: F, sm: &SourceManager) -> LookupLog {
    let list = two_dirs(&mut fs);
    let mut hs = HeaderSearch::new(fs, list);
    hs.record_lookups();
    hs.resolve(sm, &include("h.h", true));
    let mut probe = include("opt.h", true);
    probe.kind = IncludeKind::HasInclude;
    hs.resolve(sm, &probe);
    hs.take_lookup_log().unwrap()
}

/// spec/pp.md "Lookup log": a log is replayed with
/// `Trust::ProducerMisses` only against the view it was recorded under.
/// A log recorded with no overlay, replayed with `Trust::Nothing` over
/// an `OverlayFs`: changed under an overlay that creates a shadowing
/// header, under one that removes the hit and under one that creates the
/// probed header; unchanged under an unrelated one. A log recorded under
/// a removing overlay, replayed with none: changed. `ProducerMisses` on
/// the same inputs answers "unchanged" for the shadowing creation and
/// for the replay after the removal (why the rule exists).
#[test]
fn a_log_replayed_under_another_view() {
    let t = Tree::new("replay");
    std::fs::create_dir_all(t.0.join("inc1")).unwrap();
    t.write("inc2/h.h", b"h\n");
    t.write("other/x.h", b"x\n");
    let plain = SourceManager::new();
    plain.set_work_dir(t.work_dir());
    let log = record(RealFs::in_dir(t.work_dir()), &plain);
    let replay = |edit: &dyn Fn(&mut Overlay), trust: Trust| {
        let mut o = Overlay::new();
        edit(&mut o);
        let mut fs = OverlayFs::new(t.work_dir(), Arc::new(o));
        let list = two_dirs(&mut fs);
        log.check(&list, &[], &mut fs, &|_| false, trust)
    };
    let created = |name: &'static str| {
        let p = t.0.join(name);
        move |o: &mut Overlay| o.create(&p, Arc::from(&b"\n"[..])).unwrap()
    };
    let shadow = created("inc1/h.h");
    assert_eq!(
        replay(&shadow, Trust::Nothing),
        Err("lookup of <h.h> now finds 'inc1/h.h' (was 'inc2/h.h')".to_owned())
    );
    assert_eq!(replay(&shadow, Trust::ProducerMisses { same_cwd: true }), Ok(()));
    let removed = |o: &mut Overlay| o.remove(&t.0.join("inc2/h.h")).unwrap();
    assert_eq!(
        replay(&removed, Trust::Nothing),
        Err("lookup of <h.h> now finds nothing (was 'inc2/h.h')".to_owned())
    );
    assert!(replay(&created("inc2/opt.h"), Trust::Nothing).is_err());
    assert_eq!(replay(&created("other/new.h"), Trust::Nothing), Ok(()));

    let mut o = Overlay::new();
    removed(&mut o);
    let o = Arc::new(o);
    let sm = SourceManager::new();
    sm.set_work_dir(t.work_dir());
    sm.set_overlay(Arc::clone(&o));
    let under_removal = record(OverlayFs::new(t.work_dir(), o), &sm);
    let mut real = RealFs::in_dir(t.work_dir());
    let list = two_dirs(&mut real);
    assert_eq!(
        under_removal.check(&list, &[], &mut real, &|_| false, Trust::Nothing),
        Err("lookup of <h.h> now finds 'inc2/h.h' (was not found)".to_owned())
    );
    assert_eq!(
        under_removal.check(
            &list,
            &[],
            &mut real,
            &|_| false,
            Trust::ProducerMisses { same_cwd: true }
        ),
        Ok(())
    );
}
