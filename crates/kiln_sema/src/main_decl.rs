//! Declaration rules for `main` ([basic.start.main]/2-3, WP-1015): the
//! parts of clang's `CheckMain` beyond the specifier and return-type
//! checks in `act_on_function`, the `main` variable rule of
//! `ActOnVariableDeclarator`, and the deleted-`main` rule of
//! `SetDeclDeleted` (spec/sema.md §4.4.1 "The main function"). A
//! freestanding implementation has no `main` rules ([basic.start.main]/1).

use kiln_ast::decl::{DeclFlags, DeclKind, FnStorage, VarStorage};
use kiln_ast::expr::ExprKind;
use kiln_ast::names::DeclName;
use kiln_ast::types::FuncProto;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;

impl Sema<'_, '_> {
    /// Whether `name` declared in `ctx` is the global `main` of a hosted
    /// implementation (clang's `FunctionDecl::isMain` / the variable
    /// test of `ActOnVariableDeclarator`).
    pub(crate) fn is_global_main_name(&self, ctx: DeclId, name: Symbol) -> bool {
        !self.freestanding
            && self.ast.symbols().as_str(name) == "main"
            && self.ast.redecl_context(ctx) == self.ast.tu
    }

    /// clang 22's `FunctionDecl::isMain` beyond the global scope: a
    /// non-`static` function named `main` declared (lexically) in an
    /// `extern "C"` context of any namespace ([basic.start.main]/3: "an
    /// entity named `main` with C language linkage"). clang 19 treats it
    /// as an ordinary function.
    pub(crate) fn is_c_linkage_main_name(&self, lexical: DeclId, name: Symbol, st: bool) -> bool {
        self.clang_compat >= 22
            && !st
            && !self.freestanding
            && self.ast.symbols().as_str(name) == "main"
            && self.ctx_is_extern_c(lexical)
    }

    /// Whether function `f` is `main` (clang's `FunctionDecl::isMain`):
    /// the global one, or at level 22 one with C language linkage.
    pub(crate) fn is_main_fn(&self, f: DeclId) -> bool {
        let n = self.ast.decls.get(f);
        if n.kind() != DeclKind::Function {
            return false;
        }
        let (Some(s), Some(c)) = (n.name().as_ident(), n.ctx()) else { return false };
        if self.is_global_main_name(c, s) {
            return true;
        }
        let st = self.ast.decls.function(f).is_some_and(|x| x.spec.storage() == FnStorage::Static);
        self.ast.decls.lexical_ctx(f).is_some_and(|l| self.is_c_linkage_main_name(l, s, st))
    }

    /// The checks of clang's `CheckMain` that only warn: `extern "C"`
    /// (clang 22's `ext_main_invalid_linkage_specification`), variadic
    /// (`ext_variadic_main`), all at the name. Run before the errors, as
    /// clang reports them.
    pub(crate) fn warn_main_decl(&mut self, proto: &FuncProto, c_linkage: bool, loc: Loc) {
        if c_linkage {
            let w = self.diag(DiagId::sema_main_extern_c, loc);
            self.emit(w);
        }
        if proto.info().variadic() && !proto.info().no_proto() {
            let w = self.diag(DiagId::sema_main_variadic, loc);
            self.emit(w);
        }
    }

    /// clang `warn_main_one_arg`: `main` declared with exactly one
    /// parameter, when it is otherwise valid (at the name, after the
    /// parameter checks; a template is reported after it).
    pub(crate) fn warn_main_one_arg(&mut self, proto: &FuncProto, loc: Loc) {
        if self.ast.types.params(proto).len() == 1 {
            let w = self.diag(DiagId::sema_main_one_arg, loc);
            self.emit(w);
        }
    }

    /// clang `warn_main_returns_bool_literal`: `return true;` (the operand
    /// as written, a `bool` literal not in parentheses) in `main`, at
    /// `return`.
    pub(crate) fn warn_main_returns_bool(&mut self, f: DeclId, value: ExprId, loc: Loc) {
        if self.ast.exprs.get(value).kind() == ExprKind::CXXBoolLiteralExpr && self.is_main_fn(f) {
            let w = self.diag(DiagId::sema_main_returns_bool_literal, loc);
            self.emit(w);
        }
    }

    /// [basic.start.main]/3: "The function `main` shall not be named by
    /// an expression"; clang accepts it with the pedantic extension
    /// `ext_main_used` at the name (`-Wmain`, off by default).
    pub(crate) fn diagnose_main_use(&mut self, f: DeclId, loc: Loc) {
        if self.is_main_fn(f) {
            let w = self.diag(DiagId::sema_main_used, loc);
            self.emit(w);
        }
    }

    /// [basic.start.main]/2: `main` takes no parameters, or `(int,
    /// char**)`, or `(int, char**, char**)` (the implementation-defined
    /// third form clang accepts). Reported at the name; true when the
    /// declaration is invalid (clang `err_main_surplus_args`,
    /// `err_main_arg_wrong`).
    pub(crate) fn check_main_params(&mut self, proto: &FuncProto, loc: Loc) -> bool {
        let params = self.ast.types.params(proto).to_vec();
        let mut invalid = false;
        let mut n = params.len();
        if n > 3 {
            let m = self.diag(DiagId::sema_main_surplus_args, loc).arg(n.to_string());
            self.emit(m);
            invalid = true;
            n = 3;
        }
        for (i, &t) in params.iter().take(n).enumerate() {
            if self.is_error_type(t) || self.ast.types.is_dependent(t) {
                continue;
            }
            let ok = if i == 0 {
                self.ast.types.same_unqualified(t, QualType::INT)
            } else {
                self.is_main_char_pp(t)
            };
            if !ok {
                let m = self.diag(DiagId::sema_main_arg_wrong, loc).arg(i as u8).arg(i as u8);
                let m = m.arg(if i == 0 { "'int'" } else { "'char **'" }.to_owned());
                // One per parameter, all at the name.
                let m = m.allow_duplicate();
                self.emit(m);
                invalid = true;
            }
        }
        invalid
    }

    /// `char **`, or (clang's extension) the same with `const` at any
    /// level (`char const **`, `char * const *`, `const char* const*
    /// const`); no other qualifier.
    fn is_main_char_pp(&self, t: QualType) -> bool {
        let tys = &self.ast.types;
        let c = tys.canonical(t);
        let mut quals = c.quals().bits();
        let Some(p1) = tys.pointee(c).filter(|_| tys.is_pointer(c)) else { return false };
        let p1 = tys.canonical(p1);
        quals |= p1.quals().bits();
        let Some(p2) = tys.pointee(p1).filter(|_| tys.is_pointer(p1)) else { return false };
        let p2 = tys.canonical(p2);
        quals |= p2.quals().bits();
        p2.without_quals() == QualType::CHAR && quals & !kiln_ast::types::Quals::CONST.bits() == 0
    }

    /// `template<...> int main()` (clang `err_mainlike_template_decl`,
    /// at the name; only when `main` is otherwise valid).
    pub(crate) fn diagnose_main_template(&mut self, loc: Loc) {
        self.error(DiagId::sema_main_template, loc);
    }

    /// The first valid non-template `main` already declared in the
    /// translation unit: `main` is not overloaded ([basic.start.main]/3),
    /// so a declaration with another signature redeclares it (clang's
    /// `IsOverload` treats `main` as never overloaded; the merge then
    /// reports conflicting types).
    pub(crate) fn main_overload_prev(&self, name: Symbol) -> Option<DeclId> {
        let found = self.ast.lookup_local(self.ast.tu, DeclName::ident(name));
        let p = found.iter().find(|&x| {
            self.kind_of(x) == DeclKind::Function && !self.ast.decls.get(x).has(DeclFlags::INVALID)
        })?;
        Some(self.ast.first(p))
    }

    /// [basic.start.main]/3: a program that declares a variable `main`
    /// at global scope is ill-formed; so is a block-scope `extern`
    /// declaration of it in a function of the global namespace (its
    /// target scope), and (clang 22) a non-`static` namespace-scope one
    /// with C language linkage in any namespace. Not a variable template
    /// (clang accepts `template<class T> T main;`). Reported at the start
    /// of the decl-specifiers (clang `err_main_global_variable`, which
    /// clang 22 words for C linkage when the declaration has it).
    pub(crate) fn check_main_variable(
        &mut self,
        ctx: DeclId,
        name: Symbol,
        storage: VarStorage,
        loc: Loc,
    ) {
        if self.templ.cur.is_some()
            || self.freestanding
            || self.ast.symbols().as_str(name) != "main"
        {
            return;
        }
        let local = self.in_local_scope();
        if !local
            && storage != VarStorage::Static
            && self.clang_compat >= 22
            && self.ctx_is_extern_c(self.ctx)
        {
            self.error(DiagId::sema_main_c_linkage_variable, loc);
            return;
        }
        let global = if local {
            storage == VarStorage::Extern && self.enclosing_namespace() == Some(self.ast.tu)
        } else {
            self.ast.redecl_context(ctx) == self.ast.tu
        };
        if global {
            self.error(DiagId::sema_main_global_variable, loc);
        }
    }

    /// [basic.start.main]/3 (`main` is not deleted; clang
    /// `err_deleted_main`, at `delete`).
    pub(crate) fn check_deleted_main(&mut self, f: DeclId, loc: Loc) {
        if self.is_main_fn(f) {
            self.error(DiagId::sema_deleted_main, loc);
        }
    }
}
