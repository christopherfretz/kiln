//! GNU `cleanup(f)` on a local variable (clang `EmitAutoVarCleanups` /
//! `CallCleanupFunction`, `AttrDocs` "cleanup", spec/landed/codegen/WP-807.md):
//! a `NormalAndEHCleanup` pushed after the variable's destructor cleanup,
//! so `f(&v)` runs first on every exit from the scope (fallthrough,
//! `return`, `break` / `continue`, `goto`, and unwinding).

use kiln_abi::mangle;
use kiln_ast::attr::AttrKind as AstAttr;
use kiln_ast::ids::RawId;
use kiln_ast::types::BuiltinType;
use kiln_ast::{AttrId, DeclId};
use kiln_llvm::{Attr, AttrKind, Value};

use crate::cleanup::Cleanup;
use crate::fir::V;
use crate::func::CallArg;
use crate::module::{Cg, R, err};

impl Cg<'_> {
    /// The function of local variable `d`'s checked `cleanup` attribute.
    pub(crate) fn cleanup_attr_fn(&self, d: DeclId) -> Option<DeclId> {
        let ast = self.ast;
        ast.lists
            .iter::<AttrId>(ast.attrs.of_decl(d))
            .map(|a| *ast.attrs.get(a))
            .find(|n| n.kind == AstAttr::CLEANUP)
            .and_then(|n| DeclId::from_raw(n.a))
    }

    /// Pushes the `cleanup` attribute's cleanup of local variable `d`,
    /// if it has one (clang `EmitAutoVarCleanups`, after the destructor).
    pub(crate) fn push_cleanup_attr(&mut self, d: DeclId) {
        if self.f.fir.cur().is_none() {
            return;
        }
        let Some(func) = self.cleanup_attr_fn(d) else { return };
        let Some(addr) = self.f.locals.get(&d).map(|a| a.v) else { return };
        // clang declares the function here (`GetAddrOfFunction`), before
        // the scope's calls; a failure is reported by the call.
        let _ = self.get_fn(func);
        self.push_cleanup(Cleanup::AttrFn { func, addr });
    }

    /// `CallCleanupFunction::Emit`: calls `func` with the address `addr`
    /// of the variable, as an argument of the parameter's type (a
    /// `bool` parameter receives `true`: the address is not null).
    pub(crate) fn emit_cleanup_fn_call(&mut self, func: DeclId, addr: V) -> R<()> {
        let ast = self.ast;
        let loc = Some(ast.decls.get(func).loc());
        let Some(fty) = ast.decls.get(func).ty() else {
            return err(loc, "cleanup function without a type");
        };
        let Some(&p0) = ast.types.proto(fty).and_then(|p| ast.types.params(p).first()) else {
            return err(loc, "cleanup function without a parameter");
        };
        let f = self.get_fn(func)?;
        let mut sig = self.call_sig(fty, mangle::is_extern_c(ast, ast.first(func)), &[p0]);
        let mut extra = Vec::new();
        let const_pure = crate::builtins::const_pure_attrs(ast, func, &sig, &mut extra);
        sig.nothrow |= const_pure || ast.has_nothrow_attr(func);
        let (site, unreachable) = self.noreturn_call(Some(func), fty);
        if site {
            extra.push(Attr::Flag(AttrKind::NoReturn));
        }
        let arg = if ast.types.builtin(p0) == Some(BuiltinType::Bool) {
            V::C(self.m.const_bool(true))
        } else {
            addr
        };
        let callee = V::C(Value::func(f));
        let vals = [CallArg::Scalar(arg)];
        let from = self.f.calls.len();
        self.call_vals(&sig, callee, Some(f), None, &vals, &[], None, &extra)?;
        if unreachable {
            // clang `EmitCall` without a location.
            self.finish_noreturn_call(from, Some(f), None)?;
        }
        Ok(())
    }
}
