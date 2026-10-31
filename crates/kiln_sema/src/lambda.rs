//! Lambda expressions ([expr.prim.lambda]; WP-075, spec/sema-lambda.md).
//!
//! The parser drives a lambda through four entry points, in order:
//! [`Sema::act_on_lambda_start`] at the *lambda-introducer* creates the
//! closure class ([expr.prim.lambda.closure]/1) and starts its
//! definition; [`Sema::act_on_lambda_declarator`] after the
//! *lambda-declarator* creates the function call operator and opens its
//! body; [`Sema::act_on_lambda_end`] at the `}` closes the body, adds the
//! capture fields and the conversion to function pointer, completes the
//! closure and returns the `LambdaExpr`. [`Sema::act_on_lambda_error`]
//! abandons a lambda whose body could not be parsed.

use kiln_ast::decl::{Access, ConstexprKind, DeclFlags, FnSpec, FnStorage, TagKind};
use kiln_ast::ids::{AttrId, ExprId, LambdaId, StmtId};
use kiln_ast::lambda::{CaptureDefault, CaptureKind, LambdaData};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{AutoKeyword, ExcSpec, FuncInfo, Quals, RefQual, TypeView};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::declspec::{Chunk, Declarator, DefaultArg, ParamInfo};
use crate::scope::{Scope, ScopeKind};

/// One *capture* of a *lambda-capture* as written ([expr.prim.lambda.capture]/1).
#[derive(Clone, Debug)]
pub struct LambdaCaptureSpec {
    /// What is captured and how.
    pub kind: CaptureKind,
    /// The identifier of a simple or init capture.
    pub name: Option<(Symbol, Loc)>,
    /// Where the capture begins (`&`, `this`, `*` or the identifier).
    pub loc: Loc,
    /// The `...` of a pack capture.
    pub ellipsis: Option<Loc>,
    /// The initializer of an init-capture.
    pub init: Option<ExprId>,
}

/// A *lambda-introducer* ([expr.prim.lambda.general]/1).
#[derive(Clone, Debug, Default)]
pub struct LambdaIntroducer {
    /// The `[`.
    pub lbracket: Loc,
    /// The `]`.
    pub rbracket: Loc,
    /// The *capture-default*.
    pub default: CaptureDefault,
    /// Where the *capture-default* is written.
    pub default_loc: Loc,
    /// The explicit captures, in order.
    pub captures: Vec<LambdaCaptureSpec>,
}

/// The *lambda-specifier-seq* and what else the parser collected from
/// the *lambda-declarator* ([expr.prim.lambda.general]/1).
#[derive(Clone, Debug, Default)]
pub struct LambdaSpecifiers {
    /// `mutable`.
    pub mutable: Option<Loc>,
    /// `constexpr`.
    pub constexpr: Option<Loc>,
    /// `consteval`.
    pub consteval: Option<Loc>,
    /// `static`.
    pub static_: Option<Loc>,
    /// A parenthesized parameter list was written.
    pub explicit_params: bool,
    /// The attributes appertaining to the function call operator or its
    /// type.
    pub attrs: Vec<AttrId>,
}

/// A lambda whose body is being analysed: what [`Sema`] saved on entry.
#[derive(Debug)]
pub(crate) struct LambdaScope {
    pub(crate) id: LambdaId,
    pub(crate) closure: DeclId,
    pub(crate) call_op: Option<DeclId>,
    /// `scopes.len()` before the body's function scope: the scopes below
    /// belong to the enclosing context.
    pub(crate) depth: usize,
    saved_ctx: DeclId,
    saved_pending_odr: Vec<ExprId>,
    saved_cleanup: crate::class::CleanupState,
    saved_unevaluated: u32,
    saved_immediate: u32,
    saved_extend: Option<Vec<ExprId>>,
    saved_ctor_entity: Option<(crate::class::InitEntity, kiln_ast::expr::ConstructionKind)>,
    body_open: bool,
    /// `constexpr` was written (the static invoker is then constexpr too).
    explicit_constexpr: bool,
}

impl Sema<'_, '_> {
    /// The *lambda-introducer* `intro`: creates the closure type, an
    /// unnamed class in the innermost enclosing context
    /// ([expr.prim.lambda.closure]/1-2), and starts its definition.
    pub fn act_on_lambda_start(&mut self, intro: &LambdaIntroducer) -> LambdaId {
        let h = DeclHead::new(self.ctx, DeclName::EMPTY, intro.lbracket).flags(DeclFlags::IMPLICIT);
        let closure = self.builder().record(h, TagKind::Class, None);
        let _ = self.builder().begin_record_definition(closure);
        let mut data = LambdaData::new(closure, intro.lbracket);
        data.default = intro.default;
        data.default_loc = intro.default_loc;
        data.rbracket = intro.rbracket;
        let id = self.ast.exprs.lambdas.push(data);
        self.ast.decls.set_closure(closure, id);
        if let Some(c) = intro.captures.first() {
            self.unsupported("lambda captures", c.loc);
        }
        self.lambdas.push(LambdaScope {
            id,
            closure,
            call_op: None,
            depth: self.scopes.len(),
            saved_ctx: self.ctx,
            saved_pending_odr: Vec::new(),
            saved_cleanup: crate::class::CleanupState::default(),
            saved_unevaluated: 0,
            saved_immediate: 0,
            saved_extend: None,
            saved_ctor_entity: None,
            body_open: false,
            explicit_constexpr: false,
        });
        id
    }

    /// The *lambda-declarator* `d` (a member declarator whose single
    /// function chunk holds the parameters, the *noexcept-specifier* and
    /// the trailing return type) with specifiers `specs`: declares the
    /// function call operator ([expr.prim.lambda.closure]/4-5) and opens
    /// its body.
    pub fn act_on_lambda_declarator(
        &mut self,
        id: LambdaId,
        mut d: Declarator,
        specs: &LambdaSpecifiers,
    ) {
        let Some(top) = self.lambdas.last() else { return };
        debug_assert_eq!(top.id, id);
        let closure = top.closure;
        let lbracket = self.ast.exprs.lambdas.get(id).lbracket;
        if let Some(l) = specs.consteval {
            self.unsupported("consteval lambdas", l);
        }
        let is_static = specs.static_.is_some();
        // [expr.prim.lambda.closure]/5: `const` unless `mutable` or
        // `static`.
        let quals = if specs.mutable.is_some() || is_static { Quals::NONE } else { Quals::CONST };
        let mut params: Vec<ParamInfo> = Vec::new();
        for c in &mut d.chunks {
            if let Chunk::Function { quals: q, params: p, .. } = c {
                *q = quals;
                params.clone_from(p);
            }
        }
        let base = self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false);
        let ty = self.type_for_declarator_at(base, lbracket, &d);
        let mut spec =
            FnSpec::new().with_inline(true).with_constexpr_kind(ConstexprKind::Constexpr);
        if is_static {
            spec = spec.with_storage(FnStorage::Static);
        }
        let h = DeclHead::new(closure, DeclName::operator(OverloadedOperator::Call), lbracket)
            .access(Access::Public);
        let f = self.builder().function(h, kiln_ast::decl::DeclKind::CXXMethod, ty, spec, None);
        let defaults: Vec<Option<ExprId>> = params
            .iter()
            .map(|p| match p.default_arg {
                DefaultArg::Parsed(e) => Some(e),
                _ => None,
            })
            .collect();
        self.create_params(f, &params, &defaults);
        let mut attrs = specs.attrs.clone();
        attrs.extend(d.attrs.iter().copied());
        if !attrs.is_empty() {
            let _ = self.builder().attach_attrs(f, &attrs);
        }
        if let Some(r) = d.trailing_requires {
            self.act_on_trailing_requires(f, r);
        }
        {
            let l = self.ast.exprs.lambdas.get_mut(id);
            l.call_op = Some(f);
            l.explicit_params = specs.explicit_params;
            l.mutable = specs.mutable.is_some();
            l.explicit_result =
                d.chunks.iter().any(|c| matches!(c, Chunk::Function { trailing: Some(_), .. }));
        }
        self.begin_lambda_body(f, specs.constexpr.is_some());
    }

    /// Opens the body of call operator `f`: the enclosing function's
    /// state and the current full-expression's are saved, the
    /// parameters enter a new function scope.
    fn begin_lambda_body(&mut self, f: DeclId, explicit_constexpr: bool) {
        let this_ty = self.current_this();
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let ret =
            self.ast.types.proto(fty).map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
        let depth = self.scopes.len();
        let saved_ctx = self.ctx;
        let pending = std::mem::take(&mut self.pending_odr);
        let cleanup = std::mem::take(&mut self.cleanup);
        let unevaluated = std::mem::take(&mut self.unevaluated);
        let immediate = std::mem::take(&mut self.immediate);
        let extend = self.extend_collector.take();
        let entity = self.ctor_entity.take();
        if let Some(top) = self.lambdas.last_mut() {
            top.call_op = Some(f);
            top.depth = depth;
            top.saved_ctx = saved_ctx;
            top.saved_pending_odr = pending;
            top.saved_cleanup = cleanup;
            top.saved_unevaluated = unevaluated;
            top.saved_immediate = immediate;
            top.saved_extend = extend;
            top.saved_ctor_entity = entity;
            top.body_open = true;
            top.explicit_constexpr = explicit_constexpr;
        }
        self.fn_saved.push(crate::class::SavedFn { func: self.func.take(), pushed: 0 });
        self.scopes.push(Scope::new(ScopeKind::Function, None));
        self.ctx = f;
        let params: Vec<DeclId> = self
            .ast
            .decls
            .function(f)
            .map(|x| self.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        for p in params {
            if let Some(sym) = self.ast.decls.get(p).name().as_ident() {
                self.push_local(sym, p);
            }
            let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
            if !self.require_complete(pty, self.ast.decls.get(p).loc())
                && !self.ast.types.is_reference(pty)
                && !self.is_error_type(pty)
            {
                let msg = self
                    .diag(DiagId::sema_typecheck_decl_incomplete_type, self.ast.decls.get(p).loc())
                    .arg(self.ty_arg(pty));
                self.emit(self.incomplete_note(msg, pty));
            }
        }
        self.begin_function_body(f, ret);
        if let Some(state) = self.func.as_mut() {
            state.this_ty = this_ty;
        }
    }

    /// Closes the body of the innermost lambda (if open) and restores the
    /// enclosing state.
    fn end_lambda_body(&mut self, body: Option<(StmtId, Loc)>) -> Option<LambdaScope> {
        let top = self.lambdas.pop()?;
        if top.body_open {
            if let Some((b, rbrace)) = body {
                self.end_function_body(b, rbrace);
                if let Some(f) = top.call_op {
                    let _ = self.builder().set_body(f, b);
                }
            } else {
                self.func = None;
            }
            self.pop_scope();
            while self.scopes.len() > top.depth {
                self.pop_scope();
            }
            if let Some(saved) = self.fn_saved.pop() {
                self.func = saved.func;
            }
        }
        self.ctx = top.saved_ctx;
        Some(top)
    }

    /// The `}` of the innermost lambda's compound statement `body`:
    /// completes the closure type and returns the `LambdaExpr`
    /// ([expr.prim.lambda.general]/2: a prvalue of the closure type).
    pub fn act_on_lambda_end(&mut self, id: LambdaId, body: StmtId, rbrace: Loc) -> ExprId {
        let Some(mut top) = self.end_lambda_body(Some((body, rbrace))) else {
            return self.act_on_error_expr(&[], rbrace);
        };
        debug_assert_eq!(top.id, id);
        self.pending_odr = std::mem::take(&mut top.saved_pending_odr);
        self.cleanup = top.saved_cleanup;
        self.unevaluated = top.saved_unevaluated;
        self.immediate = top.saved_immediate;
        self.extend_collector = top.saved_extend.take();
        self.ctor_entity = top.saved_ctor_entity.take();
        self.ast.exprs.lambdas.get_mut(id).body = Some(body);
        let closure = top.closure;
        if let Some(f) = top.call_op {
            self.add_function_pointer_conversion(id, closure, f, top.explicit_constexpr);
        }
        self.finish_closure(closure);
        let ty = self.tag_type(closure);
        let lbracket = self.ast.exprs.lambdas.get(id).lbracket;
        let e = self.builder().lambda_expr(id, ty, lbracket, rbrace);
        // clang `MaybeBindToTemporary` marks even a trivial destructor
        // referenced, outside a decltype operand.
        let in_decltype = self.decltype_calls.last().is_some_and(|x| x.0 == self.unevaluated);
        if !in_decltype
            && let crate::class::Selected::Found(dtor) = self.select_special(
                closure,
                crate::class::SpecialKind::Dtor,
                Quals::NONE,
                Quals::NONE,
            )
        {
            self.mark_function_referenced(dtor, lbracket);
        }
        self.maybe_bind_temporary(e)
    }

    /// Abandons the innermost lambda (its body failed to parse): the
    /// closure is completed and marked invalid.
    pub fn act_on_lambda_error(&mut self, id: LambdaId) {
        let Some(mut top) = self.end_lambda_body(None) else { return };
        debug_assert_eq!(top.id, id);
        if top.body_open {
            self.pending_odr = std::mem::take(&mut top.saved_pending_odr);
            self.cleanup = top.saved_cleanup;
            self.unevaluated = top.saved_unevaluated;
            self.immediate = top.saved_immediate;
            self.extend_collector = top.saved_extend.take();
            self.ctor_entity = top.saved_ctor_entity.take();
        }
        self.ast.decls.set_flags(top.closure, DeclFlags::INVALID);
        self.finish_closure(top.closure);
    }

    /// Completes the closure class's definition at the lambda's `}`.
    fn finish_closure(&mut self, closure: DeclId) {
        let ctx = self.ctx;
        self.scopes.push(Scope::new(ScopeKind::Record, Some(closure)));
        self.ctx = closure;
        self.act_on_finish_record_definition(closure, false);
        self.ctx = ctx;
    }

    /// [expr.prim.lambda.closure]/8-11: the closure type of a
    /// non-generic lambda with no *lambda-capture* has a conversion
    /// function to pointer to function with the call operator's
    /// parameters and return type (clang `addFunctionPointerConversion`),
    /// and, unless the call operator is static, the static member
    /// `__invoke` the pointer designates.
    fn add_function_pointer_conversion(
        &mut self,
        id: LambdaId,
        closure: DeclId,
        call_op: DeclId,
        explicit_constexpr: bool,
    ) {
        let l = self.ast.exprs.lambdas.get(id);
        if l.default != CaptureDefault::None || !l.captures.is_empty() {
            return;
        }
        let lbracket = l.lbracket;
        let Some(fty) = self.ast.decls.get(call_op).ty() else { return };
        let TypeView::Function(p) = self.ast.types.view(fty.ty()) else { return };
        let params = self.ast.types.params(&p).to_vec();
        let exc = self.ast.types.exc_spec(&p);
        let info = p.info().with_this_quals(Quals::NONE).with_ref_qual(RefQual::None);
        let invoker = self.ast.types.function(p.ret(), &params, info, exc);
        let ptr = self.ast.types.pointer_to(invoker);
        let conv_ty = self.ast.types.function(
            ptr,
            &[],
            FuncInfo::new().with_this_quals(Quals::CONST),
            ExcSpec::BasicNoexcept,
        );
        let canon = self.ast.types.canonical(ptr);
        let name = self.ast.names.conversion(canon);
        let spec = FnSpec::new().with_inline(true).with_constexpr_kind(ConstexprKind::Constexpr);
        let h = DeclHead::new(closure, name, lbracket)
            .flags(DeclFlags::IMPLICIT)
            .access(Access::Public);
        let _conv = self.builder().function(
            h,
            kiln_ast::decl::DeclKind::CXXConversion,
            conv_ty,
            spec,
            None,
        );
        if self.is_static_method(call_op) {
            return;
        }
        let mut spec = FnSpec::new().with_inline(true).with_storage(FnStorage::Static);
        if explicit_constexpr {
            spec = spec.with_constexpr_kind(ConstexprKind::Constexpr);
        }
        let sym = self.ast.symbols().intern("__invoke");
        let h = DeclHead::new(closure, DeclName::ident(sym), lbracket)
            .flags(DeclFlags::IMPLICIT)
            .access(Access::Public);
        let inv =
            self.builder().function(h, kiln_ast::decl::DeclKind::CXXMethod, invoker, spec, None);
        let src: Vec<DeclId> = self
            .ast
            .decls
            .function(call_op)
            .map(|x| self.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        let ps: Vec<DeclId> = src
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let n = self.ast.decls.get(s);
                let (name, loc) = (n.name(), n.loc());
                let t = n.ty().unwrap_or(QualType::ERROR);
                let i = u16::try_from(i).unwrap_or(u16::MAX);
                self.builder().param(inv, name, loc, t, i, 0)
            })
            .collect();
        if !ps.is_empty() {
            let _ = self.builder().set_params(inv, &ps);
        }
    }

    /// True if `f` is the function call operator (or its template
    /// pattern) of a closure type.
    pub(crate) fn is_lambda_call_op(&self, f: DeclId) -> bool {
        self.ast.decls.get(f).name().as_operator() == Some(OverloadedOperator::Call)
            && self
                .ast
                .decls
                .get(f)
                .ctx()
                .is_some_and(|c| self.ast.decls.record(c).is_some_and(|r| r.lambda.is_some()))
    }

    /// True if the closure type `r` has a defaulted default constructor
    /// and copy assignment operator ([expr.prim.lambda.closure]/13: its
    /// lambda has no *lambda-capture*).
    pub(crate) fn closure_default_constructible(&self, r: DeclId) -> bool {
        let Some(id) = self.ast.decls.record(r).and_then(|x| x.lambda) else { return true };
        let l = self.ast.exprs.lambdas.get(id);
        l.default == CaptureDefault::None && l.captures.is_empty()
    }
}
