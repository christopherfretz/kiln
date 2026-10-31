//! Structured binding declarations ([dcl.struct.bind], spec/sema.md
//! §4.10): the decomposition declarator, the unnamed variable `e` that
//! holds the initializer, and the bindings of its array elements,
//! tuple-like elements (`get<i>`) or data members (clang's
//! `ActOnDecompositionDeclarator` / `CheckCompleteDecompositionDeclaration`).

use kiln_ast::decl::{DeclFlags, DeclKind, VarSpec, VarStorage};
use kiln_ast::expr::{CastKind, ExprKind, Radix, ValueCategory};
use kiln_ast::names::DeclName;
use kiln_ast::types::{Nns, Quals, TagKeyword, TemplateArg, TypeView};
use kiln_ast::{DeclHead, DeclId, ExprId, NnsId, QualType, TemplateParamListId};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;
use crate::class::InitEntity;
use crate::conv;
use crate::declspec::{Chunk, DeclSpec, Declarator, StorageSpec, TypeSpecKind};
use crate::init::Initializer;
use crate::lookup::LookupKind;
use crate::templ::actions::TemplateIdRef;

/// The outcome of clang's `isTupleLike`.
enum TupleLike {
    /// `std::tuple_size<E>` is not a complete class with a `value`.
    No,
    /// `std::tuple_size<E>::value`.
    Yes(u64),
    /// Committed to the tuple interpretation, but `value` is unusable
    /// (diagnosed).
    Error,
}

impl Sema<'_, '_> {
    /// The declarator `auto [a, b]` of a simple-declaration, condition or
    /// range-for loop variable ([dcl.pre]/6, [dcl.struct.bind]/1): checks
    /// the specifiers and declared type, declares the bindings, then the
    /// unnamed variable they decompose (located at `[`).
    pub(crate) fn act_on_decomposition_declarator(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        head: Option<TemplateParamListId>,
    ) -> Option<DeclId> {
        let dc = d.decomp.clone()?;
        if let Some(h) = head {
            let l = self.ast.templates.param_list(h).template_loc;
            self.error(DiagId::sema_decomp_decl_template, l);
            self.templ.cur = None;
            return None;
        }
        // [dcl.pre]/6: only static, thread_local, auto and cv.
        let mut bad: Vec<(&str, Loc)> = Vec::new();
        if let Some((s, l)) = ds.storage
            && s != StorageSpec::Static
        {
            bad.push((s.spelling(), l));
        }
        for (kw, at) in
            [("constexpr", ds.constexpr), ("consteval", ds.consteval), ("constinit", ds.constinit)]
        {
            if let Some(l) = at {
                bad.push((kw, l));
            }
        }
        if let Some(l) = ds.inline {
            bad.push(("inline", l));
        }
        if let Some(&(first, l)) = bad.first() {
            let arg = if bad.len() == 1 {
                format!("'{first}'")
            } else {
                let all: Vec<&str> = bad.iter().map(|b| b.0).collect();
                format!("with '{}' specifiers", all.join(" "))
            };
            let msg = self.diag(DiagId::sema_decomp_decl_spec, l).arg(arg);
            self.emit(msg);
        }
        if ds.is_typedef() {
            return None;
        }
        let mut invalid = ds.invalid || d.invalid || self.is_error_type(ty);
        let one_ref = match d.chunks.as_slice() {
            [] => true,
            [Chunk::LRef { .. } | Chunk::RRef { .. }] => true,
            _ => false,
        };
        if ds.base != TypeSpecKind::Auto || d.grouping || !one_ref {
            let id = if d.grouping {
                DiagId::sema_decomp_decl_parens
            } else {
                DiagId::sema_decomp_decl_type
            };
            let msg = self.diag(id, dc.lbracket).arg(self.ty_arg(ty));
            self.emit(msg);
            if self.ast.types.is_function(ty) {
                invalid = true;
            }
        }
        if ds.auto_constraint.is_some() {
            let at = if ds.constraint_loc.is_valid() { ds.constraint_loc } else { ds.loc };
            self.error(DiagId::sema_decomp_decl_constraint, at);
        }
        let v = self.declare_decomposition(ds, ty, d, &dc, invalid);
        if self.in_template() {
            self.decomp_written.insert(v, ty);
        }
        Some(v)
    }

    /// Declares the bindings `dc` and then the unnamed variable of type
    /// `ty` they decompose (located at `[`); `invalid` marks it so.
    pub(crate) fn declare_decomposition(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        dc: &crate::declspec::Decomposition,
        mut invalid: bool,
    ) -> DeclId {
        let (ctx, lexical) = self.decl_contexts(d);
        let mut bindings = Vec::with_capacity(dc.ids.len());
        let mut pack_reported = false;
        for b in &dc.ids {
            if let Some(e) = b.ellipsis
                && !pack_reported
            {
                // P1061 (C++26): structured binding packs.
                self.unsupported("structured binding packs", e);
                pack_reported = true;
                invalid = true;
            }
            self.check_template_param_shadow(b.name, b.loc);
            if let Some(&p) = self.prev_candidates(b.name, None).last() {
                let name = self.ast.symbols().as_str(b.name).to_owned();
                let msg = self.diag(DiagId::sema_redefinition, b.loc).arg(name);
                let msg = self.note_prev(msg, p, true);
                self.emit(msg);
            }
            let mut h = DeclHead::new(ctx, DeclName::ident(b.name), b.loc);
            h.lexical = Some(lexical);
            let bd = self.builder().binding(h);
            if !b.attrs.is_empty() {
                self.check_decl_attrs(bd, &b.attrs);
                let _ = self.builder().attach_attrs(bd, &b.attrs);
            }
            self.register_local(b.name, bd);
            bindings.push(bd);
        }
        let spec = self.var_spec(ds, d);
        // [dcl.constexpr]/6: a constexpr object is const (the specifier
        // was rejected above but is still honored, as in clang).
        let ty = if ds.constexpr.is_some() && !self.ast.types.is_reference(ty) {
            self.ast.types.qualify(ty, Quals::CONST)
        } else {
            ty
        };
        let mut h = DeclHead::new(ctx, DeclName::EMPTY, dc.lbracket);
        h.lexical = Some(lexical);
        self.mark_pattern_next();
        let v = self.builder().var(h, ty, spec, None);
        let _ = self.builder().set_decomposition(v, &bindings);
        self.lambda_mangle.var = Some(v);
        if invalid {
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
        let _ = self.finish_template_pattern(v);
        let mut attrs = ds.attrs.clone();
        attrs.extend(d.attrs.iter().copied());
        if !attrs.is_empty() {
            self.check_decl_attrs(v, &attrs);
            let _ = self.builder().attach_attrs(v, &attrs);
        }
        v
    }

    /// A decomposition declaration that is not alone in its
    /// declaration group ([dcl.pre]/6 grammar; clang reports it at the
    /// first decomposition's `[` and invalidates it).
    pub fn act_on_decomposition_not_alone(&mut self, lbracket: Loc, v: Option<DeclId>) {
        self.error(DiagId::sema_decomp_decl_not_alone, lbracket);
        if let Some(v) = v {
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
    }

    /// The bindings of decomposition `v`.
    pub(crate) fn decomp_binding_list(&self, v: DeclId) -> Vec<DeclId> {
        match self.ast.decls.decomp_bindings(v) {
            Some(l) => self.ast.lists.iter::<DeclId>(l).collect(),
            None => Vec::new(),
        }
    }

    /// `[a, b]`: a decomposition as clang names it in diagnostics.
    pub(crate) fn decomp_name(&self, v: DeclId) -> String {
        let names: Vec<String> =
            self.decomp_binding_list(v).into_iter().map(|b| self.decl_name(b)).collect();
        format!("[{}]", names.join(", "))
    }

    /// `auto [a, b] = arr;` ([dcl.struct.bind]/1): a placeholder `cv auto`
    /// (no reference) deduces `cv A` from an initializer of array type
    /// `A`, which then initializes `e` element-wise. The deduced type, or
    /// `None` when the general deduction applies.
    pub(crate) fn decomp_array_type(
        &mut self,
        declared: QualType,
        init: &Initializer,
        direct: bool,
    ) -> Option<QualType> {
        let t = &self.ast.types;
        if t.is_reference(declared) || !self.contains_undeduced_auto(declared) {
            return None;
        }
        let e = match init {
            Initializer::Expr(e) => *e,
            Initializer::Paren(es, ..) if es.len() == 1 => es[0],
            Initializer::List(l) if direct && l.items.len() == 1 => match l.items[0].value {
                crate::init::InitValue::Expr(e) if l.items[0].designator.is_empty() => e,
                _ => return None,
            },
            _ => return None,
        };
        let ety = self.ast.exprs.get(e).ty();
        if !t.is_array(ety) || t.array_size(ety).is_none() {
            return None;
        }
        let q = t.effective_quals(declared);
        Some(if q.is_empty() { ety } else { self.ast.types.qualify(ety, q) })
    }

    /// The initialization of array decomposition `v` of type `ty` from
    /// the array `init` (an `ArrayInitLoopExpr`, as clang's
    /// `TryArrayCopy` for a decomposition).
    pub(crate) fn init_decomposition_array(&mut self, v: DeclId, ty: QualType, init: &Initializer) {
        let e = match init {
            Initializer::Expr(e) => *e,
            Initializer::Paren(es, ..) => es[0],
            Initializer::List(l) => match l.items[0].value {
                crate::init::InitValue::Expr(e) => e,
                _ => return,
            },
        };
        self.ast.decls.set_ty(v, ty);
        let loc = self.eloc(e);
        let init = match self.init_subobject_from(ty, e, InitEntity::Var, loc) {
            Ok(x) => x,
            Err(()) => self.recover_err(&[e], loc),
        };
        let init = self.act_on_full_expr(init);
        let _ = self.builder().set_var_init(v, init);
    }

    /// A decomposition declared without an initializer
    /// ([dcl.pre]/6 grammar).
    pub(crate) fn decomposition_requires_init(&mut self, v: DeclId) {
        let loc = self.ast.decls.get(v).loc();
        let msg = self.diag(DiagId::sema_decomp_decl_requires_init, loc).arg(self.decomp_name(v));
        self.emit(msg);
        self.ast.decls.set_flags(v, DeclFlags::INVALID);
        self.invalidate_bindings(v);
    }

    pub(crate) fn invalidate_bindings(&mut self, v: DeclId) {
        for b in self.decomp_binding_list(v) {
            if self.ast.decls.get(b).ty().is_none() {
                self.ast.decls.set_ty(b, QualType::ERROR);
            }
            self.ast.decls.set_flags(b, DeclFlags::INVALID);
        }
    }

    /// clang `CheckCompleteDecompositionDeclaration`: once `v` is
    /// initialized, gives each binding its type and binding expression
    /// ([dcl.struct.bind]/3-5).
    pub(crate) fn complete_decomposition(&mut self, v: DeclId) {
        let bindings = self.decomp_binding_list(v);
        let vn = *self.ast.decls.get(v);
        let vty = vn.ty().unwrap_or(QualType::ERROR);
        let init = self.ast.decls.var(v).and_then(|x| x.init);
        let undeduced = self.contains_undeduced_auto(vty);
        if vn.has(DeclFlags::INVALID)
            || self.is_error_type(vty)
            || init.is_some_and(|i| self.is_err_expr(i))
            || undeduced && !self.in_template()
        {
            self.invalidate_bindings(v);
            return;
        }
        if self.ast.types.is_dependent(vty) || undeduced {
            // Decomposed at instantiation.
            for b in bindings {
                self.ast.decls.set_ty(b, QualType::DEPENDENT);
            }
            return;
        }
        let dty = self.non_reference(vty);
        let loc = vn.loc();
        let ok = if self.ast.types.is_array(dty) && self.ast.types.array_size(dty).is_some() {
            self.decompose_array(v, dty, &bindings)
        } else {
            match self.tuple_like(loc, dty) {
                TupleLike::Error => false,
                TupleLike::Yes(n) => self.decompose_tuple(v, vty, dty, n, &bindings),
                TupleLike::No => match self.ast.types.as_record(dty) {
                    Some(r) if !self.is_union(r) => self.decompose_members(v, dty, r, &bindings),
                    r => {
                        let msg = self
                            .diag(DiagId::sema_decomp_decl_unbindable_type, loc)
                            .arg(u8::from(r.is_none()))
                            .arg(self.ty_arg(dty));
                        self.emit(msg);
                        false
                    }
                },
            }
        };
        if !ok {
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
            self.invalidate_bindings(v);
        }
    }

    /// `type 'P' decomposes into 2 elements, but 3 names were provided`.
    fn wrong_number_of_bindings(&mut self, v: DeclId, dty: QualType, names: usize, elems: u64) {
        let loc = self.ast.decls.get(v).loc();
        let elements = format!("{elems} element{}", if elems == 1 { "" } else { "s" });
        let provided = if (names as u64) > elems {
            match names {
                0 => "no names were".to_owned(),
                1 => "one name was".to_owned(),
                n => format!("{n} names were"),
            }
        } else if names == 1 {
            "only 1 name was".to_owned()
        } else {
            format!("only {names} names were")
        };
        let msg = self
            .diag(DiagId::sema_decomp_decl_wrong_number_bindings, loc)
            .arg(self.ty_arg(dty))
            .arg(elements)
            .arg(provided);
        self.emit(msg);
    }

    /// A reference to the decomposed object `e` at `loc`: an lvalue of
    /// its (non-reference) type (clang `BuildDeclRefExpr(Src, DecompType,
    /// VK_LValue)`).
    fn decomp_ref(&mut self, v: DeclId, loc: Loc) -> ExprId {
        self.build_decl_ref(v, loc)
    }

    /// [dcl.struct.bind]/3: `e[i]`.
    fn decompose_array(&mut self, v: DeclId, dty: QualType, bindings: &[DeclId]) -> bool {
        let n = self.ast.types.array_size(dty).unwrap_or(0);
        if bindings.len() as u64 != n {
            self.wrong_number_of_bindings(v, dty, bindings.len(), n);
            return false;
        }
        let q = self.ast.types.effective_quals(dty);
        let elem = self.ast.types.element(dty).unwrap_or(QualType::ERROR);
        let elem = if q.is_empty() { elem } else { self.ast.types.qualify(elem, q) };
        for (i, &b) in bindings.iter().enumerate() {
            let loc = self.ast.decls.get(b).loc();
            let base = self.decomp_ref(v, loc);
            let pty = conv::array_decayed(self, self.ety(base));
            let decayed = conv::raw_cast(
                self,
                CastKind::ArrayToPointerDecay,
                base,
                pty,
                ValueCategory::PRValue,
                None,
            );
            let idx = self.builder().int_literal(QualType::INT, i as u64, Radix::Decimal, loc);
            let sub = self.builder().subscript(decayed, idx, elem, loc);
            let sub = self.act_on_full_expr(sub);
            self.ast.decls.set_ty(b, elem);
            let _ = self.ast.decls.set_binding(b, sub, None);
        }
        true
    }

    /// `std::NAME` if it is a class template.
    fn std_class_template(&self, name: &str) -> Option<(DeclId, DeclId)> {
        let std = self.intern("std");
        let ns = self.lookup_qualified(self.ast.tu, std, LookupKind::Nested);
        let ns = ns.decls.iter().copied().find(|&d| self.kind_of(d) == DeclKind::Namespace)?;
        let n = self.intern(name);
        let found = self.lookup_qualified(ns, n, LookupKind::Ordinary);
        let t =
            found.decls.iter().copied().find(|&d| self.kind_of(d) == DeclKind::ClassTemplate)?;
        Some((self.ast.first(ns), t))
    }

    /// The complete class `std::TRAIT<args>` (clang
    /// `lookupStdTypeTraitMember` up to the member lookup), if any.
    fn std_trait(&mut self, trait_name: &str, args: &[TemplateArg], loc: Loc) -> Option<DeclId> {
        self.std_trait_ty(trait_name, args, loc).map(|x| x.0)
    }

    /// [`Self::std_trait`], with the specialization's type as written
    /// (`std::NAME<args>::`, a nested-name-specifier).
    fn std_trait_ty(
        &mut self,
        trait_name: &str,
        args: &[TemplateArg],
        loc: Loc,
    ) -> Option<(DeclId, NnsId)> {
        let (ns, tmpl) = self.std_class_template(trait_name)?;
        let name = self.template_name_for(tmpl);
        let t = match self.act_template_id(name, args, loc, loc) {
            Ok(TemplateIdRef::Type(t)) => t,
            _ => return None,
        };
        if !self.require_complete_silent(t, loc) {
            return None;
        }
        let r = self.ast.types.as_record(t)?;
        let def = self.ast.definition(r)?;
        let std = self.ast.types.nns(Nns::Namespace { prefix: None, ns });
        let nns = self.ast.types.nns(Nns::Type { prefix: Some(std), ty: t, template_kw: false });
        Some((def, nns))
    }

    /// clang `isTupleLike`: `std::tuple_size<E>::value`.
    fn tuple_like(&mut self, loc: Loc, dty: QualType) -> TupleLike {
        let Some(c) = self.std_trait("tuple_size", &[TemplateArg::ty(dty)], loc) else {
            return TupleLike::No;
        };
        let value = self.intern("value");
        let found = self.lookup_in_class(c, value, LookupKind::Ordinary);
        let Some(&d) = found.decls.first() else { return TupleLike::No };
        let e = self.build_decl_ref(d, loc);
        let n = if self.is_err_expr(e) {
            None
        } else {
            self.eval_int(e, EvalContext::new(EvalKind::Other, loc)).ok()
        };
        match n.and_then(|n| u64::try_from(n).ok()) {
            Some(n) => TupleLike::Yes(n),
            None => {
                let msg = self
                    .diag(DiagId::sema_decomp_decl_tuple_size_not_constant, loc)
                    .arg(self.ty_str(dty));
                self.emit(msg);
                TupleLike::Error
            }
        }
    }

    /// The integral template argument `i` of type `std::size_t` (clang
    /// builds `get<i>` and `tuple_element<i, E>` with it).
    fn size_t_arg(&mut self, i: u64) -> TemplateArg {
        let size_t = self.size_type();
        let canon = self.ast.types.canonical(size_t);
        let v = self.int_const(canon, i128::from(i));
        TemplateArg::value(canon, v)
    }

    /// `std::tuple_element<i, E>::type` (clang `getTupleLikeElementType`).
    fn tuple_element_type(&mut self, loc: Loc, i: u64, dty: QualType) -> Option<QualType> {
        let args = [self.size_t_arg(i), TemplateArg::ty(dty)];
        let mut qual = None;
        let found = self.std_trait_ty("tuple_element", &args, loc).map(|(c, nns)| {
            qual = Some(nns);
            let ty = self.intern("type");
            self.lookup_in_class(c, ty, LookupKind::Ordinary)
        });
        let td = found.as_ref().and_then(|f| {
            f.decls.first().copied().filter(|&d| crate::lookup::is_type_kind(self.kind_of(d)))
        });
        let Some(td) = td else {
            let msg = self
                .diag(DiagId::sema_decomp_decl_tuple_element_not_specialized, loc)
                .arg(format!("{i}, {}", self.ty_str(dty)));
            self.emit(msg);
            return None;
        };
        let named = match self.kind_of(td) {
            DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::TemplateTypeParm => {
                self.ast.decls.get(td).ty().unwrap_or(QualType::ERROR)
            }
            _ => self.tag_type(td),
        };
        // clang prints the member's scope: `std::tuple_element<0, T>::type`.
        Some(self.ast.types.elaborated(TagKeyword::None, qual, named))
    }

    /// [dcl.struct.bind]/4: each binding refers to a variable of type
    /// "reference to `std::tuple_element<i, E>::type`" initialized with
    /// `e.get<i>()` or `get<i>(e)`.
    fn decompose_tuple(
        &mut self,
        v: DeclId,
        vty: QualType,
        dty: QualType,
        n: u64,
        bindings: &[DeclId],
    ) -> bool {
        if bindings.len() as u64 != n {
            self.wrong_number_of_bindings(v, dty, bindings.len(), n);
            return false;
        }
        if bindings.is_empty() {
            return true;
        }
        let vloc = self.ast.decls.get(v).loc();
        let get = self.intern("get");
        // The member `get` is used if lookup in E finds a function
        // template whose first template parameter is a non-type one.
        let mut member_get = false;
        if self.require_complete_silent(dty, vloc)
            && let Some(r) = self.ast.types.as_record(dty)
            && let Some(def) = self.ast.definition(r)
        {
            let found = self.lookup_in_class(def, get, LookupKind::Ordinary);
            member_get = found.decls.iter().any(|&d| {
                let d = self.ast.strip_shadow(d);
                self.kind_of(d) == DeclKind::FunctionTemplate
                    && self.first_template_param_is_nontype(d)
            });
        }
        let lref = matches!(self.ast.types.canonical_view(vty), TypeView::LValueRef(_));
        let (ctx, lexical) = {
            let n = self.ast.decls.get(v);
            (n.ctx().unwrap_or(self.ast.tu), self.ast.decls.lexical_ctx(v))
        };
        let vspec = self.ast.decls.var(v).map(|x| x.spec).unwrap_or_default();
        for (i, &b) in bindings.iter().enumerate() {
            let loc = self.ast.decls.get(b).loc();
            let mut e = self.decomp_ref(v, loc);
            if !lref {
                let t = self.ety(e);
                e = self.builder().implicit_cast(CastKind::NoOp, e, t, ValueCategory::XValue);
            }
            let idx = self.size_t_arg(i as u64);
            let targs = self.ast.types.arg_list(&[idx]);
            let call = if member_get {
                let m = self.act_on_member(e, get, loc, loc, false);
                if self.is_err_expr(m) {
                    return false;
                }
                let m = self.act_on_member_template_args(m, targs);
                self.act_on_call(m, &[], loc, loc)
            } else {
                let ule = self.builder().unresolved_lookup(get, &[], true, None, Some(targs), loc);
                let call = self.act_on_call(ule, &[e], loc, loc);
                // The lookup is argument-dependent only ([dcl.struct.bind]/4),
                // so the function is clang's `IsADLCandidate` (dumped `adl`).
                let mut c = call;
                while self.ast.exprs.get(c).kind() == ExprKind::CXXBindTemporaryExpr {
                    c = self.ast.exprs.operand(c);
                }
                if self.ast.exprs.get(c).kind() == ExprKind::CallExpr {
                    self.ast.exprs.set_call_adl(c);
                }
                call
            };
            if self.is_err_expr(call) {
                return false;
            }
            let Some(t) = self.tuple_element_type(loc, i as u64, dty) else { return false };
            let is_lvalue = self.ast.exprs.get(call).value_category() == ValueCategory::LValue;
            let ref_ty = if is_lvalue {
                self.ast.types.lvalue_ref_to(t)
            } else {
                self.ast.types.rvalue_ref_to(t)
            };
            let mut h = DeclHead::new(ctx, self.ast.decls.get(b).name(), loc);
            h.lexical = lexical;
            h.flags = DeclFlags::IMPLICIT;
            let spec = VarSpec::new()
                .with_storage(if vspec.storage() == VarStorage::Static {
                    VarStorage::Static
                } else {
                    VarStorage::None
                })
                .with_tls(vspec.tls());
            let hv = self.builder().holding_var(h, ref_ty, spec);
            let init = self.perform_init(ref_ty, Initializer::Expr(call), loc, false);
            let init = self.act_on_full_expr(init);
            let _ = self.builder().set_var_init(hv, init);
            if self.is_err_expr(init) {
                return false;
            }
            self.extend_temporaries(hv, init);
            let r = self.build_decl_ref(hv, loc);
            self.ast.decls.set_ty(b, t);
            let _ = self.ast.decls.set_binding(b, r, Some(hv));
        }
        true
    }

    /// Whether function template `d`'s first template parameter is a
    /// non-type parameter.
    fn first_template_param_is_nontype(&self, d: DeclId) -> bool {
        self.template_params(d)
            .first()
            .is_some_and(|&p| self.kind_of(p) == DeclKind::NonTypeTemplateParm)
    }

    /// Whether class `c` (a definition) declares a non-static data member.
    fn has_direct_fields(&self, c: DeclId) -> bool {
        let Some(def) = self.ast.definition(c) else { return false };
        self.ast.members(def).any(|m| self.kind_of(m) == DeclKind::Field)
    }

    /// The bases (at any depth, not looking past a match) of class `c`
    /// with direct fields: (base type, base class).
    fn bases_with_fields(&self, c: DeclId, out: &mut Vec<(QualType, DeclId)>) {
        let Some(def) = self.ast.definition(c) else { return };
        let bases: Vec<QualType> =
            self.ast.decls.bases(def, &self.ast.lists).map(|b| b.ty).collect();
        for t in bases {
            let Some(r) = self.ast.types.as_record(t) else { continue };
            if self.has_direct_fields(r) {
                out.push((t, self.ast.first(r)));
            } else {
                self.bases_with_fields(r, out);
            }
        }
    }

    /// clang `findDecomposableBaseClass`: the class whose direct members
    /// are the bindings ([dcl.struct.bind]/5), `None` after a diagnostic.
    fn decomposable_class(&mut self, rd: DeclId, loc: Loc) -> Option<DeclId> {
        let rty = self.tag_type(rd);
        let with_fields = if self.has_direct_fields(rd) {
            rd
        } else {
            let mut found = Vec::new();
            self.bases_with_fields(rd, &mut found);
            let Some(&(bt, bc)) = found.first() else { return Some(rd) };
            if let Some(&(ot, _)) = found.iter().find(|(t, _)| !self.ast.types.same_type(*t, bt)) {
                let msg = self
                    .diag(DiagId::sema_decomp_decl_multiple_bases_with_members, loc)
                    .arg(self.ty_arg(rty))
                    .arg(format!("its base classes {} and", self.ty_arg(bt)))
                    .arg(self.ty_arg(ot));
                self.emit(msg);
                return None;
            }
            if found.len() > 1 {
                let msg = self
                    .diag(DiagId::sema_decomp_decl_ambiguous_base, loc)
                    .arg(self.ty_arg(rty))
                    .arg(self.ty_arg(bt));
                self.emit(msg);
                return None;
            }
            bc
        };
        let mut deeper = Vec::new();
        self.bases_with_fields(with_fields, &mut deeper);
        if let Some(&(bt, _)) = deeper.first() {
            let both = self.ast.first(with_fields) == self.ast.first(rd);
            let who = if both {
                "both it and its base class".to_owned()
            } else {
                format!("its base classes {} and", self.ty_arg(self.tag_type(with_fields)))
            };
            let msg = self
                .diag(DiagId::sema_decomp_decl_multiple_bases_with_members, loc)
                .arg(self.ty_arg(rty))
                .arg(who)
                .arg(self.ty_arg(bt));
            self.emit(msg);
            return None;
        }
        Some(with_fields)
    }

    /// [dcl.struct.bind]/5: `e.mi` for the non-static data members.
    fn decompose_members(
        &mut self,
        v: DeclId,
        dty: QualType,
        r: DeclId,
        bindings: &[DeclId],
    ) -> bool {
        let loc = self.ast.decls.get(v).loc();
        if !self.require_complete(dty, loc) {
            return false;
        }
        let Some(rdef) = self.ast.definition(r) else { return false };
        let Some(c) = self.decomposable_class(rdef, loc) else { return false };
        let Some(cdef) = self.ast.definition(c) else { return false };
        let fields: Vec<DeclId> =
            self.ast.members(cdef).filter(|&m| self.kind_of(m) == DeclKind::Field).collect();
        let named = |s: &Self, f: DeclId| {
            !(s.ast.decls.get(f).name().is_empty()
                && s.ast.decls.field(f).is_some_and(|x| x.bit_width.is_some()))
        };
        let count = fields.iter().filter(|&&f| named(self, f)).count();
        let bad_count = |s: &mut Self| {
            s.wrong_number_of_bindings(v, dty, bindings.len(), count as u64);
            false
        };
        let q = self.ast.types.effective_quals(dty);
        let mut i = 0;
        for f in fields {
            if !named(self, f) {
                continue;
            }
            if self.ast.decls.get(f).name().is_empty() {
                if self.ast.closure_lambda(cdef).is_some() {
                    let msg = self.diag(DiagId::sema_decomp_decl_lambda, loc).arg(self.ty_arg(dty));
                    self.emit(msg);
                    return false;
                }
                let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                let union = self.ast.types.as_record(fty).is_some_and(|u| self.is_union(u));
                let msg = self
                    .diag(DiagId::sema_decomp_decl_anon_union_member, loc)
                    .arg(self.ty_arg(dty))
                    .arg(u8::from(union))
                    .with_note(self.diag(DiagId::note_declared_at, self.ast.decls.get(f).loc()));
                self.emit(msg);
                return false;
            }
            let Some(&b) = bindings.get(i) else { return bad_count(self) };
            i += 1;
            let bloc = self.ast.decls.get(b).loc();
            let fname = self.decl_name(f);
            let rdef_ty = dty;
            if let Some(msg) = self.special_member_access(f, rdef, |s, protected| {
                s.diag(DiagId::sema_decomp_decl_inaccessible_field, bloc)
                    .arg(u8::from(protected))
                    .arg(&fname)
                    .arg(s.ty_arg(rdef_ty))
            }) {
                self.emit(msg);
            }
            let base = self.decomp_ref(v, bloc);
            let e = self.member_of_object(base, dty, rdef, false, f, None, None, bloc);
            if self.is_err_expr(e) {
                return false;
            }
            let e = self.act_on_full_expr(e);
            let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            let mutable =
                self.ast.decls.field(f).is_some_and(kiln_ast::decl::FieldData::is_mutable);
            let fq = if mutable { q - Quals::CONST } else { q };
            let bty = if fq.is_empty() || self.ast.types.is_reference(fty) {
                fty
            } else {
                self.ast.types.qualify(fty, fq)
            };
            self.ast.decls.set_ty(b, bty);
            let _ = self.ast.decls.set_binding(b, e, None);
        }
        if i != bindings.len() {
            return bad_count(self);
        }
        true
    }
}
