//! `FrontEndOptions::overlay`, stage 2 (WP-2391, spec/source.md
//! "Overlay", spec/driver.md "Front-end library entry"): files the
//! overlay creates or removes. Each case is one front-end run over a tree
//! with an overlay, compared with the same command over a copy of the
//! tree with the edit applied (created files written, their directories
//! made; removed files unlinked; replaced files rewritten): equal
//! diagnostics, token count, AST dump, files read (names and bytes),
//! consulted paths and lookup log. Both runs use the same relative
//! command line in their own working directory, so every name they print
//! is the same.
#![forbid(unsafe_code)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use kiln_ast::dump::{DumpOptions, dump_to_string};
use kiln_diag::{DiagId, SharedBuffer};
use kiln_driver::SharedWriter;
use kiln_driver::front_end::{Consulted, ConsultedKind, FrontEnd, FrontEndOptions, run_front_end};
use kiln_pp::LookupLog;
use kiln_source::{FileOrigin, Overlay};

/// A fresh directory under the target directory, removed when dropped.
struct Tree(PathBuf);

impl Tree {
    fn new(name: &str) -> Tree {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("kiln-overlay2-{name}-{}-{n}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        Tree(dir)
    }

    /// Writes `bytes` at the relative path `name`, making its directory.
    fn write(&self, name: &str, bytes: &[u8]) {
        let p = self.0.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// A copy of the tree (symbolic links copied as links), in a fresh
    /// directory.
    fn copy(&self, name: &str) -> Tree {
        fn walk(from: &Path, to: &Path) {
            for e in std::fs::read_dir(from).unwrap() {
                let e = e.unwrap();
                let (src, dst) = (e.path(), to.join(e.file_name()));
                let ty = e.file_type().unwrap();
                if ty.is_symlink() {
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(std::fs::read_link(&src).unwrap(), &dst).unwrap();
                } else if ty.is_dir() {
                    std::fs::create_dir_all(&dst).unwrap();
                    walk(&src, &dst);
                } else {
                    std::fs::copy(&src, &dst).unwrap();
                }
            }
        }
        let t = Tree::new(name);
        walk(&self.0, &t.0);
        t
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|&s| s.to_owned()).collect()
}

/// One change of an overlay, and of the copied tree.
#[derive(Clone, Copy)]
enum Edit<'a> {
    Create(&'a str, &'a [u8]),
    Replace(&'a str, &'a [u8]),
    Remove(&'a str),
}

/// The overlay of `edits` over `t`.
fn overlay_of(t: &Tree, edits: &[Edit<'_>]) -> Overlay {
    let mut o = Overlay::new();
    for e in edits {
        match *e {
            Edit::Create(n, b) => o.create(&t.path(n), Arc::from(b)).unwrap(),
            Edit::Replace(n, b) => o.replace(&t.path(n), Arc::from(b)).unwrap(),
            Edit::Remove(n) => o.remove(&t.path(n)).unwrap(),
        }
    }
    o
}

/// A copy of `t` with `edits` applied.
fn edited_copy(t: &Tree, edits: &[Edit<'_>]) -> Tree {
    let c = t.copy("copy");
    for e in edits {
        match *e {
            Edit::Create(n, b) | Edit::Replace(n, b) => c.write(n, b),
            Edit::Remove(n) => std::fs::remove_file(c.path(n)).unwrap(),
        }
    }
    c
}

/// What a parse is compared on.
#[derive(Debug, PartialEq, Eq)]
struct Parse {
    tokens: u64,
    dump: String,
    /// Every buffer: its name and the bytes it was decoded from.
    files: Vec<(String, Option<Vec<u8>>)>,
    consulted: Vec<Consulted>,
    lookups: Option<LookupLog>,
}

/// Everything a run is compared on.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    diagnostics: String,
    errors: u32,
    warnings: u32,
    parse: Option<Parse>,
}

/// The run of `cl` in `dir` with `overlay` (lookups recorded), and each
/// buffer's origin.
fn run(dir: &Path, cl: &[String], overlay: Option<Arc<Overlay>>) -> (Outcome, Vec<FileOrigin>) {
    let buf = SharedBuffer::new();
    let mut o = FrontEndOptions::default();
    o.stderr = Some(SharedWriter::new(buf.clone()));
    o.working_directory = Some(dir.to_owned());
    o.record_lookups = true;
    o.overlay = overlay;
    let r = run_front_end("kiln++", cl, o, |fe: &FrontEnd<'_, '_>| {
        let opts = DumpOptions {
            locs: true,
            all_files: true,
            sources: Some(fe.sm),
            main_file: fe.main_file,
            ..DumpOptions::default()
        };
        let files = fe
            .sm
            .file_ids()
            .map(|f| (fe.sm.file_name(f).to_owned(), fe.sm.original_bytes(f).map(|b| b.to_vec())))
            .collect();
        let origins: Vec<FileOrigin> = fe.sm.file_ids().map(|f| fe.sm.file_origin(f)).collect();
        let parse = Parse {
            tokens: fe.tokens,
            dump: dump_to_string(fe.ast, &opts),
            files,
            consulted: fe.consulted.to_vec(),
            lookups: fe.lookups.cloned(),
        };
        (parse, origins)
    })
    .unwrap();
    let (parse, origins) = r.value.map_or((None, Vec::new()), |(p, o)| (Some(p), o));
    (
        Outcome { diagnostics: buf.contents(), errors: r.errors, warnings: r.warnings, parse },
        origins,
    )
}

/// Runs `cl` over `t` with the overlay of `edits` and over a copy of `t`
/// with `edits` applied; asserts the two are equal and that no buffer of
/// the copy's run is an `Overlay` one. Returns the overlay run.
fn same_as_copy(t: &Tree, cl: &[&str], edits: &[Edit<'_>]) -> (Outcome, Vec<FileOrigin>) {
    let cl = argv(cl);
    let copy = edited_copy(t, edits);
    let with = run(&t.0, &cl, Some(Arc::new(overlay_of(t, edits))));
    let (reference, ref_origins) = run(&copy.0, &cl, None);
    assert_eq!(with.0, reference, "the overlay run differs from the edited copy's");
    assert!(ref_origins.iter().all(|&o| o != FileOrigin::Overlay));
    with
}

impl Outcome {
    fn parse(&self) -> &Parse {
        self.parse.as_ref().unwrap_or_else(|| panic!("no parse: {}", self.diagnostics))
    }
}

/// The origin of the buffer named `name`.
fn origin_of(o: &(Outcome, Vec<FileOrigin>), name: &str) -> FileOrigin {
    let files = &o.0.parse().files;
    let i = files.iter().position(|(n, _)| n == name).unwrap_or_else(|| {
        panic!("no buffer '{name}' in {:?}", files.iter().map(|f| &f.0).collect::<Vec<_>>())
    });
    o.1[i]
}

/// [cpp.include]/2-3, `#include_next`, `-include`: created headers
/// reached as `"q.h"`, as `<a.h>`, through the second of two search
/// directories with `#include_next`, and by `-include`.
#[test]
fn created_header_by_every_route() {
    let t = Tree::new("routes");
    t.write("inc/n.h", b"int n_first;\n#include_next <n.h>\n");
    std::fs::create_dir_all(t.path("inc2")).unwrap();
    t.write("main.cpp", b"#include \"q.h\"\n#include <a.h>\n#include <n.h>\nint m = PRE;\n");
    let cl = ["-fsyntax-only", "-Iinc", "-Iinc2", "-include", "pre.h", "main.cpp"];
    let o = same_as_copy(
        &t,
        &cl,
        &[
            Edit::Create("q.h", b"int q_new;\n"),
            Edit::Create("inc/a.h", b"int a_new;\n"),
            Edit::Create("inc2/n.h", b"int n_new;\n"),
            Edit::Create("pre.h", b"#define PRE 7\n"),
        ],
    );
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    for n in ["q_new", "a_new", "n_new", "n_first"] {
        assert!(o.0.parse().dump.contains(n), "{n}");
    }
    for (name, origin) in [
        ("main.cpp", FileOrigin::Disk),
        ("./q.h", FileOrigin::Overlay),
        ("inc/a.h", FileOrigin::Overlay),
        ("inc/n.h", FileOrigin::Disk),
        ("inc2/n.h", FileOrigin::Overlay),
        ("./pre.h", FileOrigin::Overlay),
    ] {
        assert_eq!(origin_of(&o, name), origin, "{name}");
    }
}

/// [cpp.include]: a header created in an earlier search directory
/// shadows a later directory's copy on disk; `#include_next` from it
/// reaches the disk's.
#[test]
fn created_header_shadows_a_later_directory() {
    let t = Tree::new("shadow");
    t.write("inc2/s.h", b"int s_disk;\n");
    std::fs::create_dir_all(t.path("inc")).unwrap();
    t.write("main.cpp", b"#include <s.h>\nint m = s_new + s_disk;\n");
    let o = same_as_copy(
        &t,
        &["-fsyntax-only", "-Iinc", "-Iinc2", "main.cpp"],
        &[Edit::Create("inc/s.h", b"int s_new;\n#include_next <s.h>\n")],
    );
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    assert_eq!(origin_of(&o, "inc/s.h"), FileOrigin::Overlay);
    assert_eq!(origin_of(&o, "inc2/s.h"), FileOrigin::Disk);
}

/// [cpp.include], [cpp.cond]: a removed header lets a later directory's
/// copy win; with none, `__has_include` is 0 and `#include` is the
/// fatal `'g.h' file not found`.
#[test]
fn removed_header() {
    let t = Tree::new("removed");
    t.write("inc/r.h", b"int r_first;\n");
    t.write("inc2/r.h", b"int r_second;\n");
    t.write("g.h", b"int g;\n");
    t.write(
        "main.cpp",
        b"#include <r.h>\nint m = r_second;\n#if __has_include(\"g.h\")\n#error has g.h\n#endif\n#include \"g.h\"\n",
    );
    let o = same_as_copy(
        &t,
        &["-fsyntax-only", "-Iinc", "-Iinc2", "main.cpp"],
        &[Edit::Remove("inc/r.h"), Edit::Remove("g.h")],
    );
    assert_eq!(o.0.errors, 1, "{}", o.0.diagnostics);
    assert!(o.0.diagnostics.contains("fatal error: 'g.h' file not found"), "{}", o.0.diagnostics);
    assert!(!o.0.diagnostics.contains("has g.h"), "{}", o.0.diagnostics);
    assert_eq!(origin_of(&o, "inc2/r.h"), FileOrigin::Disk);
}

/// [cpp.include] header search list: a created file two directories
/// deep, its directory given as `-I`: kept in the search list (`-v`
/// prints no "ignoring nonexistent directory" for it), and `-I new -I
/// ./new` is one directory.
#[test]
fn created_directory_in_the_search_list() {
    let t = Tree::new("searchlist");
    t.write("main.cpp", b"#include <h.h>\n#include <deeper/h.h>\nint m = h1;\n");
    let cl = ["-fsyntax-only", "-v", "-Inew/deeper", "-Inew", "-I./new", "-Inone", "main.cpp"];
    let edits = [
        Edit::Create("new/deeper/h.h", b"#pragma once\nint h1;\n"),
        Edit::Create("new/h.h", b"int h0;\n"),
    ];
    let o = same_as_copy(&t, &cl, &edits);
    // `<h.h>` is found in `new/deeper` first (`h0` is never declared),
    // and `<deeper/h.h>` from `new` is the same file, entered once.
    assert!(
        o.0.diagnostics.contains("ignoring duplicate directory \"./new\""),
        "{}",
        o.0.diagnostics
    );
    assert!(o.0.diagnostics.contains("ignoring nonexistent directory \"none\""));
    assert!(!o.0.diagnostics.contains("nonexistent directory \"new"), "{}", o.0.diagnostics);
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    assert!(!o.0.parse().dump.contains("h0"));
}

/// [cpp.embed]: `#embed` and `__has_embed` of created files (empty and
/// not) and of a removed one.
#[test]
fn embed_of_created_and_removed_files() {
    let t = Tree::new("embed");
    t.write("gone.bin", b"G");
    t.write(
        "main.cpp",
        b"constexpr unsigned char d[] = {\n#embed \"data.bin\"\n};\n\
          static_assert(sizeof d == 3 && d[0] == 'x');\n\
          #if __has_embed(\"data.bin\") != __STDC_EMBED_FOUND__\n#error data\n#endif\n\
          #if __has_embed(\"empty.bin\") != __STDC_EMBED_EMPTY__\n#error empty\n#endif\n\
          #if __has_embed(\"gone.bin\") != __STDC_EMBED_NOT_FOUND__\n#error gone\n#endif\n",
    );
    let cl = ["-fsyntax-only", "-std=c++26", "-Wno-c23-extensions", "main.cpp"];
    let edits = [
        Edit::Create("data.bin", b"xyz"),
        Edit::Create("empty.bin", b""),
        Edit::Remove("gone.bin"),
    ];
    let o = same_as_copy(&t, &cl, &edits);
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    let kinds: Vec<_> = o.0.parse().consulted.iter().map(|c| (c.kind, c.path.as_str())).collect();
    assert_eq!(
        kinds,
        [(ConsultedKind::Embed, "./data.bin"), (ConsultedKind::Probe, "./empty.bin")]
    );
}

/// [cpp.include] `#pragma once` and an include guard in created headers
/// reached by two spellings and through a symbolic link to their
/// directory: each entered once (a second entry would redefine its
/// variable).
#[cfg(unix)]
#[test]
fn pragma_once_and_guard_by_identity() {
    let t = Tree::new("once");
    std::fs::create_dir_all(t.path("inc")).unwrap();
    std::os::unix::fs::symlink("inc", t.path("alias")).unwrap();
    let mut main = String::new();
    for h in ["once.h", "guard.h"] {
        for d in ["inc/", "inc/../inc/", "alias/"] {
            let _ = writeln!(main, "#include \"{d}{h}\"");
        }
    }
    t.write("main.cpp", main.as_bytes());
    let o = same_as_copy(
        &t,
        &["-fsyntax-only", "main.cpp"],
        &[
            Edit::Create("inc/once.h", b"#pragma once\nint once_v;\n"),
            Edit::Create(
                "inc/guard.h",
                b"#ifndef GUARD_H\n#define GUARD_H\nint guard_v;\n#endif\n",
            ),
        ],
    );
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    let headers = o.0.parse().files.iter().filter(|(n, _)| n.ends_with("once.h")).count();
    assert_eq!(headers, 1);
}

/// [lex.phases]: a created main input is parsed; a removed one is the
/// driver's `no such file or directory`.
#[test]
fn created_and_removed_main_input() {
    let t = Tree::new("main");
    t.write("old.cpp", b"int old_v;\n");
    let o = same_as_copy(
        &t,
        &["-fsyntax-only", "new.cpp"],
        &[Edit::Create("new.cpp", b"int new_v;\n#warning created\n")],
    );
    assert!(o.0.diagnostics.contains("new.cpp:2:2: warning: created"), "{}", o.0.diagnostics);
    assert_eq!(origin_of(&o, "new.cpp"), FileOrigin::Overlay);
    let o = same_as_copy(&t, &["-fsyntax-only", "old.cpp"], &[Edit::Remove("old.cpp")]);
    assert!(o.0.parse.is_none());
    assert!(
        o.0.diagnostics.contains("no such file or directory: 'old.cpp'"),
        "{}",
        o.0.diagnostics
    );
}

/// Header search's listings (spec/pp.md "Header search"): after four
/// misses in a directory its listing answers further misses with no
/// system call; a header the overlay creates there, and one in a
/// directory it creates there, are still found.
#[test]
fn listings_hold_created_names() {
    let t = Tree::new("listings");
    for i in 0..3 {
        t.write(&format!("many/f{i}.h"), b"\n");
    }
    let mut main = String::new();
    for i in 0..5 {
        let _ = writeln!(main, "#if __has_include(<m{i}.h>)\n#error m{i}\n#endif");
    }
    main.push_str("#include <late.h>\n#include <sub/x.h>\nint m = late + x;\n");
    t.write("main.cpp", main.as_bytes());
    let o = same_as_copy(
        &t,
        &["-fsyntax-only", "-Imany", "main.cpp"],
        &[Edit::Create("many/late.h", b"int late;\n"), Edit::Create("many/sub/x.h", b"int x;\n")],
    );
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
}

/// [cpp.include]: `"newdir/../x.h"`, `newdir` a directory the overlay
/// creates and `x.h` on disk.
#[test]
fn dot_dot_out_of_a_created_directory() {
    let t = Tree::new("dotdot");
    t.write("x.h", b"int x_disk;\n");
    t.write(
        "main.cpp",
        b"#include \"newdir/../x.h\"\n#include \"newdir/k.h\"\nint m = x_disk + k;\n",
    );
    let o = same_as_copy(
        &t,
        &["-fsyntax-only", "main.cpp"],
        &[Edit::Create("newdir/k.h", b"int k;\n")],
    );
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    assert_eq!(origin_of(&o, "./newdir/../x.h"), FileOrigin::Disk);
}

/// [cpp.cond]: a created header probed by `__has_include` and then
/// included is not consulted; one only probed is.
#[test]
fn consulted_created_headers() {
    let t = Tree::new("consulted");
    t.write(
        "main.cpp",
        b"#if __has_include(\"a.h\") && __has_include(\"p.h\")\n#include \"a.h\"\nint yes;\n#endif\n",
    );
    let o = same_as_copy(
        &t,
        &["-fsyntax-only", "main.cpp"],
        &[Edit::Create("a.h", b"int a;\n"), Edit::Create("p.h", b"int p;\n")],
    );
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    let probes: Vec<_> = o.0.parse().consulted.iter().map(|c| (c.kind, c.path.as_str())).collect();
    assert_eq!(probes, [(ConsultedKind::Probe, "./p.h")]);
}

/// Overlays A and B and none, shared as `Arc`s by runs on eight threads
/// at once: each run equals its own edited copy's.
#[test]
fn concurrent_runs_see_their_own_overlay() {
    let t = Tree::new("threads");
    t.write("inc/h.h", b"int disk_v;\n");
    t.write("main.cpp", b"#include <h.h>\n#if __has_include(<new.h>)\n#include <new.h>\n#endif\n");
    let cl = argv(&["-fsyntax-only", "-Iinc", "main.cpp"]);
    let a: Vec<Edit<'_>> = vec![Edit::Create("inc/new.h", b"int a_v;\n")];
    let b: Vec<Edit<'_>> =
        vec![Edit::Replace("inc/h.h", b"int b_v;\n"), Edit::Create("inc/new.h", b"int b2_v;\n")];
    let cases: [(Option<Arc<Overlay>>, Tree); 3] = [
        (Some(Arc::new(overlay_of(&t, &a))), edited_copy(&t, &a)),
        (Some(Arc::new(overlay_of(&t, &b))), edited_copy(&t, &b)),
        (None, t.copy("none")),
    ];
    let reference: Vec<Outcome> = cases.iter().map(|(_, c)| run(&c.0, &cl, None).0).collect();
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..8)
            .map(|i| {
                let (o, cl, root) = (cases[i % 3].0.clone(), &cl, t.0.as_path());
                s.spawn(move || (i % 3, run(root, cl, o).0))
            })
            .collect();
        for h in hs {
            let (k, out) = h.join().unwrap();
            assert_eq!(out, reference[k], "case {k}");
        }
    });
}

/// The module file of `m.cppm` in `t`, built by the `kiln++` executable.
fn build_module(t: &Tree) {
    let kilnpp = Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++");
    let o = Command::new(&kilnpp)
        .args(["-std=c++26", "--precompile", "m.cppm", "-o", "m.pcm"])
        .current_dir(&t.0)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

fn refused(dir: &Path, cl: &[&str], overlay: Overlay) -> (Option<()>, u32, Option<DiagId>, String) {
    let buf = SharedBuffer::new();
    let mut o = FrontEndOptions::default();
    o.stderr = Some(SharedWriter::new(buf.clone()));
    o.working_directory = Some(dir.to_owned());
    o.overlay = Some(Arc::new(overlay));
    let r = run_front_end("kiln++", &argv(cl), o, |_| ()).unwrap();
    (r.value, r.errors, r.first_error.and_then(|e| e.id), buf.contents())
}

/// [module.import]: a module file records its inputs, not its lookups,
/// so an overlay that creates any file refuses every module file: one
/// the command line names before the job
/// (`drv_front_end_overlay_creates`), one found through
/// `-fprebuilt-module-path=` at its import
/// (`pp_module_file_overlay_creates`). An overlay that removes an input
/// is refused as one that replaces it.
#[test]
fn module_files_refused_under_creating_or_removing_overlays() {
    let t = Tree::new("modules");
    t.write("m.cppm", b"export module m;\nexport constexpr int twice(int x) { return 2 * x; }\n");
    t.write("a.cpp", b"import m;\nstatic_assert(twice(3) == 6);\n");
    build_module(&t);
    let creating = || overlay_of(&t, &[Edit::Create("unrelated.h", b"\n")]);
    let named = ["-fsyntax-only", "-std=c++26", "-fmodule-file=m=m.pcm", "a.cpp"];
    let (value, errors, id, text) = refused(&t.0, &named, creating());
    assert_eq!(
        (value, errors, id),
        (None, 1, Some(DiagId::drv_front_end_overlay_creates)),
        "{text}"
    );
    assert!(text.contains("the overlay creates '"), "{text}");
    assert!(text.contains("unrelated.h', which the module file 'm.pcm' cannot be checked against"));
    let prebuilt = ["-fsyntax-only", "-std=c++26", "-fprebuilt-module-path=.", "a.cpp"];
    let (value, _, id, text) = refused(&t.0, &prebuilt, creating());
    assert_eq!((value, id), (Some(()), Some(DiagId::pp_module_file_overlay_creates)), "{text}");

    let removing = || overlay_of(&t, &[Edit::Remove("m.cppm")]);
    let (value, _, id, text) = refused(&t.0, &named, removing());
    assert_eq!((value, id), (None, Some(DiagId::drv_front_end_overlay_input)), "{text}");
    assert!(text.contains("the overlay replaces or removes 'm.cppm'"), "{text}");
    let (value, _, id, text) = refused(&t.0, &prebuilt, removing());
    assert_eq!((value, id), (Some(()), Some(DiagId::pp_module_file_overlaid)), "{text}");
}

/// An overlay with replacements only behaves as stage 1's (the edited
/// copy's results, lookup log included); an empty one is none.
#[test]
fn replacements_only_and_empty() {
    let t = Tree::new("replaced");
    t.write("inc/h.h", b"int h_disk;\n");
    t.write("main.cpp", b"#include <h.h>\n#include \"inc/h.h\"\nint m = h_new;\n");
    let cl = ["-fsyntax-only", "-Iinc", "main.cpp"];
    let o = same_as_copy(&t, &cl, &[Edit::Replace("inc/h.h", b"#pragma once\nint h_new;\n")]);
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    assert_eq!(origin_of(&o, "inc/h.h"), FileOrigin::Overlay);
    let none = run(&t.0, &argv(&cl), None);
    let empty = run(&t.0, &argv(&cl), Some(Arc::new(Overlay::new())));
    assert_eq!(none, empty);
}

/// A loader that counts its calls and gives `bytes`, or fails when
/// `bytes` is `None`.
fn counting(calls: &Arc<AtomicU32>, bytes: Option<&'static [u8]>) -> kiln_source::Loader {
    let calls = Arc::clone(calls);
    Box::new(move || {
        calls.fetch_add(1, Ordering::SeqCst);
        bytes.map(Arc::from).ok_or_else(|| std::io::Error::other("the client could not make it"))
    })
}

/// The errors, the AST dump and the overlay failures of `cl` in `dir`.
fn run_lazy(
    dir: &Path,
    cl: &[String],
    overlay: &Arc<Overlay>,
) -> (u32, String, Vec<(PathBuf, String)>) {
    let mut o = FrontEndOptions::default();
    o.stderr = Some(SharedWriter::new(SharedBuffer::new()));
    o.working_directory = Some(dir.to_owned());
    o.overlay = Some(Arc::clone(overlay));
    let r = run_front_end("kiln++", cl, o, |fe: &FrontEnd<'_, '_>| {
        dump_to_string(fe.ast, &DumpOptions::default())
    })
    .unwrap();
    (r.errors, r.value.unwrap_or_default(), r.overlay_failures)
}

/// Lazy contents (spec/source.md "Overlay"): sixteen concurrent runs
/// sharing one overlay call each loader they read once in all; an entry
/// no lookup names is never loaded (one `__has_include` finds is: header
/// search loads what it finds, as for a file on disk, and a probed
/// file's content is not entered); a failing loader makes its file not found, with no panic, and is in
/// `overlay_failures` of every run that read it (`__has_embed` and
/// `#include` both read), in the order read, and in the overlay's
/// `load_failures`.
#[test]
fn lazy_contents_load_once_and_fail_visibly() {
    let t = Tree::new("lazy");
    t.write("inc/h.h", b"int disk_v;\n");
    t.write(
        "main.cpp",
        b"#include <h.h>\n#include <made.h>\n#if __has_include(<probed.h>)\nint probed_v;\n#endif\nint m = made_v + replaced_v + probed_v;\n",
    );
    t.write("bad.cpp", b"#if __has_embed(<bad.bin>)\nint embedded_v;\n#endif\n#include <bad.h>\n");
    let calls: Vec<Arc<AtomicU32>> = (0..6).map(|_| Arc::new(AtomicU32::new(0))).collect();
    let mut o = Overlay::new();
    o.replace_with(&t.path("inc/h.h"), counting(&calls[0], Some(b"int replaced_v;\n"))).unwrap();
    o.create_with(&t.path("inc/made.h"), counting(&calls[1], Some(b"int made_v;\n"))).unwrap();
    o.create_with(&t.path("inc/probed.h"), counting(&calls[2], Some(b"#error read\n"))).unwrap();
    o.create_with(&t.path("inc/never.h"), counting(&calls[3], Some(b""))).unwrap();
    o.create_with(&t.path("inc/bad.h"), counting(&calls[4], None)).unwrap();
    o.create_with(&t.path("inc/bad.bin"), counting(&calls[5], None)).unwrap();
    let o = Arc::new(o);
    let cl = argv(&["-fsyntax-only", "-Iinc", "main.cpp"]);
    let bad = argv(&["-fsyntax-only", "-Iinc", "--embed-dir=inc", "bad.cpp"]);
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..16)
            .map(|i| {
                let (o, root) = (&o, t.0.as_path());
                let cl = if i % 4 == 3 { &bad } else { &cl };
                s.spawn(move || (i % 4 == 3, run_lazy(root, cl, o)))
            })
            .collect();
        for h in hs {
            let (is_bad, (errors, dump, failures)) = h.join().unwrap();
            if is_bad {
                assert!(errors > 0);
                assert!(!dump.contains("embedded_v"), "{dump}");
                let why = "the client could not make it".to_owned();
                assert_eq!(
                    failures,
                    [(t.path("inc/bad.bin"), why.clone()), (t.path("inc/bad.h"), why)]
                );
            } else {
                assert_eq!((errors, &failures[..]), (0, &[][..]), "{dump}");
                assert!(dump.contains("replaced_v") && dump.contains("made_v"), "{dump}");
            }
        }
    });
    let counts: Vec<u32> = calls.iter().map(|c| c.load(Ordering::SeqCst)).collect();
    assert_eq!(counts, [1, 1, 1, 0, 1, 1]);
    assert_eq!(o.load_failures().len(), 2);
    assert_eq!(o.load_failures()[0].0, t.path("inc/bad.bin"));
}
