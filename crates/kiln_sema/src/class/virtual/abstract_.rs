//! Abstract classes at their use sites ([class.abstract]/3–5; clang
//! `RequireNonAbstractType`, `DiagnoseAbstractType` and
//! `CheckAbstractClassUsage`): an abstract class is not the type of a
//! variable, a non-static data member, a parameter or return type of a
//! function definition, an array element, or an object created by
//! `new` or an explicit type conversion.

use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits};
use kiln_ast::{DeclId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};

use super::graph;
use crate::Sema;

/// The declaration an abstract class type appears in (clang
/// `AbstractDiagSelID`, the `{0}` select of `sema_abstract_type_in_decl`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AbstractUse {
    /// The return type of a function definition.
    Return = 0,
    /// A parameter type of a function definition.
    Param = 1,
    /// A variable.
    Variable = 2,
    /// A non-static data member.
    Field = 3,
}

impl Sema<'_, '_> {
    /// The abstract class (its definition) `q` designates: a complete
    /// class with a pure virtual final overrider. A class being defined
    /// is not known to be abstract yet (its uses are rechecked at `}`).
    pub(crate) fn abstract_class(&self, q: QualType) -> Option<DeclId> {
        let r = self.ast.types.as_record(q)?;
        let def = self.ast.definition(r)?;
        let d = self.ast.decls.record(def)?;
        (d.defn.contains(DefinitionBits::COMPLETE) && d.defn.contains(DefinitionBits::ABSTRACT))
            .then_some(def)
    }

    /// clang `DiagnoseAbstractType`: a note per unimplemented pure
    /// virtual function of `class`, on the first abstract-class
    /// diagnostic for it only.
    pub(crate) fn pure_virtual_notes(&mut self, mut msg: Diag, class: DeclId) -> Diag {
        let first = self.ast.first(class);
        let Some(def) = self.ast.definition(first) else { return msg };
        if !self.abstract_noted.insert(first) {
            return msg;
        }
        let subs = graph::subobject_graph(&self.ast, def);
        let all = graph::all_final_overriders(&self.ast, &subs);
        // method → its subobjects' final overrider candidates.
        let mut by_method: FxHashMap<DeclId, Vec<(u16, DeclId)>> = FxHashMap::default();
        for e in &all {
            by_method.entry(e.1).or_default().push((e.0.0, e.2));
        }
        let cname = self.decl_name(def);
        let mut seen: FxHashSet<DeclId> = FxHashSet::default();
        for m in graph::method_order(&self.ast, def) {
            let Some(cands) = by_method.get(&m) else { continue };
            let mut i = 0;
            while i < cands.len() {
                let mut j = i + 1;
                while j < cands.len() && cands[j].0 == cands[i].0 {
                    j += 1;
                }
                let f = cands[i].1;
                if j - i == 1 && graph::is_pure(&self.ast, f) && seen.insert(f) {
                    msg = msg.with_note(
                        self.diag(DiagId::note_pure_virtual_method, self.ast.decls.get(f).loc())
                            .arg(self.decl_name(f))
                            .arg(cname.clone()),
                    );
                }
                i = j;
            }
        }
        msg
    }

    /// clang `RequireNonAbstractType` for a declaration: `{use} type 'T'
    /// is an abstract class` at `loc` if `q` is an abstract class.
    pub(crate) fn require_non_abstract(
        &mut self,
        loc: Loc,
        q: QualType,
        use_: AbstractUse,
    ) -> bool {
        let Some(c) = self.abstract_class(q) else { return false };
        let msg =
            self.diag(DiagId::sema_abstract_type_in_decl, loc).arg(use_ as u8).arg(self.ty_arg(q));
        let msg = self.pure_virtual_notes(msg, c);
        self.emit(msg);
        true
    }

    /// `allocating an object of abstract class type 'T'` (a new-expression
    /// or an explicit type conversion creating an object of type `q`).
    pub(crate) fn require_non_abstract_alloc(&mut self, loc: Loc, q: QualType) -> bool {
        let Some(c) = self.abstract_class(q) else { return false };
        let msg = self.diag(DiagId::sema_abstract_allocation, loc).arg(self.ty_arg(q));
        let msg = self.pure_virtual_notes(msg, c);
        self.emit(msg);
        true
    }

    /// `array of abstract class type 'T'` at `loc` if element type `q` is
    /// an abstract class (clang `BuildArrayType`).
    pub(crate) fn require_non_abstract_element(&mut self, loc: Loc, q: QualType) -> bool {
        let Some(c) = self.abstract_class(q) else { return false };
        let msg = self.diag(DiagId::sema_abstract_array, loc).arg(self.ty_arg(q));
        let msg = self.pure_virtual_notes(msg, c);
        self.emit(msg);
        true
    }

    /// A variable of abstract class type (clang `ActOnUninitializedDecl`
    /// and `AddInitializerToDecl`; arrays are diagnosed when the array
    /// type is formed). Marks `v` invalid.
    pub(crate) fn check_abstract_var(&mut self, v: DeclId) {
        let n = *self.ast.decls.get(v);
        let Some(ty) = n.ty() else { return };
        if self.require_non_abstract(n.loc(), ty, AbstractUse::Variable) {
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
    }

    /// The return and parameter types of function definition `f`
    /// ([class.abstract]/3; clang `ActOnStartOfFunctionDef` and
    /// `CheckParmsForFunctionDef`): the return type at the name, a
    /// parameter at the start of its declaration.
    pub(crate) fn check_abstract_signature(&mut self, f: DeclId) {
        let n = *self.ast.decls.get(f);
        if n.has(DeclFlags::INVALID) {
            return;
        }
        let fty = n.ty().unwrap_or(QualType::ERROR);
        if let Some(p) = self.ast.types.proto(fty) {
            let ret = p.ret();
            self.require_non_abstract(n.loc(), ret, AbstractUse::Return);
        }
        let params: Vec<DeclId> = self
            .ast
            .decls
            .function(f)
            .map(|x| self.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        for p in params {
            let pn = *self.ast.decls.get(p);
            if pn.has(DeclFlags::INVALID) {
                continue;
            }
            let loc = self.ast.decls.cold.begin_loc.get(p).copied().unwrap_or(pn.loc());
            let pty = pn.ty().unwrap_or(QualType::ERROR);
            self.require_non_abstract(loc, pty, AbstractUse::Param);
        }
    }

    /// clang `CheckAbstractClassUsage` at the `}` of abstract class
    /// `class`: the uses of its own type that could not be checked while
    /// it was being defined, the static data members (and arrays of
    /// them) of the class and its nested classes. Member function
    /// definitions are checked when their bodies are replayed.
    pub(crate) fn recheck_abstract_members(&mut self, scope: DeclId, class: DeclId, depth: usize) {
        if depth > 256 {
            return;
        }
        let members: Vec<DeclId> = self.ast.members(scope).collect();
        for m in members {
            let n = *self.ast.decls.get(m);
            match n.kind() {
                DeclKind::Var if !n.has(DeclFlags::INVALID) => {
                    let ty = n.ty().unwrap_or(QualType::ERROR);
                    let mut et = ty;
                    let mut array = false;
                    for _ in 0..64 {
                        match self.ast.types.element(et) {
                            Some(e) if self.ast.types.is_array(et) => {
                                et = e;
                                array = true;
                            }
                            _ => break,
                        }
                    }
                    let mine = self
                        .ast
                        .types
                        .as_record(et)
                        .is_some_and(|r| self.ast.first(r) == self.ast.first(class));
                    if !mine {
                        continue;
                    }
                    let ok = if array {
                        self.require_non_abstract_element(n.loc(), et)
                    } else {
                        self.require_non_abstract(n.loc(), ty, AbstractUse::Variable)
                    };
                    if ok {
                        self.ast.decls.set_flags(m, DeclFlags::INVALID);
                    }
                }
                DeclKind::CXXRecord
                    if self.ast.definition(m) == Some(m)
                        && self.ast.decls.lexical_ctx(m).or(n.ctx()) == Some(scope) =>
                {
                    self.recheck_abstract_members(m, class, depth + 1);
                }
                _ => {}
            }
        }
    }
}
