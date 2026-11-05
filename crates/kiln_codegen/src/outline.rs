//! Statement-chunk outlining of oversized compound statements (WP-641,
//! spec/landed/codegen/WP-641.md).
//!
//! Some LLVM function passes (jump threading's block merging, the
//! memset merging of `MemCpyOptPass`) are super-linear in the size of ONE
//! function. A compound statement holding thousands of expression
//! statements (a generated registration function: `t.def(...);
//! t.def(...); ...`) makes one function that `opt -O2` takes minutes on,
//! although every statement is independent. Above a size threshold kiln emits runs of such statements
//! into internal `noinline` helpers, each called from the original
//! function with the addresses of the locals it names (and `this` and the
//! closure object): the giant function becomes a few hundred calls, every
//! helper is small, and each pass is linear in their total.
//!
//! The transformation is invisible: the helpers have internal linkage, an
//! exception thrown in one unwinds out of it after its own temporaries'
//! cleanups to the caller's landing pad (the same cleanup order as the
//! inline emission), and a compound below the threshold is emitted
//! exactly as before.

use kiln_ast::decl::DeclKind;
use kiln_ast::decl::builtin as id;
use kiln_ast::expr::ExprKind;
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, ExprId, LambdaId, StmtId};
use kiln_llvm::{Attr, AttrKind, CallOpts, FuncId, Linkage, TypeId as LTy, Value};
use kiln_source::fx::FxHashSet;

use crate::cleanup::JumpDest;
use crate::fir::{Fir, Op, V};
use crate::func::{Addr, Fx};
use crate::module::{Cg, DefFnAttrs, DefInline, R};

/// A compound statement is outlined when it directly holds at least this
/// many outlinable expression statements (measured,
/// spec/landed/codegen/WP-641.md: below it no function is slow enough to
/// matter).
pub(crate) const MIN_STMTS: usize = 256;
/// Each helper holds at most this many statements.
pub(crate) const CHUNK: usize = 32;
/// A run of consecutive outlinable statements shorter than this stays
/// inline (a call per one or two statements gains nothing).
pub(crate) const MIN_RUN: usize = 4;

/// Functions whose call may return twice (clang `ReturnsTwice` by name):
/// a helper would return before the second return.
const RETURNS_TWICE: &[&str] =
    &["setjmp", "_setjmp", "sigsetjmp", "__sigsetjmp", "savectx", "vfork", "getcontext"];

/// What a helper receives: the addresses of the caller's locals it names,
/// then `this`, then the closure object.
struct Captures {
    locals: Vec<(DeclId, Addr)>,
    this: Option<V>,
    closure: Option<(V, LambdaId)>,
}

impl Cg<'_> {
    /// Emits the statements `list` of a compound statement, outlining
    /// runs of them into helpers when the compound is large enough.
    pub(crate) fn emit_stmt_list(&mut self, list: &[StmtId]) -> R<()> {
        let Some(chunks) = self.outline_plan(list) else {
            for &s in list {
                self.emit_stmt(s)?;
            }
            return Ok(());
        };
        let mut i = 0;
        for (start, end) in chunks {
            for &s in &list[i..start] {
                self.emit_stmt(s)?;
            }
            self.emit_outlined(&list[start..end])?;
            i = end;
        }
        for &s in &list[i..] {
            self.emit_stmt(s)?;
        }
        Ok(())
    }

    /// The `[start, end)` ranges of `list` to outline, or `None` to emit
    /// the compound inline.
    fn outline_plan(&self, list: &[StmtId]) -> Option<Vec<(usize, usize)>> {
        if list.len() < MIN_STMTS || !self.outline_enabled() {
            return None;
        }
        let ok: Vec<bool> = list.iter().map(|&s| self.stmt_outlinable(s)).collect();
        if ok.iter().filter(|&&b| b).count() < MIN_STMTS {
            return None;
        }
        let mut chunks = Vec::new();
        let mut i = 0;
        while i < ok.len() {
            if !ok[i] {
                i += 1;
                continue;
            }
            let start = i;
            while i < ok.len() && ok[i] {
                i += 1;
            }
            let len = i - start;
            if len < MIN_RUN {
                continue;
            }
            // Balanced chunks of at most CHUNK statements.
            let n = len.div_ceil(CHUNK);
            for k in 0..n {
                chunks.push((start + k * len / n, start + (k + 1) * len / n));
            }
        }
        (!chunks.is_empty()).then_some(chunks)
    }

    /// The function being emitted may have its statements outlined: it is
    /// optimized, and none of its state a helper could not reproduce is
    /// live (a coroutine, a structor, an explicit object parameter, a VLA
    /// bound, a conditional or opaque-value context).
    fn outline_enabled(&self) -> bool {
        let f = &self.f;
        let Some(d) = f.decl else { return false };
        self.opts.optimize()
            && f.func.is_some()
            && !f.naked
            && f.coro.is_none()
            && f.await_frame.is_none()
            && f.structor.is_none()
            && f.closure_obj.is_none()
            && f.inh_args.is_none()
            && f.vla_sizes.is_empty()
            && f.conds.is_empty()
            && f.opaque.is_empty()
            && f.array_index.is_empty()
            && self.def_inline_attrs(d) != DefInline::OptNone
    }

    /// Whether `s` is an expression statement worth outlining and that a
    /// helper can evaluate: it calls or constructs something (a statement
    /// of plain arithmetic on locals stays inline, where SROA keeps the
    /// locals in registers), and has no statement expression, label
    /// address, `co_await`, splice or unexpanded construct, no call of a
    /// builtin bound to its function's frame (`alloca`, `va_start`, frame
    /// or return address), of `__builtin_object_size` (a helper sees a
    /// parameter, not the caller's object) or of a function that returns
    /// twice.
    fn stmt_outlinable(&self, s: StmtId) -> bool {
        let ast = self.ast;
        if ast.stmts.get(s).kind() != StmtKind::ExprStmt {
            return false;
        }
        let mut stack = vec![ast.stmts.expr_stmt_expr(s)];
        let mut kids = Vec::new();
        let mut calls = false;
        while let Some(e) = stack.pop() {
            let k = ast.exprs.get(e).kind();
            calls |= matches!(
                k,
                ExprKind::CallExpr
                    | ExprKind::CXXMemberCallExpr
                    | ExprKind::CXXOperatorCallExpr
                    | ExprKind::CXXConstructExpr
                    | ExprKind::CXXTemporaryObjectExpr
                    | ExprKind::CXXNewExpr
                    | ExprKind::CXXDeleteExpr
                    | ExprKind::CXXThrowExpr
            );
            match k {
                ExprKind::StmtExpr
                | ExprKind::AddrLabelExpr
                | ExprKind::CoawaitExpr
                | ExprKind::CoyieldExpr
                | ExprKind::DependentCoawaitExpr
                | ExprKind::CXXSpliceExpr
                | ExprKind::UserDefinedLiteral
                | ExprKind::PackIndexingExpr
                | ExprKind::CXXExpansionSelectExpr
                | ExprKind::RecoveryExpr
                | ExprKind::UnresolvedLookupExpr
                | ExprKind::UnresolvedMemberExpr
                | ExprKind::DependentScopeDeclRefExpr
                | ExprKind::CXXDependentScopeMemberExpr
                | ExprKind::SubstNonTypeTemplateParmPackExpr
                | ExprKind::FunctionParmPackExpr
                | ExprKind::PackExpansionExpr
                | ExprKind::CXXFoldExpr
                | ExprKind::CXXUnresolvedConstructExpr
                | ExprKind::ParenListExpr
                | ExprKind::CXXInheritedCtorInitExpr => return false,
                ExprKind::CallExpr if !self.call_outlinable(e) => return false,
                ExprKind::DeclRefExpr => {
                    let d = ast.exprs.decl(e);
                    if ast.decls.get(d).kind() == DeclKind::Binding
                        && let Some(b) = ast.decls.binding_expr(d)
                    {
                        stack.push(b);
                    }
                }
                _ => {}
            }
            kids.clear();
            ast.exprs.children_into(e, &ast.lists, &mut kids);
            stack.extend_from_slice(&kids);
        }
        calls
    }

    /// A call `e` that may run in a helper.
    fn call_outlinable(&self, e: ExprId) -> bool {
        let ast = self.ast;
        if let Some((_, b)) = crate::builtins::builtin_callee(ast, e)
            && matches!(
                b,
                id::ALLOCA
                    | id::ALLOCA_WITH_ALIGN
                    | id::VA_START
                    | id::FRAME_ADDRESS
                    | id::RETURN_ADDRESS
                    | id::OBJECT_SIZE
                    | id::DYNAMIC_OBJECT_SIZE
            )
        {
            return false;
        }
        let mut c = ast.exprs.callee(e);
        while matches!(ast.exprs.get(c).kind(), ExprKind::ParenExpr | ExprKind::ImplicitCastExpr) {
            c = ast.exprs.operand(c);
        }
        if ast.exprs.get(c).kind() == ExprKind::DeclRefExpr {
            let d = ast.exprs.decl(c);
            if ast.decls.get(d).kind() == DeclKind::Function
                && RETURNS_TWICE.contains(&kiln_abi::mangle::decl_name(ast, d).as_ref())
            {
                return false;
            }
        }
        true
    }

    /// The locals of the current function the statements `stmts` name
    /// (in first-reference order), with `this` and the closure object.
    fn outline_captures(&self, stmts: &[StmtId]) -> Captures {
        let ast = self.ast;
        let mut seen = FxHashSet::default();
        let mut locals = Vec::new();
        let mut kids = Vec::new();
        for &s in stmts {
            let mut stack = vec![ast.stmts.expr_stmt_expr(s)];
            while let Some(e) = stack.pop() {
                if ast.exprs.get(e).kind() == ExprKind::DeclRefExpr {
                    let d = ast.exprs.decl(e);
                    if let Some(&a) = self.f.locals.get(&d) {
                        if seen.insert(d) {
                            locals.push((d, a));
                        }
                    } else if ast.decls.get(d).kind() == DeclKind::Binding
                        && let Some(b) = ast.decls.binding_expr(d)
                    {
                        stack.push(b);
                    }
                }
                kids.clear();
                ast.exprs.children_into(e, &ast.lists, &mut kids);
                // Reversed: the walk visits children in source order.
                stack.extend(kids.iter().rev());
            }
        }
        Captures { locals, this: self.f.this, closure: self.f.closure }
    }

    /// Emits `stmts` into a new helper and calls it.
    fn emit_outlined(&mut self, stmts: &[StmtId]) -> R<()> {
        if self.f.fir.cur().is_none() {
            // Unreachable statements without labels emit nothing.
            return Ok(());
        }
        let cap = self.outline_captures(stmts);
        let mut args: Vec<V> = cap.locals.iter().map(|&(_, a)| a.v).collect();
        args.extend(cap.this);
        args.extend(cap.closure.map(|(v, _)| v));
        let cur = self.f.func.expect("current function");
        let name = format!("{}.stmts.{}", self.m.func(cur).name(), self.f.outlined);
        self.f.outlined += 1;
        let ptys = vec![LTy::PTR; args.len()];
        let fty = self.m.types.function(LTy::VOID, &ptys, false);
        let Ok(f) = self.m.add_function(&name, fty) else {
            for &s in stmts {
                self.emit_stmt(s)?;
            }
            return Ok(());
        };
        let pa = self.m.attr_set(&[Attr::Flag(AttrKind::NonNull), Attr::Flag(AttrKind::NoUndef)]);
        {
            let func = self.m.func_mut(f);
            func.linkage = Linkage::Internal;
            func.attrs.params = vec![pa; ptys.len()];
        }
        let decl = self.f.decl;
        let this_class = self.f.this_class;
        let saved = std::mem::replace(&mut self.f, Fx::empty());
        let r = self.emit_outlined_body(f, ptys, stmts, &cap, decl, this_class);
        self.f = saved;
        let noreturn = r?;
        let opts = CallOpts::default();
        self.emit_call_op(fty, V::C(Value::func(f)), args, opts, LTy::VOID, Some(f), false);
        if noreturn {
            self.f.fir.push(Op::Unreachable, LTy::VOID);
            self.f.fir.set_cur(None);
        }
        Ok(())
    }

    /// `StartFunction`, the statements and `FinishFunction` of helper
    /// `f`; whether no path returns.
    fn emit_outlined_body(
        &mut self,
        f: FuncId,
        ptys: Vec<LTy>,
        stmts: &[StmtId],
        cap: &Captures,
        decl: Option<DeclId>,
        this_class: Option<DeclId>,
    ) -> R<bool> {
        self.f.fir = Fir::new(ptys);
        self.f.func = Some(f);
        self.f.decl = decl;
        self.f.this_class = this_class;
        let mut arg = 0u32;
        for &(d, a) in &cap.locals {
            self.f.locals.insert(d, Addr { v: V::Arg(arg), ..a });
            arg += 1;
        }
        if cap.this.is_some() {
            self.f.this = Some(V::Arg(arg));
            arg += 1;
        }
        if let Some((_, l)) = cap.closure {
            self.f.closure = Some((V::Arg(arg), l));
        }
        let ret_block = self.f.fir.create_block();
        self.f.ret_block = Some(ret_block);
        self.f.ret_dest = Some(JumpDest { blk: ret_block, depth: Some(0), index: 1 });
        self.f.cl.next_index = 2;
        self.f.cl.prologue = self.f.cl.depth();
        for &s in stmts {
            self.emit_stmt(s)?;
        }
        let noreturn = self.f.fir.cur().is_none();
        let prologue = self.f.cl.prologue;
        self.pop_cleanup_blocks(prologue);
        self.emit_return_block();
        self.emit_epilog();
        self.finish_eh();
        self.emit_unreachable_if_used();
        if let Some(e) = self.f.error.take() {
            return Err(e);
        }
        if let Some(rb) = self.f.ret_block
            && self.f.fir.is_placed(rb)
            && self.f.fir.block_uses(rb) == 0
        {
            self.f.fir.erase_block(rb);
            self.f.ret_block = None;
        }
        let nounwind = !self.opts.exceptions || self.no_throw_calls();
        let mut extra = Vec::new();
        if let Some(d) = decl {
            self.def_opt_attrs(d, DefInline::NoInline, &mut extra);
        }
        let fa = self.def_fn_attrs(DefFnAttrs {
            decl,
            is_main: false,
            nounwind,
            mustprogress: self.f.mustprogress,
            vector_width: self.f.vector_width,
            noreturn,
            inline: DefInline::NoInline,
            extra,
        });
        self.m.func_mut(f).attrs.func = fa;
        self.nounwind.insert(f, nounwind);
        let fir = std::mem::replace(&mut self.f.fir, Fir::new(Vec::new()));
        let loc = decl.map(|d| self.ast.decls.get(d).loc());
        if self.f.uses_ph {
            let Some(d) = decl else {
                return crate::module::err(loc, "outlined statements outside a function");
            };
            self.pending.push((f, d, fir));
            return Ok(noreturn);
        }
        if let Err(e) = fir.lower(&mut self.m, f, &self.phs) {
            return crate::module::err(loc, format!("IR lowering failed: {e:?}"));
        }
        Ok(noreturn)
    }
}
