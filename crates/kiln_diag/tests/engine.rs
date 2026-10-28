//! Engine behavior: counting, the error limit, fatal suppression,
//! deduplication, summaries and thread safety (`spec/diag.md` section 7).

mod common;

use common::{assert_text, source, sp, text};
use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions, Level, MemorySink, Severity};
use kiln_source::{FileId, Span};

const SRC: &str = "#endif\n#endif\n#endif\n#endif\n#warning w\n";

fn endif(e: &DiagEngine<'_>, f: FileId, line: u32) {
    let sm = e.sm();
    Diag::new(DiagId::pp_endif_without_if, sp(sm, f, line, 2, 5)).emit(e);
}

fn warning(e: &DiagEngine<'_>, f: FileId) {
    let sm = e.sm();
    Diag::new(DiagId::pp_warning_directive, sp(sm, f, 5, 2, 7)).arg("w").emit(e);
}

// spec: diag.md §7.3 (error limit)
#[test]
fn error_limit() {
    let (sm, f) = source("e.c", SRC);
    let args = ["-fno-caret-diagnostics", "-ferror-limit=2"];
    let got = text(&sm, &args, |e| {
        for line in 1..=4 {
            endif(e, f, line);
        }
        warning(e, f);
    });
    assert_text(
        &got,
        "e.c:1:2: error: #endif without #if\n\
         e.c:2:2: error: #endif without #if\n\
         fatal error: too many errors emitted, stopping now [-ferror-limit=]\n\
         3 errors generated.\n",
    );

    // Warnings do not count toward the limit, and the limit is reached
    // only by an error *beyond* it.
    let got = text(&sm, &["-fno-caret-diagnostics", "-ferror-limit=1"], |e| {
        warning(e, f);
        endif(e, f, 1);
        assert!(!e.has_fatal());
    });
    assert_text(
        &got,
        "e.c:5:2: warning: w [-W#warnings]\n\
         e.c:1:2: error: #endif without #if\n\
         1 warning and 1 error generated.\n",
    );

    // -ferror-limit=0 disables the limit; the default is 19 (clang's
    // driver default, not cc1's 0).
    let (sm, f) = source("e.c", &"#endif\n".repeat(30));
    let count = |args: &[&str]| {
        let opts = DiagOptions::from_args(args);
        let mem = MemorySink::new();
        let e = DiagEngine::new(&sm, &opts, mem.clone());
        for line in 1..=30 {
            Diag::new(DiagId::pp_endif_without_if, sp(&sm, f, line, 2, 5)).emit(&e);
        }
        let d = mem.take();
        (d.len(), e.error_count(), d.last().unwrap().id)
    };
    assert_eq!(count(&["-ferror-limit=0"]), (30, 30, Some(DiagId::pp_endif_without_if)));
    assert_eq!(count(&[]), (20, 20, Some(DiagId::diag_too_many_errors)));
}

// spec: diag.md §7.2 (fatal errors stop everything after them)
#[test]
fn fatal_suppresses_rest() {
    let (sm, f) = source("e.c", SRC);
    let got = text(&sm, &["-fno-caret-diagnostics", "-Werror", "-Wfatal-errors"], |e| {
        warning(e, f);
        assert!(e.has_fatal());
        endif(e, f, 1);
    });
    // The option suffix of a fatal error never says -Werror.
    assert_text(&got, "e.c:5:2: fatal error: w [-W#warnings]\n1 error generated.\n");
}

// spec: diag.md §7.4 (deduplication)
#[test]
fn dedup() {
    let (sm, f) = source("e.c", SRC);
    let opts = DiagOptions::default();
    let mem = MemorySink::new();
    let e = DiagEngine::new(&sm, &opts, mem.clone());
    let s = sp(&sm, f, 1, 2, 5);
    Diag::new(DiagId::pp_endif_without_if, s).emit(&e);
    Diag::new(DiagId::pp_endif_without_if, s).emit(&e); // dropped
    Diag::new(DiagId::pp_endif_without_if, s).allow_duplicate().emit(&e);
    // Same location, different span: kept.
    Diag::new(DiagId::pp_endif_without_if, Span::new(s.lo, s.lo)).emit(&e);
    // Same span, different id: kept.
    Diag::new(DiagId::pp_warning_directive, s).arg("x").emit(&e);
    // Invalid spans never deduplicate.
    Diag::new(DiagId::pp_warning_directive, Span::INVALID).arg("x").emit(&e);
    Diag::new(DiagId::pp_warning_directive, Span::INVALID).arg("x").emit(&e);
    assert_eq!(mem.take().len(), 6);
    assert_eq!((e.error_count(), e.warning_count()), (3, 3));
    // An ignored diagnostic is not recorded as seen.
    let opts = DiagOptions::from_args(["-Wno-#warnings"]);
    let mem = MemorySink::new();
    let e = DiagEngine::new(&sm, &opts, mem.clone());
    Diag::new(DiagId::pp_warning_directive, s).arg("x").emit(&e);
    assert_eq!(mem.take().len(), 0);
    assert_eq!(e.warning_count(), 0);
}

// spec: diag.md §7.5 (summary line)
#[test]
fn summary() {
    let (sm, f) = source("e.c", SRC);
    let run = |n_warn: usize, n_err: u32| {
        let opts = DiagOptions::default();
        let e = DiagEngine::new(&sm, &opts, MemorySink::new());
        for _ in 0..n_warn {
            Diag::new(DiagId::pp_warning_directive, Span::INVALID).arg("w").emit(&e);
        }
        for line in 1..=n_err {
            endif(&e, f, line);
        }
        e.summary()
    };
    assert_eq!(run(0, 0), None);
    assert_eq!(run(1, 0).as_deref(), Some("1 warning generated."));
    assert_eq!(run(2, 0).as_deref(), Some("2 warnings generated."));
    assert_eq!(run(0, 1).as_deref(), Some("1 error generated."));
    assert_eq!(run(1, 1).as_deref(), Some("1 warning and 1 error generated."));
    assert_eq!(run(3, 2).as_deref(), Some("3 warnings and 2 errors generated."));
}

// spec: diag.md §7.1 (policy queries, record contents)
#[test]
fn queries_and_records() {
    let (sm, f) = source("e.c", SRC);
    let opts = DiagOptions::from_args(["-Wall", "-Werror=unknown-pragmas"]);
    let mem = MemorySink::new();
    let e = DiagEngine::new(&sm, &opts, mem.clone());
    assert_eq!(e.severity(DiagId::pp_unknown_pragma), Severity::Error);
    assert_eq!(e.severity(DiagId::pp_macro_unused), Severity::Ignored);
    assert!(!e.is_enabled(DiagId::pp_macro_unused));
    assert!(e.is_enabled(DiagId::pp_endif_without_if));
    let s = sp(&sm, f, 1, 2, 5);
    Diag::new(DiagId::pp_macro_redefined, s)
        .arg("A")
        .range(s)
        .span(s, "here")
        .note(DiagId::note_previous_definition, s)
        .note("free text", Span::INVALID)
        .emit(&e);
    Diag::new(DiagId::pp_unknown_pragma, sp(&sm, f, 2, 2, 5)).emit(&e);
    let d = mem.take();
    assert_eq!(d.len(), 2);
    assert_eq!(d[0].level, Level::Warning);
    assert_eq!(d[0].message, "'A' macro redefined");
    assert_eq!(d[0].option.as_deref(), Some("-Wmacro-redefined"));
    assert_eq!(d[0].group(), Some("macro-redefined"));
    assert_eq!(d[0].labels.len(), 2);
    assert_eq!(d[0].notes.len(), 2);
    assert_eq!(d[0].notes[0].level, Level::Note);
    assert_eq!(d[0].notes[0].message, "previous definition is here");
    assert_eq!(d[0].notes[1].id, None);
    assert_eq!(d[0].notes[1].message, "free text");
    assert_eq!(d[1].level, Level::Error);
    assert_eq!(d[1].option.as_deref(), Some("-Werror,-Wunknown-pragmas"));
}

// spec: diag.md §7.6 (thread safety)
#[test]
fn shared_across_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DiagEngine<'static>>();
    assert_send_sync::<MemorySink>();

    let (sm, f) = source("e.c", &"#endif\n".repeat(64));
    let opts = DiagOptions::from_args(["-ferror-limit=0"]);
    let mem = MemorySink::new();
    let e = DiagEngine::new(&sm, &opts, mem.clone());
    std::thread::scope(|s| {
        for t in 0..4u32 {
            let e = &e;
            let sm = &sm;
            s.spawn(move || {
                // Every thread emits every line: 64 distinct spans in total.
                for i in 0..64 {
                    let line = (i + t * 16) % 64 + 1;
                    Diag::new(DiagId::pp_endif_without_if, sp(sm, f, line, 2, 5)).emit(e);
                }
            });
        }
    });
    assert_eq!(mem.take().len(), 64);
    assert_eq!(e.error_count(), 64);

    // With a limit, exactly one too-many-errors fatal is emitted no matter
    // how the threads interleave.
    let opts = DiagOptions::from_args(["-ferror-limit=5"]);
    let mem = MemorySink::new();
    let e = DiagEngine::new(&sm, &opts, mem.clone());
    std::thread::scope(|s| {
        for t in 0..4u32 {
            let (e, sm) = (&e, &sm);
            s.spawn(move || {
                for i in 0..16 {
                    let line = t * 16 + i + 1;
                    Diag::new(DiagId::pp_endif_without_if, sp(sm, f, line, 2, 5)).emit(e);
                }
            });
        }
    });
    let d = mem.take();
    assert_eq!(d.len(), 6);
    assert_eq!(d.iter().filter(|d| d.level == Level::Fatal).count(), 1);
    assert_eq!(d[5].id, Some(DiagId::diag_too_many_errors));
    assert_eq!(e.error_count(), 6);
}
