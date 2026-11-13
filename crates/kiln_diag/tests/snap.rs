//! Snapshots of the diagnostics engine (spec/diag.md "Snapshots",
//! spec/snap.md "Configured roots"): the `#pragma clang diagnostic` state
//! ([cpp.pragma]) and the deduplication set restored into a fresh engine
//! answer every severity query as the original's, and a corrupt snapshot
//! is an error, never a panic.

use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions, MemorySink, Severity};
use kiln_snap::{Reader, Writer, from_bytes, to_bytes};
use kiln_source::{Loc, SourceManager, Span};

fn put(e: &DiagEngine<'_>, canonical: bool) -> Vec<u8> {
    let mut w = if canonical { Writer::canonical() } else { Writer::new() };
    e.snap_put(&mut w).unwrap();
    w.into_bytes()
}

/// An engine with pragmas in two files (one pushed state left open) and
/// one diagnostic printed.
fn build(sm: &SourceManager) -> (DiagEngine<'_>, Vec<Loc>) {
    let a = sm.add_virtual("a.cpp", "x".repeat(200)).unwrap();
    let b = sm.add_virtual("b.h", "y".repeat(100)).unwrap();
    let inc = sm.add_inclusion(b, sm.loc(a, 50)).unwrap();
    let e = DiagEngine::new(sm, &DiagOptions::default(), MemorySink::new());
    e.pragma_push(sm.loc(a, 10));
    assert!(e.pragma_set(sm.loc(a, 10), false, "#warnings", Severity::Ignored));
    assert!(e.pragma_set(inc.with_offset(5), false, "unused-macros", Severity::Error));
    e.pragma_file_exit(sm.loc(a, 51));
    assert!(e.pragma_pop(sm.loc(a, 120)));
    e.pragma_push(sm.loc(a, 130));
    assert!(e.pragma_set(sm.loc(a, 140), false, "everything", Severity::Warning));
    Diag::new(DiagId::pp_macro_redefined, Span::empty(sm.loc(a, 3))).arg("M").emit(&e);
    let locs = (0..200).map(|o| sm.loc(a, o)).chain((0..100).map(|o| inc.with_offset(o)));
    (e, locs.chain([Loc::INVALID]).collect())
}

fn answers(e: &DiagEngine<'_>, locs: &[Loc]) -> Vec<(Severity, bool)> {
    let ids = [DiagId::pp_warning_directive, DiagId::pp_macro_unused, DiagId::pp_macro_redefined];
    let mut out = Vec::new();
    for &l in locs {
        for id in ids {
            out.push((e.severity_at(id, l), e.is_enabled(id)));
        }
    }
    out
}

/// [cpp.pragma]: severities at every location, the push stack and the
/// deduplication set survive a restore into a fresh engine.
#[test]
fn engine_round_trip_answers_alike() {
    let sm = SourceManager::new();
    let (e, locs) = build(&sm);
    let bytes = put(&e, false);
    let sink = MemorySink::new();
    let mut r = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let mut rd = Reader::new(&bytes);
    r.snap_restore(&mut rd).unwrap();
    rd.finish().unwrap();
    assert_eq!(answers(&r, &locs), answers(&e, &locs));
    assert_eq!(put(&r, true), put(&e, true));
    // The pushed state pops in both; the printed diagnostic is a duplicate.
    let at = locs[180];
    assert!(r.pragma_pop(at) && e.pragma_pop(at));
    assert!(!r.pragma_pop(at));
    assert_eq!(answers(&r, &locs), answers(&e, &locs));
    Diag::new(DiagId::pp_macro_redefined, Span::empty(locs[3])).arg("M").emit(&r);
    assert!(sink.take().is_empty());
    // Counts are not carried (spec/diag.md "Snapshots").
    assert_eq!((r.warning_count(), e.warning_count()), (0, 1));
    // What the prefix reported is carried for `-verify` (spec/driver.md
    // "PCH snapshot: behaviour"), and adds up through a second snapshot.
    assert_eq!((r.prefix_reported(), e.prefix_reported()), (1, 0));
    Diag::new(DiagId::pp_macro_redefined, Span::empty(locs[4])).arg("M").emit(&r);
    let mut again = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    again.snap_restore(&mut Reader::new(&put(&r, false))).unwrap();
    assert_eq!(again.prefix_reported(), 2);
}

/// A restore needs a fresh engine; a snapshot never lands in a used one.
#[test]
fn engine_restore_needs_a_fresh_engine() {
    let sm = SourceManager::new();
    let (e, _) = build(&sm);
    let bytes = put(&e, false);
    let (mut used, _) = build(&sm);
    let err = used.snap_restore(&mut Reader::new(&bytes)).unwrap_err();
    assert!(err.to_string().contains("used diagnostics engine"), "{err}");
}

/// Truncation and bit flips at every third byte (the `enabled` flags are
/// most of the bytes): an error or an engine whose queries work.
#[test]
fn engine_decode_is_checked() {
    let sm = SourceManager::new();
    let (e, locs) = build(&sm);
    let bytes = put(&e, false);
    let restore = |b: &[u8]| {
        let mut r = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
        let mut rd = Reader::new(b);
        let ok = r.snap_restore(&mut rd).and_then(|()| rd.finish()).is_ok();
        if ok {
            let _ = answers(&r, &locs);
            r.pragma_pop(locs[199]);
        }
        ok
    };
    for cut in (0..bytes.len()).step_by(3) {
        assert!(!restore(&bytes[..cut]), "cut {cut}");
    }
    for i in (0..bytes.len()).step_by(3) {
        for flip in [0x01u8, 0x80, 0xff] {
            let mut b = bytes.clone();
            b[i] ^= flip;
            restore(&b);
        }
    }
}

/// A diagnostic id is its index in the table, checked on read.
#[test]
fn diag_id_round_trip() {
    for &id in &DiagId::ALL[..50] {
        assert_eq!(from_bytes::<DiagId>(&to_bytes(&id)).unwrap(), id);
    }
    let past = u16::try_from(DiagId::ALL.len()).unwrap();
    assert!(from_bytes::<DiagId>(&past.to_le_bytes()).is_err());
}
