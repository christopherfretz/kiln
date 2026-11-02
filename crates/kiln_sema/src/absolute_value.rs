//! `-Wabsolute-value` (WP-310): clang's `CheckAbsoluteValueFunction`, the
//! unsigned-argument case (spec/sema.md §5.18).
//!
//! A one-argument call of an absolute value function -- a builtin
//! (`__builtin_abs`, `__builtin_labs`, `__builtin_llabs`, `__builtin_fabs*`,
//! `__builtin_cabs*`), a declaration recognized as the library builtin
//! (`abs`, `labs`, `llabs`, `fabs*`; [`Sema::lib_builtin_info`]) or a
//! function named `abs` in namespace `std` -- whose argument, implicit
//! conversions and parentheses ignored, has an unsigned integer type
//! (clang `isUnsignedIntegerType`: `bool`, the unsigned and
//! unsigned-character types, unsigned `_BitInt`, unscoped enumerations with
//! an unsigned underlying type): "taking the absolute value of unsigned
//! type 'T' has no effect" at the call, with the note "remove the call to
//! 'abs' since unsigned values cannot be negative" over the callee
//! (`std::abs` for the `std` function). `std::abs(us)` with libc++ resolves
//! to the C library's `::abs(int)` (a `using`-declaration in `std`), so its
//! note names `abs`.
//!
//! clang's other `CheckAbsoluteValueFunction` warnings (pointer argument,
//! wrong kind of function, too-small parameter) are not implemented.

use kiln_ast::decl::{DeclKind, builtin};
use kiln_ast::{DeclId, ExprId};
use kiln_diag::DiagId;

use crate::Sema;

/// clang `getAbsoluteValueFunctionKind`: the builtin and library-builtin
/// ids of the absolute value functions (clang's `Builtin::ID`s).
const ABS_IDS: &[u32] = &[
    builtin::ABS,
    builtin::LABS,
    builtin::LLABS,
    builtin::FABS,
    builtin::FABSF,
    builtin::FABSL,
    // __builtin_cabsf, __builtin_cabs, __builtin_cabsl
    237,
    239,
    241,
    // abs, labs, llabs, fabsf, fabs, fabsl
    93,
    95,
    97,
    549,
    551,
    553,
];

impl Sema<'_, '_> {
    /// `-Wabsolute-value` for the call of `f` through `callee` with the
    /// converted arguments `args` (clang `CheckAbsoluteValueFunction`).
    pub(crate) fn check_absolute_value(&mut self, f: DeclId, callee: ExprId, args: &[ExprId]) {
        let [arg] = args else { return };
        let Some(name) = self.abs_function_name(f) else { return };
        let aty = self.ety(self.ignore_paren_imp_casts(*arg));
        if self.is_error_type(aty) || !self.is_unsigned_integer_type(aty) {
            return;
        }
        let at = self.begin_loc(callee);
        let note = self
            .diag(DiagId::note_remove_abs, at)
            .arg(name)
            .range(self.expr_range(callee));
        let msg = self.diag(DiagId::sema_unsigned_abs, at).arg(self.ty_arg(aty));
        self.emit(msg.with_note(note));
    }

    /// The name clang's note gives the absolute value function `f`
    /// (`std::abs`, else the builtin's spelling), or `None` when `f` is not
    /// one.
    fn abs_function_name(&mut self, f: DeclId) -> Option<&'static str> {
        if self.kind_of(f) != DeclKind::Function {
            return None;
        }
        let abs = self.intern("abs");
        if self.ast.decls.get(f).name().as_ident() == Some(abs) && self.is_in_std_namespace(f) {
            return Some("std::abs");
        }
        if let Some(id) = self.ast.decls.builtin_id(f) {
            return ABS_IDS.contains(&id).then(|| builtin::info(id).map(|i| i.name)).flatten();
        }
        self.lib_builtin_info(f).filter(|i| ABS_IDS.contains(&i.id)).map(|i| i.name)
    }

    /// clang `Decl::isInStdNamespace`: `d`'s context, linkage
    /// specifications and inline namespaces skipped, is the namespace
    /// `::std`.
    fn is_in_std_namespace(&self, d: DeclId) -> bool {
        let std = self.intern("std");
        let mut c = self.ast.decls.get(d).ctx();
        for _ in 0..64 {
            let Some(ns) = c.map(|x| self.ast.redecl_context(x)) else { return false };
            if self.kind_of(ns) != DeclKind::Namespace {
                return false;
            }
            let parent = self.ast.decls.get(ns).ctx().map(|p| self.ast.redecl_context(p));
            if self.ast.decls.get(ns).name().as_ident() == Some(std)
                && parent.is_some_and(|p| self.kind_of(p) == DeclKind::TranslationUnit)
            {
                return true;
            }
            let inline = self
                .ast
                .decls
                .namespace(self.ast.first(ns))
                .is_some_and(kiln_ast::decl::NamespaceData::is_inline);
            if !inline {
                return false;
            }
            c = self.ast.decls.get(ns).ctx();
        }
        false
    }
}
