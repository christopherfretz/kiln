//! Stage 4 of the preprocessing record (spec/pp.md "Preprocessing
//! record", WP-2113): its carriage in a PCH snapshot, and what an import
//! adds to it (the definitions a header unit exports, the module images).
//!
//! The record is not one of the preprocessor's snapshot fields: the
//! driver writes it after every root ([`Preprocessor::snap_put_record`]),
//! in the same stream, so its definitions are the macro table's own
//! `Rc`s, and only when the producer recorded; with recording off the
//! snapshot has no byte of it. The consumer reads it before it enters its
//! main file ([`Preprocessor::snap_restore_record`]) and goes on recording
//! into it.

use std::rc::Rc;

use kiln_snap::{Reader, SnapError, Writer};
use kiln_source::{FileId, Loc};

use super::{DefEnd, DefIx, IX_MASK, ImageUnit, IncOutcome, NONE, PpRecord, RecordFlags, ptr};
use crate::Preprocessor;
use crate::macros::MacroDef;

/// What a snapshot restore did with the preprocessing record
/// ([`Preprocessor::snap_restore_record`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordCarry {
    /// The prefix's record was restored: this translation unit's record
    /// goes on from it, as if the prefix had been read here.
    Restored,
    /// Recording is on but the snapshot carries no record (the PCH was
    /// built with recording off): the prefix is not recorded, and the
    /// record is [`RecordFlags::RESTORED`].
    NotCarried,
    /// Recording is off: the snapshot's record, if any, was skipped.
    Off,
}

impl PpRecord {
    /// `exported` (a header unit's definition as its importers see it,
    /// after the preprocessor-state record's round trip) is `recorded`,
    /// the definition the record saw made in the unit: expansions, reads
    /// and `#undef`s of it in the importer name the unit's row.
    pub(crate) fn alias(&mut self, exported: &Rc<MacroDef>, recorded: &Rc<MacroDef>) {
        if Rc::ptr_eq(exported, recorded) || self.knows(exported) {
            return;
        }
        let Some(&d) = self.by_ptr.get(&ptr(recorded)) else { return };
        self.by_ptr.insert(ptr(exported), d);
        self.aliases.push((Rc::clone(exported), d));
    }

    /// The unit of module `module`, imported at `at`, was built from its
    /// declaration image, whose locations are in `files`.
    pub(crate) fn image(&mut self, module: &str, at: Loc, files: &[FileId]) {
        self.set_flag(RecordFlags::IMPORTED);
        self.images.push(ImageUnit { module: module.into(), at, files: files.into() });
    }

    /// The pointer index of a decoded record.
    fn rebuild_index(&mut self) {
        self.by_ptr.clear();
        for (i, d) in self.defs.iter().enumerate() {
            self.by_ptr.insert(ptr(&d.def), DefIx(i as u32));
        }
        for (a, d) in &self.aliases {
            self.by_ptr.insert(ptr(a), *d);
        }
    }

    /// The invariants the record's readers rely on without a check, for a
    /// decoded record: every index in range, every sorted table sorted,
    /// every start within its table and in order. `file` and `sym` say
    /// whether a file id is allocated and a symbol interned; `def` checks a
    /// definition the macro table does not hold.
    fn check(
        &self,
        file: &dyn Fn(FileId) -> bool,
        sym: &dyn Fn(kiln_lex::Symbol) -> bool,
        def: &dyn Fn(&MacroDef) -> bool,
    ) -> Result<(), &'static str> {
        let (nd, ne, nc, ni) =
            (self.defs.len(), self.exps.len(), self.conds.len(), self.incs.len());
        let below = |v: u32, n: usize| (v as usize) < n;
        let opt = |v: u32, n: usize| v == NONE || below(v, n);
        let masked = |v: u32, n: usize| v & IX_MASK == IX_MASK || below(v & IX_MASK, n);
        if ne >= IX_MASK as usize || nc >= IX_MASK as usize || ni >= IX_MASK as usize {
            return Err("a table past its index space");
        }
        let mut params = 0usize;
        for d in &self.defs {
            if !def(&d.def) {
                return Err("a recorded definition is inconsistent");
            }
            let p = d.params as usize;
            let n = d.def.params.len();
            if p != params || p + n > self.params.len() {
                return Err("a definition's parameters out of range");
            }
            params = p + n;
            if matches!(d.ended, DefEnd::Replaced(r) if r.index() >= nd) {
                return Err("a definition replaced by one out of range");
            }
        }
        if self.aliases.iter().any(|(a, d)| d.index() >= nd || !def(a)) {
            return Err("an alias out of range");
        }
        let mut args = 0usize;
        for (e, x) in self.exps.iter().enumerate() {
            let parent = x.parent & IX_MASK;
            if x.def.index() >= nd || (parent != IX_MASK && parent as usize >= e) {
                return Err("an expansion's definition or parent out of range");
            }
            let a = x.args as usize;
            if a < args || a > self.args.len() {
                return Err("an expansion's arguments out of order");
            }
            args = a;
        }
        let mut toks = 0usize;
        for a in &self.args {
            let t = a.tok_start as usize;
            if t < toks || t > self.arg_toks.len() {
                return Err("an argument's tokens out of order");
            }
            toks = t;
        }
        let sorted = |v: &[(Loc, u32)]| v.windows(2).all(|w| w[0].0 < w[1].0);
        if !sorted(&self.entries) || !self.entries.iter().all(|&(_, v)| masked(v, ne)) {
            return Err("the entry rows are not sorted or name an expansion out of range");
        }
        for u in &self.undefs {
            if !sym(u.name) || u.ended.is_some_and(|d| d.index() >= nd) {
                return Err("an #undef row out of range");
            }
        }
        for c in &self.conds {
            if !masked(c.opener, nc) || !masked(c.next, nc) {
                return Err("a conditional's links out of range");
            }
        }
        for i in &self.incs {
            let guard = matches!(i.outcome, IncOutcome::SkippedGuard { def } if def.index() >= nd);
            if guard
                || !opt(i.name_exp, ne)
                || !opt(i.parent, ni)
                || i.file.is_some_and(|f| !file(f))
            {
                return Err("an inclusion row out of range");
            }
        }
        let res_sorted = self.resources.windows(2).all(|w| w[0].0 < w[1].0);
        if !res_sorted || !self.resources.iter().all(|r| below(r.0, ni)) {
            return Err("the resources are not sorted or out of range");
        }
        let bases_sorted = self.inc_bases.windows(2).all(|w| w[0].0 <= w[1].0);
        if !bases_sorted || !self.inc_bases.iter().all(|&(_, i)| below(i, ni)) {
            return Err("the instance index is not sorted or out of range");
        }
        let files = self.main_file.iter().chain(self.predef_tail.iter().map(|t| &t.0));
        let image_files = self.images.iter().flat_map(|m| m.files.iter());
        if !files.chain(image_files).all(|&f| file(f)) || !sym(self.va_opt) {
            return Err("a file id or symbol of the record is not allocated");
        }
        self.s3.check(nd, ne, nc, sym, file)
    }
}

impl Preprocessor<'_, '_> {
    /// Writes the preprocessing record for a snapshot (spec/pp.md
    /// "Preprocessing record", stage 4), after every root of the
    /// snapshot and in the same stream; nothing when recording is off.
    pub fn snap_put_record(&self, w: &mut Writer) {
        if let Some(r) = self.record.as_deref() {
            w.put(r);
        }
    }

    /// Reads what [`Self::snap_put_record`] wrote: the rest of `r`, after
    /// every root has been restored and before the main file is entered
    /// ([`Self::enter_next_main_file`]). With recording on, the prefix's
    /// record replaces the fresh one; when the snapshot has none the
    /// record stays [`RecordFlags::RESTORED`]. With recording off the
    /// bytes are skipped.
    ///
    /// # Errors
    /// A corrupt record ([`SnapError`], never a panic), or a main file
    /// already entered (`ErrorKind::Invalid`).
    pub fn snap_restore_record(&mut self, r: &mut Reader<'_>) -> Result<RecordCarry, SnapError> {
        if !self.frames.is_empty() {
            return Err(r.invalid::<PpRecord>("the main file is entered"));
        }
        let Some(fresh) = self.record.as_deref() else {
            // In pieces: a streaming reader holds what one read asks for.
            while r.remaining() > 0 {
                r.skip(r.remaining().min(1 << 16))?;
            }
            return Ok(RecordCarry::Off);
        };
        if r.remaining() == 0 {
            fresh.set_flag(RecordFlags::RESTORED);
            return Ok(RecordCarry::NotCarried);
        }
        let mut rec: PpRecord = r.get().map_err(|e| e.within("PpRecord"))?;
        let nsym = self.interner.len();
        let nfile = self.sm.file_count();
        let file = |f: FileId| f.index() < nfile;
        let sym = |s: kiln_lex::Symbol| (s.index() as usize) < nsym;
        let def = |m: &MacroDef| self.def_consistent(m);
        if let Err(why) = rec.check(&file, &sym, &def) {
            return Err(r.invalid::<PpRecord>(why));
        }
        rec.rebuild_index();
        self.record = Some(Box::new(rec));
        Ok(RecordCarry::Restored)
    }
}
