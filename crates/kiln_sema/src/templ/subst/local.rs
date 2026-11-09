//! Local classes of templated functions ([class.local], [temp.inst]/3,
//! spec/sema-templates.md §4.4): a class defined in the body of a
//! function template, or of a member function of a class template, is
//! instantiated with the enclosing function, by the function body's own
//! substituter, so its members see the enclosing function's template
//! arguments and local declarations (clang
//! `TemplateDeclInstantiator::VisitCXXRecordDecl` with a function-local
//! context, `InstantiateClass` then the member function definitions).

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::types::{ExcSpec, NoexceptValue, TagKeyword};
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_diag::DiagId;

use super::{DeclOrPack, Substituter};
use crate::decl::TagUse;
use crate::templ::stack::FrameKind;

/// A member function defined in a local class, defined once the
/// outermost local class is complete (the parser replays these bodies
/// after the class's `}`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct LocalMethod {
    /// The instantiated class `f` is a member of.
    class: DeclId,
    /// The instantiated member function.
    f: DeclId,
    /// Its pattern definition.
    pdef: DeclId,
    /// The pattern body.
    body: StmtId,
}

/// A part of a local class member in a complete-class context
/// ([class.mem.general]/7) other than a function body: substituted once
/// the outermost local class is complete, as the parser replays it
/// (spec/sema-templates.md §4.4, WP-905). Declaration parts (default
/// arguments, *noexcept-specifiers*) come first in member order, then
/// default member initializers (clang `ParseLexedMethodDeclarations`,
/// then `ParseLexedMemberInitializers`, then the bodies).
#[derive(Clone, Copy, Debug)]
pub(crate) enum LocalLate {
    /// The default arguments of member function `f` from pattern `pat`.
    DefaultArgs { pat: DeclId, f: DeclId },
    /// The *noexcept-specifier* of member function `f` from pattern `pat`.
    ExcSpec { pat: DeclId, f: DeclId },
    /// The default member initializer `init` (of the pattern) of `field`.
    Nsdmi { field: DeclId, init: ExprId },
}

impl LocalLate {
    fn decl(self) -> DeclId {
        match self {
            Self::DefaultArgs { f, .. } | Self::ExcSpec { f, .. } => f,
            Self::Nsdmi { field, .. } => field,
        }
    }
}

impl Substituter<'_, '_, '_> {
    /// Whether member `d` (of the pattern or instantiated) of a local
    /// class, not a closure type, has its complete-class-context parts
    /// substituted after the outermost local class (`local_late`).
    pub(crate) fn defers_to_local_late(&self, d: DeclId) -> bool {
        self.local_late.is_some()
            && self.sema.ast.decls.get(d).ctx().is_some_and(|c| {
                self.sema.ast.decls.record(c).is_some_and(|r| r.lambda.is_none())
                    && self.sema.decl_in_local_scope(c)
            })
    }

    /// Queues `part` for the outermost local class's completion.
    pub(crate) fn push_local_late(&mut self, part: LocalLate) {
        if let Some(v) = self.local_late.as_mut() {
            v.push(part);
        }
    }

    /// Local class `d` of the pattern (or a member class of a local
    /// class, when `methods` collects for the outermost one): declared in
    /// the current scope and, when `d` is a definition, defined now with
    /// its members; the outermost local class then defines its member
    /// functions.
    pub(crate) fn local_class(
        &mut self,
        d: DeclId,
        methods: Option<&mut Vec<LocalMethod>>,
    ) -> DeclId {
        let n = *self.sema.ast.decls.get(d);
        let loc = n.loc();
        let name = n.name().as_ident().filter(|_| !n.name().is_empty()).map(|s| (s, loc));
        let kw = match self.sema.ast.decls.record(d).map(|r| r.tag) {
            Some(kiln_ast::decl::TagKind::Class) => TagKeyword::Class,
            Some(kiln_ast::decl::TagKind::Union) => TagKeyword::Union,
            _ => TagKeyword::Struct,
        };
        let defined = self.sema.ast.definition(d) == Some(d);
        let use_ = if defined { TagUse::Definition } else { TagUse::Declaration };
        let x = self.sema.act_on_tag(kw, name, None, use_, 0, None, loc);
        self.sema.ast.templates.set_instantiated_from(x, d);
        self.locals.insert(d, DeclOrPack::Decl(x));
        // The body is instantiated with the function, under its own
        // context frame (clang `InstantiateClass` from
        // `VisitCXXRecordDecl` for a local class: "in instantiation of
        // member class 'L' requested here", at the class name).
        if defined && self.sema.push_inst_frame(FrameKind::MemberClass, x, d, loc) {
            // A block-scope anonymous union ([class.union.anon]): its
            // object is the declaration statement's next declaration.
            let anon = self.is_anonymous_record(d);
            if let Some(v) = methods {
                self.class_body_with(x, d, Some(v), anon);
            } else {
                let saved = self.local_late.replace(Vec::new());
                let mut v = Vec::new();
                self.class_body_with(x, d, Some(&mut v), anon);
                let late = std::mem::replace(&mut self.local_late, saved).unwrap_or_default();
                self.local_late_parts(x, &late);
                self.sema.pop_inst_frame();
                self.local_methods(x, &v);
                return x;
            }
            self.sema.pop_inst_frame();
        }
        x
    }

    /// Collects member `x` of local class `class`, instantiated from
    /// pattern member `m`, when it is a member function `m`'s class
    /// defines.
    pub(crate) fn collect_local_method(
        &mut self,
        class: DeclId,
        x: DeclId,
        m: DeclId,
        out: &mut Vec<LocalMethod>,
    ) {
        let is_fn = |k: DeclKind| {
            matches!(
                k,
                DeclKind::CXXMethod
                    | DeclKind::CXXConstructor
                    | DeclKind::CXXDestructor
                    | DeclKind::CXXConversion
            )
        };
        if !is_fn(self.sema.kind_of(m)) || !is_fn(self.sema.kind_of(x)) {
            return;
        }
        let Some(pdef) = self.sema.ast.definition(m) else { return };
        let Some(body) = self.sema.ast.decls.function(pdef).and_then(|f| f.body) else {
            return;
        };
        out.push(LocalMethod { class, f: x, pdef, body });
    }

    /// Defines the member functions `methods` of local class `outer` (and
    /// of its member classes), each in its re-entered class scope, then
    /// finishes `outer`'s class contexts.
    fn local_methods(&mut self, outer: DeclId, methods: &[LocalMethod]) {
        let poi = self.sema.ast.decls.get(outer).loc();
        self.sema.act_on_start_delayed_class(outer);
        for m in methods {
            let chain = self.enter_delayed_chain(outer, m.class);
            // Under its own frame, requested at the outermost local
            // class (clang `InstantiateClassMembers` from
            // `VisitCXXRecordDecl`, DR1484).
            if self.sema.ast.definition(m.f).is_none()
                && self.sema.push_inst_frame(FrameKind::MemberFunction, m.f, m.pdef, poi)
            {
                // The enclosing function's references still waiting for
                // the end of their full-expression (a tuple-like
                // binding's `get` call) are not the member's: its body is
                // a function of its own ([basic.def.odr]/10, clang's
                // pushed function scope and evaluation context).
                let odr = std::mem::take(&mut self.sema.pending_odr);
                let caps = std::mem::take(&mut self.sema.potential_caps);
                self.function_def(m.f, m.pdef, m.body);
                self.sema.pending_odr = odr;
                self.sema.potential_caps = caps;
                self.sema.pop_inst_frame();
            }
            self.leave_delayed_chain(&chain);
        }
        self.sema.act_on_finish_delayed_class(outer);
        self.sema.act_on_finish_class_contexts(outer);
    }

    /// Re-enters the scopes of the member classes between local class
    /// `outer` (already entered) and `class`, outermost first; returns
    /// them for [`Self::leave_delayed_chain`].
    fn enter_delayed_chain(&mut self, outer: DeclId, class: DeclId) -> Vec<DeclId> {
        let mut chain = Vec::new();
        let mut c = Some(class);
        while let Some(r) = c.filter(|&r| r != outer) {
            chain.push(r);
            if chain.len() > 1024 {
                break;
            }
            c = self.sema.ast.decls.get(r).ctx();
        }
        for &r in chain.iter().rev() {
            self.sema.act_on_start_delayed_class(r);
        }
        chain
    }

    /// Leaves the scopes entered by [`Self::enter_delayed_chain`].
    fn leave_delayed_chain(&mut self, chain: &[DeclId]) {
        for &r in chain {
            self.sema.act_on_finish_delayed_class(r);
        }
    }

    /// Substitutes the queued parts `late` of local class `outer`'s
    /// members (and its member classes'), now that `outer` is complete:
    /// the declaration parts, then the default member initializers, each
    /// in its re-entered class scope. One that fails with nothing
    /// diagnosed is an internal error (§4.6, WP-905).
    fn local_late_parts(&mut self, outer: DeclId, late: &[LocalLate]) {
        if late.is_empty() {
            return;
        }
        self.sema.act_on_start_delayed_class(outer);
        let decls = late.iter().filter(|p| !matches!(p, LocalLate::Nsdmi { .. }));
        let inits = late.iter().filter(|p| matches!(p, LocalLate::Nsdmi { .. }));
        for &part in decls.chain(inits) {
            let Some(class) = self.sema.ast.decls.get(part.decl()).ctx() else { continue };
            let chain = self.enter_delayed_chain(outer, class);
            let entity = self.entity.replace(class);
            let mark = self.sema.failure_mark();
            let what = match part {
                LocalLate::DefaultArgs { pat, f } => {
                    self.default_args(pat, f);
                    self.sema.ast.refresh_min_args(f);
                    self.missing_default(pat, f)
                }
                LocalLate::ExcSpec { pat, f } => self.local_exc_spec(pat, f),
                LocalLate::Nsdmi { field, init } => {
                    self.nsdmi(field, init);
                    self.sema
                        .ast
                        .decls
                        .get(field)
                        .has(DeclFlags::INVALID)
                        .then(|| "the default member initializer".to_owned())
                }
            };
            if let Some(what) = what
                && self.sema.failed_silently_since(mark)
            {
                self.part_dropped(&what, part.decl(), class);
            }
            self.entity = entity;
            self.leave_delayed_chain(&chain);
        }
        self.sema.act_on_finish_delayed_class(outer);
    }

    /// The first parameter of `f` whose default argument (from pattern
    /// `pat`) is still missing, described for [`Self::part_dropped`].
    fn missing_default(&self, pat: DeclId, f: DeclId) -> Option<String> {
        let params = |x: DeclId| -> Vec<DeclId> {
            self.sema
                .ast
                .decls
                .function(x)
                .map(|fd| self.sema.ast.lists.iter::<DeclId>(fd.params).collect())
                .unwrap_or_default()
        };
        let (pp, fp) = (params(pat), params(f));
        if pp.len() != fp.len() {
            return None;
        }
        pp.iter().zip(&fp).find_map(|(&a, &b)| {
            let had = self.sema.ast.decls.param(a).is_some_and(|x| x.default_arg.is_some());
            let has = self.sema.ast.decls.param(b).is_some_and(|x| x.default_arg.is_some());
            (had && !has)
                .then(|| format!("the default argument of parameter '{}'", self.sema.decl_name(b)))
        })
    }

    /// The *noexcept-specifier* of local class member `f` from pattern
    /// `pat`, substituted with `f`'s parameters and `this`
    /// ([basic.scope.param], [expr.prim.this]/2); a failure leaves `f`
    /// potentially throwing, as clang. Returns a description when it
    /// failed.
    fn local_exc_spec(&mut self, pat: DeclId, f: DeclId) -> Option<String> {
        use crate::templ::stack::FrameKind;
        let pty = self.sema.ast.decls.get(pat).ty()?;
        let pp = self.sema.sugared_proto(pty)?;
        let ExcSpec::NoexceptExpr { expr, value: NoexceptValue::Dependent(_) } =
            self.sema.ast.types.exc_spec(&pp)
        else {
            let spec = self.sema.ast.types.exc_spec(&pp);
            self.sema.set_member_exception_spec(f, spec);
            return None;
        };
        let at = self.sema.ast.exprs.get(expr).loc();
        let start = self.sema.begin_loc(expr);
        let mut spec = ExcSpec::None;
        let mut failed = true;
        if self.sema.push_inst_frame(FrameKind::ExceptionSpec, f, pat, at) {
            let params = |s: &Self, x: DeclId| -> Vec<DeclId> {
                s.sema
                    .ast
                    .decls
                    .function(x)
                    .map(|fd| s.sema.ast.lists.iter::<DeclId>(fd.params).collect())
                    .unwrap_or_default()
            };
            let (ps, fs) = (params(self, pat), params(self, f));
            self.map_params(&ps, &fs);
            let method = self.sema.is_implicit_object_member(f);
            let this = self.sema.act_on_start_this_scope(method, pp.info().this_quals());
            if let Ok(e) = self.expr(expr) {
                spec = self.sema.act_on_noexcept_spec(e, at, start);
                failed = false;
            }
            self.sema.act_on_end_this_scope(this);
            self.forget_pattern_params(pat);
            self.sema.pop_inst_frame();
        }
        self.sema.set_member_exception_spec(f, spec);
        failed.then(|| "the exception specification".to_owned())
    }

    /// The internal error for part `what` of member `d` of local class
    /// `class` that failed to instantiate with nothing diagnosed (§4.6).
    pub(crate) fn part_dropped(&mut self, what: &str, d: DeclId, class: DeclId) {
        let loc = self.sema.ast.decls.get(d).loc();
        let in_ = self.ctx_arg(class);
        let name = self.sema.decl_name(d);
        let diag = self
            .sema
            .diag(DiagId::sema_inst_part_dropped, loc)
            .arg(format!("{what} of '{name}'"))
            .arg(in_);
        self.sema.emit(diag);
    }
}
