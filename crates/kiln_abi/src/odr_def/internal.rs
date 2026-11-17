//! `USES_INTERNAL` (O-16; K-3e), computed during the hash walk: set when
//! the definition odr-uses a function or variable with internal linkage
//! (or one kiln emits as internal: a type without linkage in its
//! signature, [basic.link]/8), and, per mortar's answer of 11-17, when it
//! NAMES a TU-local entity: every per-TU placeholder of the walk (an
//! unnamed-namespace entity or type, a local entity of another function)
//! sets it. Not set by: a reference that is not an odr-use (sema's
//! [basic.def.odr]/5 mark: an unevaluated operand, a usable-in-constant-
//! expressions variable whose value is read, a discarded-value
//! expression), the definition's own local entities and closures, and
//! the signature area (a callee's template signature is the callee's).
//! The standard library's `static inline` helpers are not excepted
//! (O-16: libstdc++'s `__gthread_*`).

use kiln_ast::DeclId;

use super::DefHasher;
use crate::mangle;

impl DefHasher<'_> {
    /// A per-TU placeholder was written for type `rd` (a class or
    /// enumeration, when the type names one): sets the flag when `rd` is
    /// TU-local (internal or no linkage) and not the definition's own.
    pub(super) fn tu_local_type(&mut self, rd: Option<DeclId>) {
        if self.sig > 0 {
            return;
        }
        let Some(rd) = rd else {
            // A type whose TU-local part the walk did not locate.
            self.uses_internal = true;
            return;
        };
        let first = self.ast.first(rd);
        if mangle::class_is_internal(self.ast, first) && !self.own_local(first) {
            self.uses_internal = true;
            self.seen_internal(first);
        }
    }

    /// A per-TU placeholder was written for function or variable `d`:
    /// sets the flag on an odr-use of a TU-local one.
    pub(super) fn tu_local_entity(&mut self, d: DeclId) {
        let odr_use = self.cur_odr_use;
        let before = self.internal_named;
        self.named_entity(d, odr_use);
        self.internal_named = before;
    }

    /// Function or variable `d`, named by its mangled name: sets the flag
    /// when it is an odr-use of an internal entity.
    pub(super) fn named_entity(&mut self, d: DeclId, odr_use: bool) {
        if self.sig > 0 || !odr_use {
            return;
        }
        let first = self.ast.first(d);
        let internal = if let Some(&i) = self.internal.get(&first) {
            i
        } else {
            let i = mangle::is_internal_symbol(self.ast, first) && !self.own_local(first);
            self.internal.insert(first, i);
            i
        };
        if internal {
            self.uses_internal = true;
            self.internal_named = true;
            self.seen_internal(first);
        }
    }

    fn seen_internal(&mut self, first: DeclId) {
        if !self.internal_seen.contains(&first) {
            self.internal_seen.push(first);
        }
    }

    /// Whether `d` lies inside the definition being hashed (its local
    /// classes' members, its closures' call operators).
    fn own_local(&self, d: DeclId) -> bool {
        let ast = self.ast;
        let mut c = ast.decls.get(d).ctx();
        while let Some(x) = c {
            if ast.first(x) == self.root {
                return true;
            }
            c = ast.decls.get(x).ctx();
        }
        false
    }
}
