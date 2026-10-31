//! Placeholder type deduction for variables ([dcl.type.auto.deduct];
//! spec/sema-templates.md §6.3): the `deduce_auto` seam.
//!
//! `auto` deduces as template argument deduction from a call with the
//! placeholder as the invented parameter `U` (§6.1 step 2); copy-list
//! initialization deduces `std::initializer_list<U>` from each element;
//! `decltype(auto)` applies [dcl.type.decltype] to the initializer.
//!
//! **Seam (WP-070b/071):** the single-expression case is the
//! pointer/reference matcher in `crate::deduce` (`deduce_from`). When
//! `templ::deduce` exposes the general (P, A) matcher, [`Sema::deduce_auto`]
//! should call it with `U` at `AUTO_DEPTH` instead, which also covers
//! placeholders nested in other compound types (`std::vector<auto>` is
//! ill-formed, but `auto (*)()` and `C auto` constraints are not).

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::types::{AutoKeyword, TemplateArg, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use kiln_diag::DiagId;

use crate::Sema;
use crate::declspec::{DeclSpec, TypeSpecKind};
use crate::lookup::LookupKind;
use crate::templ::actions::TemplateIdRef;

/// One declarator group's declarations, kept by the parser across
/// [`Sema::act_on_group_declarator`] calls and checked by
/// [`Sema::act_on_end_group`] ([dcl.spec.auto.general]/3, /8).
/// Parser-local, so nested declarations in an initializer do not
/// interfere.
#[derive(Debug, Default)]
pub struct AutoGroup {
    decls: Vec<(DeclId, Option<QualType>)>,
}

/// The form of a placeholder variable's initializer
/// ([dcl.type.auto.deduct]/2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoInit {
    /// `= e` or `(e)` (one expression; more is [`AutoFail::Multiple`]).
    Expr,
    /// `(e1, e2, ...)`.
    Paren,
    /// `{ ... }` (direct-list-initialization, N3922: exactly one element).
    DirectList,
    /// `= { ... }` (copy-list-initialization: `std::initializer_list<U>`).
    CopyList,
}

/// Why placeholder deduction failed; the caller reports it (its message
/// names the variable).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoFail {
    /// The initializer's type does not match the declared form
    /// (`auto *p = 1;`): "variable 'x' with type T has incompatible
    /// initializer of type U".
    Incompatible(ExprId),
    /// A parenthesized or direct-list initializer with more than one
    /// element (at the second).
    Multiple(Loc),
    /// `auto x{};`.
    Empty,
    /// `= {…}` without a declared `std::initializer_list`.
    NoInitializerList,
    /// Two elements of a copy-list deduce different types.
    Conflict(QualType, QualType, Loc),
    /// A copy-list that is empty or has a braced element: `U` is not
    /// deduced.
    FromInitList,
    /// `decltype(auto) x = {…};`.
    DecltypeAutoList,
    /// An erroneous initializer (already diagnosed).
    Error,
}

impl Sema<'_, '_> {
    /// The type deduced for a variable declared with type `declared`
    /// containing a placeholder from initializer `init`
    /// ([dcl.type.auto.deduct]/2-4). `init` holds the initializer's
    /// expressions in order; `None` stands for a braced element (only a
    /// list has those). A type-dependent initializer gives the declared
    /// type with its placeholder replaced by the dependent `auto`
    /// ([`Sema::dependent_auto`]), deduced again at instantiation.
    ///
    /// # Errors
    /// [`AutoFail`] when deduction fails; nothing has been reported.
    pub fn deduce_auto(
        &mut self,
        declared: QualType,
        init: &[Option<ExprId>],
        kind: AutoInit,
    ) -> Result<QualType, AutoFail> {
        let kw = self.placeholder_keyword(declared);
        if init.iter().flatten().any(|&e| self.is_err_expr(e)) {
            return Err(AutoFail::Error);
        }
        if init.iter().flatten().any(|&e| self.expr_is_type_dependent(e)) {
            return Ok(self.dependent_auto(declared));
        }
        match kind {
            AutoInit::Expr | AutoInit::Paren | AutoInit::DirectList => {
                if kind == AutoInit::DirectList && init.is_empty() {
                    return Err(AutoFail::Empty);
                }
                if init.len() > 1 {
                    let at = init[1].map_or(Loc::INVALID, |e| self.ast.exprs.get(e).loc());
                    return Err(AutoFail::Multiple(at));
                }
                let Some(Some(e)) = init.first().copied() else {
                    return Err(AutoFail::FromInitList);
                };
                self.deduce_from(declared, e).ok_or(AutoFail::Incompatible(e))
            }
            AutoInit::CopyList => {
                if kw == Some(AutoKeyword::DecltypeAuto) {
                    return Err(AutoFail::DecltypeAutoList);
                }
                let Some(il) = self.std_initializer_list() else {
                    return Err(AutoFail::NoInitializerList);
                };
                let mut elem: Option<QualType> = None;
                for &e in init {
                    let Some(e) = e else { return Err(AutoFail::FromInitList) };
                    let u = self.auto_type(AutoKeyword::Auto);
                    let Some(t) = self.deduce_from(u, e) else {
                        return Err(AutoFail::FromInitList);
                    };
                    match elem {
                        None => elem = Some(t),
                        Some(p) if self.ast.types.canonical(p) != self.ast.types.canonical(t) => {
                            let at = self.ast.exprs.get(e).loc();
                            return Err(AutoFail::Conflict(p, t, at));
                        }
                        Some(_) => {}
                    }
                }
                let Some(u) = elem else { return Err(AutoFail::FromInitList) };
                let name = self.template_name_for(il);
                let loc = init
                    .iter()
                    .flatten()
                    .next()
                    .map_or(Loc::INVALID, |&e| self.ast.exprs.get(e).loc());
                match self.act_template_id(name, &[TemplateArg::ty(u)], loc, loc) {
                    Ok(TemplateIdRef::Type(t)) => Ok(self.replace_placeholder(declared, t)),
                    _ => Err(AutoFail::Error),
                }
            }
        }
    }

    /// Records declarator declaration `v` of `group` with its type
    /// before any initializer deduces it. The parser calls this after
    /// each declarator's `act_on_declarator` / `act_on_member`, before
    /// its initializer.
    pub fn act_on_group_declarator(&self, group: &mut AutoGroup, v: DeclId) {
        group.decls.push((v, self.ast.decls.get(v).ty()));
    }

    /// The end of a declarator group with decl-specifiers `ds` (clang's
    /// `FinalizeDeclaratorGroup`): a declarator that uses `auto` other
    /// than to declare a deduced variable must be alone in its group
    /// ([dcl.spec.auto.general]/3, clang's `err_auto_non_deduced_not_alone`),
    /// and the placeholder must deduce the same type in each declarator
    /// ([dcl.spec.auto.general]/8, [`Sema::check_auto_group`]).
    pub fn act_on_end_group(&mut self, group: AutoGroup, ds: &DeclSpec) {
        let decls = group.decls;
        if decls.len() < 2 {
            return;
        }
        if matches!(ds.base, TypeSpecKind::Auto | TypeSpecKind::DecltypeAuto) {
            self.check_non_deduced_alone(&decls);
        }
        let at = ds.type_loc.unwrap_or(ds.loc);
        let items: Vec<_> = decls.iter().map(|&(v, t)| (v, t.map(|t| (t, at)))).collect();
        if self.check_auto_group(&items) && self.in_template() {
            // Instantiation checks the group again with the deductions
            // its substituted initializers make (clang's
            // `BuildDeclaratorGroup` from `TransformDeclStmt`).
            for (v, d) in items {
                if let Some((t, at)) = d
                    && self.placeholder_keyword(t).is_some()
                {
                    self.group_placeholders.insert(v, (t, at));
                }
            }
        }
    }

    /// The declared placeholder type of pattern variable `d` of a
    /// declarator group, and its decl-specifiers' type location
    /// ([`Sema::act_on_end_group`]).
    pub(crate) fn group_placeholder(&self, d: DeclId) -> Option<(QualType, Loc)> {
        self.group_placeholders.get(&d).copied()
    }

    /// clang's `err_auto_non_deduced_not_alone`, reported once at the
    /// first declarator of `decls` that is not a variable with a deduced
    /// type, when the group has another declarator.
    fn check_non_deduced_alone(&mut self, decls: &[(DeclId, Option<QualType>)]) {
        let mut first_non_deduced = None;
        for (i, &(d, t)) in decls.iter().enumerate() {
            let kind = self.kind_of(d);
            if !matches!(
                kind,
                DeclKind::Var
                    | DeclKind::Field
                    | DeclKind::Function
                    | DeclKind::CXXMethod
                    | DeclKind::CXXConstructor
                    | DeclKind::CXXDestructor
                    | DeclKind::CXXConversion
            ) {
                continue;
            }
            let trailing = t.is_some_and(|t| self.has_auto_for_trailing_return(t));
            if first_non_deduced.is_none() && (kind != DeclKind::Var || trailing) {
                first_non_deduced = Some((d, trailing));
            }
            if i > 0
                && let Some((n, trailing)) = first_non_deduced
            {
                let loc = self.ast.decls.get(n).loc();
                let msg =
                    self.diag(DiagId::sema_auto_non_deduced_not_alone, loc).arg(u8::from(trailing));
                self.emit(msg);
                return;
            }
        }
    }

    /// clang's `Type::hasAutoForTrailingReturnType`: the first function
    /// type under pointer, reference, array and member-pointer
    /// declarators of `t` was written with a trailing return type.
    fn has_auto_for_trailing_return(&self, t: QualType) -> bool {
        let mut cur = t;
        for _ in 0..256 {
            match self.ast.types.view(cur.ty()) {
                TypeView::Function(p) => return p.info().trailing_return(),
                TypeView::Pointer(p)
                | TypeView::LValueRef(p)
                | TypeView::RValueRef(p)
                | TypeView::IncompleteArray(p)
                | TypeView::MemberPointer { pointee: p, .. } => cur = p,
                TypeView::ConstantArray(a) => cur = a.elem,
                _ => return false,
            }
        }
        false
    }

    /// [dcl.spec.auto.general]/8: in a declaration with several
    /// declarators, the placeholder must deduce the same type in each
    /// (clang's `BuildDeclaratorGroup`, DR1347). `items` are the group's
    /// declarations, each with its declared placeholder type and the
    /// location of its decl-specifiers' type when it had one. The check
    /// stops at the first declaration that is not a valid variable.
    /// Reports clang's `err_auto_different_deductions` once and returns
    /// `false` then.
    pub(crate) fn check_auto_group(&mut self, items: &[(DeclId, Option<(QualType, Loc)>)]) -> bool {
        let mut first: Option<(DeclId, QualType)> = None;
        for &(v, declared) in items {
            let n = *self.ast.decls.get(v);
            if n.kind() != DeclKind::Var || n.has(DeclFlags::INVALID) {
                break;
            }
            let Some((declared, at)) = declared else { continue };
            let Some(actual) = n.ty() else { continue };
            let Some((kw, deduced)) = self.placeholder_deduction(declared, actual) else {
                continue;
            };
            if self.is_error_type(deduced) || self.ast.types.is_dependent(deduced) {
                continue;
            }
            let Some((d0, t0)) = first else {
                first = Some((v, deduced));
                continue;
            };
            if self.ast.types.canonical(t0) == self.ast.types.canonical(deduced) {
                continue;
            }
            let msg = self
                .diag(DiagId::sema_auto_var_different_deductions, at)
                .arg(u8::from(kw == AutoKeyword::DecltypeAuto))
                .arg(self.ty_arg(t0))
                .arg(self.decl_name(d0))
                .arg(self.ty_arg(deduced))
                .arg(self.decl_name(v));
            self.emit(msg);
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
            return false;
        }
        true
    }

    /// The type the placeholder in `declared` (under pointer and
    /// reference declarators) deduced, given the declarator's type
    /// `actual` after deduction: `actual` walked in parallel, less the
    /// cv-qualifiers written on the placeholder. `auto &&` deduced from
    /// an lvalue gives the lvalue reference itself ([temp.deduct.call]/3).
    fn placeholder_deduction(
        &mut self,
        declared: QualType,
        actual: QualType,
    ) -> Option<(AutoKeyword, QualType)> {
        let (mut d, mut a) = (declared, actual);
        for _ in 0..256 {
            let av = self.ast.types.canonical_view(a);
            match (self.ast.types.canonical_view(d), av) {
                (TypeView::Auto { keyword, deduced: None, .. }, _) => {
                    let q = self.ast.types.canonical(d).quals();
                    let t = if q.is_empty() { a } else { self.ast.types.unqualified(a) };
                    return Some((keyword, t));
                }
                (TypeView::Pointer(p), TypeView::Pointer(x))
                | (TypeView::LValueRef(p), TypeView::LValueRef(x))
                | (TypeView::RValueRef(p), TypeView::RValueRef(x)) => (d, a) = (p, x),
                (TypeView::RValueRef(p), TypeView::LValueRef(_)) => {
                    let TypeView::Auto { keyword, deduced: None, .. } =
                        self.ast.types.canonical_view(p)
                    else {
                        return None;
                    };
                    return Some((keyword, a));
                }
                _ => return None,
            }
        }
        None
    }

    /// [dcl.type.auto.deduct]/2: the type a constrained placeholder in
    /// `declared` deduced (`deduced` is the whole declared type after
    /// deduction) must satisfy its type-constraint `C<T, args...>`.
    /// Reports clang's "deduced type 'T' does not satisfy 'C'" at `loc`
    /// with the concept's chain; returns false when it did.
    pub(crate) fn check_placeholder_constraint(
        &mut self,
        declared: QualType,
        deduced: QualType,
        loc: Loc,
    ) -> bool {
        let mut cur = declared;
        let mut found = None;
        for _ in 0..256 {
            match self.ast.types.canonical_view(cur) {
                TypeView::Auto { deduced: None, concept: Some(c), args, .. } => {
                    found = Some((c, args));
                    break;
                }
                TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => cur = p,
                _ => break,
            }
        }
        let Some((c, extra)) = found else { return true };
        if self.ast.types.is_dependent(deduced) || self.is_error_type(deduced) {
            return true;
        }
        let Some((_, t)) = self.placeholder_deduction(declared, deduced) else { return true };
        let mut args = vec![TemplateArg::ty(t)];
        if let Some(x) = extra {
            args.extend_from_slice(self.ast.types.args(x));
        }
        let list = self.ast.types.arg_list(&args);
        let sat = self.check_concept(c, list, loc);
        if self.sat_ok(sat) {
            return true;
        }
        let shown = match extra {
            Some(x) => self.spec_name(c, x),
            None => self.qualified_decl_name(c),
        };
        let d = self
            .diag(DiagId::sema_placeholder_constraints_not_satisfied, loc)
            .arg(self.ty_arg(t))
            .arg(format!("'{shown}'"));
        let d = self.with_sat_notes(d, sat);
        self.emit(d);
        false
    }

    /// The keyword of the placeholder in `declared` (under pointer and
    /// reference declarators).
    pub(crate) fn placeholder_keyword(&self, declared: QualType) -> Option<AutoKeyword> {
        let mut cur = declared;
        for _ in 0..256 {
            match self.ast.types.canonical_view(cur) {
                TypeView::Auto { keyword, deduced: None, .. } => return Some(keyword),
                TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => cur = p,
                _ => return None,
            }
        }
        None
    }

    /// A plain undeduced placeholder `kw`.
    fn auto_type(&mut self, kw: AutoKeyword) -> QualType {
        self.ast.types.auto_type(kw, None, None, None, false)
    }

    /// `declared` with its placeholder replaced by the dependent `auto`
    /// (an `Auto` deduced as `<dependent type>`, printed as its
    /// keyword, clang's dependent `AutoType`). Substitution turns it back
    /// into the undeduced placeholder, so the instantiated variable
    /// deduces from its substituted initializer.
    pub(crate) fn dependent_auto(&mut self, declared: QualType) -> QualType {
        let mut cur = declared;
        for _ in 0..256 {
            match self.ast.types.canonical_view(cur) {
                TypeView::Auto { keyword, deduced: None, concept, args, pack } => {
                    let d = self.ast.types.auto_type(
                        keyword,
                        Some(QualType::DEPENDENT),
                        concept,
                        args,
                        pack,
                    );
                    return self.replace_placeholder(declared, d);
                }
                TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => cur = p,
                _ => break,
            }
        }
        QualType::DEPENDENT
    }

    /// `declared` with its placeholder (under pointer and reference
    /// declarators) replaced by `with`, keeping the cv-qualifiers written
    /// on each level.
    pub(crate) fn replace_placeholder(&mut self, declared: QualType, with: QualType) -> QualType {
        let t = &self.ast.types;
        let q = t.canonical(declared).quals();
        let r = match t.canonical_view(declared) {
            TypeView::Auto { deduced: None, .. } => with,
            TypeView::Pointer(p) => {
                let inner = self.replace_placeholder(p, with);
                self.ast.types.pointer_to(inner)
            }
            TypeView::LValueRef(p) => {
                let inner = self.replace_placeholder(p, with);
                self.ast.types.lvalue_ref_to(inner)
            }
            TypeView::RValueRef(p) => {
                let inner = self.replace_placeholder(p, with);
                self.ast.types.rvalue_ref_to(inner)
            }
            _ => return declared,
        };
        if q.is_empty() { r } else { self.ast.types.qualify(r, q) }
    }

    /// `std::initializer_list` ([support.initlist]), if declared.
    pub(crate) fn std_initializer_list(&self) -> Option<DeclId> {
        let std = self.intern("std");
        let ns = self.lookup_qualified(self.ast.tu, std, LookupKind::Nested);
        let ns = ns.decls.iter().copied().find(|&d| self.kind_of(d) == DeclKind::Namespace)?;
        let il = self.intern("initializer_list");
        let found = self.lookup_qualified(ns, il, LookupKind::Ordinary);
        found.decls.iter().copied().find(|&d| self.kind_of(d) == DeclKind::ClassTemplate)
    }
}
