//! Reclaiming the code of a failed trial fold's thunk (WP-798;
//! spec/oven.md "Trial folds").
//!
//! A thunk is compiled once per expression and cached ([`Oven::thunks`]),
//! its code, sites, step locations, origins and traps appended to the
//! Oven's flat tables, none of which is ever freed. A trial fold that
//! fails is a one-shot: sema asks once whether an operand folds (the
//! `-Winteger-overflow` scan asks it of every signed `+` operand, WP-458),
//! so a body of N non-constant operands kept N thunks alive for the rest
//! of the translation unit. When the failed thunk is the last thing the
//! tables hold (nothing was compiled or appended after it, at compile or
//! run time), they are cut back to where they stood before it and the
//! cache forgets it: the next fold of the same expression compiles it
//! again.

use kiln_ast::{DeclId, ExprId};

use crate::Oven;

/// The lengths of the tables a compilation appends to.
#[derive(Clone, Copy, PartialEq, Eq, Debug, kiln_snap::Snap)]
pub(crate) struct Tables {
    code: usize,
    padded: usize,
    sites: usize,
    step_locs: usize,
    origins: usize,
    traps: usize,
    funcs: usize,
    meta_sites: usize,
}

/// The thunk [`Oven::compile_thunk`] compiled last: its index, cache key
/// and the tables just after it.
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
pub(crate) struct LastThunk {
    idx: u32,
    key: (ExprId, Option<DeclId>, bool),
    after: Tables,
}

impl Oven {
    /// The current lengths of the code tables.
    pub(crate) fn tables(&self) -> Tables {
        Tables {
            code: self.code.len(),
            padded: self.padded,
            sites: self.sites.len(),
            step_locs: self.step_locs.len(),
            origins: self.origins.len(),
            traps: self.traps.len(),
            funcs: self.funcs.len(),
            meta_sites: self.meta.sites.len(),
        }
    }

    /// Notes that thunk `idx` (cached under `key`, outside a default
    /// argument use) was just compiled.
    pub(crate) fn note_thunk(&mut self, idx: u32, key: (ExprId, Option<DeclId>, bool)) {
        self.last_thunk = Some(LastThunk { idx, key, after: self.tables() });
    }

    /// The start of an outermost trial fold: the tables, for
    /// [`Oven::reclaim_failed`]. `None` inside an evaluation (whose code
    /// is live).
    pub(crate) fn trial_mark(&mut self) -> Option<Tables> {
        if self.ctx_depth != 0 {
            return None;
        }
        self.last_thunk = None;
        Some(self.tables())
    }

    /// After a failed outermost trial fold that began at `before`: when
    /// the fold compiled a fresh thunk, compiled nothing else (no callee
    /// stub, no other unit) and the run appended nothing (no unwind
    /// block, no reflection trap), the tables are cut back to `before`
    /// and the thunk is forgotten. Otherwise nothing changes.
    pub(crate) fn reclaim_failed(&mut self, before: Option<Tables>) {
        let Some(before) = before else { return };
        let Some(t) = self.last_thunk.take() else { return };
        if t.idx as usize != before.funcs
            || t.after.funcs != before.funcs + 1
            || t.after != self.tables()
            || self.ctx_depth != 0
        {
            return;
        }
        // The thunk's entry was emitted over the padding of the unit
        // before it (`kbc::unpad`): restore that padding.
        let window = crate::kbc::WINDOW - 1;
        if before.padded != 0 && before.padded == before.code {
            self.code.truncate(before.code - window);
            crate::kbc::pad(&mut self.code, &mut self.padded);
        } else {
            self.code.truncate(before.code);
            self.padded = before.padded;
        }
        debug_assert_eq!((self.code.len(), self.padded), (before.code, before.padded));
        self.sites.truncate(before.sites);
        self.step_locs.truncate(before.step_locs);
        self.origins.truncate(before.origins);
        self.traps.truncate(before.traps);
        self.funcs.truncate(before.funcs);
        self.meta.sites.truncate(before.meta_sites);
        self.ctor_steps.remove(&t.idx);
        self.exc.forget_func(t.idx);
        self.thunks.remove(&t.key);
    }
}
