//! The erroneous value of an automatic variable without an initializer
//! (C++26 [basic.indet]/1, P2795R5; WP-448): clang's
//! `-ftrivial-auto-var-init=zero` (`initializeWhatIsTechnicallyUninitialized`
//! and `emitZeroOrPatternForAutoVarInit` in `CGDecl.cpp`), which kiln
//! applies by default in C++26 (spec/codegen.md §7q).

use kiln_ast::DeclId;
use kiln_ast::attr::AttrKind;
use kiln_diag::{Diag, DiagId};
use kiln_source::Span;

use crate::AutoVarInit;
use crate::fir::{Fir, V};
use crate::func::Addr;
use crate::module::Cg;

impl Cg<'_> {
    /// Writes the erroneous value (all-zero bytes, padding included) of
    /// the automatic variable `d` at `addr`, which has no initializer or a
    /// trivial default constructor, unless the variable is
    /// `[[indeterminate]]` ([dcl.attr.indet]) or larger than
    /// `-ftrivial-auto-var-init-max-size` (then a `-Rauto-var-init`
    /// remark). A scalar or vector gets one zero store, anything else a
    /// `memset`; either carries clang's `!annotation !{!"auto-init"}`.
    pub(crate) fn auto_var_init(&mut self, d: DeclId, addr: Addr) {
        if self.opts.auto_var_init != AutoVarInit::Zero || self.f.fir.cur().is_none() {
            return;
        }
        let ast = self.ast;
        let marked = ast.redecls_of(d).any(|r| {
            ast.lists
                .iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r))
                .any(|a| ast.attrs.get(a).kind == AttrKind::INDETERMINATE)
        });
        if marked {
            return;
        }
        let Some(q) = ast.decls.get(d).ty() else { return };
        if self.referee(q).is_some() {
            return;
        }
        let size = self.alloc_size(addr.ty);
        if size == 0 {
            return;
        }
        let max = self.opts.auto_var_init_max_size;
        if max > 0 && size > max {
            let name = ast.decls.get(d).name().as_ident().map_or("", |n| ast.symbols().as_str(n));
            let span = Span::empty(ast.decls.get(d).loc());
            let r = Diag::new(DiagId::cg_auto_var_init_too_large, span)
                .arg(format!("'{name}'"))
                .arg(size)
                .arg(max);
            self.diags.push(r);
            return;
        }
        let vol = self.is_volatile(q);
        let t = addr.ty;
        let types = &self.m.types;
        let inst = if types.is_int_or_int_vector(t)
            || types.is_ptr_or_ptr_vector(t)
            || types.is_float_or_float_vector(t)
        {
            let k = self.m.const_zero(t);
            self.store(V::C(k), addr.v, addr.align, vol)
        } else {
            self.memset(addr, 0, size, vol);
            let Some(&i) = self.f.fir.cur().and_then(|b| self.f.fir.block_insts(b).last()) else {
                return;
            };
            V::I(i)
        };
        let s = self.m.md_string(b"auto-init");
        let node = self.m.md_tuple(&[Some(s)]);
        let kind = self.m.md_kind("annotation");
        self.f.fir.add_md(Fir::last_inst(inst), kind, node);
    }
}
