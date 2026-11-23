//! `FrontEndOptions::overlay` (WP-2390, spec/source.md "Overlay",
//! spec/driver.md "Front-end library entry"): replacement contents for
//! files that exist. Each case is one front-end run over a tree with an
//! overlay, compared with the same command over a copy of the tree in
//! which the overlaid files hold the overlay's bytes: equal diagnostics,
//! token count, AST dump, files read (names and bytes) and consulted
//! paths. Both runs use the same relative command line in their own
//! working directory, so every name they print is the same.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use kiln_ast::dump::{DumpOptions, dump_to_string};
use kiln_diag::{DiagId, SharedBuffer};
use kiln_driver::SharedWriter;
use kiln_driver::front_end::{
    Consulted, ConsultedKind, Content, FrontEnd, FrontEndOptions, run_front_end,
};
use kiln_source::{FileOrigin, Overlay};

/// A fresh directory under the target directory, removed when dropped.
struct Tree(PathBuf);

impl Tree {
    fn new(name: &str) -> Tree {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("kiln-overlay-{name}-{}-{n}", std::process::id()));
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

    /// A copy of the tree (symbolic links copied as links, hard links as
    /// files), in a fresh directory.
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

/// Everything a run is compared on, and the buffers' origins (which
/// differ: `Overlay` against `Disk`).
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    diagnostics: String,
    errors: u32,
    warnings: u32,
    tokens: u64,
    dump: String,
    /// Every buffer: its name and the bytes it was decoded from.
    files: Vec<(String, Option<Vec<u8>>)>,
    consulted: Vec<Consulted>,
}

/// The run of `cl` in `dir` with `overlay`, and each buffer's origin.
fn run(dir: &Path, cl: &[String], overlay: Option<Overlay>) -> (Outcome, Vec<FileOrigin>) {
    let buf = SharedBuffer::new();
    let mut o = FrontEndOptions::default();
    o.stderr = Some(SharedWriter::new(buf.clone()));
    o.working_directory = Some(dir.to_owned());
    o.overlay = overlay.map(Arc::new);
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
        let origins = fe.sm.file_ids().map(|f| fe.sm.file_origin(f)).collect();
        (fe.tokens, dump_to_string(fe.ast, &opts), files, fe.consulted.to_vec(), origins)
    })
    .unwrap();
    let (tokens, dump, files, consulted, origins) = r.value.expect("a parse");
    let outcome = Outcome {
        diagnostics: buf.contents(),
        errors: r.errors,
        warnings: r.warnings,
        tokens,
        dump,
        files,
        consulted,
    };
    (outcome, origins)
}

/// The origin of the buffer named `name`.
fn origin_of(o: &(Outcome, Vec<FileOrigin>), name: &str) -> FileOrigin {
    let i = o.0.files.iter().position(|(n, _)| n == name).unwrap_or_else(|| {
        panic!("no buffer '{name}' in {:?}", o.0.files.iter().map(|f| &f.0).collect::<Vec<_>>())
    });
    o.1[i]
}

impl Outcome {
    /// The buffers' names, in creation order.
    fn o_names(&self) -> Vec<&str> {
        self.files.iter().map(|(n, _)| n.as_str()).collect()
    }
}

/// Runs `cl` over `tree` with `replaced` (relative name, bytes) as the
/// overlay, and over a copy of the tree with those files replaced on
/// disk; asserts the two are equal and that every replaced file read is
/// an `Overlay` buffer. Returns the overlay run.
fn same_as_copy(
    tree: &Tree,
    cl: &[&str],
    replaced: &[(&str, &[u8])],
) -> (Outcome, Vec<FileOrigin>) {
    let cl = argv(cl);
    let copy = tree.copy("copy");
    let mut overlay = Overlay::new();
    for (name, bytes) in replaced {
        overlay.replace(&tree.path(name), Arc::from(*bytes)).unwrap();
        copy.write(name, bytes);
    }
    let with = run(&tree.0, &cl, Some(overlay));
    let (reference, ref_origins) = run(&copy.0, &cl, None);
    assert_eq!(with.0, reference, "the overlay run differs from the copied tree's");
    assert!(ref_origins.iter().all(|&o| o != FileOrigin::Overlay));
    with
}

/// [cpp.include] / [lex.phases]: the main file overlaid.
#[test]
fn main_file() {
    let t = Tree::new("main");
    t.write("main.cpp", b"int on_disk;\n");
    let new: &[u8] = b"int a = 1;\nint b = \"not an int\";\n#warning from the overlay\n";
    let o = same_as_copy(&t, &["-fsyntax-only", "main.cpp"], &[("main.cpp", new)]);
    assert_eq!(o.0.errors, 1, "{}", o.0.diagnostics);
    assert!(
        o.0.diagnostics.contains("main.cpp:3:2: warning: from the overlay"),
        "{}",
        o.0.diagnostics
    );
    assert_eq!(origin_of(&o, "main.cpp"), FileOrigin::Overlay);
    assert!(!o.0.dump.contains("on_disk"));
}

/// [cpp.include]/2-3, `#include_next`, `-include`: an overlaid header
/// reached as `"q.h"`, as `<a.h>`, through the second search directory
/// with `#include_next`, and by `-include`.
#[test]
fn header_by_every_route() {
    let t = Tree::new("routes");
    t.write("q.h", b"int q_disk;\n");
    t.write("inc/a.h", b"int a_disk;\n");
    t.write("inc/n.h", b"int n_first;\n#include_next <n.h>\n");
    t.write("inc2/n.h", b"int n_disk;\n");
    t.write("pre.h", b"#define PRE_DISK 1\n");
    t.write("main.cpp", b"#include \"q.h\"\n#include <a.h>\n#include <n.h>\nint m = PRE;\n");
    let cl = ["-fsyntax-only", "-Iinc", "-Iinc2", "-include", "pre.h", "main.cpp"];
    let o = same_as_copy(
        &t,
        &cl,
        &[
            ("q.h", b"int q_new;\n"),
            ("inc/a.h", b"int a_new;\n"),
            ("inc2/n.h", b"int n_new;\n"),
            ("pre.h", b"#define PRE 7\n"),
        ],
    );
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    for n in ["q_new", "a_new", "n_new", "n_first"] {
        assert!(o.0.dump.contains(n), "{n}: {}", o.0.dump);
    }
    for (name, origin) in [
        ("main.cpp", FileOrigin::Disk),
        ("./q.h", FileOrigin::Overlay),
        ("inc/a.h", FileOrigin::Overlay),
        ("inc/n.h", FileOrigin::Disk),
        ("inc2/n.h", FileOrigin::Overlay),
    ] {
        assert_eq!(origin_of(&o, name), origin, "{name}");
    }
    assert!(o.0.files.iter().any(|(n, _)| n.ends_with("pre.h")), "{:?}", o.0.o_names());
}

/// The same header by two spellings and through a symbolic link: one
/// buffer, named by the first spelling, with the overlay's content.
#[cfg(unix)]
#[test]
fn spellings_and_symbolic_link_are_one_file() {
    let t = Tree::new("links");
    t.write("inc/h.h", b"extern int h_disk;\n");
    std::os::unix::fs::symlink("h.h", t.path("inc/link.h")).unwrap();
    t.write(
        "main.cpp",
        b"#include \"inc/h.h\"\n#include \"inc/../inc/h.h\"\n#include \"inc/link.h\"\nint m = h_new;\n",
    );
    // The entry is added through the link: keyed by the file it names.
    let cl = argv(&["-fsyntax-only", "main.cpp"]);
    let copy = t.copy("links-copy");
    copy.write("inc/h.h", b"extern int h_new;\n");
    let mut overlay = Overlay::new();
    overlay.replace(&t.path("inc/link.h"), Arc::from(&b"extern int h_new;\n"[..])).unwrap();
    let with = run(&t.0, &cl, Some(overlay));
    let (reference, _) = run(&copy.0, &cl, None);
    assert_eq!(with.0, reference);
    assert_eq!(with.0.errors, 0, "{}", with.0.diagnostics);
    let headers: Vec<_> = with.0.files.iter().filter(|(n, _)| n.contains("h.h")).collect();
    assert_eq!(headers.len(), 1, "{:?}", with.0.o_names());
    assert_eq!(headers[0].0, "./inc/h.h");
    assert_eq!(headers[0].1.as_deref(), Some(&b"extern int h_new;\n"[..]));
    assert_eq!(origin_of(&with, "./inc/h.h"), FileOrigin::Overlay);
}

/// [cpp.include] `#pragma once` and an include guard in overlaid headers
/// (the disk's have neither): each included twice, entered once (a
/// second entry would redefine its variable).
#[test]
fn pragma_once_and_include_guard() {
    let t = Tree::new("once");
    t.write("once.h", b"int once_v;\n");
    t.write("guard.h", b"int guard_v;\n");
    t.write(
        "main.cpp",
        b"#include \"once.h\"\n#include \"guard.h\"\n#include \"once.h\"\n#include \"guard.h\"\n",
    );
    let o = same_as_copy(
        &t,
        &["-fsyntax-only", "main.cpp"],
        &[
            ("once.h", b"#pragma once\nint once_v;\n"),
            ("guard.h", b"#ifndef GUARD_H\n#define GUARD_H\nint guard_v;\n#endif\n"),
        ],
    );
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    // The disk's headers would be entered twice.
    let (disk, _) = run(&t.0, &argv(&["-fsyntax-only", "main.cpp"]), None);
    assert_eq!(disk.errors, 2, "{}", disk.diagnostics);
}

/// An overlay that adds an `#include` of an existing file, and one that
/// removes one: the files read change accordingly.
#[test]
fn includes_added_and_removed() {
    let t = Tree::new("added");
    t.write("gone.h", b"int gone_v;\n");
    t.write("extra.h", b"int extra_v;\n");
    t.write("main.cpp", b"#include \"gone.h\"\nint m;\n");
    let o = same_as_copy(
        &t,
        &["-fsyntax-only", "main.cpp"],
        &[("main.cpp", b"#include \"extra.h\"\nint m;\n")],
    );
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    let read = o.0.o_names();
    assert!(read.contains(&"./extra.h") && !read.iter().any(|n| n.contains("gone")), "{read:?}");
    assert_eq!(origin_of(&o, "./extra.h"), FileOrigin::Disk);
}

/// [cpp.cond] `__has_include` of an overlaid header: it exists, as
/// before; a probe never included is consulted, read through the
/// overlay when its buffer is loaded.
#[test]
fn has_include_unchanged() {
    let t = Tree::new("has-include");
    t.write("h.h", b"int h_disk;\n");
    t.write("p.h", b"int p_disk;\n");
    t.write(
        "main.cpp",
        b"#if __has_include(\"h.h\") && __has_include(\"p.h\")\n#include \"h.h\"\nint yes;\n#endif\n",
    );
    let o = same_as_copy(
        &t,
        &["-fsyntax-only", "main.cpp"],
        &[("h.h", b"int h_new;\n"), ("p.h", b"int p_new;\n")],
    );
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    assert!(o.0.dump.contains("yes") && o.0.dump.contains("h_new"), "{}", o.0.dump);
    let probes: Vec<_> = o.0.consulted.iter().map(|c| (c.kind, c.path.as_str())).collect();
    assert_eq!(probes, [(ConsultedKind::Probe, "./p.h")]);
}

/// [cpp.embed] `#embed` and `__has_embed` of an overlaid file: the
/// overlay's bytes.
#[test]
fn embed_reads_the_overlay() {
    let t = Tree::new("embed");
    t.write("data.bin", b"AB");
    t.write(
        "main.cpp",
        b"constexpr unsigned char d[] = {\n#embed \"data.bin\"\n};\n\
          static_assert(sizeof d == 5 && d[0] == 'v' && d[4] == 'z');\n\
          #if __has_embed(\"data.bin\") != __STDC_EMBED_FOUND__\n#error data.bin\n#endif\n",
    );
    let cl = ["-fsyntax-only", "-std=c++26", "-Wno-c23-extensions", "main.cpp"];
    let o = same_as_copy(&t, &cl, &[("data.bin", b"vwxyz")]);
    assert_eq!(o.0.errors, 0, "{}", o.0.diagnostics);
    assert_eq!(
        o.0.consulted,
        [Consulted {
            kind: ConsultedKind::Embed,
            path: "./data.bin".to_owned(),
            content: Some(Content::of(b"vwxyz")),
        }]
    );
}

/// `SourceManager::original_bytes` of an overlaid buffer is the overlay's
/// bytes and its origin `Overlay`; a file not overlaid is `Disk`.
#[test]
fn original_bytes_and_origin() {
    let t = Tree::new("bytes");
    t.write("h.h", b"int h_disk;\n");
    t.write("k.h", b"int k_disk;\n");
    t.write("main.cpp", b"#include \"h.h\"\n#include \"k.h\"\n");
    let o = same_as_copy(&t, &["-fsyntax-only", "main.cpp"], &[("h.h", b"int h_new; \n")]);
    let bytes = |n: &str| o.0.files.iter().find(|f| f.0 == n).and_then(|f| f.1.clone());
    assert_eq!(bytes("./h.h").as_deref(), Some(&b"int h_new; \n"[..]));
    assert_eq!(bytes("./k.h").as_deref(), Some(&b"int k_disk;\n"[..]));
    assert_eq!(origin_of(&o, "./h.h"), FileOrigin::Overlay);
    assert_eq!(origin_of(&o, "./k.h"), FileOrigin::Disk);
    assert_eq!(origin_of(&o, "main.cpp"), FileOrigin::Disk);
}

/// [lex.phases]/1: invalid UTF-8 and a byte-order mark in an overlay are
/// handled as the same bytes on disk (the load report and its
/// diagnostics included).
#[test]
fn invalid_utf8_and_byte_order_mark() {
    let t = Tree::new("utf8");
    t.write("bom.h", b"int bom_disk;\n");
    t.write("bad.h", b"int bad_disk;\n");
    t.write("main.cpp", b"#include \"bom.h\"\n#include \"bad.h\"\n");
    let o = same_as_copy(
        &t,
        &["-fsyntax-only", "-Winvalid-utf8", "main.cpp"],
        &[
            ("bom.h", b"\xEF\xBB\xBFint bom_new;\n"),
            ("bad.h", b"// bad \xFF\xFE bytes\nconst char* s = \"\xC3(\";\nint bad_new;\n"),
        ],
    );
    assert!(o.0.dump.contains("bom_new") && o.0.dump.contains("bad_new"), "{}", o.0.dump);
    let bad = o.0.files.iter().find(|f| f.0 == "./bad.h").unwrap();
    assert!(bad.1.as_deref().unwrap().starts_with(b"// bad \xFF\xFE"));
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

/// A PCH whose input is overlaid: refused (the entry refuses every
/// `-include-pch`); a module file of the command line whose input is
/// overlaid: refused before the job with `drv_front_end_overlay_input`;
/// one found through `-fprebuilt-module-path=`: refused at its import
/// with `pp_module_file_overlaid`. An overlay of other files leaves the
/// module file in use.
#[test]
fn pch_and_module_file_inputs_refused() {
    let t = Tree::new("caches");
    t.write("p.h", b"int p_disk;\n");
    t.write("p.h.pch", b"not read\n");
    t.write("main.cpp", b"int m;\n");
    let mut o = Overlay::new();
    o.replace(&t.path("p.h"), Arc::from(&b"int p_new;\n"[..])).unwrap();
    let (value, errors, id, text) =
        refused(&t.0, &["-fsyntax-only", "-include-pch", "p.h.pch", "main.cpp"], o);
    assert_eq!((value, errors, id), (None, 1, Some(DiagId::drv_front_end_unsupported)), "{text}");

    t.write("m.cppm", b"export module m;\nexport constexpr int twice(int x) { return 2 * x; }\n");
    t.write("a.cpp", b"import m;\nstatic_assert(twice(3) == 6);\n");
    build_module(&t);
    let overlaid = || {
        let mut o = Overlay::new();
        o.replace(&t.path("m.cppm"), Arc::from(&b"export module m;\n"[..])).unwrap();
        o
    };
    let named = ["-fsyntax-only", "-std=c++26", "-fmodule-file=m=m.pcm", "a.cpp"];
    let (value, errors, id, text) = refused(&t.0, &named, overlaid());
    assert_eq!((value, errors, id), (None, 1, Some(DiagId::drv_front_end_overlay_input)), "{text}");
    assert!(
        text.contains("the overlay replaces 'm.cppm', an input of the module file 'm.pcm'"),
        "{text}"
    );
    let nameless = ["-fsyntax-only", "-std=c++26", "-fmodule-file=m.pcm", "a.cpp"];
    let (value, _, id, text) = refused(&t.0, &nameless, overlaid());
    assert_eq!((value, id), (None, Some(DiagId::drv_front_end_overlay_input)), "{text}");

    let prebuilt = ["-fsyntax-only", "-std=c++26", "-fprebuilt-module-path=.", "a.cpp"];
    let (value, _, id, text) = refused(&t.0, &prebuilt, overlaid());
    assert_eq!((value, id), (Some(()), Some(DiagId::pp_module_file_overlaid)), "{text}");
    assert!(text.contains("which the overlay replaces"), "{text}");

    // Another file overlaid: the module file is used.
    let mut other = Overlay::new();
    other.replace(&t.path("p.h"), Arc::from(&b"int p_new;\n"[..])).unwrap();
    let (value, errors, _, text) = refused(&t.0, &named, other);
    assert_eq!((value, errors), (Some(()), 0), "{text}");
}

/// The overlay belongs to its run: a second run in the same process
/// without it reads the disk, and runs with different overlays at the
/// same time see their own.
#[test]
fn runs_do_not_share_an_overlay() {
    let t = Tree::new("runs");
    t.write("h.h", b"int disk_v;\n");
    t.write("main.cpp", b"#include \"h.h\"\n");
    let cl = argv(&["-fsyntax-only", "main.cpp"]);
    let overlay = |text: &'static [u8]| {
        let mut o = Overlay::new();
        o.replace(&t.path("h.h"), Arc::from(text)).unwrap();
        o
    };
    let first = run(&t.0, &cl, Some(overlay(b"int first_v;\n")));
    assert!(first.0.dump.contains("first_v"));
    let second = run(&t.0, &cl, None);
    assert!(second.0.dump.contains("disk_v") && !second.0.dump.contains("first_v"));
    assert_eq!(origin_of(&second, "./h.h"), FileOrigin::Disk);
    let texts: [&'static [u8]; 4] =
        [b"int t0_v;\n", b"int t1_v;\n", b"int t2_v;\n", b"int t3_v;\n"];
    std::thread::scope(|s| {
        let hs: Vec<_> = texts
            .iter()
            .enumerate()
            .map(|(i, &text)| {
                let (cl, o, root) = (&cl, overlay(text), t.0.as_path());
                s.spawn(move || (i, run(root, cl, Some(o)).0.dump))
            })
            .collect();
        for h in hs {
            let (i, dump) = h.join().unwrap();
            for j in 0..texts.len() {
                assert_eq!(dump.contains(&format!("t{j}_v")), i == j, "{i}: {dump}");
            }
        }
    });
}

/// An empty overlay is no overlay: byte-identical results, every buffer
/// from disk.
#[test]
fn empty_overlay_is_none() {
    let t = Tree::new("empty");
    t.write("h.h", b"int h_v;\n#warning in h\n");
    t.write("data.bin", b"xyz");
    t.write(
        "main.cpp",
        b"#include \"h.h\"\nconstexpr unsigned char d[] = {\n#embed \"data.bin\"\n};\nint bad = \"x\";\n",
    );
    let cl = argv(&["-fsyntax-only", "-std=c++26", "-Wno-c23-extensions", "main.cpp"]);
    let none = run(&t.0, &cl, None);
    let empty = run(&t.0, &cl, Some(Overlay::new()));
    assert_eq!(none, empty);
    assert!(empty.1.iter().all(|&o| o != FileOrigin::Overlay));
    assert_eq!(empty.0.errors, 1, "{}", empty.0.diagnostics);
}
