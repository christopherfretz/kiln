//! The canonical expression profiler (spec/types.md §7.2): the word
//! sequence [`super::TypeContext::profile`] interns.
//!
//! Modelled on clang's `StmtProfiler` with `Canonical = true`. Two
//! expressions are *equivalent* ([temp.over.link]/5) iff their profiles are
//! equal; "functionally equivalent but not equivalent" expressions are
//! ill-formed, no diagnostic required, and get different profiles.
//!
//! The encoding is prefix-free: every node starts with a tag word (its kind
//! plus a per-kind detail such as the operator, never 0), followed by a
//! fixed per-kind layout; lists carry a length prefix and an absent
//! optional operand is the word [`NONE`]. Rules:
//! - Types are their canonical `QualType`; nested-name-specifiers,
//!   template-argument lists and template names are their canonical
//!   (hash-consed) ids, which already embed the profiles of the
//!   expressions inside them.
//! - Template parameters are `(depth, index, pack)` (plus the canonical
//!   type for a non-type parameter, a placeholder's type-constraint
//!   ignored); function parameters are
//!   `(index, pack, canonical type)` like the Itanium mangling; every
//!   other declaration is its first declaration.
//! - Unresolved names are `(qualifier, name, template args)`; the
//!   declarations lookup found are not part of the profile.
//! - `ImplicitCastExpr`, `ConstantExpr`, `MaterializeTemporaryExpr`,
//!   `CXXBindTemporaryExpr` and `ExprWithCleanups` are transparent;
//!   `SubstNonTypeTemplateParmExpr` profiles as its replacement;
//!   parentheses are kept.
//! - Literals are their value and canonical type (not their spelling).
//! - Kinds with no equivalence rule (lambdas, statement-expressions,
//!   recovery expressions, coroutine operators, ...) are
//!   *unique*: `[tag | UNIQUE, expr id]`, equal only to themselves. The
//!   match below is exhaustive so a new `ExprKind` must pick a rule.
//!
//! The walk uses an explicit work stack (no recursion), so deeply nested
//! expressions cannot overflow the native stack.

use super::kinds::TypeFlags;
use crate::ast::Ast;
use crate::decl::{DeclKind, ParamData};
use crate::expr::{ExprDeps, ExprFlags, ExprKind as K, TypeidOperand};
use crate::ids::{ArgListId, DeclId, ExprId, ListId, NnsId, ProfileId, QualType, RawId};
use crate::names::DeclNameView;
use crate::templates::Requirement;
use crate::types::{Nns, TagKeyword, TypeView};
use kiln_source::symbol::Symbol;

/// An absent optional operand.
const NONE: u32 = 0;
/// Marks a tag word of a unique (never-equivalent) expression.
const UNIQUE: u32 = 0xFF00_0000;

// Declaration encodings (after a node's tag word).
const D_TEMPLATE_PARM: u32 = 1;
const D_NTTP: u32 = 2;
const D_PARM: u32 = 3;
const D_DECL: u32 = 4;
/// A postcondition's result name: one per assertion, so it compares by
/// kind alone ([dcl.contract.func]/5, spec/contracts.md §5).
const D_RESULT_NAME: u32 = 5;

// Requirement encodings inside a `RequiresExpr`.
const R_SIMPLE: u32 = 1;
const R_TYPE: u32 = 2;
const R_COMPOUND: u32 = 3;
const R_NESTED: u32 = 4;

/// One pending item of the profile walk.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Work {
    /// Profile this expression.
    Expr(ExprId),
    /// Profile this optional expression ([`NONE`] when absent).
    Opt(Option<ExprId>),
    /// Emit this word.
    Word(u32),
}

/// Reusable buffers for [`Ast::profile_expr`] (kept on the
/// `TypeContext` so profiling allocates only while the buffers grow).
#[derive(Debug, Default)]
pub(crate) struct ProfileScratch {
    words: Vec<u32>,
    stack: Vec<Work>,
}

/// Reusable buffers for [`Ast::exprs_equivalent_in`]: the two profiles
/// and the walk stack, so a caller that compares often allocates only
/// while they grow.
#[derive(Debug, Default)]
pub struct EquivScratch {
    a: Vec<u32>,
    b: Vec<u32>,
    stack: Vec<Work>,
}

/// The tag word of a node of kind `k` with per-kind detail `extra`
/// (< 2^16).
fn tag(k: K, extra: u32) -> u32 {
    debug_assert!(extra < 1 << 16, "profile tag detail {extra} of {k:?} too wide");
    (k as u32 + 1) | (extra << 8)
}

/// The type flags a profile records for an expression with dependence
/// `deps` (spec/types.md §7.2).
pub fn deps_type_flags(deps: ExprDeps) -> TypeFlags {
    let mut f = TypeFlags::NONE;
    if deps.intersects(ExprDeps::TYPE.with(ExprDeps::VALUE)) {
        f |= TypeFlags::DEPENDENT_INSTANTIATION;
    }
    if deps.contains(ExprDeps::INSTANTIATION) {
        f |= TypeFlags::INSTANTIATION_DEPENDENT;
    }
    if deps.contains(ExprDeps::UNEXPANDED_PACK) {
        f |= TypeFlags::UNEXPANDED_PACK;
    }
    f
}

impl Ast {
    /// The canonical profile of expression `e` (spec/types.md §7.2):
    /// equal for equivalent expressions ([temp.over.link]/5). The profile
    /// records `e` as its first expression (when new) and the type flags
    /// of `e`'s dependence.
    pub fn profile_expr(&mut self, e: ExprId) -> ProfileId {
        let flags = deps_type_flags(self.exprs.get(e).deps());
        self.profile_expr_with(e, flags)
    }

    /// [`Ast::profile_expr`] recording `flags` instead of the flags of
    /// `e`'s dependence.
    pub fn profile_expr_with(&mut self, e: ExprId, flags: TypeFlags) -> ProfileId {
        let mut s = std::mem::take(&mut self.types.profile_scratch);
        s.words.clear();
        self.profile_walk(e, &mut s.words, &mut s.stack);
        let p = self.types.profile(&s.words, e, flags);
        self.types.profile_scratch = s;
        p
    }

    /// The identifier a member access on an object of dependent type
    /// keeps for the first level of its qualifier `nns` (clang's
    /// identifier nested-name-specifier: that name cannot be looked up in
    /// the object's class yet, [basic.lookup.qual.general]/2): the name
    /// of a template type parameter, a typedef or alias, or a
    /// non-template class or enumeration, written with no prefix and no
    /// `template`. `t.T::f()` mangles `sr1TE1f` and is not equivalent to
    /// `t.P::f()` for `T` = `P` (spec/abi.md "Mangling", WP-1029).
    pub fn spelled_member_qualifier(&self, nns: NnsId) -> Option<Symbol> {
        let Nns::Type { prefix: None, ty, template_kw: false } = self.types.nns_view(nns) else {
            return None;
        };
        self.spelled_type_name(ty)
    }

    /// The name `ty` is written with, when it is one identifier: a
    /// template type parameter, a typedef or alias, a using-declaration,
    /// or a non-template class or enumeration.
    fn spelled_type_name(&self, mut ty: QualType) -> Option<Symbol> {
        let d = loop {
            match self.types.view(ty.ty()) {
                TypeView::TemplateTypeParm(d) | TypeView::Record(d) | TypeView::Enum(d) => break d,
                TypeView::Typedef { decl, .. } => break decl,
                TypeView::Using { shadow, .. } => break shadow,
                TypeView::Elaborated { keyword: TagKeyword::None, nns: None, named } => ty = named,
                _ => return None,
            }
        };
        let node = self.decls.get(d);
        if matches!(
            node.kind(),
            DeclKind::ClassTemplateSpecialization | DeclKind::ClassTemplatePartialSpecialization
        ) {
            return None;
        }
        match self.names.view(node.name()) {
            DeclNameView::Identifier(s) => Some(s),
            _ => None,
        }
    }

    /// The words of qualifier `nns` as spelled, for a member access on an
    /// object of dependent type: clang keeps every level by name there,
    /// so `t.A::Self::f()`, `t.P::Self::f()` and `t.ns2::P::f()` are not
    /// equivalent to `t.P::f()` even when they name the same class
    /// (spec/abi.md "Mangling", WP-1029). A level that is not one
    /// identifier (a template-id, a splice) keeps its canonical word.
    fn spelled_nns_words(&self, nns: NnsId, out: &mut Vec<u32>) {
        let mut chain = Vec::new();
        let mut cur = Some(nns);
        while let Some(n) = cur {
            chain.push(n);
            cur = match self.types.nns_view(n) {
                Nns::Namespace { prefix, .. }
                | Nns::NamespaceAlias { prefix, .. }
                | Nns::Type { prefix, .. }
                | Nns::Identifier { prefix, .. } => prefix,
                Nns::Global | Nns::Splice(_) => None,
            };
        }
        out.push(chain.len() as u32);
        let name = |d: DeclId| match self.names.view(self.decls.get(d).name()) {
            DeclNameView::Identifier(s) => s.index(),
            _ => d.raw(),
        };
        for &n in chain.iter().rev() {
            match self.types.nns_view(n) {
                Nns::Global => out.extend([1, 0]),
                Nns::Namespace { ns, .. } => out.extend([2, name(ns)]),
                Nns::NamespaceAlias { alias, .. } => out.extend([3, name(alias)]),
                Nns::Type { ty, template_kw: false, .. }
                    if let Some(s) = self.spelled_type_name(ty) =>
                {
                    out.extend([4, s.index()]);
                }
                Nns::Identifier { name, .. } => out.extend([5, name.index()]),
                Nns::Type { .. } | Nns::Splice(_) => {
                    out.extend([6, self.types.canonical_nns(n).raw()]);
                }
            }
        }
    }

    /// The profile words of `e`, without interning them.
    pub fn expr_profile_words(&self, e: ExprId) -> Vec<u32> {
        let mut words = Vec::new();
        self.profile_walk(e, &mut words, &mut Vec::new());
        words
    }

    /// True if `a` and `b` are equivalent ([temp.over.link]/5): their
    /// profiles are equal.
    pub fn exprs_equivalent(&self, a: ExprId, b: ExprId) -> bool {
        self.exprs_equivalent_in(a, b, &mut EquivScratch::default())
    }

    /// [`Ast::exprs_equivalent`] profiling into the buffers of `s`.
    pub fn exprs_equivalent_in(&self, a: ExprId, b: ExprId, s: &mut EquivScratch) -> bool {
        if a == b {
            return true;
        }
        s.a.clear();
        s.b.clear();
        self.profile_walk(a, &mut s.a, &mut s.stack);
        self.profile_walk(b, &mut s.b, &mut s.stack);
        s.a == s.b
    }

    fn profile_walk(&self, root: ExprId, out: &mut Vec<u32>, stack: &mut Vec<Work>) {
        stack.clear();
        stack.push(Work::Expr(root));
        while let Some(w) = stack.pop() {
            match w {
                Work::Word(x) => out.push(x),
                Work::Opt(None) => out.push(NONE),
                Work::Opt(Some(e)) | Work::Expr(e) => self.profile_node(e, out, stack),
            }
        }
    }

    fn canon_word(&self, q: QualType) -> u32 {
        self.types.canonical(q).raw()
    }

    /// The words of the type a dependent `^^type-id` reflects: a
    /// top-level alias (a typedef-name or an alias template
    /// specialization, under elaboration, `using` and parentheses only) is the
    /// alias, not its type ([meta.reflection], as
    /// [`Ast::reflection_type`] keeps it once substituted), so `^^AT<T>`
    /// and `^^A<T>` differ; any other type is its canonical type.
    fn reflected_type_words(&self, q: QualType, out: &mut Vec<u32>) {
        use super::TypeView;
        let mut cur = q;
        while cur.quals_bits() == 0 {
            match self.types.view(cur.ty()) {
                TypeView::Elaborated { named, .. } => cur = named,
                TypeView::Using { underlying, .. } => cur = underlying,
                TypeView::Paren(inner) => cur = inner,
                TypeView::Typedef { decl, .. } => {
                    out.extend([1, self.first(decl).raw()]);
                    return;
                }
                TypeView::TemplateSpecialization { name, args, alias: true, .. } => {
                    out.extend([
                        2,
                        self.types.canonical_template_name(name).raw(),
                        self.types.canonical_args(args).raw(),
                    ]);
                    return;
                }
                _ => break,
            }
        }
        out.extend([0, self.canon_word(q)]);
    }

    /// The template of a class template name to deduce (`view(r, f)`):
    /// its canonical type is `Dependent` whatever the template, so the
    /// name tells `drop_view(...)` from `take_view(...)` (clang profiles
    /// the `DeducedTemplateSpecializationType`'s template name).
    fn ctad_word(&self, q: QualType) -> u32 {
        match self.types.view(q.ty()) {
            TypeView::DeducedTemplateSpecialization { name, deduced }
                if deduced.is_none_or(|d| d == QualType::DEPENDENT) =>
            {
                self.types.canonical_template_name(name).raw().wrapping_add(1)
            }
            _ => NONE,
        }
    }

    fn opt_canon_word(&self, q: Option<QualType>) -> u32 {
        q.map_or(NONE, |q| self.canon_word(q))
    }

    fn args_word(&self, l: Option<ArgListId>) -> u32 {
        l.map_or(NONE, |l| self.types.canonical_args(l).raw())
    }

    fn nns_word(&self, n: Option<NnsId>) -> u32 {
        n.map_or(NONE, |n| self.types.canonical_nns(n).raw())
    }

    /// The words identifying declaration `d` (canonical rules above).
    fn decl_words(&self, d: DeclId, out: &mut Vec<u32>) {
        let node = self.decls.get(d);
        match node.kind() {
            DeclKind::TemplateTypeParm
            | DeclKind::NonTypeTemplateParm
            | DeclKind::TemplateTemplateParm => {
                let Some(p) = self.template_parm(d) else {
                    out.extend([D_DECL, self.first(d).raw()]);
                    return;
                };
                let pos = (u32::from(p.depth) << 16) | u32::from(p.index);
                if node.kind() == DeclKind::NonTypeTemplateParm {
                    out.extend([D_NTTP, pos, u32::from(p.pack)]);
                    self.nttp_type_words(node.ty(), out);
                } else {
                    out.extend([D_TEMPLATE_PARM, pos, u32::from(p.pack)]);
                }
            }
            DeclKind::ParmVar => {
                // Position, pack-ness and function-scope depth (clang's
                // `VisitDecl` for a `ParmVarDecl`): `a` of
                // `void (*)(T a, decltype(a))` is not the outer `a`.
                let (index, pack, depth) = self.decls.param(d).map_or((0, false, 0), |p| {
                    (u32::from(p.index), p.flags & ParamData::PACK != 0, p.scope_depth())
                });
                // A requires-expression's parameters are a scope of their own.
                let in_requires = node
                    .ctx()
                    .is_some_and(|c| self.decls.get(c).kind() == DeclKind::RequiresExprBody);
                let pos = index
                    | (u32::from(pack) << 16)
                    | (u32::from(in_requires) << 17)
                    | (depth << 18);
                out.extend([D_PARM, pos, self.opt_canon_word(node.ty())]);
            }
            DeclKind::ContractResultName => out.push(D_RESULT_NAME),
            _ => out.extend([D_DECL, self.first(d).raw()]),
        }
    }

    /// The type of a non-type template parameter, ignoring the
    /// type-constraint of a top-level placeholder ([temp.over.link]/6.2,
    /// clang's `getUnconstrainedType`): `D` of `template <C auto D>` and of
    /// `template <auto D>` profile alike, so `decltype(D)` in the
    /// immediately-declared constraints `C<decltype(D)>` and
    /// `C2<decltype(D)>` of two templates names one type and their atoms
    /// can subsume ([temp.param]/5, [temp.constr.order]). Two words:
    /// `[0, canonical type]` or `[1, keyword | pack | qualifiers]`.
    fn nttp_type_words(&self, q: Option<QualType>, out: &mut Vec<u32>) {
        if let Some(q) = q {
            let c = self.types.canonical(q);
            if let TypeView::Auto { keyword, deduced: None, pack, .. } = self.types.view(c.ty()) {
                let w = keyword as u32 | (u32::from(pack) << 4) | (u32::from(c.quals_bits()) << 8);
                out.extend([1, w]);
                return;
            }
        }
        out.extend([0, self.opt_canon_word(q)]);
    }

    /// The written template arguments of a reference to `d`. A
    /// non-dependent list naming a specialization is implied by the
    /// specialization itself and is left out (clang's canonical
    /// `VisitDeclRefExpr` profiles no written arguments): its
    /// non-dependent argument expressions have no profile, so keeping them
    /// would make `mag<1>` in a redeclared requires-clause differ.
    fn ref_args_word(&self, d: DeclId, l: Option<ArgListId>) -> u32 {
        match l {
            Some(l)
                if !self.types.arg_list_flags(l).intersects(TypeFlags::DEPENDENT_INSTANTIATION)
                    && !matches!(
                        self.decls.get(d).kind(),
                        DeclKind::FunctionTemplate | DeclKind::VarTemplate
                    ) =>
            {
                NONE
            }
            _ => self.args_word(l),
        }
    }

    /// Pushes a length-prefixed expression list.
    fn push_list(&self, l: ListId, stack: &mut Vec<Work>) {
        stack.push(Work::Word(self.lists.len(l) as u32));
        stack.extend(self.lists.iter::<ExprId>(l).map(Work::Expr));
    }

    /// Emits the header words of `e` to `out` and pushes its remaining
    /// items (in order) onto `stack`.
    #[allow(clippy::too_many_lines, reason = "one arm per expression kind")]
    fn profile_node(&self, e: ExprId, out: &mut Vec<u32>, stack: &mut Vec<Work>) {
        let x = &self.exprs;
        let n = *x.get(e);
        let k = n.kind();
        let aux = u32::from(n.aux());
        let id = |w: u32| ExprId::from_raw(w);
        let mark = stack.len();
        match k {
            // ----- transparent nodes
            K::ImplicitCastExpr
            | K::ConstantExpr
            | K::MaterializeTemporaryExpr
            | K::CXXBindTemporaryExpr
            | K::ExprWithCleanups => stack.push(Work::Opt(id(n.a()))),
            K::SubstNonTypeTemplateParmExpr => stack.push(Work::Opt(id(n.b()))),
            K::OpaqueValueExpr => match x.opaque_source(e) {
                Some(s) => stack.push(Work::Expr(s)),
                None => out.push(tag(k, 0)),
            },

            // ----- literals: value and type
            K::IntegerLiteral => {
                out.extend([tag(k, 0), self.canon_word(n.ty())]);
                match x.int_value(e) {
                    Some(v) => out.extend([0, v as u32, (v >> 32) as u32]),
                    None => out.extend([1, x.literal_const(e).map_or(NONE, RawId::raw)]),
                }
            }
            K::FloatingLiteral => out.extend([
                tag(k, 0),
                self.canon_word(n.ty()),
                x.literal_const(e).map_or(NONE, RawId::raw),
            ]),
            K::CharacterLiteral => out.extend([tag(k, aux), self.canon_word(n.ty()), n.a()]),
            // Interned by (encoding, code units); the type carries the length.
            K::StringLiteral => out.extend([tag(k, 0), self.canon_word(n.ty()), n.a()]),
            K::CXXBoolLiteralExpr => out.push(tag(k, u32::from(x.bool_value(e)))),
            // A label is local to its function: its declaration identifies it.
            K::AddrLabelExpr => out.extend([tag(k, 0), n.a()]),
            K::CXXNullPtrLiteralExpr
            | K::GNUNullExpr
            | K::ImplicitValueInitExpr
            | K::CXXScalarValueInitExpr => {
                out.extend([tag(k, 0), self.canon_word(n.ty())]);
            }

            // ----- names
            K::DeclRefExpr => {
                out.push(tag(k, 0));
                self.decl_words(x.decl(e), out);
                out.push(
                    self.ref_args_word(
                        x.decl(e),
                        x.decl_ref_extra(e).and_then(|r| r.template_args),
                    ),
                );
            }
            K::MemberExpr => {
                out.push(tag(k, u32::from(n.has(ExprFlags::ARROW))));
                self.decl_words(x.decl(e), out);
                out.push(
                    self.ref_args_word(
                        x.decl(e),
                        x.decl_ref_extra(e).and_then(|r| r.template_args),
                    ),
                );
                stack.push(Work::Opt(id(n.a())));
            }
            K::CXXThisExpr => out.push(tag(k, u32::from(x.this_is_implicit(e)))),
            K::PredefinedExpr | K::SourceLocExpr => out.push(tag(k, aux)),
            K::UnresolvedLookupExpr => {
                out.extend([tag(k, aux & 1), n.a()]);
                // `V<args>` for a variable template template parameter:
                // the parameter by position (P2841R7 [temp.over.link]/5).
                if let Some(d) = self.lists.iter::<DeclId>(ListId::new(n.b())).next()
                    && self.decls.get(d).kind() == DeclKind::TemplateTemplateParm
                {
                    out.pop();
                    self.decl_words(d, out);
                }
                out.extend([
                    self.nns_word(self.templates.expr_nns.get(e).copied()),
                    self.args_word(self.templates.expr_targs.get(e).copied()),
                ]);
            }
            K::DependentScopeDeclRefExpr => out.extend([
                tag(k, 0),
                self.nns_word(NnsId::from_raw(n.a())),
                n.b(),
                self.args_word(self.templates.expr_targs.get(e).copied()),
            ]),
            K::CXXDependentScopeMemberExpr => {
                let base = id(n.a());
                // The arrow only matters with an explicit object.
                let arrow = u32::from(base.is_some() && aux & 1 != 0);
                let nns = self.templates.expr_nns.get(e).copied();
                // Behind an explicit object of dependent type the qualifier
                // is kept as spelled (`spelled_nns_words`).
                let spelled = base
                    .filter(|&b| {
                        self.exprs.get(b).is_type_dependent()
                            && !(self.exprs.get(b).kind() == K::CXXThisExpr
                                && self.exprs.this_is_implicit(b))
                    })
                    .and(nns);
                out.extend([
                    tag(k, arrow | u32::from(spelled.is_some()) << 1),
                    n.b(),
                    if spelled.is_some() { NONE } else { self.nns_word(nns) },
                    self.args_word(self.templates.expr_targs.get(e).copied()),
                ]);
                if let Some(q) = spelled {
                    self.spelled_nns_words(q, out);
                }
                stack.push(Work::Opt(base));
            }
            K::SizeOfPackExpr => {
                out.push(tag(k, 0));
                match DeclId::from_raw(n.a()) {
                    Some(d) => self.decl_words(d, out),
                    None => out.push(NONE),
                }
            }

            // ----- operators
            K::UnaryOperator | K::ParenExpr | K::PackExpansionExpr | K::CXXNoexceptExpr => {
                let detail = if k == K::UnaryOperator { aux } else { 0 };
                out.push(tag(k, detail));
                stack.push(Work::Opt(id(n.a())));
            }
            K::BinaryOperator | K::ArraySubscriptExpr | K::PackIndexingExpr => {
                let detail = if k == K::BinaryOperator { aux } else { 0 };
                out.push(tag(k, detail));
                stack.extend([Work::Opt(id(n.a())), Work::Opt(id(n.b()))]);
            }
            K::CompoundAssignOperator => {
                out.push(tag(k, aux));
                stack.extend([Work::Opt(id(n.a())), Work::Expr(x.compound_assign(e).rhs)]);
            }
            K::ConditionalOperator => {
                let [t, f] = x.cond_arms(e);
                out.push(tag(k, 0));
                stack.extend([Work::Opt(id(n.a())), Work::Expr(t), Work::Expr(f)]);
            }
            K::CXXFoldExpr => {
                out.push(tag(k, aux));
                stack.extend([Work::Opt(id(n.a())), Work::Opt(id(n.b()))]);
            }
            K::CallExpr | K::CXXMemberCallExpr | K::CXXOperatorCallExpr => {
                let detail = if k == K::CXXOperatorCallExpr { aux } else { 0 };
                out.push(tag(k, detail));
                stack.push(Work::Opt(id(n.a())));
                self.push_list(ListId::new(n.b()), stack);
            }
            // Profiled in its syntactic form, like clang.
            K::PseudoObjectExpr => stack.push(Work::Expr(x.pseudo_syntactic(e, &self.lists))),
            K::CXXRewrittenBinaryOperator => {
                let (op, written) = x.rewritten_written(e);
                out.push(tag(k, op as u32));
                self.push_list(written, stack);
            }
            K::BinaryConditionalOperator | K::ParenListExpr | K::InitListExpr => {
                out.push(tag(k, 0));
                self.push_list(ListId::new(n.a()), stack);
            }
            K::DesignatedInitExpr => {
                // clang `VisitDesignatedInitExpr`: the designators, then
                // the initializer.
                let ds = x.designators(e);
                out.push(tag(k, ds.len() as u32));
                for d in ds {
                    match *d {
                        crate::expr::Designator::Field { name, .. } => {
                            stack.extend([Work::Word(1), Work::Word(name.index())]);
                        }
                        crate::expr::Designator::Index { index, .. } => {
                            stack.extend([Work::Word(2), Work::Expr(index)]);
                        }
                        crate::expr::Designator::Range { first, last, .. } => {
                            stack.extend([Work::Word(3), Work::Expr(first), Work::Expr(last)]);
                        }
                    }
                }
                stack.push(Work::Opt(id(n.a())));
            }

            // ----- casts and construction: the written type matters
            K::CStyleCastExpr
            | K::CXXFunctionalCastExpr
            | K::CXXStaticCastExpr
            | K::CXXConstCastExpr
            | K::CXXReinterpretCastExpr
            | K::CXXDynamicCastExpr
            | K::BuiltinBitCastExpr => {
                out.extend([
                    tag(k, aux | ((n.value_category() as u32) << 8)),
                    self.canon_word(n.ty()),
                ]);
                stack.push(Work::Opt(id(n.a())));
            }
            K::CXXUnresolvedConstructExpr => {
                out.extend([tag(k, aux & 1), self.canon_word(n.ty()), self.ctad_word(n.ty())]);
                self.push_list(ListId::new(n.a()), stack);
            }
            K::CXXParenListInitExpr => {
                out.extend([tag(k, 0), self.canon_word(n.ty())]);
                self.push_list(ListId::new(n.a()), stack);
            }
            // ImplicitValueInit / CXXScalarValueInit: with the null literals above.
            K::CXXConstructExpr | K::CXXTemporaryObjectExpr => {
                out.extend([tag(k, 0), self.canon_word(n.ty())]);
                self.decl_words(x.constructor(e), out);
                self.push_list(ListId::new(n.b()), stack);
            }
            K::CompoundLiteralExpr => {
                out.extend([tag(k, 0), self.canon_word(n.ty())]);
                stack.push(Work::Opt(id(n.a())));
            }
            K::CXXDefaultArgExpr => {
                out.push(tag(k, 0));
                self.decl_words(x.default_arg_param(e), out);
            }
            K::CXXDefaultInitExpr => {
                out.push(tag(k, 0));
                self.decl_words(x.default_init_field(e), out);
            }
            K::ArrayInitLoopExpr | K::DesignatedInitUpdateExpr => {
                out.push(tag(k, 0));
                stack.extend([Work::Opt(id(n.a())), Work::Opt(id(n.b()))]);
            }
            K::ArrayInitIndexExpr | K::NoInitExpr => out.push(tag(k, 0)),

            // ----- memory, exceptions, RTTI
            K::CXXNewExpr => {
                let ne = x.new_extra(e);
                let detail = u32::from(n.has(ExprFlags::GLOBAL))
                    | (u32::from(n.has(ExprFlags::ARRAY_FORM)) << 1);
                out.extend([tag(k, detail), self.canon_word(ne.allocated_ty)]);
                stack.extend([Work::Opt(ne.array_size), Work::Opt(ne.init)]);
                match ne.placement {
                    Some(p) => self.push_list(p, stack),
                    None => stack.push(Work::Word(0)),
                }
            }
            K::CXXDeleteExpr => {
                let detail = u32::from(n.has(ExprFlags::GLOBAL))
                    | (u32::from(n.has(ExprFlags::ARRAY_FORM)) << 1);
                out.push(tag(k, detail));
                stack.push(Work::Opt(id(n.a())));
            }
            K::CXXThrowExpr => {
                out.push(tag(k, 0));
                stack.push(Work::Opt(x.throw_operand(e)));
            }
            // `p->~T` (WP-121): the object, the arrow and the destroyed type.
            K::CXXPseudoDestructorExpr => {
                let (base, arrow, destroyed) = x.pseudo_destructor(e);
                out.extend([tag(k, u32::from(arrow)), self.canon_word(destroyed)]);
                stack.push(Work::Expr(base));
            }
            K::CXXTypeidExpr => match x.typeid_operand(e) {
                TypeidOperand::Type(t) => out.extend([tag(k, 1), self.canon_word(t)]),
                TypeidOperand::Expr(o) => {
                    out.push(tag(k, 0));
                    stack.push(Work::Expr(o));
                }
            },

            // ----- traits
            K::UnaryExprOrTypeTraitExpr => {
                if n.has(ExprFlags::ARG_IS_TYPE) {
                    out.extend([tag(k, aux | 0x100), self.opt_canon_word(x.trait_type_arg(e))]);
                } else {
                    out.push(tag(k, aux));
                    stack.push(Work::Opt(id(n.a())));
                }
            }
            K::TypeTraitExpr => {
                let l = x.type_trait_args(e);
                out.extend([tag(k, aux), self.lists.len(l) as u32]);
                for t in self.lists.iter::<QualType>(l) {
                    out.push(self.canon_word(t));
                }
            }
            K::ArrayTypeTraitExpr => {
                out.extend([tag(k, aux), self.canon_word(x.array_trait_type(e))]);
                stack.push(Work::Opt(x.array_trait(e).dim));
            }
            K::ExpressionTraitExpr => {
                out.push(tag(k, aux));
                stack.push(Work::Expr(x.expression_trait_operand(e)));
            }
            K::RequiresExpr => match self.requires_of(e) {
                Some(r) => {
                    out.extend([tag(k, 0), self.lists.len(r.params) as u32]);
                    for p in self.lists.iter::<DeclId>(r.params) {
                        out.push(self.opt_canon_word(self.decls.get(p).ty()));
                    }
                    let reqs = self.templates.requirements(r);
                    out.push(reqs.len() as u32);
                    for req in reqs {
                        match *req {
                            Requirement::Simple(s) => {
                                stack.extend([Work::Word(R_SIMPLE), Work::Expr(s)]);
                            }
                            Requirement::Type(t, _) => {
                                stack.extend([Work::Word(R_TYPE), Work::Word(self.canon_word(t))]);
                            }
                            Requirement::Compound { expr, noexcept, constraint } => {
                                let w = R_COMPOUND | (u32::from(noexcept.is_some()) << 8);
                                stack.extend([
                                    Work::Word(w),
                                    Work::Expr(expr),
                                    Work::Opt(constraint),
                                ]);
                            }
                            Requirement::Nested(c) => {
                                stack.extend([Work::Word(R_NESTED), Work::Expr(c)]);
                            }
                        }
                    }
                }
                None => out.extend([tag(k, 0) | UNIQUE, e.raw()]),
            },
            K::ConceptSpecializationExpr => {
                out.push(tag(k, 0));
                match DeclId::from_raw(n.a()) {
                    // A concept template parameter by position (P2841R7
                    // [temp.over.link]/5).
                    Some(c) if self.decls.get(c).kind() == DeclKind::TemplateTemplateParm => {
                        self.decl_words(c, out);
                    }
                    Some(c) => out.push(self.first(c).raw()),
                    None => out.push(NONE),
                }
                out.push(self.args_word(ArgListId::from_raw(n.b())));
            }

            // ----- a reflection: its value (interned, so equal values
            // are one id) once known. A dependent one profiles its
            // operand structurally, so the reflection operands of
            // `C<^^T*>` in two templates are equivalent
            // ([temp.over.link]/5): a dependent qualified name (`^^T::x`)
            // as that name, a type as [`Self::reflected_type_words`]
            // (its top-level alias kept, [meta.reflection]), a template
            // by its canonical name, a declaration by the declaration
            // rules above.
            K::CXXReflectExpr => {
                use crate::reflect::Reflectee;
                if let Some(v) = x.reflect_value(e) {
                    out.extend([tag(k, 2), v.raw()]);
                } else if let Some(name) = x.reflect_dependent_name(e) {
                    out.push(tag(k, 3));
                    stack.push(Work::Expr(name));
                } else {
                    match self.reflect_operand(e) {
                        Reflectee::Type(q) => {
                            out.push(tag(k, 1));
                            self.reflected_type_words(q, out);
                        }
                        Reflectee::Template(t) => out.extend([
                            tag(k, 4),
                            self.types.canonical_template_name(t).raw(),
                            self.nns_word(self.templates.expr_nns.get(e).copied()),
                        ]),
                        Reflectee::Decl(d) => {
                            out.push(tag(k, 5));
                            self.decl_words(d, out);
                            out.push(
                                self.ref_args_word(d, self.templates.expr_targs.get(e).copied()),
                            );
                        }
                        _ => out.extend([tag(k, 0) | UNIQUE, e.raw()]),
                    }
                }
            }

            // ----- a dependent splice `[: r :]`, `template [: r :] < args >`
            // or a member splice `x.[: r :]` / `p->[: r :]`: its operand
            // (and object expression) structurally, and the template
            // arguments as written, so the same splice written in two
            // templates is equivalent ([temp.over.link]/5, P2996
            // [temp.type]: dependent splice-specifiers are equivalent
            // when their constant-expressions are). The detail tells the
            // three forms apart (WP-884).
            K::CXXSpliceExpr => {
                let base = x.splice_base(e);
                let form = base.map_or(0, |(_, arrow)| 1 + u32::from(arrow));
                out.extend([tag(k, form), self.args_word(x.splice_template_args(e))]);
                stack.push(Work::Expr(x.splice_operand(e)));
                stack.push(Work::Opt(base.map(|(b, _)| b)));
            }

            // ----- no equivalence rule: equal only to itself
            K::UserDefinedLiteral
            | K::ImaginaryLiteral
            | K::EmbedExpr
            | K::UnresolvedMemberExpr
            | K::SubstNonTypeTemplateParmPackExpr
            | K::FunctionParmPackExpr
            | K::CXXInheritedCtorInitExpr
            | K::CXXStdInitializerListExpr
            | K::OffsetOfExpr
            | K::LambdaExpr
            | K::CoawaitExpr
            | K::DependentCoawaitExpr
            | K::CoyieldExpr
            | K::StmtExpr
            | K::ChooseExpr
            | K::VAArgExpr
            | K::AtomicExpr
            | K::ShuffleVectorExpr
            | K::ConvertVectorExpr
            | K::ExtVectorElementExpr
            | K::RecoveryExpr
            | K::CXXExpansionSelectExpr => out.extend([tag(k, 0) | UNIQUE, e.raw()]),
        }
        stack[mark..].reverse();
    }
}
