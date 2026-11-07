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
//!   type for a non-type parameter); function parameters are
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
//!   recovery expressions, coroutine and reflection operators, ...) are
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
use crate::templates::Requirement;

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
                    out.extend([D_NTTP, pos, u32::from(p.pack), self.opt_canon_word(node.ty())]);
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
                let targs = x.decl_ref_extra(e).and_then(|r| r.template_args);
                out.push(self.args_word(targs));
            }
            K::MemberExpr => {
                out.push(tag(k, u32::from(n.has(ExprFlags::ARROW))));
                self.decl_words(x.decl(e), out);
                let targs = x.decl_ref_extra(e).and_then(|r| r.template_args);
                out.push(self.args_word(targs));
                stack.push(Work::Opt(id(n.a())));
            }
            K::CXXThisExpr => out.push(tag(k, u32::from(x.this_is_implicit(e)))),
            K::PredefinedExpr | K::SourceLocExpr => out.push(tag(k, aux)),
            K::UnresolvedLookupExpr => out.extend([
                tag(k, aux & 1),
                n.a(),
                self.nns_word(self.templates.expr_nns.get(e).copied()),
                self.args_word(self.templates.expr_targs.get(e).copied()),
            ]),
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
                out.extend([
                    tag(k, arrow),
                    n.b(),
                    self.nns_word(self.templates.expr_nns.get(e).copied()),
                    self.args_word(self.templates.expr_targs.get(e).copied()),
                ]);
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
                out.extend([tag(k, aux & 1), self.canon_word(n.ty())]);
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
                    Some(c) => out.push(self.first(c).raw()),
                    None => out.push(NONE),
                }
                out.push(self.args_word(ArgListId::from_raw(n.b())));
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
            | K::CXXReflectExpr
            | K::CXXSpliceExpr
            | K::RecoveryExpr
            | K::CXXExpansionSelectExpr => out.extend([tag(k, 0) | UNIQUE, e.raw()]),
        }
        stack[mark..].reverse();
    }
}
