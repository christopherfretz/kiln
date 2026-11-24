//! [module.import]/6: a header unit shall not contain a definition of a
//! non-inline function or variable whose name has external linkage
//! (spec/modules.md §3 "Header units"; WP-1800).
//!
//! clang 19 and 22 diagnose it while the header unit is built
//! (`err_extern_def_in_header_unit`, in `ActOnStartOfFunctionDef` and
//! `AddInitializerToDecl`): kiln does so in a `-fmodule-header` build and
//! in its in-process build of a header unit at an import. clang's test
//! leaves out more than the rule names, and kiln follows it: a variable
//! is checked only when it has an initializer (`int v;` is not), an
//! `extern` variable never (clang tests before the initializer is
//! attached), and a templated entity, an instantiation, a variable
//! template specialization, a static data member specialization of an
//! instantiated class, and a deleted or defaulted function are not
//! checked; nor, from clang 22 (`-fclang-compat=19` checks them), a
//! `weak` or `selectany` entity.

use kiln_ast::DeclId;
use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind, VarStorage};
use kiln_ast::expr::ExprKind;
use kiln_ast::modules::UnitKind;
use kiln_ast::templates::SpecKind;
use kiln_diag::DiagId;

use crate::Sema;

impl Sema<'_, '_> {
    /// The main file is a header unit being built (`-fmodule-header`,
    /// `-x c++-user-header`, ...; spec/modules.md §11.6): unit 0 is a
    /// header unit from its first token.
    pub fn act_on_main_header_unit(&mut self) {
        self.module_activate();
        if let Some(u) = self.ast.modules.units.first_mut() {
            u.kind = UnitKind::Header;
        }
    }

    /// Whether the declarations made now belong to a header unit (clang
    /// `currentModuleIsHeaderUnit`).
    pub(crate) fn in_header_unit(&self) -> bool {
        self.ast.modules.active
            && self
                .ast
                .modules
                .units
                .get(usize::from(self.module_cur_unit()))
                .is_some_and(|u| u.kind == UnitKind::Header)
    }

    /// [module.import]/6 for function `f` at the start of its body.
    pub(crate) fn check_header_unit_function_def(&mut self, f: DeclId) {
        if !self.in_header_unit()
            || self.ast.decls.get(f).has(DeclFlags::INVALID)
            || self.inst.stack.in_instantiation_record()
            || self.fn_inlined(f)
            || self.header_unit_exempt(f)
            || !self.has_external_linkage(f)
        {
            return;
        }
        let loc = self.ast.decls.get(f).loc();
        self.error(DiagId::sema_extern_def_in_header_unit, loc);
        self.invalidate(f);
    }

    /// [module.import]/6 for variable `v` as its initializer starts; true
    /// when it is diagnosed (the caller makes it invalid once the
    /// initializer is checked, as clang's `setInvalidDecl` does not stop
    /// `AddInitializerToDecl`).
    pub(crate) fn check_header_unit_var_def(&mut self, v: DeclId) -> bool {
        if !self.in_header_unit() {
            return false;
        }
        let n = *self.ast.decls.get(v);
        let decls = &self.ast.decls;
        let Some(spec) = decls.var(v).map(|x| x.spec) else { return false };
        // A static data member declared in its class is not a
        // definition there (an `inline` one is inline).
        let in_class = decls.lexical_ctx(v).is_some_and(|c| {
            matches!(
                self.kind_of(c),
                DeclKind::CXXRecord
                    | DeclKind::ClassTemplateSpecialization
                    | DeclKind::ClassTemplatePartialSpecialization
            )
        });
        if n.kind() != DeclKind::Var
            || n.has(DeclFlags::INVALID)
            || spec.is_inline()
            || spec.storage() == VarStorage::Extern
            || decls.is_single_line_linkage(v)
            || in_class
            || self.inst.stack.in_instantiation_record()
            || self.ast.spec_info(self.ast.first(v)).is_some()
            || self.ast.member_spec_info(self.ast.first(v)).is_some()
            || self.header_unit_exempt(v)
            || !self.has_external_linkage(v)
        {
            return false;
        }
        self.error(DiagId::sema_extern_def_in_header_unit, n.loc());
        true
    }

    /// A placeholder variable whose initializer names an invalid
    /// declaration or holds an error: clang's deduction fails on the
    /// error-dependent initializer and the variable is invalid, so
    /// [module.import]/6 is not checked for it. The walk is bounded.
    pub(crate) fn deduced_from_error(&self, v: DeclId) -> bool {
        let Some(init) = self.ast.decls.var(v).and_then(|x| x.init) else { return false };
        let mut stack = vec![init];
        let mut budget = 1usize << 14;
        while let Some(x) = stack.pop() {
            if budget == 0 {
                break;
            }
            budget -= 1;
            if self.is_error_expr(x) {
                return true;
            }
            if self.ast.exprs.get(x).kind() == ExprKind::DeclRefExpr {
                if self.ast.decls.get(self.ast.exprs.decl(x)).has(DeclFlags::INVALID) {
                    return true;
                }
                continue;
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        false
    }

    /// clang `FunctionDecl::isInlined()` at the start of a definition:
    /// some declaration says `inline`, or the function is implicitly
    /// inline (defined in its class, `constexpr`, `consteval`, deleted).
    fn fn_inlined(&self, f: DeclId) -> bool {
        self.ast.redecls_of(f).any(|r| {
            let s = self.fn_spec(r);
            s.is_inline()
                || s.implicit_inline()
                || s.constexpr_kind() != ConstexprKind::None
                || s.is_deleted()
        })
    }

    /// The exemptions shared by functions and variables: a templated
    /// entity (a template's pattern or a member of a templated class), an
    /// instantiation, and from clang 22 a `weak` or `selectany` entity.
    fn header_unit_exempt(&self, d: DeclId) -> bool {
        let first = self.ast.first(d);
        let instantiated = |k: SpecKind| k != SpecKind::ExplicitSpecialization;
        self.ast.described_template(first).is_some()
            || self.in_templated_class(self.ast.decls.get(d).ctx())
            || self.ast.spec_info(first).is_some_and(|(_, i)| instantiated(i.kind))
            || self.ast.member_spec_info(first).is_some_and(|(_, i)| instantiated(i.kind))
            || (self.clang_compat >= 22
                && (self.ast.is_weak_decl(d)
                    || self.ast.inherited_attr(d, AttrKind::SELECTANY).is_some()))
    }
}
