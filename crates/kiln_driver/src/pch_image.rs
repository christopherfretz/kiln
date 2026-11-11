//! The PCH's preprocessor-state record (WP-1280, spec/driver.md
//! "Precompiled headers", `-fkiln-pch-image`; the record is
//! spec/modules.md §11.13).
//!
//! The producer's job takes the state at the end of the header against
//! the state before its first `-include` and writes it after the
//! `KILNPCH` text. A consumer job either replays the header and compares
//! the state replay reached with the record (`check`), or, for `-E` and
//! `-M`, installs the record in place of the header (`load`).

use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_lex::Interner;
use kiln_pp::Preprocessor;
use kiln_pp::state::{PpRecord, SemaPragmas, StateHooks};
use kiln_source::{FileId, SourceManager, Span};

use crate::args::{Args, PchImage};

/// Installs the job's state hooks (before its first token is read).
pub(crate) fn start(
    a: &Args,
    pp: &mut Preprocessor<'_, '_>,
    sm: &SourceManager,
    interner: &Interner,
    engine: &DiagEngine<'_>,
) {
    if a.pch.producing {
        if a.pch.image != PchImage::Off {
            pp.set_state_hooks(StateHooks { baseline: true, ..StateHooks::default() });
        }
        return;
    }
    let Some(u) = &a.pch.image_use else { return };
    if !u.install {
        pp.set_state_hooks(StateHooks {
            baseline: true,
            checkpoint: Some(u.checkpoint),
            ..StateHooks::default()
        });
        return;
    }
    match PpRecord::decode(&u.bytes).and_then(|r| r.materialize(sm, interner)) {
        Ok(st) => {
            pp.set_state_hooks(StateHooks { install: Some((0, st)), ..StateHooks::default() });
        }
        Err(why) => {
            Diag::new(DiagId::drv_pch_image_load, Span::INVALID).arg(&u.pch).arg(why).emit(engine);
        }
    }
}

/// After the job's last token: the producer's record (`Some(Err)`: why
/// it could not be written), or `None` after a consumer's check (which
/// reports a mismatch).
pub(crate) fn finish(
    a: &Args,
    pp: &mut Preprocessor<'_, '_>,
    sm: &SourceManager,
    interner: &Interner,
    engine: &DiagEngine<'_>,
    produced: Option<(FileId, &kiln_sema::PragmaState)>,
) -> Option<Result<Vec<u8>, String>> {
    if a.pch.producing {
        if a.pch.image == PchImage::Off {
            return None;
        }
        let (main, pragmas) = produced?;
        let hooks = pp.take_state_hooks()?;
        let Some(base) = hooks.taken_baseline else {
            return Some(Err("the predefines were not read to their end".to_owned()));
        };
        let mut st = pp.state_snapshot(&base);
        st.sema = SemaPragmas {
            pack: pragmas.pack,
            pack_stack: pragmas.pack_stack.clone(),
            ms_struct: pragmas.ms_struct,
            visibility: pragmas.visibility.clone(),
            fp: pragmas.fp,
            fp_stack: pragmas.fp_stack.clone(),
        };
        return Some(match st.encode(sm, interner, main) {
            Ok(bytes) => Ok(kiln_pp::bmi::write_sections(&[kiln_pp::bmi::Section {
                tag: kiln_pp::bmi::SECTION_PP,
                bytes,
            }])),
            Err(why) => Err(why.to_owned()),
        });
    }
    let u = a.pch.image_use.as_ref()?;
    if u.install {
        // The record's token spellings are read from its files when
        // printed: a file whose text changed (same size, so the
        // record's file table could not tell at load) left blanks.
        if sm.lazy_fallbacks().iter().any(|(_, o)| *o != kiln_source::LazyOutcome::Disk) {
            Diag::new(DiagId::drv_pch_image_load, Span::INVALID)
                .arg(&u.pch)
                .arg("a file changed since the record was written")
                .emit(engine);
        }
        return None;
    }
    let hooks = pp.take_state_hooks()?;
    let mismatch = |what: String| {
        Diag::new(DiagId::drv_pch_image_mismatch, Span::INVALID).arg(&u.pch).arg(what).emit(engine);
    };
    let Some(replayed) = hooks.taken else {
        // A fatal error stopped the job before the end of the header.
        if engine.error_count() == 0 {
            mismatch("replay did not reach the end of the header".to_owned());
        }
        return None;
    };
    let rec = match PpRecord::decode(&u.bytes).and_then(|r| r.materialize(sm, interner)) {
        Ok(st) => st,
        Err(why) => {
            Diag::new(DiagId::drv_pch_image_load, Span::INVALID).arg(&u.pch).arg(why).emit(engine);
            return None;
        }
    };
    if let Some(what) =
        first_difference(&rec.describe(sm, interner), &replayed.describe(sm, interner))
    {
        mismatch(what);
    }
    None
}

/// The first line the record (`rec`) and replay (`replayed`) disagree
/// on, both sorted.
fn first_difference(rec: &[String], replayed: &[String]) -> Option<String> {
    let (mut i, mut j) = (0, 0);
    while i < rec.len() || j < replayed.len() {
        match (rec.get(i), replayed.get(j)) {
            (Some(r), Some(p)) if r == p => (i, j) = (i + 1, j + 1),
            (Some(r), Some(p)) if r < p => return Some(format!("only the record has '{r}'")),
            (Some(r), None) => return Some(format!("only the record has '{r}'")),
            (_, Some(p)) => return Some(format!("only replay has '{p}'")),
            (None, None) => break,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::first_difference;

    #[test]
    fn differences() {
        let v = |s: &[&str]| s.iter().map(|&x| x.to_owned()).collect::<Vec<_>>();
        assert_eq!(first_difference(&v(&["a", "b"]), &v(&["a", "b"])), None);
        assert_eq!(
            first_difference(&v(&["a", "c"]), &v(&["a", "b", "c"])).as_deref(),
            Some("only replay has 'b'")
        );
        assert_eq!(
            first_difference(&v(&["a", "b"]), &v(&["a"])).as_deref(),
            Some("only the record has 'b'")
        );
    }
}
