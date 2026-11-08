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

use kiln_ast::decl::{
    Access, ConstexprKind, DeclFlags, DeclKind, FnSpec, FnStorage, ParamData, TagKind,
};
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

mod capture;
mod generic;

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
    /// The initializer of an init-capture, with its syntax as for a
    /// variable's ([`kiln_ast::decl::InitStyle`]): `= e` and `= {...}`
    /// are [`InitStyle::C`](kiln_ast::decl::InitStyle::C), `(e, ...)` is
    /// `Call`, `{...}` is `List` ([expr.prim.lambda.capture]/6).
    pub init: Option<(kiln_ast::decl::InitStyle, crate::init::Initializer)>,
    /// The variable a simple capture names, already resolved (a lambda
    /// rebuilt by template instantiation); `None` looks the name up.
    pub var: Option<DeclId>,
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
    /// lambda) and <lambda-sig> (the explicit template parameters, the
    /// template-head's requires-clause and the parameter types).
    numbers: kiln_source::fx::FxHashMap<(DeclId, String), u32>,
    /// The numbers given while a pack expansion's pattern is probed for
    /// its packs (`templ::pack`): the probe's lambdas are discarded when
    /// it found a pack, and their numbers with them, as clang never
    /// creates them.
    journal: Option<Vec<(DeclId, String)>>,
}

impl ManglingState {
    /// Starts journaling the numbers given; returns the enclosing
    /// journal for [`Self::end_probe`].
    pub(crate) fn begin_probe(&mut self) -> Option<Vec<(DeclId, String)>> {
        self.journal.replace(Vec::new())
    }

    /// Ends a probe begun by [`Self::begin_probe`]: with `discard`, the
    /// numbers it gave are taken back; otherwise they stay and join the
    /// enclosing journal.
    pub(crate) fn end_probe(&mut self, outer: Option<Vec<(DeclId, String)>>, discard: bool) {
        let mine = std::mem::replace(&mut self.journal, outer).unwrap_or_default();
        if discard {
            for k in mine {
                if let Some(n) = self.numbers.get_mut(&k) {
                    *n = n.saturating_sub(1);
                }
            }
        } else if let Some(j) = self.journal.as_mut() {
            j.extend(mine);
        }
    }
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
    pub(crate) saved_ctx: DeclId,
    saved_pending_odr: Vec<ExprId>,
    saved_potential_caps: Vec<ExprId>,
    saved_cleanup: crate::class::CleanupState,
    saved_unevaluated: u32,
    saved_immediate: u32,
    saved_imm: crate::consteval::SavedImm,
    saved_extend: Option<Vec<ExprId>>,
    saved_ctor_entity: Option<(crate::class::InitEntity, kiln_ast::expr::ConstructionKind)>,
    saved_mangle_var: Option<DeclId>,
    saved_default_arg: Option<DeclId>,
    pub(crate) body_open: bool,
    /// The body names a parameter pack of an enclosing template that it
    /// does not expand: the lambda-expression contains an unexpanded
    /// pack ([temp.variadic]/5; clang's
    /// `LambdaScopeInfo::ContainsUnexpandedParameterPack`).
    has_packs: bool,
    /// The packs behind `has_packs`: declared outside this lambda
    /// (clang's `LambdaScopeInfo::LocalPacks` are the complement, which
    /// are diagnosed in the body; [`Sema::lambda_unexpanded`]).
    packs: Vec<DeclId>,
    /// `constexpr` was written (the static invoker is then constexpr too).
    explicit_constexpr: bool,
    /// `templ.heads.len()` at the introducer: a deeper head after the
    /// parameters is a generic lambda's invented one.
    heads: usize,
    /// `scopes.len()` at the introducer: a template parameter scope above
    /// it is the lambda's *template-parameter-list*.
    scope_base: usize,
    /// The call operator's template decl (a generic lambda).
    call_template: Option<DeclId>,
    /// The type of the member of each capture so far, in order.
    pub(crate) cap_tys: Vec<QualType>,
    /// The init-captures whose variables the body declares.
    init_caps: Vec<capture::InitCapture>,
    /// The init-capture variables by name, in scope from the
    /// introducer on (P2036: in the *lambda-declarator* too; the body
    /// declares them in its own scope).
    pub(crate) init_cap_vars: Vec<(Symbol, DeclId)>,
    /// The local entities (`None`: `*this`) this lambda captured
    /// implicitly whose every reference so far is in one of its contract
    /// assertions, with the first such reference
    /// ([expr.prim.lambda.closure]/10; [`Sema::note_contract_capture_use`]).
    pub(crate) contract_caps: Vec<(Option<DeclId>, Loc)>,
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
        self.lambdas.push(LambdaScope {
            id,
            closure,
            call_op: None,
            depth: self.scopes.len(),
            saved_ctx: self.ctx,
            saved_pending_odr: Vec::new(),
            saved_potential_caps: Vec::new(),
            saved_cleanup: crate::class::CleanupState::default(),
            saved_unevaluated: 0,
            saved_immediate: 0,
            saved_imm: crate::consteval::SavedImm::default(),
            saved_extend: None,
            saved_ctor_entity: None,
            saved_mangle_var: None,
            saved_default_arg: None,
            body_open: false,
            has_packs: false,
            packs: Vec::new(),
            explicit_constexpr: false,
            heads: self.templ.heads.len(),
            scope_base: self.scopes.len(),
            call_template: None,
            cap_tys: Vec::new(),
            init_caps: Vec::new(),
            init_cap_vars: Vec::new(),
            contract_caps: Vec::new(),
        });
        self.act_on_explicit_captures(id, intro);
        self.create_init_capture_vars(id);
        id
    }

    /// The number of template heads outside the innermost lambda whose
    /// body is open (0 outside lambda bodies): a declaration in the body
    /// cannot be headed by them ([`Sema::take_template_head`]). The
    /// lambdas around a point of instantiation do not count
    /// ([`Sema::lambda_base`]).
    pub(crate) fn lambda_head_floor(&self) -> usize {
        let inner = self.lambdas.get(self.lambda_base..).unwrap_or_default();
        inner.iter().rev().find(|l| l.body_open).map_or(0, |l| l.heads)
    }

    /// `<` after the *lambda-introducer*: opens the lambda's
    /// *template-parameter-list* ([expr.prim.lambda.general]/1), a head
    /// one level deeper than the enclosing ones. Closed with the lambda.
    pub fn act_on_lambda_template_params_start(&mut self, l_angle: Loc) {
        let depth = self.template_depth();
        self.templ.heads.push(crate::templ::HeadBuild::explicit(l_angle, depth));
        self.scopes.push(Scope::new(ScopeKind::TemplateParams, None));
        self.ast.templates.dependent_depth += 1;
    }

    /// Pops the template heads (and the parameter scope of a written
    /// *template-parameter-list*) a generic lambda pushed.
    fn pop_lambda_heads(&mut self, heads: usize, scope_base: usize) {
        while self.templ.heads.len() > heads {
            self.templ.heads.pop();
            self.ast.templates.dependent_depth =
                self.ast.templates.dependent_depth.saturating_sub(1);
        }
        self.templ.invented = 0;
        while self.scopes.len() > scope_base
            && self.scopes.last().is_some_and(|s| s.kind == ScopeKind::TemplateParams)
        {
            self.scopes.pop();
        }
    }

    /// The default arguments of lambda parameters `params`, checking
    /// that every parameter after one with a default argument has one
    /// too ([dcl.fct.default]/4).
    fn lambda_default_args(&mut self, params: &[ParamInfo]) -> Vec<Option<ExprId>> {
        if let Some(first) = params.iter().position(|p| p.default_arg != DefaultArg::None) {
            for p in params.iter().skip(first + 1) {
                if p.default_arg != DefaultArg::None
                    || p.invalid
                    || self.ast.types.kind(p.ty.ty()) == kiln_ast::types::TypeKind::PackExpansion
                {
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
        params
            .iter()
            .map(|p| match p.default_arg {
                DefaultArg::Parsed(e) => Some(e),
                _ => None,
            })
            .collect()
    }

    /// The call operator's type `ty` as written, adjusted: clang
    /// `getDummyLambdaType` makes it `auto () const -> R` with no
    /// parameter-declaration-clause, and a GNU `noreturn` makes it a
    /// `noreturn` function type, and so the conversion function's
    /// pointer (spec/types.md §5.4).
    fn adjust_lambda_call_type(
        &mut self,
        mut ty: QualType,
        specs: &LambdaSpecifiers,
        d: &Declarator,
    ) -> QualType {
        if !specs.explicit_params
            && let TypeView::Function(p) = self.ast.types.view(ty.ty())
            && !p.info().trailing_return()
        {
            let params = self.ast.types.params(&p).to_vec();
            let exc = self.ast.types.exc_spec(&p);
            let info = p.info().with_trailing_return(true);
            ty = self.ast.types.function(p.ret(), &params, info, exc);
        }
        let mut written: Vec<AttrId> = specs.attrs.iter().chain(&d.attrs).copied().collect();
        let ty = self.apply_noreturn_attrs(ty, &written);
        self.apply_cc_attrs(ty, &mut written, true)
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
        // [expr.prim.lambda.general]/6: a *template-parameter-list* or an
        // `auto` parameter makes the call operator a member template.
        let generic = self.templ.heads.len() > top.heads;
        let lbracket = self.ast.exprs.lambdas.get(id).lbracket;
        if generic {
            self.finish_invented_head();
            self.take_innermost_head();
        }
        let is_static = specs.static_.is_some();
        let params: Vec<ParamInfo> =
            d.function_params().map(<[ParamInfo]>::to_vec).unwrap_or_default();
        let explicit_obj = self.lambda_explicit_object(&params, specs);
        // [expr.prim.lambda.closure]/5: `const` unless `mutable`,
        // `static` or with an explicit object parameter.
        let quals = if specs.mutable.is_some() || is_static || explicit_obj {
            Quals::NONE
        } else {
            Quals::CONST
        };
        for c in &mut d.chunks {
            if let Chunk::Function { quals: q, .. } = c {
                *q = quals;
            }
        }
        let base = self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false);
        // A trailing return type's checks (array or function return,
        // volatile return) point at the lambda declarator's '(' as clang
        // (there is no `auto` type-specifier to point at).
        let lparen = d
            .chunks
            .iter()
            .find_map(|c| match c {
                Chunk::Function { loc, .. } => Some(*loc),
                _ => None,
            })
            .unwrap_or(lbracket);
        let ty = self.type_for_declarator_at(base, lparen, &d);
        // [temp.variadic]/5: a parameter or return type naming a pack it
        // does not expand (`[](Ts a) {}`, also after the varargs `...` of
        // `[](Ts a...)`) makes the lambda-expression contain an
        // unexpanded pack (clang `ActOnStartOfLambdaDefinition`).
        if self.has_unexpanded_pack(ty)
            && let Some(top) = self.lambdas.last_mut()
        {
            top.has_packs = true;
        }
        let ty = self.adjust_lambda_call_type(ty, specs, &d);
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
        if explicit_obj {
            spec = spec.with_explicit_object(true);
        }
        let h = DeclHead::new(closure, DeclName::operator(OverloadedOperator::Call), lbracket)
            .access(Access::Public);
        if generic {
            self.mark_pattern_next();
        }
        let f = self.builder().function(h, DeclKind::CXXMethod, ty, spec, None);
        let call_template = self.finish_template_pattern(f);
        if let Some(t) = call_template {
            self.ast.decls.set_access(t, Access::Public);
        }
        if let Some(top) = self.lambdas.last_mut() {
            top.call_template = call_template;
        }
        let defaults = self.lambda_default_args(&params);
        self.create_params(f, &params, &defaults);
        let mut attrs: Vec<AttrId> = specs.attrs.iter().chain(&d.attrs).copied().collect();
        if !attrs.is_empty() {
            // Nothing here an annotation appertains to ([dcl.attr.annotation]/1).
            self.reject_annotations(&mut attrs, Some("a lambda call operator"));
            self.check_alignas_subject(f, &mut attrs);
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
        // A non-template call operator's object parameter type is checked
        // here, once, at the parameter (clang); a specialization's at
        // each call.
        if explicit_obj
            && call_template.is_none()
            && let Some(p) = params.first()
        {
            let at = p.name.map_or(p.loc, |(_, l)| l);
            self.check_lambda_explicit_object_type(f, at);
        }
        self.number_lambda(id, closure, f);
        self.begin_lambda_body(f, specs.constexpr.is_some());
    }

    /// Whether the call operator of a lambda with parameters `params` and
    /// specifiers `specs` is an explicit object member function
    /// ([expr.prim.lambda.closure]/5, P0847): its first parameter is an
    /// explicit object parameter. Diagnoses one anywhere else, in a
    /// `static` or `mutable` lambda, with a default argument or as a pack
    /// (clang `CheckExplicitObjectLambda`, `ActOnLambdaClosureParameters`).
    fn lambda_explicit_object(&mut self, params: &[ParamInfo], specs: &LambdaSpecifiers) -> bool {
        for p in params.iter().skip(1) {
            if p.explicit_object.is_some() {
                let m = self.diag(DiagId::sema_explicit_object_param_not_first, p.loc).arg(1u8);
                self.emit(m);
            }
        }
        let Some(first) = params.first() else { return false };
        let Some(this_loc) = first.explicit_object else { return false };
        if specs.static_.is_some() {
            let m =
                self.diag(DiagId::sema_explicit_object_param_nonmember, this_loc).arg(0u8).arg(1u8);
            self.emit(m);
            return false;
        }
        if specs.mutable.is_some() {
            self.error(DiagId::sema_explicit_object_lambda_mutable, this_loc);
        }
        match first.default_arg {
            DefaultArg::None => {}
            DefaultArg::Unparsed(l, _) => self.error(DiagId::sema_explicit_object_param_default, l),
            DefaultArg::Parsed(e) => {
                let l = if first.default_loc.is_valid() { first.default_loc } else { self.eloc(e) };
                self.error(DiagId::sema_explicit_object_param_default, l);
            }
        }
        if matches!(self.ast.types.view(first.ty.ty()), TypeView::PackExpansion { .. }) {
            self.error(DiagId::sema_explicit_object_param_pack, this_loc);
        }
        true
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
    /// lambdas in the body of an inline or templated function (one in a
    /// template definition or being instantiated), in a default argument
    /// in a class definition, in a default member initializer or in the
    /// initializer of an inline variable are numbered per context and
    /// <lambda-sig>; any other has internal linkage and
    /// no number.
    fn number_lambda(&mut self, id: LambdaId, closure: DeclId, f: DeclId) {
        let Some((key, context)) = self.lambda_numbering_context(closure) else { return };
        // clang `ItaniumNumberingContext::getManglingNumber` keys on the
        // mangled <lambda-sig>.
        let sig = kiln_abi::mangle::lambda_sig(&self.ast, f);
        if let Some(j) = self.lambda_mangle.journal.as_mut() {
            j.push((key, sig.clone()));
        }
        let n = self.lambda_mangle.numbers.entry((key, sig)).or_insert(0);
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
        if is_function(kind)
            && let Some(p) = self.lambda_mangle.default_arg
            && self.ast.decls.get(p).ctx() == Some(ctx)
        {
            // An instantiated default argument of `ctx`'s parameter `p`
            // (clang `getCurrentMangleNumberContext`, `DefaultArgument`):
            // numbered per parameter only for a member function defined
            // in its class; otherwise it has no number.
            let in_class = self
                .ast
                .decls
                .lexical_ctx(ctx)
                .is_some_and(|l| self.kind_of(l) == DeclKind::CXXRecord);
            return in_class.then_some((p, Some(p)));
        }
        if is_function(kind) {
            // clang `getCurrentMangleNumberContext`: `inTemplateInstantiation()
            // || CurContext->isDependentContext()` numbers it too; a
            // template parameter list open at the introducer is the latter.
            let templated =
                self.inst.stack.depth() > 0 || self.lambdas.last().is_some_and(|t| t.heads > 0);
            return (templated || self.in_inline_function(ctx)).then_some((ctx, None));
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
        // clang `TemplatedVariable`: a variable template's pattern or an
        // implicit specialization of one.
        let templated = self.ast.described_template(v).is_some()
            || self.ast.spec_info(v).is_some_and(|(_, i)| {
                i.kind != kiln_ast::templates::SpecKind::ExplicitSpecialization
            });
        (inline || templated).then_some((v, Some(v)))
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
        let outer_this = self.current_this();
        let id = self.lambdas.last().map(|t| t.id);
        let this_ty = match id {
            Some(id) => self.lambda_body_this(id, outer_this),
            None => outer_this,
        };
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let ret =
            self.ast.types.proto(fty).map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
        let depth = self.scopes.len();
        let saved_ctx = self.ctx;
        let pending = std::mem::take(&mut self.pending_odr);
        let potential = std::mem::take(&mut self.potential_caps);
        let cleanup = std::mem::take(&mut self.cleanup);
        let unevaluated = std::mem::take(&mut self.unevaluated);
        let immediate = std::mem::take(&mut self.immediate);
        let imm = self.take_immediate_state(true);
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
            top.saved_potential_caps = potential;
            top.saved_cleanup = cleanup;
            top.saved_unevaluated = unevaluated;
            top.saved_immediate = immediate;
            top.saved_imm = imm;
            top.saved_extend = extend;
            top.saved_ctor_entity = entity;
            top.body_open = true;
            top.explicit_constexpr = explicit_constexpr;
        }
        self.fn_saved.push(crate::class::SavedFn { func: self.func.take(), pushed: 0 });
        let mut fs = Scope::new(ScopeKind::Function, None);
        fs.errors = self.diags.unrecoverable_error_count();
        self.scopes.push(fs);
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
        if let Some(id) = id {
            self.declare_init_captures(id, f);
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
                let b = self.end_function_body(b, rbrace);
                if let Some(f) = top.call_op {
                    // clang `ActOnFinishFunctionBody` -> `checkTypeSupport`
                    // (the deduced return type included).
                    if let Some(fty) = self.ast.decls.get(f).ty() {
                        self.check_sve_support(fty, self.ast.decls.get(f).loc());
                    }
                    let _ = self.builder().set_body(f, b);
                    // [dcl.constexpr]/3 before C++23 (constexpr_body.rs).
                    if top.explicit_constexpr {
                        self.check_constexpr_lambda_body_vars(f, b);
                    } else {
                        self.implicit_constexpr_lambda_body_vars(f, b);
                    }
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
        self.potential_caps = std::mem::take(&mut top.saved_potential_caps);
        self.cleanup = top.saved_cleanup;
        self.unevaluated = top.saved_unevaluated;
        self.restore_immediate_state(std::mem::take(&mut top.saved_imm));
        self.immediate = top.saved_immediate;
        self.extend_collector = top.saved_extend.take();
        self.ctor_entity = top.saved_ctor_entity.take();
        self.pop_lambda_heads(top.heads, top.scope_base);
        self.ast.exprs.lambdas.get_mut(id).body = Some(body);
        let closure = top.closure;
        self.finish_captures(id, closure, &top.cap_tys);
        self.check_contract_only_captures(&top.contract_caps);
        if let Some(f) = top.call_op {
            self.add_function_pointer_conversion(id, closure, f, top.explicit_constexpr);
        }
        self.finish_closure(closure);
        let ty = self.tag_type(closure);
        let lbracket = self.ast.exprs.lambdas.get(id).lbracket;
        let e = self.builder().lambda_expr(id, ty, top.has_packs, lbracket, rbrace);
        if !top.packs.is_empty() {
            self.lambda_packs.insert(id, top.packs);
        }
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

    /// A full-expression or declaration in the innermost lambda's body
    /// that contains an unexpanded parameter pack: a pack declared
    /// outside the lambda is expanded by an expansion enclosing it
    /// (`[] { return sizeof(T); }()...`; clang
    /// `DiagnoseUnexpandedParameterPacks` inside a lambda). The lambda's
    /// own packs are left to the site's diagnostic.
    pub(crate) fn note_unexpanded_in_lambda(&mut self, op: crate::templ::pack::PackOperand) {
        let _ = self.lambda_unexpanded(op);
    }

    /// [temp.variadic]/12 at the end of full-expression `e` in a lambda
    /// body (clang's `ActOnFinishFullExpr`, `DiagnoseUnexpandedParameterPack`
    /// with `UPPC_Expression` at the expression's begin): an enclosing
    /// scope's packs are recorded on the lambda; the lambda's own make
    /// the expression erroneous. `Some(recovery)` after the diagnostic.
    /// A substituted expression is not checked (§7.2).
    pub(crate) fn diagnose_unexpanded_full_expr(&mut self, e: ExprId) -> Option<ExprId> {
        let local = self.lambda_unexpanded(crate::templ::pack::PackOperand::Expr(e));
        if local.is_empty() || !self.inst.stack.is_empty() || self.discarding_init {
            return None;
        }
        let at = self.begin_loc(e);
        self.report_unexpanded_packs(&local, "expression", at);
        Some(self.recover_err(&[], at))
    }

    /// [temp.variadic]/12 in the innermost lambda's body ([temp.variadic]/5,
    /// clang `DiagnoseUnexpandedParameterPacks` with `LocalPacks`): of the
    /// packs `op` names outside an expansion, those declared in an
    /// enclosing scope make the lambda-expression contain an unexpanded
    /// pack (recorded); those declared by the lambda itself (an
    /// init-capture pack, a parameter pack, a template parameter pack of
    /// a generic lambda) are returned, for the caller to diagnose. Empty
    /// outside a lambda body.
    pub(crate) fn lambda_unexpanded(&mut self, op: crate::templ::pack::PackOperand) -> Vec<DeclId> {
        if !self.lambdas.last().is_some_and(|top| top.body_open) {
            return Vec::new();
        }
        self.split_lambda_packs(op)
    }

    /// [`Sema::lambda_unexpanded`] also in the innermost lambda's
    /// declarator (before its body opens): a default argument of its
    /// parameters. The lambda's own packs there are those of its
    /// *template-parameter-list*.
    pub(crate) fn split_lambda_packs(
        &mut self,
        op: crate::templ::pack::PackOperand,
    ) -> Vec<DeclId> {
        let Some(top) = self.lambdas.last() else { return Vec::new() };
        let base = top.scope_base;
        let packs = self.unexpanded_packs(op);
        let own = |s: &Self, d: DeclId| {
            s.scopes
                .get(base..)
                .is_some_and(|sc| sc.iter().any(|x| x.locals.iter().any(|&(_, l, _)| l == d)))
        };
        let (local, outer): (Vec<DeclId>, Vec<DeclId>) =
            packs.iter().copied().partition(|&d| own(self, d));
        let flagged = packs.is_empty() && self.operand_has_packs(op);
        let Some(top) = self.lambdas.last_mut() else { return Vec::new() };
        if flagged || !outer.is_empty() {
            top.has_packs = true;
        }
        for d in outer {
            if !top.packs.contains(&d) {
                top.packs.push(d);
            }
        }
        local
    }

    /// Abandons the innermost lambda (its body failed to parse): the
    /// closure is completed and marked invalid.
    pub fn act_on_lambda_error(&mut self, id: LambdaId) {
        let Some(mut top) = self.end_lambda_body(None) else { return };
        debug_assert_eq!(top.id, id);
        if top.body_open {
            self.pending_odr = std::mem::take(&mut top.saved_pending_odr);
            self.potential_caps = std::mem::take(&mut top.saved_potential_caps);
            self.cleanup = top.saved_cleanup;
            self.unevaluated = top.saved_unevaluated;
            self.restore_immediate_state(std::mem::take(&mut top.saved_imm));
            self.immediate = top.saved_immediate;
            self.extend_collector = top.saved_extend.take();
            self.ctor_entity = top.saved_ctor_entity.take();
        }
        self.pop_lambda_heads(top.heads, top.scope_base);
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
        // P2564 / [expr.prim.lambda.closure]/10: an escalated call
        // operator makes the conversion immediate too (clang 19 marks only
        // the invoker and rejects every use of the conversion instead).
        let escalated = self.ast.decls.function(call_op).is_some_and(|x| x.spec.escalated());
        let spec =
            FnSpec::new().with_inline(true).with_constexpr_kind(kind).with_escalated(escalated);
        let h = DeclHead::new(closure, name, lbracket)
            .flags(DeclFlags::IMPLICIT)
            .access(Access::Public);
        // A generic lambda's are templates with the call operator's
        // template parameters ([expr.prim.lambda.closure]/9).
        let list = self
            .ast
            .described_template(call_op)
            .and_then(|t| self.ast.template_info(t))
            .map(|i| i.params);
        self.ast.templates.pattern_next = list.is_some();
        let conv = self.builder().function(h, DeclKind::CXXConversion, conv_ty, spec, None);
        self.wrap_thunk_template(conv, list);
        // The conversion of a static or explicit object call operator
        // returns its address ([expr.prim.lambda.closure]/8, /11).
        if self.is_static_method(call_op) || self.has_explicit_object(call_op) {
            return;
        }
        let mut spec = FnSpec::new().with_inline(true).with_storage(FnStorage::Static);
        if consteval {
            spec = spec.with_constexpr_kind(ConstexprKind::Consteval);
        } else if explicit_constexpr {
            spec = spec.with_constexpr_kind(ConstexprKind::Constexpr);
        }
        // P2564: and the invoker.
        if escalated {
            spec = spec.with_escalated(true);
        }
        let sym = self.ast.symbols().intern("__invoke");
        let h = DeclHead::new(closure, DeclName::ident(sym), lbracket)
            .flags(DeclFlags::IMPLICIT)
            .access(Access::Public);
        self.ast.templates.pattern_next = list.is_some();
        let inv = self.builder().function(h, DeclKind::CXXMethod, invoker, spec, None);
        self.wrap_thunk_template(inv, list);
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
                // A function parameter pack stays one.
                let flags = self.ast.decls.param(s).map_or(0, |x| x.flags & ParamData::PACK);
                self.builder().param(inv, name, loc, t, i, flags)
            })
            .collect();
        if !ps.is_empty() {
            let _ = self.builder().set_params(inv, &ps);
        }
    }

    /// Wraps the conversion or invoker pattern `f` of a generic lambda in
    /// a function template over the call operator's parameter list.
    fn wrap_thunk_template(&mut self, f: DeclId, list: Option<kiln_ast::TemplateParamListId>) {
        if let Some(list) = list {
            let t = self.builder().template_decl(DeclKind::FunctionTemplate, f, list, None);
            self.ast.decls.set_access(t, Access::Public);
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
        // to it. Of a generic lambda, that is the call operator's
        // specialization for the invoker's arguments, not its pattern
        // (clang `DefineImplicitLambdaToFunctionPointerConversion`).
        let call_op = match self.ast.closure_thunk(target) {
            Some(ClosureThunk::Invoker { call_op }) => Some(call_op),
            _ => self
                .ast
                .decls
                .get(conv)
                .ctx()
                .and_then(|c| self.ast.decls.record(c).and_then(|r| r.lambda))
                .and_then(|id| self.ast.exprs.lambdas.get(id).call_op),
        };
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

    /// True if the body of call operator `f` is open and its lambda was
    /// declared `constexpr` or `consteval` explicitly.
    pub(crate) fn lambda_explicitly_constexpr(&self, f: DeclId) -> bool {
        self.lambdas.iter().rev().any(|t| t.call_op == Some(f) && t.explicit_constexpr)
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
