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

use kiln_ast::decl::DeclKind;
use kiln_ast::types::{AutoKeyword, TemplateArg, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use crate::Sema;
use crate::lookup::LookupKind;
use crate::templ::actions::TemplateIdRef;

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
                let loc = init.iter().flatten().next().map_or(Loc::INVALID, |&e| {
                    self.ast.exprs.get(e).loc()
                });
                match self.act_template_id(name, &[TemplateArg::ty(u)], loc, loc) {
                    Ok(TemplateIdRef::Type(t)) => Ok(self.replace_placeholder(declared, t)),
                    _ => Err(AutoFail::Error),
                }
            }
        }
    }

    /// The keyword of the placeholder in `declared` (under pointer and
    /// reference declarators).
    fn placeholder_keyword(&self, declared: QualType) -> Option<AutoKeyword> {
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
