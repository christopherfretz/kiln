//! Explicitly defaulted comparison operators: the declaration checks of
//! [class.compare.default] and the implicit `operator==` of a defaulted
//! `operator<=>` ([class.compare.default]/4; spec/sema-class.md §4.12).

use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind, DeclMisc, Defaulted, FnSpec, FriendKind};
use kiln_ast::names::{DeclName, DeclNameView, OverloadedOperator as OO};
use kiln_ast::types::{ExcSpec, FuncProto, Quals, RefQual, TypeView};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::CmpUsage;
use crate::Sema;
use crate::overload::oper::rewritten_of;
use crate::type_diff::DiffArgs as _;

/// The kind of a defaulted comparison (clang `DefaultedComparisonKind`);
/// the discriminant is the `{0:equality|three-way|equality|relational}`
/// diagnostic select.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CmpKind {
    /// `operator==`.
    Equal = 0,
    /// `operator<=>`.
    ThreeWay = 1,
    /// `operator!=`.
    NotEqual = 2,
    /// `operator<`, `>`, `<=`, `>=`.
    Relational = 3,
}

impl CmpKind {
    /// The diagnostic select.
    pub(crate) const fn index(self) -> u8 {
        self as u8
    }

    /// `==` and `<=>`, compared subobject-wise ([class.eq],
    /// [class.spaceship]); the others are secondary ([class.compare.secondary]).
    pub(crate) const fn is_primary(self) -> bool {
        matches!(self, CmpKind::Equal | CmpKind::ThreeWay)
    }
}

/// The comparison kind of operator `op`.
pub(crate) fn cmp_kind_of(op: OO) -> Option<CmpKind> {
    Some(match op {
        OO::EqualEqual => CmpKind::Equal,
        OO::Spaceship => CmpKind::ThreeWay,
        OO::ExclaimEqual => CmpKind::NotEqual,
        OO::Less | OO::Greater | OO::LessEqual | OO::GreaterEqual => CmpKind::Relational,
        _ => return None,
    })
}

impl Sema<'_, '_> {
    /// The overloaded operator function `f` is, if any.
    pub(crate) fn fn_operator(&self, f: DeclId) -> Option<OO> {
        match self.ast.names.view(self.ast.decls.get(f).name()) {
            DeclNameView::Operator(op) => Some(op),
            _ => None,
        }
    }

    /// The comparison kind of function `f` (an operator function).
    pub(crate) fn cmp_kind(&self, f: DeclId) -> Option<CmpKind> {
        self.fn_operator(f).and_then(cmp_kind_of)
    }

    /// Whether `f` is an explicitly defaulted comparison operator (or the
    /// implicit `operator==` of one).
    pub(crate) fn is_defaulted_cmp(&self, f: DeclId) -> bool {
        self.cmp_kind(f).is_some() && self.fn_spec(self.ast.first(f)).defaulted() != Defaulted::No
            || self.special.cmp.default_loc.contains_key(&self.ast.first(f))
    }

    /// The class a defaulted comparison `f` compares: its lexical class,
    /// else (an out-of-line member) its class.
    pub(crate) fn cmp_class(&self, f: DeclId) -> Option<DeclId> {
        let lex = self.ast.decls.lexical_ctx(f)?;
        if self.ast.decls.record(lex).is_some() {
            return Some(self.class_def(lex));
        }
        if self.kind_of(f) == DeclKind::CXXMethod {
            return self.ast.decls.get(f).ctx().map(|c| self.class_def(c));
        }
        // A non-member defaulted outside its class: the class of its
        // first parameter.
        let p = *self.param_decls(f).first()?;
        let t = self.ast.decls.get(p).ty()?;
        let t = self.non_reference(t);
        self.ast.types.as_record(t).map(|r| self.class_def(r))
    }

    /// `= default` on comparison operator `f` (clang `SetDeclDefaulted`
    /// for a comparison; [class.compare.default]/1). `loc` is the
    /// `default` keyword.
    pub(crate) fn defaulted_comparison(&mut self, f: DeclId, loc: Loc) {
        let Some(k) = self.cmp_kind(f) else { return };
        if self.ast.described_template(f).is_some() {
            self.error(DiagId::sema_defaulted_cmp_template, loc);
            return;
        }
        self.ast.decls.mark_function(f, FnSpec::new().with_defaulted(Defaulted::Explicit));
        let first = self.ast.first(f);
        let _ = self.special.cmp.default_loc.insert(first, (f, loc));
        if self.inst.stack.is_empty() {
            self.capture_cmp_lookups(f);
        }
        let lex = self.ast.decls.lexical_ctx(f);
        if self.classes.iter().any(|c| Some(c.record) == lex) {
            // Checked when the lexical class is complete.
            self.special.defaulted.push((f, loc));
            return;
        }
        if self.ast.decls.get(f).has(DeclFlags::DEPENDENT_CTX) {
            return;
        }
        if self.kind_of(f) == DeclKind::CXXMethod {
            let pattern = self.ast.instantiated_from(f).unwrap_or(f);
            // Defaulted on its first declaration: checked with its class.
            if self.fn_spec(self.ast.first(pattern)).defaulted() != Defaulted::No {
                return;
            }
        }
        let from = self.ctx;
        if self.check_defaulted_comparison(f, k, from) {
            let _ = self.special.cmp.invalid.insert(first);
        } else {
            self.define_defaulted_comparison(f, loc);
        }
    }

    /// The unqualified operator lookups a defaulted comparison's
    /// subobject comparisons use, captured at its `= default` (clang
    /// `lookupOperatorsForDefaultedFunction`).
    pub(crate) fn capture_cmp_lookups(&mut self, f: DeclId) {
        let first = self.ast.first(f);
        let Some(op) = self.fn_operator(f) else { return };
        let mut ops = vec![op];
        if let Some(r) = rewritten_of(op) {
            ops.push(r);
        }
        if op == OO::Spaceship {
            ops.extend([OO::ExclaimEqual, OO::Less, OO::EqualEqual]);
        }
        let mut out: Vec<(DeclId, DeclId)> = Vec::new();
        for o in ops {
            let found = self.lookup_operator_name(DeclName::operator(o));
            for (d, fd) in found.decls.iter().copied().zip(found.found.iter().copied()) {
                if !out.iter().any(|&(x, _)| x == d) {
                    out.push((d, fd));
                }
            }
        }
        let _ = self.special.cmp.lookups.insert(first, out);
    }

    /// The return type `f` was declared with (a deduced `auto` keeps its
    /// placeholder).
    pub(crate) fn declared_ret(&self, f: DeclId) -> QualType {
        let first = self.ast.first(f);
        if let Some(&r) = self.declared_returns.get(&first) {
            return r;
        }
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        self.ast.types.proto(ty).map_or(QualType::ERROR, FuncProto::ret)
    }

    /// Rebuilds the type of function `f` with prototype info `info`.
    fn set_fn_info(
        &mut self,
        f: DeclId,
        info: impl FnOnce(kiln_ast::types::FuncInfo) -> kiln_ast::types::FuncInfo,
    ) {
        let Some(fty) = self.ast.decls.get(f).ty() else { return };
        let TypeView::Function(p) = self.ast.types.view(fty.ty()) else { return };
        let params = self.ast.types.params(&p).to_vec();
        let exc = self.ast.types.exc_spec(&p);
        let nty = self.ast.types.function(p.ret(), &params, info(p.info()), exc);
        self.ast.decls.set_ty(f, nty);
    }

    /// clang `CheckExplicitlyDefaultedComparison`: the declaration checks
    /// of defaulted comparison `f` of kind `k` ([class.compare.default]/1,
    /// [class.eq]/1, [class.spaceship]/2, [class.compare.secondary]/1),
    /// then deletion, the deduced `<=>` return type, `constexpr` and the
    /// implicit exception specification. `from` is the context the
    /// subobject comparisons are resolved in. True if ill-formed.
    #[allow(clippy::too_many_lines, reason = "clang's checks, in its order")]
    pub(crate) fn check_defaulted_comparison(
        &mut self,
        f: DeclId,
        k: CmpKind,
        from: DeclId,
    ) -> bool {
        let floc = self.ast.decls.get(f).loc();
        let implicit = self.ast.decls.get(f).has(DeclFlags::IMPLICIT);
        let is_method = self.kind_of(f) == DeclKind::CXXMethod;
        let lex = self.ast.decls.lexical_ctx(f);
        let lex_is_record = lex.is_some_and(|l| self.ast.decls.record(l).is_some());
        let mut rd = lex.filter(|_| lex_is_record).map(|l| self.class_def(l));
        let params = self.param_decls(f);
        let explicit_obj = self.has_explicit_object(f);
        if is_method {
            let info = self.fn_info_of(f);
            if info.ref_qual() == RefQual::RValue {
                self.error(DiagId::sema_defaulted_cmp_ref_qual, floc);
                self.set_fn_info(f, |i| i.with_ref_qual(RefQual::None));
            }
            if rd.is_none() {
                rd = self.ast.decls.get(f).ctx().map(|c| self.class_def(c));
            }
            let (obj_const, lref, obj_param) = if explicit_obj {
                let p0 = params[0];
                let t = self.ast.decls.get(p0).ty().unwrap_or(QualType::ERROR);
                match self.ast.types.canonical_view(t) {
                    TypeView::LValueRef(i) => {
                        (self.ast.types.effective_quals(i).is_const(), true, Some((p0, i)))
                    }
                    TypeView::RValueRef(i) => {
                        (self.ast.types.effective_quals(i).is_const(), false, None)
                    }
                    _ => (self.ast.types.effective_quals(t).is_const(), false, None),
                }
            } else {
                (self.fn_info_of(f).this_quals().is_const(), false, None)
            };
            if !obj_const && (!explicit_obj || lref) {
                if !implicit {
                    let at = obj_param.map_or(floc, |(p, _)| self.ast.decls.get(p).loc());
                    let d = self.diag(DiagId::sema_defaulted_cmp_non_const, at).arg(k.index());
                    self.emit(d);
                }
                if let Some((p0, inner)) = obj_param {
                    let c = self.ast.types.qualify(inner, Quals::CONST);
                    let r = self.ast.types.lvalue_ref_to(c);
                    self.ast.decls.set_ty(p0, r);
                    // The function type's parameter follows.
                    if let Some(fty) = self.ast.decls.get(f).ty()
                        && let TypeView::Function(p) = self.ast.types.view(fty.ty())
                    {
                        let mut ps = self.ast.types.params(&p).to_vec();
                        let exc = self.ast.types.exc_spec(&p);
                        if let Some(x) = ps.first_mut() {
                            *x = r;
                        }
                        let nty = self.ast.types.function(p.ret(), &ps, p.info(), exc);
                        self.ast.decls.set_ty(f, nty);
                    }
                } else {
                    self.set_fn_info(f, |i| {
                        let q = i.this_quals();
                        i.with_this_quals(Quals::from_bits(q.bits() | Quals::CONST.bits()))
                    });
                }
            }
            if !explicit_obj && self.fn_info_of(f).this_quals().is_volatile() {
                self.error(DiagId::sema_defaulted_cmp_volatile, floc);
                self.set_fn_info(f, |i| {
                    let q = i.this_quals();
                    i.with_this_quals(Quals::from_bits(q.bits() & !Quals::VOLATILE.bits()))
                });
            }
        }
        let cmp_params: Vec<DeclId> =
            params.iter().copied().skip(usize::from(explicit_obj)).collect();
        if cmp_params.len() != if is_method { 1 } else { 2 } {
            // Already diagnosed as an ill-formed operator declaration.
            return true;
        }
        let mut known: Option<QualType> = None;
        for &p in &cmp_params {
            let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
            if let Some(kt) = known {
                if !self.ast.types.same_type(kt, pty) {
                    let d = self
                        .diag(DiagId::sema_defaulted_cmp_param_mismatch, floc)
                        .arg(k.index())
                        .diff_args(self, kt, pty);
                    self.emit(d);
                    return true;
                }
                continue;
            }
            let mut cty = pty;
            let mut ok = !is_method;
            let mut expected = rd.map(|r| self.class_type(r));
            if let TypeView::LValueRef(i) = self.ast.types.canonical_view(pty) {
                cty = i;
                expected = expected.map(|e| self.ast.types.qualify(e, Quals::CONST));
                ok = true;
            }
            if !ok {
            } else if let Some(r) = rd {
                let dependent = self.ast.decls.get(r).has(DeclFlags::DEPENDENT_CTX)
                    || self.is_dependent_type(self.class_type(r));
                if !dependent && !expected.is_some_and(|e| self.ast.types.same_type(cty, e)) {
                    ok = false;
                }
            } else if let Some(r) = self.ast.types.as_record(cty) {
                rd = Some(self.class_def(r));
            } else {
                ok = false;
            }
            if !ok {
                if !implicit {
                    let d = if let Some(r) = rd {
                        let plain = self.class_type(r);
                        let c = self.ast.types.qualify(plain, Quals::CONST);
                        let rt = self.ast.types.lvalue_ref_to(c);
                        self.diag(DiagId::sema_defaulted_cmp_param, floc)
                            .arg(k.index())
                            .arg(self.ty_arg(pty))
                            .arg(self.ty_arg(rt))
                            .arg(u8::from(!is_method))
                            .arg(self.ty_arg(plain))
                    } else {
                        self.diag(DiagId::sema_defaulted_cmp_param_unknown, floc)
                            .arg(k.index())
                            .arg(self.ty_arg(pty))
                    };
                    self.emit(d);
                }
                return true;
            }
            known = Some(pty);
        }
        let Some(rd) = rd else { return true };
        if !is_method && !lex_is_record {
            let rty = self.class_type(rd);
            if !self.is_complete(rty) {
                let d = self
                    .diag(DiagId::sema_defaulted_cmp_not_friend, floc)
                    .arg(k.index())
                    .arg(1u8)
                    .arg(self.decl_arg(rd));
                let d = self.incomplete_note(d, rty);
                self.emit(d);
                return true;
            }
            let first = self.ast.first(f);
            if !self.friends_of(rd).iter().any(|&x| self.ast.first(x) == first) {
                let d = self
                    .diag(DiagId::sema_defaulted_cmp_not_friend, floc)
                    .arg(k.index())
                    .arg(0u8)
                    .arg(self.decl_arg(rd));
                let d =
                    d.with_note(self.diag(DiagId::note_declared_at, self.ast.decls.get(rd).loc()));
                self.emit(d);
                return true;
            }
        }
        let ret = self.declared_ret(f);
        if k != CmpKind::ThreeWay
            && !self.is_dependent_type(ret)
            && !self.ast.types.same_type(ret, QualType::BOOL)
        {
            let d = self
                .diag(DiagId::sema_defaulted_cmp_return, floc)
                .arg(k.index())
                .arg(self.ty_arg(ret));
            self.emit(d);
            return true;
        }
        if k == CmpKind::ThreeWay && self.contains_deduced(ret) && !self.is_unconstrained_auto(ret)
        {
            let d = self
                .diag(DiagId::sema_defaulted_cmp_deduced_return, floc)
                .arg(k.index())
                .arg(self.ty_arg(ret));
            self.emit(d);
            return true;
        }
        if self.ast.decls.get(rd).has(DeclFlags::DEPENDENT_CTX)
            || self.is_dependent_type(self.class_type(rd))
        {
            return false;
        }
        let (info, _) = self.analyze_cmp(f, k, rd, from, false);
        let first = self.ast.first(f) == f;
        if !first {
            if info.deleted {
                let what = if implicit {
                    "the corresponding implicit 'operator==' for this defaulted 'operator<=>'"
                        .to_owned()
                } else {
                    let w = ["equality", "three-way", "equality", "relational"][k.index() as usize];
                    format!("this {w} comparison operator")
                };
                let mut d = self.diag(DiagId::sema_defaulted_cmp_non_first_deletes, floc).arg(what);
                for n in self.analyze_cmp(f, k, rd, from, true).1 {
                    d = d.with_note(n);
                }
                self.emit(d);
                return true;
            }
            if lex_is_record {
                let d =
                    self.diag(DiagId::sema_defaulted_cmp_non_first_in_class, floc).arg(k.index());
                let prev = self.ast.prev(f).unwrap_or(f);
                let d = self.note_prev(d, prev, false);
                self.emit(d);
                return true;
            }
        }
        if info.deleted {
            self.ast.decls.mark_function(f, FnSpec::new().with_deleted(true));
            if self.inst.stack.is_empty() && !implicit {
                let mut d = self.diag(DiagId::sema_defaulted_cmp_deleted, floc).arg(k.index());
                for n in self.analyze_cmp(f, k, rd, from, true).1 {
                    d = d.with_note(n);
                }
                if let Some(&(_, l)) = self.special.cmp.default_loc.get(&self.ast.first(f)) {
                    d = d.with_note(self.diag(DiagId::note_replace_default_with_delete, l));
                }
                self.emit(d);
            }
            return false;
        }
        if k == CmpKind::ThreeWay && self.is_undeduced_auto(ret) && !self.has_deduced_return(f) {
            // todo: the begin location of the declared return type (clang
            // `getReturnTypeSourceRange().getBegin()`), not recorded.
            let Some(cat) = self.cmp_category_type(info.category, floc, CmpUsage::Defaulted) else {
                return true;
            };
            self.set_deduced_return(f, cat);
        }
        let ck = self.fn_spec(f).constexpr_kind();
        if ck != ConstexprKind::None && !info.constexpr && self.std < kiln_lex::Std::Cxx23 {
            // [dcl.fct.def.default]/3 before C++23 (P2448): a `constexpr`
            // or `consteval` defaulted comparison must be
            // constexpr-compatible (clang
            // `err_defaulted_comparison_constexpr_mismatch`, at the
            // declaration's begin).
            let at = self
                .constexpr_begin_locs
                .get(&f)
                .or_else(|| {
                    self.ast.instantiated_from(f).and_then(|p| self.constexpr_begin_locs.get(&p))
                })
                .copied()
                .unwrap_or(floc);
            // The implicit `operator==` reports at its `operator<=>`, under
            // its own id (one error per id and location, spec/diag.md).
            let consteval = u8::from(ck == ConstexprKind::Consteval);
            let mut d = if implicit {
                self.diag(DiagId::sema_defaulted_cmp_constexpr_mismatch_implicit, at).arg(consteval)
            } else {
                self.diag(DiagId::sema_defaulted_cmp_constexpr_mismatch, at)
                    .arg(k.index())
                    .arg(consteval)
            };
            for n in self.explain_cmp_not_constexpr(f, k, rd, from) {
                d = d.with_note(n);
            }
            self.emit(d);
        }
        if first && ck == ConstexprKind::None && info.constexpr {
            self.ast
                .decls
                .mark_function(f, FnSpec::new().with_constexpr_kind(ConstexprKind::Constexpr));
        }
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        if self.written_exc_spec(fty) == ExcSpec::None {
            let t = self.with_exc_spec(fty, ExcSpec::Deferred(f));
            self.ast.decls.set_ty(f, t);
        }
        false
    }

    /// The prototype info of function `f`.
    pub(crate) fn fn_info_of(&self, f: DeclId) -> kiln_ast::types::FuncInfo {
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        self.ast.types.proto(ty).map(FuncProto::info).unwrap_or_default()
    }

    /// Whether the return type of `f` is no longer a placeholder.
    fn has_deduced_return(&self, f: DeclId) -> bool {
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        self.ast.types.proto(ty).is_some_and(|p| !self.contains_deduced(p.ret()))
    }

    /// Whether `q` contains a placeholder type (clang
    /// `getContainedDeducedType`, for the declarator forms kiln accepts).
    pub(crate) fn contains_deduced(&self, q: QualType) -> bool {
        let mut cur = q;
        for _ in 0..256 {
            match self.ast.types.canonical_view(cur) {
                TypeView::Auto { deduced: None, .. }
                | TypeView::DeducedTemplateSpecialization { .. } => return true,
                TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => cur = p,
                _ => return false,
            }
        }
        false
    }

    /// Whether `q` is exactly an unqualified, unconstrained `auto`.
    fn is_unconstrained_auto(&self, q: QualType) -> bool {
        q.quals_bits() == 0
            && matches!(
                self.ast.types.view(self.ast.types.canonical(q).ty()),
                TypeView::Auto {
                    keyword: kiln_ast::types::AutoKeyword::Auto,
                    deduced: None,
                    concept: None,
                    ..
                }
            )
            && self.ast.types.canonical(q).quals_bits() == 0
    }

    /// clang `findImplicitlyDeclaredEqualityComparisons` +
    /// `DeclareImplicitEqualityComparison` ([class.compare.default]/4): at
    /// the `}` of class `r`, if it declares no `operator==`, an implicit
    /// `operator==` for each explicitly defaulted `operator<=>`, with the
    /// same parameters, access and `constexpr`ness and return type `bool`.
    /// A friend's goes to `friends` (the class's friend list, not yet set).
    pub(crate) fn declare_implicit_equality(&mut self, r: DeclId, friends: &mut Vec<DeclId>) {
        let eq = DeclName::operator(OO::EqualEqual);
        let ss = DeclName::operator(OO::Spaceship);
        let members: Vec<DeclId> = self.ast.decls.members(r).collect();
        if members.iter().any(|&m| self.ast.decls.get(m).name() == eq) {
            return;
        }
        if friends.iter().any(|&f| {
            self.ast.decls.get(f).kind().is_function() && self.ast.decls.get(f).name() == eq
        }) {
            return;
        }
        let is_candidate = |s: &Self, f: DeclId| {
            s.ast.decls.get(f).name() == ss
                && !s.ast.decls.get(f).has(DeclFlags::INVALID)
                && s.ast.described_template(f).is_none()
                && s.fn_spec(f).defaulted() == Defaulted::Explicit
                && s.special.cmp.default_loc.get(&s.ast.first(f)).is_some_and(|&(d, _)| d == f)
        };
        let mut spaceships: Vec<DeclId> = Vec::new();
        for &fr in friends.iter() {
            let latest = self
                .ast
                .redecls_of(fr)
                .find(|&d| self.ast.decls.lexical_ctx(d).is_some_and(|l| self.class_def(l) == r));
            if let Some(d) = latest
                && is_candidate(self, d)
            {
                spaceships.push(d);
            }
        }
        for &m in &members {
            if self.ast.decls.get(m).kind() == DeclKind::CXXMethod && is_candidate(self, m) {
                spaceships.push(m);
            }
        }
        for s in spaceships {
            let eq_fn = self.declare_implicit_equality_for(r, s);
            if self.ast.decls.get(eq_fn).misc().friend() != FriendKind::None {
                let loc = self.ast.decls.get(s).loc();
                let _ = self.builder().friend(r, loc, kiln_ast::decl::FriendTarget::Decl(eq_fn));
                friends.push(eq_fn);
            }
        }
    }

    fn declare_implicit_equality_for(&mut self, r: DeclId, s: DeclId) -> DeclId {
        let eq = DeclName::operator(OO::EqualEqual);
        let n = self.ast.decls.get(s);
        let loc = n.loc();
        let access = n.misc().access();
        let sctx = n.ctx().unwrap_or(self.ast.tu);
        let sty = n.ty().unwrap_or(QualType::ERROR);
        let (params, info, exc) = match self.ast.types.view(sty.ty()) {
            TypeView::Function(p) => {
                (self.ast.types.params(&p).to_vec(), p.info(), self.ast.types.exc_spec(&p))
            }
            _ => (Vec::new(), kiln_ast::types::FuncInfo::new(), ExcSpec::None),
        };
        let exc = match exc {
            ExcSpec::Deferred(_) => ExcSpec::None,
            e => e,
        };
        let fty =
            self.ast.types.function(QualType::BOOL, &params, info.with_trailing_return(false), exc);
        let src_spec = self.fn_spec(s);
        let spec = FnSpec::new()
            .with_implicit_inline(true)
            .with_constexpr_kind(src_spec.constexpr_kind())
            .with_defaulted(Defaulted::Explicit)
            .with_explicit_object(src_spec.has_explicit_object());
        let f = if self.ast.decls.get(s).kind() == DeclKind::CXXMethod {
            let h = DeclHead::new(r, eq, loc).flags(DeclFlags::IMPLICIT).access(access);
            self.builder().append_implicit_member(h, DeclKind::CXXMethod, fty, spec)
        } else {
            let h = DeclHead::new(sctx, eq, loc)
                .lexical(r)
                .misc(DeclMisc::default().with_friend(FriendKind::Undeclared))
                .flags(DeclFlags::IMPLICIT);
            self.builder().function(h, DeclKind::Function, fty, spec, None)
        };
        let sparams = self.param_decls(s);
        let ps: Vec<DeclId> = sparams
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                let pn = self.ast.decls.get(p);
                let (name, ploc, pty) = (pn.name(), pn.loc(), pn.ty().unwrap_or(QualType::ERROR));
                let i = u16::try_from(i).unwrap_or(u16::MAX);
                self.builder().param(f, name, ploc, pty, i, 0)
            })
            .collect();
        if !ps.is_empty() {
            let _ = self.builder().set_params(f, &ps);
        }
        if let Some(&l) = self.constexpr_begin_locs.get(&s) {
            let _ = self.constexpr_begin_locs.insert(f, l);
        }
        let sfirst = self.ast.first(s);
        if let Some(&(_, dl)) = self.special.cmp.default_loc.get(&sfirst) {
            let _ = self.special.cmp.default_loc.insert(f, (f, dl));
            self.special.defaulted.push((f, dl));
        }
        if let Some(l) = self.special.cmp.lookups.get(&sfirst).cloned() {
            let _ = self.special.cmp.lookups.insert(f, l);
        }
        f
    }
}
