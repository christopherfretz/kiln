//! Expansion statements ([stmt.expand], P1306R5): `template for`.
//!
//! The for-range-declaration and the body (a compound-statement, or any
//! statement as an extension) are a template pattern of a synthetic
//! template level (spec/sema.md "Expansion statements"): the declaration
//! is initialized from a `CXXExpansionSelectExpr` naming that level. Once
//! the expansion-initializer is not dependent (at once outside a template,
//! else when the enclosing template is instantiated), the statement is
//! expanded: the form is classified ([stmt.expand]/3), its prelude
//! declared, and the pattern instantiated once per element, the select
//! becoming the element (spec/sema-templates.md §5.5).

use kiln_ast::decl::{DeclFlags, DeclKind, InitStyle, VarSpec, VarStorage};
use kiln_ast::expr::{BinaryOp, ExprKind, NonOdrUse, Radix, UnaryOp, ValueCategory};
use kiln_ast::ids::ArgListId;
use kiln_ast::names::DeclName;
use kiln_ast::stmt::{ExpansionData, ExpansionForm, StmtKind};
use kiln_ast::types::{AutoKeyword, Quals};
use kiln_ast::{DeclHead, DeclId, ExprId, ListId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;
use crate::conv;
use crate::declspec::{
    BindingId, Chunk, DeclSpec, Declarator, DeclaratorContext, Decomposition, StorageSpec,
    TypeSpecKind,
};
use crate::expr::CastStyle;
use crate::init::{InitList, InitValue, Initializer};
use crate::scope::ScopeKind;
use crate::templ::args::{LevelMode, MultiLevelArgs};
use crate::templ::subst::{SubstFail, SubstResult, Substituter};

/// The most elements an iterating expansion statement may have: its
/// size is counted by evaluating `begin + i != end` for each `i`.
const MAX_ITERATING: u64 = 1 << 20;

/// The outcome of [`Sema::expansion_count_elements`] when it does not
/// fall back.
enum OnePass {
    /// The count reached `end`: the elements, in order.
    Elements(Vec<Element>),
    /// The 2^20 element limit was hit (diagnosed).
    Capped,
}

/// An expansion-initializer as parsed.
#[derive(Debug)]
pub enum ExpansionInit {
    /// An expression.
    Expr(ExprId),
    /// An expansion-init-list `{ a, b }`.
    List(InitList),
}

/// The head of an expansion statement whose body is being
/// parsed (inside its synthetic template level).
#[derive(Clone, Copy, Debug)]
pub struct ExpansionHead {
    init: Option<StmtId>,
    decl: DeclId,
    exprs: ListId,
    range_init: Option<ExprId>,
    colon: Loc,
    loc: Loc,
    depth: u16,
    form: ExpansionForm,
}

/// An element of an expansion statement being expanded: what its
/// `CXXExpansionSelectExpr` at `depth` becomes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ExpansionElem {
    /// The synthetic template level.
    pub depth: u16,
    /// The element expression (used once).
    pub elem: ExprId,
}

/// The expansion-initializer of a statement being expanded.
enum ExpansionSource {
    /// The expressions of an expansion-init-list.
    List(Vec<ExprId>),
    /// An expression.
    Expr(ExprId),
}

/// One element's code before the for-range-declaration: an optional
/// declaration statement (an iterating form's `iter`), and the element.
type Element = (Option<StmtId>, ExprId);

/// How an expansion statement's range is declared: the iterating form's
/// `__range` ([`Sema::expansion_iterating`]: [`RangeBind::Automatic`] or
/// [`RangeBind::Runtime`], as the working draft says), or a `constexpr`
/// destructuring form's `auto&& [__u...]` ([`Sema::expansion_fallback`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RangeBind {
    /// Destructuring only: `static constexpr auto&&`, a reference to an
    /// object with static storage duration or to a static
    /// lifetime-extended temporary.
    Static,
    /// `constexpr` with automatic storage duration, as [stmt.expand]/5
    /// declares it for a `constexpr` for-range-declaration: a reference
    /// to an automatic object of the same function is
    /// constexpr-referenceable by P2686R5 ([expr.const.init]/2). The
    /// iterating form's `__begin`, `__end` and `__iter` are automatic
    /// `constexpr` variables too: one with static storage duration could
    /// not point into it ([expr.const.init]/4).
    /// A `constexpr` destructuring form over a prvalue binds its
    /// lifetime-extended temporary this way too (WP-1512; the range is
    /// never copied).
    Automatic,
    /// The iterating form of a for-range-declaration that is not
    /// `constexpr`: [stmt.expand]/5.2's `decltype(auto) range = (init);
    /// auto begin = begin-expr;`, automatic and not `constexpr`, as are
    /// the elements' `__iter`; N is counted on `begin-expr` and
    /// `end-expr`, which name `range`, a reference of unknown referent
    /// (P2280).
    Runtime,
}

impl RangeBind {
    /// Whether the implicit variables have automatic storage duration.
    fn automatic(self) -> bool {
        matches!(self, Self::Automatic | Self::Runtime)
    }
}

/// Where the uses an expansion statement's initializer names start
/// ([`Sema::expansion_init_mark`]).
#[derive(Clone, Copy, Debug)]
pub struct ExpansionInitMark {
    pending: usize,
    potential: usize,
}

impl Sema<'_, '_> {
    /// The start of an expansion-initializer or expansion-init-list
    /// (before it is parsed or substituted).
    pub fn expansion_init_mark(&self) -> ExpansionInitMark {
        ExpansionInitMark { pending: self.pending_odr.len(), potential: self.potential_caps.len() }
    }

    /// The end of an expansion statement's expansion-initializer (or
    /// expansion-init-list) begun at `mark` ([basic.def.odr]/5,
    /// [expr.prim.lambda.capture]/7-8, [stmt.expand]/5). It is no
    /// full-expression of its own: whether a variable named in it is
    /// odr-used depends on the form and the for-range-declaration, both
    /// decided per expansion (`auto&& __range = b` binds a reference and
    /// odr-uses `b`; an element `b.x` copied into `int e` does not), so
    /// the uses are decided by the full-expressions of the declarations
    /// the expansion builds: the pending odr-uses noted since `mark` are
    /// dropped here and noted again where the expression is used
    /// ([`Sema::note_expansion_uses`]), one element at a time. In a
    /// template, the potential captures are resolved as those of an
    /// instantiation-dependent full-expression: a lambda with a
    /// capture-default captures them now (its closure is complete before
    /// any expansion), and a use no lambda can capture waits for the
    /// expansion, whose odr-uses are diagnosed then (`capture_chain`).
    pub fn act_on_expansion_initializer_captures(&mut self, mark: ExpansionInitMark) {
        self.pending_odr.truncate(mark.pending);
        if self.potential_caps.len() > mark.potential && self.in_template() {
            let caps = self.potential_caps.split_off(mark.potential);
            let none = crate::expr::ConstantCandidates::new(Vec::new());
            self.resolve_potential_capture_list(caps, true, &none, None);
        }
    }

    /// The variables expression `e` names, potentially evaluated, as
    /// pending odr-uses of the full-expression being built: `e` is an
    /// expansion-initializer or an element, used by an implicit or
    /// element declaration of an expansion statement after its own uses
    /// were dropped ([`Sema::act_on_expansion_initializer_captures`]).
    /// Lambdas and unevaluated operands are not entered (their uses were
    /// decided where they were built).
    pub(crate) fn note_expansion_uses(&mut self, e: ExprId) {
        if self.unevaluated > 0 || self.in_template() {
            return;
        }
        let mut stack = self.take_walk_stack(e);
        while let Some(x) = stack.pop() {
            let ex = &self.ast.exprs;
            match ex.get(x).kind() {
                ExprKind::DeclRefExpr => {
                    let d = ex.decl(x);
                    if ex.non_odr_use(x) == NonOdrUse::None
                        && matches!(self.kind_of(d), DeclKind::Var | DeclKind::ParmVar)
                    {
                        self.pending_odr.push(x);
                    }
                }
                ExprKind::LambdaExpr
                | ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::ExpressionTraitExpr
                | ExprKind::RequiresExpr
                | ExprKind::ConceptSpecializationExpr => {}
                ExprKind::CXXTypeidExpr if !ex.typeid_potentially_evaluated(x) => {}
                _ => ex.children_into(x, &self.ast.lists, &mut stack),
            }
        }
        self.put_walk_stack(stack);
    }

    /// `template for ( init-statement_opt for-range-declaration :
    /// expansion-initializer )` ([stmt.expand]/1): opens the synthetic
    /// template level and declares the for-range-declaration in it,
    /// initialized from the element select. `None` when the statement is
    /// dropped (diagnosed); the body is then parsed as dead code.
    #[allow(clippy::too_many_arguments, reason = "the parts of the head")]
    pub fn act_on_expansion_head(
        &mut self,
        init: Option<StmtId>,
        ds: &DeclSpec,
        base: QualType,
        d: &Declarator,
        range: ExpansionInit,
        colon: Loc,
        loc: Loc,
    ) -> Option<ExpansionHead> {
        let (exprs, range_init, form) = match range {
            ExpansionInit::List(l) => {
                // [stmt.expand]/1: the list is an expression-list, whose
                // initializer-clauses may be braced-init-lists but not
                // designated (clang 23 parses it with
                // `ParseExpressionList`: "expected expression" at the
                // designator).
                if let Some(d) = l.items.iter().find_map(|i| i.designator.first()) {
                    self.error(DiagId::parse_expected_expression, d.loc());
                    return None;
                }
                // A braced element is kept as written (a dependent
                // `InitListExpr`): it list-initializes the
                // for-range-declaration of its own expansion ([stmt.expand]/5.1,
                // `decl = {…};`).
                let es: Vec<ExprId> = l
                    .items
                    .iter()
                    .map(|i| match &i.value {
                        InitValue::Expr(e) => *e,
                        InitValue::List(n) => self.dependent_init_list(n),
                    })
                    .collect();
                (self.ast.lists.push(&es), None, ExpansionForm::Enumerating)
            }
            ExpansionInit::Expr(e) => (ListId::EMPTY, Some(e), ExpansionForm::Dependent),
        };
        let depth = self.push_expansion_level(loc);
        let v = self.act_on_declarator(ds, base, d).filter(|&v| self.act_on_for_range_decl(ds, v));
        let Some(v) = v else {
            self.pop_expansion_level();
            return None;
        };
        let sel = self.builder().cxx_expansion_select(form, depth, colon);
        self.act_on_init(v, Initializer::Expr(sel));
        Some(ExpansionHead { init, decl: v, exprs, range_init, colon, loc, depth, form })
    }

    /// The expansion statement `h` with body `body` (a compound-statement,
    /// or any statement as an extension): a pattern in a template, else
    /// expanded now.
    pub fn act_on_finish_expansion(&mut self, h: ExpansionHead, body: StmtId) -> Option<StmtId> {
        self.pop_expansion_level();
        let data = ExpansionData {
            init: h.init,
            decl: h.decl,
            exprs: h.exprs,
            range_init: h.range_init,
            body,
            prelude: ListId::EMPTY,
            expansions: ListId::EMPTY,
            colon: h.colon,
        };
        if self.in_template() {
            return Some(self.builder().cxx_expansion_stmt(data, h.form, false, h.depth, h.loc));
        }
        // The pattern's declaration leaves the scope of the head: each
        // element redeclares it in a block of its own.
        let mut pattern = self.decomp_binding_list(h.decl);
        pattern.push(h.decl);
        if let Some(sc) = self.scopes.last_mut() {
            sc.locals.retain(|&(_, d, _)| !pattern.contains(&d));
        }
        let src = match h.range_init {
            Some(e) => ExpansionSource::Expr(e),
            None => ExpansionSource::List(self.ast.lists.iter::<ExprId>(h.exprs).collect()),
        };
        let mut args = MultiLevelArgs::new();
        for _ in 0..h.depth {
            args.push(ArgListId::EMPTY, self.ctx, LevelMode::Substitute);
        }
        let mut sub = Substituter::new(self, &args);
        sub.expand_expansion(h.init, src, &data, h.depth, h.loc).ok()
    }

    /// An implicit `constexpr` variable of an iterating expansion
    /// statement ([stmt.expand]/5.2), not found by lookup: `static`
    /// unless `bind` is automatic, and not `constexpr` for
    /// [`RangeBind::Runtime`].
    fn expansion_var(&mut self, name: &str, ty: QualType, bind: RangeBind, loc: Loc) -> DeclId {
        let sym = self.intern(name);
        let mut h = DeclHead::new(self.ctx, DeclName::ident(sym), loc);
        h.flags = h.flags.with(DeclFlags::IMPLICIT);
        let storage = if bind.automatic() { VarStorage::None } else { VarStorage::Static };
        let spec = VarSpec::new()
            .with_storage(storage)
            .with_constexpr(bind != RangeBind::Runtime)
            .with_init_style(InitStyle::C);
        self.builder().var(h, ty, spec, None)
    }

    /// Whether `d` is the error of a use of an enclosing function's
    /// local that cannot be captured (`err_lambda_impcap` and kin).
    fn is_capture_error(d: &kiln_diag::Diag) -> bool {
        matches!(
            d.id(),
            Some(
                DiagId::sema_lambda_impcap
                    | DiagId::sema_lambda_capture_anonymous_var
                    | DiagId::sema_reference_to_local_in_enclosing
            )
        )
    }

    /// The diagnostics `stash` of a failed implicit declaration
    /// initialized from range `e`: when `e` names a local of an enclosing
    /// function that the lambda could not capture (its capture error in
    /// `stash` or already emitted), only the capture errors and the
    /// non-errors; else `stash` unchanged.
    fn drop_after_capture_error(
        &mut self,
        stash: Vec<kiln_diag::Diag>,
        e: ExprId,
    ) -> Vec<kiln_diag::Diag> {
        if !stash.iter().any(Self::is_capture_error) && !self.names_uncaptured_local(e) {
            return stash;
        }
        stash
            .into_iter()
            .filter(|d| {
                Self::is_capture_error(d)
                    || d.id().is_some_and(|id| {
                        self.diags.severity_at(id, d.loc()) < kiln_diag::Severity::Error
                    })
            })
            .collect()
    }

    /// Initializes implicit variable `v` from `e` (deducing its type);
    /// false after an error.
    fn init_expansion_var(&mut self, v: DeclId, e: ExprId, loc: Loc, id: DiagId) -> bool {
        !self.finish_range_var(v, Initializer::Expr(e), loc, id)
            && !self.ast.decls.get(v).has(DeclFlags::INVALID)
    }

    /// The `constexpr` declaration of a destructuring expansion
    /// statement over `e`, built by `f(bind)` (spec/sema.md "Expansion
    /// statements"), always by reference ([stmt.expand]/5.3 declares
    /// `constexpr auto&& [u...] = e`): a prvalue's temporary is bound by
    /// the automatic `constexpr` reference ([`RangeBind::Automatic`], in
    /// every mode; a reference to an automatic temporary is a constant
    /// expression only by P2686R5, and the temporary is not const, so an
    /// element that reads it is no constant expression: WP-1512, where it
    /// was copied into a `static constexpr auto`). A glvalue: `static` first
    /// ([`RangeBind::Static`]), then, at C++26, automatic
    /// ([`RangeBind::Automatic`], P2686R5: the `constexpr` reference
    /// [stmt.expand]/5.3 declares, to a local of this function). When
    /// neither binds, the first attempt's diagnostics are emitted.
    fn expansion_fallback<T>(
        &mut self,
        e: ExprId,
        f: impl Fn(&mut Self, RangeBind) -> Option<T>,
    ) -> Option<T> {
        let attempt = |s: &mut Self, bind: RangeBind| {
            let (r, stash) = s.capturing(|s| f(s, bind));
            let r = r.filter(|_| !s.stash_has_error(&stash));
            (r, stash)
        };
        let glvalue = self.ast.exprs.get(e).value_category().is_glvalue();
        let first = if glvalue { RangeBind::Static } else { RangeBind::Automatic };
        let (r, stash) = attempt(self, first);
        if r.is_some() {
            self.release(stash, Vec::new());
            return r;
        }
        if !glvalue {
            self.release(stash, Vec::new());
            return None;
        }
        if self.std >= kiln_lex::Std::Cxx26 {
            let (r2, stash2) = attempt(self, RangeBind::Automatic);
            if r2.is_some() {
                self.release(stash2, Vec::new());
                return r2;
            }
        }
        // A glvalue is never copied: [stmt.expand]/5.3 binds it by
        // `constexpr auto&&`, so a reference to an object that is not
        // constexpr-referenceable (an automatic variable before C++26,
        // or one that is not usable in constant expressions) is ill-formed
        // even when the destructuring is empty (CWG3048; GCC 16.1 agrees).
        // A local the lambda did not capture is diagnosed once, by the
        // capture error.
        let stash = self.drop_after_capture_error(stash, e);
        self.release(stash, Vec::new());
        None
    }

    /// The iterating form ([stmt.expand]/5.2), as the working draft
    /// declares it, in one path (spec/sema.md "Iterating expansion
    /// statements"): `constexpr_opt decltype(auto) range = (init);` and
    /// `constexpr_opt auto begin = begin-expr;`, automatic, `constexpr`
    /// exactly when the for-range-declaration is (`constexpr_decl`); per
    /// element `constexpr_opt auto iter = begin + i;` and the element
    /// `*iter`. N is counted on `begin-expr` and `end-expr` as a constant
    /// expression: on the `constexpr` variables `__begin` and `__end`
    /// when they are (`__end` is not part of the rewrite: it is built
    /// only to count, and never declared in the prelude), else on their
    /// initializers ([`Sema::expansion_runtime_size`]). These implicit
    /// variables follow C++26 in every mode (`Sema::expansion_vars`). The
    /// prelude and the elements.
    fn expansion_iterating(
        &mut self,
        e: ExprId,
        constexpr_decl: bool,
        colon: Loc,
    ) -> Option<(Vec<StmtId>, Vec<Element>)> {
        let bind = if constexpr_decl { RangeBind::Automatic } else { RangeBind::Runtime };
        let range_loc = self.begin_loc(e);
        let range_ty = self.ety(e);
        let auto = self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false);
        // `decltype(auto) range = (init);`: a reference for a glvalue
        // (which `auto&&` deduces alike), an object for a prvalue.
        let glvalue = self.ast.exprs.get(e).value_category().is_glvalue();
        let ty = if glvalue { self.ast.types.rvalue_ref_to(auto) } else { auto };
        let range = self.expansion_var("__range", ty, bind, range_loc);
        self.expansion_vars.push(range);
        // A `constexpr` range that is not constant-initialized is
        // diagnosed and not marked invalid: stop there too.
        let (ok, stash) = self.capturing(|s| {
            s.note_expansion_uses(e);
            s.init_expansion_var(range, e, range_loc, DiagId::sema_for_range_deduction_failure)
        });
        let ok = ok && !self.stash_has_error(&stash);
        if !ok {
            // A local of an enclosing function the lambda did not
            // capture: one error for one mistake. `__range` is invalid,
            // and its failed constant initialization is not diagnosed
            // again ("constexpr variable '__range' must be initialized by
            // a constant expression").
            let stash = self.drop_after_capture_error(stash, e);
            self.ast.decls.set_flags(range, DeclFlags::INVALID);
            self.release(stash, Vec::new());
            return None;
        }
        self.release(stash, Vec::new());
        if !ok {
            return None;
        }
        if bind == RangeBind::Runtime && glvalue {
            // [class.temporary]/9: the expansion-initializer's temporaries
            // persist for the lifetime of the reference it initializes,
            // "if any": a prvalue initializes the object `range` itself,
            // and its temporaries end with the declaration. (A `constexpr`
            // range's are part of its constant initialization.)
            self.extend_init_temporaries(range);
        }
        let vt = if constexpr_decl { self.ast.types.qualify(auto, Quals::CONST) } else { auto };
        let begin = self.expansion_var("__begin", vt, bind, colon);
        let end = self.expansion_var("__end", vt, bind, colon);
        self.expansion_vars.extend([begin, end]);
        let br = self.build_decl_ref(range, colon);
        let er = self.build_decl_ref(range, colon);
        let (ok, stash) =
            self.capturing(|s| s.expansion_begin_end(br, er, range_ty, (begin, end), colon));
        let ok = ok && !self.stash_has_error(&stash);
        self.release(stash, Vec::new());
        if !ok || [begin, end].iter().any(|&v| self.ast.decls.get(v).has(DeclFlags::INVALID)) {
            return None;
        }
        let diff = self.expansion_difference_type(begin, colon)?;
        let inc = self.expansion_increment(begin, colon).ok()?;
        let prelude = vec![
            self.act_on_decl_stmt(&[range], range_loc),
            self.act_on_decl_stmt(&[begin], colon),
        ];
        let n = if bind == RangeBind::Runtime {
            self.expansion_runtime_size(begin, end, diff, colon)?
        } else if let Some(n) = self.expansion_size_difference(begin, end, colon) {
            n
        } else {
            if let Some(one) = self.expansion_count_elements(begin, end, bind, diff, colon) {
                return match one {
                    OnePass::Elements(elems) => {
                        self.expansion_increment_constant(inc, elems.len() as u64, colon)?;
                        Some((prelude, elems))
                    }
                    OnePass::Capped => None,
                };
            }
            self.expansion_size(begin, end, diff, colon)?
        };
        self.expansion_increment_constant(inc, n, colon)?;
        let mut elems = Vec::new();
        for i in 0..n {
            let iter = self.expansion_iter(begin, i, bind, diff, colon)?;
            let ds = self.act_on_decl_stmt(&[iter], colon);
            elems.push((Some(ds), self.expansion_deref(iter, colon)?));
        }
        Some((prelude, elems))
    }

    /// Element `i`'s `static constexpr auto __iter = begin + i;`
    /// ([stmt.expand]/5.2; not `static` when `automatic`), initialized;
    /// `None` after an error.
    fn expansion_iter(
        &mut self,
        begin: DeclId,
        i: u64,
        bind: RangeBind,
        diff: QualType,
        colon: Loc,
    ) -> Option<DeclId> {
        let cauto = {
            let auto = self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false);
            if bind == RangeBind::Runtime {
                auto
            } else {
                self.ast.types.qualify(auto, Quals::CONST)
            }
        };
        let iter = self.expansion_var("__iter", cauto, bind, colon);
        self.expansion_vars.push(iter);
        let b = self.build_decl_ref(begin, colon);
        let lit = self.expansion_index(diff, i, colon);
        let add = self.act_on_binary(BinaryOp::Add, b, lit, colon);
        (!self.is_err_expr(add)
            && self.init_expansion_var(
                iter,
                add,
                colon,
                DiagId::sema_for_range_iter_deduction_failure,
            ))
        .then_some(iter)
    }

    /// `decltype(begin - begin)` ([stmt.expand]/5.2): the type of each
    /// element's index `i`, so `begin - begin` must be well-formed (an
    /// unevaluated operand). `None` after it is diagnosed.
    fn expansion_difference_type(&mut self, begin: DeclId, colon: Loc) -> Option<QualType> {
        self.enter_unevaluated();
        let l = self.build_decl_ref(begin, colon);
        let r = self.build_decl_ref(begin, colon);
        let d = self.act_on_binary(BinaryOp::Sub, l, r, colon);
        self.exit_unevaluated();
        if self.is_err_expr(d) {
            return None;
        }
        let t = self.ety(d);
        let t = if self.ast.exprs.get(d).value_category().is_glvalue() {
            self.ast.types.lvalue_ref_to(t)
        } else {
            t
        };
        Some(t)
    }

    /// `++b` ([stmt.expand]/5.2): N is counted by `for (auto b =
    /// begin-expr; b != e; ++b)`, so the iterator must be incrementable
    /// as a non-const lvalue of `begin`'s type even though kiln counts
    /// with `begin + i` (GCC 16.1 checks it too). Built unevaluated on an
    /// implicit variable that is never declared; `Err` after "cannot
    /// increment value of type" (or the overload error) is diagnosed,
    /// else the `operator++` it calls when that is not `constexpr`
    /// ([`Sema::expansion_increment_constant`]).
    fn expansion_increment(&mut self, begin: DeclId, colon: Loc) -> Result<Option<DeclId>, ()> {
        let bt = self.ast.decls.get(begin).ty().ok_or(())?;
        let bt = self.ast.types.unqualified(bt);
        let b = self.expansion_var("__b", bt, RangeBind::Runtime, colon);
        self.enter_unevaluated();
        let r = self.build_decl_ref(b, colon);
        let inc = self.act_on_unary(UnaryOp::PreInc, r, colon);
        self.exit_unevaluated();
        if self.is_err_expr(inc) {
            return Err(());
        }
        if self.ast.exprs.get(inc).kind() != ExprKind::CXXOperatorCallExpr {
            return Ok(None);
        }
        let f = self.callee_decl(inc).ok_or(())?;
        let f = self.ast.latest(f);
        let non_constexpr = self
            .ast
            .decls
            .function(f)
            .is_some_and(|fd| fd.spec.constexpr_kind() == kiln_ast::decl::ConstexprKind::None);
        Ok(non_constexpr.then_some(f))
    }

    /// The count's `++b` is evaluated in the consteval lambda of
    /// [stmt.expand]/5.2 once per element, so with N > 0 a non-constexpr
    /// `operator++` `inc` makes the expansion size no constant
    /// expression: `sema_expansion_size_not_constant` with the
    /// evaluator's notes for a call of a non-constexpr function ("non-
    /// constexpr function 'operator++' cannot be used in a constant
    /// expression", "declared here"), as for a non-constexpr `begin`.
    /// GCC 16.1 rejects it too ("call to non-'constexpr' function").
    /// `None` after that error.
    fn expansion_increment_constant(
        &mut self,
        inc: Option<DeclId>,
        n: u64,
        colon: Loc,
    ) -> Option<()> {
        let Some(f) = inc.filter(|_| n > 0) else { return Some(()) };
        let name = self.decl_name(f);
        let floc = self.ast.decls.get(f).loc();
        let note = self.diag(DiagId::note_constexpr_invalid_function, colon).arg(name);
        let decl = self.diag(DiagId::note_declared_at, floc);
        let d = self
            .diag(DiagId::sema_expansion_size_not_constant, colon)
            .with_note(note)
            .with_note(decl);
        self.emit(d);
        None
    }

    /// `decltype(begin - begin){i}` ([stmt.expand]/5.2), `i` a prvalue
    /// of type `std::ptrdiff_t`: the literal itself when the difference
    /// type is `std::ptrdiff_t` (the language's and the standard
    /// library's iterators), else a list-initialization of `diff`.
    fn expansion_index(&mut self, diff: QualType, i: u64, colon: Loc) -> ExprId {
        let pd = QualType::unqual(self.target.ptrdiff_t);
        let lit = self.builder().int_literal(pd, i, Radix::Decimal, colon);
        if self.ast.types.canonical(diff) == self.ast.types.canonical(pd) {
            return lit;
        }
        let item = crate::init::InitItem { designator: Vec::new(), value: InitValue::Expr(lit) };
        let l = InitList { items: vec![item], lbrace: colon, rbrace: colon };
        self.act_on_list_cast(diff, &l, colon)
    }

    /// Element expression `*__iter`; `None` after an error.
    fn expansion_deref(&mut self, iter: DeclId, colon: Loc) -> Option<ExprId> {
        let r = self.build_decl_ref(iter, colon);
        let deref = self.act_on_unary(UnaryOp::Deref, r, colon);
        (!self.is_err_expr(deref)).then_some(deref)
    }

    /// The count and the elements of an iterating expansion outside the
    /// `end - begin` fast path, in one pass (spec/sema.md §7.6a, WP-467):
    /// element `i`'s `__iter` (`begin + i`) is built first and the count
    /// tests `__iter != end` on it, so `begin + i` is resolved and
    /// evaluated once per element rather than once for the count and
    /// once for the element (`begin != end` for `i == 0`, as
    /// [`Sema::expansion_size`]; the `__iter` built for `i == N` is
    /// dropped). `None` when anything fails, with nothing diagnosed: the
    /// separate count and elements then run and diagnose.
    /// [`OnePass::Capped`] after the 2^20 element limit is diagnosed.
    fn expansion_count_elements(
        &mut self,
        begin: DeclId,
        end: DeclId,
        bind: RangeBind,
        diff: QualType,
        colon: Loc,
    ) -> Option<OnePass> {
        let (r, stash) = self.capturing(|s| {
            if !s.expansion_more(begin, end, colon)? {
                return Some(OnePass::Elements(Vec::new()));
            }
            let mut elems = Vec::new();
            for i in 0..MAX_ITERATING {
                let iter = s.expansion_iter(begin, i, bind, diff, colon)?;
                if i > 0 && !s.expansion_more(iter, end, colon)? {
                    return Some(OnePass::Elements(elems));
                }
                let ds = s.act_on_decl_stmt(&[iter], colon);
                elems.push((Some(ds), s.expansion_deref(iter, colon)?));
            }
            Some(OnePass::Capped)
        });
        if r.is_none() || self.stash_has_error(&stash) {
            return None;
        }
        self.release(stash, Vec::new());
        if matches!(r, Some(OnePass::Capped)) {
            self.error(DiagId::sema_expansion_size_not_constant, colon);
        }
        r
    }

    /// Whether `it != end` is true, as a constant expression; `None`
    /// (nothing diagnosed but what building the expression diagnoses)
    /// when it is not one.
    fn expansion_more(&mut self, it: DeclId, end: DeclId, colon: Loc) -> Option<bool> {
        let it = self.build_decl_ref(it, colon);
        let e = self.build_decl_ref(end, colon);
        let ne = self.act_on_binary(BinaryOp::NE, it, e, colon);
        if self.is_err_expr(ne) {
            return None;
        }
        let c = conv::contextually_convert_to_bool(self, ne).ok()?;
        let v = self.eval_int(c, EvalContext::new(EvalKind::Other, colon)).ok()?;
        Some(v != 0)
    }

    /// N of an iterating expansion statement ([stmt.expand]/5.2): the
    /// first `i` for which `begin + i != end` is false, each evaluated
    /// as a constant expression (spec/sema.md: the wording counts with
    /// `++` in a consteval lambda; `begin + i` is what each element uses).
    /// A contiguous iterator whose arithmetic is the language's or the
    /// standard library's counts in one evaluation of `end - begin`
    /// ([`Sema::expansion_size_difference`], tried first by the caller);
    /// this count runs only when the one-pass
    /// [`Sema::expansion_count_elements`] fails, to diagnose.
    fn expansion_size(
        &mut self,
        begin: DeclId,
        end: DeclId,
        diff: QualType,
        colon: Loc,
    ) -> Option<u64> {
        for i in 0..MAX_ITERATING {
            let b = self.build_decl_ref(begin, colon);
            let it = if i == 0 {
                b
            } else {
                let lit = self.expansion_index(diff, i, colon);
                self.act_on_binary(BinaryOp::Add, b, lit, colon)
            };
            let e = self.build_decl_ref(end, colon);
            let ne = self.act_on_binary(BinaryOp::NE, it, e, colon);
            if self.is_err_expr(ne) {
                return None;
            }
            let c = match conv::contextually_convert_to_bool(self, ne) {
                Ok(c) => c,
                Err(err) => {
                    self.emit_conv_error(err, colon);
                    return None;
                }
            };
            match self.eval_int(c, EvalContext::new(EvalKind::Other, colon)) {
                Ok(0) => return Some(i),
                Ok(_) => {}
                Err(f) => {
                    let d = self.diag(DiagId::sema_expansion_size_not_constant, colon);
                    let d = Self::with_failure_notes(d, f);
                    self.emit(d);
                    return None;
                }
            }
        }
        self.error(DiagId::sema_expansion_size_not_constant, colon);
        None
    }

    /// The fast path of [`Sema::expansion_size`] (spec/sema.md §7.6a):
    /// when `begin` and `end` have the same type and it is a pointer to
    /// a complete object type or a standard library contiguous iterator
    /// ([`Sema::is_lib_contiguous_iterator`]), the first `i` with
    /// `begin + i != end` false is `end - begin` whenever that is a
    /// non-negative constant: `begin + i` for `i` in `[0, end - begin]`
    /// stays within the range and compares unequal to `end` exactly
    /// before it. `None` (the incremental count decides, and diagnoses)
    /// otherwise; nothing is diagnosed here.
    fn expansion_size_difference(&mut self, begin: DeclId, end: DeclId, colon: Loc) -> Option<u64> {
        let b = self.build_decl_ref(begin, colon);
        let e = self.build_decl_ref(end, colon);
        let (bt, et) = (self.ety(b), self.ety(e));
        if !self.ast.types.same_unqualified(bt, et) {
            return None;
        }
        let contiguous = match self.ast.types.pointee(bt) {
            Some(p) if self.ast.types.is_pointer(bt) => {
                crate::target::is_complete_object(&self.ast, p)
            }
            _ => self.is_lib_contiguous_iterator(bt),
        };
        if !contiguous {
            return None;
        }
        let d = self.act_on_binary(BinaryOp::Sub, e, b, colon);
        if self.is_err_expr(d) || !self.ast.types.is_integral(self.ety(d)) {
            return None;
        }
        let n = self.eval_int(d, EvalContext::new(EvalKind::Other, colon)).ok()?;
        u64::try_from(n).ok()
    }

    /// N of the iterating form with a for-range-declaration that is not
    /// `constexpr` ([`RangeBind::Runtime`], [stmt.expand]/5.2): the
    /// working draft's `[&] consteval { auto b = begin-expr; auto e =
    /// end-expr; for (; b != e; ++b) ++result; ... }()`, counted on the
    /// initializers of `__begin` and `__end`, which name the non-constexpr
    /// `__range`, a reference of unknown referent (P2280, in every mode
    /// for this variable). A contiguous iterator counts in one
    /// evaluation of `end-expr - begin-expr` when that is a non-negative
    /// constant (silently, as [`Sema::expansion_size_difference`]); else
    /// the first `i` for which `begin-expr + i != end-expr` is false,
    /// each a constant expression, capped at 2^20. A test that is not a
    /// constant expression is `sema_expansion_size_not_constant` with
    /// the evaluator's notes (why `begin-expr` or `end-expr` is not one).
    fn expansion_runtime_size(
        &mut self,
        begin: DeclId,
        end: DeclId,
        diff: QualType,
        colon: Loc,
    ) -> Option<u64> {
        let b = self.ast.decls.var(begin).and_then(|x| x.init)?;
        let e = self.ast.decls.var(end).and_then(|x| x.init)?;
        let (bt, et) = (self.ety(b), self.ety(e));
        let contiguous = self.ast.types.same_unqualified(bt, et)
            && match self.ast.types.pointee(bt) {
                Some(p) if self.ast.types.is_pointer(bt) => {
                    crate::target::is_complete_object(&self.ast, p)
                }
                _ => self.is_lib_contiguous_iterator(bt),
            };
        if contiguous {
            let (r, stash) = self.capturing(|s| {
                let d = s.act_on_binary(BinaryOp::Sub, e, b, colon);
                if s.is_err_expr(d) || !s.ast.types.is_integral(s.ety(d)) {
                    return None;
                }
                let n = s.eval_int(d, EvalContext::new(EvalKind::Other, colon)).ok()?;
                u64::try_from(n).ok()
            });
            if r.is_some() && !self.stash_has_error(&stash) {
                self.release(stash, Vec::new());
                return r;
            }
        }
        for i in 0..MAX_ITERATING {
            let it = if i == 0 {
                b
            } else {
                let lit = self.expansion_index(diff, i, colon);
                self.act_on_binary(BinaryOp::Add, b, lit, colon)
            };
            let ne = self.act_on_binary(BinaryOp::NE, it, e, colon);
            if self.is_err_expr(ne) {
                return None;
            }
            let c = match conv::contextually_convert_to_bool(self, ne) {
                Ok(c) => c,
                Err(err) => {
                    self.emit_conv_error(err, colon);
                    return None;
                }
            };
            match self.eval_int(c, EvalContext::new(EvalKind::Other, colon)) {
                Ok(0) => return Some(i),
                Ok(_) => {}
                Err(f) => {
                    let d = self.diag(DiagId::sema_expansion_size_not_constant, colon);
                    let d = Self::with_failure_notes(d, f);
                    self.emit(d);
                    return None;
                }
            }
        }
        self.error(DiagId::sema_expansion_size_not_constant, colon);
        None
    }

    /// Whether class type `t` is one of the standard library's
    /// contiguous iterator adaptors over a pointer, whose `+`, `-` and
    /// `!=` are the pointer's: libc++'s `std::__wrap_iter`,
    /// `std::__bounded_iter` and `std::__static_bounded_iter`, and
    /// libstdc++'s `__gnu_cxx::__normal_iterator` (the iterators of
    /// `std::array`, `std::span`, `std::basic_string_view`,
    /// `std::basic_string` and `std::vector` are these or pointers).
    fn is_lib_contiguous_iterator(&self, t: QualType) -> bool {
        let Some(r) = self.ast.types.as_record(t) else { return false };
        let Some(name) = self.ast.decls.get(r).name().as_ident() else { return false };
        let Some((_, spec)) = self.ast.spec_info(r) else { return false };
        let over_pointer =
            match self.ast.types.args(spec.args).first().map(kiln_ast::types::TemplateArg::view) {
                Some(kiln_ast::types::ArgView::Type(p)) => self.ast.types.is_pointer(p),
                _ => false,
            };
        if !over_pointer {
            return false;
        }
        match self.ast.symbols().as_str(name) {
            "__wrap_iter" | "__bounded_iter" | "__static_bounded_iter" => {
                self.is_in_std_namespace(r)
            }
            "__normal_iterator" => {
                let Some(ns) = self.ast.decls.get(r).ctx().map(|c| self.ast.redecl_context(c))
                else {
                    return false;
                };
                let top = self.ast.decls.get(ns).ctx().map(|p| self.ast.redecl_context(p));
                self.kind_of(ns) == DeclKind::Namespace
                    && self.ast.decls.get(ns).name().as_ident() == Some(self.intern("__gnu_cxx"))
                    && top.is_some_and(|p| self.kind_of(p) == DeclKind::TranslationUnit)
            }
            _ => false,
        }
    }

    /// The destructuring form ([stmt.expand]/5.3): `constexpr_opt auto&&
    /// [u...] = init;` (`constexpr` when the for-range-declaration is),
    /// the elements its bindings. The prelude and the elements.
    fn expansion_destructuring(
        &mut self,
        e: ExprId,
        constexpr: bool,
        colon: Loc,
    ) -> Option<(Vec<StmtId>, Vec<Element>)> {
        let (v, prelude, elems) = if constexpr {
            // Each attempt's bindings stay out of the enclosing scope.
            self.expansion_fallback(e, |s, bind| {
                s.push_scope(ScopeKind::Block);
                let r = s.expansion_decomp(e, Some(bind), colon);
                s.pop_scope();
                r
            })?
        } else {
            self.expansion_decomp(e, None, colon)?
        };
        // [class.temporary]/9 (CWG3043): the expansion-initializer's
        // temporaries persist for the lifetime of the reference (an
        // automatic one; a `static constexpr` one's are part of its
        // constant initialization). Extended only once the binding that
        // wins is known: `e` is shared by every attempt, and an
        // abandoned attempt must not claim its temporaries.
        self.extend_init_temporaries(v);
        Some((prelude, elems))
    }

    /// `auto&& [__u...] = e;`, or for a `constexpr` one (`bind` given)
    /// `[static] constexpr auto&& [__u...] = e;` as `bind` says: the
    /// decomposition, its declaration and its bindings.
    fn expansion_decomp(
        &mut self,
        e: ExprId,
        bind: Option<RangeBind>,
        colon: Loc,
    ) -> Option<(DeclId, Vec<StmtId>, Vec<Element>)> {
        let mut ds = DeclSpec::new(colon);
        let _ = ds.set_base(TypeSpecKind::Auto, None, colon);
        if let Some(bind) = bind {
            ds.constexpr = Some(colon);
            if bind != RangeBind::Automatic {
                let _ = ds.set_storage(StorageSpec::Static, colon);
            }
        }
        let mut d = Declarator::new(DeclaratorContext::Block, colon);
        d.chunks.push(Chunk::RRef { loc: colon });
        let ids = vec![BindingId {
            name: self.intern("__u"),
            loc: colon,
            ellipsis: Some(colon),
            attrs: Vec::new(),
        }];
        let dc = Decomposition { lbracket: colon, rbracket: colon, ids };
        d.decomp = Some(dc.clone());
        d.init_style = Some(InitStyle::C);
        let auto = self.type_from_declspec_at(&ds, colon);
        let ty = self.ast.types.rvalue_ref_to(auto);
        let v = self.declare_decomposition(&ds, ty, &d, &dc, false);
        // `e` was parsed (or substituted) long before this declaration,
        // whose full-expression it ends: its temporaries are this
        // initializer's cleanups, as for a shared subexpression.
        self.note_shared_temporaries(e);
        self.note_expansion_uses(e);
        self.act_on_init(v, Initializer::Expr(e));
        if self.ast.decls.get(v).has(DeclFlags::INVALID) {
            return None;
        }
        let prelude = vec![self.act_on_decl_stmt(&[v], colon)];
        // [stmt.expand]/5.3 (CWG3149): `v_i` is `u_i` for an lvalue
        // expansion-initializer, else `static_cast<decltype(u_i)&&>(u_i)`.
        let lvalue = self.ast.exprs.get(e).value_category() == ValueCategory::LValue;
        let elems = self
            .decomp_binding_list(v)
            .into_iter()
            .map(|b| {
                let u = self.build_decl_ref(b, colon);
                if lvalue || self.expr_has_error(u) {
                    return (None, u);
                }
                let t = self.decltype_of(u);
                let t = if self.ast.types.is_reference(t) {
                    t
                } else {
                    self.ast.types.rvalue_ref_to(t)
                };
                (None, self.act_on_cast(CastStyle::Static, t, u, colon, colon))
            })
            .collect();
        Some((v, prelude, elems))
    }
}

impl Substituter<'_, '_, '_> {
    /// Pattern expansion statement `s` (spec/sema-templates.md §5.5):
    /// expanded once every level outside its synthetic one is
    /// substituted, else rebuilt as a pattern.
    pub(crate) fn expansion_stmt(&mut self, s: StmtId) -> SubstResult<StmtId> {
        let ast = &self.sema.ast;
        let data = *ast.stmts.expansion_data(s);
        let depth = ast.stmts.expansion_depth(s);
        let form = ast.stmts.expansion_form(s);
        let loc = ast.stmts.get(s).loc();
        let levels = &self.args.levels;
        let expand = levels.len() >= usize::from(depth)
            && levels[..usize::from(depth)].iter().all(|l| l.mode == LevelMode::Substitute);
        self.sema.push_scope(ScopeKind::Control);
        let r = (|| {
            let init = data.init.map(|x| self.stmt(x));
            let exprs: Vec<ExprId> = self.sema.ast.lists.iter::<ExprId>(data.exprs).collect();
            let mark = self.sema.expansion_init_mark();
            let src = if let Some(e) = data.range_init {
                // [class.temporary]/9: a lifetime-extending context, as
                // when parsed.
                let collect = self.sema.begin_for_range_init(true);
                let r = self.expr(e);
                let _ = self.sema.end_for_range_init(collect);
                ExpansionSource::Expr(r?)
            } else {
                // A lifetime-extending context, as when parsed
                // (`materialize_discarded`).
                let collect = self.sema.begin_for_range_init(true);
                let l = self.expansion_list(&exprs);
                let _ = self.sema.end_for_range_init(collect);
                ExpansionSource::List(l?)
            };
            // Its uses are decided per expansion, as when parsed.
            self.sema.act_on_expansion_initializer_captures(mark);
            if expand {
                return self.expand_expansion(init, src, &data, depth, loc);
            }
            let (exprs, range_init) = match src {
                ExpansionSource::Expr(e) => (ListId::EMPTY, Some(e)),
                ExpansionSource::List(es) => (self.sema.ast.lists.push(&es), None),
            };
            let new_depth = self.args.retained_depth(depth);
            let _ = self.sema.push_expansion_level(loc);
            let r = (|| {
                let decl = self.expansion_decl(data.decl)?;
                self.sema.act_on_start_loop();
                let body = self.stmt(data.body);
                self.sema.act_on_end_loop();
                Ok((decl, body))
            })();
            self.sema.pop_expansion_level();
            let (decl, body) = r?;
            let data = ExpansionData { init, decl, exprs, range_init, body, ..data };
            Ok(self.sema.builder().cxx_expansion_stmt(data, form, false, new_depth, loc))
        })();
        self.sema.pop_scope();
        r
    }

    /// The elements of a pattern expansion-init-list, substituted: a
    /// braced element (and a braced pack expansion `{x}...`,
    /// [temp.variadic]/5.4) stays a dependent braced list, list-
    /// initializing its expansion's declaration.
    fn expansion_list(&mut self, exprs: &[ExprId]) -> SubstResult<Vec<ExprId>> {
        use crate::templ::pack::Expansion;
        let mut out = Vec::with_capacity(exprs.len());
        for &x in exprs {
            let ex = &self.sema.ast.exprs;
            if ex.get(x).kind() == ExprKind::InitListExpr {
                let l = self.init_list(x)?;
                out.push(self.sema.dependent_init_list(&l));
                continue;
            }
            let pat = crate::templ::pack::expr_pattern(self.sema, x)
                .filter(|&p| self.sema.ast.exprs.get(p).kind() == ExprKind::InitListExpr);
            match pat {
                Some(p) => {
                    let ellipsis = self.sema.ast.exprs.get(x).loc();
                    match self.expand_pattern_at(Some(ellipsis), |s| s.init_list(p))? {
                        Expansion::Elements(ls) => {
                            out.extend(ls.iter().map(|l| self.sema.dependent_init_list(l)));
                        }
                        Expansion::Unexpanded(l) => {
                            let e = self.sema.dependent_init_list(&l);
                            out.push(self.sema.act_pack_expansion(e, ellipsis));
                        }
                    }
                }
                None => out.extend(self.expand_exprs(&[x])?),
            }
        }
        Ok(out)
    }

    /// Whether the expansion element `e` is a braced element (a
    /// `void`-typed `InitListExpr` kept as written).
    pub(crate) fn is_braced_element(&self, e: ExprId) -> bool {
        let ex = &self.sema.ast.exprs;
        ex.get(e).kind() == ExprKind::InitListExpr && self.sema.ast.types.is_void(ex.get(e).ty())
    }

    /// The for-range-declaration of pattern `d`, instantiated.
    fn expansion_decl(&mut self, d: DeclId) -> SubstResult<DeclId> {
        if self.sema.ast.decls.decomp_bindings(d).is_some() {
            self.decomp_decl(d, DeclaratorContext::ForInit)
        } else {
            self.var_decl(d, DeclaratorContext::ForInit)
        }
    }

    /// A `CXXExpansionSelectExpr`: the element of the expansion being
    /// expanded at its depth, else (an enclosing level is retained) the
    /// select renumbered.
    pub(crate) fn expansion_select(&mut self, e: ExprId) -> SubstResult<ExprId> {
        let n = *self.sema.ast.exprs.get(e);
        let depth = u16::try_from(n.a()).map_err(|_| SubstFail)?;
        if let Some(x) = self.expansions.iter().rev().find(|x| x.depth == depth) {
            // The element was built before its declaration: its
            // temporaries and immediate invocations belong to the
            // declaration's full-expression (the temporaries then
            // extended to the declared variable, CWG3043).
            let elem = x.elem;
            self.sema.note_expansion_uses(elem);
            self.sema.note_shared_temporaries(elem);
            self.sema.note_deferred_invocations(elem);
            return Ok(elem);
        }
        let form = ExpansionForm::from_aux(u16::from(n.aux()));
        let d = self.args.retained_depth(depth);
        Ok(self.sema.builder().cxx_expansion_select(form, d, n.loc()))
    }

    /// Expands the statement with pattern `data` at `depth` over `src`
    /// ([stmt.expand]/3-5): classifies it, declares its prelude, and
    /// instantiates the pattern per element.
    fn expand_expansion(
        &mut self,
        init: Option<StmtId>,
        src: ExpansionSource,
        data: &ExpansionData,
        depth: u16,
        loc: Loc,
    ) -> SubstResult<StmtId> {
        // The implicit variables of this statement (and of the
        // statements nested in its elements) are known while it expands.
        let mark = self.sema.expansion_vars.len();
        let r = self.expand_expansion_in(init, src, data, depth, loc);
        self.sema.expansion_vars.truncate(mark);
        r
    }

    /// [`Substituter::expand_expansion`], its implicit variables noted.
    fn expand_expansion_in(
        &mut self,
        init: Option<StmtId>,
        src: ExpansionSource,
        data: &ExpansionData,
        depth: u16,
        loc: Loc,
    ) -> SubstResult<StmtId> {
        let colon = data.colon;
        let (form, prelude, elems) = match src {
            ExpansionSource::List(es) => (
                ExpansionForm::Enumerating,
                Vec::new(),
                es.into_iter().map(|e| (None, e)).collect(),
            ),
            ExpansionSource::Expr(e) => {
                if self.sema.is_err_expr(e) || self.sema.ast.exprs.get(e).is_type_dependent() {
                    return Err(SubstFail);
                }
                let constexpr =
                    self.sema.ast.decls.var(data.decl).is_some_and(|v| v.spec.is_constexpr());
                if self.sema.expansion_iterable(e, colon) {
                    let (p, el) =
                        self.sema.expansion_iterating(e, constexpr, colon).ok_or(SubstFail)?;
                    (ExpansionForm::Iterating, p, el)
                } else {
                    let (p, el) =
                        self.sema.expansion_destructuring(e, constexpr, colon).ok_or(SubstFail)?;
                    (ExpansionForm::Destructuring, p, el)
                }
            }
        };
        let mut out = Vec::with_capacity(elems.len());
        for (prefix, elem) in elems {
            out.push(self.expansion_element(prefix, elem, data, depth));
        }
        let prelude = self.sema.ast.lists.push(&prelude);
        let expansions = self.sema.ast.lists.push(&out);
        let x = ExpansionData { init, prelude, expansions, ..*data };
        Ok(self.sema.builder().cxx_expansion_stmt(x, form, true, depth, loc))
    }

    /// One element's statement `{ prefix decl = elem; body }`
    /// ([stmt.expand]/5): the pattern instantiated with the select at
    /// `depth` replaced by `elem`, in a block of its own.
    fn expansion_element(
        &mut self,
        prefix: Option<StmtId>,
        elem: ExprId,
        data: &ExpansionData,
        depth: u16,
    ) -> StmtId {
        let mut args = MultiLevelArgs::new();
        args.levels.extend_from_slice(&self.args.levels[..usize::from(depth)]);
        args.push(ArgListId::EMPTY, self.sema.ctx, LevelMode::Substitute);
        // A non-compound body (an extension) spans its own location.
        let st = &self.sema.ast.stmts;
        let lb = st.get(data.body).loc();
        let rb = if st.get(data.body).kind() == StmtKind::CompoundStmt {
            st.rbrace(data.body)
        } else {
            lb
        };
        let mut child = Substituter::new(&mut *self.sema, &args);
        child.locals.clone_from(&self.locals);
        child.pairs.clone_from(&self.pairs);
        child.entity = self.entity;
        child.pack_index = self.pack_index;
        child.lambda_bodies = self.lambda_bodies;
        child.written_locs = self.written_locs;
        child.final_subst = self.final_subst;
        child.expansions.clone_from(&self.expansions);
        child.expansions.push(ExpansionElem { depth, elem });
        child.sema.push_scope(ScopeKind::Block);
        let mut stmts: Vec<StmtId> = prefix.into_iter().collect();
        let outer = std::mem::replace(&mut child.sema.expansion_element_init, true);
        let decl = child.expansion_decl(data.decl);
        child.sema.expansion_element_init = outer;
        if let Ok(v) = decl {
            // [class.temporary]/8 (CWG3043): the element's temporaries
            // persist for the lifetime of the for-range-declaration.
            child.sema.extend_init_temporaries(v);
            let vloc = child.sema.ast.decls.get(v).loc();
            stmts.push(child.sema.act_on_decl_stmt(&[v], vloc));
            child.sema.act_on_start_loop();
            stmts.push(child.stmt(data.body));
            child.sema.act_on_end_loop();
        }
        child.sema.pop_scope();
        let failed = child.stmt_failed;
        let init_failures = child.init_failures;
        let failed_inits = std::mem::take(&mut child.failed_inits);
        let failed_init_use = child.failed_init_use;
        let failed_locals = std::mem::take(&mut child.failed_locals);
        drop(child);
        self.stmt_failed |= failed;
        self.init_failures += init_failures;
        self.failed_inits.extend(failed_inits);
        self.failed_init_use |= failed_init_use;
        self.failed_locals.extend(failed_locals);
        self.sema.act_on_compound(&stmts, lb, rb)
    }
}
