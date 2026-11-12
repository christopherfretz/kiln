//! The virtual-function checks at a class's `}` (clang
//! `CXXRecordDecl::completeDefinition` and `CheckCompletedCXXClass`) and
//! after its complete-class contexts (the delayed exception-specification
//! check and the key function).

use kiln_ast::decl::{
    ConstexprKind, DeclFlags, DeclKind, DefinitionBits, FnSpec, FnStorage, TagKind, TemplLinkKind,
};
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{ExcSpec, Quals};
use kiln_ast::{DeclId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

use super::graph::{self, is_method_kind, is_pure, is_virtual, overridden};
use crate::Sema;

impl Sema<'_, '_> {
    /// The class properties virtual functions and virtual bases decide
    /// ([class.prop], [class.default.ctor]/3, [class.copy.ctor]/11,
    /// [class.dtor]/8, [basic.types.general]/10, [dcl.init.aggr]/1,
    /// [class.abstract]/2), applied to the `bits` of class `r` at its
    /// `}` before it is marked complete: `POLYMORPHIC`,
    /// `HAS_VIRTUAL_BASES` and `ABSTRACT`.
    pub(crate) fn virtual_class_bits(&self, r: DeclId, mut bits: DefinitionBits) -> DefinitionBits {
        let mut own_virtual = false;
        let mut own_pure = false;
        for m in self.ast.members(r) {
            if is_method_kind(self.ast.decls.get(m).kind()) && is_virtual(&self.ast, m) {
                own_virtual = true;
                own_pure |= is_pure(&self.ast, m);
            }
        }
        let mut polymorphic = own_virtual;
        let mut vbases = false;
        let mut abstract_base = false;
        for b in self.ast.decls.bases(r, &self.ast.lists) {
            let bb = self
                .ast
                .types
                .as_record(b.ty)
                .and_then(|d| self.ast.definition(d))
                .and_then(|d| self.ast.decls.record(d))
                .map(|x| x.defn)
                .unwrap_or_default();
            polymorphic |= bb.contains(DefinitionBits::POLYMORPHIC);
            vbases |= b.is_virtual || bb.contains(DefinitionBits::HAS_VIRTUAL_BASES);
            abstract_base |= bb.contains(DefinitionBits::ABSTRACT);
        }
        if polymorphic {
            bits = bits.with(DefinitionBits::POLYMORPHIC);
        }
        if vbases {
            // [basic.types.general]/10.5: no virtual base classes before
            // C++26 (a constexpr destructor could not have one; P3533R2
            // lifted that, and `literal_bits` decides).
            bits = bits.with(DefinitionBits::HAS_VIRTUAL_BASES).without(DefinitionBits::STRUCTURAL);
            if self.std < kiln_lex::Std::Cxx26 {
                bits = bits.without(DefinitionBits::LITERAL);
            }
        }
        if polymorphic || vbases {
            for b in [
                DefinitionBits::AGGREGATE,
                DefinitionBits::STANDARD_LAYOUT,
                DefinitionBits::EMPTY,
                DefinitionBits::TRIVIALLY_COPYABLE,
                DefinitionBits::TRIVIAL_DEFAULT_CTOR,
                DefinitionBits::TRIVIAL_COPY_CTOR,
                DefinitionBits::TRIVIAL_MOVE_CTOR,
                DefinitionBits::TRIVIAL_COPY_ASSIGN,
                DefinitionBits::TRIVIAL_MOVE_ASSIGN,
            ] {
                bits = bits.without(b);
            }
        }
        if self.ast.destructor(r).is_some_and(|d| is_virtual(&self.ast, d)) {
            bits = bits.without(DefinitionBits::TRIVIAL_DTOR);
        }
        // [class.abstract]/2: a pure virtual final overrider (clang
        // computes it only when a base is abstract; an own pure function
        // makes the class abstract outright). Without a declared
        // destructor, the implicit one (declared after this, never pure)
        // overrides a base's pure virtual destructor (boost::exception's
        // `virtual ~exception() = 0`; clang declares it before for a
        // dynamic class).
        let implicit_dtor = self.ast.destructor(r).is_none();
        if own_pure
            || abstract_base
                && graph::virtuals_of(&self.ast, r).overriders.iter().any(|e| {
                    is_pure(&self.ast, e.2)
                        && !(implicit_dtor
                            && self.ast.decls.get(e.2).kind() == DeclKind::CXXDestructor)
                })
        {
            bits = bits.with(DefinitionBits::ABSTRACT);
        }
        bits
    }

    /// The checks at the `}` of class `r`, once it is complete
    /// (clang `CheckCompletedCXXClass`).
    pub(crate) fn act_on_complete_virtual(&mut self, r: DeclId) {
        let defn = self.ast.decls.record(r).map(|x| x.defn).unwrap_or_default();
        // clang runs the final-overrider and per-member-function checks
        // only for a non-dependent class; a templated class is checked at
        // instantiation ([temp.res.general]/6).
        let dependent = self.ast.decls.get(r).has(DeclFlags::DEPENDENT_CTX);
        if defn.contains(DefinitionBits::HAS_VIRTUAL_BASES) && !dependent {
            self.check_unique_final_overriders(r);
        }
        if defn.contains(DefinitionBits::ABSTRACT) {
            if defn.contains(DefinitionBits::FINAL) {
                let msg = self.diag(DiagId::sema_abstract_final_class, self.ast.decls.get(r).loc());
                let msg = self.pure_virtual_notes(msg, r);
                self.emit(msg);
            }
            self.recheck_abstract_members(r, r, 0);
        }
        if !dependent {
            self.check_refconst_members(r, defn);
            self.report_overrides(r);
        }
    }

    /// `virtual function 'f' has more than one final overrider`
    /// ([class.virtual]/2: a virtual function has a unique final
    /// overrider in a complete class).
    fn check_unique_final_overriders(&mut self, r: DeclId) {
        let subs = graph::subobject_graph(&self.ast, r);
        let all = graph::all_final_overriders(&self.ast, &subs);
        let mut by_method: FxHashMap<DeclId, Vec<(u16, DeclId)>> = FxHashMap::default();
        for e in &all {
            by_method.entry(e.1).or_default().push((e.0.0, e.2));
        }
        let rname = format!("'{}'", self.qualified_decl_name(r));
        let mut bad = false;
        for m in graph::method_order(&self.ast, r) {
            let Some(cands) = by_method.get(&m) else { continue };
            let mut i = 0;
            while i < cands.len() {
                let mut j = i + 1;
                while j < cands.len() && cands[j].0 == cands[i].0 {
                    j += 1;
                }
                if j - i > 1 {
                    let mname = self.qualified_decl_name(m);
                    let mut msg = self
                        .diag(DiagId::sema_multiple_final_overriders, self.ast.decls.get(r).loc())
                        .arg(mname.clone())
                        .arg(rname.clone())
                        .with_note(self.overridden_note(m));
                    for &(_, f) in &cands[i..j] {
                        let parent = self.ast.decls.get(f).ctx().unwrap_or(r);
                        msg = msg.with_note(
                            self.diag(DiagId::note_final_overrider, self.ast.decls.get(f).loc())
                                .arg(mname.clone())
                                .arg(self.qualified_decl_name(parent)),
                        );
                    }
                    self.emit(msg);
                    bad = true;
                }
                i = j;
            }
        }
        if bad {
            self.ast.decls.set_flags(r, DeclFlags::INVALID);
        }
    }

    /// The per-member-function checks at `}` (clang
    /// `CheckCompletedCXXClass`): a static, deleted or consteval function
    /// overriding one that is not, `-Woverloaded-virtual` and
    /// `-Winconsistent-missing-override`.
    fn report_overrides(&mut self, r: DeclId) {
        let methods: Vec<DeclId> =
            self.ast.members(r).filter(|&m| is_method_kind(self.ast.decls.get(m).kind())).collect();
        let hidden_on = self.diags.is_enabled(DiagId::sema_hidden_overloaded_virtual);
        let mut any_override = false;
        let mut missing: Vec<DeclId> = Vec::new();
        for &m in &methods {
            let n = *self.ast.decls.get(m);
            let spec = self.ast.decls.function(m).map(|f| f.spec).unwrap_or_default();
            let os: Vec<DeclId> = overridden(&self.ast, m).collect();
            if !n.has(DeclFlags::IMPLICIT) {
                any_override |= spec.is_override();
                if !spec.is_override() && !spec.is_final() && !os.is_empty() {
                    missing.push(m);
                }
            }
            if !os.is_empty() {
                self.report_override_mismatch(m, &os);
            }
            if hidden_on && !n.has(DeclFlags::INVALID) && n.kind() == DeclKind::CXXMethod {
                let ty = n.ty().unwrap_or(QualType::ERROR);
                let hidden = self.hidden_virtuals(r, n.name(), ty, &[]);
                if !hidden.is_empty() {
                    let msg = self
                        .diag(DiagId::sema_hidden_overloaded_virtual, n.loc())
                        .arg(self.qualified_decl_name(m))
                        .arg(u8::from(hidden.len() > 1));
                    let msg = self.hidden_virtual_notes(msg, ty, &hidden);
                    self.emit(msg);
                }
            }
        }
        if any_override {
            for m in missing {
                let n = *self.ast.decls.get(m);
                // The destructor form is off by default
                // (`-Winconsistent-missing-destructor-override`).
                if n.has(DeclFlags::INVALID)
                    || n.kind() == DeclKind::CXXDestructor
                    || self.name_spelled_in_system_header(n.loc())
                {
                    continue;
                }
                let Some(o) = overridden(&self.ast, m).next() else { continue };
                let msg = self
                    .diag(DiagId::sema_inconsistent_missing_override, n.loc())
                    .arg(self.decl_name(m))
                    .with_note(self.overridden_note(o));
                self.emit(msg);
            }
        }
    }

    /// clang `DiagnoseAbsenceOfOverrideControl`: a method whose name is
    /// spelled in a system header is not diagnosed; a name that is a macro
    /// argument counts where the macro's body uses the parameter, so a
    /// system-header macro that declares the method (googletest's
    /// `MOCK_METHOD` under `-isystem`) silences it even though the name
    /// was written in user code.
    fn name_spelled_in_system_header(&self, loc: Loc) -> bool {
        let sm = self.diags.sm();
        let at = match sm.expansion_info(loc) {
            Some(e) if e.kind == kiln_source::ExpansionKind::MacroArg => e.expansion.lo,
            _ => loc,
        };
        let at = sm.spelling_loc(at);
        at.is_valid() && sm.is_in_system_header(at)
    }

    /// clang `ReportOverrides` for the static, deleted and consteval
    /// mismatches of `m` against the functions it
    /// overrides.
    fn report_override_mismatch(&mut self, m: DeclId, os: &[DeclId]) {
        let spec_of =
            |s: &Self, d: DeclId| s.ast.decls.function(d).map(|f| f.spec).unwrap_or_default();
        let ms = spec_of(self, m);
        let name = self.decl_name(m);
        let loc = self.ast.decls.get(m).loc();
        if ms.storage() == FnStorage::Static {
            let mut msg = self.diag(DiagId::sema_static_overrides_virtual, loc).arg(name);
            for &o in os {
                msg = msg.with_note(self.overridden_note(o));
            }
            self.emit(msg);
            return;
        }
        let deleted: Vec<DeclId> = os
            .iter()
            .copied()
            .filter(|&o| spec_of(self, o).is_deleted() != ms.is_deleted())
            .collect();
        if !deleted.is_empty() {
            let mut msg = self
                .diag(DiagId::sema_deleted_overrides, loc)
                .arg(u8::from(!ms.is_deleted()))
                .arg(name);
            for o in deleted {
                msg = msg.with_note(self.overridden_note(o));
            }
            // An implicitly deleted override is explained (clang
            // `NoteDeletedFunction` after `err_deleted_override`).
            if ms.is_deleted() && ms.defaulted() != kiln_ast::decl::Defaulted::No {
                for n in self.deleted_function_notes(m, 0) {
                    msg = msg.with_note(n);
                }
            }
            self.emit(msg);
            return;
        }
        let ce = |s: FnSpec| s.constexpr_kind() == ConstexprKind::Consteval;
        // [class.virtual]/18 (the working draft, P4101R1 reverted
        // CWG3117's allowance): a consteval function never overrides a
        // non-consteval one, nor the reverse. GCC 16.1's libstdc++
        // `<meta>` still relies on the allowance (`meta::exception::what`
        // is a consteval override of `std::exception::what`), so an
        // overrider declared in a system header is exempt (kiln
        // extension; spec/reflection-repr.md §6).
        let exempt = ce(ms) && self.reflection && self.diags.sm().is_in_system_header(loc);
        let consteval: Vec<DeclId> =
            os.iter().copied().filter(|&o| ce(spec_of(self, o)) != ce(ms) && !exempt).collect();
        if !consteval.is_empty() {
            let mut msg =
                self.diag(DiagId::sema_consteval_overrides, loc).arg(u8::from(!ce(ms))).arg(name);
            for o in consteval {
                msg = msg.with_note(self.overridden_note(o));
            }
            self.emit(msg);
        }
    }

    /// The member functions of `r` and its nested classes that override
    /// a virtual function, in declaration order.
    fn collect_overriders(&self, r: DeclId, depth: usize, out: &mut Vec<DeclId>) {
        if depth > 256 {
            return;
        }
        for m in self.ast.members(r) {
            let k = self.ast.decls.get(m).kind();
            if is_method_kind(k) && overridden(&self.ast, m).next().is_some() {
                out.push(m);
            } else if k == DeclKind::CXXRecord && self.ast.definition(m) == Some(m) {
                self.collect_overriders(m, depth + 1, out);
            }
        }
    }

    /// After the complete-class contexts of class `r` (and its nested
    /// classes) are parsed: the overriders' exception specifications
    /// ([except.spec]/8; `noexcept(expr)` is parsed late) and the key
    /// function (Itanium C++ ABI §5.2.3).
    pub(crate) fn act_on_virtual_class_contexts(&mut self, r: DeclId, depth: usize) {
        if depth > 256 {
            return;
        }
        if depth == 0 {
            // clang checks an overrider without a written specification
            // when it is declared, the others (and implicit ones) at the
            // end of the outermost class: those first.
            let mut overriders = Vec::new();
            self.collect_overriders(r, 0, &mut overriders);
            let (now, delayed): (Vec<DeclId>, Vec<DeclId>) =
                overriders.into_iter().partition(|&m| {
                    self.ast.decls.get(m).ty().map(|t| self.written_exc_spec(t))
                        == Some(ExcSpec::None)
                });
            for m in now.into_iter().chain(delayed) {
                self.check_override_exc_spec(m);
            }
        }
        let members: Vec<DeclId> = self.ast.members(r).collect();
        let polymorphic =
            self.ast.decls.record(r).is_some_and(|x| x.defn.contains(DefinitionBits::POLYMORPHIC));
        for &m in &members {
            let k = self.ast.decls.get(m).kind();
            if k == DeclKind::CXXRecord && self.ast.definition(m) == Some(m) {
                self.act_on_virtual_class_contexts(m, depth + 1);
            }
        }
        if polymorphic
            && self.may_have_key_function(r)
            && !self.ast.decls.cold.key_function.contains(r)
        {
            let key = members.iter().copied().find(|&m| self.is_key_function_candidate(m));
            if let Some(k) = key {
                let _ = self.ast.decls.cold.key_function.insert(r, k);
            }
        }
        if polymorphic {
            self.mark_module_vtable_used(r);
        }
    }

    /// clang `computeKeyFunction`: a dependent class (a template pattern,
    /// a partial specialization, or a class nested in one) has no vtable,
    /// and a template instantiation (implicit or explicit, and a member
    /// class of one) has no key function (Itanium C++ ABI §5.2.6: its
    /// vtable has vague linkage and is emitted wherever it is used). An
    /// explicit specialization is an ordinary class and keeps it.
    fn may_have_key_function(&self, r: DeclId) -> bool {
        let ast = &self.ast;
        if ast.decls.get(r).has(DeclFlags::DEPENDENT_CTX) {
            return false;
        }
        match ast.templ_link(r).get() {
            None => true,
            Some((
                TemplLinkKind::Pattern
                | TemplLinkKind::PartialSpecialization
                | TemplLinkKind::DependentFriend,
                _,
            )) => false,
            Some((TemplLinkKind::Specialization, _)) => {
                ast.spec_info(r).is_some_and(|(_, s)| s.kind == SpecKind::ExplicitSpecialization)
            }
            Some((TemplLinkKind::MemberOfSpecialization, _)) => ast
                .member_spec_info(r)
                .is_some_and(|(_, m)| m.kind == SpecKind::ExplicitSpecialization),
        }
    }

    /// Itanium C++ ABI §5.2.3: the first non-pure virtual function that
    /// is not inline at the point of class definition (declared, not
    /// defined, in the class, and neither `inline` nor `constexpr`).
    fn is_key_function_candidate(&self, m: DeclId) -> bool {
        let n = self.ast.decls.get(m);
        if !is_method_kind(n.kind()) || n.has(DeclFlags::IMPLICIT) || !is_virtual(&self.ast, m) {
            return false;
        }
        let Some(f) = self.ast.decls.function(m) else { return false };
        let s = f.spec;
        !s.is_pure()
            && !s.is_inline()
            && !s.implicit_inline()
            && s.constexpr_kind() == ConstexprKind::None
            && !s.is_deleted()
            && s.defaulted() == kiln_ast::decl::Defaulted::No
            && self.ast.definition(m).is_none_or(|d| d != m)
    }

    /// clang `CheckCompletedCXXClass`: a non-aggregate class with no
    /// user-declared constructor cannot initialize its reference or const
    /// scalar members that lack a default member initializer
    /// ([class.default.ctor]/2 deletes the default constructor).
    fn check_refconst_members(&mut self, r: DeclId, defn: DefinitionBits) {
        if self.ast.decls.get(r).has(DeclFlags::INVALID)
            || defn.contains(DefinitionBits::AGGREGATE)
            || defn.contains(DefinitionBits::USER_DECLARED_CTOR)
            || defn.contains(DefinitionBits::LAMBDA)
        {
            return;
        }
        let mut msg: Option<Diag> = None;
        let fields: Vec<DeclId> =
            self.ast.decls.members(r).filter(|&m| self.kind_of(m) == DeclKind::Field).collect();
        for f in fields {
            if self.field_has_nsdmi(f) || self.is_unnamed_bit_field(f) {
                continue;
            }
            let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            let which = if self.ast.types.is_reference(ty) {
                1u8
            } else if self.ast.types.canonical(ty).quals().contains(Quals::CONST)
                && self.is_scalar(ty)
            {
                0
            } else {
                continue;
            };
            let m = msg.take().unwrap_or_else(|| {
                let tag = match self.ast.decls.record(r).map(|x| x.tag) {
                    Some(TagKind::Union) => 2u8,
                    Some(TagKind::Class) => 3,
                    _ => 0,
                };
                // clang names the declaration: a specialization with its
                // template arguments, a nested class without an `aka`.
                let name = if self.ast.spec_info(r).is_some() {
                    let ty = self.this_class_type(r);
                    self.ty_arg(ty)
                } else {
                    self.decl_arg(r)
                };
                self.diag(DiagId::sema_refconst_no_ctor, self.ast.decls.get(r).loc())
                    .arg(tag)
                    .arg(name)
            });
            let note = self
                .diag(DiagId::note_refconst_member_not_initialized, self.ast.decls.get(f).loc())
                .arg(which)
                .arg(self.decl_arg(f));
            msg = Some(m.with_note(note));
        }
        if let Some(m) = msg {
            self.emit(m);
        }
    }
}
