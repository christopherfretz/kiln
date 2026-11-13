//! Structured binding declarations ([dcl.struct.bind], spec/sema.md
//! §4.10): the decomposition declarator, the unnamed variable `e` that
//! holds the initializer, and the bindings of its array elements,
//! tuple-like elements (`get<i>`) or data members (clang's
//! `ActOnDecompositionDeclarator` / `CheckCompleteDecompositionDeclaration`).

use kiln_ast::decl::{DeclFlags, DeclKind, DeclMisc, VarSpec, VarStorage};
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
use crate::declspec::{Chunk, DeclSpec, Declarator, DeclaratorContext, StorageSpec, TypeSpecKind};
use crate::init::Initializer;
use crate::lookup::LookupKind;
use crate::templ::actions::TemplateIdRef;
use crate::templ::stack::FrameKind;

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

/// What every binding of a tuple-like decomposition shares.
#[derive(Clone, Copy)]
struct TupleCtx {
    v: DeclId,
    dty: QualType,
    lref: bool,
    member_get: bool,
    get: kiln_source::symbol::Symbol,
    ctx: DeclId,
    lexical: Option<DeclId>,
    vspec: VarSpec,
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
        if d.ctx == DeclaratorContext::Condition && self.std < kiln_lex::Std::Cxx26 {
            // [stmt.pre]/4 (C++26, P0963): a structured binding declaration
            // may be a condition; before C++26 clang's
            // `ext_decomp_decl_cond` (clang 19's wording).
            self.error(DiagId::sema_decomp_decl_cond, dc.lbracket);
        }
        // [dcl.pre]/6: only static, thread_local, auto and cv; C++26
        // (P2686R5) adds constexpr and constinit (clang 22 accepts them
        // at C++26 only, an error before).
        let p2686 = self.std >= kiln_lex::Std::Cxx26;
        let mut bad: Vec<(&str, Loc)> = Vec::new();
        if let Some((s, l)) = ds.storage
            && s != StorageSpec::Static
        {
            bad.push((s.spelling(), l));
        }
        for (kw, at, ok) in [
            ("constexpr", ds.constexpr, p2686),
            ("consteval", ds.consteval, false),
            ("constinit", ds.constinit, p2686),
        ] {
            if let Some(l) = at
                && !ok
            {
                bad.push((kw, l));
            }
        }
        if let Some(l) = ds.inline {
            bad.push(("inline", l));
        }
        self.decomp_cxx20_specifiers(ds, !bad.is_empty());
        if self.clang_compat >= 22 {
            // clang 22: one error per specifier, at its location, the
            // storage class first, then `inline`, then the constexpr kind.
            let rank = |kw: &str| match kw {
                "inline" => 1,
                "constexpr" | "consteval" | "constinit" => 2,
                _ => 0,
            };
            bad.sort_by_key(|b| rank(b.0));
            for &(kw, l) in &bad {
                let msg = self.diag(DiagId::sema_decomp_decl_spec, l).arg(format!("'{kw}'"));
                self.emit(msg);
            }
        } else if let Some(&(first, l)) = bad.first() {
            // clang 19: one error at the first, naming them all.
            let arg = if bad.len() == 1 {
                format!("'{first}'")
            } else {
                let all: Vec<&str> = bad.iter().map(|b| b.0).collect();
                format!("with '{}' specifiers", all.join(" "))
            };
            let msg = self.diag(DiagId::sema_decomp_decl_spec, l).arg(arg);
            self.emit(msg);
        }
        // [dcl.constexpr]/2: consteval only on a function (clang's
        // `err_constexpr_wrong_decl_kind`, as for any variable).
        if let Some(l) = ds.consteval {
            self.error(DiagId::sema_consteval_wrong_decl_kind, l);
        }
        if ds.is_typedef() {
            return None;
        }
        // [depr.volatile.type]/4 (C++20 [dcl.struct.bind]/1): a cv that
        // includes volatile is deprecated (clang
        // `warn_deprecated_volatile_structured_binding`, at `volatile`).
        if self.std >= kiln_lex::Std::Cxx20
            && let Some(at) = ds.cv_locs[1]
        {
            let d = self.diag(DiagId::sema_deprecated_volatile_binding, at);
            self.emit(d);
        }
        let mut invalid = ds.invalid || d.invalid || self.is_error_type(ty);
        let one_ref = matches!(d.chunks.as_slice(), [] | [Chunk::LRef { .. } | Chunk::RRef { .. }]);
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
        if !self.check_binding_packs(&dc) {
            return None;
        }
        let v = self.declare_decomposition(ds, ty, d, &dc, invalid);
        if self.in_template() {
            self.decomp_written.insert(v, ty);
        }
        Some(v)
    }

    /// C++20 [dcl.dcl]/8 (P1091R3): `static` and `thread_local` on a
    /// structured binding declaration are a C++20 extension in C++17
    /// (clang `ext_decomp_decl_spec`, `-Wc++20-extensions`). clang 22
    /// reports each specifier at its location; clang 19 one warning at the
    /// first, naming them all, and none when another specifier is an
    /// error.
    fn decomp_cxx20_specifiers(&mut self, ds: &DeclSpec, has_bad: bool) {
        if self.std >= kiln_lex::Std::Cxx20 {
            return;
        }
        let mut specs: Vec<(&str, Loc)> = Vec::new();
        if let Some((StorageSpec::Static, l)) = ds.storage {
            specs.push(("static", l));
        }
        if let Some(l) = ds.thread_local {
            let name = if ds.thread_kind == kiln_ast::decl::TlsKind::Dynamic {
                "thread_local"
            } else if self.diags.sm().span_text(kiln_source::Span::new(l, l.with_offset(8)))
                == Some("__thread")
            {
                "__thread"
            } else {
                "_Thread_local"
            };
            specs.push((name, l));
        }
        if self.clang_compat >= 22 {
            for (name, l) in specs {
                let msg = self.diag(DiagId::sema_ext_decomp_decl_spec, l).arg(format!("'{name}'"));
                self.emit(msg);
            }
        } else if let Some(&(first, l)) = specs.first()
            && !has_bad
        {
            let arg = if specs.len() == 1 {
                format!("'{first}'")
            } else {
                let all: Vec<&str> = specs.iter().map(|s| s.0).collect();
                format!("with '{}' specifiers", all.join(" "))
            };
            let msg = self.diag(DiagId::sema_ext_decomp_decl_spec, l).arg(arg);
            self.emit(msg);
        }
    }

    /// [dcl.struct.bind]/1 (P1061): at most one sb-identifier is a
    /// structured binding pack, and only in a templated entity (clang
    /// 22 `err_pack_outside_template`, `err_decomp_decl_multiple_ellipsis`;
    /// the declaration is dropped, as in clang). False after a diagnostic.
    fn check_binding_packs(&mut self, dc: &crate::declspec::Decomposition) -> bool {
        let mut first: Option<Loc> = None;
        for b in &dc.ids {
            let Some(e) = b.ellipsis else { continue };
            if !self.in_template() {
                self.error(DiagId::sema_pack_outside_template, e);
                return false;
            }
            if let Some(p) = first {
                let msg = self
                    .diag(DiagId::sema_decomp_decl_multiple_packs, e)
                    .with_note(self.diag(DiagId::note_previous_binding_pack, p));
                self.emit(msg);
                return false;
            }
            first = Some(e);
        }
        true
    }

    /// Declares the bindings `dc` and then the unnamed variable of type
    /// `ty` they decompose (located at `[`); `invalid` marks it so.
    pub(crate) fn declare_decomposition(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        dc: &crate::declspec::Decomposition,
        invalid: bool,
    ) -> DeclId {
        let (ctx, lexical) = self.decl_contexts(d);
        // [basic.scope.scope]/5 (P2169): a structured binding `_` not at
        // namespace scope (of automatic storage duration, as clang) is
        // name-independent.
        let automatic = self.in_local_scope()
            && ds.thread_local.is_none()
            && !matches!(ds.storage, Some((StorageSpec::Static | StorageSpec::Extern, _)));
        let mut bindings = Vec::with_capacity(dc.ids.len());
        for b in &dc.ids {
            self.check_template_param_shadow(b.name, b.loc);
            let independent = automatic && self.is_placeholder_name(b.name);
            let prev = self.prev_candidates(b.name, None).last().copied();
            if independent
                && let Some(p) = prev
                && matches!(self.kind_of(p), DeclKind::Var | DeclKind::Binding)
            {
                self.diagnose_placeholder_redecl(b.loc);
            } else if let Some(p) = prev {
                let name = self.ast.symbols().as_str(b.name).to_owned();
                let msg = self.diag(DiagId::sema_redefinition, b.loc).arg(name);
                let msg = self.note_prev(msg, p, true);
                self.emit(msg);
            }
            let mut h = DeclHead::new(ctx, DeclName::ident(b.name), b.loc)
                .misc(DeclMisc::new().with_name_independent(independent));
            h.lexical = Some(lexical);
            let bd = self.builder().binding(h);
            if !b.attrs.is_empty() {
                let mut attrs = b.attrs.clone();
                self.check_decl_attrs(bd, &mut attrs);
                let _ = self.builder().attach_attrs(bd, &attrs);
            }
            if prev.is_none() {
                self.check_shadow(bd, b.name);
            }
            self.register_local(b.name, bd);
            if b.ellipsis.is_some() {
                self.binding_packs.insert(bd);
            }
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
        // P2686R5: `constinit auto [a, b] = e;` requires constant
        // initialization of the unnamed variable ([dcl.constinit]/2).
        self.record_constinit(v, ds);
        if invalid || self.storage_invalidates_var(ds, d) {
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
        let _ = self.finish_template_pattern(v);
        let mut attrs = ds.attrs.clone();
        attrs.extend(d.attrs.iter().copied());
        if !attrs.is_empty() {
            self.check_decl_attrs(v, &mut attrs);
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
                crate::init::InitValue::List(_) => return,
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
        // P2686R5: a constexpr one is constant-initialized from the array.
        if self.ast.decls.var(v).is_some_and(|x| x.spec.is_constexpr()) && !self.is_err_expr(init) {
            self.check_constexpr_init(v, init);
        }
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
        let has_pack = bindings.iter().any(|b| self.binding_packs.contains(b));
        if self.ast.types.is_dependent(vty) || undeduced || has_pack && self.in_template() {
            // Decomposed at instantiation; a binding pack (P1061) is an
            // unexpanded pack of dependent type until then, whatever
            // the initializer (as clang 22).
            for b in bindings {
                let t = if self.binding_packs.contains(&b) {
                    self.ast.types.pack_expansion(QualType::DEPENDENT, None)
                } else {
                    QualType::DEPENDENT
                };
                self.ast.decls.set_ty(b, t);
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

    /// [dcl.struct.bind]/1 (P1061): with a structured binding pack among
    /// `bindings`, once `e` is known to have `n` elements the pack
    /// introduces `n - (bindings - 1)` bindings of its name, which replace
    /// it in `v`'s binding list (instantiation maps the pattern pack to
    /// them, spec/sema-templates.md §7.1). The flattened list, or `None`
    /// after a count diagnostic (the pack counts as one name).
    fn expand_binding_pack(
        &mut self,
        v: DeclId,
        dty: QualType,
        bindings: &[DeclId],
        n: u64,
    ) -> Option<Vec<DeclId>> {
        let Some(p) = bindings.iter().position(|b| self.binding_packs.contains(b)) else {
            return Some(bindings.to_vec());
        };
        let fixed = (bindings.len() - 1) as u64;
        if fixed > n {
            self.wrong_number_of_bindings(v, dty, bindings.len(), n);
            return None;
        }
        let pack = bindings[p];
        let pn = *self.ast.decls.get(pack);
        let lexical = self.ast.decls.lexical_ctx(pack);
        let mut elems = Vec::new();
        for _ in 0..n - fixed {
            let mut h = DeclHead::new(pn.ctx().unwrap_or(self.ast.tu), pn.name(), pn.loc());
            h.lexical = lexical;
            h.flags = DeclFlags::IMPLICIT;
            elems.push(self.builder().binding(h));
        }
        let el = self.ast.lists.push(&elems);
        self.binding_pack_elems.insert(pack, el);
        let mut flat = bindings[..p].to_vec();
        flat.extend_from_slice(&elems);
        flat.extend_from_slice(&bindings[p + 1..]);
        let list = self.ast.lists.push(&flat);
        if self.ast.decls.expand_decomp_bindings(v, list, &elems).is_err() {
            return None;
        }
        Some(flat)
    }

    /// `type 'P' decomposes into 2 elements, but 3 names were provided`.
    fn wrong_number_of_bindings(&mut self, v: DeclId, dty: QualType, names: usize, elems: u64) {
        let loc = self.ast.decls.get(v).loc();
        let elements = format!("{elems} element{}", if elems == 1 { "" } else { "s" });
        let provided = if (names as u64) > elems {
            match names {
                0 => "no names were".to_owned(),
                1 => "1 name was".to_owned(),
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
        let Some(bindings) = self.expand_binding_pack(v, dty, bindings, n) else { return false };
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
            // A consteval-only `e` is checked as `v`'s initializer
            // (spec/reflection-repr.md §6): its binding expressions are
            // not evaluated where they are built.
            self.note_var_init_full_expr(v);
            let sub = self.act_on_full_expr(sub);
            self.ast.decls.set_ty(b, elem);
            let _ = self.ast.decls.set_binding(b, sub, None);
        }
        true
    }

    /// `std::NAME` if it is a class template.
    pub(crate) fn std_class_template(&self, name: &str) -> Option<(DeclId, DeclId)> {
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
        self.std_trait_ty(trait_name, args, loc, false).ok().map(|x| x.0)
    }

    /// [`Self::std_trait`], with the specialization's type as written
    /// (`std::NAME<args>::`, a nested-name-specifier). With `complain`, an
    /// incomplete specialization is diagnosed as clang's
    /// `RequireCompleteType` does (`implicit instantiation of undefined
    /// template`), and the error is `Err(true)`.
    fn std_trait_ty(
        &mut self,
        trait_name: &str,
        args: &[TemplateArg],
        loc: Loc,
        complain: bool,
    ) -> Result<(DeclId, NnsId), bool> {
        let (ns, tmpl) = self.std_class_template(trait_name).ok_or(false)?;
        let name = self.template_name_for(tmpl);
        let Ok(TemplateIdRef::Type(t)) = self.act_template_id(name, args, loc, loc) else {
            return Err(false);
        };
        let complete = if complain {
            self.require_complete_unnoted(t, loc)
        } else {
            self.require_complete_silent(t, loc)
        };
        if !complete {
            return Err(complain);
        }
        let r = self.ast.types.as_record(t).ok_or(false)?;
        let def = self.ast.definition(r).ok_or(false)?;
        let std = self.ast.types.nns(Nns::Namespace { prefix: None, ns });
        let nns = self.ast.types.nns(Nns::Type { prefix: Some(std), ty: t, template_kw: false });
        Ok((def, nns))
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
        let (n, fail) = if self.is_err_expr(e) {
            (None, None)
        } else {
            match self.eval_int(e, EvalContext::new(EvalKind::Other, loc)) {
                Ok(n) => (Some(n), None),
                Err(f) => (None, Some(f)),
            }
        };
        if let Some(n) = n.and_then(|n| u64::try_from(n).ok()) {
            return TupleLike::Yes(n);
        }
        let mut msg =
            self.diag(DiagId::sema_decomp_decl_tuple_size_not_constant, loc).arg(self.ty_str(dty));
        if let Some(f) = fail {
            msg = Self::with_failure_notes(msg, f);
        }
        self.emit(msg);
        TupleLike::Error
    }

    /// The integral template argument `i` of type `std::size_t`, as the
    /// reflection library traits pass it (`std::tuple_element<i, T>`,
    /// `std::array<T, 0>`).
    pub(crate) fn size_t_arg(&mut self, i: u64) -> TemplateArg {
        let size_t = self.size_type();
        let canon = self.ast.types.canonical(size_t);
        let v = self.int_const(canon, i128::from(i));
        TemplateArg::value(canon, v)
    }

    /// `std::tuple_element<i, E>::type` (clang `getTupleLikeElementType`).
    fn tuple_element_type(&mut self, loc: Loc, i: u64, dty: QualType) -> Option<QualType> {
        // The index is an integer literal of type `std::size_t` converted
        // to the template parameter's type ([dcl.struct.bind]/4, a
        // template-argument; [temp.arg.nontype]/2), so `template<unsigned
        // I, class T> struct tuple_element` still finds its specialization
        // `tuple_element<0, T>`. clang 22 keeps the literal as written (the
        // qualifier prints `tuple_element<0UL, E>`); clang 19 writes the
        // converted value (`tuple_element<0, E>`, `'\x00'` for an
        // `unsigned char` index).
        let size_t = self.size_type();
        let lit = self.ast.builder().int_literal(size_t, i, Radix::Decimal, loc);
        let mut args = [TemplateArg::expr(lit, None), TemplateArg::ty(dty)];
        let mut converted = true;
        if self.clang_compat < 22
            && let Some((_, tmpl)) = self.std_class_template("tuple_element")
        {
            match self.check_template_args(tmpl, &args, &[], loc) {
                Ok(conv) => {
                    if let Some(&a) = self.ast.types.args(conv.sugared).first() {
                        args[0] = a;
                    }
                }
                // Diagnosed; the template-id is not formed (clang's
                // `CheckTemplateIdType` fails the same way).
                Err(()) => converted = false,
            }
        }
        let mut qual = None;
        let found = if converted {
            match self.std_trait_ty("tuple_element", &args, loc, true) {
                Ok((c, nns)) => {
                    qual = Some(nns);
                    let ty = self.intern("type");
                    Some(self.lookup_in_class(c, ty, LookupKind::Ordinary))
                }
                Err(true) => return None,
                Err(false) => None,
            }
        } else {
            None
        };
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
        let Some(bindings) = self.expand_binding_pack(v, dty, bindings, n) else { return false };
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
        let tc = TupleCtx { v, dty, lref, member_get, get, ctx, lexical, vspec };
        let mut ok = true;
        for (i, &b) in bindings.iter().enumerate() {
            let loc = self.ast.decls.get(b).loc();
            // Diagnostics name the binding being initialized.
            if !self.push_inst_frame(FrameKind::BindingInit, b, b, loc) {
                return false;
            }
            let r = self.decompose_tuple_binding(&tc, i, b);
            self.pop_inst_frame();
            match r {
                Some(true) => {}
                Some(false) => ok = false,
                None => return false,
            }
        }
        ok
    }

    /// One binding of [`Self::decompose_tuple`]: its holding variable and
    /// initializer. `Some(false)` when only the `get` call failed: clang
    /// recovers it (a `RecoveryExpr`) and goes on to the next binding;
    /// `None` stops the decomposition.
    fn decompose_tuple_binding(&mut self, tc: &TupleCtx, i: usize, b: DeclId) -> Option<bool> {
        let TupleCtx { v, dty, lref, member_get, get, ctx, lexical, vspec } = *tc;
        let loc = self.ast.decls.get(b).loc();
        let mut e = self.decomp_ref(v, loc);
        if !lref {
            let t = self.ety(e);
            e = self.builder().implicit_cast(CastKind::NoOp, e, t, ValueCategory::XValue);
        }
        // `get<i>`: `i` is written as a `std::size_t` integer literal
        // (clang `getTrivialIntegralTemplateArgument`), so it is
        // converted to the type of `get`'s parameter like any written
        // argument: `template <int I> get` is `get<0>` with an `int`
        // argument, mangled `ILi0E` ([temp.arg.nontype]/2).
        let size_t = self.size_type();
        let lit = self.ast.builder().int_literal(size_t, i as u64, Radix::Decimal, loc);
        let idx = TemplateArg::expr(lit, None);
        let targs = self.ast.types.arg_list(&[idx]);
        // An empty lookup is not recovered (clang `DiagnoseEmptyLookup`
        // in `BuildRecoveryCallExpr`); a failed overload resolution is.
        let mut recoverable = true;
        let call = if member_get {
            let m = self.act_on_member(e, get, loc, loc, false);
            if self.is_err_expr(m) {
                return None;
            }
            let m = self.act_on_member_template_args(m, targs);
            self.act_on_call(m, &[], loc, loc)
        } else {
            let mut found = Vec::new();
            self.adl_lookup_dn(DeclName::ident(get), &[e], loc, &mut found);
            recoverable = !found.is_empty();
            let ule = self.builder().unresolved_lookup(
                DeclName::ident(get),
                &[],
                true,
                None,
                Some(targs),
                loc,
            );
            let saved = self.ovl.binding_get.replace(ule);
            let call = self.act_on_call(ule, &[e], loc, loc);
            self.ovl.binding_get = saved;
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
        if self.is_err_expr(call) && !recoverable {
            return None;
        }
        let t = self.tuple_element_type(loc, i as u64, dty)?;
        if self.is_err_expr(call) {
            return Some(false);
        }
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
            .with_tls(vspec.tls())
            // P2686R5 [dcl.struct.bind]/4: a constexpr declaration's
            // holding variables are constexpr.
            .with_constexpr(vspec.is_constexpr());
        let hv = self.builder().holding_var(h, ref_ty, spec);
        let init = self.perform_init(ref_ty, Initializer::Expr(call), loc, false);
        let init = self.act_on_full_expr(init);
        let _ = self.builder().set_var_init(hv, init);
        if self.is_err_expr(init) {
            return None;
        }
        self.extend_temporaries(hv, init);
        // [dcl.struct.bind]/4: the holding variable is checked as any
        // reference variable (clang's implicit `-Wdangling` wording).
        self.check_var_lifetime(hv, init);
        let r = self.build_decl_ref(hv, loc);
        self.ast.decls.set_ty(b, t);
        let _ = self.ast.decls.set_binding(b, r, Some(hv));
        Some(true)
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
        let Some(bindings) = self.expand_binding_pack(v, dty, bindings, count as u64) else {
            return false;
        };
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
            // clang 22 reports an inaccessible member at the declaration's
            // `[`, clang 19 at the binding naming it.
            let aloc = if self.clang_compat >= 22 { loc } else { bloc };
            if let Some(msg) = self.special_member_access(f, rdef, |s, protected| {
                s.diag(DiagId::sema_decomp_decl_inaccessible_field, aloc)
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
            // As for an array element ([`Self::decompose_array`]).
            self.note_var_init_full_expr(v);
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
