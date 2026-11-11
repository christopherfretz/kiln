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
    if let Some(d) = &u.decls
        && let Some(f) = d.image.files().first()
    {
        // The header under the name replay gives it; the record's and
        // the image's file tables find this buffer.
        let _ = sm.add_lazy_file(&d.header, std::path::Path::new(&f.path), f.len, f.hash, None);
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

/// A compile's declaration image (WP-1036): loads its file table into
/// the source manager (sharing the record's buffers) and hands it to
/// the parser with the header's Sema pragma state. Call after
/// [`start`], before the first token. `Err` after reporting why the
/// image cannot be used: never a partial header.
pub(crate) fn sema_load(
    a: &Args,
    pp: &Preprocessor<'_, '_>,
    sm: &SourceManager,
    engine: &DiagEngine<'_>,
) -> Result<Option<kiln_parse::PchLoad>, ()> {
    use kiln_source::LazyFile;
    let Some(u) = &a.pch.image_use else { return Ok(None) };
    let Some(d) = &u.decls else { return Ok(None) };
    let fail = |why: String| {
        Diag::new(DiagId::drv_pch_image_load, Span::INVALID).arg(&u.pch).arg(why).emit(engine);
        Err(())
    };
    let Some((_, st)) = pp.state_hooks().and_then(|h| h.install.as_ref()) else {
        // `start` reported why the record cannot be installed.
        return Err(());
    };
    let mut files = Vec::with_capacity(d.image.files().len());
    for f in d.image.files() {
        let id = if let Some(t) = &f.text {
            match sm.add_virtual(&f.name, t.clone()) {
                Ok(id) => id,
                Err(e) => return fail(e.to_string()),
            }
        } else {
            match sm.add_lazy_file(&f.name, std::path::Path::new(&f.path), f.len, f.hash, None) {
                LazyFile::Added(id) => id,
                LazyFile::Loaded(id) if sm.same_content(id, f.len, f.hash) => id,
                LazyFile::Loaded(_) | LazyFile::Changed => {
                    return fail(format!("'{}' changed since the image was written", f.name));
                }
                LazyFile::Missing => return fail(format!("'{}' is missing", f.name)),
                LazyFile::Exhausted => return fail(kiln_source::LocSpaceExhausted.to_string()),
            }
        };
        files.push(id);
    }
    let p = &st.sema;
    let pragmas = kiln_sema::PragmaState {
        pack: p.pack,
        pack_stack: p.pack_stack.clone(),
        ms_struct: p.ms_struct,
        visibility: p.visibility.clone(),
        fp: p.fp,
        fp_stack: p.fp_stack.clone(),
    };
    Ok(Some(kiln_parse::PchLoad {
        path: u.pch.clone(),
        decls: std::sync::Arc::clone(&d.image),
        files,
        root: root(pp, sm, u.lines),
        pragmas,
    }))
}

/// The predefines line replay includes the header from: the last of the
/// `lines` [`kiln_pp::PCH_LINE`]s.
fn root(pp: &Preprocessor<'_, '_>, sm: &SourceManager, lines: u32) -> kiln_source::Loc {
    let Some(b) = pp.predefines_file() else { return kiln_source::Loc::INVALID };
    let text = sm.text(b);
    let mut at = None;
    let mut from = 0;
    for _ in 0..lines {
        let Some(i) = text[from..].find(kiln_pp::PCH_LINE) else { break };
        at = Some(from + i);
        from += i + kiln_pp::PCH_LINE.len();
    }
    at.and_then(|o| u32::try_from(o).ok()).map_or(kiln_source::Loc::INVALID, |o| sm.loc(b, o))
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
    produced: Option<(FileId, &kiln_sema::PragmaState, Option<&kiln_sema::ModuleImage>)>,
) -> Option<Result<Vec<u8>, String>> {
    if a.pch.producing {
        if a.pch.image == PchImage::Off {
            return None;
        }
        let (main, pragmas, decls) = produced?;
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
            Ok(bytes) => {
                use kiln_pp::bmi::{SECTION_IMAGE, SECTION_PP, Section};
                let mut sections = vec![Section { tag: SECTION_PP, bytes }];
                // WP-1036: the declaration image when it carries the
                // whole header (else the consumer replays it).
                if let Some(d) = decls.filter(|d| d.fallback.is_none()) {
                    sections.push(Section { tag: SECTION_IMAGE, bytes: d.bytes.clone() });
                }
                Ok(kiln_pp::bmi::write_sections(&sections))
            }
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
