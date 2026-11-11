//! Diagnostic state changed by `#pragma clang diagnostic` /
//! `#pragma GCC diagnostic` (`spec/diag.md` section "Pragma state"): a
//! transcription of clang's `DiagnosticsEngine::DiagStateMap`.
//!
//! A pragma makes a new state (the current one with some mappings
//! replaced) current from its location on; `pop` makes the state saved by
//! the matching `push` current again. The severity of a diagnostic is
//! decided by the state at its location (its expansion location), so a
//! diagnostic emitted late (a template instantiation at the end of the
//! translation unit) still sees the pragmas around the code it is about.
//!
//! Transition points are kept sorted by location. Locations of one buffer
//! range (one inclusion) are contiguous and a range has no transition
//! points of another inside it, so the state at a location is the last
//! point at or before it in the same range, else the state at the range's
//! include location (clang `File::lookup` through `Parent`). Leaving an
//! included file adds a point at the includer's resume location when the
//! current state differs there, so a pragma in a header without a `push`
//! affects the rest of the includer, as in clang.

use std::collections::HashMap;
use std::sync::Arc;

use kiln_source::{Loc, SourceManager};

use crate::groups::{Flavor, group_members};
use crate::ids::{DiagClass, DiagId};
use crate::policy::{Mapping, Policy, Severity};

/// Mappings a state replaces, over the command-line policy.
type Overrides = HashMap<DiagId, Mapping>;

/// The `#pragma clang diagnostic` state at one point, detached from
/// locations ([`crate::DiagEngine::pragma_snapshot`]): the states the
/// current state and the `push` stack name, each as its overrides.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PragmaSnapshot {
    /// Each state's overrides: `(id, severity, flags)`, flags bit 0 set
    /// by a pragma, bit 1 `-Wno-error`, bit 2 `-Wno-fatal-errors`.
    pub states: Vec<Vec<(DiagId, Severity, u8)>>,
    /// The current state (an index into `states`).
    pub cur: u32,
    /// The states saved by `push`, innermost last.
    pub stack: Vec<u32>,
}

#[derive(Debug)]
pub(crate) struct Pragmas {
    /// Index 0 is the command-line state (no overrides).
    states: Vec<Arc<Overrides>>,
    /// Transition points, sorted by location: `(at, state)`.
    points: Vec<(Loc, u32)>,
    /// The state the preprocessor is in now (clang `getCurDiagState`).
    cur: u32,
    /// The states saved by `push` (clang `DiagStateOnPushStack`).
    stack: Vec<u32>,
    /// Ids some pragma maps to a severity other than `Ignored`.
    enabled: Vec<bool>,
}

impl Pragmas {
    pub(crate) fn new() -> Pragmas {
        Pragmas {
            states: vec![Arc::new(Overrides::new())],
            points: Vec::new(),
            cur: 0,
            stack: Vec::new(),
            enabled: vec![false; DiagId::ALL.len()],
        }
    }

    /// Whether any pragma changed the state (otherwise every location is
    /// in the command-line state).
    pub(crate) fn is_empty(&self) -> bool {
        self.points.is_empty() && self.cur == 0
    }

    /// Whether some pragma enables `id`.
    pub(crate) fn enables(&self, id: DiagId) -> bool {
        self.enabled[id as usize]
    }

    /// The mapping override for `id` at `loc`, if a pragma set one. An
    /// invalid location uses the current state (as clang).
    pub(crate) fn mapping_at(&self, sm: &SourceManager, id: DiagId, loc: Loc) -> Option<Mapping> {
        let s = if loc.is_valid() { self.state_at(sm, loc) } else { self.cur };
        self.states[s as usize].get(&id).copied()
    }

    fn state_at(&self, sm: &SourceManager, loc: Loc) -> u32 {
        let mut loc = sm.expansion_loc(loc);
        // The include chain is finite; the bound only guards a cycle.
        for _ in 0..1024 {
            if !loc.is_valid() {
                return 0;
            }
            let i = self.points.partition_point(|&(at, _)| at <= loc);
            if i > 0 {
                let (at, s) = self.points[i - 1];
                if sm.same_entry_distance(at, loc).is_some() {
                    return s;
                }
            }
            loc = sm.include_loc(loc);
        }
        0
    }

    /// Makes `s` current from `at` on. A point already at `at` is
    /// replaced (clang modifies the state in place).
    fn transition(&mut self, at: Loc, s: u32) {
        self.cur = s;
        let i = self.points.partition_point(|&(l, _)| l < at);
        match self.points.get_mut(i) {
            Some(p) if p.0 == at => p.1 = s,
            _ => self.points.insert(i, (at, s)),
        }
    }

    /// `#pragma ... diagnostic push` (clang `pushMappings`).
    pub(crate) fn push(&mut self) {
        self.stack.push(self.cur);
    }

    /// `#pragma ... diagnostic pop` (clang `popMappings`); false if
    /// nothing was pushed.
    pub(crate) fn pop(&mut self, at: Loc) -> bool {
        let Some(s) = self.stack.pop() else { return false };
        self.transition(at, s);
        true
    }

    /// `#pragma ... diagnostic {ignored,warning,error,fatal} "-Wgroup"`
    /// (clang `setSeverityForGroup` / `setSeverityForAll` for
    /// `-Weverything`, then `setSeverity` per member). False if `group`
    /// is unknown.
    pub(crate) fn set(
        &mut self,
        policy: &Policy,
        at: Loc,
        flavor: Flavor,
        group: &str,
        sev: Severity,
    ) -> bool {
        let ids: Vec<DiagId> = if group == "everything" {
            DiagId::ALL
                .iter()
                .copied()
                .filter(|&id| {
                    flavor.matches(id)
                        && (id.is_warning_or_extension() || id.class() == DiagClass::Remark)
                })
                .collect()
        } else {
            match group_members(flavor, group, policy.compat) {
                Some(ids) => ids,
                None => return false,
            }
        };
        let mut over = Overrides::clone(&self.states[self.cur as usize]);
        for id in ids {
            let old = over.get(&id).copied().unwrap_or_else(|| policy.mapping(id));
            // Unlike a command-line `-Wfoo`, a `warning` pragma overrides
            // an error mapping; `-Wno-error` carries over (clang
            // `setSeverity`).
            over.insert(
                id,
                Mapping { severity: sev, user: true, no_werror: old.no_werror, no_fatal: false },
            );
            if sev != Severity::Ignored {
                self.enabled[id as usize] = true;
            }
        }
        let s = u32::try_from(self.states.len()).unwrap_or(u32::MAX);
        self.states.push(Arc::new(over));
        self.transition(at, s);
        true
    }

    /// The current state and the `push` stack, as the states they name
    /// (`(id, severity, flags)` with flags bit 0 `user`, bit 1
    /// `no_werror`, bit 2 `no_fatal`; ids sorted).
    pub(crate) fn snapshot(&self) -> PragmaSnapshot {
        let mut snap = PragmaSnapshot::default();
        let mut ix: HashMap<u32, u32> = HashMap::new();
        let mut state = |s: u32, snap: &mut PragmaSnapshot| -> u32 {
            *ix.entry(s).or_insert_with(|| {
                let mut v: Vec<(DiagId, Severity, u8)> = self.states[s as usize]
                    .iter()
                    .map(|(&id, m)| {
                        let f = u8::from(m.user)
                            | (u8::from(m.no_werror) << 1)
                            | (u8::from(m.no_fatal) << 2);
                        (id, m.severity, f)
                    })
                    .collect();
                v.sort_by_key(|e| e.0 as usize);
                snap.states.push(v);
                u32::try_from(snap.states.len() - 1).unwrap_or(u32::MAX)
            })
        };
        snap.cur = state(self.cur, &mut snap);
        for &s in &self.stack {
            let k = state(s, &mut snap);
            snap.stack.push(k);
        }
        snap
    }

    /// Makes `snap`'s current state current from `at` on and appends its
    /// stack to the `push` stack.
    pub(crate) fn install(&mut self, at: Loc, snap: &PragmaSnapshot) {
        let base = u32::try_from(self.states.len()).unwrap_or(u32::MAX);
        for st in &snap.states {
            let mut over = Overrides::new();
            for &(id, severity, f) in st {
                over.insert(
                    id,
                    Mapping {
                        severity,
                        user: f & 1 != 0,
                        no_werror: f & 2 != 0,
                        no_fatal: f & 4 != 0,
                    },
                );
                if severity != Severity::Ignored {
                    self.enabled[id as usize] = true;
                }
            }
            self.states.push(Arc::new(over));
        }
        self.stack.extend(snap.stack.iter().map(|&k| base.saturating_add(k)));
        // An empty current state is the default one: nothing changes.
        if snap.states.get(snap.cur as usize).is_some_and(|s| !s.is_empty()) {
            self.transition(at, base.saturating_add(snap.cur));
        }
    }

    /// The preprocessor left an included file and resumes its includer at
    /// `resume`: the current state continues there.
    pub(crate) fn file_exit(&mut self, sm: &SourceManager, resume: Loc) {
        if self.is_empty() || !resume.is_valid() {
            return;
        }
        if self.state_at(sm, resume) != self.cur {
            self.transition(resume, self.cur);
        }
    }
}
