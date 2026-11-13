//! Snapshots of the source manager and the interner (spec/source.md
//! "Snapshots", spec/snap.md): a restored root answers every query as the
//! original did, allocates the same ids and locations for content added
//! after the restore, and re-encodes to the same canonical bytes.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use kiln_snap::{from_bytes, schema_hash, to_bytes, to_bytes_canonical};
use kiln_source::hash::content_hash;
use kiln_source::symbol::Interner;
use kiln_source::{
    ExpansionKind, FileId, FileKind, LazyFile, LazyOutcome, Loc, SourceManager, Span,
};

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("kiln_source-snap-{tag}-{}", std::process::id()));
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

const MACRO_BIT: u32 = 1 << 31;

/// Everything a query says about `loc`, as text.
fn loc_answers(sm: &SourceManager, loc: Loc, out: &mut String) {
    let _ = write!(
        out,
        "{loc:?}: {:?} {:?} {:?} {} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {} {} {} {:?} {:?}",
        sm.offset_of(loc),
        sm.line_col(loc),
        sm.presumed_line_col(loc).map(|p| (p.file, p.file_name.to_owned(), p.line, p.col)),
        sm.is_macro_loc(loc),
        sm.expansion_info(loc),
        sm.immediate_spelling_loc(loc),
        sm.spelling_loc(loc),
        sm.expansion_loc(loc),
        sm.expansion_span(loc),
        sm.immediate_macro_caller_loc(loc),
        sm.include_loc(loc),
        sm.file_kind(loc),
        sm.is_in_system_header(loc),
        sm.is_written_in_scratch_space(loc),
        sm.is_in_system_macro(loc),
        sm.entry_bounds(loc),
        sm.file_kind_marks(loc),
    );
    if let Some(p) = sm.presumed_line_col(loc) {
        let _ = write!(out, " {}", sm.presumed_file_name(&p, loc));
    }
    out.push('\n');
}

/// Every query over every location and file of `sm`.
fn answers(sm: &SourceManager) -> String {
    assert_eq!(files(sm).len(), sm.file_count());
    answers_every(sm, 1)
}

/// Every query over every `step`-th location and every file of `sm`.
fn answers_every(sm: &SourceManager, step: usize) -> String {
    let mut out = String::new();
    for raw in (0..4096).step_by(step) {
        loc_answers(sm, Loc::from_raw(raw), &mut out);
    }
    for raw in (MACRO_BIT..MACRO_BIT + 512).step_by(step) {
        loc_answers(sm, Loc::from_raw(raw), &mut out);
    }
    for f in files(sm) {
        let _ = writeln!(
            out,
            "{f:?}: {:?} {:?} {:?} {} {:?} {} {} {:?} {} {:?} {:?} {}",
            sm.file_name(f),
            sm.text(f),
            sm.load_report(f),
            sm.line_count(f),
            (0..=sm.line_count(f) + 1).map(|l| sm.line_text(f, l)).collect::<Vec<_>>(),
            sm.is_virtual(f),
            sm.has_line_directives(f),
            sm.line_directives(f),
            sm.gcc_file_name(f),
            sm.include_guarded_suggestion(f),
            sm.file_span(f),
            sm.same_content(f, sm.text(f).len() as u32, content_hash(sm.text(f).as_bytes())),
        );
        let s = sm.file_span(f);
        let _ = writeln!(out, "  {:?} {:?}", sm.span_text(s), sm.loc(f, 0));
    }
    let _ = writeln!(out, "fallbacks {:?}", sm.lazy_fallbacks());
    out
}

/// Every buffer with a location below 4096 (all of them here), in id
/// order.
fn files(sm: &SourceManager) -> Vec<FileId> {
    let mut ids: Vec<FileId> =
        (1..4096).filter_map(|raw| sm.offset_of(Loc::from_raw(raw))).map(|(f, _)| f).collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// A manager with every kind of content the tables hold.
fn build(dir: &TempDir) -> (SourceManager, Vec<FileId>) {
    let p = |n: &str| dir.0.join(n);
    fs::write(p("main.cpp"), "#include \"a.h\"\n#define M(x) x + 1\nint y = M(2);\n").unwrap();
    fs::write(p("a.h"), "#pragma once\nint a;\r\nint b;\rint c;\n").unwrap();
    fs::write(p("sys.h"), "#pragma GCC system_header\ntypedef int s;\n").unwrap();
    fs::write(p("bad.h"), b"ok\xFFx\n\xE2\x82y\n").unwrap();
    fs::write(p("lazy_same.h"), "int lazy_same;\n").unwrap();
    fs::write(p("lazy_copy.h"), "abcd").unwrap();

    let sm = SourceManager::new();
    let main = sm.load_file(&p("main.cpp")).unwrap();
    let builtin = sm.add_virtual("<built-in>", "#define __kiln__ 1\n".into()).unwrap();
    let a = sm.load_file(&p("a.h")).unwrap();
    let a_inc = sm.add_inclusion(a, sm.loc(main, 0)).unwrap();
    let sys = sm.load_regular_file(&p("sys.h")).unwrap();
    let sys_inc = sm.add_inclusion(sys, sm.loc(a, 13)).unwrap();
    sm.set_file_kind(sys_inc, FileKind::System);
    sm.set_file_kind(sys_inc.with_offset(26), FileKind::ExternCSystem);
    let bad = sm.load_file(&p("bad.h")).unwrap();
    let text = "int lazy_same;\n";
    let LazyFile::Added(same) = sm.add_lazy_file(
        "lazy_same.h",
        &p("lazy_same.h"),
        text.len() as u32,
        content_hash(text.as_bytes()),
        None,
    ) else {
        panic!("lazy_same.h not added")
    };
    let LazyFile::Added(copy) = sm.add_lazy_file(
        "lazy_copy.h",
        &p("lazy_copy.h"),
        4,
        content_hash(b"wxyz"),
        Some(Box::new(|| Some("wxyz".to_owned()))),
    ) else {
        panic!("lazy_copy.h not added")
    };
    assert_eq!(sm.text(copy), "wxyz");
    let scratch = sm.add_virtual("<scratch space>", "y_z\n".into()).unwrap();
    sm.add_line_directive(main, 15, 100, Some("renamed.cpp"));
    sm.add_line_directive(main, 34, 200, None);
    sm.add_line_directive(bad, 5, 7, Some("other.h"));
    sm.mark_include_guarded(a, "A_H".into());
    sm.set_gcc_file_name(sys, &p("sys.h").to_string_lossy(), true);

    // `M(2)` at main:42..46 expands to `2 + 1` (spelled at the
    // definition, main:29..34), with the argument `2` spelled at main:44.
    let invocation = Span::new(sm.loc(main, 42), sm.loc(main, 45));
    let name = Span::new(sm.loc(main, 42), sm.loc(main, 42));
    let m =
        sm.add_expansion(sm.loc(main, 29), 5, invocation, ExpansionKind::Macro { name }).unwrap();
    let arg =
        sm.add_expansion(sm.loc(main, 44), 1, Span::empty(m), ExpansionKind::MacroArg).unwrap();
    let paste = sm
        .add_expansion(
            sm.loc(scratch, 0),
            3,
            Span::new(arg, arg),
            ExpansionKind::Macro { name: Span::new(sm.loc(builtin, 8), sm.loc(builtin, 15)) },
        )
        .unwrap();
    let _ = sm.add_expansion(paste.with_offset(1), 1, Span::empty(paste), ExpansionKind::MacroArg);
    let _ = a_inc;
    (sm, vec![main, builtin, a, sys, bad, same, copy, scratch])
}

/// Content added the same way to two managers.
fn extend(sm: &SourceManager, dir: &TempDir, ids: &[FileId]) -> String {
    let p = |n: &str| dir.0.join(n);
    let mut out = String::new();
    let new = sm.load_file(&p("new.h")).unwrap();
    let again = sm.load_file(&p("a.h")).unwrap();
    let same_again = sm.load_file(&p("lazy_same.h")).unwrap();
    let copy_again = sm.load_file(&p("lazy_copy.h")).unwrap();
    let virt = sm.add_virtual("<command line>", "#define X 1\n".into()).unwrap();
    let inc = sm.add_inclusion(new, sm.loc(ids[0], 0)).unwrap();
    let e = sm
        .add_expansion(
            sm.loc(new, 0),
            3,
            Span::empty(inc),
            ExpansionKind::Macro { name: Span::empty(inc) },
        )
        .unwrap();
    sm.add_line_directive(new, 4, 9, Some("late.h"));
    let _ = writeln!(out, "{new:?} {again:?} {same_again:?} {copy_again:?} {virt:?} {inc:?} {e:?}");
    out
}

#[test]
fn source_manager_round_trip_answers_every_query_alike() {
    let dir = TempDir::new("sm");
    let (sm, ids) = build(&dir);
    let bytes = to_bytes(&sm);
    let canonical = to_bytes_canonical(&sm);
    let restored: SourceManager = from_bytes(&bytes).unwrap();
    assert_eq!(answers(&restored), answers(&sm));
    assert_eq!(restored.lazy_fallbacks(), vec![(ids[6], LazyOutcome::Copy)]);
    // Unread buffers re-encode as they were encoded.
    let fresh: SourceManager = from_bytes(&bytes).unwrap();
    assert_eq!(to_bytes_canonical(&fresh), canonical);
    // Read ones too.
    assert_eq!(to_bytes_canonical(&restored), canonical);
    assert_eq!(to_bytes(&restored).len(), bytes.len());

    fs::write(dir.0.join("new.h"), "int n;\nint m;\n").unwrap();
    let a = extend(&sm, &dir, &ids);
    let b = extend(&fresh, &dir, &ids);
    assert_eq!(a, b);
    assert_eq!(answers(&fresh), answers(&sm));
    // `a.h`, `lazy_same.h` and `lazy_copy.h` (whose file changed) are
    // found as they were before the restore.
    let found: Vec<&str> = a.split(' ').collect();
    assert_eq!(found[1], format!("{:?}", ids[2]));
    assert_eq!(found[2], format!("{:?}", ids[5]));
    assert_ne!(found[3], format!("{:?}", ids[6]));
}

#[test]
fn source_manager_disk_text_is_not_stored() {
    let dir = TempDir::new("text");
    let big = "int x;\n".repeat(10_000);
    fs::write(dir.0.join("big.h"), &big).unwrap();
    let sm = SourceManager::new();
    let f = sm.load_file(&dir.0.join("big.h")).unwrap();
    let bytes = to_bytes(&sm);
    assert!(bytes.len() < 1024, "{} bytes", bytes.len());
    // A changed file is unavailable after the restore, not misread.
    fs::write(dir.0.join("big.h"), big.replace('x', "y")).unwrap();
    let restored: SourceManager = from_bytes(&bytes).unwrap();
    assert_eq!(restored.line_col(restored.loc(f, 8)), None);
    assert_eq!(restored.lazy_fallbacks(), vec![(f, LazyOutcome::Unavailable)]);
    // And a load of it gets a new buffer with the new text.
    let g = restored.load_file(&dir.0.join("big.h")).unwrap();
    assert_ne!(g, f);
    assert!(restored.text(g).starts_with("int y;"));
}

/// The physical line a `#line` directive records is checked when the
/// restored buffer is read (it cannot be before): a directive that does
/// not match its text makes the buffer unavailable, never a wrong
/// presumed line (WP-1387).
#[test]
fn source_manager_line_directive_is_checked_on_read() {
    let dir = TempDir::new("physline");
    fs::write(dir.0.join("l.h"), "a\nb\n#line 50\nc\n").unwrap();
    let sm = SourceManager::new();
    let f = sm.load_file(&dir.0.join("l.h")).unwrap();
    sm.add_line_directive(f, 14, 50, None);
    let c = sm.loc(f, 14);
    assert_eq!(sm.presumed_line_col(c).map(|p| p.line), Some(50));
    let bytes = to_bytes(&sm);
    // The directive: offset 14, phys_line 4, new_line 50.
    let rec: Vec<u8> = [14u32, 4, 50].iter().flat_map(|v| v.to_le_bytes()).collect();
    let at = bytes.windows(12).position(|w| w == rec).expect("the directive's encoding");
    let good: SourceManager = from_bytes(&bytes).unwrap();
    assert_eq!(good.presumed_line_col(c).map(|p| p.line), Some(50));
    assert!(good.lazy_fallbacks().is_empty());
    for wrong in [3u32, 5, 1000] {
        let mut b = bytes.clone();
        b[at + 4..at + 8].copy_from_slice(&wrong.to_le_bytes());
        let bad: SourceManager = from_bytes(&b).unwrap();
        assert!(bad.presumed_line_col(c).is_none(), "phys_line {wrong}");
        assert_eq!(bad.line_col(c), None);
        assert_eq!(bad.lazy_fallbacks(), vec![(f, LazyOutcome::Unavailable)]);
    }
}

#[test]
fn source_manager_decode_is_checked() {
    let dir = TempDir::new("checked");
    let (sm, _) = build(&dir);
    let bytes = to_bytes(&sm);
    for cut in 0..bytes.len() {
        assert!(from_bytes::<SourceManager>(&bytes[..cut]).is_err(), "cut {cut}");
    }
    let mut seen = 0;
    for i in 0..bytes.len() {
        for flip in [0x01u8, 0x80, 0xff] {
            let mut b = bytes.clone();
            b[i] ^= flip;
            if let Ok(sm) = from_bytes::<SourceManager>(&b) {
                // Whatever decodes answers queries without panicking.
                let _ = answers_every(&sm, 7);
                seen += 1;
            }
        }
    }
    assert!(seen > 0);
}

#[test]
fn source_manager_schema_hash_is_stable() {
    assert_eq!(schema_hash::<SourceManager>(), schema_hash::<SourceManager>());
    assert_ne!(schema_hash::<SourceManager>(), schema_hash::<Interner>());
}

#[test]
fn interner_round_trip_keeps_every_symbol() {
    let i = Interner::new();
    let words: Vec<String> = (0..5000).map(|k| format!("w{k}")).collect();
    let syms: Vec<_> = words.iter().map(|w| i.intern(w)).collect();
    let _ = i.intern("");
    let bytes = to_bytes(&i);
    let r: Interner = from_bytes(&bytes).unwrap();
    assert_eq!(r.len(), i.len());
    for (w, &s) in words.iter().zip(&syms) {
        assert_eq!(r.as_str(s), w);
        assert_eq!(r.intern(w), s);
        assert_eq!(r.get(w), Some(s));
    }
    assert_eq!(r.intern("later"), i.intern("later"));
    assert_eq!(to_bytes_canonical(&r), to_bytes_canonical(&i));

    // The same string twice is refused, not silently renumbered.
    let mut dup = kiln_snap::Writer::new();
    dup.length(2);
    dup.str("x");
    dup.str("x");
    let e = from_bytes::<Interner>(dup.as_bytes()).unwrap_err();
    assert!(e.to_string().contains("interned twice"), "{e}");
}

use kiln_source::fx::FxBuildHasher as Fx;

/// Does a hash map's iteration order survive a round trip? The capacity
/// is carried and the entries are inserted in iteration order into a map
/// of that capacity with the same hasher, so it usually does; it is not
/// guaranteed (a slot freed by a removal can take a re-inserted entry
/// earlier than the original probe sequence did, and the group width is
/// per target), so this records the answer rather than asserting it. Code
/// whose output depends on map order uses the canonical mode or sorts.
#[test]
fn fx_map_iteration_order_survives_a_round_trip() {
    let mut report = String::new();
    for n in [10u32, 100, 1000, 10_000, 100_000] {
        let mut m: HashMap<u32, u32, Fx> = HashMap::default();
        for k in 0..n {
            m.insert(k.wrapping_mul(2_654_435_761), k);
        }
        for k in (0..n).step_by(3) {
            m.remove(&k.wrapping_mul(2_654_435_761));
        }
        let r: HashMap<u32, u32, Fx> = from_bytes(&to_bytes(&m)).unwrap();
        let same = m.iter().eq(r.iter());
        let _ = writeln!(
            report,
            "n={n} len={} cap={}/{} order-equal={same}",
            m.len(),
            m.capacity(),
            r.capacity()
        );
        assert_eq!(m, r);
    }
    println!("{report}");
}
