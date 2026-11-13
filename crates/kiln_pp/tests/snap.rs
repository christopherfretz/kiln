//! Snapshots of the preprocessor side of the front end (spec/pp.md and
//! spec/diag.md "Snapshots", spec/snap.md "Configured roots"): the
//! interner, the source manager, the diagnostics engine and the
//! preprocessor, saved at the end of a prefix ([cpp.include]: a header
//! read as the main file, as `-emit-pch` reads it) and restored into
//! fresh objects built as a consumer builds them, go on exactly as the
//! originals do: the same tokens ([lex.pptoken], [cpp.replace],
//! [cpp.predefined] `__COUNTER__`), diagnostics ([cpp.error],
//! [cpp.pragma]), dependencies and state.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, Std};
use kiln_pp::{
    ClangFeatures, DirGroup, FileChange, HeaderSearch, IncludeRequest, IncludeResolver, Marker,
    MemoryResolver, ModuleOptions, PpCallbacks, PpItem, PpOptions, Preprocessor, RealFs,
    ResolvedInclude, SearchDirSpec, SearchList,
};
use kiln_snap::{Reader, Schema, Writer, from_bytes, to_bytes, to_bytes_canonical};
use kiln_source::SourceManager;

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("kiln_pp-snap-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The host's normalized triple (the resource files are per target).
fn triple() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "aarch64" => Some("aarch64-unknown-linux-gnu"),
        "x86_64" => Some("x86_64-unknown-linux-gnu"),
        _ => None,
    }
}

fn resource() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resource")
}

/// What a consumer configures from its command line: libstdc++'s
/// system search list (clang 19's, with kiln's resource directory), the
/// target's predefines and feature queries. `None` when the host has no
/// libstdc++ headers.
#[derive(Clone)]
struct Config {
    triple: &'static str,
    dirs: Vec<SearchDirSpec>,
    predefines: String,
}

impl Config {
    fn host() -> Option<Config> {
        let triple = triple()?;
        let list =
            fs::read_to_string(resource().join(format!("include-paths/{triple}.txt"))).ok()?;
        let res_inc = resource().join("include").display().to_string();
        let mut dirs = Vec::new();
        let mut on = false;
        for line in list.lines() {
            if line.starts_with("#include <...>") {
                on = true;
            } else if line.starts_with("End of search list") {
                on = false;
            } else if on && let Some(p) = line.strip_prefix(' ') {
                let p = if p.ends_with("/lib/clang/19/include") {
                    res_inc.clone()
                } else {
                    p.to_owned()
                };
                dirs.push(SearchDirSpec::new(p, DirGroup::System));
            }
        }
        let has_vector = dirs.iter().any(|d| Path::new(&d.path).join("vector").is_file());
        if !has_vector {
            println!("skipped: no libstdc++ headers on this host");
            return None;
        }
        let predefines =
            fs::read_to_string(resource().join(format!("predefines/{triple}.c++26.txt"))).ok()?;
        Some(Config { triple, dirs, predefines })
    }

    fn opts(&self) -> PpOptions {
        let mut o = PpOptions::new(Std::Cxx26);
        o.predefines.clone_from(&self.predefines);
        o.reproducible_time = Some(("Jan  1 2000".to_owned(), "00:00:00".to_owned()));
        o.main_is_header = true;
        o.modules = Some(ModuleOptions::default());
        o
    }

    /// A preprocessor configured as the consumer's command line says.
    fn preprocessor<'a, 'd>(
        &self,
        sm: &'a SourceManager,
        engine: &'d DiagEngine<'a>,
        interner: &'a Interner,
        deps: &Deps,
    ) -> Preprocessor<'a, 'd> {
        let mut pp = Preprocessor::new(sm, engine, interner, self.opts());
        let mut fs = RealFs;
        let list = SearchList::build(&self.dirs, true, &mut fs);
        let r: Box<dyn IncludeResolver> = Box::new(HeaderSearch::new(fs, list));
        pp.set_include_resolver(r);
        pp.set_feature_queries(Box::new(ClangFeatures::new(self.triple, Std::Cxx26, &[]).unwrap()));
        pp.set_callbacks(Box::new(deps.clone()));
        pp
    }
}

/// The dependency events (`-M`'s inputs): files entered, skipped and
/// looked up.
#[derive(Clone, Default)]
struct Deps(Rc<RefCell<Vec<String>>>);

impl Deps {
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}

impl PpCallbacks for Deps {
    fn file_changed(&mut self, change: FileChange) {
        if let FileChange::Enter { file, system, .. } = change {
            self.0.borrow_mut().push(format!("enter {file:?} {system}"));
        }
    }

    fn inclusion_directive(&mut self, req: &IncludeRequest, result: Option<&ResolvedInclude>) {
        let found = result.map(|r| (r.file, r.system, r.path.clone()));
        self.0.borrow_mut().push(format!("include {:?} {found:?}", req.name));
    }

    fn file_skipped(&mut self, result: &ResolvedInclude, system: bool) {
        self.0.borrow_mut().push(format!("skip {:?} {system}", result.file));
    }
}

/// Every token to the end of the main file: kind, spelling, location
/// and flags.
fn tokens(pp: &mut Preprocessor<'_, '_>) -> Vec<String> {
    let sm = pp.source_manager();
    let lex = *pp.lex_options();
    let mut out = Vec::new();
    loop {
        let t = pp.next();
        out.push(format!(
            "{:?} {:?} {:?} {:?}",
            t.kind,
            kiln_lex::spelling(sm, &t, &lex),
            t.span,
            t.flags
        ));
        if t.is_eof() {
            return out;
        }
    }
}

/// The four roots, encoded (canonical mode: comparable across histories).
fn canonical(
    i: &Interner,
    sm: &SourceManager,
    e: &DiagEngine<'_>,
    pp: &Preprocessor<'_, '_>,
) -> [Vec<u8>; 4] {
    let mut we = Writer::canonical();
    e.snap_put(&mut we).unwrap();
    let mut wp = Writer::canonical();
    pp.snap_put(&mut wp).unwrap();
    [to_bytes_canonical(i), to_bytes_canonical(sm), we.into_bytes(), wp.into_bytes()]
}

/// The snapshot of a quiescent front end: interner, source manager,
/// engine, preprocessor (normal mode), and the encode time.
struct Snapshot {
    bytes: [Vec<u8>; 4],
    encode: f64,
}

fn snapshot(
    i: &Interner,
    sm: &SourceManager,
    e: &DiagEngine<'_>,
    pp: &Preprocessor<'_, '_>,
) -> Snapshot {
    let t = Instant::now();
    let mut we = Writer::new();
    e.snap_put(&mut we).unwrap();
    let mut wp = Writer::new();
    pp.snap_put(&mut wp).unwrap();
    let bytes = [to_bytes(i), to_bytes(sm), we.into_bytes(), wp.into_bytes()];
    Snapshot { bytes, encode: t.elapsed().as_secs_f64() }
}

const PREFIX: &str = r#"#include <vector>
#include <string>
#include <map>
#if __has_include(<optional>) && !__has_include(<no/such/header.h>)
#define HAS_OPTIONAL 1
#endif
#include "once.h"
#include "guard.h"
#include "once.h"
#define ADD(a, b) ((a) + (b))
#define VEC std::vector<int>
#define PUSHED 1
#pragma push_macro("PUSHED")
#undef PUSHED
#define PUSHED 2
int c0 = __COUNTER__, c1 = __COUNTER__;
#pragma GCC poison forbidden
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-W#warnings"
#warning silenced in the prefix
#line 500 "renamed.h"
int after_line = __LINE__;
"#;

const MAIN: &str = r#"#include <vector>
#include "once.h"
#include "guard.h"
#include <map>
VEC v; int sum = ADD(1, 2); int opt = HAS_OPTIONAL;
int c2 = __COUNTER__;
int p2 = PUSHED;
#pragma pop_macro("PUSHED")
int p1 = PUSHED;
#warning suppressed by the prefix's pragma
#pragma clang diagnostic pop
#warning shown after the pop
#define ADD(a, b) a
int forbidden;
int line = __LINE__; const char *file = __FILE__;
"#;

fn write_files(dir: &TempDir) -> (PathBuf, PathBuf) {
    let p = |n: &str| dir.0.join(n);
    fs::write(p("prefix.h"), PREFIX).unwrap();
    fs::write(p("main.cpp"), MAIN).unwrap();
    fs::write(p("once.h"), "#pragma once\nint once_v;\n").unwrap();
    fs::write(p("guard.h"), "#ifndef GUARD_H\n#define GUARD_H\nint guard_v;\n#endif\n").unwrap();
    (p("prefix.h"), p("main.cpp"))
}

/// Round trip at a real checkpoint: a prefix of libstdc++ headers and
/// every kind of preprocessor state, snapshotted at its end, restored
/// into a fresh front end, then the same main file read by both.
#[test]
fn restored_front_end_goes_on_as_the_original() {
    let Some(cfg) = Config::host() else { return };
    let dir = TempDir::new("rt");
    let (prefix, main) = write_files(&dir);

    // The producer reads the prefix as its main file, to its end.
    let interner = Interner::new();
    let sm = SourceManager::new();
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let deps = Deps::default();
    let mut pp = cfg.preprocessor(&sm, &engine, &interner, &deps);
    let pf = sm.load_file(&prefix).unwrap();
    pp.enter_main_file(pf);
    let prefix_tokens = tokens(&mut pp).len();
    pp.end_main_file().unwrap();
    let snap = snapshot(&interner, &sm, &engine, &pp);
    let before = canonical(&interner, &sm, &engine, &pp);
    let prefix_diags = sink.take();
    assert!(prefix_diags.is_empty(), "{prefix_diags:#?}");
    let _ = deps.take();

    // The consumer restores in order: interner and source manager, the
    // engine built from its options, the preprocessor built and
    // configured from its options.
    let t = Instant::now();
    let interner2: Interner = from_bytes(&snap.bytes[0]).unwrap();
    let sm2: SourceManager = from_bytes(&snap.bytes[1]).unwrap();
    let sink2 = MemorySink::new();
    let mut engine2 = DiagEngine::new(&sm2, &DiagOptions::default(), sink2.clone());
    let mut r = Reader::new(&snap.bytes[2]);
    engine2.snap_restore(&mut r).unwrap();
    r.finish().unwrap();
    let deps2 = Deps::default();
    let mut pp2 = cfg.preprocessor(&sm2, &engine2, &interner2, &deps2);
    let mut r = Reader::new(&snap.bytes[3]);
    pp2.snap_restore(&mut r).unwrap();
    r.finish().unwrap();
    let decode = t.elapsed().as_secs_f64();
    assert!(
        canonical(&interner2, &sm2, &engine2, &pp2) == before,
        "state differs after the restore"
    );

    // The same main file, read by both.
    let m1 = sm.load_file(&main).unwrap();
    let m2 = sm2.load_file(&main).unwrap();
    assert_eq!(m1, m2);
    pp.enter_next_main_file(m1).unwrap();
    pp2.enter_next_main_file(m2).unwrap();
    let toks = tokens(&mut pp);
    assert_eq!(tokens(&mut pp2), toks);
    let diags = sink.take();
    assert_eq!(sink2.take(), diags);
    let dep_list = deps.take();
    assert_eq!(deps2.take(), dep_list);
    // Every `#include` of the main file was skipped: by include guard
    // (`<vector>`, `<map>`, `guard.h`) or `#pragma once` (`once.h`).
    let skips = dep_list.iter().filter(|d| d.starts_with("skip")).count();
    assert_eq!(skips, 4, "{dep_list:#?}");
    pp.end_main_file().unwrap();
    pp2.end_main_file().unwrap();
    assert!(canonical(&interner2, &sm2, &engine2, &pp2) == canonical(&interner, &sm, &engine, &pp));

    // The main file used the restored state.
    let text: Vec<String> =
        toks.iter().map(|t| t.split(' ').nth(1).unwrap_or("").to_owned()).collect();
    let text = text.join(" ");
    for want in [
        r#""c2" "=" "2""#,
        r#""p2" "=" "2""#,
        r#""p1" "=" "1""#,
        r#""sum" "=" "(" "(" "1" ")" "+" "(" "2" ")" ")""#,
        r#""opt" "=" "1""#,
        r#""std" "::" "vector" "<" "int" ">" "v""#,
    ] {
        assert!(text.contains(want), "{want} not in {text}");
    }
    let messages: Vec<String> =
        diags.iter().map(|d| format!("{:?} {}", d.level, d.message)).collect();
    assert_eq!(
        messages,
        [
            "Warning shown after the pop",
            "Warning 'ADD' macro redefined",
            "Error attempt to use a poisoned identifier",
        ],
        "{diags:#?}"
    );
    report("prefix", &snap, decode, prefix_tokens);
}

fn report(what: &str, snap: &Snapshot, decode: f64, tokens: usize) {
    let mut s = format!(
        "{what} ({tokens} tokens): encode {:.2} ms, decode {:.2} ms;",
        snap.encode * 1e3,
        decode * 1e3
    );
    for (name, b) in
        ["Interner", "SourceManager", "DiagEngine", "Preprocessor"].iter().zip(&snap.bytes)
    {
        let _ = write!(s, " {name} {} B", b.len());
    }
    println!("{s}");
}

/// The sizes and times of a `<bits/stdc++.h>` prefix (printed with
/// `--nocapture`), and its round trip.
#[test]
fn bits_stdcxx_round_trip() {
    let Some(cfg) = Config::host() else { return };
    let dir = TempDir::new("gnu");
    let prefix = dir.0.join("gnu.h");
    fs::write(&prefix, "#include <bits/stdc++.h>\n").unwrap();
    let interner = Interner::new();
    let sm = SourceManager::new();
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let deps = Deps::default();
    let mut pp = cfg.preprocessor(&sm, &engine, &interner, &deps);
    let pf = sm.load_file(&prefix).unwrap();
    pp.enter_main_file(pf);
    let n = tokens(&mut pp).len();
    pp.end_main_file().unwrap();
    assert!(sink.take().is_empty());
    let snap = snapshot(&interner, &sm, &engine, &pp);
    let t = Instant::now();
    let interner2: Interner = from_bytes(&snap.bytes[0]).unwrap();
    let sm2: SourceManager = from_bytes(&snap.bytes[1]).unwrap();
    let mut engine2 = DiagEngine::new(&sm2, &DiagOptions::default(), MemorySink::new());
    engine2.snap_restore(&mut Reader::new(&snap.bytes[2])).unwrap();
    let mut pp2 = cfg.preprocessor(&sm2, &engine2, &interner2, &Deps::default());
    pp2.snap_restore(&mut Reader::new(&snap.bytes[3])).unwrap();
    let decode = t.elapsed().as_secs_f64();
    assert!(canonical(&interner2, &sm2, &engine2, &pp2) == canonical(&interner, &sm, &engine, &pp));
    report("<bits/stdc++.h>", &snap, decode, n);
}

/// A small front end over in-memory headers, read to the end of `main`
/// (its own main file) unless `stop` says where to stop.
fn small<'a, 'd>(
    sm: &'a SourceManager,
    engine: &'d DiagEngine<'a>,
    interner: &'a Interner,
    main: &str,
) -> Preprocessor<'a, 'd> {
    let mut o = PpOptions::new(Std::Cxx26);
    "#define __small__ 1\n".clone_into(&mut o.predefines);
    o.modules = Some(ModuleOptions::default());
    let mut pp = Preprocessor::new(sm, engine, interner, o);
    let r = MemoryResolver::new()
        .with_header("h.h", "int a;\nint b;\n")
        .with_header("g.h", "#ifndef G\n#define G\n#define F(x) x\n#endif\n");
    pp.set_include_resolver(Box::new(r));
    let f = sm.add_virtual("t.cpp", main.to_owned()).unwrap();
    pp.enter_main_file(f);
    pp
}

fn refused(pp: &Preprocessor<'_, '_>) -> String {
    let mut w = Writer::new();
    let e = pp.snap_put(&mut w).unwrap_err();
    assert!(w.as_bytes().is_empty());
    assert_eq!(pp.snap_ready().unwrap_err(), e);
    e
}

/// The snapshot point must be quiescent: inside an include, with a token
/// read ahead or queued, inside a conditional group, with the main file
/// still open or a diagnostic hold open, the encoder refuses and names
/// the field.
#[test]
fn snapshot_outside_a_resting_point_is_refused() {
    let (sm, interner) = (SourceManager::new(), Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let ident = |pp: &mut Preprocessor<'_, '_>, name: &str| {
        let lex = *pp.lex_options();
        loop {
            let t = pp.next();
            assert!(!t.is_eof(), "{name} not found");
            if kiln_lex::spelling(&sm, &t, &lex) == name {
                return;
            }
        }
    };

    // [cpp.include]: in the middle of an included file.
    let mut pp = small(&sm, &engine, &interner, "#include \"h.h\"\nint after;\n");
    ident(&mut pp, "a");
    let e = refused(&pp);
    assert!(e.starts_with("Preprocessor.frames: 2 open, inside an included file: `h.h`"), "{e}");

    // A token queued behind a file-change marker.
    let mut pp = small(&sm, &engine, &interner, "#include \"h.h\"\n");
    loop {
        let item = pp.next_item();
        if let PpItem::Marker(Marker::FileChange(FileChange::Enter { file, .. })) = item
            && sm.file_name(file) == "h.h"
        {
            break;
        }
        assert!(!matches!(item, PpItem::Token(t) if t.is_eof()));
    }
    let e = refused(&pp);
    assert!(e.starts_with("Preprocessor.pending"), "{e}");

    // [cpp.replace]/10: the `(` lookahead after a function-like macro's
    // name read a token ahead.
    let mut pp = small(&sm, &engine, &interner, "#include \"g.h\"\nF + 1\n");
    ident(&mut pp, "F");
    let e = refused(&pp);
    assert!(e.starts_with("Preprocessor.frames[0].back"), "{e}");

    // [cpp.cond]: inside a conditional group.
    let mut pp = small(&sm, &engine, &interner, "#if 1\nint x;\n#endif\n");
    ident(&mut pp, "x");
    let e = refused(&pp);
    assert!(e.starts_with("Preprocessor.frames[0].conds"), "{e}");

    // The main file has ended but is still open.
    let mut pp = small(&sm, &engine, &interner, "int x;\n");
    while !pp.next().is_eof() {}
    let e = refused(&pp);
    assert!(e.contains("the main file is open"), "{e}");
    pp.end_main_file().unwrap();
    assert!(pp.snap_ready().is_ok());
    // And `end_main_file` before the end is refused.
    let mut pp = small(&sm, &engine, &interner, "int x;\n");
    ident(&mut pp, "x");
    assert!(pp.end_main_file().is_err());

    // A diagnostic hold.
    engine.hold();
    let mut w = Writer::new();
    let e = engine.snap_put(&mut w).unwrap_err();
    assert!(e.starts_with("DiagEngine.holding"), "{e}");
    engine.release();
    assert!(engine.snap_put(&mut w).is_ok());
}

/// A small snapshot and the fresh consumer to restore it into.
fn small_snapshot() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let (sm, interner) = (SourceManager::new(), Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let main = "#include \"g.h\"\n#define P [[fallthrough]]\n#pragma push_macro(\"P\")\n#define P 2\n\
                #pragma GCC poison bad\n#pragma clang diagnostic ignored \"-W#warnings\"\n\
                int c = __COUNTER__;\n_Pragma(\"clang diagnostic push\")\n";
    let mut pp = small(&sm, &engine, &interner, main);
    while !pp.next().is_eof() {}
    pp.end_main_file().unwrap();
    let mut we = Writer::new();
    engine.snap_put(&mut we).unwrap();
    let mut wp = Writer::new();
    pp.snap_put(&mut wp).unwrap();
    (to_bytes(&interner), to_bytes(&sm), we.into_bytes(), wp.into_bytes())
}

/// Restores a (maybe corrupt) engine and preprocessor snapshot into a
/// fresh consumer; when both decode, reads a main file with them.
fn restore_and_use(ib: &[u8], sb: &[u8], eb: &[u8], pb: &[u8]) -> bool {
    let interner: Interner = from_bytes(ib).unwrap();
    let sm: SourceManager = from_bytes(sb).unwrap();
    let mut engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let mut r = Reader::new(eb);
    if engine.snap_restore(&mut r).and_then(|()| r.finish()).is_err() {
        return false;
    }
    let mut o = PpOptions::new(Std::Cxx26);
    o.modules = Some(ModuleOptions::default());
    let mut pp = Preprocessor::new(&sm, &engine, &interner, o);
    pp.set_include_resolver(Box::new(MemoryResolver::new().with_header("g.h", "#define G 2\n")));
    let mut r = Reader::new(pb);
    if pp.snap_restore(&mut r).and_then(|()| r.finish()).is_err() {
        return false;
    }
    let f = sm
        .add_virtual(
            "u.cpp",
            "#include \"g.h\"\nF(1) P G bad __COUNTER__\n#pragma pop_macro(\"P\")\nP\n#warning w\n"
                .to_owned(),
        )
        .unwrap();
    pp.enter_next_main_file(f).unwrap();
    while !pp.next().is_eof() {}
    true
}

/// Corrupt snapshots: truncation at a sample of offsets and bit flips are
/// errors or values that work, never a panic.
#[test]
fn corrupt_snapshots_never_panic() {
    let (ib, sb, eb, pb) = small_snapshot();
    assert!(restore_and_use(&ib, &sb, &eb, &pb));
    for (k, bytes) in [&eb, &pb].into_iter().enumerate() {
        let step = (bytes.len() / 300).max(1);
        let with = |b: &[u8]| {
            if k == 0 {
                restore_and_use(&ib, &sb, b, &pb)
            } else {
                restore_and_use(&ib, &sb, &eb, b)
            }
        };
        for cut in (0..bytes.len()).step_by(step) {
            assert!(!with(&bytes[..cut]), "root {k} cut at {cut} decoded");
        }
        let mut ok = 0;
        for at in (0..bytes.len()).step_by(step) {
            for flip in [0x01u8, 0x10, 0x80, 0xff] {
                let mut b = bytes.clone();
                b[at] ^= flip;
                ok += usize::from(with(&b));
            }
        }
        println!("root {k}: {} bytes, {ok} flipped snapshots decoded", bytes.len());
    }
}

/// The schema hash of each root is a function of the sources: equal on
/// every computation, different between roots.
#[test]
fn schema_hashes_are_stable() {
    let hash = |f: fn(&mut Schema)| {
        let mut s = Schema::new();
        f(&mut s);
        s.hash()
    };
    let e = hash(DiagEngine::snap_schema);
    let p = hash(Preprocessor::snap_schema);
    assert_eq!(e, hash(DiagEngine::snap_schema));
    assert_eq!(p, hash(Preprocessor::snap_schema));
    assert_ne!(e, p);
    println!("schema DiagEngine {e:016x} Preprocessor {p:016x}");
}
