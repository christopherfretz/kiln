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
use kiln_ast::types::{ArgView, AutoKeyword, TemplateArg, TypeKind, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use kiln_diag::DiagId;

use crate::Sema;
use crate::declspec::{DeclSpec, TypeSpecKind};
use crate::init::InitValue;
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
    /// `auto x({1});` (0) or `auto x{{1}};` (1): a direct initializer
    /// whose only element is braced (reported by `deduce_var_auto`).
    ParenBraces(usize),
    /// An erroneous initializer (already diagnosed).
    Error,
}

/// The items of braced-init-list `l` that deduce a placeholder (`None`
/// for one that cannot: a designated item, or a nested braced-init-list
/// of a direct-list-initialization). For copy-list-initialization
/// (`copy`), deduction is against `std::initializer_list<U>`, and a nested
/// braced-init-list element is a non-deduced context ([temp.deduct.call]/1,
/// [dcl.type.auto.deduct]/2.1), so it is left out: its type comes from the
/// element type deduced from the other elements.
pub(crate) fn auto_list_items(l: &crate::init::InitList, copy: bool) -> Vec<Option<ExprId>> {
    l.items
        .iter()
        .filter(|it| !(copy && it.designator.is_empty() && matches!(it.value, InitValue::List(_))))
        .map(|it| match it.value {
            InitValue::Expr(e) if it.designator.is_empty() => Some(e),
            _ => None,
        })
        .collect()
}

impl Sema<'_, '_> {
    /// [`Self::deduce_auto`] from `e`, an overloaded member access `x`
    /// (kiln's `UnresolvedMemberExpr`). A set of only implicit object
    /// member functions can only be called ([expr.ref]/7.3.2; clang gives
    /// it the bound-member type and diagnoses it before deduction).
    /// Otherwise P is no function type, so only a member template-id
    /// naming a single specialization deduces ([temp.deduct.call]/6,
    /// [over.over]/3), and only a static one: a non-static member is no
    /// function lvalue without a call (clang's `GetTypeOfFunction`).
    fn deduce_from_member_set(
        &mut self,
        declared: QualType,
        e: ExprId,
        x: ExprId,
    ) -> Result<QualType, AutoFail> {
        if self.member_set_needs_call(x) {
            // Recovered by calling it when that takes no arguments
            // (clang's `tryToRecoverWithCall`, WP-832).
            let call = self.recover_not_called_as(e, true);
            if self.is_err_expr(call) {
                return Err(AutoFail::Error);
            }
            return self.deduce_from(declared, call).ok_or(AutoFail::Incompatible(call));
        }
        let Some((d, _)) = self.single_member_template_spec(x) else {
            return Err(AutoFail::Incompatible(e));
        };
        if !self.is_static_method(d) {
            return Err(AutoFail::Incompatible(e));
        }
        let loc = self.ast.exprs.get(x).loc();
        if self.check_undeduced_use(d, loc) {
            return Err(AutoFail::Error);
        }
        let Some(fty) = self.ast.decls.get(d).ty() else { return Err(AutoFail::Error) };
        self.deduce_from_function_lvalue(declared, fty).ok_or(AutoFail::Incompatible(e))
    }

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
        // The expression count is checked before dependence, as clang's
        // `deduceVarTypeFromInitializer` does: `auto x(t, t);` is
        // ill-formed in a template definition (a pack expansion counts as
        // one expression).
        if matches!(kind, AutoInit::Expr | AutoInit::Paren | AutoInit::DirectList) {
            if kind == AutoInit::DirectList && init.is_empty() {
                return Err(AutoFail::Empty);
            }
            if init.len() > 1 {
                let at = init[1].map_or(Loc::INVALID, |e| self.ast.exprs.get(e).loc());
                return Err(AutoFail::Multiple(at));
            }
        }
        if init.iter().flatten().any(|&e| self.expr_is_type_dependent(e)) {
            return Ok(self.dependent_auto(declared));
        }
        match kind {
            AutoInit::Expr | AutoInit::Paren | AutoInit::DirectList => {
                let Some(Some(e)) = init.first().copied() else {
                    return Err(AutoFail::FromInitList);
                };
                if let Some(x) = self.member_overload(e) {
                    return self.deduce_from_member_set(declared, e, x);
                }
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
                    Ok(TemplateIdRef::Type(t)) => {
                        // clang's `BuildStdInitializerList`: the type is
                        // spelled `std::initializer_list<U>`.
                        let t = match self.ast.decls.get(il).ctx() {
                            Some(ns) => {
                                let ns = self.ast.first(ns);
                                let n = self
                                    .ast
                                    .types
                                    .nns(kiln_ast::types::Nns::Namespace { prefix: None, ns });
                                self.ast.types.elaborated(
                                    kiln_ast::types::TagKeyword::None,
                                    Some(n),
                                    t,
                                )
                            }
                            None => t,
                        };
                        Ok(self.replace_placeholder(declared, t))
                    }
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
                .arg(kw as u8)
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
                | (TypeView::RValueRef(p), TypeView::RValueRef(x))
                | (
                    TypeView::MemberPointer { pointee: p, .. },
                    TypeView::MemberPointer { pointee: x, .. },
                ) => (d, a) = (p, x),
                (TypeView::Function(p), TypeView::Function(x)) => (d, a) = (p.ret(), x.ret()),
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
        let found = self.contained_placeholder(declared).and_then(|leaf| {
            match self.ast.types.canonical_view(leaf) {
                TypeView::Auto { concept: Some(c), args, .. } => Some((c, args)),
                _ => None,
            }
        });
        let Some((c, extra)) = found else { return true };
        if self.ast.types.is_dependent(deduced) || self.is_error_type(deduced) {
            return true;
        }
        let Some((_, t)) = self.placeholder_deduction(declared, deduced) else { return true };
        let mut args = vec![TemplateArg::ty(t)];
        if let Some(x) = extra {
            args.extend_from_slice(self.ast.types.args(x));
        }
        // A constant argument still as written (`C<tag{3}> auto`, or
        // `C<f(R)> auto` once substituted) is converted to the concept's
        // parameter first, as a concept-id's is ([temp.arg.nontype]/2;
        // WP-1235): its satisfaction reads the parameter's value.
        let list = if extra.is_some_and(|x| {
            self.ast.types.args(x).iter().any(|a| matches!(a.view(), ArgView::Expr { .. }))
        }) {
            match self.check_template_args(c, &args, &[], loc) {
                Ok(conv) => self.settle_inst_sugar(conv.sugared),
                Err(()) => return false,
            }
        } else {
            let list = self.ast.types.arg_list(&args);
            // `C auto` names `C<T, args...>`, completed with C's defaults.
            self.concept_default_args(c, list, loc)
        };
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

    /// The type one declarator further toward the placeholder of a
    /// declared type `t` (clang's `GetContainedDeducedTypeVisitor`,
    /// restricted to the declarators a placeholder appears under in
    /// kiln): a pointer's, member pointer's or reference's pointee
    /// (`auto Q::* p = &Q::m;`, [dcl.type.auto.deduct]/4), a function type's
    /// return type (`auto (*p)(int) -> auto (*)(int) -> auto`,
    /// [dcl.spec.auto.general]/3, [dcl.fct]/22). Spec:
    /// sema-templates.md §6.3.
    pub(crate) fn placeholder_step(&self, t: QualType) -> Option<QualType> {
        match self.ast.types.canonical_view(t) {
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => Some(p),
            TypeView::MemberPointer { pointee, .. } => Some(pointee),
            TypeView::Function(f) => Some(f.ret()),
            _ => None,
        }
    }

    /// Whether `t` contains a placeholder, deduced or not (as
    /// [`Sema::placeholder_step`] walks it; clang `getContainedAutoType`).
    /// A templated function's `auto` return type is "deduced" as a
    /// dependent type, so this is how a function template's deduced
    /// return type is recognized.
    pub(crate) fn contains_placeholder(&self, t: QualType) -> bool {
        let mut cur = t;
        for _ in 0..256 {
            if let TypeView::Auto { .. } = self.ast.types.canonical_view(cur) {
                return true;
            }
            let Some(next) = self.placeholder_step(cur) else { return false };
            cur = next;
        }
        false
    }

    /// The undeduced placeholder contained in `t` (as
    /// [`Sema::placeholder_step`] walks it), canonical.
    pub(crate) fn contained_placeholder(&self, t: QualType) -> Option<QualType> {
        let mut cur = t;
        for _ in 0..256 {
            let c = self.ast.types.canonical(cur).ty();
            if let Some(TypeView::Auto { deduced: None, .. }) =
                self.ast.types.view_if(c, TypeKind::Auto)
            {
                return Some(self.ast.types.canonical(cur));
            }
            cur = self.placeholder_step(cur)?;
        }
        None
    }

    /// Whether declaration `d`'s type still holds an undeduced placeholder
    /// (`auto x = ...` before its initializer is attached).
    pub fn has_undeduced_type(&self, d: DeclId) -> bool {
        self.ast.decls.get(d).ty().is_some_and(|t| self.contained_placeholder(t).is_some())
    }

    /// The keyword of the placeholder in `declared` (under pointer and
    /// reference declarators and function return types).
    pub(crate) fn placeholder_keyword(&self, declared: QualType) -> Option<AutoKeyword> {
        match self.ast.types.canonical_view(self.contained_placeholder(declared)?) {
            TypeView::Auto { keyword, .. } => Some(keyword),
            _ => None,
        }
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
        let Some(leaf) = self.contained_placeholder(declared) else { return QualType::DEPENDENT };
        let TypeView::Auto { keyword, concept, args, pack, .. } =
            self.ast.types.canonical_view(leaf)
        else {
            return QualType::DEPENDENT;
        };
        let d = self.ast.types.auto_type(keyword, Some(QualType::DEPENDENT), concept, args, pack);
        self.replace_placeholder(declared, d)
    }

    /// `declared` with its placeholder (under pointer and reference
    /// declarators and function return types) replaced by `with`,
    /// keeping the cv-qualifiers written on each level. A function type
    /// keeps its sugar (parameters as written, the trailing-return form:
    /// clang prints `auto (*)(int) -> int`).
    pub(crate) fn replace_placeholder(&mut self, declared: QualType, with: QualType) -> QualType {
        let t = &self.ast.types;
        // The declarator nodes as written first (their sugar survives),
        // then the canonical type.
        let (view, q) = match t.view(declared.ty()) {
            v @ (TypeView::Pointer(_)
            | TypeView::LValueRef(_)
            | TypeView::RValueRef(_)
            | TypeView::MemberPointer { .. }
            | TypeView::Function(_)
            | TypeView::Paren(_)) => (v, declared.quals()),
            _ => (t.canonical_view(declared), t.canonical(declared).quals()),
        };
        let r = match view {
            TypeView::Auto { deduced: None, .. } => with,
            TypeView::Function(f) => {
                let ret = self.replace_placeholder(f.ret(), with);
                let params = self.ast.types.params(&f).to_vec();
                let exc = self.ast.types.exc_spec(&f);
                self.ast.types.function(ret, &params, f.info(), exc)
            }
            TypeView::Paren(p) => {
                let inner = self.replace_placeholder(p, with);
                self.ast.types.paren_type(inner)
            }
            TypeView::Pointer(p) => {
                let inner = self.replace_placeholder(p, with);
                self.ast.types.pointer_to(inner)
            }
            TypeView::MemberPointer { pointee, class } => {
                let inner = self.replace_placeholder(pointee, with);
                self.ast.types.member_pointer(inner, class)
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
