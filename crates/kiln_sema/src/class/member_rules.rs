//! Member declaration rules of [class.static.data], [class.union] and
//! [class.bit] (WP-1015): clang's static-data-member reinitialization
//! check of `AddInitializerToDecl`, the union rules of
//! `ActOnFunctionDeclarator` and `CheckCompletedCXXClass`, and the
//! anonymous bit-field rule of `CheckFieldDecl` (spec/sema.md §4.4.1
//! "Union, bit-field and static data member rules").

use kiln_ast::decl::{DeclKind, FieldData};
use kiln_ast::types::RefQual;
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::init::Initializer;

impl Sema<'_, '_> {
    /// [class.static.data]/4: when the in-class declaration of a static
    /// data member has an initializer, the namespace-scope definition
    /// has none. Reported at the new initializer (clang
    /// `err_static_data_member_reinitialization`, at its expression
    /// location: `(` of a parenthesized list, `{` of a braced one) with
    /// a note at the in-class one; true when it did (the initializer is
    /// then dropped). A constexpr or inline member's in-class
    /// declaration is a definition: that is a redefinition, reported by
    /// the redeclaration merge before this runs.
    pub(crate) fn static_member_reinit(&mut self, v: DeclId, init: &Initializer) -> bool {
        let n = *self.ast.decls.get(v);
        let Some(ctx) = n.ctx() else { return false };
        if self.kind_of(ctx) != DeclKind::CXXRecord || self.is_in_class_static(v) {
            return false;
        }
        let first = self.ast.first(v);
        if first == v || !self.is_in_class_static(first) {
            return false;
        }
        let Some(old) = self.ast.decls.var(first).and_then(|x| x.init) else { return false };
        let loc: Loc = match init {
            Initializer::Expr(e) => self.expr_loc(*e),
            Initializer::Paren(_, lp, _) => *lp,
            Initializer::List(l) => l.lbrace,
        };
        let name = self.decl_name_str(n.name());
        let note = self.diag(DiagId::note_init_previous, self.expr_loc(old));
        let m = self.diag(DiagId::sema_static_data_member_reinit, loc).arg(name).with_note(note);
        self.emit(m);
        true
    }

    /// [class.union.general]/6 (CWG1460): at most one variant member of
    /// a union has a default member initializer. Run before a member
    /// with one (at `loc`: its name, or the first initialized member of
    /// an anonymous member about to be added) joins class `ctx`: when
    /// `ctx` is a union that already has one, clang's
    /// `checkDuplicateDefaultInit` reports `err_init_multiple_union` at
    /// `loc` with a note at the first. Only for the written definition
    /// (an instantiation is not diagnosed again).
    pub(crate) fn check_union_default_inits(&mut self, ctx: DeclId, loc: Loc) {
        if !self.inst.stack.is_empty() || self.kind_of(ctx) != DeclKind::CXXRecord {
            return;
        }
        if !self.ast.decls.record(ctx).is_some_and(|x| x.tag == kiln_ast::decl::TagKind::Union) {
            return;
        }
        let Some(p) = self.first_default_init(ctx) else { return };
        let note = self.diag(DiagId::note_init_previous, self.ast.decls.get(p).loc());
        let m = self.diag(DiagId::sema_init_multiple_union, loc).with_note(note).allow_duplicate();
        self.emit(m);
    }

    /// The first member of class `r` with a default member initializer,
    /// looking into its anonymous members (clang's
    /// `findDefaultInitializer`, which walks the indirect fields too).
    pub(crate) fn first_default_init(&self, r: DeclId) -> Option<DeclId> {
        self.first_default_init_in(r, 0)
    }

    fn first_default_init_in(&self, r: DeclId, depth: u32) -> Option<DeclId> {
        if depth > 64 {
            return None;
        }
        for m in self.ast.decls.members(r) {
            let n = self.ast.decls.get(m);
            if n.kind() != DeclKind::Field {
                continue;
            }
            let fd = self.ast.decls.field(m);
            if fd.is_some_and(|f| f.flags & FieldData::HAS_DEFAULT_INIT != 0) {
                return Some(m);
            }
            if n.name().as_ident().is_none()
                && fd.is_some_and(|f| f.bit_width.is_none())
                && let Some(rec) = n.ty().and_then(|t| self.ast.types.as_record(t))
                && self.ast.decls.get(rec).name().as_ident().is_none()
                && let Some(x) = self.first_default_init_in(rec, depth + 1)
            {
                return Some(x);
            }
        }
        None
    }
}

impl Sema<'_, '_> {
    /// A non-static member function `new` declared in its class with the
    /// parameter-type-list of an earlier member function whose ref-
    /// qualifier differs, one of them having none. Before C++23 the two
    /// cannot be overloaded ([over.load]/2.3; clang
    /// `err_ref_qualifier_overload`, at the name with a note at the
    /// earlier one; both stay declared). From C++23 their object
    /// parameters correspond when their cv-qualifiers agree
    /// ([basic.scope.scope]/3): the earlier one is then returned as the
    /// declaration `new` redeclares, which a member function cannot be
    /// ("class member cannot be redeclared").
    pub(crate) fn ref_qualifier_mix(
        &mut self,
        dname: kiln_ast::names::DeclName,
        new: crate::overload::redecl::NewFn,
        loc: Loc,
    ) -> Option<DeclId> {
        if new.is_static {
            return None;
        }
        let cands = self.prev_candidates_dn(dname, None);
        let p = self.ref_qualifier_mix_prev(&cands, new)?;
        let (pq, pr) = self.method_object_quals(p);
        let info =
            self.ast.types.proto(new.ty).map(kiln_ast::types::FuncProto::info).unwrap_or_default();
        if self.std >= kiln_lex::Std::Cxx23 {
            return (pq == info.this_quals()).then_some(p);
        }
        let m = self
            .diag(DiagId::sema_ref_qualifier_overload, loc)
            .arg(info.ref_qual() as u8)
            .arg(pr as u8);
        let m = self.note_prev(m, p, false);
        self.emit(m);
        None
    }

    /// The virtual-override form of the ref-qualifier rule (CWG2496; clang
    /// 22 in every language mode, clang 19 only before C++23): a non-static member function of `class` with the
    /// parameter-type-list of a virtual function of a base whose ref-
    /// qualifier differs, one of them having none, whatever their
    /// cv-qualifiers (clang's `AddOverriddenMethods` runs `IsOverload`
    /// on each virtual base member, which reports the mix). One error per
    /// such base function, at `loc`, with a note at it; neither overrides
    /// the other.
    pub(crate) fn check_override_ref_mix(
        &mut self,
        class: DeclId,
        name: kiln_ast::names::DeclName,
        ty: QualType,
        loc: Loc,
    ) {
        let Some(info) = self.ast.types.proto(ty).map(kiln_ast::types::FuncProto::info) else {
            return;
        };
        let mut mixed = Vec::new();
        let mut seen_v = kiln_source::fx::FxHashSet::default();
        self.overridden_walk(class, &mut seen_v, 0, &mut |s, b| {
            for x in s.ast.lookup_local(b, name).iter() {
                if s.kind_of(x) != DeclKind::CXXMethod {
                    continue;
                }
                let m = s.ast.first(x);
                if !crate::class::is_virtual(&s.ast, m) {
                    continue;
                }
                let mty = s.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
                if !s.is_method_overload(ty, mty) {
                    return true;
                }
                if s.same_params_ref_mix(ty, mty) && !mixed.contains(&m) {
                    mixed.push(m);
                }
            }
            false
        });
        for m in mixed {
            let (_, pr) = self.method_object_quals(m);
            let msg = self
                .diag(DiagId::sema_ref_qualifier_overload, loc)
                .arg(info.ref_qual() as u8)
                .arg(pr as u8)
                .allow_duplicate();
            let msg = self.note_prev(msg, m, false);
            self.emit(msg);
        }
    }

    /// Whether function types `a` and `b` have the same parameter-type-
    /// list and variadic-ness and differing ref-qualifiers, one of them
    /// none.
    fn same_params_ref_mix(&self, a: QualType, b: QualType) -> bool {
        let t = &self.ast.types;
        let (Some(pa), Some(pb)) = (t.proto(a), t.proto(b)) else { return false };
        let (ia, ib) = (pa.info(), pb.info());
        let (xa, xb) = (t.params(pa), t.params(pb));
        let (ra, rb) = (ia.ref_qual(), ib.ref_qual());
        xa.len() == xb.len()
            && xa.iter().zip(xb).all(|(x, y)| t.same_type(*x, *y))
            && ia.variadic() == ib.variadic()
            && ra != rb
            && (ra == RefQual::None || rb == RefQual::None)
    }

    /// [class.mem.general]/5: a member is not declared twice in the
    /// member-specification, except that a nested class can be declared
    /// and later defined. A class or enumeration redeclared without a
    /// definition in the class that declared it is diagnosed at the name
    /// with a note at the latest declaration (clang `ext_member_redeclared`
    /// in `ActOnTag`, an extension warning, `-Wredeclared-class-member`;
    /// not again at instantiation).
    pub(crate) fn check_member_tag_redeclared(&mut self, prev: DeclId, loc: Loc) {
        if !self.inst.stack.is_empty()
            || self.kind_of(self.ctx) != DeclKind::CXXRecord
            || self.ast.decls.get(prev).ctx() != Some(self.ctx)
        {
            return;
        }
        let latest = self.ast.latest(prev);
        let m = self.diag(DiagId::sema_member_redeclared_tag, loc);
        let m = self.note_prev(m, latest, false);
        self.emit(m);
    }

    /// [dcl.meaning.general]/2, [class.mem.general]/7: a class or
    /// enumeration declared in a member-specification with a qualified
    /// name. A qualifier naming the class itself is "extra qualification"
    /// (clang `err_member_extra_qualification`), any other is
    /// `err_member_qualification` (clang `diagnoseQualifiedDeclaration`
    /// from `ActOnTag` / `CheckClassTemplate`), at the name; the
    /// declaration is still made (clang's recovery). A friend never comes
    /// here, nor a member explicit specialization.
    pub(crate) fn check_qualified_member_tag(
        &mut self,
        qual: DeclId,
        name: kiln_source::symbol::Symbol,
        loc: Loc,
    ) {
        if !self.inst.stack.is_empty()
            || self.templ.member_spec.is_some()
            || self.kind_of(self.ctx) != DeclKind::CXXRecord
        {
            return;
        }
        let id = if self.ast.first(qual) == self.ast.first(self.ctx) {
            DiagId::sema_member_extra_qualification
        } else {
            DiagId::sema_member_qualification
        };
        let m = self.diag(id, loc).arg(self.ast.symbols().as_str(name).to_owned());
        self.emit(m);
    }
}
