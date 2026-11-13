//! Whole-pipeline snapshot tests of the AST (spec/ast.md "Snapshots",
//! spec/snap.md; WP-1380).
//!
//! A prefix of the `bench/pch` header sets (spec/bench.md "The pch
//! group") is compiled with `-fsyntax-only` through [`crate::run_as`];
//! [`observe`], called by the syntax job at the end of the translation
//! unit, takes the finished [`Ast`], encodes it, decodes it, and checks
//! that the restored AST is the original: the same `-fdump-ast` text (ids,
//! locations, canonical types, implicit declarations, all files), the same
//! canonical encoding, and the same answers from the hash-consing tables
//! for existing and for new content.
//!
//! The `mid` and `<meta>` prefixes run with the unit tests. The `gnu`
//! (`<bits/stdc++.h>`) and `lcx` (libc++) prefixes take minutes in a debug
//! build and are `#[ignore]`d; run them, with the timing and size report,
//! in release:
//!
//! ```text
//! KILN_SNAP_REPORT=1 cargo test --release -p kiln_driver --lib \
//!     snap_tests -- --ignored --include-ignored --nocapture --test-threads=1
//! ```
//!
//! `KILN_SNAP_OUT=<dir>` also writes each snapshot to `<dir>/<case>.snap`;
//! `KILN_SNAP_DECODE=<file> cargo test --release -p kiln_driver --lib
//! snap_tests::decode_file -- --ignored --nocapture` decodes one and
//! prints the peak RSS of the process.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::hash::Hasher;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use kiln_ast::Ast;
use kiln_ast::dump::{DumpOptions, dump};
use kiln_ast::ids::{ArgListId, TypeId};
use kiln_source::fx::FxHasher;
use kiln_source::{FileId, SourceManager};

/// What a test does with the AST at the end of its translation unit.
type Check = Box<dyn FnOnce(&mut Ast, &SourceManager, Option<FileId>)>;

thread_local! {
    static OBSERVER: RefCell<Option<Check>> = const { RefCell::new(None) };
}

/// The syntax job's end-of-unit hook (test builds only): hands the AST to
/// the current test's check, if one is installed on this thread.
pub(crate) fn observe(mut ast: Ast, sm: &SourceManager, main: Option<FileId>) -> Ast {
    if let Some(check) = OBSERVER.with(|o| o.borrow_mut().take()) {
        check(&mut ast, sm, main);
    }
    ast
}

/// The repository root.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A writer that keeps only a length and a hash of what it is given.
struct Digest {
    len: u64,
    hash: FxHasher,
}

impl Write for Digest {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.len += buf.len() as u64;
        self.hash.write(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The length and hash of `-fdump-ast=ids,locs,canonical,implicit,all-files`.
fn dump_digest(ast: &Ast, sm: &SourceManager, main: Option<FileId>) -> (u64, u64) {
    let opts = DumpOptions {
        ids: true,
        locs: true,
        canonical_types: true,
        implicit: true,
        all_files: true,
        sources: Some(sm),
        main_file: main,
        ..DumpOptions::default()
    };
    let mut d = Digest { len: 0, hash: FxHasher::default() };
    dump(ast, &opts, &mut d).expect("dump");
    (d.len, d.hash.finish())
}

/// The peak resident set of this process, in KiB (`VmHWM`).
fn peak_rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("VmHWM:"))
                .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
        })
        .unwrap_or(0)
}

/// The load average, for the timing report.
fn load() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

/// Bytes of each top-level table of `ast`, largest first.
fn table_sizes(ast: &Ast) -> Vec<(&'static str, usize)> {
    use kiln_snap::to_bytes as b;
    let mut v = vec![
        ("types", b(&ast.types).len()),
        ("decls", b(&ast.decls).len()),
        ("exprs", b(&ast.exprs).len()),
        ("stmts", b(&ast.stmts).len()),
        ("attrs", b(&ast.attrs).len()),
        ("annots", b(&ast.annots).len()),
        ("lists", b(&ast.lists).len()),
        ("strings", b(&ast.strings).len()),
        ("names", b(&ast.names).len()),
        ("lookup", b(&ast.lookup).len()),
        ("redecls", b(&ast.redecls).len()),
        ("templates", b(&ast.templates).len()),
        ("consts", b(&ast.consts).len()),
        ("reflections", b(&ast.reflections).len()),
        ("layouts", b(&ast.layouts).len()),
        ("virtuals", b(&ast.virtuals).len()),
        ("vtables", b(&ast.vtables).len()),
        ("modules", b(&ast.modules).len()),
        ("contract_sites", b(&ast.contract_sites).len()),
    ];
    v.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
    v
}

/// The round trip and its checks (see the module docs).
#[allow(clippy::too_many_lines, reason = "one check after another, in order")]
fn check_round_trip(case: &str, orig: &mut Ast, sm: &SourceManager, main: Option<FileId>) {
    let report = std::env::var_os("KILN_SNAP_REPORT").is_some();
    let t = Instant::now();
    let bytes = kiln_snap::to_bytes(&*orig);
    let encode = t.elapsed();
    let t = Instant::now();
    let mut restored: Ast = match kiln_snap::from_bytes(&bytes) {
        Ok(a) => a,
        Err(e) => panic!("{case}: decoding the snapshot failed: {e}"),
    };
    let decode = t.elapsed();
    if report {
        let mut sizes = String::new();
        for (name, n) in table_sizes(orig).into_iter().take(10) {
            let _ = write!(sizes, " {name}={n}");
        }
        eprintln!(
            "snap {case}: {} bytes; encode {:.1} ms, decode {:.1} ms (load {}); largest:{sizes}",
            bytes.len(),
            encode.as_secs_f64() * 1e3,
            decode.as_secs_f64() * 1e3,
            load(),
        );
    }
    if let Some(dir) = std::env::var_os("KILN_SNAP_OUT") {
        let path = PathBuf::from(dir).join(format!("{case}.snap"));
        std::fs::write(&path, &bytes).expect("write the snapshot");
    }
    // (a) The dump.
    assert_eq!(dump_digest(orig, sm, main), dump_digest(&restored, sm, main), "{case}: dump");
    // (b) The canonical encoding.
    let canon = kiln_snap::to_bytes_canonical(&*orig);
    assert!(canon == kiln_snap::to_bytes_canonical(&restored), "{case}: canonical bytes");
    // (c) Interning existing content gives the old ids: every type node's
    // key, every argument list, every constant, every string literal.
    let n_types = orig.types.len() as u32 + 1;
    for i in 1..n_types {
        let t = TypeId::new(i);
        assert_eq!(orig.types.interned_as(t), restored.types.interned_as(t), "{case}: type {i}");
    }
    let n_lists = orig.types.arg_list_count() as u32;
    assert_eq!(n_lists, restored.types.arg_list_count() as u32);
    for i in 1..n_lists {
        let l = ArgListId::new(i);
        let args = orig.types.args(l).to_vec();
        let (a, b) = (orig.types.arg_list(&args), restored.types.arg_list(&args));
        assert_eq!((a, b), (a, a), "{case}: argument list {i}");
    }
    for id in orig.consts.ids() {
        let v = orig.consts.get(id);
        assert_eq!(orig.consts.lookup(&v), Some(id), "{case}: constant {id:?}");
        assert_eq!(restored.consts.lookup(&v), Some(id), "{case}: constant {id:?}");
    }
    for i in 1..=orig.strings.len() as u32 {
        let id = kiln_ast::StringLitId::new(i);
        let (enc, units) = orig.strings.get(id);
        let units = units.to_vec();
        assert_eq!(restored.strings.intern(enc, &units), id, "{case}: string {i}");
        assert_eq!(orig.strings.intern(enc, &units), id, "{case}: string {i}");
    }
    // New content gets the same new ids on both.
    for ast in [&mut *orig, &mut restored] {
        let last = kiln_ast::QualType::unqual(TypeId::new(n_types - 1));
        let once = ast.types.pointer_to(last);
        let twice = ast.types.pointer_to(once);
        let new_list = ast.types.arg_list(&[kiln_ast::types::TemplateArg::ty(twice)]);
        let s = ast.strings.intern(kiln_ast::strings::StringEncoding::Ordinary, b"kiln snapshot");
        let _ = (new_list, s);
    }
    assert!(
        kiln_snap::to_bytes_canonical(&*orig) == kiln_snap::to_bytes_canonical(&restored),
        "{case}: canonical bytes after adding the same content"
    );
}

/// Compiles the prefix `include_lines` (one `#include` per line, given on
/// standard input) with `flags` and runs [`check_round_trip`] on its AST.
/// `false` (and a note) when the toolchain is absent.
fn run_case(case: &'static str, flags: &[&str], text: &str) -> bool {
    let root = root();
    let mut argv: Vec<String> = vec![
        "-fsyntax-only".into(),
        "-resource-dir".into(),
        root.join("resource").to_string_lossy().into_owned(),
    ];
    argv.extend(flags.iter().map(|&f| f.to_owned()));
    argv.extend(["-x".into(), "c++".into(), "-".into()]);
    let size = crate::templ_opts::stack_size_of(&argv);
    let text = text.to_owned();
    let worker = std::thread::Builder::new().name(format!("snap-{case}")).stack_size(size);
    let handle = worker
        .spawn(move || {
            let ran = std::rc::Rc::new(std::cell::Cell::new(false));
            let flag = std::rc::Rc::clone(&ran);
            OBSERVER.with(|o| {
                *o.borrow_mut() = Some(Box::new(move |ast, sm, main| {
                    flag.set(true);
                    check_round_trip(case, ast, sm, main);
                }));
            });
            let stderr = crate::SharedWriter::new(Vec::new());
            let io = crate::Io {
                stdout: Box::new(std::io::sink()),
                stderr: stderr.clone(),
                stdin: Box::new(std::io::Cursor::new(text.into_bytes())),
            };
            let code = crate::run_as("kiln++", &argv, io);
            assert_eq!(code, 0, "{case}: the prefix does not compile");
            assert!(ran.get(), "{case}: the syntax job did not reach the end of the unit");
        })
        .expect("spawn");
    handle.join().unwrap_or_else(|p| std::panic::resume_unwind(p));
    true
}

/// The header set `bench/pch/<case>.h` as `#include` lines of its own.
fn header_set(case: &str) -> String {
    let path = root().join(format!("bench/pch/{case}.h"));
    format!("#include \"{}\"\n", path.display())
}

/// `mid`: `<algorithm>`, `<map>`, `<string>`, `<vector>` (libstdc++).
#[test]
fn snapshot_round_trip_mid() {
    run_case("mid", &["-std=c++26"], &header_set("mid"));
}

/// GCC 16.1 libstdc++'s `<meta>` with nine common headers, the way the
/// harness's `--meta-header` mode compiles it (spec/harness.md); skipped
/// when the toolchain is absent.
#[test]
fn snapshot_round_trip_meta() {
    // `.toolchain/` sits in some ancestor of the checkout (worktrees are
    // nested in it).
    let image = ".toolchain/gcc-16.1.0-image/usr/local";
    let Some(gcc) = root().ancestors().map(|a| a.join(image)).find(|g| g.join("include").is_dir())
    else {
        eprintln!("unsupported: snapshot_round_trip_meta: no {image} above the checkout");
        return;
    };
    let text = "#include <meta>\n#include <vector>\n#include <string>\n#include <map>\n\
                #include <optional>\n#include <variant>\n#include <tuple>\n\
                #include <functional>\n#include <algorithm>\n#include <memory>\n";
    let toolchain = format!("--gcc-toolchain={}", gcc.display());
    let flags: &'static [&'static str] = Box::leak(vec!["-std=c++26"].into_boxed_slice());
    let mut all = flags.to_vec();
    all.push(&toolchain);
    run_case("meta", &all, text);
}

/// `gnu`: `<bits/stdc++.h>` (libstdc++); slow in a debug build.
#[test]
#[ignore = "minutes in a debug build; run in release (see the module docs)"]
fn snapshot_round_trip_gnu() {
    run_case("gnu", &["-std=c++26"], &header_set("gnu"));
}

/// `lcx`: 106 libc++ headers; slow in a debug build.
#[test]
#[ignore = "minutes in a debug build; run in release (see the module docs)"]
fn snapshot_round_trip_lcx() {
    run_case("lcx", &["-std=c++26", "-stdlib=libc++"], &header_set("lcx"));
}

/// Decodes the snapshot file `$KILN_SNAP_DECODE` and prints the peak RSS
/// of the process (a measurement, not a check).
#[test]
#[ignore = "a measurement: needs KILN_SNAP_DECODE"]
fn decode_file() {
    let Some(path) = std::env::var_os("KILN_SNAP_DECODE") else { return };
    let bytes = std::fs::read(&path).expect("read the snapshot");
    let before = peak_rss_kib();
    let t = Instant::now();
    let ast: Ast = kiln_snap::from_bytes(&bytes).expect("decode");
    let ms = t.elapsed().as_secs_f64() * 1e3;
    eprintln!(
        "decode {}: {ms:.1} ms, peak RSS {} KiB (before decoding {before} KiB, file {} bytes), {} decls",
        PathBuf::from(path).display(),
        peak_rss_kib(),
        bytes.len(),
        ast.decls.len()
    );
}

/// Truncations and bit flips of the `mid` snapshot never panic: decoding
/// gives an error or a value (spec/snap.md "Errors").
#[test]
fn snapshot_corruption_mid() {
    let root = root();
    let argv: Vec<String> = vec![
        "-fsyntax-only".into(),
        "-resource-dir".into(),
        root.join("resource").to_string_lossy().into_owned(),
        "-std=c++26".into(),
        "-x".into(),
        "c++".into(),
        "-".into(),
    ];
    let size = crate::templ_opts::stack_size_of(&argv);
    let text = header_set("mid");
    let worker = std::thread::Builder::new().name("snap-corrupt".into()).stack_size(size);
    worker
        .spawn(move || {
            OBSERVER.with(|o| {
                *o.borrow_mut() = Some(Box::new(|ast: &mut Ast, _: &SourceManager, _| {
                    corrupt(&kiln_snap::to_bytes(&*ast));
                }));
            });
            let io = crate::Io {
                stdout: Box::new(std::io::sink()),
                stderr: crate::SharedWriter::new(Vec::new()),
                stdin: Box::new(std::io::Cursor::new(text.into_bytes())),
            };
            assert_eq!(crate::run_as("kiln++", &argv, io), 0);
        })
        .expect("spawn")
        .join()
        .unwrap_or_else(|p| std::panic::resume_unwind(p));
}

/// Decodes truncations of `bytes` at a spread of offsets (every byte of
/// the first 4 KiB, then about 400 more), and copies with one bit flipped
/// at about 400 offsets: each is an error or an AST, never a panic.
fn corrupt(bytes: &[u8]) {
    let n = bytes.len();
    let step = (n / 400).max(1);
    let offsets: Vec<usize> = (0..n.min(4096)).chain((4096..n).step_by(step)).collect();
    for &cut in &offsets {
        assert!(kiln_snap::from_bytes::<Ast>(&bytes[..cut]).is_err(), "truncation at {cut}");
    }
    let mut copy = bytes.to_vec();
    for (k, &at) in offsets.iter().step_by(11).enumerate() {
        let bit = 1u8 << (k % 8);
        copy[at] ^= bit;
        let _ = kiln_snap::from_bytes::<Ast>(&copy);
        copy[at] ^= bit;
    }
}
