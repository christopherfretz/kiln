//! Written extents of expressions (spec/ast.md §21, WP-2327): the first
//! and the last token of an expression as written, exactly, or nothing.
//!
//! [`Ast::begin_loc`] and [`Ast::end_loc`] follow clang's `getBeginLoc` /
//! `getEndLoc`, which fall back to the node's own location when the tree
//! records no better one; a tooling client that moves an expression as
//! text cannot use a range one token short. [`WrittenExtents::get`] is
//! the same walk with every fallback that is not known to be exact
//! replaced by `None`, per kind (the table of §21.3).
//!
//! The extent is a pair of token locations, as the tree has them: either
//! may lie in a macro expansion. `kiln_lex::token_range` turns it into
//! bytes of one file, and a client that needs that guarantee reads
//! extents through `kiln_parse::ExprExtents`, which returns `None` for a
//! pair it does not map (§21.1).

use kiln_source::Loc;

use kiln_source::fx::FxHashSet;

use crate::Ast;
use crate::decl::DeclKind;
use crate::expr::{ExprFlags, ExprKind};
use crate::ids::{ExprId, LambdaId};
use crate::lambda::CaptureKind;
use crate::names::DeclNameKind;

/// Bound on the operand walks (each step moves to a child; a tree is
/// finite, the bound only guards a malformed one).
const MAX_STEPS: u32 = 1 << 20;

/// The written extents of a finished tree's expressions (spec/ast.md
/// §21): the tree, and the nodes it holds that stand for nothing written
/// although their kind is written elsewhere (the initializers of implicit
/// lambda captures), found once when the view is made.
pub struct WrittenExtents<'a> {
    ast: &'a Ast,
    unwritten: FxHashSet<ExprId>,
}

impl std::fmt::Debug for WrittenExtents<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WrittenExtents")
            .field("unwritten", &self.unwritten.len())
            .finish_non_exhaustive()
    }
}

impl std::ops::Deref for WrittenExtents<'_> {
    type Target = Ast;
    fn deref(&self) -> &Ast {
        self.ast
    }
}

impl<'a> WrittenExtents<'a> {
    /// The view of `ast`: one pass over its lambdas and its expressions.
    pub fn new(ast: &'a Ast) -> Self {
        let mut unwritten = FxHashSet::default();
        let mut stack = Vec::new();
        for i in 1..=ast.exprs.lambdas.len() {
            let l = ast.exprs.lambdas.get(LambdaId::new(u32::try_from(i).unwrap_or(u32::MAX)));
            let Some(inits) = l.inits else { continue };
            let inits = ast.lists.get(inits);
            if inits.len() != l.captures.len() {
                continue;
            }
            for (c, &i) in l.captures.iter().zip(inits) {
                // An implicit capture's member is initialized from the
                // entity, at the capture default, and a variable length
                // array bound's from the bound of the array's declaration:
                // nothing is written there.
                if c.implicit || c.kind == CaptureKind::VlaBound {
                    stack.push(ExprId::new(i));
                }
            }
        }
        // The bound of `new T` with `T` an array typedef, or of
        // `new int[]{...}` completed from the initializer: a literal the
        // compiler made at the type-id (a written bound follows `[`).
        for i in 1..=ast.exprs.len() {
            let e = ExprId::new(u32::try_from(i).unwrap_or(u32::MAX));
            let n = ast.exprs.get(e);
            // The function an unwritten call (a coroutine's handle, a
            // range-for's `begin`) calls by name: no name is written.
            if n.kind() == ExprKind::CallExpr && ast.exprs.end_loc(e) == Some(n.loc()) {
                let mut f = ast.exprs.callee(e);
                while ast.exprs.get(f).kind() == ExprKind::ImplicitCastExpr {
                    match ast.exprs.first_child(f, &ast.lists) {
                        Some(x) => f = x,
                        None => break,
                    }
                }
                if ast.exprs.get(f).kind() == ExprKind::DeclRefExpr
                    && ast.decls.get(ast.exprs.decl(f)).name().kind()
                        != DeclNameKind::LiteralOperator
                {
                    stack.push(ast.exprs.callee(e));
                }
                continue;
            }
            if n.kind() != ExprKind::CXXNewExpr {
                continue;
            }
            let x = ast.exprs.new_extra(e);
            if let Some(s) = x.array_size
                && ast.exprs.get(s).kind() == ExprKind::IntegerLiteral
                && ast.exprs.get(s).loc() == x.type_loc
            {
                stack.push(s);
            }
        }
        // The whole initializer: a copy's construction and the reference
        // to the entity in it.
        let mut kids = Vec::new();
        while let Some(e) = stack.pop() {
            if unwritten.insert(e) {
                kids.clear();
                ast.exprs.children_into(e, &ast.lists, &mut kids);
                stack.extend_from_slice(&kids);
            }
        }
        WrittenExtents { ast, unwritten }
    }

    /// The locations of the first and the last token of `e` as written
    /// (spec/ast.md §21), or `None` when the tree does not record them
    /// exactly. A `Some` is exact for every kind.
    ///
    /// - An implicit node around a written one (an implicit conversion,
    ///   a materialized or bound temporary, cleanups, a constant-expression
    ///   wrapper) has the extent of the written node inside it; a node with
    ///   nothing written (a default argument or default member initializer
    ///   at its use, a value-initialization the compiler supplied, the
    ///   initializer of an implicit capture) is `None`.
    /// - A rewritten comparison (`a != b` calling `operator==`, a
    ///   synthesized three-way comparison) has the written extent on the
    ///   `CXXRewrittenBinaryOperator`; the nodes built inside it are those
    ///   of an operator, whose callee is `None`.
    /// - A member access through an implicit `this` (`f(x)` in a member
    ///   function) begins at the member's name; the implicit `this` is
    ///   `None`.
    ///
    /// Both locations are token starts in kiln's location space; they may
    /// be macro locations, and the pair is then a file range only by the
    /// rule of `kiln_lex::token_range`.
    pub fn get(&self, e: ExprId) -> Option<(Loc, Loc)> {
        let first = self.written_first(e)?;
        let last = self.written_last(e)?;
        (first.is_valid() && last.is_valid()).then_some((first, last))
    }

    /// The written node `e` stands for: `e` itself, or for an implicit
    /// wrapper the written node inside it; `None` when nothing is written.
    fn written_node(&self, mut e: ExprId) -> Option<ExprId> {
        for _ in 0..MAX_STEPS {
            if self.unwritten.contains(&e) {
                return None;
            }
            let n = self.exprs.get(e);
            match n.kind() {
                ExprKind::ImplicitCastExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::ConstantExpr
                | ExprKind::CXXStdInitializerListExpr => {
                    e = self.exprs.first_child(e, &self.lists)?;
                }
                // An initial or final suspend point, the `co_await` of a
                // `co_yield`.
                ExprKind::CoawaitExpr if self.exprs.coawait_is_implicit(e) => return None,
                ExprKind::CXXDefaultArgExpr
                | ExprKind::CXXDefaultInitExpr
                | ExprKind::ImplicitValueInitExpr
                | ExprKind::NoInitExpr
                | ExprKind::ArrayInitIndexExpr
                | ExprKind::ArrayInitLoopExpr
                | ExprKind::OpaqueValueExpr => return None,
                ExprKind::CXXThisExpr if self.exprs.this_is_implicit(e) => return None,
                // The fold a constrained pack's type-constraint makes
                // (`C... Ts`, [temp.param]/4): no parentheses written.
                ExprKind::CXXFoldExpr if self.exprs.end_loc(e) == Some(n.loc()) => return None,
                // A variable the compiler declared (a coroutine's promise,
                // a range-`for`'s range): no name of it is written.
                ExprKind::DeclRefExpr if self.names_implicit_var(e) => return None,
                // A list a designator opens (`.a.y = 3`'s list of `a`)
                // begins inside the designator, whose parts the tree does
                // not keep.
                ExprKind::InitListExpr if n.has(ExprFlags::DESIGNATED) => return None,
                // A construction with no written initializer of its own: a
                // copy, move or converting construction from its one
                // operand is that operand; a default construction (`S s;`)
                // has nothing written.
                ExprKind::CXXConstructExpr if self.exprs.end_loc(e).is_none() => {
                    match self.exprs.children(e, &self.lists)[..] {
                        [a] => e = a,
                        _ => return None,
                    }
                }
                ExprKind::CallExpr | ExprKind::CXXMemberCallExpr if self.call_is_unwritten(e) => {
                    match self.unwritten_call_object(e) {
                        // An implicit conversion function call: the object.
                        Some(obj) => e = obj,
                        // A literal with a ud-suffix is written.
                        None if self.call_is_udl(e) => return Some(e),
                        None => return None,
                    }
                }
                _ => return Some(e),
            }
        }
        None
    }

    /// The first token of `e`, exactly, or `None`.
    fn written_first(&self, e: ExprId) -> Option<Loc> {
        let mut e = self.written_node(e)?;
        for _ in 0..MAX_STEPS {
            let n = self.exprs.get(e);
            if n.kind() == ExprKind::CallExpr && self.call_is_unwritten(e) {
                // A user-defined literal (`written_node` keeps no other
                // unwritten call): the literal's first token.
                return Some(n.loc());
            }
            if self.init_list_is_elided(e) {
                // The braces of a sub-aggregate were elided: the list
                // begins where its first written initializer does.
                return self.elided_list_first(e);
            }
            let next = match n.kind() {
                // The first operand is written first.
                ExprKind::BinaryOperator
                | ExprKind::CompoundAssignOperator
                | ExprKind::ConditionalOperator
                | ExprKind::BinaryConditionalOperator
                | ExprKind::ArraySubscriptExpr
                | ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXPseudoDestructorExpr
                | ExprKind::ExtVectorElementExpr
                | ExprKind::PackExpansionExpr => self.exprs.first_child(e, &self.lists)?,
                // `pack...[index]`: the pack (not among the children the
                // tree walks list).
                ExprKind::PackIndexingExpr => ExprId::try_new(n.a())?,
                ExprKind::UnaryOperator if self.exprs.unary_op(e).is_postfix() => {
                    self.exprs.first_child(e, &self.lists)?
                }
                ExprKind::MemberExpr | ExprKind::CXXDependentScopeMemberExpr => {
                    let base = self.exprs.first_child(e, &self.lists)?;
                    if self.written_node(base).is_some() {
                        base
                    } else if !self.is_implicit_this(base) {
                        // A base the compiler made (an awaiter, a
                        // structured binding's object): nothing written.
                        return None;
                    } else {
                        // An implicit `this`: the qualifier's recorded
                        // begin (`B::f`), else the member's name, unless a
                        // qualifier whose begin is not recorded comes first.
                        if let Some(&b) = self.templates.expr_begin.get(e) {
                            return Some(b);
                        }
                        let qualified =
                            self.member_access_name(e).is_some_and(|m| m.qualifier.is_some());
                        return (!qualified).then(|| n.loc());
                    }
                }
                ExprKind::CXXRewrittenBinaryOperator => {
                    let (_, ops) = self.exprs.rewritten_written(e);
                    self.lists.at::<ExprId>(ops, 0)?
                }
                ExprKind::CXXOperatorCallExpr => {
                    use crate::names::OverloadedOperator as O;
                    let op = self.exprs.operator_call_op(e);
                    let args = self.exprs.call_args(e);
                    let first = self.lists.at::<ExprId>(args, 0);
                    let prefix = self.lists.get(args).len() == 1
                        && !matches!(op, O::Call | O::Subscript | O::Arrow);
                    match first {
                        Some(a) if !prefix => a,
                        _ => return Some(n.loc()),
                    }
                }
                ExprKind::DeclRefExpr
                | ExprKind::UnresolvedLookupExpr
                | ExprKind::DependentScopeDeclRefExpr
                | ExprKind::ConceptSpecializationExpr => {
                    if let Some(&b) = self.templates.expr_begin.get(e) {
                        return Some(b);
                    }
                    // A qualifier whose begin the tree does not keep.
                    let qualified = if n.kind() == ExprKind::DeclRefExpr {
                        self.exprs.decl_ref_extra(e).is_some_and(|x| x.qualifier.is_some())
                    } else {
                        self.templates.expr_nns.get(e).is_some()
                    };
                    return (!qualified).then(|| n.loc());
                }
                // The `.` or `[` of the first designator, or a GNU
                // `name:`'s name; the `(` of a parenthesized aggregate
                // initialization, or the cast that makes one: recorded by
                // sema, as clang's `getBeginLoc`.
                ExprKind::DesignatedInitExpr | ExprKind::CXXParenListInitExpr => {
                    return self.templates.expr_begin.get(e).copied();
                }
                k if leaf_begins_at_loc(k) => return Some(n.loc()),
                _ => return None,
            };
            e = self.written_node(next)?;
        }
        None
    }

    /// The last token of `e`, exactly, or `None`.
    fn written_last(&self, e: ExprId) -> Option<Loc> {
        let mut e = self.written_node(e)?;
        for _ in 0..MAX_STEPS {
            let n = self.exprs.get(e);
            if n.kind() == ExprKind::CallExpr && self.call_is_unwritten(e) {
                // A user-defined literal: its last token, the last piece
                // of a concatenated string.
                let arg = self.lists.at::<ExprId>(self.exprs.call_args(e), 0);
                return Some(
                    arg.and_then(|a| self.written_node(a))
                        .filter(|&a| self.exprs.get(a).kind() == ExprKind::StringLiteral)
                        .and_then(|a| self.exprs.end_loc(a))
                        .unwrap_or(n.loc()),
                );
            }
            if self.init_list_is_elided(e) {
                return self.elided_list_last(e);
            }
            let next = match n.kind() {
                // A stored closing token.
                ExprKind::ParenExpr | ExprKind::InitListExpr => return self.exprs.end_loc(e),
                k if keeps_closing_token(k) && self.exprs.end_loc(e).is_some() => {
                    return self.exprs.end_loc(e);
                }
                // The last operand is written last.
                ExprKind::BinaryOperator
                | ExprKind::CompoundAssignOperator
                | ExprKind::ConditionalOperator
                | ExprKind::BinaryConditionalOperator
                | ExprKind::CStyleCastExpr
                | ExprKind::CXXDeleteExpr => self.exprs.children(e, &self.lists).last().copied()?,
                ExprKind::UnaryOperator => {
                    if self.exprs.unary_op(e).is_postfix() {
                        return Some(n.loc());
                    }
                    self.exprs.first_child(e, &self.lists)?
                }
                // `sizeof e` / `sizeof(e)`: the operand (its parentheses
                // are a `ParenExpr`).
                ExprKind::UnaryExprOrTypeTraitExpr => self.exprs.first_child(e, &self.lists)?,
                // The pattern's `...`; an ext-vector accessor.
                ExprKind::PackExpansionExpr | ExprKind::ExtVectorElementExpr => {
                    return Some(n.loc());
                }
                // `(T){...}`: its list.
                ExprKind::CompoundLiteralExpr => self.exprs.compound_literal_init(e),
                // `.a = x`: its initializer.
                ExprKind::DesignatedInitExpr => self.exprs.designated_init(e),
                ExprKind::CoawaitExpr | ExprKind::CoyieldExpr | ExprKind::DependentCoawaitExpr => {
                    self.exprs.coawait_operand(e, &self.lists)
                }
                ExprKind::CXXThrowExpr => match self.exprs.throw_operand(e) {
                    Some(o) => o,
                    None => return Some(n.loc()),
                },
                ExprKind::CXXRewrittenBinaryOperator => {
                    let (_, ops) = self.exprs.rewritten_written(e);
                    self.lists.at::<ExprId>(ops, 1)?
                }
                ExprKind::CXXOperatorCallExpr => {
                    use crate::names::OverloadedOperator as O;
                    if let Some(l) = self.exprs.end_loc(e) {
                        return Some(l);
                    }
                    let op = self.exprs.operator_call_op(e);
                    let args = self.lists.get(self.exprs.call_args(e));
                    let postfix = matches!(op, O::PlusPlus | O::MinusMinus) && args.len() == 2;
                    if matches!(op, O::Arrow) || postfix {
                        return Some(n.loc());
                    }
                    if matches!(op, O::Call | O::Subscript) {
                        return None;
                    }
                    ExprId::new(*args.last()?)
                }
                ExprKind::DeclRefExpr
                | ExprKind::MemberExpr
                | ExprKind::UnresolvedLookupExpr
                | ExprKind::DependentScopeDeclRefExpr
                | ExprKind::CXXDependentScopeMemberExpr => return self.name_last(e),
                // A concatenation stores its last piece.
                ExprKind::StringLiteral => return Some(self.exprs.end_loc(e).unwrap_or(n.loc())),
                k if leaf_is_one_token(k) => return Some(n.loc()),
                _ => return None,
            };
            e = self.written_node(next)?;
        }
        None
    }
}

impl WrittenExtents<'_> {
    /// The last token of a name (a reference or a member access): its
    /// stored `>`, or the name itself when it is a plain identifier.
    fn name_last(&self, e: ExprId) -> Option<Loc> {
        let n = self.exprs.get(e);
        match n.kind() {
            ExprKind::DeclRefExpr | ExprKind::MemberExpr => {
                if self.exprs.end_loc(e).is_some() {
                    return self.exprs.end_loc(e);
                }
                let d = self.exprs.decl(e);
                // A variable template specialization is named with
                // its template arguments, whose `>` is not kept.
                let plain = self.decls.get(d).name().kind() == DeclNameKind::Identifier
                    && self.decls.get(d).kind() != DeclKind::VarTemplateSpecialization;
                let targs = self.member_access_name(e).is_some_and(|m| m.template_args.is_some())
                    || self.exprs.decl_ref_extra(e).is_some_and(|x| x.template_args.is_some());
                (plain && !targs).then(|| n.loc())
            }
            // A dependent or unresolved name: a plain identifier is
            // its own token; template arguments end at a stored `>`.
            ExprKind::UnresolvedLookupExpr
            | ExprKind::DependentScopeDeclRefExpr
            | ExprKind::CXXDependentScopeMemberExpr => {
                if self.exprs.end_loc(e).is_some() {
                    return self.exprs.end_loc(e);
                }
                let name = match n.kind() {
                    ExprKind::UnresolvedLookupExpr => self.unresolved_name(e),
                    ExprKind::DependentScopeDeclRefExpr => self.dependent_scope_ref_name(e),
                    _ => self.dependent_member_name(e),
                };
                let plain = name.kind() == DeclNameKind::Identifier
                    && self.templates.expr_targs.get(e).is_none();
                plain.then(|| n.loc())
            }
            _ => None,
        }
    }

    /// A call with no written parentheses: its stored closing token is
    /// its own location (sema builds such calls for a user-defined
    /// literal, an implicit conversion function, a structured binding's
    /// `get`, a coroutine's promise and awaiter calls). A written call's
    /// `)` always follows its first token.
    fn call_is_unwritten(&self, e: ExprId) -> bool {
        self.exprs.end_loc(e) == Some(self.exprs.get(e).loc())
    }

    /// For an unwritten call of a conversion function (`e.operator T()`
    /// built by an implicit conversion), the object expression.
    fn unwritten_call_object(&self, e: ExprId) -> Option<ExprId> {
        let n = self.exprs.get(e);
        if n.kind() != ExprKind::CXXMemberCallExpr
            || !self.lists.get(self.exprs.call_args(e)).is_empty()
        {
            return None;
        }
        let callee = self.exprs.callee(e);
        let m = self.exprs.get(callee);
        if m.kind() != ExprKind::MemberExpr || m.loc() != n.loc() {
            return None;
        }
        let d = self.exprs.decl(callee);
        (self.decls.get(d).name().kind() == DeclNameKind::Conversion)
            .then(|| self.exprs.first_child(callee, &self.lists))
            .flatten()
    }

    /// An initializer list whose braces were elided ([dcl.init.aggr]/16):
    /// sema gives it one location for both braces, where its first
    /// initializer is written. Written braces are two tokens.
    fn init_list_is_elided(&self, e: ExprId) -> bool {
        let n = self.exprs.get(e);
        n.kind() == ExprKind::InitListExpr && self.exprs.end_loc(e) == Some(n.loc())
    }

    /// Whether the `DeclRefExpr` `e` names an implicitly declared
    /// variable.
    fn names_implicit_var(&self, e: ExprId) -> bool {
        let d = self.decls.get(self.exprs.decl(e));
        d.kind() == DeclKind::Var && d.has(crate::decl::DeclFlags::IMPLICIT)
    }

    /// Whether `e` is an implicit `this` (under implicit wrappers).
    fn is_implicit_this(&self, mut e: ExprId) -> bool {
        for _ in 0..MAX_STEPS {
            match self.exprs.get(e).kind() {
                ExprKind::ImplicitCastExpr | ExprKind::MaterializeTemporaryExpr => {
                    match self.exprs.first_child(e, &self.lists) {
                        Some(x) => e = x,
                        None => return false,
                    }
                }
                ExprKind::CXXThisExpr => return self.exprs.this_is_implicit(e),
                _ => return false,
            }
        }
        false
    }

    /// The first token of an elided list: that of its first written
    /// initializer (`None` when nothing in it is written).
    fn elided_list_first(&self, e: ExprId) -> Option<Loc> {
        let inits = self.lists.get(self.exprs.init_list_inits(e));
        let first =
            inits.iter().map(|&i| ExprId::new(i)).find(|&i| self.written_node(i).is_some())?;
        self.written_first(first)
    }

    /// The last token of an elided list: that of its last written
    /// initializer (`None` when nothing in it is written).
    fn elided_list_last(&self, e: ExprId) -> Option<Loc> {
        let inits = self.lists.get(self.exprs.init_list_inits(e));
        let last = inits
            .iter()
            .rev()
            .map(|&i| ExprId::new(i))
            .find(|&i| self.written_node(i).is_some())?;
        self.written_last(last)
    }

    /// Whether the unwritten call `e` calls a literal operator.
    fn call_is_udl(&self, e: ExprId) -> bool {
        if self.exprs.get(e).kind() != ExprKind::CallExpr {
            return false;
        }
        let mut c = self.exprs.callee(e);
        for _ in 0..8 {
            let n = self.exprs.get(c);
            match n.kind() {
                ExprKind::ImplicitCastExpr => match self.exprs.first_child(c, &self.lists) {
                    Some(x) => c = x,
                    None => return false,
                },
                ExprKind::DeclRefExpr => {
                    let d = self.exprs.decl(c);
                    return self.decls.get(d).name().kind() == DeclNameKind::LiteralOperator;
                }
                ExprKind::UnresolvedLookupExpr => {
                    return self.unresolved_name(c).kind() == DeclNameKind::LiteralOperator;
                }
                _ => return false,
            }
        }
        false
    }
}

/// Kinds that end at a closing token sema or the parser stores (`)`,
/// `]`, `}`, `>`), when it is stored.
fn keeps_closing_token(k: ExprKind) -> bool {
    use ExprKind as K;
    matches!(
        k,
        K::CallExpr
            | K::CXXMemberCallExpr
            | K::ArraySubscriptExpr
            | K::CXXTemporaryObjectExpr
            | K::CXXConstructExpr
            | K::CXXFunctionalCastExpr
            | K::CXXStaticCastExpr
            | K::CXXConstCastExpr
            | K::CXXReinterpretCastExpr
            | K::CXXDynamicCastExpr
            | K::BuiltinBitCastExpr
            | K::CXXScalarValueInitExpr
            | K::CXXUnresolvedConstructExpr
            | K::UnaryExprOrTypeTraitExpr
            | K::CXXNoexceptExpr
            | K::CXXTypeidExpr
            | K::TypeTraitExpr
            | K::VAArgExpr
            | K::LambdaExpr
            | K::CXXFoldExpr
            | K::StmtExpr
            | K::OffsetOfExpr
            | K::ShuffleVectorExpr
            | K::ConvertVectorExpr
            | K::AtomicExpr
            | K::ChooseExpr
            | K::CXXNewExpr
            | K::SizeOfPackExpr
            | K::RequiresExpr
            | K::CXXParenListInitExpr
            | K::AddrLabelExpr
            | K::SourceLocExpr
            | K::GenericSelectionExpr
            | K::ArrayTypeTraitExpr
            | K::ExpressionTraitExpr
            | K::ParenListExpr
            | K::PackIndexingExpr
            | K::ConceptSpecializationExpr
            | K::CompoundLiteralExpr
            | K::CXXPseudoDestructorExpr
    )
}

/// Kinds whose own location is their first token and that begin with no
/// operand.
fn leaf_begins_at_loc(k: ExprKind) -> bool {
    use ExprKind as K;
    leaf_is_one_token(k)
        || matches!(
            k,
            K::StringLiteral
                | K::UnaryOperator
                | K::ParenExpr
                | K::CStyleCastExpr
                | K::CXXFunctionalCastExpr
                | K::CXXStaticCastExpr
                | K::CXXConstCastExpr
                | K::CXXReinterpretCastExpr
                | K::CXXDynamicCastExpr
                | K::BuiltinBitCastExpr
                | K::CXXTemporaryObjectExpr
                | K::CXXConstructExpr
                | K::CXXScalarValueInitExpr
                | K::CXXUnresolvedConstructExpr
                | K::InitListExpr
                | K::UnaryExprOrTypeTraitExpr
                | K::CXXNoexceptExpr
                | K::CXXTypeidExpr
                | K::TypeTraitExpr
                | K::VAArgExpr
                | K::LambdaExpr
                | K::CXXFoldExpr
                | K::StmtExpr
                | K::OffsetOfExpr
                | K::ShuffleVectorExpr
                | K::ConvertVectorExpr
                | K::AtomicExpr
                | K::ChooseExpr
                | K::CXXNewExpr
                | K::CXXDeleteExpr
                | K::CXXThrowExpr
                | K::SizeOfPackExpr
                | K::RequiresExpr
                | K::AddrLabelExpr
                | K::SourceLocExpr
                | K::GenericSelectionExpr
                | K::ArrayTypeTraitExpr
                | K::ExpressionTraitExpr
                | K::ParenListExpr
                | K::CompoundLiteralExpr
                | K::CoawaitExpr
                | K::CoyieldExpr
                | K::DependentCoawaitExpr
        )
}

/// Kinds that are one token, at their own location.
fn leaf_is_one_token(k: ExprKind) -> bool {
    use ExprKind as K;
    matches!(
        k,
        K::IntegerLiteral
            | K::FloatingLiteral
            | K::CharacterLiteral
            | K::CXXBoolLiteralExpr
            | K::CXXNullPtrLiteralExpr
            | K::GNUNullExpr
            | K::CXXThisExpr
            | K::ImaginaryLiteral
            | K::PredefinedExpr
            // The value of a non-type template argument, at the name of
            // the parameter it replaces (always one identifier).
            | K::SubstNonTypeTemplateParmExpr
    )
}
