//! Functions and variables odr-used but never defined (clang
//! `UndefinedButUsed`, `getUndefinedButUsed` and `checkUndefinedButUsed`,
//! spec/sema.md "Undefined but used entities").
//!
//! [basic.def.odr]/11 requires a definition of every odr-used non-inline
//! function or variable somewhere in the program, and of an inline one in
//! every translation unit that odr-uses it. When no other translation unit
//! can supply it, the program is ill-formed. At the end of the translation
//! unit, after the pending instantiations and only when no error occurred,
//! each entity still undefined is diagnosed at its first declaration, in
//! the order of the first uses, each with a "used here" note at that use:
//! - one with external linkage whose type has no linkage or internal
//!   linkage ([basic.link]/8, a lambda, local or unnamed-namespace class in
//!   its signature) is an error (clang `err_undefined_internal_type`);
//! - one that is not externally visible (internal linkage, or a
//!   specialization or member taking internal linkage from a template
//!   argument or its class) is `-Wundefined-internal`;
//! - an inline function is `-Wundefined-inline`, an inline variable an
//!   error (clang `err_undefined_inline_var`).

use kiln_abi::mangle;
use kiln_ast::DeclId;
use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind, Defaulted};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

/// What [`Sema::undefined_used_kind`] found to say about an entity.
enum Undefined {
    /// External linkage, a type without (external) linkage: an error.
    NoLinkageType,
    /// Not externally visible: `-Wundefined-internal`.
    Internal,
    /// An inline function: `-Wundefined-inline`.
    InlineFunction,
    /// An inline variable: an error.
    InlineVar,
}

impl Sema<'_, '_> {
    /// The odr-use of function or variable `d` at `loc` (clang
    /// `MarkFunctionReferenced` / `MarkVarDeclODRUsed`): its first use
    /// while it is undefined is remembered for the end of the translation
    /// unit. Whether it needs a diagnostic is decided there, so a later
    /// definition or instantiation is seen.
    pub(crate) fn note_undefined_use(&mut self, d: DeclId, loc: Loc) {
        if self.in_template() || self.unevaluated > 0 {
            return;
        }
        let first = self.ast.first(d);
        if self.entity_defined(first) {
            return;
        }
        // A default argument is potentially evaluated only where a call
        // uses it (clang's `PotentiallyEvaluatedIfUsed` context): the use
        // is recorded with the argument and noted by
        // [`Sema::queue_default_arg_uses`].
        let cur = self.func.as_ref().map(|s| s.decl);
        if let Some((at, uses)) = self.inst.default_arg_uses.as_mut()
            && *at == cur
        {
            uses.push((d, loc));
            return;
        }
        if self.undefined_used_seen.insert(first) {
            self.undefined_used.push((first, loc));
        }
    }

    /// The start of a default argument's parse: the mark to pass to
    /// [`Sema::end_default_arg_uses`].
    pub fn begin_default_arg_uses(&self) -> usize {
        self.undefined_used.len()
    }

    /// The end of a default argument's parse, `arg` the stored argument
    /// (`None` when it is invalid). A default argument is potentially
    /// evaluated only where a call uses it ([dcl.fct.default]/5, clang's
    /// `PotentiallyEvaluatedIfUsed` context), so the undefined entities
    /// it used since `mark` are moved to the argument and noted again by
    /// [`Sema::queue_default_arg_uses`] at each call that uses it.
    pub fn end_default_arg_uses(&mut self, mark: usize, arg: Option<kiln_ast::ExprId>) {
        if mark >= self.undefined_used.len() {
            return;
        }
        let moved = self.undefined_used.split_off(mark);
        for (d, _) in &moved {
            self.undefined_used_seen.remove(d);
        }
        if let Some(arg) = arg {
            self.inst.deferred_default_uses.entry(arg).or_default().extend(moved);
        }
    }

    /// Whether function or variable `first` has a definition (a deleted
    /// or defaulted function counts, as in clang `FunctionDecl::isDefined`).
    fn entity_defined(&self, first: DeclId) -> bool {
        match self.ast.decls.get(first).kind() {
            DeclKind::Var => self.ast.redecls_of(first).any(|r| self.is_var_definition(r)),
            k if k.is_function() => {
                self.ast.definition(first).is_some()
                    || self.ast.redecls_of(first).any(|r| {
                        self.ast.decls.function(r).is_some_and(|x| {
                            x.spec.defaulted() != Defaulted::No || x.spec.is_deleted()
                        })
                    })
            }
            _ => true,
        }
    }

    /// clang `checkUndefinedButUsed`, at the end of the translation unit.
    pub(crate) fn diagnose_undefined_used(&mut self) {
        let list = std::mem::take(&mut self.undefined_used);
        self.undefined_used_seen = Default::default();
        if list.is_empty() || self.diags.error_count() > 0 {
            return;
        }
        for (d, use_loc) in list {
            let Some(kind) = self.undefined_used_kind(d) else { continue };
            let n = *self.ast.decls.get(d);
            let is_var = u8::from(n.kind() == DeclKind::Var);
            let name = match self.ast.spec_info(d) {
                Some((_, i)) if n.kind().is_function() => {
                    self.function_spec_name(d, i.template, i.args)
                }
                _ => self.qualified_decl_name(d),
            };
            let msg = match kind {
                Undefined::NoLinkageType => {
                    self.diag(DiagId::sema_undefined_internal_type, n.loc()).arg(is_var).arg(name)
                }
                Undefined::Internal => {
                    self.diag(DiagId::sema_undefined_internal, n.loc()).arg(is_var).arg(name)
                }
                Undefined::InlineFunction => {
                    self.diag(DiagId::sema_undefined_inline, n.loc()).arg(name)
                }
                Undefined::InlineVar => {
                    self.diag(DiagId::sema_undefined_inline_var, n.loc()).arg(name)
                }
            };
            // Each specialization of one template is diagnosed at the
            // template's location.
            let msg = msg.allow_duplicate().with_note(self.diag(DiagId::note_used_here, use_loc));
            self.emit(msg);
        }
    }

    /// Whether `d` is declared in namespace `::std` or a namespace nested
    /// in it.
    fn in_std_tree(&self, d: DeclId) -> bool {
        let std = self.intern("std");
        let mut c = self.ast.decls.get(d).ctx();
        for _ in 0..64 {
            let Some(ns) = c.map(|x| self.ast.redecl_context(x)) else { return false };
            if self.kind_of(ns) == DeclKind::TranslationUnit {
                return false;
            }
            let parent = self.ast.decls.get(ns).ctx().map(|p| self.ast.redecl_context(p));
            if self.kind_of(ns) == DeclKind::Namespace
                && self.ast.decls.get(ns).name().as_ident() == Some(std)
                && parent.is_some_and(|p| self.kind_of(p) == DeclKind::TranslationUnit)
            {
                return true;
            }
            c = parent;
        }
        false
    }

    /// clang `getUndefinedButUsed` and the classification of
    /// `checkUndefinedButUsed` for first declaration `d`.
    fn undefined_used_kind(&self, d: DeclId) -> Option<Undefined> {
        let n = *self.ast.decls.get(d);
        let kind = n.kind();
        let is_var = kind == DeclKind::Var;
        if (!is_var && !kind.is_function()) || kind == DeclKind::CXXDeductionGuide {
            return None;
        }
        let redecls: Vec<DeclId> = self.ast.redecls_of(d).collect();
        if redecls.iter().any(|&r| {
            self.ast.decls.get(r).has(DeclFlags::INVALID)
                || self.find_attr(r, AttrKind::WEAKREF).is_some()
                || self.find_attr(r, AttrKind::ALIAS).is_some()
        }) || self.entity_defined(d)
        {
            return None;
        }
        // A static data member with an in-class initializer is used as a
        // constant without a definition (clang skips it at the use).
        if is_var
            && redecls.iter().any(|&r| self.ast.decls.var(r).is_some_and(|v| v.init.is_some()))
        {
            return None;
        }
        // A pure virtual function is odr-used only by a qualified call
        // ([basic.def.odr]/7); its virtual calls are not uses. kiln does
        // not tell them apart here, so it is never diagnosed.
        if !is_var
            && (self.ast.decls.builtin_id(d).is_some()
                || self.ast.decls.function(d).is_some_and(|x| x.spec.is_pure()))
        {
            return None;
        }
        // The evaluator supplies some library functions declared without
        // a definition (the `std::meta` metafunctions, a `constexpr`
        // `std::uncaught_exceptions`), so nothing in `std` is diagnosed.
        if self.in_std_tree(d) {
            return None;
        }
        let inline = if is_var {
            redecls.iter().any(|&r| self.ast.decls.var(r).is_some_and(|v| v.spec.is_inline()))
        } else {
            redecls.iter().all(|&r| self.find_attr(r, AttrKind::GNU_INLINE).is_none())
                && redecls.iter().any(|&r| {
                    self.ast.decls.function(r).is_some_and(|x| {
                        x.spec.is_inline()
                            || x.spec.implicit_inline()
                            || x.spec.constexpr_kind() != ConstexprKind::None
                    })
                })
        };
        // clang `isExternalWithNoLinkageType`: external formal linkage, not
        // `extern "C"`, and a type that is not externally visible.
        if !mangle::has_internal_linkage(&self.ast, d)
            && !mangle::is_extern_c(&self.ast, d)
            && n.ty().is_some_and(|t| !mangle::type_externally_visible(&self.ast, t))
        {
            return Some(Undefined::NoLinkageType);
        }
        if mangle::is_internal_symbol(&self.ast, d) {
            return Some(Undefined::Internal);
        }
        match (inline, is_var) {
            (true, false) => Some(Undefined::InlineFunction),
            (true, true) => Some(Undefined::InlineVar),
            _ => None,
        }
    }
}
