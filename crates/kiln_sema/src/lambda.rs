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

use kiln_ast::decl::{Access, ConstexprKind, DeclFlags, DeclKind, FnSpec, FnStorage, TagKind};
use kiln_ast::ids::{AttrId, ExprId, LambdaId, StmtId};
use kiln_ast::lambda::{CaptureDefault, CaptureKind, ClosureThunk, LambdaData};
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

/// What decides a closure type's mangling number (clang
/// `getCurrentMangleNumberContext`, `ItaniumNumberingContext`).
#[derive(Debug, Default)]
pub(crate) struct ManglingState {
    /// The variable whose initializer may be being parsed (set when it is
    /// declared, cleared at its initializer's end).
    pub(crate) var: Option<DeclId>,
    /// The parameter whose delayed default argument is being parsed.
    pub(crate) default_arg: Option<DeclId>,
    /// The last number given per numbering context (a function, or the
    /// variable, data member or parameter whose initializer holds the
    /// lambda) and call-operator parameter list.
    numbers: kiln_source::fx::FxHashMap<(DeclId, Vec<QualType>, bool), u32>,
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
    saved_mangle_var: Option<DeclId>,
    saved_default_arg: Option<DeclId>,
    body_open: bool,
    /// `constexpr` was written (the static invoker is then constexpr too).
    explicit_constexpr: bool,
    /// `templ.heads.len()` at the introducer: a deeper head after the
    /// parameters is a generic lambda's invented one.
    heads: usize,
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
            saved_mangle_var: None,
            saved_default_arg: None,
            body_open: false,
            explicit_constexpr: false,
            heads: self.templ.heads.len(),
        });
        id
    }

    /// After the *lambda-declarator*'s parameters: whether `auto`
    /// parameters made the lambda generic ([expr.prim.lambda.general]/6).
    /// Generic lambdas are not supported yet: reported, and the caller
    /// abandons the lambda through [`Self::act_on_lambda_error`].
    pub fn act_on_lambda_check_generic(&mut self, loc: Loc) -> bool {
        let Some(top) = self.lambdas.last() else { return false };
        if self.templ.heads.len() <= top.heads {
            return false;
        }
        self.unsupported("generic lambdas", loc);
        true
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
            // Immediate functions are not implemented (decl.rs reports a
            // `consteval` function the same way).
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
        let mut ty = self.type_for_declarator_at(base, lbracket, &d);
        // clang `getDummyLambdaType`: with no parameter-declaration-clause
        // the call operator's type is `auto () const -> R`.
        if !specs.explicit_params
            && let TypeView::Function(p) = self.ast.types.view(ty.ty())
            && !p.info().trailing_return()
        {
            let params = self.ast.types.params(&p).to_vec();
            let exc = self.ast.types.exc_spec(&p);
            let info = p.info().with_trailing_return(true);
            ty = self.ast.types.function(p.ret(), &params, info, exc);
        }
        // [dcl.constexpr]/1: `consteval` makes the call operator an
        // immediate function; otherwise it is constexpr if it can be.
        let kind = if specs.consteval.is_some() {
            ConstexprKind::Consteval
        } else {
            ConstexprKind::Constexpr
        };
        let mut spec = FnSpec::new().with_inline(true).with_constexpr_kind(kind);
        if is_static {
            spec = spec.with_storage(FnStorage::Static);
        }
        let h = DeclHead::new(closure, DeclName::operator(OverloadedOperator::Call), lbracket)
            .access(Access::Public);
        let f = self.builder().function(h, DeclKind::CXXMethod, ty, spec, None);
        let defaults: Vec<Option<ExprId>> = params
            .iter()
            .map(|p| match p.default_arg {
                DefaultArg::Parsed(e) => Some(e),
                _ => None,
            })
            .collect();
        // [dcl.fct.default]/4: every parameter after one with a default
        // argument has one too.
        if let Some(first) = params.iter().position(|p| p.default_arg != DefaultArg::None) {
            for p in params.iter().skip(first + 1) {
                if p.default_arg != DefaultArg::None || p.invalid {
                    continue;
                }
                let msg = match p.name {
                    Some((n, _)) => self
                        .diag(DiagId::sema_missing_default_arg, p.loc)
                        .arg(self.ast.symbols().as_str(n).to_string()),
                    None => self.diag(DiagId::sema_missing_default_arg_unnamed, p.loc),
                };
                self.emit(msg);
            }
        }
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
        self.number_lambda(id, closure, f);
        self.begin_lambda_body(f, specs.constexpr.is_some());
    }

    /// Before the delayed default argument of parameter `p` is parsed: a
    /// lambda in it is numbered in the argument's own context (Itanium
    /// ABI §5.1.8, "default arguments appearing in class definitions").
    pub fn act_on_start_delayed_default_arg(&mut self, p: DeclId) {
        self.lambda_mangle.default_arg = Some(p);
    }

    /// After a delayed default argument.
    pub fn act_on_end_delayed_default_arg(&mut self) {
        self.lambda_mangle.default_arg = None;
    }

    /// Gives closure type `closure` of lambda `id` with call operator `f`
    /// its mangling number and context (clang `handleLambdaNumbering`):
    /// lambdas in the body of an inline function, in a default argument
    /// in a class definition, in a default member initializer or in the
    /// initializer of an inline variable are numbered per context and
    /// call-operator parameter list; any other has internal linkage and
    /// no number.
    fn number_lambda(&mut self, id: LambdaId, closure: DeclId, f: DeclId) {
        let Some((key, context)) = self.lambda_numbering_context(closure) else { return };
        let Some(fty) = self.ast.decls.get(f).ty() else { return };
        let Some(p) = self.ast.types.proto(self.ast.types.canonical(fty)).copied() else {
            return;
        };
        let params: Vec<QualType> = self
            .ast
            .types
            .params(&p)
            .iter()
            .map(|&q| self.ast.types.canonical(q).without_quals())
            .collect();
        let n = self.lambda_mangle.numbers.entry((key, params, p.variadic())).or_insert(0);
        *n += 1;
        let number = *n;
        let m = &mut self.ast.exprs.lambdas.get_mut(id).mangling;
        m.number = number;
        m.context = context;
    }

    /// The numbering context of a closure type created in context `ctx`
    /// (its parent) and its context declaration, if it has one.
    fn lambda_numbering_context(&self, closure: DeclId) -> Option<(DeclId, Option<DeclId>)> {
        let ctx = self.ast.decls.get(closure).ctx()?;
        let kind = self.kind_of(ctx);
        if is_function(kind) {
            return self.in_inline_function(ctx).then_some((ctx, None));
        }
        if kind == DeclKind::CXXRecord {
            if let Some(p) = self.lambda_mangle.default_arg {
                return Some((p, Some(p)));
            }
            if let Some(&(field, _)) = self.nsdmi_this.last()
                && self.ast.decls.get(field).ctx() == Some(ctx)
            {
                return Some((field, Some(field)));
            }
        }
        let v = self.lambda_mangle.var?;
        let inline = self
            .ast
            .redecls_of(v)
            .any(|r| self.ast.decls.var(r).is_some_and(|x| x.spec.is_inline()));
        inline.then_some((v, Some(v)))
    }

    /// clang `isInLineFunction`: `dc` or a lexically enclosing function
    /// (up to the namespace) is inline.
    fn in_inline_function(&self, dc: DeclId) -> bool {
        let mut c = Some(dc);
        while let Some(d) = c {
            let k = self.kind_of(d);
            if matches!(k, DeclKind::TranslationUnit | DeclKind::Namespace) {
                return false;
            }
            if is_function(k)
                && self.ast.redecls_of(d).any(|r| {
                    self.ast.decls.function(r).is_some_and(|x| {
                        x.spec.is_inline()
                            || x.spec.implicit_inline()
                            || x.spec.constexpr_kind() != ConstexprKind::None
                    })
                })
            {
                return true;
            }
            c = self.ast.decls.lexical_ctx(d);
        }
        false
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
        let mangle_var = self.lambda_mangle.var.take();
        let default_arg = self.lambda_mangle.default_arg.take();
        if let Some(top) = self.lambdas.last_mut() {
            top.saved_mangle_var = mangle_var;
            top.saved_default_arg = default_arg;
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
        if top.body_open {
            self.lambda_mangle.var = top.saved_mangle_var;
            self.lambda_mangle.default_arg = top.saved_default_arg;
        }
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
        // A generic lambda's invented template head.
        while self.templ.heads.len() > top.heads {
            self.templ.heads.pop();
            self.ast.templates.dependent_depth =
                self.ast.templates.dependent_depth.saturating_sub(1);
        }
        self.templ.invented = 0;
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
        // [expr.prim.lambda.closure]/10-11: the conversion and the
        // invoker are immediate functions if the call operator is.
        let consteval = self
            .ast
            .decls
            .function(call_op)
            .is_some_and(|x| x.spec.constexpr_kind() == ConstexprKind::Consteval);
        let kind = if consteval { ConstexprKind::Consteval } else { ConstexprKind::Constexpr };
        let spec = FnSpec::new().with_inline(true).with_constexpr_kind(kind);
        let h = DeclHead::new(closure, name, lbracket)
            .flags(DeclFlags::IMPLICIT)
            .access(Access::Public);
        let _conv = self.builder().function(h, DeclKind::CXXConversion, conv_ty, spec, None);
        if self.is_static_method(call_op) {
            return;
        }
        let mut spec = FnSpec::new().with_inline(true).with_storage(FnStorage::Static);
        if consteval {
            spec = spec.with_constexpr_kind(ConstexprKind::Consteval);
        } else if explicit_constexpr {
            spec = spec.with_constexpr_kind(ConstexprKind::Constexpr);
        }
        let sym = self.ast.symbols().intern("__invoke");
        let h = DeclHead::new(closure, DeclName::ident(sym), lbracket)
            .flags(DeclFlags::IMPLICIT)
            .access(Access::Public);
        let inv = self.builder().function(h, DeclKind::CXXMethod, invoker, spec, None);
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

    /// clang `DefineImplicitLambdaToFunctionPointerConversion`: on the
    /// first odr-use of the conversion function `conv` of a closure type,
    /// gives it the body `{ return target; }` (the static invoker, or the
    /// static call operator) and marks the invoker defined with an empty
    /// body (code generation and the Oven synthesize what it does).
    pub(crate) fn define_lambda_conversion(&mut self, conv: DeclId, loc: Loc) {
        let Some(ClosureThunk::Conversion { target }) = self.ast.closure_thunk(conv) else {
            return;
        };
        if self.ast.definition(conv).is_some() {
            return;
        }
        let at = self.ast.decls.get(conv).loc();
        // clang marks the call operator referenced: the invoker forwards
        // to it.
        let call_op = self
            .ast
            .decls
            .get(conv)
            .ctx()
            .and_then(|c| self.ast.decls.record(c).and_then(|r| r.lambda))
            .and_then(|id| self.ast.exprs.lambdas.get(id).call_op);
        if let Some(op) = call_op
            && op != target
        {
            self.mark_function_referenced(op, loc);
        }
        let Some(tq) = self.ast.decls.get(target).ty() else { return };
        let ptr = self.ast.types.pointer_to(tq);
        let r = self.builder().decl_ref(
            target,
            tq,
            at,
            None,
            kiln_ast::expr::NonOdrUse::None,
            kiln_ast::expr::ExprFlags::NONE,
        );
        let c = self.builder().implicit_cast(
            kiln_ast::expr::CastKind::FunctionToPointerDecay,
            r,
            ptr,
            kiln_ast::expr::ValueCategory::PRValue,
        );
        let ret = self.builder().return_stmt(Some(c), None, at);
        let body = self.builder().compound(&[ret], at, at);
        let _ = self.builder().set_body(conv, body);
        if matches!(self.ast.closure_thunk(target), Some(ClosureThunk::Invoker { .. }))
            && self.ast.definition(target).is_none()
        {
            let body = self.builder().compound(&[], at, at);
            let _ = self.builder().set_body(target, body);
        }
        self.mark_function_referenced(target, loc);
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

/// A function declaration kind.
fn is_function(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::Function
            | DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion
    )
}
