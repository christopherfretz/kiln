//! Expressions ([expr], spec/sema.md §5): names, literals, operators,
//! calls, member access, `sizeof`/`alignof`, casts, and the end of a
//! full-expression (the odr-use pass, [basic.def.odr]).
//!
//! Each `act_on_*` builds the typed node clang builds, with the implicit
//! conversions of [`crate::conv`] spelled out as `ImplicitCastExpr`s.

use kiln_ast::attr::AttrKind;
use kiln_ast::consts::{IntBits, Scalar};
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{
    BinaryOp, CastKind, CharKind, ExprFlags, ExprKind, NonOdrUse, ObjectKind, Radix, StringKind,
    TraitKind, UnaryOp, ValueCategory,
};
use kiln_ast::ids::RawId;
use kiln_ast::names::DeclName;
use kiln_ast::strings::StringEncoding;
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{Ast, AttrId, ConstValueId, DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_lex::Encoding;
use kiln_lex::literal::{CharLiteral, CodeUnits, FloatLiteral, IntegerLiteral, StringLiteral};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::constfold::int_info;
use crate::conv::{self, ArithConvKind, ConvCtx, ConvError};
use crate::init::InitList;
use crate::literal;
use crate::lookup::LookupKind;
use crate::overload::oper::{OperForm, OperResult};
use crate::type_diff::DiffArgs as _;
use crate::vector_ops::VecParts;

mod ref_cast;

/// The spelling of an explicit type conversion ([expr.cast],
/// [expr.type.conv], [expr.static.cast], ...): selects the node kind and
/// the conversions tried.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CastStyle {
    /// `const_cast<T>(e)`.
    Const,
    /// `static_cast<T>(e)`.
    Static,
    /// `reinterpret_cast<T>(e)`.
    Reinterpret,
    /// `dynamic_cast<T>(e)` (not supported in M2).
    Dynamic,
    /// `(T)e`.
    CStyle,
    /// `T(e)`.
    Functional,
}

impl CastStyle {
    /// clang's `%select` index in the bad-cast diagnostics.
    fn select(self) -> &'static str {
        match self {
            CastStyle::Const => "0",
            CastStyle::Static => "1",
            CastStyle::Reinterpret => "2",
            CastStyle::Dynamic => "3",
            CastStyle::CStyle => "4",
            CastStyle::Functional => "5",
        }
    }

    pub(crate) fn kind(self) -> ExprKind {
        match self {
            CastStyle::Const => ExprKind::CXXConstCastExpr,
            CastStyle::Static => ExprKind::CXXStaticCastExpr,
            CastStyle::Reinterpret => ExprKind::CXXReinterpretCastExpr,
            CastStyle::Dynamic => ExprKind::CXXDynamicCastExpr,
            CastStyle::CStyle => ExprKind::CStyleCastExpr,
            CastStyle::Functional => ExprKind::CXXFunctionalCastExpr,
        }
    }

    /// Whether the `static_cast` rules apply ([expr.cast]/4).
    fn allows_static(self) -> bool {
        matches!(self, CastStyle::Static | CastStyle::CStyle | CastStyle::Functional)
    }

    /// Whether the `reinterpret_cast` rules apply.
    fn allows_reinterpret(self) -> bool {
        matches!(self, CastStyle::Reinterpret | CastStyle::CStyle | CastStyle::Functional)
    }

    /// Whether qualifiers may be cast away.
    fn allows_const(self) -> bool {
        matches!(self, CastStyle::Const | CastStyle::CStyle | CastStyle::Functional)
    }

    /// Whether a conversion between a derived class and its base is
    /// valid even if the base is inaccessible ([expr.cast]/4).
    fn ignores_base_access(self) -> bool {
        matches!(self, CastStyle::CStyle | CastStyle::Functional)
    }
}

impl Sema<'_, '_> {
    // -- small helpers ---------------------------------------------------------

    pub(crate) fn ety(&self, e: ExprId) -> QualType {
        self.ast.exprs.get(e).ty()
    }

    pub(crate) fn eloc(&self, e: ExprId) -> Loc {
        self.ast.exprs.get(e).loc()
    }

    pub(crate) fn is_err_expr(&self, e: ExprId) -> bool {
        let n = self.ast.exprs.get(e);
        n.kind() == ExprKind::RecoveryExpr || self.is_error_type(n.ty())
    }

    fn is_bitfield(&self, e: ExprId) -> bool {
        self.ast.exprs.get(e).object_kind() == ObjectKind::BitField
    }

    /// The type a pointer or reference type points to, keeping sugar
    /// (clang's `getPointeeType` on the written type).
    pub(crate) fn pointee_sugared(&self, q: QualType) -> Option<QualType> {
        let mut cur = q;
        for _ in 0..256 {
            match self.ast.types.view(cur.ty()) {
                TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                    return Some(p);
                }
                _ => {
                    let next = self.ast.types.desugar_once(cur);
                    if next == cur {
                        break;
                    }
                    cur = next;
                }
            }
        }
        self.ast.types.pointee(q)
    }

    /// The type of a name or member of declared type `t`: references
    /// designate their referent ([expr.type]/1).
    pub(crate) fn non_reference(&self, t: QualType) -> QualType {
        if self.ast.types.is_reference(t) {
            self.pointee_sugared(t).unwrap_or(QualType::ERROR)
        } else {
            t
        }
    }

    /// The type of `e` as an operand reads it: `T` for an `_Atomic(T)`
    /// (clang `DefaultLvalueConversion`).
    pub(crate) fn value_ty(&self, e: ExprId) -> QualType {
        let t = self.ety(e);
        self.ast.types.as_atomic(t).unwrap_or(t)
    }

    pub(crate) fn is_integral_or_enum(&self, t: QualType) -> bool {
        self.ast.types.is_integral(t) || self.ast.types.as_enum(t).is_some()
    }

    /// A scalar type ([basic.types.general]/9): arithmetic, enumeration,
    /// pointer, pointer-to-member, `std::nullptr_t` and `std::meta::info`
    /// (P2996 [basic.fundamental]), plus GNU complex (clang `isScalarType`).
    pub(crate) fn is_scalar(&self, t: QualType) -> bool {
        self.ast.types.is_arithmetic(t)
            || self.is_meta_info(t)
            || self.ast.types.as_complex(t).is_some()
            || self.ast.types.as_enum(t).is_some()
            || self.ast.types.is_pointer(t)
            || self.is_member_pointer(t)
            || self.is_nullptr_type(t)
    }

    /// An arithmetic, unscoped enumeration or GNU complex type: the
    /// operand types of the arithmetic operators (clang's
    /// `isArithmeticType`, which includes complex types).
    pub(crate) fn is_arith_operand(&self, t: QualType) -> bool {
        self.ast.types.is_arithmetic(t)
            || conv::is_unscoped_enum(self, t)
            || self.ast.types.as_complex(t).is_some()
    }

    pub(crate) fn is_nullptr_type(&self, t: QualType) -> bool {
        self.ast.types.builtin(t) == Some(BuiltinType::NullPtr)
    }

    /// `size_t` ([support.types.layout]): the type of `sizeof`/`alignof`
    /// and of the allocation functions' size parameter. Under
    /// `-fclang-compat=22` it carries clang 22's `__size_t` sugar
    /// (`'__size_t':'unsigned long'` in the AST dump, `'__size_t' (aka
    /// 'unsigned long')` in diagnostics); the canonical type is the
    /// target's.
    pub(crate) fn size_type(&self) -> QualType {
        if self.clang_compat >= 22 {
            self.size_t_sugar
        } else {
            QualType::unqual(self.target.size_t)
        }
    }

    /// The signed integer type corresponding to `size_t`: the type of a
    /// `z` literal ([lex.icon] table 8), `'__signed_size_t':'long'` under
    /// `-fclang-compat=22` (see [`Sema::size_type`]).
    pub(crate) fn signed_size_type(&self) -> QualType {
        if self.clang_compat >= 22 {
            self.signed_size_t_sugar
        } else {
            self.ast.types.canonical(self.signed_size_t_sugar)
        }
    }

    /// `ptrdiff_t` ([expr.add]/5): the type of a pointer difference,
    /// `'__ptrdiff_t':'long'` under `-fclang-compat=22` (see
    /// [`Sema::size_type`]).
    pub(crate) fn ptrdiff_type(&self) -> QualType {
        if self.clang_compat >= 22 {
            self.ptrdiff_t_sugar
        } else {
            QualType::unqual(self.target.ptrdiff_t)
        }
    }

    /// A `RecoveryExpr` of type `ty` over `kids`.
    pub(crate) fn recover(&mut self, ty: QualType, kids: &[ExprId], loc: Loc) -> ExprId {
        // clang `CreateRecoveryExpr`: a reference type is the referenced
        // type, an lvalue (`T&`, a function) or an xvalue (`T&&`).
        if ty != QualType::ERROR && self.ast.types.is_reference(ty) {
            let lref = matches!(self.ast.types.canonical_view(ty), TypeView::LValueRef(_));
            let r = self.non_reference(ty);
            let vk = if lref || self.ast.types.is_function(r) {
                ValueCategory::LValue
            } else {
                ValueCategory::XValue
            };
            let e = self.builder().recovery(Some(r), vk, kids, loc);
            self.note_spec_recovery(e);
            return e;
        }
        let e = self.builder().recovery(Some(ty), ValueCategory::PRValue, kids, loc);
        self.note_spec_recovery(e);
        e
    }

    /// Records that a failed call is one clang recovers with a
    /// `RecoveryExpr` (`FinishOverloadedCallExpr`,
    /// `BuildCallToMemberFunction`): never under SFINAE, where clang
    /// builds none (spec/sema.md §1.2).
    /// `ty`: the recovery's type, `None` when it is type-dependent.
    pub(crate) fn note_call_recovery(&mut self, ty: Option<QualType>) {
        if !self.inst.stack.is_sfinae() {
            self.call_recovery = Some((self.inst.stack.len(), ty));
        }
    }

    /// A `RecoveryExpr` of error type over `kids`.
    pub(crate) fn recover_err(&mut self, kids: &[ExprId], loc: Loc) -> ExprId {
        self.recover(QualType::ERROR, kids, loc)
    }

    /// An erroneous expression at `loc` over `kids` (a `RecoveryExpr` of
    /// the error type), for the parser's recovery after a syntax error
    /// that was already reported.
    pub fn act_on_error_expr(&mut self, kids: &[ExprId], loc: Loc) -> ExprId {
        self.recover_err(kids, loc)
    }

    /// True if `e` is erroneous (a `RecoveryExpr` or of the error type).
    pub fn is_error_expr(&self, e: ExprId) -> bool {
        self.is_err_expr(e)
    }

    /// clang's `Expr::containsErrors`: `e` or an expression under it is
    /// erroneous (a `RecoveryExpr` or of the error type). The walk is
    /// bounded: it serves parser recovery only (WP-391).
    pub fn contains_errors(&self, e: ExprId) -> bool {
        let mut stack = vec![e];
        let mut budget = 1usize << 16;
        while let Some(x) = stack.pop() {
            if budget == 0 {
                break;
            }
            budget -= 1;
            if self.is_err_expr(x) {
                return true;
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        false
    }

    /// True if an integer type `t` is narrower than `int`: arithmetic on
    /// it is done in `int` and cannot overflow (clang's
    /// `UnaryOperator::canOverflow` rule).
    fn narrower_than_int(&self, t: QualType) -> bool {
        // A `_BitInt` is never promoted, so its arithmetic can overflow
        // at any width (clang: `nsw` on `-`/`++` of `_BitInt(7)`).
        if !self.is_integral_or_enum(t)
            || matches!(self.ast.types.canonical_view(t), TypeView::BitInt { .. })
        {
            return false;
        }
        let int_w = conv::int_width(self, QualType::INT).unwrap_or(32);
        conv::int_width(self, t).is_some_and(|w| w < int_w)
    }

    // -- constants ---------------------------------------------------------------

    /// Emits a conversion error at `loc`.
    pub(crate) fn emit_conv_error(&self, err: ConvError, loc: Loc) {
        let loc = err.at.unwrap_or(loc);
        let d = err.args.into_iter().fold(self.diag(err.id, loc), Diag::arg);
        self.emit(err.notes.into_iter().fold(d, Diag::with_note));
    }

    /// Interns the integer (or `bool`) constant `v` of type `ty`.
    ///
    /// # Panics
    /// If the constant store is exhausted.
    pub(crate) fn int_const(&mut self, ty: QualType, v: i128) -> ConstValueId {
        let t = conv::strip_cv(self, ty);
        let scalar = if self.ast.types.builtin(t) == Some(BuiltinType::Bool) {
            Scalar::Bool { ty: t, value: v != 0 }
        } else {
            let (w, signed) = int_info(&self.ast, t).unwrap_or((64, true));
            if w > 128 {
                // A wide `_BitInt`: `v` extended over its words.
                let n = IntBits::words_for(w);
                let fill = if v < 0 { u64::MAX } else { 0 };
                let mut words: Vec<u64> = (0..n)
                    .map(|i| match i {
                        0 => v as u64,
                        1 => (v >> 64) as u64,
                        _ => fill,
                    })
                    .collect();
                if w % 64 != 0 {
                    words[n - 1] &= (1u64 << (w % 64)) - 1;
                }
                if let Some(bits) = IntBits::from_words(w, &words) {
                    let scalar = Scalar::Int { ty: t, signed, bits };
                    return self.ast.consts.push_scalar(scalar).expect("integer constants intern");
                }
            }
            Scalar::Int { ty: t, signed, bits: IntBits::from_i128(w.clamp(1, 128), v) }
        };
        self.ast.consts.push_scalar(scalar).expect("integer constants intern")
    }

    /// clang's `UsefulToPrintExpr` negated: literals (and simple
    /// operators over them) are not worth an "evaluates to" note.
    pub(crate) fn is_literal_expr(ast: &Ast, e: ExprId) -> bool {
        let mut e = e;
        for _ in 0..256 {
            let n = ast.exprs.get(e);
            match n.kind() {
                ExprKind::ParenExpr | ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr => {
                    e = ast.exprs.operand(e);
                }
                ExprKind::SubstNonTypeTemplateParmExpr => e = ast.exprs.subst_replacement(e),
                ExprKind::IntegerLiteral
                | ExprKind::FloatingLiteral
                | ExprKind::CharacterLiteral
                | ExprKind::CXXBoolLiteralExpr
                | ExprKind::CXXNullPtrLiteralExpr
                | ExprKind::GNUNullExpr
                | ExprKind::StringLiteral => return true,
                ExprKind::UnaryOperator => e = ast.exprs.operand(e),
                ExprKind::BinaryOperator => {
                    use BinaryOp as B;
                    return !matches!(
                        ast.exprs.binary_op(e),
                        B::Shl
                            | B::Shr
                            | B::Add
                            | B::Sub
                            | B::Mul
                            | B::Div
                            | B::Rem
                            | B::And
                            | B::Or
                            | B::Xor
                    );
                }
                _ => return false,
            }
        }
        false
    }

    pub(crate) fn ignore_paren_imp_casts(&self, e: ExprId) -> ExprId {
        let mut e = e;
        for _ in 0..256 {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ParenExpr | ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr => {
                    e = self.ast.exprs.operand(e);
                }
                ExprKind::SubstNonTypeTemplateParmExpr => e = self.ast.exprs.subst_replacement(e),
                _ => break,
            }
        }
        e
    }

    /// As [`Self::ignore_paren_imp_casts`], also through an
    /// `ExprWithCleanups` (clang's `IgnoreParenImpCasts` steps through
    /// every `FullExpr`): a condition that materializes a temporary, as
    /// `__builtin_bit_cast` does, is still a comparison.
    pub(crate) fn ignore_paren_full_imp_casts(&self, e: ExprId) -> ExprId {
        let mut e = e;
        for _ in 0..256 {
            let x = self.ignore_paren_imp_casts(e);
            if self.ast.exprs.get(x).kind() != ExprKind::ExprWithCleanups {
                return x;
            }
            e = self.ast.exprs.operand(x);
        }
        e
    }

    pub(crate) fn ignore_parens(&self, e: ExprId) -> ExprId {
        let mut e = e;
        for _ in 0..256 {
            // clang `IgnoreParensSingleStep` also looks through a
            // `__builtin_choose_expr` to its chosen operand.
            match self.ast.exprs.get(e).kind() {
                ExprKind::ParenExpr => e = self.ast.exprs.operand(e),
                ExprKind::ChooseExpr => match self.ast.exprs.choose_chosen(e) {
                    Some(c) => e = c,
                    None => break,
                },
                _ => break,
            }
        }
        e
    }

    /// The operator and operands of a failed `static_assert` condition
    /// worth explaining (clang's `DiagnoseStaticAssertDetails`).
    pub(crate) fn comparison_parts(&self, e: ExprId) -> Option<(&'static str, ExprId, ExprId)> {
        let e = self.ignore_paren_full_imp_casts(e);
        if self.ast.exprs.get(e).kind() != ExprKind::BinaryOperator {
            return None;
        }
        let op = self.ast.exprs.binary_op(e);
        if matches!(op, BinaryOp::LOr | BinaryOp::LAnd | BinaryOp::Comma) || op.is_assignment() {
            return None;
        }
        let [l, r] = self.ast.exprs.operands(e);
        let (l, r) = (self.ignore_paren_imp_casts(l), self.ignore_paren_imp_casts(r));
        let is_bool = |x: ExprId| self.ast.types.builtin(self.ety(x)) == Some(BuiltinType::Bool);
        let is_bool_lit = |x: ExprId| self.ast.exprs.get(x).kind() == ExprKind::CXXBoolLiteralExpr;
        if (is_bool_lit(l) && is_bool(r)) || (is_bool_lit(r) && is_bool(l)) {
            return None;
        }
        Some((op.spelling(), l, r))
    }

    /// The location of the first token of `e` (clang's `getBeginLoc`;
    /// [`kiln_ast::Ast::begin_loc`]).
    pub(crate) fn begin_loc(&self, e: ExprId) -> Loc {
        self.ast.begin_loc(e)
    }

    /// The source range of `e` (clang `getSourceRange`) for a diagnostic:
    /// from its first token to the end of its last (to the last token's
    /// start when `token_end` cannot measure it).
    pub(crate) fn expr_range(&self, e: ExprId) -> kiln_source::Span {
        let end = self.expr_end_loc(e);
        kiln_source::Span::new(self.begin_loc(e), self.token_end(end).unwrap_or(end))
    }

    /// The location of the last token of `e` (clang's `getEndLoc`): the
    /// stored closing token (`)`, `}`) when there is one, else the end of
    /// the last operand for a node that ends with one (a binary or prefix
    /// operator, a conditional, a C-style cast, an implicit node), else
    /// the node's own location (a name, a literal, a member name, a
    /// postfix operator).
    pub(crate) fn expr_end_loc(&self, mut e: ExprId) -> Loc {
        loop {
            if let Some(l) = self.ast.exprs.end_loc(e) {
                return l;
            }
            let n = self.ast.exprs.get(e);
            let last = match n.kind() {
                ExprKind::ImplicitCastExpr
                | ExprKind::CStyleCastExpr
                | ExprKind::ConstantExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::BinaryOperator
                | ExprKind::CompoundAssignOperator
                | ExprKind::ConditionalOperator
                | ExprKind::BinaryConditionalOperator => true,
                ExprKind::UnaryOperator => !self.ast.exprs.unary_op(e).is_postfix(),
                ExprKind::CXXRewrittenBinaryOperator => {
                    e = self.ast.exprs.rewritten_semantic(e);
                    continue;
                }
                // clang: the base's range.
                ExprKind::DesignatedInitUpdateExpr => {
                    e = self.ast.exprs.designated_init_update(e)[0];
                    continue;
                }
                ExprKind::CXXOperatorCallExpr => {
                    use kiln_ast::names::OverloadedOperator as O;
                    let op = self.ast.exprs.operator_call_op(e);
                    let args = self.ast.lists.get(self.ast.exprs.call_args(e));
                    let postfix = matches!(op, O::PlusPlus | O::MinusMinus) && args.len() == 2;
                    if matches!(op, O::Call | O::Subscript | O::Arrow) || postfix {
                        return n.loc();
                    }
                    match args.last() {
                        Some(&a) => {
                            e = ExprId::new(a);
                            continue;
                        }
                        None => return n.loc(),
                    }
                }
                _ => false,
            };
            let kids = if last { self.ast.exprs.children(e, &self.ast.lists) } else { Vec::new() };
            match kids.last() {
                Some(&k) => e = k,
                None => return n.loc(),
            }
        }
    }

    /// Records `r`, the `>` closing the template-id `e` names (clang's
    /// `RAngleLoc`, where a template-id's `getEndLoc` is), unless `e`
    /// already ends at a stored token.
    pub fn set_template_id_end(&mut self, e: ExprId, r: Loc) {
        if r != Loc::INVALID && self.ast.exprs.end_loc(e).is_none() {
            let _ = self.ast.exprs.cold.end_loc.insert(e, r);
        }
    }

    // -- full-expressions --------------------------------------------------------

    /// The end of a full-expression ([intro.execution]/5): resolves the
    /// potential odr-uses it contains ([basic.def.odr]/5, spec/sema.md
    /// §5.2). A variable usable in constant expressions whose name is the
    /// operand of an lvalue-to-rvalue conversion is not odr-used
    /// (`non_odr_use_constant`); every other named variable is `used`.
    pub fn act_on_full_expr(&mut self, e: ExprId) -> ExprId {
        let cc = match self.ast.exprs.get(e).kind() {
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr => self.begin_loc(e),
            _ => self.eloc(e),
        };
        self.act_on_full_expr_at(e, cc)
    }

    /// [`Sema::act_on_full_expr`] of a constant-expression
    /// ([expr.const]/22: a `static_assert` condition, an array bound, a
    /// bit-field width, an enumerator value, a `case` value, an alignment,
    /// a template argument).
    /// The immediate invocations of an enclosing full-expression (a
    /// default template argument substituted by its overload resolution)
    /// stay pending ([expr.const]/16).
    ///
    /// Likewise a constant-expression without temporaries of its own (a
    /// template argument converted while an enclosing full-expression is
    /// being built: `use(D(3)) + C<1>::v`, `f<1>(D(3))`, an
    /// `enable_if_t<..., int> = 0` default substituted by overload
    /// resolution) leaves that full-expression's pending cleanups in
    /// place, so its temporaries are still destroyed at its end
    /// ([intro.execution]/5, [class.temporary]/4; clang's
    /// `ConstantEvaluated` context restores `ParentCleanup`).
    ///
    /// A constant-expression with temporaries of its own gets its own
    /// `ExprWithCleanups`; the enclosing full-expression's state from
    /// before it began (recorded by [`Sema::exit_constant_evaluated`] or
    /// the substituter) is then restored, so `use(D(3)) + C<D(1).v>::v`
    /// still destroys `D(3)` at its end. A default template argument is
    /// substituted and converted in a context of its own
    /// ([`Sema::default_template_arg`]).
    ///
    /// A non-static member named without an object makes it erroneous
    /// (clang's `ExprError` from `DiagnoseInstanceReference`): no "not a
    /// constant expression" follows the error.
    pub(crate) fn act_on_constant_full_expr(&mut self, e: ExprId) -> ExprId {
        if self.flush_member_refs() {
            let loc = self.ast.exprs.get(e).loc();
            return self.recover_err(&[e], loc);
        }
        let outer = self.set_aside_outer_immediates(e);
        let pending = if self.has_own_temporaries(e) {
            self.take_constant_outer_cleanups(e)
        } else {
            Some(std::mem::take(&mut self.cleanup))
        };
        // Only the names this constant expression uses are decided here
        // (clang keeps the potential odr-uses per expression evaluation
        // context): the later arguments of `f<0, k>()`, or the rest of an
        // enclosing expression, are still pending, and a constant `k` from
        // an enclosing function named in a lambda is not captured because
        // an earlier argument completed first.
        let names = self.decl_refs_in(e);
        let (mine, others): (Vec<ExprId>, Vec<ExprId>) =
            std::mem::take(&mut self.pending_odr).into_iter().partition(|r| names.contains(r));
        self.pending_odr = mine;
        self.constant_full_expr += 1;
        let r = self.act_on_full_expr(e);
        self.constant_full_expr -= 1;
        let rest = std::mem::replace(&mut self.pending_odr, others);
        self.pending_odr.extend(rest);
        if let Some(p) = pending {
            self.cleanup = p;
        }
        self.restore_outer_immediates(outer);
        r
    }

    /// The reused walk stack ([`Sema::walk_stack`]), holding just `root`.
    /// Give it back with [`Sema::put_walk_stack`].
    pub(crate) fn take_walk_stack(&self, root: ExprId) -> Vec<ExprId> {
        let mut stack = self.walk_stack.take();
        stack.clear();
        stack.push(root);
        stack
    }

    /// Returns the stack of [`Sema::take_walk_stack`] for the next walk.
    pub(crate) fn put_walk_stack(&self, stack: Vec<ExprId>) {
        self.walk_stack.replace(stack);
    }

    /// The `DeclRefExpr`s in `e`.
    fn decl_refs_in(&self, e: ExprId) -> Vec<ExprId> {
        let mut out = Vec::new();
        let mut stack = self.take_walk_stack(e);
        while let Some(x) = stack.pop() {
            if out.len() > 1_000_000 {
                break;
            }
            if self.ast.exprs.get(x).kind() == ExprKind::DeclRefExpr {
                out.push(x);
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        self.put_walk_stack(stack);
        out
    }

    /// Whether `e` materializes or binds a temporary, or uses a default
    /// argument or member initializer (whose temporaries it then owns),
    /// outside a nested `ExprWithCleanups`: its own cleanups, as opposed
    /// to those of a full-expression it is nested in.
    fn has_own_temporaries(&self, e: ExprId) -> bool {
        let mut stack = self.take_walk_stack(e);
        let mut found = false;
        while let Some(x) = stack.pop() {
            match self.ast.exprs.get(x).kind() {
                ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::CXXDefaultArgExpr
                | ExprKind::CXXDefaultInitExpr => {
                    found = true;
                    break;
                }
                ExprKind::ExprWithCleanups | ExprKind::LambdaExpr => {}
                _ => self.ast.exprs.children_into(x, &self.ast.lists, &mut stack),
            }
        }
        self.put_walk_stack(stack);
        found
    }

    /// [`Sema::act_on_full_expr`] with conversion context `cc` for the
    /// implicit-conversion warnings (clang's `CheckCompletedExpr`
    /// location: the declared variable, `return`).
    pub(crate) fn act_on_full_expr_at(&mut self, e: ExprId, cc: Loc) -> ExprId {
        if !self.lambdas.is_empty()
            && self.ast.exprs.get(e).deps().contains(kiln_ast::expr::ExprDeps::UNEXPANDED_PACK)
            && let Some(r) = self.diagnose_unexpanded_full_expr(e)
        {
            return r;
        }
        // clang never completes an `ExprError` (a failed list
        // initialization, [`Sema::is_expr_error`]).
        if !self.is_expr_error(e) {
            self.check_implicit_conversions(e, cc);
            // clang `CheckCompletedExpr`: not for a `constexpr`
            // variable's initializer (its evaluation reports it).
            let v = self.pending_var_init();
            self.check_for_int_overflow(e, v);
        }
        self.flush_member_refs();
        if self.special.has_pending() {
            self.flush_pending_deleted();
        }
        // [expr.const]/16: the immediate invocations are evaluated.
        self.finish_immediate_invocations(e);
        // [intro.execution]/5: the temporaries of the full-expression are
        // destroyed at its end (spec/sema-class.md §5.3).
        let e = self.finish_cleanups(e);
        if !self.misaligned.is_empty() {
            self.diagnose_misaligned_members(e);
        }
        if self.pending_odr.is_empty() && self.potential_caps.is_empty() {
            return e;
        }
        let mut constant = Vec::new();
        let mut stack = vec![e];
        let mut seen = 0usize;
        while let Some(x) = stack.pop() {
            seen += 1;
            if seen > 1_000_000 {
                break;
            }
            let n = *self.ast.exprs.get(x);
            if n.kind() == ExprKind::ImplicitCastExpr
                && self.ast.exprs.cast_kind(x) == CastKind::LValueToRValue
            {
                self.potential_results(self.ast.exprs.operand(x), &mut constant);
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        if !self.potential_caps.is_empty() && self.in_template() {
            self.resolve_potential_captures(e, &constant);
        }
        let pending = std::mem::take(&mut self.pending_odr);
        for r in pending {
            let d = self.ast.exprs.decl(r);
            let is_ref =
                self.ast.types.is_reference(self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR));
            if (is_ref || constant.contains(&r)) && self.usable_in_constant_expressions(d) {
                let _ = self.ast.exprs.set_non_odr_use(r, NonOdrUse::Constant);
                continue;
            }
            self.mark_used(d);
            let at = self.eloc(r);
            self.note_undefined_use(d, at);
            self.capture_odr_use(d, at);
        }
        e
    }

    /// The `DeclRefExpr`s among the potential results of `e`
    /// ([basic.def.odr]/3).
    pub(crate) fn potential_results(&self, e: ExprId, out: &mut Vec<ExprId>) {
        let n = self.ast.exprs.get(e);
        match n.kind() {
            ExprKind::DeclRefExpr => out.push(e),
            ExprKind::ParenExpr => self.potential_results(self.ast.exprs.operand(e), out),
            ExprKind::ConditionalOperator => {
                let [t, f] = self.ast.exprs.cond_arms(e);
                self.potential_results(t, out);
                self.potential_results(f, out);
            }
            ExprKind::BinaryOperator if self.ast.exprs.binary_op(e) == BinaryOp::Comma => {
                self.potential_results(self.ast.exprs.operands(e)[1], out);
            }
            _ => {}
        }
    }

    /// [expr.const]/4: a `constexpr` variable, or a `const`, non-volatile
    /// integral or enumeration variable, or a reference, with a constant
    /// initializer. (A reference's initializer is constant in M2 when it
    /// names a function or a variable with static storage duration.)
    pub(crate) fn usable_in_constant_expressions(&mut self, d: DeclId) -> bool {
        let n = self.ast.decls.get(d);
        if n.kind() != DeclKind::Var {
            return false;
        }
        let Some(v) = self.ast.decls.var(d) else { return false };
        // The initializer may be on another declaration (an out-of-line
        // definition of a static data member, [class.static.data]/3).
        let spec = v.spec;
        let holder = if v.init.is_some() {
            d
        } else {
            let Some(x) = self
                .ast
                .redecls_of(d)
                .find(|&x| self.ast.decls.var(x).is_some_and(|y| y.init.is_some()))
            else {
                return false;
            };
            x
        };
        let Some(init) = self.ast.decls.var(holder).and_then(|y| y.init) else { return false };
        let ty = n.ty().unwrap_or(QualType::ERROR);
        if self.ast.types.is_reference(ty) {
            // A reference is usable in constant expressions when its
            // initializer is a constant expression ([expr.const]/4): a
            // static lvalue (the common case, decided syntactically), or
            // anything else the Oven evaluates to an lvalue constant
            // (`const int& r = 5;`, `const P& r = P{1};`, `int& r = a[1];`
            // at namespace scope; clang marks these `non_odr_use_constant`
            // too, and codegen then reads through them, WP-231).
            if self.is_static_lvalue(init) {
                return true;
            }
            if let Some(&u) = self.ref_usable.get(&holder) {
                return u && self.representable_here(holder);
            }
            if self.has_errors_in(init)
                || self.expr_is_value_dependent(init)
                || self.is_dependent_type(ty)
            {
                return false;
            }
            let u = self.is_constant_initialized(holder);
            self.ref_usable.insert(holder, u);
            return u && self.representable_here(holder);
        }
        let q = self.ast.types.effective_quals(ty);
        let ok = spec.is_constexpr()
            || (q.is_const()
                && !q.is_volatile()
                && self.is_integral_or_enum(ty)
                && !self.ast.types.is_reference(ty));
        ok && self.is_constant_initialized(holder) && self.representable_here(holder)
    }

    /// P2686R5 [expr.const]/8-9: a variable whose value refers to an
    /// automatic object (`constexpr int& r = local;`) is usable in
    /// constant expressions only where that object is
    /// constexpr-referenceable: in its own function, not in a lambda
    /// nested in it, where naming the variable odr-uses (captures) it.
    fn representable_here(&mut self, d: DeclId) -> bool {
        let x = if let Some(&x) = self.auto_referent.get(&d) {
            x
        } else {
            let Some(kiln_ast::decl::VarEvalState::Value(c)) =
                self.ast.decls.var(d).map(|v| v.eval.state())
            else {
                return true;
            };
            let x = kiln_oven::automatic_referent(&self.ast, c);
            self.auto_referent.insert(d, x);
            x
        };
        x.is_none_or(|x| self.ast.decls.get(x).ctx() == self.enclosing_function(self.ctx))
    }

    /// True if the glvalue `e` designates a function or a variable with
    /// static storage duration (through parentheses and no-op casts).
    fn is_static_lvalue(&self, mut e: ExprId) -> bool {
        loop {
            let n = self.ast.exprs.get(e);
            match n.kind() {
                ExprKind::ParenExpr => e = self.ast.exprs.operand(e),
                ExprKind::ImplicitCastExpr
                    if matches!(
                        self.ast.exprs.cast_kind(e),
                        CastKind::NoOp | CastKind::DerivedToBase | CastKind::UncheckedDerivedToBase
                    ) =>
                {
                    e = self.ast.exprs.operand(e);
                }
                ExprKind::DeclRefExpr => {
                    let d = self.ast.exprs.decl(e);
                    return match self.ast.decls.get(d).kind() {
                        DeclKind::Function => true,
                        DeclKind::Var => self.has_static_storage(d),
                        _ => false,
                    };
                }
                _ => return false,
            }
        }
    }

    /// True if variable `d` has static storage duration ([basic.stc.static]):
    /// a namespace-scope variable or a `static`/`extern` local, not
    /// `thread_local`.
    pub(crate) fn has_static_storage(&self, d: DeclId) -> bool {
        let Some(v) = self.ast.decls.var(d) else { return false };
        if v.spec.tls() != kiln_ast::decl::TlsKind::None {
            return false;
        }
        let local =
            self.ast.decls.get(d).ctx().is_some_and(|c| self.ast.decls.get(c).kind().is_function());
        !local
            || matches!(
                v.spec.storage(),
                kiln_ast::decl::VarStorage::Static | kiln_ast::decl::VarStorage::Extern
            )
    }

    /// Marks `d` odr-used ([basic.def.odr]/5; clang's `markUsed`, kept on
    /// the first declaration).
    pub(crate) fn mark_used(&mut self, d: DeclId) {
        let f = self.ast.first(d);
        self.ast.decls.set_flags(f, DeclFlags::USED);
    }

    /// Whether function `d` has an invalid parameter (a `void`
    /// parameter, an erroneous default argument), which made it invalid
    /// at its declaration.
    pub(crate) fn invalid_by_param(&self, d: DeclId) -> bool {
        self.param_decls(d).iter().any(|&p| self.ast.decls.get(p).has(DeclFlags::INVALID))
    }

    /// [`Sema::mark_used`] for a use at `loc`: a member function of a
    /// class template specialization is instantiated ([temp.inst]/5).
    pub(crate) fn mark_used_at(&mut self, d: DeclId, loc: Loc) {
        self.mark_used(d);
        self.note_used_for_instantiation(d, loc);
        self.note_undefined_use(d, loc);
    }

    /// Converts `e` to `ty` as in `ctx`, diagnosing failure (the result
    /// is then a `RecoveryExpr`).
    pub(crate) fn convert_for_init(&mut self, e: ExprId, ty: QualType, ctx: ConvCtx) -> ExprId {
        // A braced-init-list argument (`T x({1, 2})`) list-initializes
        // the parameter ([dcl.init.list]).
        if let Some(l) = self.take_braced_arg(e) {
            let to = if self.ast.types.is_reference(ty) { ty } else { conv::strip_cv(self, ty) };
            return self.init_list_entity(to, &l, l.lbrace, ctx, None);
        }
        // `_Atomic(T)` from a `T` of class type: `T` is initialized, then
        // made atomic (a `return` or argument; WP-183).
        if self.ast.types.as_atomic(ty).is_some()
            && let Some(x) = self.atomic_class_init(ty, e, ctx, self.begin_loc(e))
        {
            return x;
        }
        match conv::implicit_convert(self, e, ty, ctx) {
            Ok(x) => x,
            Err(err) => {
                // clang's `InitializationKind::CreateCopy` of a return
                // operand or argument is at the expression's start.
                let d = if matches!(ctx, ConvCtx::Return | ConvCtx::Argument) {
                    err.diag_at(self, self.begin_loc(e))
                } else {
                    err.diag(self, e)
                };
                self.emit(d);
                let loc = self.eloc(e);
                self.recover(ty, &[e], loc)
            }
        }
    }

    /// clang's `DiagnoseUnusedExprResult` for an expression statement.
    pub(crate) fn diagnose_unused(&mut self, e: ExprId) {
        // A type-dependent expression is checked at instantiation (clang
        // does not warn in the template definition). Nothing is unused in
        // an unevaluated operand (clang `DiagnoseUnusedExprResult`'s
        // `isUnevaluatedContext()`: a statement expression in `decltype`).
        if self.unevaluated > 0
            || self.is_err_expr(e)
            || self.expr_is_type_dependent(e)
            || self.holds_typo_expr(e)
        {
            return;
        }
        if let Some(w) = self.unused_warning(e) {
            self.emit_unused(e, w, false);
        }
    }

    /// clang `CheckCommaOperands` → `DiagnoseUnusedExprResult(LHS,
    /// warn_unused_comma_left_operand)`: the left operand of a built-in
    /// comma whose value is unused. Not in an unevaluated operand, and the
    /// generic warning not in a SFINAE context.
    pub(crate) fn diagnose_comma_lhs(&mut self, l: ExprId) {
        if self.unevaluated > 0
            || self.is_err_expr(l)
            || self.expr_is_type_dependent(l)
            || self.holds_typo_expr(l)
        {
            return;
        }
        if let Some(w) = self.unused_warning(l) {
            self.emit_unused(l, w, true);
        }
    }

    /// Emits `unused_warning(e)` as clang's `DiagnoseUnusedExprResult`
    /// does: a comparison is `-Wunused-comparison` only when it is the
    /// unused expression itself (clang `DiagnoseUnusedComparison` sees the
    /// top expression, so `(x == 1);` and `y, x == 1;` are the generic
    /// warning), with the `=` / `|=` fix-it note; the generic warning is
    /// the comma form for a comma's left operand (`comma`); nothing but
    /// `nodiscard` is diagnosed for an expression spelled in a macro body
    /// (clang `ShouldSuppress` / `isMacroBodyExpansion`).
    fn emit_unused(&mut self, e: ExprId, w: (DiagId, Loc, Vec<String>), comma: bool) {
        let (mut id, loc, mut args) = w;
        let mut note = None;
        if id == DiagId::sema_unused_comparison {
            let (top, n) = self.top_comparison(e, loc);
            note = n;
            if !top {
                // clang `DiagnoseUnusedExprResult`: a comparison that is
                // not the statement itself (`(a == b);`) is checked as
                // the call it is, at the operator.
                let call = self.unused_comparison_call(e, loc);
                if let Some((i, _, a)) = call.and_then(|c| self.unused_call_result(c, loc)) {
                    (id, args) = (i, a);
                } else {
                    id = DiagId::sema_unused_expr;
                    args.clear();
                }
            }
        }
        if comma && id == DiagId::sema_unused_expr {
            if self.inst.stack.is_sfinae() {
                return;
            }
            id = DiagId::sema_unused_comma_left_operand;
        }
        if !Self::is_nodiscard_warning(id) && self.in_macro_body(e) {
            return;
        }
        // The expression's range (clang `<< R1 << R2` / `getSourceRange`):
        // a diagnostic with a range keeps its "expanded from macro" notes
        // for a macro argument.
        let mut d = args.into_iter().fold(self.diag(id, loc), Diag::arg);
        if id == DiagId::sema_unused_voidptr {
            // clang's only range is the fix-it removing the `*`.
            if let Some(star) = self.voidptr_star(e) {
                d = d.fixit(kiln_diag::FixIt::remove(kiln_source::Span::new(
                    star,
                    star.with_offset(1),
                )));
            }
        } else if self.clang_compat >= 22 {
            // clang 22 decides the macro-argument backtrace on the
            // diagnostic's `R1`/`R2` as streamed (spec/diag.md §3.3): a
            // built-in binary operator's two operands, else the
            // expression with an unset (invalid) `R2`.
            let x = self.ignore_parens(e);
            if self.ast.exprs.get(x).kind() == ExprKind::BinaryOperator
                && matches!(id, DiagId::sema_unused_expr | DiagId::sema_unused_comma_left_operand)
            {
                let [l, r] = self.ast.exprs.operands(x);
                d = d.range(self.expr_range(l)).range(self.expr_range(r));
            } else {
                d = d.range(self.expr_range(e)).range(kiln_source::Span::INVALID);
            }
        } else {
            d = d.range(self.expr_range(e));
        }
        // clang rebuilds every expression statement and for-increment of
        // an instantiation (`TransformExprStmt` → `ActOnExprStmt`), so a
        // non-dependent one warns again with the instantiation's notes,
        // not deduplicated against the template definition's warning.
        // The comma's left operand is checked only as the comma is built.
        if !comma && !self.inst.stack.is_empty() {
            d = d.allow_duplicate();
        }
        if let Some(n) = note {
            // The fix-it's range (clang `FixItHint::CreateReplacement` of
            // the operator) keeps the note's macro-argument backtrace too.
            let text = if n == DiagId::note_equality_comparison_to_assign { "=" } else { "|=" };
            let op = kiln_source::Span::new(loc, loc.with_offset(2));
            d = d.with_note(self.diag(n, loc).fixit(kiln_diag::FixIt::replace(op, text)));
        }
        // Only the generic warnings go through clang's `DiagIfReachable`.
        if matches!(id, DiagId::sema_unused_expr | DiagId::sema_unused_comma_left_operand) {
            self.diag_if_reachable(d, e);
        } else {
            self.emit(d);
        }
    }

    /// The overloaded comparison at `loc` that [`Self::unused_warning`]
    /// reported for `e` (clang's `WarnExpr`), along the operands it looks
    /// through.
    fn unused_comparison_call(&self, e: ExprId, loc: Loc) -> Option<ExprId> {
        let ex = &self.ast.exprs;
        let mut x = e;
        for _ in 0..256 {
            x = self.ignore_parens(x);
            let n = ex.get(x);
            x = match n.kind() {
                ExprKind::CXXOperatorCallExpr if n.loc() == loc => return Some(x),
                ExprKind::CXXBindTemporaryExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::ImplicitCastExpr => ex.operand(x),
                ExprKind::CStyleCastExpr | ExprKind::CXXFunctionalCastExpr
                    if ex.cast_kind(x) == CastKind::ConstructorConversion =>
                {
                    ex.operand(x)
                }
                ExprKind::BinaryOperator if ex.binary_op(x) == BinaryOp::Comma => ex.operands(x)[1],
                ExprKind::ConditionalOperator => ex.cond_arms(x)[1],
                ExprKind::BinaryConditionalOperator => {
                    ex.binary_conditional_operands(x, &self.ast.lists)[3]
                }
                _ => return None,
            };
        }
        None
    }

    /// The `*` of the `(void*)` cast `e` (the last one spelled before
    /// the operand).
    fn voidptr_star(&self, e: ExprId) -> Option<Loc> {
        let x = self.ignore_parens(e);
        let at = self.ast.exprs.get(x).loc();
        let end = self.begin_loc(self.ast.exprs.operand(x));
        let text = self.diags.sm().span_text(kiln_source::Span::new(at, end))?;
        let i = text.rfind('*')?;
        Some(at.with_offset(u32::try_from(i).ok()?))
    }

    /// clang 22 `warn_unused_return_type`: "ignoring {return
    /// value|temporary} of type 'T' declared with 'nodiscard' attribute",
    /// with the attribute's message if any.
    fn unused_type_warning(
        &self,
        which: &str,
        ty: QualType,
        name: String,
        text: Option<String>,
        at: Loc,
    ) -> (DiagId, Loc, Vec<String>) {
        let mut args = vec![which.to_owned(), self.ty_arg(ty), name];
        let id = match text {
            Some(t) => {
                args.push(t);
                DiagId::sema_unused_return_type_msg
            }
            None => DiagId::sema_unused_return_type,
        };
        (id, at, args)
    }

    /// The `nodiscard` forms of the unused-result warning.
    fn is_nodiscard_warning(id: DiagId) -> bool {
        matches!(
            id,
            DiagId::sema_unused_result
                | DiagId::sema_unused_result_msg
                | DiagId::sema_unused_constructor
                | DiagId::sema_unused_constructor_msg
                | DiagId::sema_unused_return_type
                | DiagId::sema_unused_return_type_msg
        )
    }

    /// clang `SourceManager::isMacroBodyExpansion` of the expression's
    /// location (its operator, for an operator expression).
    fn in_macro_body(&self, e: ExprId) -> bool {
        let at = self.eloc(self.ignore_paren_imp_casts(e));
        self.diags
            .sm()
            .expansion_info(at)
            .is_some_and(|x| matches!(x.kind, kiln_source::ExpansionKind::Macro { .. }))
    }

    /// clang `DiagnoseUnusedComparison`: whether `e` itself (not
    /// parenthesized, not nested) is the comparison at `loc`, and then the
    /// fix-it note for an `==` / `!=` (built-in or overloaded) whose left
    /// operand is an lvalue: `=` / `|=`.
    fn top_comparison(&self, e: ExprId, loc: Loc) -> (bool, Option<DiagId>) {
        use kiln_ast::names::OverloadedOperator as O;
        let mut x = e;
        while matches!(
            self.ast.exprs.get(x).kind(),
            ExprKind::ExprWithCleanups
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ImplicitCastExpr
        ) {
            x = self.ast.exprs.operand(x);
        }
        if self.ast.exprs.get(x).loc() != loc {
            return (false, None);
        }
        let (eq, lhs) = match self.ast.exprs.get(x).kind() {
            ExprKind::BinaryOperator => match self.ast.exprs.binary_op(x) {
                BinaryOp::EQ => (Some(true), self.ast.exprs.operands(x)[0]),
                BinaryOp::NE => (Some(false), self.ast.exprs.operands(x)[0]),
                BinaryOp::LT | BinaryOp::GT | BinaryOp::LE | BinaryOp::GE | BinaryOp::Cmp => {
                    return (true, None);
                }
                _ => return (false, None),
            },
            ExprKind::CXXOperatorCallExpr => {
                let eq = match self.ast.exprs.operator_call_op(x) {
                    O::EqualEqual => true,
                    O::ExclaimEqual => false,
                    O::Less | O::Greater | O::LessEqual | O::GreaterEqual | O::Spaceship => {
                        return (true, None);
                    }
                    _ => return (false, None),
                };
                let args = self.ast.lists.get(self.ast.exprs.call_args(x));
                let Some(&a) = args.first() else { return (false, None) };
                (Some(eq), ExprId::new(a))
            }
            _ => return (false, None),
        };
        // clang `IgnoreParenImpCasts` also looks through a materialized
        // temporary (`M{} == 1` is not assignable).
        let mut lhs = self.ignore_paren_imp_casts(lhs);
        while self.ast.exprs.get(lhs).kind() == ExprKind::MaterializeTemporaryExpr {
            lhs = self.ignore_paren_imp_casts(self.ast.exprs.operand(lhs));
        }
        if self.ast.exprs.get(lhs).value_category() != ValueCategory::LValue {
            return (true, None);
        }
        let note = eq.map(|eq| {
            if eq {
                DiagId::note_equality_comparison_to_assign
            } else {
                DiagId::note_inequality_comparison_to_or_assign
            }
        });
        (true, note)
    }

    #[allow(clippy::too_many_lines, reason = "one flat classification of expression kinds")]
    pub(crate) fn unused_warning(&self, e: ExprId) -> Option<(DiagId, Loc, Vec<String>)> {
        let x = self.ignore_parens(e);
        let n = *self.ast.exprs.get(x);
        let loc = n.loc();
        match n.kind() {
            // clang's `isUnusedResultAWarning`: user-defined comparisons
            // warn as the built-in ones do.
            ExprKind::CXXOperatorCallExpr
                if !self.ast.types.is_reference(n.ty())
                    && !self.ast.types.is_void(n.ty())
                    && matches!(
                        self.ast.exprs.operator_call_op(x),
                        kiln_ast::names::OverloadedOperator::EqualEqual
                            | kiln_ast::names::OverloadedOperator::ExclaimEqual
                            | kiln_ast::names::OverloadedOperator::Less
                            | kiln_ast::names::OverloadedOperator::Greater
                            | kiln_ast::names::OverloadedOperator::LessEqual
                            | kiln_ast::names::OverloadedOperator::GreaterEqual
                    ) =>
            {
                let which = match self.ast.exprs.operator_call_op(x) {
                    kiln_ast::names::OverloadedOperator::EqualEqual => "0",
                    kiln_ast::names::OverloadedOperator::ExclaimEqual => "1",
                    _ => "2",
                };
                Some((DiagId::sema_unused_comparison, loc, vec![which.to_owned()]))
            }
            ExprKind::CXXRewrittenBinaryOperator => {
                Some((DiagId::sema_unused_expr, loc, Vec::new()))
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr | ExprKind::CXXMemberCallExpr => {
                // clang `isUnusedResultAWarning`: a call with a callee
                // declaration (`getCalleeDecl`), reported at the callee's
                // start.
                let callee = self.ast.exprs.callee(x);
                // clang 22 reports an operator call at its start.
                let at = if self.clang_compat >= 22 && n.kind() == ExprKind::CXXOperatorCallExpr {
                    self.begin_loc(x)
                } else {
                    self.begin_loc(callee)
                };
                self.unused_call_result(x, at)
            }
            ExprKind::BinaryOperator => {
                let op = self.ast.exprs.binary_op(x);
                match op {
                    BinaryOp::EQ => {
                        Some((DiagId::sema_unused_comparison, loc, vec!["0".to_owned()]))
                    }
                    BinaryOp::NE => {
                        Some((DiagId::sema_unused_comparison, loc, vec!["1".to_owned()]))
                    }
                    BinaryOp::LT | BinaryOp::GT | BinaryOp::LE | BinaryOp::GE => {
                        Some((DiagId::sema_unused_comparison, loc, vec!["2".to_owned()]))
                    }
                    BinaryOp::Comma => {
                        // clang `isUnusedResultAWarning`: `(x = y, 0)` is
                        // an idiom for hiding an assignment's result (an
                        // integer literal 0 as the right operand, [expr.comma]).
                        let r = self.ast.exprs.operands(x)[1];
                        let ri = self.ignore_parens(r);
                        if self.ast.exprs.get(ri).kind() == ExprKind::IntegerLiteral
                            && self.ast.exprs.int_value(ri) == Some(0)
                        {
                            return None;
                        }
                        self.unused_warning(r)
                    }
                    BinaryOp::LAnd | BinaryOp::LOr => {
                        // `a && f()` is control flow when the right side has effects.
                        let r = self.ast.exprs.operands(x)[1];
                        self.unused_warning(r).map(|_| (DiagId::sema_unused_expr, loc, Vec::new()))
                    }
                    _ if op.is_assignment() => None,
                    // A volatile scalar `.*` / `->*` result is read when
                    // discarded ([expr.context]/2): clang sees the
                    // `LValueToRValue` cast and does not warn.
                    BinaryOp::PtrMemD | BinaryOp::PtrMemI
                        if self.std >= kiln_lex::Std::Cxx11
                            && self.ast.types.effective_quals(n.ty()).is_volatile()
                            && self.ast.types.as_record(n.ty()).is_none() =>
                    {
                        None
                    }
                    _ => Some((DiagId::sema_unused_expr, loc, Vec::new())),
                }
            }
            ExprKind::UnaryOperator => {
                let op = self.ast.exprs.unary_op(x);
                if op.is_inc_dec() {
                    return None;
                }
                if op == UnaryOp::Deref && self.ast.types.effective_quals(n.ty()).is_volatile() {
                    return None;
                }
                Some((DiagId::sema_unused_expr, loc, Vec::new()))
            }
            ExprKind::ConditionalOperator => {
                // Only when both arms warn (otherwise it may be control
                // flow); clang reports the false arm's warning.
                let [t, f] = self.ast.exprs.cond_arms(x);
                self.unused_warning(t)?;
                self.unused_warning(f)
            }
            ExprKind::BinaryConditionalOperator => {
                let ops = self.ast.exprs.binary_conditional_operands(x, &self.ast.lists);
                self.unused_warning(ops[3])
            }
            ExprKind::CXXScalarValueInitExpr if !self.ast.types.is_void(n.ty()) => {
                Some((DiagId::sema_unused_expr, self.begin_loc(x), Vec::new()))
            }
            ExprKind::CStyleCastExpr | ExprKind::CXXFunctionalCastExpr => {
                match self.ast.exprs.cast_kind(x) {
                    CastKind::ToVoid | CastKind::Dependent => None,
                    // `X(1);` is checked through its constructor call
                    // (clang `isUnusedResultAWarning`), reported at the cast.
                    CastKind::ConstructorConversion => self
                        .unused_warning(self.ast.exprs.operand(x))
                        .map(|(id, _, args)| (id, loc, args)),
                    _ if let Some(w) = self.unused_through_cast(x, loc) => Some(w),
                    // clang `warn_unused_voidptr`: `(void*)x` written so
                    // (not through a typedef) is taken for a typo of
                    // `(void)x`.
                    _ if n.kind() == ExprKind::CStyleCastExpr
                        && n.ty().quals_bits() == 0
                        && matches!(self.ast.types.view(n.ty().ty()),
                            TypeView::Pointer(p) if p == QualType::VOID) =>
                    {
                        Some((DiagId::sema_unused_voidptr, loc, Vec::new()))
                    }
                    _ => Some((DiagId::sema_unused_expr, loc, Vec::new())),
                }
            }
            // clang `isUnusedResultAWarning`'s default case: a void
            // result is never unused (`static_cast<void>(x);`).
            ExprKind::CXXStaticCastExpr if self.ast.types.is_void(n.ty()) => None,
            // Always unused, but through a conversion of a call or
            // construction its attributes are checked.
            ExprKind::CXXStaticCastExpr
            | ExprKind::CXXConstCastExpr
            | ExprKind::CXXReinterpretCastExpr => {
                Some(self.unused_through_cast(x, loc).unwrap_or((
                    DiagId::sema_unused_expr,
                    loc,
                    Vec::new(),
                )))
            }
            ExprKind::CXXBindTemporaryExpr
            | ExprKind::MaterializeTemporaryExpr
            | ExprKind::ExprWithCleanups
            | ExprKind::ImplicitCastExpr => self.unused_warning(self.ast.exprs.operand(x)),
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                self.unused_construct_result(x, loc)
            }
            ExprKind::StmtExpr => self.unused_stmt_expr(x, loc),
            // clang's default case, at `getExprLoc` (the base's begin:
            // `ExtVectorElementExpr` has no location of its own there).
            ExprKind::ExtVectorElementExpr => {
                Some((DiagId::sema_unused_expr, self.begin_loc(x), Vec::new()))
            }
            ExprKind::DeclRefExpr | ExprKind::MemberExpr | ExprKind::ArraySubscriptExpr => {
                if self.ast.types.effective_quals(n.ty()).is_volatile() {
                    None
                } else if n.kind() == ExprKind::ArraySubscriptExpr {
                    // clang `isUnusedResultAWarning` reports a subscript
                    // at its `]` (`getRBracketLoc`).
                    let at = self.ast.exprs.end_loc(x).unwrap_or(loc);
                    Some((DiagId::sema_unused_expr, at, Vec::new()))
                } else if n.kind() == ExprKind::DeclRefExpr {
                    // clang's `Expr::getExprLoc` of a DeclRefExpr is its
                    // begin: a qualified name (`N::x;`) is reported at
                    // its nested-name-specifier (WP-318).
                    Some((DiagId::sema_unused_expr, self.begin_loc(x), Vec::new()))
                } else {
                    Some((DiagId::sema_unused_expr, loc, Vec::new()))
                }
            }
            ExprKind::IntegerLiteral
            | ExprKind::FloatingLiteral
            | ExprKind::CharacterLiteral
            | ExprKind::StringLiteral
            | ExprKind::CXXBoolLiteralExpr
            | ExprKind::CXXNullPtrLiteralExpr
            | ExprKind::GNUNullExpr
            | ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::TypeTraitExpr
            | ExprKind::ArrayTypeTraitExpr
            | ExprKind::ExpressionTraitExpr
            | ExprKind::CXXTypeidExpr
            | ExprKind::CompoundLiteralExpr
            | ExprKind::LambdaExpr => Some((DiagId::sema_unused_expr, loc, Vec::new())),
            _ => None,
        }
    }

    /// clang `DiagnoseUnusedExprResult` for a discarded call `x` with a
    /// callee declaration (`getCalleeDecl`), reported at `at`: a
    /// nodiscard result type or callee, else a `pure` / `const` callee.
    fn unused_call_result(&self, x: ExprId, at: Loc) -> Option<(DiagId, Loc, Vec<String>)> {
        let f = self.referenced_decl_of_callee(self.ast.exprs.callee(x))?;
        if self.ast.types.is_void(self.ast.exprs.get(x).ty()) {
            return None;
        }
        // clang `getUnusedResultAttr`: a nodiscard class or
        // enumeration returned by value first, then the callee
        // ([dcl.attr.nodiscard]/2).
        let t = self.ast.exprs.get(x).ty();
        let by_type = if self.call_returns_reference(x) {
            None
        } else {
            let tag = self.ast.types.as_record(t).or_else(|| self.ast.types.as_enum(t));
            tag.and_then(|d| self.find_attr(d, AttrKind::NODISCARD))
        };
        if let Some(a) = by_type.or_else(|| self.find_attr(f, AttrKind::NODISCARD)) {
            // clang appends the attribute's message
            // ([dcl.attr.nodiscard]/4).
            let attr = *self.ast.attrs.get(a);
            let text = ExprId::from_raw(attr.a).and_then(|m| self.string_text(m));
            let name = "'nodiscard'".to_owned();
            // clang 22 `warn_unused_return_type`: a nodiscard
            // type is named, under -Wunused-value.
            if by_type.is_some() && self.clang_compat >= 22 {
                return Some(self.unused_type_warning("0", t, name, text, at));
            }
            return Some(match text {
                Some(t) => (DiagId::sema_unused_result_msg, at, vec![name, t]),
                None => (DiagId::sema_unused_result, at, vec![name]),
            });
        }
        // clang `warn_unused_call`: a `pure` / `const` callee
        // whose result is not `void` (WP-182).
        for (k, name) in [(AttrKind::PURE, "pure"), (AttrKind::CONST, "const")] {
            if self.find_attr(f, k).is_some() {
                return Some((DiagId::sema_unused_call, at, vec![name.to_owned()]));
            }
        }
        None
    }

    /// A discarded temporary `x` of a nodiscard class, or made by a
    /// nodiscard constructor ([dcl.attr.nodiscard]/4, clang
    /// `warn_unused_constructor`), reported at `loc`.
    fn unused_construct_result(&self, x: ExprId, loc: Loc) -> Option<(DiagId, Loc, Vec<String>)> {
        let n = *self.ast.exprs.get(x);
        let ctor = self.ast.exprs.constructor(x);
        let by_ctor = self.find_attr(ctor, AttrKind::NODISCARD);
        let a = by_ctor.or_else(|| {
            let class = self.ast.types.as_record(n.ty())?;
            self.find_attr(class, AttrKind::NODISCARD)
        })?;
        let attr = *self.ast.attrs.get(a);
        let name = "'nodiscard'".to_owned();
        if by_ctor.is_none() && self.clang_compat >= 22 {
            let text = ExprId::from_raw(attr.a).and_then(|m| self.string_text(m));
            return Some(self.unused_type_warning("1", n.ty(), name, text, loc));
        }
        Some(match ExprId::from_raw(attr.a).and_then(|m| self.string_text(m)) {
            Some(t) => (DiagId::sema_unused_constructor_msg, loc, vec![name, t]),
            None => (DiagId::sema_unused_constructor, loc, vec![name]),
        })
    }

    /// clang `DiagnoseUnusedExprResult` under an explicit cast `x`: a
    /// `NoOp`, constructor or integral conversion of a call or
    /// construction (`static_cast<bool>(s)` calling a nodiscard
    /// conversion function), its implicit casts stripped, is checked as
    /// that call or construction, at the cast.
    fn unused_through_cast(&self, x: ExprId, loc: Loc) -> Option<(DiagId, Loc, Vec<String>)> {
        if !matches!(
            self.ast.exprs.cast_kind(x),
            CastKind::NoOp | CastKind::ConstructorConversion | CastKind::IntegralCast
        ) {
            return None;
        }
        let inner = self.ignore_imp_casts(self.ast.exprs.operand(x));
        match self.ast.exprs.get(inner).kind() {
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr | ExprKind::CXXMemberCallExpr => {
                self.unused_call_result(inner, loc)
            }
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                self.unused_construct_result(inner, loc)
            }
            _ => None,
        }
    }

    /// clang `Expr::getReferencedDeclOfCallee`: the declaration a callee
    /// names, under parens, implicit casts and substituted template
    /// parameters, through `.*` / `->*` (its right operand) and unary
    /// `*`, `&` and `+`.
    pub(crate) fn referenced_decl_of_callee(&self, callee: ExprId) -> Option<DeclId> {
        let ex = &self.ast.exprs;
        let mut e = self.ignore_paren_imp_casts(callee);
        for _ in 0..256 {
            let n = ex.get(e);
            e = match n.kind() {
                ExprKind::SubstNonTypeTemplateParmExpr => {
                    self.ignore_paren_imp_casts(ex.subst_replacement(e))
                }
                ExprKind::BinaryOperator if ex.binary_op(e).is_ptr_mem() => {
                    self.ignore_paren_imp_casts(ex.operands(e)[1])
                }
                ExprKind::UnaryOperator
                    if matches!(
                        ex.unary_op(e),
                        UnaryOp::Deref | UnaryOp::AddrOf | UnaryOp::Plus
                    ) =>
                {
                    self.ignore_paren_imp_casts(ex.operand(e))
                }
                ExprKind::DeclRefExpr | ExprKind::MemberExpr => return Some(ex.decl(e)),
                _ => return None,
            };
        }
        None
    }

    /// True if `e` (under parens) names a function that is already invalid
    /// (an unknown return or parameter type): clang's
    /// `BuildDeclarationNameExpr` makes such a name an error unless it is
    /// resolved as a call's overload set, so taking its address or
    /// converting it is erroneous without a diagnostic of its own
    /// (`&A::f` into a pointer to member, `void (*p)() = k;`; WP-379).
    pub(crate) fn names_invalid_function(&self, e: ExprId) -> bool {
        let x = self.ignore_parens(e);
        self.ast.exprs.get(x).kind() == ExprKind::DeclRefExpr && {
            let d = self.ast.decls.get(self.ast.exprs.decl(x));
            d.kind().is_function() && d.has(DeclFlags::INVALID)
        }
    }

    /// clang `CallExpr::getCallReturnType` is a reference: the callee's
    /// declared result (a call's own type drops the reference).
    fn call_returns_reference(&self, call: ExprId) -> bool {
        self.ast.exprs.get(call).value_category().is_glvalue()
    }

    pub(crate) fn find_attr(&self, d: DeclId, kind: AttrKind) -> Option<AttrId> {
        let mut cur = Some(d);
        let mut n = 0;
        while let Some(x) = cur {
            if let Some(a) = self
                .ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(x))
                .find(|&a| self.ast.attrs.get(a).kind == kind)
            {
                return Some(a);
            }
            n += 1;
            if n > 4096 {
                break;
            }
            cur = self.ast.prev(x);
        }
        None
    }

    // -- names -------------------------------------------------------------------

    /// An unqualified id-expression `name` that is the callee of a call
    /// (followed by `(`): as [`Sema::act_on_id_expression`], but non-member
    /// functions it names become an `UnresolvedLookupExpr` with ADL
    /// unless argument-dependent lookup is suppressed
    /// (spec/sema-overload.md §2.1–§2.2), and a function it names
    /// directly is marked referenced only by [`Sema::act_on_call`] once
    /// the call is known not to be dependent ([temp.dep.candidate]).
    pub fn act_on_call_name(&mut self, name: Symbol, loc: Loc) -> ExprId {
        self.act_on_call_name_dn(DeclName::ident(name), loc)
    }

    /// True if `d` is declared directly in a function body (a block-scope
    /// declaration, [basic.scope.block]).
    pub(crate) fn is_block_scope_decl(&self, d: DeclId) -> bool {
        self.ast.decls.get(d).ctx().is_some_and(|c| self.kind_of(c).is_function())
    }

    /// [`Sema::act_on_call_name`] for any unqualified-id (`operator()(x)`
    /// inside a member function names `this->operator()`; WP-121).
    pub fn act_on_call_name_dn(&mut self, name: DeclName, loc: Loc) -> ExprId {
        let saved = std::mem::replace(&mut self.deferring_call_ref, true);
        let e = self.act_on_id_expression_dn(name, loc, None);
        self.deferring_call_ref = saved;
        e
    }

    /// An id-expression naming `name` ([expr.prim.id]), optionally
    /// qualified by the nested-name-specifier `qualifier`, which the
    /// resulting `DeclRefExpr` records as written (clang's
    /// `getQualifier()`; the expression printer shows it).
    pub fn act_on_id_expression(
        &mut self,
        name: Symbol,
        loc: Loc,
        qualifier: Option<crate::using::Qualifier>,
    ) -> ExprId {
        self.act_on_id_expression_dn(DeclName::ident(name), loc, qualifier)
    }

    /// [`Self::act_on_id_expression`], told whether the name is the whole
    /// operand of unary `&` (clang's `isAddressOfOperand`). A qualified
    /// name there that names a non-static member forms no implicit member
    /// access in a lambda, so `&C::m` does not capture `this`
    /// ([expr.prim.id.general]/3, [expr.prim.lambda.capture]/8).
    pub fn act_on_id_expression_in(
        &mut self,
        name: Symbol,
        loc: Loc,
        qualifier: Option<crate::using::Qualifier>,
        address_operand: bool,
    ) -> ExprId {
        let addr = address_operand && qualifier.is_some();
        let saved = std::mem::replace(&mut self.address_operand, addr);
        let e = self.act_on_id_expression(name, loc, qualifier);
        self.address_operand = saved;
        e
    }

    /// The qualifier's begin `b` of the overload-set reference `e` named
    /// at `loc` (clang's `OverloadExpr::getBeginLoc`).
    fn set_overload_begin(&mut self, e: ExprId, begin: Option<Loc>, loc: Loc) {
        if let Some(b) = begin
            && b != loc
        {
            let _ = self.ast.templates.expr_begin.insert(e, b);
        }
    }

    /// clang's `getBeginLoc` of a qualified name is the qualifier's: the
    /// begin `b` of `DeclRefExpr` `e` named at `loc`.
    fn set_qualified_begin(&mut self, e: ExprId, begin: Option<Loc>, loc: Loc) {
        if let Some(b) = begin
            && b != loc
        {
            if self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr {
                let _ = self.ast.templates.expr_begin.insert(e, b);
            } else if let Some(r) = self.member_refs.get_mut(&e) {
                r.begin = Some(b);
            }
        }
    }

    /// The empty-lookup error for the unqualified `name`: `use of
    /// undeclared 'operator+'` for an operator-, conversion- or
    /// literal-operator-id, `use of undeclared identifier 'f'` otherwise
    /// (clang's `DiagnoseEmptyLookup`).
    pub(crate) fn undeclared_use_id(name: DeclName) -> DiagId {
        use kiln_ast::names::DeclNameKind as K;
        match name.kind() {
            K::Operator | K::Conversion | K::LiteralOperator => DiagId::sema_undeclared_use,
            _ => DiagId::sema_undeclared_var_use,
        }
    }

    /// The replaceable global allocation functions are visible to lookup
    /// without `<new>` ([basic.stc.dynamic.general]/2): declares them
    /// before an unqualified or `::`-qualified lookup of their names.
    fn declare_global_new_delete_for(&mut self, name: DeclName, qualifier: Option<DeclId>) {
        use kiln_ast::names::OverloadedOperator as O;
        if qualifier.is_none_or(|q| q == self.ast.tu)
            && matches!(name.as_operator(), Some(O::New | O::ArrayNew | O::Delete | O::ArrayDelete))
        {
            self.declare_global_new_delete();
        }
    }

    /// [`Sema::act_on_id_expression`] for any unqualified-id: an
    /// identifier, an operator-function-id or a conversion-function-id
    /// ([expr.prim.id.unqual]/1; WP-121).
    #[allow(clippy::too_many_lines)]
    pub fn act_on_id_expression_dn(
        &mut self,
        name: DeclName,
        loc: Loc,
        qualifier: Option<crate::using::Qualifier>,
    ) -> ExprId {
        let written = qualifier;
        let nns = qualifier.map(|q| q.nns);
        let begin = qualifier.map(|q| q.loc);
        let qualifier = qualifier.map(|q| q.ctx);
        self.declare_global_new_delete_for(name, qualifier);
        match qualifier {
            Some(q) => self.declare_implicit_named(q, name),
            None => self.declare_implicit_named_in_scope(name),
        }
        let found = match qualifier {
            Some(q) => self.lookup_qualified_dn(q, name, LookupKind::Ordinary),
            None => self.lookup_name_dn(name, LookupKind::Ordinary),
        };
        if self.diagnose_member_ambiguity(&found, name, loc) {
            return self.recover_err(&[], loc);
        }
        // A builtin whose visible declarations are all invalid
        // redeclarations of it (spec/sema.md §5.4): clang's lookup finds
        // its implicit declaration too.
        if qualifier.is_none_or(|q| q == self.ast.tu)
            && !found.decls.is_empty()
            && found.decls.iter().all(|&d| {
                self.kind_of(d) == DeclKind::Function
                    && self.ast.decls.get(d).flags().contains(DeclFlags::INVALID)
            })
            && let Some(sym) = name.as_ident()
            && let Some(b) = self.lazily_create_builtin(sym, loc)
        {
            return self.build_decl_ref(b, loc);
        }
        let Some(&d) = found.decls.first() else {
            // A builtin is declared in the global namespace on first use,
            // so `::__builtin_memmove` names it too (clang).
            if qualifier.is_none_or(|q| q == self.ast.tu)
                && let Some(sym) = name.as_ident()
                && let Some(b) = self.lazily_create_builtin(sym, loc)
            {
                return self.build_decl_ref(b, loc);
            }
            if qualifier.is_none()
                && self.deferring_call_ref
                && Self::undeclared_use_id(name) == DiagId::sema_undeclared_use
            {
                // `operator<=>(a, b)` with nothing visible: argument-
                // dependent lookup at the call, or at instantiation when
                // an argument is dependent ([basic.lookup.argdep]/1,
                // [temp.dep.candidate]), like an undeclared identifier.
                return self.act_on_undeclared_call_name_dn(name, loc);
            }
            // `D::x` with `D` the current instantiation and nothing found:
            // a member of an unknown specialization when `D` has a
            // dependent base ([temp.dep.type]/6, [temp.dep.expr]/3;
            // clang's `wasNotFoundInCurrentInstantiation`), looked up again
            // at instantiation (libc++ test support's
            // `return (UnwrapAdaptor::c);`).
            if let (Some(ctx), Some(nns), Some(b)) = (qualifier, nns, begin)
                && self.kind_of(ctx) == DeclKind::CXXRecord
                && let Some(n) =
                    self.unknown_specialization_nns(crate::using::Qualifier { ctx, nns, loc: b })
            {
                return self.act_dependent_scope_ref(n, name, None, b, loc);
            }
            // A declaration that is reachable but not visible: clang's
            // missing-import error, then the name is used as found
            // ([module.reach], spec/modules.md §5).
            if let Some(f) = self.module_hidden_lookup(name, qualifier, LookupKind::Ordinary) {
                self.module_diagnose_hidden(&f, name, loc);
                self.modules_see_hidden(true);
                let e = self.act_on_id_expression_dn(name, loc, written);
                self.modules_see_hidden(false);
                return e;
            }
            let text = self.decl_name_str(name);
            if let Some(q) = qualifier {
                // clang reports it through delayed typo correction, at the
                // end of the full-expression.
                let msg =
                    self.diag(DiagId::sema_no_member, loc).arg(text).arg(self.qualifier_desc(q));
                let e = self.recover_err(&[], loc);
                let e = self.mark_expr_error(e);
                self.emit_delayed_typo(msg, e);
                return e;
            }
            let msg = self.diag(Self::undeclared_use_id(name), loc).arg(text);
            let e = self.recover_err(&[], loc);
            let e = self.mark_expr_error(e);
            if self.clang_compat < 22 {
                // clang 19's `TypoExpr`: reported at the end of the
                // full-expression or where the parser gives up on it
                // (`f(u 8, x)` skips to the `)`, WP-420).
                self.emit_delayed_typo(msg, e);
            } else {
                // clang 22: the range of the name.
                let end = self.token_end(loc).unwrap_or(loc);
                self.emit(msg.range(kiln_source::Span::new(loc, end)));
            }
            return e;
        };
        if found.decls.len() > 1
            && found.decls.iter().any(|&x| self.kind_of(x) == DeclKind::UnresolvedUsingValue)
        {
            // A dependent using-declarator beside the class's own
            // overloads (`using T::f; int f(char);`): one overload set,
            // resolved at instantiation, like a lone one (below).
            return self.builder().dependent_member(None, true, name, false, nns, None, loc);
        }
        // clang `LookupResult::resolveKind`: an invalid function template
        // (one declared with a parameter type that names no type) is
        // ignored beside other declarations, so `t3` naming an invalid
        // template and a valid function is that function (WP-308).
        let bad = |s: &Self, x: DeclId| {
            s.kind_of(x) == DeclKind::FunctionTemplate && s.ast.decls.get(x).has(DeclFlags::INVALID)
        };
        let mut found = found;
        let mut d = d;
        if found.decls.len() > 1 && found.decls.iter().any(|&x| !bad(self, x)) {
            let keep: Vec<bool> = found.decls.iter().map(|&x| !bad(self, x)).collect();
            if keep.contains(&false) {
                let mut k = keep.iter();
                found.decls.retain(|_| k.next().copied().unwrap_or(true));
                if found.found.len() == keep.len() {
                    let mut k = keep.iter();
                    found.found.retain(|_| k.next().copied().unwrap_or(true));
                }
                d = found.decls[0];
            }
        }
        // A function template declared at file scope with a builtin's name
        // (`template<class D, class S> void *__builtin_memmove(D*, S*,
        // size_t)`) does not redeclare the builtin: clang's redeclaration
        // lookup creates the builtin in the translation unit first
        // (`HandleDeclarator`'s `CreateBuiltins`), so both are candidates
        // and the builtin wins a tie as a non-template ([over.match.best]/2.4;
        // spec/sema.md §5.4).
        if qualifier.is_none_or(|q| q == self.ast.tu)
            && found.naming.is_none()
            && found.decls.iter().any(|&x| self.kind_of(x) == DeclKind::FunctionTemplate)
            && found.decls.iter().all(|&x| {
                matches!(self.kind_of(x), DeclKind::FunctionTemplate | DeclKind::Function)
                    && self.ast.decls.builtin_id(x).is_none()
                    && self.ast.decls.get(x).ctx().map(|c| self.ast.redecl_context(c))
                        == Some(self.ast.tu)
            })
            && let Some(sym) = name.as_ident()
            && let Some(b) = self.lazily_create_builtin(sym, loc)
        {
            found.decls.push(b);
            if !found.found.is_empty() {
                found.found.push(b);
            }
        }
        let any_tmpl = found.decls.iter().any(|&x| self.kind_of(x) == DeclKind::FunctionTemplate);
        if any_tmpl
            && found.decls.iter().all(|&x| {
                let k = self.kind_of(x);
                k == DeclKind::FunctionTemplate || k.is_function()
            })
        {
            // A name of a function template without `<` ([temp.arg.explicit]/4):
            // an overload set resolved at the call (clang's
            // `UnresolvedLookupExpr`; deduction is WP-071).
            if found.naming.is_some()
                && !self.address_operand
                && found.decls.iter().any(|&x| self.member_via_dependent_base(x))
                && let Some((this_ty, _)) = self.this_class()
            {
                return self.dependent_implicit_set(this_ty, &found.decls, name, nns, loc);
            }
            if found.naming.is_some() {
                // A member template: the member overload set (an
                // implicit `this->` for instance members).
                let valid = found.decls.clone();
                let e = self.overload_name_ref(name, &found, &valid, qualifier.is_none(), nns, loc);
                self.set_overload_begin(e, begin, loc);
                return e;
            }
            // ADL only for a name followed by `(` ([basic.lookup.argdep]/1:
            // `(f)(x)` suppresses it; clang `UseArgumentDependentLookup`).
            let adl = self.call_uses_adl(&found.decls, qualifier.is_none());
            let e = self.builder().unresolved_lookup(
                name,
                &lookup_set_order(&found.decls, qualifier.is_none()),
                adl,
                nns,
                None,
                loc,
            );
            self.set_overload_begin(e, begin, loc);
            return e;
        }
        let fns = found.decls.iter().all(|&x| self.ast.decls.get(x).kind().is_function());
        // An invalid redeclaration is not a separate overload: clang's
        // `resolveKind` ignores an invalid declaration unless it is the
        // only one left (redeclarations of an invalid function are
        // invalid and off its chain, WP-288).
        let mut valid: Vec<DeclId> = found
            .decls
            .iter()
            .copied()
            .filter(|&x| !self.ast.decls.get(x).has(DeclFlags::INVALID))
            .collect();
        if fns && valid.is_empty() {
            valid.extend(found.decls.last().copied());
        }
        // `&N::f` naming an overload set stays the qualified name, not
        // `this->N::f`: an address-of operand forms no implicit member
        // access ([expr.unary.op]/3), so the set is resolved against the
        // target type ([over.over]) even when `N` turns out unrelated to
        // the instantiated class (WP-715).
        let addr_set = self.address_operand && valid.len() > 1;
        if fns
            && !addr_set
            && found.naming.is_some()
            && valid.iter().any(|&x| self.member_via_dependent_base(x))
            && let Some((this_ty, _)) = self.this_class()
        {
            let e = self.dependent_implicit_set(this_ty, &valid, name, nns, loc);
            if let [d] = valid[..] {
                self.note_qualified_dependent_member(e, d, nns);
            }
            self.set_overload_begin(e, begin, loc);
            return e;
        }
        if fns && (valid.len() > 1 || self.call_uses_adl(&valid, qualifier.is_none())) {
            let e = self.overload_name_ref(name, &found, &valid, qualifier.is_none(), nns, loc);
            self.set_overload_begin(e, begin, loc);
            return e;
        }
        if fns && valid.len() == 1 {
            d = valid[0];
        } else if found.decls.len() > 1 && valid.len() <= 1 {
            // Likewise for other entities: `const int c = 1; const c = 2;`
            // names the valid `c`, `y; y;` the last `y` (silently).
            d = valid.first().or(found.decls.last()).copied().unwrap_or(d);
        } else if found.decls.len() > 1 {
            // Distinct entities found via using-directives ([namespace.udir]/6).
            self.diagnose_ambiguous_reference(name, &found.decls, qualifier.is_none(), loc);
            return self.recover_err(&[], loc);
        }
        if self.kind_of(d) == DeclKind::UnresolvedUsingIfExists {
            self.diag_unresolved_using(d, loc);
            return self.recover_err(&[], loc);
        }
        if self.kind_of(d) == DeclKind::UnresolvedUsingValue && self.is_block_scope_decl(d) {
            // A local named by a dependent block-scope using-declarator
            // (`using E<T>::a; ... a`): a dependent name looked up again
            // in the instantiation (clang's `UnresolvedLookupExpr` over
            // the `UnresolvedUsingValueDecl`, without ADL).
            return self.builder().unresolved_lookup_in(name, &[d], false, nns, None, true, loc);
        }
        if self.kind_of(d) == DeclKind::UnresolvedUsingValue {
            // A member named by a dependent using-declarator: an implicit
            // `this->name` (or `Class::name`) resolved at instantiation
            // (clang's UnresolvedLookupExpr over the
            // UnresolvedUsingValueDecl).
            return self.builder().dependent_member(None, true, name, false, nns, None, loc);
        }
        if let Some(naming) = found.naming {
            // [class.access.base]/5: the object is `*this` for an implicit
            // member access, also one through a dependent base (an implicit
            // `this->Q::name`, [temp.dep.type]/6) whose check is replayed in
            // the instantiation with `*this`'s instantiated class.
            let object = if !self.is_instance_member(d) {
                None
            } else if self.member_via_dependent_base(d) {
                self.this_class().map(|(_, c)| c)
            } else {
                self.implicit_object_class(d)
            };
            let via = found.found.first().copied().unwrap_or(d);
            self.check_member_access(via, naming, object, loc);
        }
        if self.is_class_member_ref(d) {
            if self.member_via_dependent_base(d)
                && let Some((this_ty, _)) = self.this_class()
            {
                let e = self.dependent_implicit_member(this_ty, name, nns, loc);
                self.note_qualified_dependent_member(e, d, nns);
                self.set_qualified_begin(e, begin, loc);
                return e;
            }
            let e = self.implicit_member_ref(d, loc, found.shadow(0), nns);
            self.set_qualified_begin(e, begin, loc);
            return e;
        }
        let e = self.build_decl_ref_qualified(d, loc, found.shadow(0), nns);
        self.set_qualified_begin(e, begin, loc);
        self.note_function_name_use(d, e, loc, qualifier.is_none());
        e
    }

    /// [basic.lookup.argdep]/1–3 for the callee of a call being built by
    /// [`Sema::act_on_call_name`]: an `unqualified` name whose lookup
    /// found only non-member functions (or function templates), none of
    /// them a block-scope declaration or an implicit builtin
    /// (spec/sema-overload.md §2.2, clang `UseArgumentDependentLookup`).
    #[inline(never)]
    pub(crate) fn call_uses_adl(&self, decls: &[DeclId], unqualified: bool) -> bool {
        unqualified
            && self.deferring_call_ref
            && !decls.is_empty()
            && decls.iter().all(|&d| {
                matches!(self.kind_of(d), DeclKind::Function | DeclKind::FunctionTemplate)
                    && self.ast.decls.builtin_id(d).is_none()
                    && !self
                        .ast
                        .decls
                        .lexical_ctx(d)
                        .is_some_and(|l| self.ast.decls.get(l).kind().is_function())
                    && !self
                        .ast
                        .decls
                        .get(d)
                        .ctx()
                        .is_some_and(|c| crate::overload::adl::is_class_kind(self.kind_of(c)))
            })
    }

    /// The `UnresolvedLookupExpr` for an id-expression naming the
    /// functions `valid` (the non-invalid decls of `found`): overload
    /// resolution happens at the call (spec/sema-overload.md §2.1).
    /// Unqualified results are listed newest first (§2.1, candidate
    /// order). `None` after diagnosing an unsupported form.
    #[inline(never)]
    pub(crate) fn overload_name_ref(
        &mut self,
        name: DeclName,
        found: &crate::lookup::Found,
        valid: &[DeclId],
        unqualified: bool,
        nns: Option<kiln_ast::NnsId>,
        loc: Loc,
    ) -> ExprId {
        if let Some(e) = self.implicit_member_overload(name, found, valid, nns, loc) {
            return e;
        }
        let adl = found.naming.is_none() && self.call_uses_adl(valid, unqualified);
        let mut decls: Vec<DeclId> = Vec::with_capacity(valid.len());
        let mut via: Vec<DeclId> = Vec::with_capacity(valid.len());
        for (i, &d) in found.decls.iter().enumerate() {
            if valid.contains(&d) {
                decls.push(d);
                via.push(found.found.get(i).copied().unwrap_or(d));
            }
        }
        if unqualified && found.naming.is_none() {
            decls.reverse();
            via.reverse();
        }
        self.build_overload_ref(name, &decls, &via, found.naming, adl, nns, loc)
    }

    /// An overloaded member name used with an implicit object
    /// ([class.mfct.non.static]/2, [over.match.call]/2.2): `this->f` where
    /// `this` may be used and its class is (derived from) the members'
    /// class (spec/sema-overload.md §2.3). Without such a `this` the set
    /// stays an `UnresolvedLookupExpr`, and a non-static winner is "call
    /// to non-static member function without an object argument".
    fn implicit_member_overload(
        &mut self,
        name: DeclName,
        found: &crate::lookup::Found,
        valid: &[DeclId],
        nns: Option<kiln_ast::NnsId>,
        loc: Loc,
    ) -> Option<ExprId> {
        let instance = |s: &Self, d: DeclId| {
            let d = if s.kind_of(d) == DeclKind::FunctionTemplate {
                s.pattern_of(d).unwrap_or(d)
            } else {
                d
            };
            s.is_instance_member(d)
        };
        let &first = valid.iter().find(|&&d| instance(self, d))?;
        let first = if self.kind_of(first) == DeclKind::FunctionTemplate {
            self.pattern_of(first).unwrap_or(first)
        } else {
            first
        };
        let Some(class) = self.implicit_object_class(first) else {
            // A base on the way to the members' class is dependent: the
            // call is resolved at instantiation. Not for `&N::f`, which
            // stays the qualified set (WP-715).
            if self.address_operand && nns.is_some() {
                return None;
            }
            let this_ty = self.dependent_implicit_this(first)?;
            // With a static candidate too, `this` is captured only if
            // an instance member wins at instantiation.
            return Some(self.dependent_implicit_set(this_ty, valid, name, nns, loc));
        };
        let (this_ty, _) = self.this_class()?;
        let mut decls = Vec::with_capacity(valid.len());
        let mut via = Vec::with_capacity(valid.len());
        for (i, &d) in found.decls.iter().enumerate() {
            if valid.contains(&d) {
                decls.push(d);
                via.push(found.found.get(i).copied().unwrap_or(d));
            }
        }
        // With a static candidate too, `this` is captured only once the
        // call names an instance member (`finish_member_winner`).
        let all_instance =
            decls.iter().all(|&d| self.is_instance_member(self.templated_or_self(d)));
        let obj_ty = self.ast.types.pointee(this_ty).unwrap_or(QualType::ERROR);
        if all_instance {
            self.note_this_use(loc);
        }
        let this = self.builder().this_expr(this_ty, true, loc);
        if self.ast.types.is_dependent(obj_ty) {
            // [temp.dep.expr]/3: `this` is type-dependent in a member of a
            // class template, so the call of an overloaded member through
            // it is resolved at instantiation, as `this->f(…)` is
            // (`try_dependent_member_q`). Overloads that differ only in a
            // dependent `enable_if_t<…, int> = 0` cannot be told apart
            // before then (libc++ `__shared_ptr_emplace`).
            let b = conv::decay_dependent(self, this);
            return Some(self.builder().dependent_member(
                Some(b),
                true,
                name,
                false,
                nns,
                None,
                loc,
            ));
        }
        let set = crate::overload::OvlMember {
            name,
            decls,
            found: via,
            naming: found.naming.unwrap_or(class),
            obj_ty,
            def: class,
            base: this,
            arrow: true,
            nns,
            implicit: true,
            targs: None,
        };
        Some(self.build_member_overload(set, loc))
    }

    /// A `DeclRefExpr` naming the value `d`.
    pub fn build_decl_ref(&mut self, d: DeclId, loc: Loc) -> ExprId {
        self.build_decl_ref_via(d, loc, None)
    }

    /// A `DeclRefExpr` naming the value `d`, found through the
    /// using-shadow `shadow` when that is set (clang's `FoundDecl`).
    pub fn build_decl_ref_via(&mut self, d: DeclId, loc: Loc, shadow: Option<DeclId>) -> ExprId {
        self.build_decl_ref_qualified(d, loc, shadow, None)
    }

    /// A `DeclRefExpr` naming the value `d`, found through `shadow`, with
    /// the nested-name-specifier `nns` as written (`N::x`, `A<int>::y`,
    /// `::z`).
    pub fn build_decl_ref_qualified(
        &mut self,
        d: DeclId,
        loc: Loc,
        shadow: Option<DeclId>,
        nns: Option<kiln_ast::ids::NnsId>,
    ) -> ExprId {
        let extra = (shadow.is_some() || nns.is_some()).then_some(kiln_ast::expr::DeclRefExtra {
            qualifier: nns,
            template_args: None,
            found_decl: shadow,
        });
        self.build_decl_ref_extra(d, loc, extra)
    }

    /// [`Self::build_decl_ref_via`] with the side record `extra` (a
    /// variable template specialization records the template as its
    /// found decl and the arguments as written; a qualified name its
    /// nested-name-specifier).
    #[allow(
        clippy::too_many_lines,
        reason = "the 074/087 use checks and the 070b variable-template record meet here"
    )]
    pub(crate) fn build_decl_ref_extra(
        &mut self,
        d: DeclId,
        loc: Loc,
        extra: Option<kiln_ast::expr::DeclRefExtra>,
    ) -> ExprId {
        // A variable template specialization records its template as the
        // found decl, which is no using-shadow.
        let shadow = extra.and_then(|x| x.found_decl).filter(|&f| {
            extra.is_some_and(|x| x.template_args.is_none())
                || self.kind_of(f) != DeclKind::VarTemplate
        });
        let n = *self.ast.decls.get(d);
        match n.kind() {
            // [dcl.struct.bind]/1: a binding is not usable before its
            // decomposition is initialized.
            DeclKind::Binding if n.has(DeclFlags::INVALID) => return self.recover_err(&[], loc),
            DeclKind::Binding if n.ty().is_none() => {
                let msg =
                    self.diag(DiagId::sema_binding_in_own_initializer, loc).arg(self.decl_name(d));
                self.emit(msg);
                return self.recover_err(&[], loc);
            }
            // A local variable named before it has an initializer (in its
            // own initializer, or declared without one) arms the
            // uninitialized-values analysis of the body.
            DeclKind::Var => self.note_uninit_var(d),
            DeclKind::ParmVar
            | DeclKind::Binding
            | DeclKind::Function
            | DeclKind::EnumConstant
            | DeclKind::NonTypeTemplateParm
            | DeclKind::ContractResultName => {}
            DeclKind::IndirectField
                if self
                    .ast
                    .lists
                    .iter::<DeclId>(self.ast.indirect_field_chain(d))
                    .next()
                    .map(|h| self.kind_of(h))
                    == Some(DeclKind::Var) =>
            {
                return self.anonymous_member_ref(d, loc);
            }
            DeclKind::CXXMethod if self.is_static_method(d) => {}
            _ if self.is_class_member_ref(d) => {
                return self.implicit_member_ref(d, loc, shadow, extra.and_then(|x| x.qualifier));
            }
            // [temp.names]/1: a variable template or concept is named with
            // arguments (clang's `diagnoseMissingTemplateArguments`).
            DeclKind::VarTemplate | DeclKind::Concept => {
                let what =
                    if n.kind() == DeclKind::Concept { "concept" } else { "variable template" };
                self.missing_template_args(d, what, extra.and_then(|x| x.qualifier), loc);
                return self.recover_err(&[], loc);
            }
            k if crate::lookup::is_type_kind(k) || k == DeclKind::Namespace => {
                let msg = self
                    .diag(DiagId::sema_ref_non_value, loc)
                    .arg(format!("'{}'", self.decl_name(d)));
                self.emit(msg);
                return self.recover_err(&[], loc);
            }
            _ => {
                self.unsupported("this kind of name", loc);
                return self.recover_err(&[], loc);
            }
        }
        // A reference to a function already invalid (its body failed to
        // instantiate, diagnosed then) is erroneous with no diagnostic of
        // its own (clang's `BuildDeclarationNameExpr`).
        // A function invalid only for an invalid parameter is named as
        // usual (clang resolves its calls through an overload set).
        if n.kind().is_function() && n.has(DeclFlags::INVALID) && !self.invalid_by_param(d) {
            return self.recover_err(&[], loc);
        }
        if matches!(n.kind(), DeclKind::Function | DeclKind::CXXMethod)
            && self.check_undeduced_use(d, loc)
        {
            return self.recover_err(&[], loc);
        }
        // clang `BuildDeclarationNameExpr`: "Only create DeclRefExpr's for
        // valid Decl's" (an invalid variable's uses raise no more errors).
        if matches!(n.kind(), DeclKind::Var | DeclKind::ParmVar) && n.has(DeclFlags::INVALID) {
            return self.recover_err(&[], loc);
        }
        self.check_sve_use(d, loc);
        // [dcl.spec.auto.general]/11: a placeholder variable's type is
        // not known inside its own initializer.
        if n.kind() == DeclKind::Var
            && !n.has(DeclFlags::INVALID)
            && let Some(t) = n.ty()
            && self.contains_undeduced_auto(t)
            && !self.undeduced_var_spec_use_19(d)
        {
            let msg = match self.auto_expl_spec_in_own_init(d, loc) {
                Some(m) => m,
                None => self
                    .diag(DiagId::sema_auto_var_in_own_initializer, loc)
                    .arg(self.decl_name(d))
                    .arg(self.ty_arg(t)),
            };
            self.emit(msg);
            self.ast.decls.set_flags(d, DeclFlags::INVALID);
            return self.recover_err(&[], loc);
        }
        // The exception specification is part of the type ([dcl.fct]/14):
        // resolved before the reference takes it (clang's
        // `DiagnoseUseOfDecl`).
        if matches!(n.kind(), DeclKind::Function | DeclKind::CXXMethod) {
            self.resolve_exc_for_type(d, loc);
        }
        // Re-read: `check_undeduced_use` may have deduced the return type.
        let mut declared = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        // A pack names its pattern ([temp.variadic]/5): `Ts... x` gives `x`
        // the type `Ts`.
        if let TypeView::PackExpansion { pattern, .. } = self.ast.types.view(declared.ty()) {
            declared = pattern;
        }
        let ty = if self.ast.decls.builtin_id(d).is_some() {
            QualType::BUILTIN_FN
        } else if matches!(n.kind(), DeclKind::Function | DeclKind::CXXMethod)
            && self.undeduced_dependent_use(d)
        {
            QualType::DEPENDENT
        } else if n.kind() == DeclKind::NonTypeTemplateParm
            && self.ast.types.as_record(declared).is_some()
        {
            // [temp.param]/8: an lvalue of type `const T`.
            self.ast.types.qualify(declared, kiln_ast::types::Quals::CONST)
        } else if n.kind() == DeclKind::NonTypeTemplateParm && self.is_ctad_placeholder(declared) {
            // `template<C V>` of a class template `C`: the type is
            // deduced per argument, so a use is type-dependent (clang's
            // dependent `DeducedTemplateSpecializationType`).
            QualType::DEPENDENT
        } else if n.kind() == DeclKind::EnumConstant
            && let Some(t) = self.enumerator_pre_brace_type(d)
        {
            t
        } else if n.kind() == DeclKind::NonTypeTemplateParm
            && self.placeholder_keyword(declared).is_some()
        {
            // `template<auto& R>`: the placeholder is deduced per
            // argument, so a use is type-dependent (clang's dependent
            // `AutoType`).
            let t = self.dependent_auto(declared);
            self.non_reference(t)
        } else {
            self.non_reference(declared)
        };
        let defer = self.deferring_call_ref
            && n.kind() == DeclKind::Function
            && self.ast.decls.builtin_id(d).is_none();
        if !defer {
            self.ast.decls.set_flags(d, DeclFlags::REFERENCED);
        }
        if n.kind() == DeclKind::Var {
            self.note_var_referenced(d, loc);
        }
        if matches!(n.kind(), DeclKind::Var | DeclKind::ParmVar) {
            self.count_var_ref(d);
        }
        self.diagnose_deprecated(d, loc);
        let nou = if self.unevaluated > 0 { NonOdrUse::Unevaluated } else { NonOdrUse::None };
        // [expr.prim.id.unqual]/3: naming a variable captured by an
        // enclosing lambda (spec/sema-lambda.md §3).
        let (ty, flags) = match n.kind() {
            DeclKind::Var | DeclKind::ParmVar | DeclKind::ContractResultName => {
                match self.capture_at_decl_ref(d, loc) {
                    Some(t) => (t, ExprFlags::REFERS_TO_ENCLOSING),
                    None => (ty, ExprFlags::NONE),
                }
            }
            // clang 19 keeps a captured binding's own type (its
            // `getCapturedDeclRefType` adjusts variables only); clang 22
            // follows [expr.prim.id.unqual]/3 for bindings too (`const`
            // inside a non-mutable lambda that copies it).
            DeclKind::Binding => match self.capture_at_decl_ref(d, loc) {
                Some(t) if self.clang_compat >= 22 => (t, ExprFlags::REFERS_TO_ENCLOSING),
                Some(_) => (ty, ExprFlags::REFERS_TO_ENCLOSING),
                None => (ty, ExprFlags::NONE),
            },
            _ => (ty, ExprFlags::NONE),
        };
        // [expr.prim.id.unqual]/6-7: const in a contract predicate.
        let ty = if self.contract_ctx.is_empty() {
            ty
        } else {
            self.contract_constify(d, ty, flags == ExprFlags::REFERS_TO_ENCLOSING)
        };
        let e = self.builder().decl_ref(d, ty, loc, extra, nou, flags);
        if flags == ExprFlags::REFERS_TO_ENCLOSING
            && matches!(n.kind(), DeclKind::Var | DeclKind::ParmVar)
            && self.captured_by_copy_in_dependent_object_lambda(d)
        {
            self.ast.exprs.mark_type_dependent(e);
        }
        // A binding to a bit-field designates it (clang `BuildDeclRefExpr`).
        if n.kind() == DeclKind::Binding
            && let Some(b) = self.ast.decls.binding_expr(d)
            && self.is_bitfield(b)
        {
            self.ast.exprs.mark_bitfield(e);
        }
        if defer {
            self.deferred_call_refs.push((e, d));
        }
        // Inside a template definition a use is only a reference; odr-use
        // happens at instantiation ([temp.res.general]/6).
        if self.unevaluated == 0 && !self.in_template() {
            match n.kind() {
                DeclKind::Function | DeclKind::CXXMethod => {
                    let at = self.eloc(e);
                    self.mark_function_referenced(d, at);
                    self.note_immediate_ref(e, d);
                }
                DeclKind::Var | DeclKind::ParmVar => self.pending_odr.push(e),
                _ => {}
            }
            self.note_consteval_only_ref(e, d);
        } else if matches!(n.kind(), DeclKind::Function | DeclKind::CXXMethod) {
            let at = self.eloc(e);
            self.exc_spec_needed(d, at);
            self.note_immediate_ref(e, d);
        } else if self.unevaluated == 0
            && matches!(n.kind(), DeclKind::Var | DeclKind::ParmVar | DeclKind::Binding)
        {
            self.note_potential_capture(e, d);
        }
        e
    }

    /// The text of a narrow string literal.
    pub(crate) fn string_text(&self, e: ExprId) -> Option<String> {
        let s = self.string_lit_of(e)?;
        let id = self.ast.exprs.string_lit(s);
        let (enc, bytes) = self.ast.strings.get(id);
        if !matches!(enc, StringEncoding::Ordinary | StringEncoding::Utf8) {
            return None;
        }
        let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
        Some(String::from_utf8_lossy(bytes).into_owned())
    }

    // -- literals ----------------------------------------------------------------

    /// Rejects a user-defined literal: M2 declares no literal operators
    /// ([over.literal]), so lookup always fails ([lex.ext]/2-8). The
    /// error is clang's, at the suffix `ud_loc`.
    pub(crate) fn reject_ud_suffix(
        &mut self,
        ud: Option<&str>,
        ud_loc: Loc,
        operand: UdOperand,
    ) -> bool {
        let Some(suffix) = ud else { return false };
        let op = format!("'operator\"\"{suffix}'");
        let msg = match operand {
            UdOperand::Integer | UdOperand::Float => {
                let ty = if operand == UdOperand::Integer {
                    QualType::ULONGLONG
                } else {
                    QualType::LONGDOUBLE
                };
                let raw = self.const_char_ptr(QualType::CHAR);
                self.diag(DiagId::sema_udl_no_operator_raw, ud_loc)
                    .arg(op)
                    .arg(self.ty_arg(ty))
                    .arg(self.ty_arg(raw))
            }
            UdOperand::Char(ty) => {
                self.diag(DiagId::sema_udl_no_operator, ud_loc).arg(op).arg(self.ty_arg(ty))
            }
            UdOperand::String(elem) => {
                let p = self.const_char_ptr(elem);
                let size = self.size_type();
                self.diag(DiagId::sema_udl_no_operator_string, ud_loc)
                    .arg(op)
                    .arg(self.ty_arg(p))
                    .arg(self.ty_arg(size))
            }
        };
        self.emit(msg);
        true
    }

    /// `const elem *`.
    pub(crate) fn const_char_ptr(&mut self, elem: QualType) -> QualType {
        let c = self.ast.types.qualify(elem, kiln_ast::types::Quals::CONST);
        self.ast.types.pointer_to(c)
    }

    /// An integer literal ([lex.icon]).
    pub fn act_on_integer_literal(
        &mut self,
        lit: &IntegerLiteral<'_>,
        loc: Loc,
        ud_loc: Loc,
    ) -> ExprId {
        if self.reject_ud_suffix(lit.ud_suffix, ud_loc, UdOperand::Integer) {
            return self.recover_err(&[], loc);
        }
        let radix = ast_radix(lit.radix);
        match literal::classify_integer_literal(self, lit) {
            Ok(t) => {
                if let Some(w) = &t.warning {
                    let d = w.diag(self, loc);
                    self.emit(d);
                }
                let e = if let Ok(v) = u64::try_from(t.value) {
                    self.builder().int_literal(t.ty, v, radix, loc)
                } else {
                    #[allow(clippy::cast_possible_wrap, reason = "bit pattern")]
                    let cv = self.int_const(t.ty, t.value as i128);
                    self.builder().wide_int_literal(t.ty, cv, radix, loc)
                };
                if lit.imaginary { self.imaginary_literal(e, loc) } else { e }
            }
            Err(err) => {
                let d = err.diag(self, loc);
                self.emit(d);
                self.recover_err(&[], loc)
            }
        }
    }

    /// A floating literal ([lex.fcon]).
    pub fn act_on_float_literal(
        &mut self,
        lit: &FloatLiteral<'_>,
        loc: Loc,
        ud_loc: Loc,
    ) -> ExprId {
        if self.reject_ud_suffix(lit.ud_suffix, ud_loc, UdOperand::Float) {
            return self.recover_err(&[], loc);
        }
        match literal::classify_float_literal(self, lit) {
            Ok(t) => {
                if let Some(w) = &t.warning {
                    let d = w.diag(self, loc);
                    self.emit(d);
                }
                let cv = literal::float_const(self, t.ty, t.bits);
                let e = self.builder().float_literal(t.ty, cv, loc);
                if lit.imaginary { self.imaginary_literal(e, loc) } else { e }
            }
            Err(err) => {
                let d = err.diag(self, loc);
                self.emit(d);
                self.recover_err(&[], loc)
            }
        }
    }

    /// The GNU imaginary constant of real literal `sub` (clang
    /// `ActOnNumericConstant`: `ext_imaginary_constant`).
    fn imaginary_literal(&mut self, sub: ExprId, loc: Loc) -> ExprId {
        self.error(DiagId::sema_imaginary_constant, loc);
        let ty = self.ety(sub);
        let cty = self.ast.types.complex(ty);
        self.builder().imaginary_literal(sub, cty, loc)
    }

    /// A character literal ([lex.ccon]).
    pub fn act_on_char_literal(&mut self, lit: &CharLiteral<'_>, loc: Loc, ud_loc: Loc) -> ExprId {
        if let Some(sfx) = lit.ud_suffix {
            let plain = CharLiteral { ud_suffix: None, ..lit.clone() };
            let e = self.act_on_char_literal(&plain, loc, ud_loc);
            return self.char_udl(e, sfx, loc, ud_loc);
        }
        let kind = match lit.encoding {
            Encoding::Ordinary => CharKind::Ascii,
            Encoding::Wide => CharKind::Wide,
            Encoding::Utf8 => CharKind::Utf8,
            Encoding::Utf16 => CharKind::Utf16,
            Encoding::Utf32 => CharKind::Utf32,
        };
        match literal::type_char_literal(self, lit) {
            Ok((ty, v)) => self.builder().char_literal(ty, kind, v, loc),
            Err(err) => {
                let d = err.diag(self, loc);
                self.emit(d);
                self.recover_err(&[], loc)
            }
        }
    }

    /// A string literal, concatenated from adjacent pieces
    /// ([lex.string]/7). `unevaluated` for unevaluated strings
    /// ([lex.string.uneval]: `static_assert` messages, `extern "C"`,
    /// attribute arguments).
    pub fn act_on_string_literal(
        &mut self,
        pieces: &[(StringLiteral<'_>, Loc)],
        unevaluated: bool,
        ud_loc: Loc,
    ) -> ExprId {
        let loc = pieces.first().map_or(Loc::INVALID, |p| p.1);
        if let Some(ud) = pieces.iter().find_map(|(p, _)| p.ud_suffix) {
            let enc = pieces
                .iter()
                .map(|(p, _)| p.encoding)
                .find(|&e| e != Encoding::Ordinary)
                .unwrap_or(Encoding::Ordinary);
            let elem = literal::string_literal_elem(self.char8(), enc);
            let plain: Vec<(StringLiteral<'_>, Loc)> = pieces
                .iter()
                .map(|(p, l)| (StringLiteral { ud_suffix: None, ..p.clone() }, *l))
                .collect();
            let e = self.act_on_string_literal(&plain, unevaluated, ud_loc);
            return self.string_udl(e, ud, elem, loc, ud_loc);
        }
        // The encoding: all the same, or ordinary pieces and one other.
        let mut enc = Encoding::Ordinary;
        let mut units = None::<u32>;
        for (p, l) in pieces {
            if p.encoding == Encoding::Ordinary {
                continue;
            }
            if enc != Encoding::Ordinary && enc != p.encoding {
                self.error(DiagId::sema_unsupported_string_concat, *l);
                return self.recover_err(&[], loc);
            }
            enc = p.encoding;
            units = Some(p.units.unit_bits());
        }
        let bits = units.unwrap_or(8);
        let mut out: Vec<u32> = Vec::new();
        for (p, _) in pieces {
            if p.encoding == Encoding::Ordinary && bits != 8 {
                // Re-encode the ordinary piece's UTF-8 in the wider form.
                let bytes: Vec<u8> =
                    p.units.iter().map(|u| u8::try_from(u).unwrap_or(b'?')).collect();
                for c in String::from_utf8_lossy(&bytes).chars() {
                    if bits == 16 {
                        out.extend(c.encode_utf16(&mut [0; 2]).iter().map(|&u| u32::from(u)));
                    } else {
                        out.push(u32::from(c));
                    }
                }
            } else {
                out.extend(p.units.iter());
            }
        }
        let count = out.len() as u64 + 1;
        let senc = match enc {
            Encoding::Ordinary => StringEncoding::Ordinary,
            Encoding::Utf8 => StringEncoding::Utf8,
            Encoding::Utf16 => StringEncoding::Utf16,
            Encoding::Utf32 => StringEncoding::Utf32,
            Encoding::Wide if bits == 16 => StringEncoding::Wide16,
            Encoding::Wide => StringEncoding::Wide32,
        };
        let mut bytes = Vec::with_capacity(out.len() * (bits as usize / 8));
        for u in &out {
            #[allow(clippy::cast_possible_truncation, reason = "code units fit their width")]
            match bits {
                8 => bytes.push(*u as u8),
                16 => bytes.extend_from_slice(&(*u as u16).to_le_bytes()),
                _ => bytes.extend_from_slice(&u.to_le_bytes()),
            }
        }
        let id = self.ast.strings.intern(senc, &bytes);
        let ty = literal::string_literal_type(self, enc, count);
        let kind = if unevaluated {
            StringKind::Unevaluated
        } else {
            match enc {
                Encoding::Ordinary => StringKind::Ordinary,
                Encoding::Wide => StringKind::Wide,
                Encoding::Utf8 => StringKind::Utf8,
                Encoding::Utf16 => StringKind::Utf16,
                Encoding::Utf32 => StringKind::Utf32,
            }
        };
        let _ = CodeUnits::U8(Vec::new());
        self.builder().string_literal(ty, kind, id, loc)
    }

    /// `true` / `false` ([lex.bool]).
    pub fn act_on_bool_literal(&mut self, value: bool, loc: Loc) -> ExprId {
        self.builder().bool_literal(QualType::BOOL, value, loc)
    }

    /// `nullptr` ([lex.nullptr]).
    pub fn act_on_nullptr(&mut self, loc: Loc) -> ExprId {
        self.builder().nullptr_literal(QualType::NULLPTR, loc)
    }

    /// `( e )` ([expr.prim.paren]).
    pub fn act_on_paren(&mut self, e: ExprId, lparen: Loc, rparen: Loc) -> ExprId {
        let x = self.builder().paren(e, lparen, rparen);
        if self.special.has_pending() {
            self.widen_pending_template_id(e, x);
        }
        x
    }

    // -- assignment checks -------------------------------------------------------

    /// [expr.assign]/1, [expr.pre.incr]: the operand must be a modifiable
    /// lvalue. Diagnoses and returns false otherwise.
    pub(crate) fn check_modifiable(&mut self, e: ExprId, loc: Loc) -> bool {
        self.check_shadowing_decl_modification(e, loc);
        let n = *self.ast.exprs.get(e);
        let t = n.ty();
        if self.ast.types.is_function(t) || self.ast.types.is_array(t) {
            let id = if self.ast.types.is_function(t) {
                DiagId::sema_non_object_not_modifiable
            } else {
                DiagId::sema_array_not_modifiable
            };
            let msg = self.diag(id, loc).arg(self.ty_arg(t));
            self.emit(msg);
            return false;
        }
        if n.value_category() != ValueCategory::LValue {
            // clang's `CM_LValueCast`: an explicit cast of an lvalue, for a
            // better diagnostic than the generic one (GCC's cast-as-lvalue).
            let inner = self.ignore_parens(e);
            if n.value_category() == ValueCategory::PRValue
                && self.ast.exprs.get(inner).kind().is_explicit_cast()
            {
                let sub = self.ignore_paren_imp_casts(self.ast.exprs.operand(inner));
                if self.ast.exprs.get(sub).value_category() == ValueCategory::LValue {
                    let at = self.begin_loc(inner);
                    self.error(DiagId::sema_lvalue_cast_not_supported, at);
                    return false;
                }
            }
            self.error(DiagId::sema_not_assignable, loc);
            return false;
        }
        if self.has_duplicate_vector_lanes(e) {
            self.error(DiagId::sema_duplicate_vector_components_not_mlvalue, loc);
            return false;
        }
        if self.ast.types.effective_quals(t).is_const() {
            let mut base = self.ignore_parens(e);
            // clang classifies a vector element through its vector.
            while self.ast.exprs.get(base).object_kind() == ObjectKind::VectorComponent
                && match self.ast.exprs.get(base).kind() {
                    ExprKind::ArraySubscriptExpr => true,
                    ExprKind::ExtVectorElementExpr => {
                        !self.ast.exprs.get(base).has(ExprFlags::ARROW)
                    }
                    _ => false,
                }
            {
                let v = ExprId::new(self.ast.exprs.get(base).a());
                base = self.ignore_parens(v);
            }
            if self.ast.exprs.get(base).kind() == ExprKind::DeclRefExpr
                && !self.contract_ctx.is_empty()
                && let d = self.ast.exprs.decl(base)
                && self.contract_made_const(d)
            {
                // [expr.prim.id.unqual]/6-7 (spec/contracts.md §5.2).
                let msg = self.diag(DiagId::sema_contract_const_modify, loc).arg(self.decl_name(d));
                self.emit(msg);
            } else if !self.contract_ctx.is_empty()
                && let Some(field) = self.contract_this_member(base)
            {
                // [expr.prim.this]/2 (P2900R14): `this` points to const
                // in a contract predicate (spec/contracts.md §5.2).
                let msg =
                    self.diag(DiagId::sema_contract_const_modify, loc).arg(self.decl_name(field));
                self.emit(msg);
            } else if self.ast.exprs.get(base).kind() == ExprKind::DeclRefExpr
                && self.ast.exprs.get(base).has(ExprFlags::REFERS_TO_ENCLOSING)
                && let d = self.ast.exprs.decl(base)
                && !self
                    .ast
                    .types
                    .effective_quals(self.ast.decls.get(d).ty().unwrap_or(t))
                    .is_const()
            {
                // [expr.prim.lambda.closure]/5: the copy is a member of
                // the const closure object.
                self.error(DiagId::sema_assign_captured_by_copy, loc);
            } else if self.ast.exprs.get(base).kind() == ExprKind::DeclRefExpr {
                let d = self.ast.exprs.decl(base);
                let dt = self.ast.decls.get(d).ty().unwrap_or(t);
                let msg = self
                    .diag(DiagId::sema_assign_const_var, loc)
                    .arg(self.decl_name(d))
                    .arg(self.ty_arg(dt));
                let note = self
                    .diag(DiagId::note_var_declared_const, self.ast.decls.get(d).loc())
                    .arg(self.decl_name(d));
                self.emit(msg.with_note(note));
            } else if self.diagnose_const_return(e, loc) {
            } else if !self.diagnose_const_this_member(e, loc) {
                self.error(DiagId::sema_read_only_not_assignable, loc);
            }
            return false;
        }
        true
    }

    /// clang `DiagnoseConstAssignment`'s `ConstFunction`: the modified
    /// operand is a call to a function whose return type is
    /// const-qualified (after its reference), [expr.assign]/1. The note
    /// is at the return type's first type-specifier.
    fn diagnose_const_return(&mut self, e: ExprId, loc: Loc) -> bool {
        let mut c = self.ignore_paren_imp_casts(e);
        // Through `.` member accesses of non-const members (clang walks
        // the `MemberExpr` bases: `get().m = 1`).
        while self.ast.exprs.get(c).kind() == ExprKind::MemberExpr
            && !self.ast.exprs.get(c).has(ExprFlags::ARROW)
        {
            let field = self.ast.exprs.decl(c);
            let declared = self.ast.decls.get(field).ty().unwrap_or(QualType::ERROR);
            if self.ast.types.effective_quals(declared).is_const() {
                return false;
            }
            c = self.ignore_paren_imp_casts(self.ast.exprs.operand(c));
        }
        if !matches!(
            self.ast.exprs.get(c).kind(),
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr
        ) {
            return false;
        }
        let Some(f) = self.callee_decl(c) else { return false };
        let n = self.ast.decls.get(f);
        if !n.kind().is_function() {
            return false;
        }
        let fty = n.ty().unwrap_or(QualType::ERROR);
        let Some(ret) = self.ast.types.proto(fty).map(kiln_ast::types::FuncProto::ret) else {
            return false;
        };
        let pointee = match self.ast.types.canonical_view(ret) {
            TypeView::LValueRef(p) | TypeView::RValueRef(p) => p,
            _ => ret,
        };
        if !self.ast.types.effective_quals(pointee).is_const() {
            return false;
        }
        let name = format!("'{}'", self.name_for_diagnostic(f));
        let at = self.ret_type_loc(f);
        let note = self
            .diag(DiagId::note_function_returns_const, at)
            .arg(name.clone())
            .arg(self.ty_arg(ret));
        let msg = self.diag(DiagId::sema_assign_const_function_ret, loc).arg(name);
        self.emit(msg.with_note(note));
        true
    }

    /// Where function `f`'s return type was written (its first
    /// type-specifier, clang `getReturnTypeSourceRange`), for a function
    /// whose return type is dependent or const-qualified; else its name.
    fn ret_type_loc(&self, f: DeclId) -> Loc {
        let mut d = f;
        for _ in 0..8 {
            if let Some(&l) = self.type_spec_locs.get(&d) {
                return l;
            }
            let from = self.ast.instantiated_from(d).or_else(|| {
                self.ast.spec_info(d).and_then(|(_, i)| self.templated_function(i.from))
            });
            match from {
                Some(p) if p != d => d = p,
                _ => break,
            }
        }
        self.ast.decls.get(f).loc()
    }

    fn mark_bitfield_if(&mut self, result: ExprId, from: ExprId) {
        if !self.ast.exprs.get(result).value_category().is_glvalue() {
            return;
        }
        if self.is_bitfield(from) {
            self.ast.exprs.mark_bitfield(result);
        } else if self.ast.exprs.get(from).object_kind() == ObjectKind::VectorComponent {
            self.ast.exprs.mark_vector_component(result);
        }
    }

    // -- unary operators ---------------------------------------------------------

    /// A prefix unary operator, or postfix `++`/`--` ([expr.unary.op],
    /// [expr.pre.incr], [expr.post.incr]).
    pub fn act_on_unary(&mut self, op: UnaryOp, e: ExprId, loc: Loc) -> ExprId {
        if self.is_err_expr(e) {
            return self.recover_err(&[e], loc);
        }
        let e = if op == UnaryOp::AddrOf {
            self.undo_dependent_implicit_member(e)
        } else {
            // [expr.ref]/7.3.2: clang's `BuildUnaryOp` recovers a bound
            // member function operand of any operator but `&`.
            let e = self.recover_bound_member(e);
            if self.is_err_expr(e) {
                return self.recover_err(&[e], loc);
            }
            e
        };
        let t0 = self.ety(e);
        if self.is_dependent_overloadable(t0) {
            // `&C::m` of a member of dependent type: formed at
            // instantiation, where the qualified name is rebuilt.
            if op == UnaryOp::AddrOf && self.member_refs.get(&e).is_some_and(|r| r.qualified) {
                self.member_refs.remove(&e);
            }
            return self.builder().dependent_unary(op, e, loc);
        }
        // `&C::m` of a non-static member forms a pointer to member
        // ([expr.unary.op]/3) whatever the type of `m`: it never calls an
        // `operator&` (clang's `isQualifiedMemberAccess` in
        // `BuildUnaryOp`). Looking one up would also complete the class
        // of `m` and its associated classes, an instantiation run while
        // the name is still pending.
        let x = if op == UnaryOp::AddrOf && self.member_refs.get(&e).is_some_and(|r| r.qualified) {
            self.builtin_unary(op, e, loc)
        } else {
            match self.try_overloaded_operator(OperForm::Unary(op), &[e], &[], loc, Loc::INVALID) {
                OperResult::Done(x) => x,
                OperResult::Converted(v) => self.builtin_unary(op, v[0], loc),
                OperResult::Builtin | OperResult::BuiltinNotes(_) => self.builtin_unary(op, e, loc),
            }
        };
        if op == UnaryOp::AddrOf && self.special.has_pending() {
            self.widen_pending_template_id(e, x);
        }
        x
    }

    /// The built-in unary operator `op` ([expr.unary.op], [expr.pre.incr],
    /// [expr.post.incr]).
    pub(crate) fn builtin_unary(&mut self, op: UnaryOp, e: ExprId, loc: Loc) -> ExprId {
        let e = if op == UnaryOp::AddrOf { e } else { self.resolve_single_overload_candidate(e) };
        let t0 = self.ety(e);
        if op != UnaryOp::AddrOf && self.check_overload_use(e) {
            return self.recover_err(&[e], loc);
        }
        let bad = |s: &mut Self, e: ExprId| {
            let msg = s.diag(DiagId::sema_invalid_unary_operand, loc).arg(s.ty_arg(t0));
            s.emit(msg);
            s.recover_err(&[e], loc)
        };
        // clang `CreateBuiltinUnaryOp`: SVE vectors allow `+`, `-` and,
        // with an integer element, `~`.
        if self.is_sve_vector(t0)
            && (matches!(op, UnaryOp::Plus | UnaryOp::Minus)
                || (op == UnaryOp::Not && self.has_integer_repr(t0)))
        {
            let x = conv::decay(self, e);
            let ty = self.ety(x);
            let flags =
                if op == UnaryOp::Minus { ExprFlags::NONE } else { ExprFlags::CANNOT_OVERFLOW };
            return self.builder().unary(op, x, ty, loc, flags);
        }
        if self.is_vector_type(t0)
            && matches!(op, UnaryOp::Plus | UnaryOp::Minus | UnaryOp::Not | UnaryOp::LNot)
        {
            return match self.vector_unary(op, e, loc) {
                Some(x) => x,
                None => bad(self, e),
            };
        }
        // An `_Atomic(T)` operand is checked as its `T` (the lvalue
        // conversion strips it, clang `DefaultLvalueConversion`).
        let tv = self.ast.types.as_atomic(t0).unwrap_or(t0);
        match op {
            UnaryOp::Plus | UnaryOp::Minus => {
                let ok = self.is_arith_operand(tv)
                    || (op == UnaryOp::Plus
                        && (self.ast.types.is_pointer(tv) || self.ast.types.is_array(t0)));
                if !ok {
                    return bad(self, e);
                }
                let x = conv::usual_unary_conversions(self, e);
                let ty = self.ety(x);
                let flags =
                    if op == UnaryOp::Plus || self.narrower_than_int(conv::strip_cv(self, tv)) {
                        ExprFlags::CANNOT_OVERFLOW
                    } else {
                        ExprFlags::NONE
                    };
                self.builder().unary(op, x, ty, loc, flags)
            }
            UnaryOp::Not => {
                // GNU: `~` of a complex operand is its conjugate.
                let complex = self.ast.types.as_complex(tv).is_some();
                if !(complex || self.ast.types.is_integral(tv) || conv::is_unscoped_enum(self, tv))
                {
                    return bad(self, e);
                }
                let x = conv::usual_unary_conversions(self, e);
                let ty = self.ety(x);
                self.builder().unary(op, x, ty, loc, ExprFlags::CANNOT_OVERFLOW)
            }
            UnaryOp::LNot => match conv::contextually_convert_to_bool(self, e) {
                Ok(x) => {
                    self.builder().unary(op, x, QualType::BOOL, loc, ExprFlags::CANNOT_OVERFLOW)
                }
                Err(_) => bad(self, e),
            },
            UnaryOp::Deref => {
                // A type-dependent operand that is not of an overloadable
                // type (`*a` for `T (&a)[N]`): the result type is dependent
                // (clang's `CheckIndirectionOperand`).
                if self.ast.types.is_dependent(t0) {
                    return self.builder().dependent_unary(op, e, loc);
                }
                let x = conv::decay(self, e);
                let pt = self.ety(x);
                let Some(p) =
                    self.ast.types.is_pointer(pt).then(|| self.pointee_sugared(pt)).flatten()
                else {
                    let msg = self
                        .diag(DiagId::sema_indirection_requires_pointer, loc)
                        .arg(self.ty_arg(t0));
                    self.emit(msg);
                    return self.recover_err(&[e], loc);
                };
                if self.ast.types.is_void(p) {
                    let msg =
                        self.diag(DiagId::sema_indirection_incomplete, loc).arg(self.ty_arg(pt));
                    self.emit(msg);
                    return self.recover_err(&[x], loc);
                }
                self.builder().unary(op, x, p, loc, ExprFlags::CANNOT_OVERFLOW)
            }
            UnaryOp::AddrOf => self.address_of(e, loc),
            UnaryOp::Real | UnaryOp::Imag => self.real_imag(op, e, loc),
            UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec => {
                self.inc_dec(op, e, loc)
            }
            _ => {
                self.unsupported(&format!("unary operator '{}'", op.spelling()), loc);
                self.recover_err(&[e], loc)
            }
        }
    }

    fn address_of(&mut self, e: ExprId, loc: Loc) -> ExprId {
        self.record_modified_param(e);
        if let Some(x) = self.member_overload(e) {
            if x == e
                && let Some(ule) = self.member_overload_as_lookup(x)
            {
                // `&C::f` in a member function of `C`: the overload set
                // a pointer to member is taken from ([expr.unary.op]/3).
                return self.builder().unary(
                    UnaryOp::AddrOf,
                    ule,
                    QualType::OVERLOAD,
                    loc,
                    ExprFlags::CANNOT_OVERFLOW,
                );
            }
            if x == e && self.implicit_member_operand(e).is_some() {
                // `&f` of an implicit member set: a lookup of the name
                // (clang's `ActOnIdExpression` for an address-of operand),
                // from which the target selects a static member or a
                // static member template's specialization ([over.over]/1,
                // /3; spec/sema-overload.md §2.3). `&(f)` stays a member
                // access.
                return self.builder().unary(
                    UnaryOp::AddrOf,
                    e,
                    QualType::OVERLOAD,
                    loc,
                    ExprFlags::CANNOT_OVERFLOW,
                );
            }
            // `&b.f<args>` naming one static member function
            // specialization is that function's address ([over.over]/3;
            // clang's `ResolveSingleFunctionTemplateSpecialization`).
            if let Some((d, f)) = self.single_member_template_spec(x)
                && self.is_static_method(d)
                && let Some(m) = self.ovl.members.remove(&x)
            {
                let at = self.ast.exprs.get(x).loc();
                let r = self.finish_member_winner(&m, d, f, at);
                let r = self.rebuild_parens(e, r);
                return self.address_of(r, loc);
            }
            // [expr.unary.op]/4: `&b.f` is not a pointer to member.
            self.error(DiagId::sema_nonconst_member_fn_ptr, loc);
            return self.recover_err(&[e], loc);
        }
        // `&(C::f)` of an overload set of instance members without an
        // object: as for one member function below.
        let x = self.ignore_parens(e);
        if x != e && self.ast.exprs.get(x).kind() == ExprKind::UnresolvedLookupExpr {
            let decls = self.ast.unresolved_decls(x);
            let instance = |s: &Self, d: DeclId| {
                let f = s.templated_or_self(d);
                s.kind_of(f) == DeclKind::CXXMethod
                    && !s.is_static_method(f)
                    && !s.has_explicit_object(f)
            };
            if !decls.is_empty() && decls.iter().all(|&d| instance(self, d)) {
                self.error(DiagId::sema_member_call_without_object, self.eloc(x));
                return self.recover_err(&[e], loc);
            }
        }
        if self.overload_operand(e).is_some_and(|(_, addr)| !addr) {
            // [over.over]: `&f` of an overload set is resolved by its target.
            return self.builder().unary(
                UnaryOp::AddrOf,
                e,
                QualType::OVERLOAD,
                loc,
                ExprFlags::CANNOT_OVERFLOW,
            );
        }
        // `&(C::f)` is no pointer to member ([expr.unary.op]/4): without
        // an object, its member name is "without an object" (clang's
        // `DiagnoseInstanceReference`, at the name, before validity).
        if x != e
            && let Some(r) = self.member_refs.get(&x).copied()
            && r.error
            && self.kind_of(r.decl) == DeclKind::CXXMethod
        {
            self.member_refs.remove(&x);
            self.error(DiagId::sema_member_call_without_object, self.eloc(x));
            return self.recover_err(&[e], loc);
        }
        if self.names_invalid_function(e) && (x == e || !self.member_refs.contains_key(&x)) {
            self.member_refs.remove(&x);
            return self.recover_err(&[e], loc);
        }
        if let Some(mp) = self.member_pointer_of(e, loc) {
            return mp;
        }
        // [expr.unary.op]/4: neither is `&(C::f)` with an implicit object,
        // `&(f)` nor `&b.f` (clang's `err_parens_pointer_member_function`
        // for a bound member function).
        if self.ety(e) == QualType::BOUND_MEMBER
            && self.ast.exprs.get(x).kind() == ExprKind::MemberExpr
        {
            self.error(DiagId::sema_nonconst_member_fn_ptr, loc);
            return self.recover_err(&[e], loc);
        }
        let Some(t) = self.check_addressable(e, loc) else {
            return self.recover_err(&[e], loc);
        };
        self.check_address_of_packed_member(e);
        let ty = self.ast.types.pointer_to(t);
        self.builder().unary(UnaryOp::AddrOf, e, ty, loc, ExprFlags::CANNOT_OVERFLOW)
    }

    /// clang's `CheckAddressOfOperand` for an operand that is not a
    /// member-pointer name ([expr.unary.op]/3): the operand's type, or
    /// `None` after reporting a builtin function name, a bit-field or an
    /// rvalue at `loc`.
    pub(crate) fn check_addressable(&mut self, e: ExprId, loc: Loc) -> Option<QualType> {
        if self.check_builtin_fn_use(e) {
            return None;
        }
        let n = *self.ast.exprs.get(e);
        if n.object_kind() == ObjectKind::BitField {
            self.error(DiagId::sema_addrof_bitfield, loc);
            return None;
        }
        if n.object_kind() == ObjectKind::VectorComponent {
            self.error(DiagId::sema_addrof_vector_element, loc);
            return None;
        }
        if n.value_category() != ValueCategory::LValue {
            let msg = self.diag(DiagId::sema_addrof_rvalue, loc).arg(self.ty_arg(n.ty()));
            self.emit(msg);
            return None;
        }
        Some(n.ty())
    }

    fn inc_dec(&mut self, op: UnaryOp, e: ExprId, loc: Loc) -> ExprId {
        // clang `CheckIncrementDecrementOperand`: an `_Atomic` operand is
        // checked, and yields, its value type.
        let operand_t = self.ety(e);
        let atomic = self.ast.types.as_atomic(operand_t);
        let t = atomic.unwrap_or(operand_t);
        let inc = matches!(op, UnaryOp::PreInc | UnaryOp::PostInc);
        if self.ast.types.builtin(t) == Some(BuiltinType::Bool) {
            // clang: decrementing is always an error; incrementing is a
            // DefaultError extension (`-Wno-increment-bool` accepts it).
            if !inc {
                let msg = self.diag(DiagId::sema_decrement_bool, loc);
                self.emit(msg);
                return self.recover_err(&[e], loc);
            }
            let msg = self.diag(DiagId::sema_increment_bool, loc);
            self.emit(msg);
        }
        if self.ast.types.as_enum(t).is_some() {
            let msg = self
                .diag(DiagId::sema_incdec_enum, loc)
                .arg(if inc { "0" } else { "1" })
                .arg(self.ty_arg(t));
            self.emit(msg);
            return self.recover_err(&[e], loc);
        }
        if self.ast.types.is_pointer(t) {
            if !self.check_pointer_arith(t, loc) {
                return self.recover_err(&[e], loc);
            }
        } else if self.ast.types.as_complex(t).is_some() {
            // clang 19 `ext_increment_complex`: every complex type.
            let msg = self.diag(DiagId::sema_increment_complex, loc).arg(op.spelling());
            self.emit(msg);
        } else if !self.ast.types.is_arithmetic(t) {
            let msg = self
                .diag(DiagId::sema_incdec_invalid, loc)
                .arg(if inc { "0" } else { "1" })
                .arg(self.ty_arg(t));
            self.emit(msg);
            return self.recover_err(&[e], loc);
        }
        if !self.check_modifiable(e, loc) {
            return self.recover_err(&[e], loc);
        }
        self.check_volatile_inc_dec(inc, operand_t, loc);
        let flags =
            if self.narrower_than_int(t) { ExprFlags::CANNOT_OVERFLOW } else { ExprFlags::NONE };
        let ty = if op.is_postfix() || atomic.is_some() { conv::strip_cv(self, t) } else { t };
        let r = self.builder().unary(op, e, ty, loc, flags);
        self.mark_bitfield_if(r, e);
        r
    }

    /// Pointer arithmetic needs a pointer to a complete object type
    /// ([expr.add]/1); a `void` or function pointee is an error in C++
    /// (a GNU extension only in C: `sema_arith_void_pointer`).
    fn check_pointer_arith(&mut self, pt: QualType, loc: Loc) -> bool {
        let Some(p) = self.pointee_sugared(pt) else { return true };
        if self.ast.types.is_void(p) {
            let msg = self.diag(DiagId::sema_arith_void_pointer_cxx, loc).arg("0");
            self.emit(msg);
            return false;
        }
        if self.ast.types.is_function(p) {
            let msg = self
                .diag(DiagId::sema_arith_function_pointer_cxx, loc)
                .arg("0")
                .arg(self.ty_arg(p))
                .arg("0")
                .arg("");
            self.emit(msg);
            return false;
        }
        if self.ast.types.is_sizeless(p) {
            // clang `err_typecheck_arithmetic_incomplete_or_sizeless_type`.
            let msg = self.diag(DiagId::sema_arith_sizeless_pointee, loc).arg(self.ty_arg(p));
            self.emit(msg);
            return false;
        }
        if !self.require_complete(p, loc) {
            let msg = self.diag(DiagId::sema_arith_incomplete_pointee, loc).arg(self.ty_arg(p));
            self.emit(self.incomplete_note(msg, p));
            return false;
        }
        true
    }

    /// [`Self::check_pointer_arith`] for the pointer difference `lt - rt`
    /// (clang `checkArithmeticBinOpPointerOperands`): a `void` pointee on
    /// one side is diagnosed for that side, on both as `pointers to void`;
    /// likewise a function pointee, the two-pointer form naming the second
    /// pointee only when the pointer types differ (`pointers to function
    /// types 'void ()' and 'void (int)'`); then an incomplete pointee,
    /// left first.
    fn check_pointer_pair_arith(&mut self, lt: QualType, rt: QualType, loc: Loc) -> bool {
        let (Some(a), Some(b)) = (self.pointee_sugared(lt), self.pointee_sugared(rt)) else {
            return true;
        };
        let t = &self.ast.types;
        let pick = |l: bool, r: bool| match (l, r) {
            (true, true) => None,
            (true, false) => Some(lt),
            _ => Some(rt),
        };
        let (av, bv) = (t.is_void(a), t.is_void(b));
        if av || bv {
            let Some(one) = pick(av, bv) else {
                let msg = self.diag(DiagId::sema_arith_void_pointer_cxx, loc).arg("1");
                self.emit(msg);
                return false;
            };
            return self.check_pointer_arith(one, loc);
        }
        let (af, bf) = (t.is_function(a), t.is_function(b));
        if af || bf {
            let Some(one) = pick(af, bf) else {
                let differ = !self.ast.types.same_unqualified(lt, rt);
                let msg = self
                    .diag(DiagId::sema_arith_function_pointer_cxx, loc)
                    .arg("1")
                    .arg(self.ty_arg(a))
                    .arg(if differ { "1" } else { "0" })
                    .arg(self.ty_arg(b));
                self.emit(msg);
                return false;
            };
            return self.check_pointer_arith(one, loc);
        }
        self.check_pointer_arith(lt, loc) && self.check_pointer_arith(rt, loc)
    }

    // -- binary operators --------------------------------------------------------

    pub(crate) fn invalid_operands(&mut self, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        let (a, b) = self.ty_arg_pair(self.ety(l), self.ety(r));
        let msg = self
            .diag(DiagId::sema_invalid_operands, loc)
            .arg(a)
            .arg(b)
            .range(self.expr_range(l))
            .range(self.expr_range(r));
        self.emit(msg);
        self.recover_err(&[l, r], loc)
    }

    /// Reports at `loc` (the operator) the warning the last usual
    /// arithmetic conversions left, if any ([`Sema::arith_warning`]).
    pub(crate) fn report_arith_warning(&mut self, loc: Loc) {
        if let Some(w) = self.arith_warning.take() {
            self.emit_conv_error(w, loc);
        }
    }

    pub(crate) fn conv_failed(
        &mut self,
        err: &ConvError,
        l: ExprId,
        r: ExprId,
        loc: Loc,
    ) -> ExprId {
        self.emit_conv_error(err.clone(), loc);
        self.recover_err(&[l, r], loc)
    }

    /// A binary operator ([expr.mul] .. [expr.comma]).
    pub fn act_on_binary(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        if self.is_err_expr(l) || self.is_err_expr(r) {
            return self.recover_err(&[l, r], loc);
        }
        if self.expr_is_type_dependent(l) || self.expr_is_type_dependent(r) {
            return self.builder().dependent_binary(op, l, r, loc);
        }
        self.binary_operator(op, l, r, None, loc)
    }

    /// A binary operator on non-dependent operands: an overloaded
    /// operator if an operand has class or enumeration type
    /// ([over.match.oper]), else the built-in one. `braced`: the braced
    /// right operand of an assignment ([expr.assign]/8).
    pub(crate) fn binary_operator(
        &mut self,
        op: BinaryOp,
        l: ExprId,
        r: ExprId,
        braced: Option<InitList>,
        loc: Loc,
    ) -> ExprId {
        // [expr.ref]/7.3.2: a bound member function operand is recovered
        // before overload resolution (clang's `BuildBinOp` runs
        // `CheckPlaceholderExpr` on each non-overload placeholder).
        if !matches!(op, BinaryOp::PtrMemD | BinaryOp::PtrMemI)
            && [l, r].iter().any(|&x| self.ety(x) == QualType::BOUND_MEMBER)
        {
            let (l, r) = (self.recover_bound_member(l), self.recover_bound_member(r));
            if self.is_err_expr(l) || self.is_err_expr(r) {
                return self.recover_err(&[l, r], loc);
            }
            return self.binary_operator(op, l, r, braced, loc);
        }
        let br = [None, braced];
        match self.try_overloaded_operator(OperForm::Binary(op), &[l, r], &br, loc, Loc::INVALID) {
            OperResult::Done(e) => e,
            OperResult::Builtin => self.builtin_binary(op, l, r, loc),
            OperResult::Converted(v) => self.builtin_binary(op, v[0], v[1], loc),
            OperResult::BuiltinNotes(notes) => {
                *self.trailing_notes.borrow_mut() = notes;
                let e = self.builtin_binary(op, l, r, loc);
                self.trailing_notes.borrow_mut().clear();
                e
            }
        }
    }

    /// `-Wconstant-logical-operand` ([expr.log.and], [expr.log.or]; clang
    /// `CheckLogicalOperands`): a non-`bool` integer left operand with a
    /// right operand that folds to an integer probably meant `&` / `|`.
    /// In C++ any folded value warns unless the right operand is `bool`
    /// or written by a macro; otherwise only a value other than 0 or 1.
    /// Not for an operator spelled in a macro or in an instantiation; the
    /// caller skips it when an enumerator operand already drew
    /// `-Wint-in-bool-context` (`diagnose_enum_constant_in_bool_context`).
    fn check_constant_logical_operand(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) {
        let (lt, rt) = (self.ety(l), self.ety(r));
        let int_ty = |s: &Self, t: QualType| conv::is_integral_or_unscoped_enum(s, t);
        if !int_ty(self, lt)
            || self.ast.types.builtin(lt) == Some(BuiltinType::Bool)
            || !int_ty(self, rt)
            || self.expr_is_value_dependent(r)
            || self.diags.sm().is_macro_loc(loc)
            || !self.inst.stack.is_empty()
        {
            return;
        }
        let Some(v) = self.fold_rvalue_int(r) else { return };
        if (self.ast.types.builtin(rt) == Some(BuiltinType::Bool)
            || self.diags.sm().is_macro_loc(self.eloc(r)))
            && (v == 0 || v == 1)
        {
            return;
        }
        let and = op == BinaryOp::LAnd;
        let (text, bit) = if and { ("&&", "&") } else { ("||", "|") };
        let op_span = kiln_source::Span::new(loc, loc.with_offset(2));
        // Fix-its are file edits: one with an end inside a macro expansion
        // is dropped (`macro_fixit.rs`).
        let mut change = self.diag(DiagId::note_logical_instead_of_bitwise_change_operator, loc);
        change = change.arg(bit);
        for f in self.file_fixits(vec![kiln_diag::FixIt::replace(op_span, bit)]) {
            change = change.fixit(f);
        }
        let mut d = self
            .diag(DiagId::sema_constant_logical_operand, loc)
            .arg(text)
            .range(self.expr_range(r))
            .with_note(change);
        if and {
            let l_end = self.expr_end_loc(l);
            let r_end = self.expr_range(r).hi;
            let mut n = self.diag(DiagId::note_logical_instead_of_bitwise_remove_constant, loc);
            if let Some(from) = self.file_token_end(l_end) {
                let rm = kiln_diag::FixIt::remove(kiln_source::Span::new(from, r_end));
                for f in self.file_fixits(vec![rm]) {
                    n = n.fixit(f);
                }
            }
            d = d.with_note(n);
        }
        self.emit(d);
    }

    /// The built-in comma operator ([expr.comma]).
    fn comma_binary(&mut self, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        self.diagnose_comma_lhs(l);
        self.discard_assignment_ref(l);
        // clang `CheckCommaOperands`: the left operand is a
        // discarded-value expression ([expr.comma]/1).
        let l = self.ignored_value_conversions(l);
        let ty = self.ety(r);
        let res = self.builder().binary(BinaryOp::Comma, l, r, ty, loc);
        self.mark_bitfield_if(res, r);
        res
    }

    /// The built-in `&&` / `||` ([expr.log.and], [expr.log.or]).
    fn logical_binary(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        if !self.diagnose_enum_constant_in_bool_context(l, r, loc) {
            self.check_constant_logical_operand(op, l, r, loc);
        }
        // clang `CheckLogicalOperands`: the first operand that
        // does not convert is diagnosed at its start.
        let conv_op = |s: &mut Self, e: ExprId| {
            conv::contextually_convert_to_bool(s, e).map_err(|err| {
                let at = s.begin_loc(e);
                s.emit(err.diag_at(s, at));
            })
        };
        let Ok(l2) = conv_op(self, l) else { return self.invalid_operands(l, r, loc) };
        let Ok(r2) = conv_op(self, r) else { return self.invalid_operands(l, r, loc) };
        self.builder().binary(op, l2, r2, QualType::BOOL, loc)
    }

    /// The built-in binary operator `op` ([expr.mul] .. [expr.comma]).
    pub(crate) fn builtin_binary(
        &mut self,
        op: BinaryOp,
        l: ExprId,
        r: ExprId,
        loc: Loc,
    ) -> ExprId {
        // [over.over]/1: an overload set needs a target; only an
        // assignment's right operand has one.
        let l = self.resolve_single_overload_candidate(l);
        let r = if op == BinaryOp::Assign { r } else { self.resolve_single_overload_candidate(r) };
        if self.check_overload_use(l) || (op != BinaryOp::Assign && self.check_overload_use(r)) {
            return self.recover_err(&[l, r], loc);
        }
        self.check_sve_operands([l, r], loc);
        // [expr.ref]/7.3.2: a bound member function operand can only be
        // called; clang's `CheckPlaceholderExpr` rejects the operator.
        if !matches!(op, BinaryOp::PtrMemD | BinaryOp::PtrMemI)
            && let Some(&x) =
                [l, r].iter().find(|&&x| self.ast.exprs.get(x).ty() == QualType::BOUND_MEMBER)
        {
            self.diagnose_bound_member(x);
            return self.recover_err(&[l, r], loc);
        }
        self.check_arithmetic_null(op, l, r, loc);
        if !matches!(op, BinaryOp::Assign | BinaryOp::Comma | BinaryOp::PtrMemD | BinaryOp::PtrMemI)
            && (self.is_vector_type(self.ety(l)) || self.is_vector_type(self.ety(r)))
        {
            return self.vector_binary(op, l, r, loc);
        }
        if !matches!(
            op,
            BinaryOp::Assign
                | BinaryOp::Comma
                | BinaryOp::PtrMemD
                | BinaryOp::PtrMemI
                | BinaryOp::LAnd
                | BinaryOp::LOr
        ) && (self.is_sve_vector(self.ety(l)) || self.is_sve_vector(self.ety(r)))
        {
            return self.sve_binary(op, l, r, loc);
        }
        match op {
            BinaryOp::Assign => self.assign(l, r, loc),
            _ if op.is_compound_assignment() => self.compound_assign(op, l, r, loc),
            BinaryOp::Comma => self.comma_binary(l, r, loc),
            BinaryOp::LAnd | BinaryOp::LOr => self.logical_binary(op, l, r, loc),
            BinaryOp::Add | BinaryOp::Sub => self.additive(op, l, r, loc),
            BinaryOp::Shl | BinaryOp::Shr => {
                let (lt, rt) = (self.value_ty(l), self.value_ty(r));
                let int_like = |s: &Self, t: QualType| {
                    s.ast.types.is_integral(t) || conv::is_unscoped_enum(s, t)
                };
                if !int_like(self, lt) || !int_like(self, rt) {
                    return self.invalid_operands(l, r, loc);
                }
                let l2 = conv::usual_unary_conversions(self, l);
                let r2 = conv::usual_unary_conversions(self, r);
                let ty = self.ety(l2);
                self.diagnose_shift_count(r2, ty, loc);
                self.builder().binary(op, l2, r2, ty, loc)
            }
            BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::And
            | BinaryOp::Xor
            | BinaryOp::Or => {
                let kind = if matches!(op, BinaryOp::And | BinaryOp::Xor | BinaryOp::Or) {
                    ArithConvKind::BitwiseOp
                } else {
                    ArithConvKind::Arithmetic
                };
                let (lt, rt) = (self.value_ty(l), self.value_ty(r));
                if !self.is_arith_operand(lt) || !self.is_arith_operand(rt) {
                    return self.invalid_operands(l, r, loc);
                }
                match conv::usual_arithmetic_conversions_kind(self, l, r, kind) {
                    Ok((l2, r2, ty)) => {
                        self.report_arith_warning(loc);
                        let needs_int = !matches!(op, BinaryOp::Mul | BinaryOp::Div);
                        if needs_int && !self.ast.types.is_integral(ty) {
                            return self.invalid_operands(l, r, loc);
                        }
                        if matches!(op, BinaryOp::Div | BinaryOp::Rem) {
                            self.diagnose_division_by_zero(op, r2, loc);
                        }
                        self.builder().binary(op, l2, r2, ty, loc)
                    }
                    Err(err) => self.conv_failed(&err, l, r, loc),
                }
            }
            BinaryOp::LT
            | BinaryOp::GT
            | BinaryOp::LE
            | BinaryOp::GE
            | BinaryOp::EQ
            | BinaryOp::NE => self.comparison(op, l, r, loc),
            BinaryOp::Cmp => self.builtin_three_way(l, r, loc),
            BinaryOp::PtrMemD | BinaryOp::PtrMemI => self.act_on_ptr_mem(op, l, r, loc),
            _ => {
                self.unsupported(&format!("operator '{}'", op.spelling()), loc);
                self.recover_err(&[l, r], loc)
            }
        }
    }

    /// clang's `-Wdivision-by-zero`: an integer divisor that folds to 0
    /// ([expr.mul]/4), a run-time behavior diagnostic
    /// (`DiagRuntimeBehavior` anchored at the divisor).
    fn diagnose_division_by_zero(&mut self, op: BinaryOp, r: ExprId, loc: Loc) {
        if self.runtime_diag_quiet() || !self.ast.types.is_integral(self.ety(r)) {
            return;
        }
        // A value-dependent divisor (`sizeof(T)`) has no value yet.
        if self.expr_is_value_dependent(r) {
            return;
        }
        if self.fold_int(r) == Some(0) {
            let what = if matches!(op, BinaryOp::Div | BinaryOp::DivAssign) {
                "division"
            } else {
                "remainder"
            };
            let msg =
                self.diag(DiagId::sema_division_by_zero, loc).arg(what).range(self.expr_range(r));
            self.diag_if_reachable(msg, r);
        }
    }

    /// clang's `-Wshift-count-negative`/`-Wshift-count-overflow`: a
    /// constant shift count outside `[0, width)` of the promoted left
    /// operand type `lty` ([expr.shift]/1), a run-time behavior
    /// diagnostic (clang `DiagnoseBadShiftValues`).
    fn diagnose_shift_count(&mut self, r: ExprId, lty: QualType, loc: Loc) {
        if self.runtime_diag_quiet() {
            return;
        }
        let Some((width, _)) = int_info(&self.ast, lty) else { return };
        let Some(v) = self.fold_int(r) else { return };
        let id = if v < 0 {
            DiagId::sema_shift_negative
        } else if v >= i128::from(width) {
            DiagId::sema_shift_gt_typewidth
        } else {
            return;
        };
        let msg = self.diag(id, loc).range(self.expr_range(r));
        self.diag_if_reachable(msg, r);
    }

    fn assign(&mut self, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        self.record_modified_param(l);
        if !self.check_modifiable(l, loc) {
            return self.recover_err(&[l, r], loc);
        }
        self.check_null_indirection(l);
        let lt = self.ety(l);
        let to = conv::strip_cv(self, lt);
        let conv = match conv::atomic_class_assign(self, to, r) {
            Some(x) => Ok(x),
            None => conv::implicit_convert(self, r, to, ConvCtx::Assignment),
        };
        let r2 = match conv {
            Ok(x) => x,
            Err(err) => {
                // An `_Atomic` target is checked as in C, at the operator
                // (clang `DiagnoseAssignmentResult`).
                let d = if self.ast.types.as_atomic(to).is_some() {
                    err.diag_at(self, loc)
                } else {
                    err.diag(self, r)
                };
                self.emit(d);
                return self.recover_err(&[l, r], loc);
            }
        };
        // clang `CheckAssignmentOperands`' lifetime check (spec/sema-class.md §5.6).
        self.check_assignment_lifetime(l, r2);
        let res = self.builder().binary(BinaryOp::Assign, l, r2, lt, loc);
        self.mark_bitfield_if(res, l);
        self.note_volatile_assign(l);
        res
    }

    fn compound_assign(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        // The operands are checked as for `E1 op E2` before `E1` is
        // checked to be modifiable (clang's `CreateBuiltinBinOp`).
        // An `_Atomic` target is checked by its value type (clang's
        // `CreateBuiltinBinOp` via `UsualArithmeticConversions`).
        let target_t = self.ety(l);
        let lt = self.ast.types.as_atomic(target_t).unwrap_or(target_t);
        let rt = self.ety(r);
        let rt = self.ast.types.as_atomic(rt).unwrap_or(rt);
        let (r2, lhs_ty, res_ty) = match op {
            BinaryOp::AddAssign | BinaryOp::SubAssign if self.ast.types.is_pointer(lt) => {
                if !self.is_integral_or_enum(rt) || conv::is_scoped_enum(self, rt) {
                    return self.invalid_operands(l, r, loc);
                }
                if !self.check_pointer_arith(lt, loc) {
                    return self.recover_err(&[l, r], loc);
                }
                let r2 = conv::usual_unary_conversions(self, r);
                let p = conv::strip_cv(self, lt);
                (r2, p, p)
            }
            // [expr.ass]/6: `b += p` is `b = b + p`, and only a `bool`
            // takes the pointer back ([conv.bool]); the computation is the
            // promoted `b` plus `p` (clang's `CheckAdditionOperands` with
            // `CompLHSTy`, then `CheckAssignmentOperands`).
            BinaryOp::AddAssign
                if (self.ast.types.is_pointer(rt) || self.ast.types.is_array(rt))
                    && self.ast.types.same_unqualified(lt, QualType::BOOL) =>
            {
                let r2 = conv::usual_unary_conversions(self, r);
                let p = conv::strip_cv(self, self.ety(r2));
                if !self.check_pointer_arith(p, loc) {
                    return self.recover_err(&[l, r], loc);
                }
                (r2, QualType::INT, p)
            }
            BinaryOp::ShlAssign | BinaryOp::ShrAssign => {
                let int_like = |s: &Self, t: QualType| {
                    s.ast.types.is_integral(t) || conv::is_unscoped_enum(s, t)
                };
                if !int_like(self, lt) || !int_like(self, rt) {
                    return self.invalid_operands(l, r, loc);
                }
                let r2 = conv::usual_unary_conversions(self, r);
                let bw = conv::bit_field_width(self, l);
                let base = conv::strip_cv(self, lt);
                let p = conv::promoted_type(self, base, bw).unwrap_or(base);
                // clang `CheckShiftOperands` with `IsCompAssign`.
                self.diagnose_shift_count(r2, p, loc);
                (r2, p, p)
            }
            _ => {
                if !self.is_arith_operand(lt) || !self.is_arith_operand(rt) {
                    return self.invalid_operands(l, r, loc);
                }
                match conv::usual_arithmetic_conversions_kind(
                    self,
                    l,
                    r,
                    ArithConvKind::CompoundAssign,
                ) {
                    Ok((_, r2, ty)) => {
                        self.report_arith_warning(loc);
                        let needs_int = !matches!(
                            op,
                            BinaryOp::MulAssign
                                | BinaryOp::DivAssign
                                | BinaryOp::AddAssign
                                | BinaryOp::SubAssign
                        );
                        if needs_int && !self.ast.types.is_integral(ty) {
                            return self.invalid_operands(l, r, loc);
                        }
                        // clang `CheckMultiplyDivideOperands` /
                        // `CheckRemainderOperands` with `IsCompAssign`.
                        if matches!(op, BinaryOp::DivAssign | BinaryOp::RemAssign) {
                            self.diagnose_division_by_zero(op, r2, loc);
                        }
                        (r2, ty, ty)
                    }
                    Err(err) => return self.conv_failed(&err, l, r, loc),
                }
            }
        };
        if !self.check_modifiable(l, loc) {
            return self.recover_err(&[l, r], loc);
        }
        self.check_null_indirection(l);
        // [expr.ass]/6: the result `E1 op E2` is assigned back to `E1`. An
        // arithmetic computation type converts implicitly to any arithmetic
        // left type, but not to an unscoped enumeration ([conv.prom]/3
        // promoted it), so `e += 1` is ill-formed (clang's
        // `CheckAssignmentOperands` with the computation result type).
        let real_from_complex =
            self.ast.types.as_complex(res_ty).is_some() && self.ast.types.as_complex(lt).is_none();
        if (real_from_complex || conv::is_unscoped_enum(self, lt))
            && !self.ast.types.same_unqualified(res_ty, conv::strip_cv(self, lt))
        {
            let msg = self.diag(DiagId::sema_assign_incompatible, loc).diff_args(self, lt, res_ty);
            self.emit(msg);
            return self.recover_err(&[l, r], loc);
        }
        // P1467 [conv.double]/1: the result converts back to `E1`'s type
        // (GCC: diagnosed at `E2`).
        conv::fp_rank_check(self, r, res_ty, conv::strip_cv(self, lt));
        self.check_assignment_lifetime(l, r2);
        let res = self.builder().compound_assign(op, l, r2, target_t, lhs_ty, res_ty, loc);
        self.mark_bitfield_if(res, l);
        res
    }

    fn additive(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        let l2 = conv::decay(self, l);
        let r2 = conv::decay(self, r);
        let (lt, rt) = (self.ety(l2), self.ety(r2));
        let (lp, rp) = (self.ast.types.is_pointer(lt), self.ast.types.is_pointer(rt));
        let int_like =
            |s: &Self, t: QualType| s.ast.types.is_integral(t) || conv::is_unscoped_enum(s, t);
        if lp && rp {
            if op != BinaryOp::Sub {
                return self.invalid_operands(l, r, loc);
            }
            let (a, b) = (self.ast.types.pointee(lt), self.ast.types.pointee(rt));
            let same = match (a, b) {
                (Some(a), Some(b)) => self.ast.types.same_unqualified(a, b),
                _ => false,
            };
            // clang `CheckSubtractionOperands`: the pointee check runs
            // after an incompatibility too.
            if !same {
                let msg = self
                    .diag(DiagId::sema_sub_ptr_compatible, loc)
                    .arg(self.ty_arg(lt))
                    .arg(self.ty_arg(rt));
                self.emit(msg);
            }
            if !self.check_pointer_pair_arith(lt, rt, loc) || !same {
                return self.recover_err(&[l2, r2], loc);
            }
            let ty = self.ptrdiff_type();
            return self.builder().binary(op, l2, r2, ty, loc);
        }
        if lp && int_like(self, rt) {
            if !self.check_pointer_arith(lt, loc) {
                return self.recover_err(&[l2, r2], loc);
            }
            let r3 = conv::usual_unary_conversions(self, r2);
            return self.builder().binary(op, l2, r3, lt, loc);
        }
        if rp && int_like(self, lt) && op == BinaryOp::Add {
            if !self.check_pointer_arith(rt, loc) {
                return self.recover_err(&[l2, r2], loc);
            }
            let l3 = conv::usual_unary_conversions(self, l2);
            return self.builder().binary(op, l3, r2, rt, loc);
        }
        if lp || rp {
            return self.invalid_operands(l, r, loc);
        }
        if !self.is_arith_operand(lt) || !self.is_arith_operand(rt) {
            return self.invalid_operands(l, r, loc);
        }
        match conv::usual_arithmetic_conversions_kind(self, l, r, ArithConvKind::Arithmetic) {
            Ok((l3, r3, ty)) => {
                self.report_arith_warning(loc);
                self.builder().binary(op, l3, r3, ty, loc)
            }
            Err(err) => self.conv_failed(&err, l, r, loc),
        }
    }

    /// The composite pointer type of two pointer operands
    /// ([expr.type]/4), converting the operands to it.
    pub(crate) fn composite_pointer(
        &mut self,
        l: ExprId,
        r: ExprId,
    ) -> Option<(ExprId, ExprId, QualType)> {
        let (lt, rt) = (self.ety(l), self.ety(r));
        // [expr.type]/4 applies alike to pointers to members.
        let ptr = |s: &Self, t: QualType| s.ast.types.is_pointer(t) || s.is_member_pointer(t);
        let (lp, rp) = (
            self.ast.types.is_pointer(lt) || self.is_nullptr_type(lt),
            self.ast.types.is_pointer(rt) || self.is_nullptr_type(rt),
        );
        let (lmp, rmp) = (self.is_member_pointer(lt), self.is_member_pointer(rt));
        let (lnull, rnull) =
            (conv::is_null_pointer_constant(self, l), conv::is_null_pointer_constant(self, r));
        if self.is_nullptr_type(lt) && self.is_nullptr_type(rt) {
            return Some((l, r, lt));
        }
        if ptr(self, lt) && (rnull || self.is_nullptr_type(rt)) {
            let r2 = conv::implicit_convert(self, r, lt, ConvCtx::Comparison).ok()?;
            return Some((l, r2, lt));
        }
        if ptr(self, rt) && (lnull || self.is_nullptr_type(lt)) {
            let l2 = conv::implicit_convert(self, l, rt, ConvCtx::Comparison).ok()?;
            return Some((l2, r, rt));
        }
        if self.is_nullptr_type(lt) && rnull {
            let r2 = conv::implicit_convert(self, r, lt, ConvCtx::Comparison).ok()?;
            return Some((l, r2, lt));
        }
        if self.is_nullptr_type(rt) && lnull {
            let l2 = conv::implicit_convert(self, l, rt, ConvCtx::Comparison).ok()?;
            return Some((l2, r, rt));
        }
        if !((lp && rp) || (lmp && rmp)) {
            return None;
        }
        if self.ast.types.same_type(lt, rt) {
            return Some((l, r, lt));
        }
        if conv::standard_conversion(self, r, lt, ConvCtx::Comparison).is_some() {
            let r2 = conv::implicit_convert(self, r, lt, ConvCtx::Comparison).ok()?;
            return Some((l, r2, lt));
        }
        if conv::standard_conversion(self, l, rt, ConvCtx::Comparison).is_some() {
            let l2 = conv::implicit_convert(self, l, rt, ConvCtx::Comparison).ok()?;
            return Some((l2, r, rt));
        }
        // /4.6: similar types (any depth) give their cv-combined type,
        // `int **` and `const int **` give `const int *const *`.
        if let Some(to) = conv::cv_combined_type(self, lt, rt)
            && conv::standard_conversion(self, l, to, ConvCtx::Comparison).is_some()
            && conv::standard_conversion(self, r, to, ConvCtx::Comparison).is_some()
        {
            let l2 = conv::implicit_convert(self, l, to, ConvCtx::Comparison).ok()?;
            let r2 = conv::implicit_convert(self, r, to, ConvCtx::Comparison).ok()?;
            return Some((l2, r2, to));
        }
        self.cv_combined_pointer(l, r, lt, rt)
    }

    /// [expr.type]/4.3, /4.5 at one level: `void*` and `const T*` give
    /// `const void*`, `Base*` and `const Derived*` give `const Base*` -- a
    /// pointer to either pointee with both sides' cv-qualifiers, if both
    /// operands convert to it. For pointers to members (/4.6), `int B::*`
    /// and `const int D::*` give `const int D::*` the same way.
    fn cv_combined_pointer(
        &mut self,
        l: ExprId,
        r: ExprId,
        lt: QualType,
        rt: QualType,
    ) -> Option<(ExprId, ExprId, QualType)> {
        let types = &self.ast.types;
        let (lpe, rpe) = (types.pointee(lt)?, types.pointee(rt)?);
        let cv = types.effective_quals(lpe) | types.effective_quals(rpe);
        let class_of = |s: &Self, t: QualType| match s.ast.types.canonical_view(t) {
            TypeView::MemberPointer { class, .. } => Some(class),
            _ => None,
        };
        for (side, class) in [(lpe, class_of(self, lt)), (rpe, class_of(self, rt))] {
            let pointee = self.ast.types.unqualified(side);
            let pointee = self.ast.types.qualify(pointee, cv);
            let to = match class {
                Some(c) => self.ast.types.member_pointer(pointee, c),
                None => self.ast.types.pointer_to(pointee),
            };
            if conv::standard_conversion(self, l, to, ConvCtx::Comparison).is_some()
                && conv::standard_conversion(self, r, to, ConvCtx::Comparison).is_some()
            {
                let l2 = conv::implicit_convert(self, l, to, ConvCtx::Comparison).ok()?;
                let r2 = conv::implicit_convert(self, r, to, ConvCtx::Comparison).ok()?;
                return Some((l2, r2, to));
            }
        }
        None
    }

    /// An equality or relational comparison of two arrays (clang's
    /// `diagnoseTautologicalComparison`, array part): ill-formed in C++26
    /// ([expr.eq]/2, [expr.rel]/2, P2865; clang's `DefaultError`
    /// `-Warray-compare-cxx26`), deprecated in C++20 and C++23
    /// ([depr.array.comp]; `-Warray-compare` when that warning is off),
    /// `-Warray-compare` before. Then, where run-time behavior is
    /// diagnosed, clang's `warn_comparison_always` when both operands name
    /// variables: distinct ones never compare equal, one is a
    /// self-comparison. Not in a template instantiation (clang checks the
    /// definition only), constraint satisfaction included: clang's
    /// `inTemplateInstantiation` counts a `ConstraintsCheck` frame, so
    /// libc++'s `equality_comparable<int[5]>` holds (WP-446).
    fn diagnose_array_comparison(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) {
        if self.inst.stack.in_instantiation_record() {
            return;
        }
        let (ls, rs) = (self.ignore_paren_imp_casts(l), self.ignore_paren_imp_casts(r));
        if !self.ast.types.is_array(self.ety(ls)) || !self.ast.types.is_array(self.ety(rs)) {
            return;
        }
        let id = if self.std >= kiln_lex::Std::Cxx26 {
            DiagId::sema_array_comparison_cxx26
        } else if self.std >= kiln_lex::Std::Cxx20
            && self.diags.is_enabled_at(DiagId::sema_depr_array_comparison, loc)
        {
            DiagId::sema_depr_array_comparison
        } else {
            DiagId::sema_array_comparison
        };
        let msg = self.diag(id, loc).range(self.expr_range(l)).range(self.expr_range(r));
        self.emit(msg);
        let var = |s: &Self, e: ExprId| {
            (s.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr)
                .then(|| s.ast.exprs.decl(e))
                .filter(|&d| {
                    s.kind_of(d) == DeclKind::Var
                        && s.ast.decls.get(d).ty().is_some_and(|t| !s.ast.types.is_reference(t))
                })
        };
        let (Some(ld), Some(rd)) = (var(self, ls), var(self, rs)) else { return };
        // clang's `%select`s: self-|array, a constant|true|false.
        let (what, result) = if ld == rd {
            (0, if matches!(op, BinaryOp::EQ | BinaryOp::LE | BinaryOp::GE) { 1 } else { 2 })
        } else {
            let result = match op {
                BinaryOp::EQ => 2,
                BinaryOp::NE => 1,
                _ => 0,
            };
            (1, result)
        };
        let msg = self.diag(DiagId::sema_comparison_always, loc).arg(what).arg(result);
        self.diag_runtime_behavior_now(msg);
    }

    /// clang `Expr::isSameComparisonOperand` (simplified): through
    /// parentheses and implicit conversions, the same variable, `this`,
    /// or the same member of the same such operand.
    fn same_comparison_operand(&self, l: ExprId, r: ExprId) -> bool {
        let (l, r) = (self.ignore_paren_imp_casts(l), self.ignore_paren_imp_casts(r));
        let (ln, rn) = (self.ast.exprs.get(l), self.ast.exprs.get(r));
        if ln.kind() != rn.kind() {
            return false;
        }
        match ln.kind() {
            ExprKind::CXXThisExpr => true,
            ExprKind::DeclRefExpr => {
                let d = self.ast.exprs.decl(l);
                d == self.ast.exprs.decl(r)
                    && matches!(self.kind_of(d), DeclKind::Var | DeclKind::ParmVar)
            }
            ExprKind::MemberExpr => {
                self.ast.exprs.decl(l) == self.ast.exprs.decl(r)
                    && ln.has(ExprFlags::ARROW) == rn.has(ExprFlags::ARROW)
                    && self.kind_of(self.ast.exprs.decl(l)) == DeclKind::Field
                    && self.same_comparison_operand(
                        self.ast.exprs.operand(l),
                        self.ast.exprs.operand(r),
                    )
            }
            _ => false,
        }
    }

    /// clang `diagnoseTautologicalComparison`, self-comparison part:
    /// `x == x` of a non-floating type always evaluates to `true`
    /// (`==`, `<=`, `>=`), `false` (`!=`, `<`, `>`) or
    /// `std::strong_ordering::equal` (`<=>`), where run-time behavior is
    /// diagnosed. Not in a template instantiation nor from a macro; a
    /// class operand only at clang 22, where an operator without a viable
    /// candidate is checked as a built-in comparison (spec/sema.md §1.1).
    /// Arrays are `diagnose_array_comparison`'s. A vector comparison is
    /// checked after its operands are converted (clang
    /// `CheckVectorCompareOperands`), except a floating vector (clang's
    /// `hasFloatingRepresentation`).
    pub(crate) fn diagnose_self_comparison(
        &mut self,
        op: BinaryOp,
        l: ExprId,
        r: ExprId,
        loc: Loc,
    ) {
        if self.inst.stack.depth() > 0 || !self.same_comparison_operand(l, r) {
            return;
        }
        let t = self.ety(self.ignore_paren_imp_casts(l));
        let ty = &self.ast.types;
        if ty.is_array(t)
            || ty.is_floating(t)
            || ty.as_complex(t).is_some()
            || self.vector_parts(t).is_some_and(|v| ty.is_floating(v.elem))
            || (ty.as_record(t).is_some() && self.clang_compat < 22)
            || self.is_nullptr_type(t)
            || self.is_dependent_type(t)
        {
            return;
        }
        let begin_macro = |s: &Self, e: ExprId| s.begin_loc(e).is_macro();
        if loc.is_macro() || begin_macro(self, l) || begin_macro(self, r) {
            return;
        }
        let result = match op {
            BinaryOp::EQ | BinaryOp::LE | BinaryOp::GE => 1,
            BinaryOp::NE | BinaryOp::LT | BinaryOp::GT => 2,
            BinaryOp::Cmp => 3,
            _ => return,
        };
        let msg = self.diag(DiagId::sema_comparison_always, loc).arg(0).arg(result);
        self.diag_runtime_behavior_now(msg);
    }

    fn comparison(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        self.diagnose_array_comparison(op, l, r, loc);
        self.diagnose_self_comparison(op, l, r, loc);
        if matches!(op, BinaryOp::EQ | BinaryOp::NE) {
            self.diagnose_always_non_null_compare(op == BinaryOp::EQ, l, r);
        }
        let l2 = conv::decay(self, l);
        let r2 = conv::decay(self, r);
        let (lt, rt) = (self.ety(l2), self.ety(r2));
        if !self.misaligned.is_empty()
            && self.ast.types.is_pointer(lt)
            && self.ast.types.is_pointer(rt)
        {
            // clang converts both pointer operands to their composite
            // pointer type as initializations (`-Waddress-of-packed-member`).
            self.discard_misaligned_member_address(lt, l2);
            self.discard_misaligned_member_address(rt, r2);
        }
        if self.is_member_pointer(lt) || self.is_member_pointer(rt) {
            return self.member_pointer_comparison(op, l2, r2, loc);
        }
        // [expr.eq]/7 (P2996): two `std::meta::info` operands compare
        // equal when they reflect the same entity; nothing converts.
        if self.is_meta_info(lt) || self.is_meta_info(rt) {
            if self.is_meta_info(lt)
                && self.is_meta_info(rt)
                && matches!(op, BinaryOp::EQ | BinaryOp::NE)
            {
                return self.builder().binary(op, l2, r2, QualType::BOOL, loc);
            }
            return self.invalid_operands(l, r, loc);
        }
        let ptr_like = |s: &Self, t: QualType| s.ast.types.is_pointer(t) || s.is_nullptr_type(t);
        if ptr_like(self, lt) || ptr_like(self, rt) {
            if !matches!(op, BinaryOp::EQ | BinaryOp::NE) {
                // [expr.rel]/1: the operands of a relational operator have
                // arithmetic, enumeration or pointer type, so a
                // `std::nullptr_t` operand is `invalid operands` (CWG583)
                // and a null pointer constant does not convert to the other
                // side's pointer type ([expr.rel]/3 applies only when both
                // are pointers): clang's `CheckCompareOperands`.
                if self.is_nullptr_type(lt) || self.is_nullptr_type(rt) {
                    return self.invalid_operands(l2, r2, loc);
                }
                let (lp, rp) = (self.ast.types.is_pointer(lt), self.ast.types.is_pointer(rt));
                let int = |s: &Self, t: QualType| s.ast.types.is_integral(t);
                let null_int = (lp && int(self, rt) && conv::is_null_pointer_constant(self, r2))
                    || (rp && int(self, lt) && conv::is_null_pointer_constant(self, l2));
                if null_int {
                    let msg = self
                        .diag(DiagId::sema_ordered_pointer_zero, loc)
                        .arg(self.ty_arg(lt))
                        .arg(self.ty_arg(rt));
                    self.emit(msg);
                    return self.recover_err(&[l2, r2], loc);
                }
            }
            if let Some((l3, r3, _)) = self.composite_pointer(l2, r2) {
                return self.builder().binary(op, l3, r3, QualType::BOOL, loc);
            }
            let int_like = |s: &Self, t: QualType| s.is_integral_or_enum(t);
            let id = if (ptr_like(self, lt) && int_like(self, rt))
                || (ptr_like(self, rt) && int_like(self, lt))
            {
                DiagId::sema_comparison_pointer_integer
            } else if ptr_like(self, lt) && ptr_like(self, rt) {
                DiagId::sema_comparison_distinct_pointers
            } else {
                DiagId::sema_invalid_operands
            };
            let msg = self
                .diag(id, loc)
                .arg(self.ty_arg(lt))
                .arg(self.ty_arg(rt))
                .range(self.expr_range(l))
                .range(self.expr_range(r));
            self.emit(msg);
            return self.recover_err(&[l2, r2], loc);
        }
        if !self.is_scalar(lt) || !self.is_scalar(rt) {
            return self.invalid_operands(l, r, loc);
        }
        let complex =
            self.ast.types.as_complex(lt).is_some() || self.ast.types.as_complex(rt).is_some();
        if complex && !matches!(op, BinaryOp::EQ | BinaryOp::NE) {
            return self.invalid_operands(l, r, loc);
        }
        match conv::usual_arithmetic_conversions_kind(self, l, r, ArithConvKind::Comparison) {
            Ok((l3, r3, _)) => {
                self.report_arith_warning(loc);
                self.builder().binary(op, l3, r3, QualType::BOOL, loc)
            }
            Err(err) => self.conv_failed(&err, l, r, loc),
        }
    }

    pub(crate) fn is_member_pointer(&self, t: QualType) -> bool {
        matches!(self.ast.types.canonical_view(t), TypeView::MemberPointer { .. })
    }

    /// A comparison with a pointer-to-member operand (`l` and `r` already
    /// decayed), clang's `CheckCompareOperands` order: only `==` and `!=`
    /// are defined ([expr.rel]/2 does not cover member pointers, so `<`
    /// is `invalid operands`); a null pointer constant on the other side
    /// converts to the member pointer type (`NullToMemberPointer`,
    /// [expr.eq]/4); two member pointers are brought to their composite
    /// pointer type ([expr.type]/4: the derived class's member pointer,
    /// `BaseToDerivedMemberPointer`, [conv.mem]/2). No composite type,
    /// or a pointer on the other side, is `comparison of distinct pointer
    /// types`; anything else is `invalid operands`.
    fn member_pointer_comparison(
        &mut self,
        op: BinaryOp,
        l: ExprId,
        r: ExprId,
        loc: Loc,
    ) -> ExprId {
        if !matches!(op, BinaryOp::EQ | BinaryOp::NE) {
            return self.invalid_operands(l, r, loc);
        }
        let (lt, rt) = (self.ety(l), self.ety(r));
        let (lmp, rmp) = (self.is_member_pointer(lt), self.is_member_pointer(rt));
        let to_null = |s: &mut Self, e: ExprId, to: QualType| {
            conv::implicit_convert(s, e, to, ConvCtx::Comparison).ok()
        };
        if lmp
            && !rmp
            && conv::is_null_pointer_constant(self, r)
            && let Some(r2) = to_null(self, r, lt)
        {
            return self.builder().binary(op, l, r2, QualType::BOOL, loc);
        }
        if rmp
            && !lmp
            && conv::is_null_pointer_constant(self, l)
            && let Some(l2) = to_null(self, l, rt)
        {
            return self.builder().binary(op, l2, r, QualType::BOOL, loc);
        }
        if lmp && rmp {
            if self.ast.types.same_type(lt, rt) {
                return self.builder().binary(op, l, r, QualType::BOOL, loc);
            }
            // The composite type is the operand type the other operand
            // converts to; a conversion that exists but is ill-formed
            // (through a virtual base, ambiguous) is diagnosed at the
            // converted operand before the comparison itself, as clang does.
            let pair = if conv::standard_conversion(self, r, lt, ConvCtx::Comparison).is_some() {
                Some((r, lt, true))
            } else if conv::standard_conversion(self, l, rt, ConvCtx::Comparison).is_some() {
                Some((l, rt, false))
            } else {
                None
            };
            if let Some((e, to, right)) = pair {
                match conv::implicit_convert(self, e, to, ConvCtx::Comparison) {
                    Ok(e2) => {
                        let (l2, r2) = if right { (l, e2) } else { (e2, r) };
                        return self.builder().binary(op, l2, r2, QualType::BOOL, loc);
                    }
                    Err(err) => {
                        let at = self.begin_loc(e);
                        self.emit_conv_error(err, at);
                    }
                }
            }
        }
        let pointer_like =
            |s: &Self, t: QualType| s.ast.types.is_pointer(t) || s.is_member_pointer(t);
        if !(pointer_like(self, lt) && pointer_like(self, rt)) {
            return self.invalid_operands(l, r, loc);
        }
        let msg = self
            .diag(DiagId::sema_comparison_distinct_pointers, loc)
            .arg(self.ty_arg(lt))
            .arg(self.ty_arg(rt));
        self.emit(msg);
        self.recover_err(&[l, r], loc)
    }

    /// `c ? t : f` ([expr.cond]).
    pub fn act_on_conditional(&mut self, c: ExprId, t: ExprId, f: ExprId, loc: Loc) -> ExprId {
        if self.is_err_expr(c) || self.is_err_expr(t) || self.is_err_expr(f) {
            return self.recover_err(&[c, t, f], loc);
        }
        // clang's `CheckConditionalOperands`: `CheckPlaceholderExpr` on
        // the condition and both operands first (a bound member function
        // is recovered by calling it, [expr.ref]/7.3.2).
        let (c, t, f) = (
            self.recover_bound_member(c),
            self.recover_bound_member(t),
            self.recover_bound_member(f),
        );
        if self.is_err_expr(c) || self.is_err_expr(t) || self.is_err_expr(f) {
            return self.recover_err(&[c, t, f], loc);
        }
        let t = self.resolve_single_overload_candidate(t);
        let f = self.resolve_single_overload_candidate(f);
        // The GNU element-wise conditional (clang `CXXCheckConditionalOperands`).
        if !self.expr_is_type_dependent(c) {
            let ct = self.ety(c);
            if self.is_vector_condition(ct) {
                return self.act_on_vector_conditional(c, t, f, loc);
            }
            if self.is_sve_condition(ct) {
                return self.sve_conditional(c, t, f, loc);
            }
        }
        let c2 = if self.expr_is_type_dependent(c) {
            c
        } else {
            match conv::contextually_convert_to_bool(self, c) {
                Ok(x) => x,
                Err(err) => {
                    let d = err.diag(self, c);
                    self.emit(d);
                    return self.recover_err(&[c, t, f], loc);
                }
            }
        };
        // [temp.dep.expr]/1: type-dependent if any operand is, the
        // condition included.
        if self.expr_is_type_dependent(c)
            || self.expr_is_type_dependent(t)
            || self.expr_is_type_dependent(f)
        {
            return self.builder().dependent_conditional(c2, t, f, loc);
        }
        let (tt, ft) = (self.ety(t), self.ety(f));
        if self.ast.types.is_void(tt) || self.ast.types.is_void(ft) {
            return self.void_conditional(CondOps { c, c2, t, f, loc });
        }
        // [expr.cond]/4: operands of different types, one of class type:
        // each is tried as converted to match the other; exactly one
        // conversion is applied (clang `TryClassUnification`).
        let (t, f) = if !self.ast.types.same_type(tt, ft)
            && (self.ast.types.as_record(tt).is_some() || self.ast.types.as_record(ft).is_some())
        {
            match self.unify_class_arms(CondOps { c, c2, t, f, loc }) {
                Ok(x) => x,
                Err(res) => return res,
            }
        } else {
            (t, f)
        };
        let ops = CondOps { c, c2, t, f, loc };
        if let Some(res) = self.same_category_conditional(ops) {
            return res;
        }
        // [expr.cond]/6: arms still of different types, one of class
        // type: overload resolution over the built-in `operator?:`
        // candidates converts them.
        let (tt, ft) = (self.ety(t), self.ety(f));
        let ops = if !self.ast.types.same_unqualified(tt, ft)
            && (self.ast.types.as_record(tt).is_some() || self.ast.types.as_record(ft).is_some())
        {
            match self.overload_class_arms(ops) {
                Ok((t, f)) => CondOps { c, c2, t, f, loc },
                Err(res) => return res,
            }
        } else {
            ops
        };
        if let Some(res) = self.arith_conditional(ops) {
            return res;
        }
        self.other_conditional(ops)
    }

    /// [expr.cond]/2: a `void` operand. Exactly one throw-expression gives
    /// the other operand's type and value category (/2.1); both `void`
    /// give `void` (/2.2); otherwise ill-formed.
    fn void_conditional(&mut self, o: CondOps) -> ExprId {
        let CondOps { c, c2, t, f, loc } = o;
        let (tt, ft) = (self.ety(t), self.ety(f));
        let (tv, fv) = (self.ast.types.is_void(tt), self.ast.types.is_void(ft));
        let (tthrow, fthrow) = (self.is_throw_expr(t), self.is_throw_expr(f));
        if tthrow != fthrow {
            let other = if tthrow { f } else { t };
            let ty = self.ety(other);
            let res = self.builder().conditional(c2, t, f, ty, loc);
            if self.is_bitfield(other) {
                self.ast.exprs.mark_bitfield(res);
            }
            return res;
        }
        if tv && fv {
            return self.builder().conditional(c2, t, f, QualType::VOID, loc);
        }
        let msg = self
            .diag(DiagId::sema_conditional_void_nonvoid, loc)
            .arg(if tv { "0" } else { "1" })
            .arg(self.ty_arg(if tv { ft } else { tt }));
        self.emit(msg);
        self.recover_err(&[c, t, f], loc)
    }

    /// [expr.cond]/4: the class-type arms converted to match each other
    /// (clang `TryClassUnification` both ways); `Err` carries the
    /// recovery expression after a diagnosed failure.
    fn unify_class_arms(&mut self, o: CondOps) -> Result<(ExprId, ExprId), ExprId> {
        let CondOps { c, t, f, loc, .. } = o;
        let (tt, ft) = (self.ety(t), self.ety(f));
        let l2r = self.try_class_unification(t, f);
        let r2l = self.try_class_unification(f, t);
        let (t, f) = match (l2r, r2l) {
            (Err(()), _) | (_, Err(())) => return Err(self.recover_err(&[c, t, f], loc)),
            (Ok(Some(_)), Ok(Some(_))) => {
                let msg =
                    self.diag(DiagId::sema_conditional_ambiguous, loc).diff_args(self, tt, ft);
                self.emit(msg);
                return Err(self.recover_err(&[c, t, f], loc));
            }
            (Ok(Some(ty)), Ok(None)) => {
                match conv::implicit_convert(self, t, ty, ConvCtx::Initialization) {
                    Ok(x) => (self.unified_class_prvalue(x, ty), f),
                    Err(err) => return Err(self.conv_failed(&err, t, f, loc)),
                }
            }
            (Ok(None), Ok(Some(ty))) => {
                match conv::implicit_convert(self, f, ty, ConvCtx::Initialization) {
                    Ok(x) => (t, self.unified_class_prvalue(x, ty)),
                    Err(err) => return Err(self.conv_failed(&err, t, f, loc)),
                }
            }
            (Ok(None), Ok(None)) => (t, f),
        };
        if self.is_err_expr(t) || self.is_err_expr(f) {
            return Err(self.recover_err(&[c, t, f], loc));
        }
        Ok((t, f))
    }

    /// [expr.cond]/6: the arms converted by overload resolution over the
    /// built-in `operator?:` candidates ([over.built]/25-26; clang
    /// `FindConditionalOverload`). No viable candidate leaves the arms
    /// unchanged (the caller reports them incompatible); `Err` carries
    /// the recovery expression after a diagnosed failure.
    fn overload_class_arms(&mut self, o: CondOps) -> Result<(ExprId, ExprId), ExprId> {
        use crate::overload::{OvlOutcome, SetKind};
        let CondOps { c, t, f, loc, .. } = o;
        let mut set = self.begin_set(SetKind::Normal, loc);
        set.object = Some(self.arg_info(t));
        let mark = self.push_call_args(&mut set, &[f], &[]);
        let types = [self.ety(t), self.ety(f)];
        self.add_conditional_builtin_candidates(&mut set, types);
        let out = self.resolve_set(&mut set);
        self.ovl.lists.truncate(mark);
        let res = match out {
            OvlOutcome::Success(b) => {
                let params = set.cand(b).builtin;
                let conv = self.builtin_operand_conversions(&set, b, &[t, f]);
                self.end_set(set);
                let Some(arms) = conv else { return Err(self.recover_err(&[c, t, f], loc)) };
                let mut out = [t, f];
                for (i, &a) in arms.iter().enumerate().take(2) {
                    match conv::implicit_convert(self, a, params[i], ConvCtx::Initialization) {
                        Ok(x) if !self.is_err_expr(x) => out[i] = x,
                        Ok(_) => return Err(self.recover_err(&[c, t, f], loc)),
                        Err(err) => return Err(self.conv_failed(&err, t, f, loc)),
                    }
                }
                return Ok((out[0], out[1]));
            }
            OvlOutcome::Ambiguous => {
                let msg = self
                    .diag(DiagId::sema_conditional_ambiguous_ovl, loc)
                    .diff_args(self, types[0], types[1]);
                self.emit(msg);
                Err(self.recover_err(&[c, t, f], loc))
            }
            OvlOutcome::NoViable | OvlOutcome::Deleted(_) => Ok((t, f)),
        };
        self.end_set(set);
        res
    }

    /// [expr.cond]/4.3: an arm converted to a class type `ty` (not a
    /// reference) is a prvalue of exactly `ty`, cv-qualifiers included: a
    /// `V` prvalue unified with a `const V` lvalue becomes `const V`, so
    /// the arms then have the same type and the result is a `const V`
    /// prvalue (clang's `NoOp` cast in `TryClassUnification`).
    fn unified_class_prvalue(&mut self, x: ExprId, ty: QualType) -> ExprId {
        if self.is_err_expr(x)
            || self.ast.types.is_reference(ty)
            || self.ast.types.same_type(self.ety(x), ty)
        {
            return x;
        }
        conv::raw_cast(self, CastKind::NoOp, x, ty, ValueCategory::PRValue, None)
    }

    /// [expr.cond]/4 and /5: glvalues of the same type and value category
    /// give that glvalue; an lvalue of a less cv-qualified type binds to a
    /// reference to the other operand's type (/4.1).
    fn same_category_conditional(&mut self, o: CondOps) -> Option<ExprId> {
        let CondOps { c2, t, f, loc, .. } = o;
        let (tn, fnn) = (*self.ast.exprs.get(t), *self.ast.exprs.get(f));
        let (tt, ft) = (tn.ty(), fnn.ty());
        if tn.value_category().is_glvalue()
            && tn.value_category() == fnn.value_category()
            && self.ast.types.same_type(tt, ft)
        {
            let res = self.builder().conditional(c2, t, f, tt, loc);
            if self.is_bitfield(t) || self.is_bitfield(f) {
                self.ast.exprs.mark_bitfield(res);
            }
            return Some(res);
        }
        if tn.value_category() != ValueCategory::LValue
            || fnn.value_category() != ValueCategory::LValue
            || self.ast.types.as_record(tt).is_some()
            || !self.ast.types.same_unqualified(tt, ft)
        {
            return None;
        }
        let (tq, fq) = (self.ast.types.effective_quals(tt), self.ast.types.effective_quals(ft));
        let lv = ValueCategory::LValue;
        let (t2, f2, ty) = if tq.contains(fq) {
            (t, conv::raw_cast(self, CastKind::NoOp, f, tt, lv, None), tt)
        } else if fq.contains(tq) {
            (conv::raw_cast(self, CastKind::NoOp, t, ft, lv, None), f, ft)
        } else {
            return None;
        };
        let res = self.builder().conditional(c2, t2, f2, ty, loc);
        if self.is_bitfield(t) || self.is_bitfield(f) {
            self.ast.exprs.mark_bitfield(res);
        }
        Some(res)
    }

    /// [expr.cond]/7.2: arithmetic or enumeration arms. After the standard
    /// conversions, operands of the same type give that type (no
    /// promotion: `b ? s1 : s2` is `short`); otherwise the usual
    /// arithmetic conversions.
    fn arith_conditional(&mut self, o: CondOps) -> Option<ExprId> {
        let CondOps { c2, t, f, loc, .. } = o;
        let (tt, ft) = (self.ety(t), self.ety(f));
        let arith = |s: &Self, q: QualType| {
            s.ast.types.is_arithmetic(q)
                || s.ast.types.as_enum(q).is_some()
                || s.ast.types.as_complex(q).is_some()
        };
        if !arith(self, tt) || !arith(self, ft) {
            return None;
        }
        let t2 = conv::decay(self, t);
        let f2 = conv::decay(self, f);
        let (t2ty, f2ty) = (self.ety(t2), self.ety(f2));
        if self.ast.types.same_type(t2ty, f2ty) {
            return Some(self.builder().conditional(c2, t2, f2, t2ty, loc));
        }
        Some(
            match conv::usual_arithmetic_conversions_kind(self, t, f, ArithConvKind::Conditional) {
                Ok((t2, f2, ty)) => {
                    self.report_arith_warning(loc);
                    // clang's `PrepareScalarCast` to the result type: a
                    // real arm of a complex result becomes complex.
                    let t2 = self.real_to_complex(t2, ty);
                    let f2 = self.real_to_complex(f2, ty);
                    self.builder().conditional(c2, t2, f2, ty, loc)
                }
                Err(err) => self.conv_failed(&err, t, f, loc),
            },
        )
    }

    /// [expr.cond]/7.3-7.5 and the vector extension (clang
    /// `CheckVectorOperands`): vector arms, the composite pointer type,
    /// and same-class prvalues; otherwise the operands are incompatible.
    fn other_conditional(&mut self, o: CondOps) -> ExprId {
        let CondOps { c, c2, t, f, loc } = o;
        let (tt, ft) = (self.ety(t), self.ety(f));
        if self.is_vector_type(tt) || self.is_vector_type(ft) {
            let mut t2 = conv::decay(self, t);
            let mut f2 = conv::decay(self, f);
            return match self.scalar_cond_vector_operands(&mut t2, &mut f2, loc) {
                Some(ty) => self.builder().conditional(c2, t2, f2, ty, loc),
                None => self.recover_err(&[c, t, f], loc),
            };
        }
        let t2 = conv::decay(self, t);
        let f2 = conv::decay(self, f);
        if let Some((t3, f3, ty)) = self.composite_pointer(t2, f2) {
            return self.builder().conditional(c2, t3, f3, ty, loc);
        }
        // [expr.cond]/7.2 (P2996): two `std::meta::info` arms.
        let same_info = self.is_meta_info(tt) && self.is_meta_info(ft);
        if (same_info || self.ast.types.as_record(tt).is_some())
            && self.ast.types.same_unqualified(tt, ft)
        {
            // Class arms of the same type, cv-qualifiers included (after
            // the /4 unification, `c ? V() : cref` has two `const V`
            // arms), give that type: the result is a `const V` prvalue
            // (clang `hasSameType(LTy, RTy)`; duckdb binds `auto &` to
            // one, WP-799).
            let ty = if !same_info && self.ast.types.same_type(tt, ft) {
                tt
            } else {
                conv::strip_cv(self, tt)
            };
            if same_info {
                return self.builder().conditional(c2, t2, f2, ty, loc);
            }
            // [expr.cond]/7: a class glvalue arm copy-initializes the
            // result prvalue (clang `PerformCopyInitialization` of an
            // `InitializeTemporary` entity per arm): its copy or move
            // constructor runs.
            let mut arms = [t2, f2];
            for a in &mut arms {
                if !self.ast.exprs.get(*a).value_category().is_glvalue() {
                    continue;
                }
                match conv::implicit_convert(self, *a, ty, ConvCtx::Initialization) {
                    Ok(x) if !self.is_err_expr(x) => *a = x,
                    Ok(_) => return self.recover_err(&[c, t, f], loc),
                    Err(err) => return self.conv_failed(&err, t, f, loc),
                }
            }
            return self.builder().conditional(c2, arms[0], arms[1], ty, loc);
        }
        // clang names the arms' types after the /7 standard conversions
        // (an array or function arm as its pointer, WP-408).
        let (tt, ft) = (self.ety(t2), self.ety(f2));
        let msg = self.diag(DiagId::sema_incompatible_cond_operands, loc).diff_args(self, tt, ft);
        self.emit(msg);
        self.recover_err(&[c, t, f], loc)
    }

    /// [expr.cond]/4: the type operand `from` is converted to so that it
    /// matches operand `to`, if there is one (clang
    /// `TryClassUnification`); `Err` when that conversion is ambiguous
    /// (diagnosed).
    fn try_class_unification(&mut self, from: ExprId, to: ExprId) -> Result<Option<QualType>, ()> {
        use crate::conv::scs::RefBinding;
        use crate::overload::ics::{Ics, IcsOptions};
        let tn = *self.ast.exprs.get(to);
        let tty = tn.ty();
        let a = self.arg_info(from);
        let from_glvalue = a.vk.is_glvalue() && a.bit_field.is_none();
        let ambiguous = |s: &mut Self, ty: QualType| {
            if let Err(err) = conv::implicit_convert(s, from, ty, ConvCtx::Initialization) {
                let at = s.begin_loc(from);
                s.emit_conv_error(err, at);
            }
            Err(())
        };
        // /4.1, /4.2: a reference to `to`'s type that binds directly to a
        // glvalue.
        if tn.value_category().is_glvalue() {
            let r = if tn.value_category() == ValueCategory::LValue {
                self.ast.types.lvalue_ref_to(tty)
            } else {
                self.ast.types.rvalue_ref_to(tty)
            };
            match self.try_implicit_conversion(&a, r, IcsOptions::NONE) {
                Ics::Standard(scs) if scs.rb.contains(RefBinding::DIRECT) && from_glvalue => {
                    return Ok(Some(r));
                }
                Ics::UserDefined(u)
                    if u.after.rb.contains(RefBinding::DIRECT)
                        && u.func.is_some_and(|f| {
                            let ft = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                            self.ast
                                .types
                                .proto(ft)
                                .is_some_and(|p| self.ast.types.is_reference(p.ret()))
                        }) =>
                {
                    return Ok(Some(r));
                }
                Ics::Ambiguous { .. } => return ambiguous(self, r),
                _ => {}
            }
        }
        // /4.3.1, /4.3.2: the same class or a base of it, at least as
        // cv-qualified.
        let fty = a.ty;
        if let (Some(fr), Some(tr)) = (self.ast.types.as_record(fty), self.ast.types.as_record(tty))
        {
            let same = conv::scs::class_decl(self, fty.ty())
                == conv::scs::class_decl(self, tty.ty())
                || self.ast.types.same_unqualified(fty, tty);
            let fd = self.ast.definition(fr).unwrap_or(fr);
            let td = self.ast.definition(tr).unwrap_or(tr);
            if same || (self.ast.definition(fr).is_some() && self.derived_inclusive(fd, td)) {
                let (fq, tq) =
                    (self.ast.types.effective_quals(fty), self.ast.types.effective_quals(tty));
                if tq.contains(fq & kiln_ast::types::Quals::CV) {
                    return match self.try_implicit_conversion(&a, tty, IcsOptions::NONE) {
                        Ics::Ambiguous { .. } => ambiguous(self, tty),
                        Ics::Bad(_) | Ics::Ellipsis => Ok(None),
                        _ => Ok(Some(tty)),
                    };
                }
                return Ok(None);
            }
            // A base class operand is never converted to match a derived
            // one, not even by a converting constructor (clang's
            // `TryClassUnification`, the C++11 [expr.cond]/3 rule that
            // GCC also keeps): `c ? B() : D()` has type `B` although
            // `D(B)` exists.
            if self.ast.definition(tr).is_some() && self.derived_inclusive(td, fd) {
                return Ok(None);
            }
        }
        // /4.3.3: the type `to` has as a prvalue.
        let target =
            if self.ast.types.as_record(tty).is_some() { tty } else { conv::strip_cv(self, tty) };
        match self.try_implicit_conversion(&a, target, IcsOptions::NONE) {
            Ics::Ambiguous { .. } => ambiguous(self, target),
            Ics::Bad(_) | Ics::Ellipsis => Ok(None),
            _ => Ok(Some(target)),
        }
    }

    // -- postfix -----------------------------------------------------------------

    /// The function prototype of `q` as written: sugar is looked through
    /// only down to the first function type, so parameter types keep
    /// their typedef names (clang converts arguments to them).
    pub(crate) fn sugared_proto(&self, q: QualType) -> Option<kiln_ast::types::FuncProto> {
        let mut cur = q;
        for _ in 0..256 {
            if let Some(p) = self.ast.types.proto_of(cur.ty()) {
                return Some(*p);
            }
            let next = self.ast.types.desugar_once(cur);
            if next == cur {
                break;
            }
            cur = next;
        }
        self.ast.types.proto(q).copied()
    }

    /// `e1[args]` ([expr.sub], [over.sub]): `loc` is the `[`, `end` the
    /// `]`, recorded as a built-in subscript's end location.
    pub fn act_on_subscript_args(
        &mut self,
        base: ExprId,
        args: &[ExprId],
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let r = self.subscript_args(base, args, loc, end);
        self.builder().set_subscript_end(r, end);
        r
    }

    fn subscript_args(&mut self, base: ExprId, args: &[ExprId], loc: Loc, end: Loc) -> ExprId {
        let braced: Vec<Option<InitList>> = args.iter().map(|&a| self.take_braced_arg(a)).collect();
        let mut kids = vec![base];
        kids.extend_from_slice(args);
        if kids.iter().any(|&e| self.is_err_expr(e)) {
            return self.recover_err(&kids, loc);
        }
        let single = (args.len() == 1 && braced[0].is_none()).then(|| args[0]);
        if let Some(i) = single
            && self.std >= kiln_lex::Std::Cxx20
            && self.is_comma_expr(i)
        {
            // [depr.comma.subscript]: before C++23 the index is one
            // expression (clang `warn_deprecated_comma_subscript`, at the
            // comma; C++23 parses an expression-list instead).
            let at = self.ast.exprs.get(i).loc();
            self.error(DiagId::sema_deprecated_comma_subscript, at);
        }
        if kids.iter().any(|&e| self.expr_is_type_dependent(e)) {
            if let Some(idx) =
                single.filter(|&i| self.ast.exprs.get(i).kind() != ExprKind::PackExpansionExpr)
            {
                return self.act_on_subscript(base, idx, loc);
            }
            // clang `CreateOverloadedArraySubscriptExpr`: several indices
            // or a pack expansion (`m[is...]`, [expr.sub]/1) make a
            // dependent `operator[]` call with an empty lookup set,
            // resolved when instantiated.
            let name = DeclName::operator(kiln_ast::names::OverloadedOperator::Subscript);
            let callee = self.builder().unresolved_lookup(name, &[], true, None, None, loc);
            return self.builder().operator_call(
                kiln_ast::names::OverloadedOperator::Subscript,
                callee,
                &kids,
                QualType::DEPENDENT,
                ValueCategory::PRValue,
                loc,
                end,
            );
        }
        let mut br = vec![None];
        br.extend(braced.iter().cloned());
        match self.try_overloaded_operator(OperForm::Subscript, &kids, &br, loc, end) {
            OperResult::Done(e) => return e,
            OperResult::Converted(v) if v.len() == 2 => {
                return self.builtin_subscript(v[0], v[1], loc);
            }
            OperResult::Builtin | OperResult::BuiltinNotes(_) | OperResult::Converted(_) => {}
        }
        if let Some(idx) = single {
            return self.act_on_subscript(base, idx, loc);
        }
        if args.len() == 1 {
            // A braced index of a built-in subscript ([expr.sub]/2).
            self.error(DiagId::sema_subscript_not_integer, self.begin_loc(args[0]));
        } else {
            let msg =
                self.diag(DiagId::sema_no_subscript_operator, loc).arg(self.ty_arg(self.ety(base)));
            self.emit(msg);
        }
        self.recover_err(&kids, loc)
    }

    /// A built-in or overloaded comma operator expression, unparenthesized.
    fn is_comma_expr(&self, e: ExprId) -> bool {
        match self.ast.exprs.get(e).kind() {
            ExprKind::BinaryOperator => self.ast.exprs.binary_op(e) == BinaryOp::Comma,
            ExprKind::CXXOperatorCallExpr => {
                self.ast.exprs.operator_call_op(e) == kiln_ast::names::OverloadedOperator::Comma
            }
            _ => false,
        }
    }

    /// `e1[e2]` ([expr.sub]).
    pub fn act_on_subscript(&mut self, base: ExprId, idx: ExprId, loc: Loc) -> ExprId {
        if self.is_err_expr(base) || self.is_err_expr(idx) {
            return self.recover_err(&[base, idx], loc);
        }
        if !self.expr_is_type_dependent(base)
            && !self.expr_is_type_dependent(idx)
            && (self.ast.types.as_record(self.non_reference(self.ety(base))).is_some()
                || self.ast.types.as_record(self.non_reference(self.ety(idx))).is_some())
        {
            return self.act_on_subscript_args_resolved(base, idx, loc);
        }
        self.builtin_subscript(base, idx, loc)
    }

    /// The built-in subscript `base[idx]` ([expr.sub]/2).
    pub(crate) fn builtin_subscript(&mut self, base: ExprId, idx: ExprId, loc: Loc) -> ExprId {
        // Either operand type-dependent: an unanalyzed lvalue with no
        // operand conversions ([temp.dep.expr]/3; clang's
        // `getDependentArraySubscriptType`).
        if self.expr_is_type_dependent(base) || self.expr_is_type_dependent(idx) {
            let ty = self.dependent_subscript_type(base, idx);
            return self.builder().subscript(base, idx, ty, loc);
        }
        if self.is_vector_type(self.ety(base)) || self.is_sve_vector(self.ety(base)) {
            return self.vector_subscript(base, idx, loc);
        }
        let b = conv::decay(self, base);
        let i = conv::decay(self, idx);
        let (bt, it) = (self.ety(b), self.ety(i));
        let (ptr, int) = if self.ast.types.is_pointer(bt) {
            (bt, it)
        } else if self.ast.types.is_pointer(it) {
            (it, bt)
        } else {
            self.error(DiagId::sema_subscript_value, loc);
            return self.recover_err(&[b, i], loc);
        };
        if !self.ast.types.is_integral(int) && !conv::is_unscoped_enum(self, int) {
            self.error(DiagId::sema_subscript_not_integer, loc);
            return self.recover_err(&[b, i], loc);
        }
        let elem = self.pointee_sugared(ptr).unwrap_or(QualType::ERROR);
        if self.ast.types.is_function(elem) {
            // clang `err_subscript_function_type`, at the pointer operand.
            let at = self.begin_loc(if ptr == bt { b } else { i });
            let msg = self.diag(DiagId::sema_subscript_function_type, at).arg(self.ty_arg(elem));
            self.emit(msg);
            return self.recover_err(&[b, i], loc);
        }
        if self.ast.types.is_sizeless(elem) {
            let msg = self.diag(DiagId::sema_subscript_sizeless_type, loc).arg(self.ty_arg(elem));
            self.emit(msg);
            return self.recover_err(&[b, i], loc);
        }
        if !self.require_complete(elem, loc) {
            // clang `err_subscript_incomplete_or_sizeless_type`.
            let msg = self.diag(DiagId::sema_subscript_incomplete_type, loc).arg(self.ty_arg(elem));
            self.emit(self.incomplete_note(msg, elem));
            return self.recover_err(&[b, i], loc);
        }
        self.builder().subscript(b, i, elem, loc)
    }

    /// `act_on_subscript` with a class base: the overloaded `operator[]`.
    fn act_on_subscript_args_resolved(&mut self, base: ExprId, idx: ExprId, loc: Loc) -> ExprId {
        match self.try_overloaded_operator(
            OperForm::Subscript,
            &[base, idx],
            &[],
            loc,
            Loc::INVALID,
        ) {
            OperResult::Done(e) => e,
            OperResult::Converted(v) => self.builtin_subscript(v[0], v[1], loc),
            OperResult::Builtin | OperResult::BuiltinNotes(_)
                if self.ast.types.as_record(self.non_reference(self.ety(base))).is_none() =>
            {
                self.builtin_subscript(base, idx, loc)
            }
            OperResult::Builtin | OperResult::BuiltinNotes(_) => {
                let msg = self
                    .diag(DiagId::sema_no_subscript_operator, loc)
                    .arg(self.ty_arg(self.ety(base)));
                self.emit(msg);
                self.recover_err(&[base, idx], loc)
            }
        }
    }

    /// Reports argument `a` (the `i`th, 0-based) of a call of `d` (a
    /// one-candidate overload set) not converting to its parameter type
    /// `pt`: clang's "no matching function" with the candidate's note.
    /// The note details the candidate's bad conversion sequence (clang
    /// `DiagnoseBadConversion`: a temporary's referent, an lvalue
    /// required, lost qualifiers).
    pub(crate) fn bad_argument(
        &mut self,
        d: DeclId,
        callee: ExprId,
        a: ExprId,
        pt: QualType,
        i: usize,
    ) {
        let msg = self
            .diag(DiagId::sema_ovl_no_viable_function_in_call, self.begin_loc(callee))
            .arg(self.decl_name(d))
            .range(self.expr_range(callee));
        let info = self.arg_info(a);
        let ics =
            self.try_implicit_conversion(&info, pt, crate::overload::IcsOptions::CANDIDATE_ARG);
        let detail = if ics.is_bad() {
            self.bad_conversion_detail(&ics, Some(i), Some(a))
        } else {
            format!(
                "no known conversion from {} to {} for {} argument",
                self.ty_arg(self.ety(a)),
                self.ty_arg(pt),
                ordinal(i + 1)
            )
        };
        let fl = self.ast.decls.get(d).loc();
        let note = self.diag(DiagId::note_ovl_candidate_not_viable, fl).arg(detail);
        self.emit(msg.with_note(note));
    }

    /// A function call `f(args)` ([expr.call]). `loc` is the callee's
    /// location; `rparen` the closing parenthesis.
    #[allow(
        clippy::too_many_lines,
        reason = "callee checks, argument conversion and default arguments in [expr.call] order"
    )]
    pub fn act_on_call(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        // A callee from `act_on_call_name`: its function is referenced
        // unless the call stays dependent ([temp.dep.candidate]).
        if let Some(i) = self.deferred_call_refs.iter().position(|&(e, _)| e == callee) {
            let (_, d) = self.deferred_call_refs.swap_remove(i);
            if !args.iter().any(|&a| self.expr_is_type_dependent(a)) {
                self.ast.decls.set_flags(d, DeclFlags::REFERENCED);
            }
        }
        // [dcl.fct.def.delete]/2: a call of a deleted function by its
        // unqualified name (spec/sema-class.md §4.3).
        if self.special.has_pending() {
            let invalid = self.is_err_expr(callee) || args.iter().any(|&a| self.is_err_expr(a));
            let dependent = self.expr_is_type_dependent(callee)
                || args.iter().any(|&a| self.expr_is_type_dependent(a));
            self.act_on_deleted_callee(callee, dependent, invalid);
        }
        // [expr.call]/5: a call of a pseudo-destructor has type `void`
        // (clang's `ActOnCallExpr`). With a type-dependent object or
        // destroyed type the call is a type-dependent expression
        // ([temp.dep.expr]/1; clang's `CallExpr` takes the callee's
        // dependence), so the operators, `sizeof`, initializations and
        // calls that use it are formed at instantiation: kiln gives it the
        // dependent type, which the AST dump prints as `void`
        // (spec/sema-class.md "Dependent pseudo-destructor calls").
        if self.ast.exprs.get(self.ignore_parens(callee)).kind()
            == ExprKind::CXXPseudoDestructorExpr
        {
            if !args.is_empty() {
                let d = self.diag(DiagId::sema_pseudo_dtor_call_with_args, self.begin_loc(callee));
                self.emit(d);
            }
            let ty = if self.expr_is_type_dependent(callee) {
                QualType::DEPENDENT
            } else {
                QualType::VOID
            };
            return self.builder().call(
                callee,
                &[],
                ty,
                ValueCategory::PRValue,
                loc,
                rparen,
                ExprFlags::NONE,
            );
        }
        let mut kids = vec![callee];
        kids.extend_from_slice(args);
        // Braced-init-list arguments (placeholders until the parameter
        // types are known, [dcl.init.list]/1).
        let mut braced: Vec<Option<InitList>> =
            args.iter().map(|&a| self.take_braced_arg(a)).collect();
        if self.is_err_expr(callee) || args.iter().any(|&a| self.is_err_expr(a)) {
            return self.recover_err(&kids, loc);
        }
        // [temp.dep.expr]/3: a braced-init-list argument with a
        // type-dependent initializer (at any depth) makes the call
        // dependent.
        let args_dep = args.iter().any(|&a| self.expr_is_type_dependent(a))
            || braced.iter().flatten().any(|l| self.init_list_type_dependent(l));
        // A callee named by a `DeclRefExpr` before the call (clang's
        // non-overloaded name), not resolved by it.
        let direct = self.ast.exprs.get(self.ignore_parens(callee)).kind() == ExprKind::DeclRefExpr;
        let callee = if self.ast.exprs.get(self.ignore_parens(callee)).kind()
            == ExprKind::UnresolvedLookupExpr
            && !self.expr_is_type_dependent(callee)
            && !args_dep
        {
            // `(f)(x)`, `(f<A>)(x)`: the parentheses stay around the
            // resolved name (clang's `FixOverloadedFunctionReference`).
            match self.resolve_unresolved_call(callee, args, &braced) {
                Some(e) if !self.is_err_expr(e) => self.rebuild_parens(callee, e),
                _ => return self.recover_err(&kids, loc),
            }
        } else if self.member_overload(callee).is_some() && !args_dep {
            match self.resolve_member_call(callee, args, &braced) {
                Some(e) if !self.is_err_expr(e) => self.rebuild_parens(callee, e),
                _ => return self.recover_err(&kids, loc),
            }
        } else {
            callee
        };
        if self.expr_is_type_dependent(callee) || args_dep {
            // A braced-init-list argument is kept as written (nested
            // lists included) and list-initializes its parameter at
            // instantiation ([dcl.init.list]/1, [temp.dep.expr]/3).
            let args: Vec<ExprId> = args
                .iter()
                .zip(&braced)
                .map(|(&a, b)| b.as_ref().map_or(a, |l| self.dependent_init_list(l)))
                .collect();
            let c = self.ast.exprs.get(callee);
            if c.kind() == ExprKind::UnresolvedLookupExpr
                && self.ast.templates.expr_targs.contains(callee)
                && self.ast.unresolved_decls(callee).is_empty()
                && self.ovl.binding_get != Some(callee)
            {
                self.diagnose_adl_only_template_id(c.loc());
            }
            self.note_implicit_this_of_dependent_call(callee);
            return self.dependent_call(callee, &args, loc, rparen);
        }
        // [over.call.object]: a call of an object of class type.
        if self.ast.types.as_record(self.ety(callee)).is_some()
            && self.member_overload(callee).is_none()
        {
            let mut ops = vec![callee];
            ops.extend_from_slice(args);
            let mut br = vec![None];
            br.append(&mut braced);
            if let OperResult::Done(e) =
                self.try_overloaded_operator(OperForm::Call, &ops, &br, loc, rparen)
            {
                return e;
            }
            braced = br.split_off(1);
        }
        if let Some(b) = self.builtin_callee(callee)
            && let Some(id) = self.ast.decls.builtin_id(b)
        {
            self.atomic_flag_order_check(id, args);
        }
        if let Some(b) = self.builtin_callee(callee)
            && let Some(id) = self.ast.decls.builtin_id(b)
            && kiln_ast::decl::builtin::is_custom(id)
        {
            let has_braced = braced.iter().any(Option::is_some);
            return self.builtin_custom_call(callee, b, id, args, has_braced, loc, rparen);
        }
        // A member function named by a member access with no overload set
        // to resolve (`x.f()`): a placeholder return type is deduced first,
        // instantiating the definition of a class template member (clang's
        // `DiagnoseUseOfDecl` -> `DeduceReturnType`).
        {
            let x = self.ignore_parens(callee);
            if self.ast.exprs.get(x).kind() == ExprKind::MemberExpr {
                let m = self.ast.exprs.decl(x);
                let at = self.ast.exprs.get(x).loc();
                if self.ast.decls.get(m).kind().is_function() && self.check_undeduced_use(m, at) {
                    return self.recover_err(&kids, loc);
                }
            }
        }
        if let Some(e) = self.try_member_call(callee, args, &mut braced, loc, rparen) {
            return e;
        }
        // The function the callee names, if any (calls through pointers
        // report conversions directly, not as overload candidates): a
        // function named by its name, or a static member function named
        // by a member access (`x.f()`, `this->f()`; a non-static one is
        // a bound member, called above), whose default arguments, format
        // checks and immediate invocation are those of a direct call
        // ([expr.ref], [dcl.fct.default]/1).
        let named = {
            let x = self.ignore_parens(callee);
            match self.ast.exprs.get(x).kind() {
                ExprKind::DeclRefExpr | ExprKind::MemberExpr => Some(self.ast.exprs.decl(x)),
                _ => None,
            }
            .filter(|&d| self.ast.decls.function(d).is_some())
        };
        // A function named directly whose definition failed to
        // instantiate when this reference odr-used it (it is now invalid):
        // the call is erroneous, undiagnosed (clang builds a
        // `RecoveryExpr`; a member call is not affected). A function
        // invalid only for an invalid parameter (an erroneous default
        // argument) is still called: its arguments are checked
        // ([dcl.fct.default]; clang resolves it like any other).
        if direct
            && named.is_some_and(|d| {
                self.ast.decls.get(d).has(DeclFlags::INVALID) && !self.invalid_by_param(d)
            })
        {
            return self.recover_err(&kids, loc);
        }
        let f = match self.builtin_callee(callee) {
            Some(b) => self.builtin_fn_to_ptr(callee, b),
            // A builtin alias (WP-810) may be called: its decay is not a use
            // `check_builtin_fn_use` rejects.
            None if self.is_builtin_alias_ref(callee) => {
                let t = self.ety(callee);
                let p = self.ast.types.pointer_to(t);
                conv::raw_cast(
                    self,
                    CastKind::FunctionToPointerDecay,
                    callee,
                    p,
                    ValueCategory::PRValue,
                    None,
                )
            }
            None => conv::decay(self, callee),
        };
        let ft = self.ety(f);
        // A pointer to member function is not callable by itself: it
        // needs an object (`(o.*pmf)()`, [expr.call]/1).
        let fn_ty = (!self.is_member_pointer(ft))
            .then(|| self.pointee_sugared(ft))
            .flatten()
            .filter(|p| self.ast.types.is_function(*p));
        let Some(fn_ty) = fn_ty else {
            let msg =
                self.diag(DiagId::sema_call_not_function, loc).arg(self.ty_arg(self.ety(callee)));
            self.emit(msg);
            return self.recover_err(&kids, loc);
        };
        let Some(proto) = self.sugared_proto(fn_ty) else {
            self.unsupported("calls through this function type", loc);
            return self.recover_err(&kids, loc);
        };
        // A builtin is not overloadable, and its diagnostics name no
        // declaration (clang's `ConvertArgumentsForCall`).
        let adl_single = self.ovl.adl_single.take();
        let callee_info = named.map(|d| CallCallee {
            decl: d,
            overloadable: self.kind_of(d) == DeclKind::Function
                && self.ast.decls.builtin_id(d).is_none()
                && adl_single == Some(callee),
            enable_if: true,
        });
        let Some(conv_args) =
            self.convert_call_args(callee_info, callee, args, &mut braced, &proto, loc, rparen)
        else {
            // A one-candidate overload set: clang resolved it as a set.
            if adl_single == Some(callee) {
                self.note_call_recovery(Some(proto.ret()));
            }
            return self.recover_err(&kids, loc);
        };
        if named.and_then(|d| self.ast.decls.builtin_id(d))
            == Some(kiln_ast::decl::builtin::EXPECT_WITH_PROBABILITY)
            && !self.check_expect_probability(conv_args[2])
        {
            return self.recover_err(&kids, loc);
        }
        if let Some(id) = named.and_then(|d| self.ast.decls.builtin_id(d))
            && !self.check_builtin_ice_args(id, callee, &conv_args)
        {
            return self.recover_err(&kids, loc);
        }
        if named.and_then(|d| self.ast.decls.builtin_id(d))
            == Some(kiln_ast::decl::builtin::PREFETCH)
            && !self.check_prefetch(callee, &conv_args, rparen)
        {
            return self.recover_err(&kids, loc);
        }
        if let Some(d) = named {
            // A function invalid for an invalid parameter: its arguments
            // were checked, the call itself is erroneous and undiagnosed
            // (not constant-evaluated).
            if self.ast.decls.get(d).has(DeclFlags::INVALID) && self.invalid_by_param(d) {
                return self.recover_err(&kids, loc);
            }
            self.check_format_attr(d, &conv_args);
            self.check_absolute_value(d, callee, &conv_args);
        }
        let at = self.begin_loc(callee);
        let (ty, vk) = self.call_result_of(proto.ret(), loc, named, at);
        // `__builtin_assume(e)` never evaluates `e`: with possible side
        // effects the assumption is dropped (clang `-Wassume`).
        let mut flags = ExprFlags::NONE;
        if named.and_then(|d| self.ast.decls.builtin_id(d)) == Some(kiln_ast::decl::builtin::ASSUME)
            && let Some(&c) = conv_args.first()
            && self.has_possible_side_effects(c)
        {
            let msg = self.diag(DiagId::sema_assume_side_effects, self.begin_loc(c));
            self.emit(msg);
            flags = ExprFlags::ASSUME_IGNORED;
        }
        let call = self.builder().call(f, &conv_args, ty, vk, loc, rparen, flags);
        let call = self.maybe_bind_temporary(call);
        match named {
            Some(d) => {
                let r = self.ignore_parens(callee);
                self.check_immediate_invocation(call, d, Some(r))
            }
            None => call,
        }
    }

    /// Converts the arguments of a call through a function of type `proto`
    /// ([expr.call]/7), appending the default arguments of the omitted
    /// trailing parameters ([dcl.fct.default]/1) when the callee is known.
    /// `braced` holds the braced-init-list arguments, parallel to `args`.
    /// Diagnoses and returns `None` on an arity or conversion error.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "one per part of the call; one step per paragraph of [expr.call]"
    )]
    pub(crate) fn convert_call_args(
        &mut self,
        named: Option<CallCallee>,
        callee: ExprId,
        args: &[ExprId],
        braced: &mut [Option<InitList>],
        proto: &kiln_ast::types::FuncProto,
        loc: Loc,
        rparen: Loc,
    ) -> Option<Vec<ExprId>> {
        let params: Vec<QualType> = self.ast.types.params(proto).to_vec();
        let variadic = proto.info().variadic();
        let n = params.len();
        let have = args.len();
        // Default arguments of the named function ([dcl.fct.default]/1;
        // a redeclaration holds the ones it inherits, /4).
        let pdecls: Vec<DeclId> = named
            .and_then(|c| self.ast.decls.function(c.decl))
            .map(|x| self.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        // [temp.inst]/12: the default arguments this call uses are
        // instantiated now.
        if let Some(c) = named {
            // Where the instantiation is required: the call's start; at
            // clang 22 a member call's member name (`getExprLoc`).
            let poi = if self.clang_compat >= 22
                && self.ast.exprs.get(callee).kind() == ExprKind::MemberExpr
                && !self.is_static_method(c.decl)
            {
                self.eloc(callee)
            } else {
                self.begin_loc(callee)
            };
            for &p in pdecls.iter().take(n).skip(have) {
                if self.diagnose_unparsed_default_arg(c.decl, p, callee) {
                    return None;
                }
                if self.inst.lazy_defaults.contains_key(&p)
                    && !self.instantiate_lazy_default(c.decl, p, poi)
                {
                    return None;
                }
            }
        }
        let defaults: Vec<Option<ExprId>> = (0..n)
            .map(|i| {
                pdecls.get(i).and_then(|&p| self.ast.decls.param(p)).and_then(|x| x.default_arg)
            })
            .collect();
        let min = defaults.iter().rposition(Option::is_none).map_or(0, |i| i + 1);
        if have < min || (have > n && !variadic) {
            let few = have < min;
            match named {
                Some(c) if c.overloadable => {
                    let d = c.decl;
                    let msg = self
                        .diag(DiagId::sema_ovl_no_viable_function_in_call, self.begin_loc(callee))
                        .arg(self.decl_name(d))
                        .range(self.expr_range(callee));
                    let detail = self.arity_detail(d, n, min, have, variadic);
                    let fl = self.ast.decls.get(d).loc();
                    let note = self.diag(DiagId::note_ovl_candidate_not_viable, fl).arg(detail);
                    self.emit(msg.with_note(note));
                }
                _ => {
                    let id = match (few, variadic || min != n) {
                        (true, true) => DiagId::sema_call_too_few_args_at_least,
                        (true, false) => DiagId::sema_call_too_few_args,
                        (false, _) => DiagId::sema_call_too_many_args,
                    };
                    let expected = if few { min } else { n };
                    // clang's direct-call diagnostics name the callee in a
                    // note and point at the `)` or the first extra argument.
                    let at = match named {
                        Some(_) if few => rparen,
                        Some(_) => self.begin_loc(args[n]),
                        None => loc,
                    };
                    let mut msg = self.diag(id, at).arg(expected).arg(have);
                    if let Some(c) = named.filter(|c| self.ast.decls.builtin_id(c.decl).is_none()) {
                        let d = c.decl;
                        let note = self
                            .diag(DiagId::note_entity_declared_here, self.ast.decls.get(d).loc())
                            .arg(format!("'{}'", self.decl_name(d)));
                        msg = msg.with_note(note);
                    }
                    self.emit(msg);
                }
            }
            return None;
        }
        let mut conv_args = Vec::with_capacity(have.max(n));
        for (i, &a) in args.iter().enumerate() {
            if let Some(l) = braced[i].take() {
                if i >= n {
                    let kind = self.variadic_call_kind(named.map(|c| c.decl), callee);
                    self.braced_variadic_argument(&l, kind);
                    return None;
                }
                conv_args.push(self.braced_argument(
                    named.map(|c| c.decl),
                    callee,
                    &l,
                    params[i],
                    (i, pdecls.get(i).copied()),
                )?);
                continue;
            }
            if i < n {
                let pt = conv::strip_cv(self, params[i]);
                let to = if self.ast.types.is_reference(params[i]) { params[i] } else { pt };
                if self.ast.types.as_record(to).is_some()
                    && !self.is_err_expr(a)
                    && !self.expr_is_type_dependent(a)
                {
                    // A one-candidate overload set: a bad conversion
                    // sequence makes the candidate not viable
                    // ([over.match.viable]/4) where the direct path
                    // diagnoses the initialization.
                    if let Some(c) = named.filter(|c| c.overloadable) {
                        let info = self.arg_info(a);
                        let opts = crate::overload::IcsOptions::CANDIDATE_ARG;
                        if self.try_implicit_conversion(&info, to, opts).is_bad() {
                            self.bad_argument(c.decl, callee, a, params[i], i);
                            return None;
                        }
                    }
                    let note = match pdecls.get(i) {
                        Some(&p) => crate::class::DeletedAs::Param(self.param_decl_note(p)),
                        None => crate::class::DeletedAs::Call,
                    };
                    let at = self.begin_loc(a);
                    let built = if self.ast.types.as_record(self.ety(a)).is_some() {
                        self.construct_from_class(a, to, ConvCtx::Argument, at, note)
                    } else {
                        self.convert_to_class(a, to, ConvCtx::Argument, at, note)
                    };
                    if let Some(x) = built {
                        conv_args.push(x);
                        continue;
                    }
                }
                if named.is_some_and(|c| c.overloadable)
                    && matches!(self.ast.types.canonical_view(to), TypeView::LValueRef(_))
                    && self.ast.types.as_record(self.ety(a)).is_some()
                    && !self.is_err_expr(a)
                {
                    // The one-candidate set's conversion sequence: a
                    // deleted conversion function to a reference is no
                    // conversion ([over.match.ref], clang's
                    // `FindConversionForRefInit`).
                    let info = self.arg_info(a);
                    let opts = crate::overload::IcsOptions::CANDIDATE_ARG;
                    if self.try_implicit_conversion(&info, to, opts).is_bad()
                        && let Some(c) = named
                    {
                        self.bad_argument(c.decl, callee, a, params[i], i);
                        return None;
                    }
                }
                match conv::implicit_convert(self, a, to, ConvCtx::Argument) {
                    Ok(x) => conv_args.push(x),
                    Err(err) => {
                        // clang reports an ambiguous or deleted
                        // user-defined conversion of the selected
                        // function's argument as such, and so does a
                        // callee without overload resolution (its direct
                        // path, noting the parameter, unless a builtin);
                        // otherwise "no matching function".
                        let as_such = matches!(
                            err.id,
                            DiagId::sema_ovl_ambiguous_conversion
                                | DiagId::sema_ref_init_ambiguous
                                | DiagId::sema_conv_deleted_ctor
                                | DiagId::sema_complex_to_real_cxx
                                // [except.spec]/5 (C++14): checked once
                                // the conversion is performed.
                                | DiagId::sema_incompatible_exception_specs
                                | DiagId::sema_deep_exception_specs_differ
                        );
                        if let Some(c) = named.filter(|c| c.overloadable && !as_such) {
                            self.bad_argument(c.decl, callee, a, params[i], i);
                        } else {
                            let mut d = err.diag(self, a);
                            let builtin =
                                named.is_some_and(|c| self.ast.decls.builtin_id(c.decl).is_some());
                            if !builtin
                                && err.at.is_none()
                                && let Some(&p) = pdecls.get(i)
                            {
                                d = d.with_note(self.param_decl_note(p));
                            }
                            self.emit(d);
                        }
                        return None;
                    }
                }
            } else {
                conv_args.push(self.default_argument_promotion(a));
            }
        }
        for (i, &d) in defaults.iter().enumerate().skip(have) {
            let (Some(p), Some(e)) = (pdecls.get(i).copied(), d) else { break };
            let x = *self.ast.exprs.get(e);
            let used = self.ctx;
            let arg = self.builder().default_arg(p, used, x.ty(), x.value_category(), Loc::INVALID);
            let call_begin = self.begin_loc(callee);
            self.note_default_arg_use(arg, p, call_begin);
            self.note_default_use(arg, p, call_begin);
            conv_args.push(arg);
        }
        if have > n {
            let kind = self.variadic_call_kind(named.map(|c| c.decl), callee);
            self.check_variadic_args(&conv_args[n..have], kind);
        }
        if let Some(c) = named
            && c.enable_if
            && !self.check_enable_if_direct(c.decl, callee, &conv_args, loc)
        {
            return None;
        }
        // clang `checkCall`: also a static member function named by a
        // member access (`x.s(1)`), which is not `named` here.
        let checked = named.map(|c| c.decl).or_else(|| {
            let x = self.ignore_parens(callee);
            (self.ast.exprs.get(x).kind() == ExprKind::MemberExpr)
                .then(|| self.ast.exprs.decl(x))
                .filter(|&d| self.is_static_method(d))
        });
        if let Some(f) = checked {
            self.check_diagnose_if_call(f, &conv_args, rparen);
        }
        Some(conv_args)
    }

    /// How clang names the callee of a variadic call in its argument
    /// diagnostics (`getVariadicCallType`, the `{function|block|method|
    /// constructor}` select): 3 for a constructor, 2 for an instance
    /// member function or a call through a bound member (`(x.*pmf)(...)`),
    /// 0 otherwise.
    pub(crate) fn variadic_call_kind(&self, decl: Option<DeclId>, callee: ExprId) -> u8 {
        match decl {
            Some(d) if self.kind_of(d) == DeclKind::CXXConstructor => 3,
            Some(d) if self.is_instance_member(d) => 2,
            None if self.ety(callee) == QualType::BOUND_MEMBER => 2,
            Some(_) | None => 0,
        }
    }

    /// The checks of the promoted arguments `args` passed for the
    /// ellipsis ([expr.call]/12, clang `checkVariadicArgument`), made
    /// once the call is built (neither stops it): an argument of type
    /// `void` is an error ("cannot pass expression of type 'void' to
    /// variadic ..."); an object of class type with a non-trivial copy
    /// constructor, move constructor or destructor (conditionally-
    /// supported) is the `-Wnon-pod-varargs` error-by-default warning, a
    /// runtime-behavior diagnostic (not in an unevaluated operand or a
    /// constant-evaluated context). Both point at the argument's begin;
    /// `kind` is [`Sema::variadic_call_kind`]'s. clang skips the checks
    /// in a dependent context and makes them when the template is
    /// instantiated: an argument that would be diagnosed is recorded in
    /// `vararg_recheck`, and an instantiation that shares its call checks
    /// it then ([`Sema::mark_shared_expr_referenced`]).
    pub(crate) fn check_variadic_args(&mut self, args: &[ExprId], kind: u8) {
        let defer = self.in_template();
        for &a in args {
            if self.is_err_expr(a) || self.expr_is_type_dependent(a) {
                continue;
            }
            let t = self.ety(a);
            if defer {
                if self.ast.types.is_void(t) || self.vararg_non_trivial(t) {
                    let _ = self.vararg_recheck.insert(a, kind);
                }
                continue;
            }
            if self.ast.types.is_void(t) {
                let msg = self
                    .diag(DiagId::sema_cannot_pass_to_vararg, self.begin_loc(a))
                    .arg(self.ty_arg(t))
                    .arg(kind);
                self.emit(msg);
            } else if self.vararg_non_trivial(t) {
                let msg = self
                    .diag(DiagId::sema_non_pod_vararg, self.begin_loc(a))
                    .arg(self.ty_arg(t))
                    .arg(kind);
                self.diag_runtime_behavior_now(msg);
            }
        }
    }

    /// clang `isValidVarArgType` returning `VAK_Undefined` in C++11 and
    /// later: a complete class with a non-trivial copy constructor, move
    /// constructor or destructor.
    fn vararg_non_trivial(&self, t: QualType) -> bool {
        use crate::class::SpecialKind as K;
        let Some(r) = self.ast.types.as_record(t) else { return false };
        let Some(r) = self.ast.definition(r) else { return false };
        !self.has_trivial(r, K::CopyCtor)
            || (self.has_special(r, K::MoveCtor) && !self.has_trivial(r, K::MoveCtor))
            || !self.has_trivial(r, K::Dtor)
    }

    /// The type and value category of a call returning `ret`
    /// ([expr.call]/14), diagnosing an incomplete class result.
    pub(crate) fn call_result(&mut self, ret: QualType, loc: Loc) -> (QualType, ValueCategory) {
        self.call_result_of(ret, loc, None, loc)
    }

    /// [`Self::call_result`] for a call at `loc` of `callee` (the named
    /// function, if any) beginning at `at`: an incomplete class prvalue
    /// result is clang's `CheckCallReturnType` error at `at`, deferred to
    /// the end of an enclosing `decltype` operand ([dcl.type.decltype]/2).
    pub(crate) fn call_result_of(
        &mut self,
        ret: QualType,
        loc: Loc,
        callee: Option<DeclId>,
        at: Loc,
    ) -> (QualType, ValueCategory) {
        let (ty, vk) = if self.ast.types.is_reference(ret) {
            // [expr.call]/14: an rvalue reference to function is an
            // lvalue too (clang `Expr::getValueKindForType`).
            let r = self.non_reference(ret);
            let lvalue = matches!(self.ast.types.canonical_view(ret), TypeView::LValueRef(_))
                || self.ast.types.is_function(r);
            (r, if lvalue { ValueCategory::LValue } else { ValueCategory::XValue })
        } else if self.ast.types.as_record(ret).is_some() || self.ast.types.is_dependent(ret) {
            // [expr.type]/2: only a non-class prvalue loses its
            // cv-qualifiers; `const X f()` yields a `const X` prvalue
            // (clang `QualType::getNonLValueExprType`).
            (ret, ValueCategory::PRValue)
        } else {
            (conv::strip_cv(self, ret), ValueCategory::PRValue)
        };
        if vk == ValueCategory::PRValue && self.ast.types.as_record(ty).is_some() {
            let call = crate::deduce::DeferredCall { loc, ty, callee, at };
            let unevaluated = self.unevaluated;
            if let Some((depth, calls)) = self.decltype_calls.last_mut()
                && *depth == unevaluated
            {
                calls.push(call);
            } else if !self.check_call_return(call) {
                // An invalid call (clang's `ExprError`): no follow-on
                // diagnostics.
                return (QualType::ERROR, vk);
            }
        }
        (ty, vk)
    }

    /// clang's "candidate function not viable" detail for an arity
    /// mismatch (`DiagnoseArityMismatch`): `n` parameters, `min` of them
    /// without a default argument, `have` arguments.
    pub(crate) fn arity_detail(
        &self,
        d: DeclId,
        n: usize,
        min: usize,
        have: usize,
        variadic: bool,
    ) -> String {
        // 0: at least, 1: at most, 2: exactly.
        let (mode, count) = if have < min {
            (if min != n || variadic { 0 } else { 2 }, min)
        } else {
            (if min == n { 2 } else { 1 }, n)
        };
        let pname = self
            .ast
            .decls
            .function(d)
            .and_then(|f| self.ast.lists.at::<DeclId>(f.params, 0))
            .map(|p| self.decl_name(p))
            .unwrap_or_default();
        if count == 1 && !pname.is_empty() {
            let provided = match have {
                0 => "no arguments were provided".to_owned(),
                1 => "1 argument was provided".to_owned(),
                k => format!("{k} arguments were provided"),
            };
            let how = [
                "requires at least argument",
                "allows at most single argument",
                "requires single argument",
            ][mode];
            return format!("{how} '{pname}', but {provided}");
        }
        let provided =
            if have == 1 { "1 was provided".to_owned() } else { format!("{have} were provided") };
        let need = ["at least ", "at most ", ""][mode];
        let plural = if count == 1 { "argument" } else { "arguments" };
        format!("requires {need}{count} {plural}, but {provided}")
    }

    /// The default argument promotions for a variadic argument
    /// ([expr.call]/12).
    pub(crate) fn default_argument_promotion(&mut self, a: ExprId) -> ExprId {
        let x = conv::decay(self, a);
        let t = self.ety(x);
        if self.ast.types.builtin(t) == Some(BuiltinType::Float) {
            return conv::implicit_convert(self, x, QualType::DOUBLE, ConvCtx::Argument)
                .unwrap_or(x);
        }
        if self.is_integral_or_enum(t) && !conv::is_scoped_enum(self, t) {
            return conv::integral_promotion(self, x);
        }
        // [expr.call]/12 (CWG722): a `std::nullptr_t` argument is
        // converted to `void *`; clang 22 builds the `NullToPointer` cast.
        if self.clang_compat >= 22 && self.is_nullptr_type(t) {
            let vp = self.ast.types.pointer_to(QualType::VOID);
            return conv::implicit_convert(self, x, vp, ConvCtx::Argument).unwrap_or(x);
        }
        x
    }

    /// `e.name` / `e->name` ([expr.ref]); `loc` is the name, `op_loc` the
    /// operator.
    pub fn act_on_member(
        &mut self,
        base: ExprId,
        name: Symbol,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        self.act_on_member_dn(base, DeclName::ident(name), loc, op_loc, arrow)
    }

    /// [`Sema::act_on_member`] naming any unqualified-id: an identifier,
    /// `operator@` or `operator T` (WP-121; destructor names go through
    /// [`Sema::act_on_member_destructor`]).
    pub fn act_on_member_dn(
        &mut self,
        base: ExprId,
        name: DeclName,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        self.act_on_member_q(base, None, name, loc, op_loc, arrow)
    }

    /// [`Sema::act_on_member_dn`] with an optional nested-name-specifier
    /// before the name (`e.N::m`, `p->B::f`, `e.::N::m`; [expr.ref]/4,
    /// WP-121).
    /// clang's `tryToRecoverWithCall` in `LookupMemberExpr`: a base naming
    /// a function callable with no arguments whose result is a class (for
    /// `.`) or a pointer to one (for `->`) gets "base of member reference
    /// is a function; perhaps you meant to call it with no arguments?"
    /// (fix-it `()`) and is recovered as that call ([expr.ref]/1).
    fn member_base_needs_call(&mut self, base: ExprId, arrow: bool) -> Option<ExprId> {
        let x = self.ignore_parens(base);
        if self.ast.exprs.get(x).kind() != ExprKind::DeclRefExpr {
            return None;
        }
        let d = self.ast.exprs.decl(x);
        if !matches!(self.kind_of(d), DeclKind::Function | DeclKind::CXXMethod)
            || self.is_class_member_ref(d)
        {
            return None;
        }
        let fty = self.ast.decls.get(d).ty()?;
        let TypeView::Function(p) = self.ast.types.view(self.ast.types.desugar_top(fty).ty())
        else {
            return None;
        };
        let ret = p.ret();
        let params = self.param_decls(d);
        if !params.iter().all(|&q| self.param_has_default(q)) || self.ast.types.is_dependent(ret) {
            return None;
        }
        let plausible = if arrow {
            self.ast.types.is_pointer(ret)
                && self
                    .ast
                    .types
                    .pointee(ret)
                    .is_some_and(|t| self.ast.types.as_record(t).is_some())
        } else {
            self.ast.types.as_record(ret).is_some()
        };
        if !plausible {
            return None;
        }
        let at = self.expr_loc(base);
        let end = self.expr_end_loc(base);
        let after = self.token_end(end).unwrap_or(end);
        let msg = self
            .diag(DiagId::sema_member_reference_needs_call, at)
            .fixit(kiln_diag::FixIt::insert(after, "()"));
        self.emit(msg);
        Some(self.act_on_call(base, &[], after, after))
    }

    pub fn act_on_member_q(
        &mut self,
        base: ExprId,
        qual: Option<crate::using::Qualifier>,
        name: DeclName,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        if self.is_err_expr(base) {
            return self.recover_err(&[base], loc);
        }
        let base = self.resolve_single_overload_candidate(base);
        if let Some(call) = self.member_base_needs_call(base, arrow) {
            return self.act_on_member_q(call, qual, name, loc, op_loc, arrow);
        }
        // [expr.ref]/7.3.2: a bound member function base (`g.s.x` for a
        // member function `s`) is recovered by calling it, as clang's
        // `PerformMemberExprBaseConversion` does.
        if self.ety(base) == QualType::BOUND_MEMBER {
            let call = self.recover_bound_member(base);
            if self.is_err_expr(call) {
                return self.recover_err(&[call], loc);
            }
            return self.act_on_member_q(call, qual, name, loc, op_loc, arrow);
        }
        if let Some(e) = self.try_dependent_member_q(base, qual, name, None, false, loc, arrow) {
            if op_loc != loc {
                let _ = self.ast.templates.member_op_loc.insert(e, op_loc);
            }
            return e;
        }
        let (base, arrow) = match self.member_arrow_base(base, loc, op_loc, arrow) {
            Ok(x) => x,
            Err(e) => return e,
        };
        // clang `LookupMemberExpr`: `v.xy` / `p->xy` on an ext vector.
        if qual.is_none()
            && let Some(id) = name.as_ident()
            && let Some(vt) = self.ext_vector_base(base, arrow)
        {
            return self.ext_vector_member(base, vt, id, loc, op_loc, arrow);
        }
        let (b, obj_ty, def) = match self.member_object(base, loc, op_loc, arrow) {
            Ok(x) => x,
            Err(e) => return e,
        };
        self.member_access(base, b, obj_ty, def, arrow, name, qual, loc)
    }

    /// [over.ref]/1: `x->m` on a class `x` is `(x.operator->())->m`;
    /// the base and whether the access is still an arrow. `Err` carries
    /// the recovery expression.
    pub(crate) fn member_arrow_base(
        &mut self,
        base: ExprId,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> Result<(ExprId, bool), ExprId> {
        if arrow && self.ast.types.as_record(self.ety(base)).is_some() {
            match self.arrow_base(base, op_loc) {
                crate::overload::arrow::ArrowBase::Arrow(e) => Ok((e, true)),
                crate::overload::arrow::ArrowBase::Dot => Ok((base, false)),
                crate::overload::arrow::ArrowBase::Err => Err(self.recover_err(&[base], loc)),
            }
        } else {
            Ok((base, arrow))
        }
    }

    /// The object of a class member access `base.m` / `base->m`
    /// ([expr.ref]/2-4): the (decayed) base, the object type and the
    /// complete class definition. `Err` carries the recovery expression
    /// after the diagnostic.
    pub(crate) fn member_object(
        &mut self,
        base: ExprId,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> Result<(ExprId, QualType, DeclId), ExprId> {
        let (b, obj_ty) = if arrow {
            // clang `LookupMemberExpr`: the pointee or element type, or a
            // function type as written.
            let ot = self.ety(base);
            let pointee = if self.ast.types.is_pointer(ot) {
                self.pointee_sugared(ot)
            } else if self.ast.types.is_array(ot) {
                self.ast.types.element(ot)
            } else if self.ast.types.is_function(ot) {
                Some(ot)
            } else {
                None
            };
            let Some(p) = pointee else {
                let msg =
                    self.diag(DiagId::sema_operator_arrow_not_pointer, loc).arg(self.ty_arg(ot));
                self.emit(msg);
                return Err(self.recover_err(&[base], loc));
            };
            if self.ast.types.as_record(p).is_none() {
                let msg = self
                    .diag(DiagId::sema_member_reference_struct_union, op_loc)
                    .arg(self.ty_arg(p));
                self.emit(msg);
                return Err(self.recover_err(&[base], loc));
            }
            (conv::decay(self, base), p)
        } else {
            let bt = self.ety(base);
            if self.ast.types.as_record(bt).is_none() {
                let ptr_rec = self
                    .ast
                    .types
                    .pointee(bt)
                    .is_some_and(|p| self.ast.types.as_record(p).is_some())
                    && self.ast.types.is_pointer(bt);
                let msg = if ptr_rec {
                    self.diag(DiagId::sema_member_reference_suggestion, op_loc)
                        .arg(self.ty_arg(bt))
                        .arg("0")
                } else {
                    self.diag(DiagId::sema_member_reference_struct_union, op_loc)
                        .arg(self.ty_arg(bt))
                };
                self.emit(msg);
                return Err(self.recover_err(&[base], loc));
            }
            (base, bt)
        };
        let rec = self.ast.types.as_record(obj_ty).expect("checked above");
        // [expr.ref]/4 (clang diagnoses at the operator). A class being
        // defined is complete enough only where `this` may appear outside
        // a function body: its members declared so far are found (a
        // trailing return type naming `this->m`, [class.mem]/7).
        if self.instantiation_fails(obj_ty, op_loc) {
            return Err(self.recover_err(&[base], loc));
        }
        // A member access the pattern resolved into the current
        // instantiation is formed again with the member it found, without
        // a completeness check (clang `TreeTransform::RebuildMemberExpr`):
        // a friend's trailing `decltype(l.base())` on the class being
        // instantiated.
        let resolved = std::mem::take(&mut self.pattern_member_access)
            && self.defining.iter().any(|&d| self.ast.first(d) == self.ast.first(rec));
        let complete =
            resolved || self.this_outside_member_body(rec) || self.require_complete(obj_ty, op_loc);
        let Some(def) = self.ast.definition(rec).filter(|_| complete) else {
            let msg = self
                .diag(DiagId::sema_member_reference_incomplete, op_loc)
                .arg(self.ty_arg(obj_ty));
            self.emit(self.incomplete_note(msg, obj_ty));
            return Err(self.recover_err(&[base], loc));
        };
        Ok((b, obj_ty, def))
    }

    /// A name declared by an anonymous union outside a class
    /// ([class.union.anon]/1): a member access chain rooted at the
    /// union's implicit variable.
    fn anonymous_member_ref(&mut self, d: DeclId, loc: Loc) -> ExprId {
        let chain: Vec<DeclId> =
            self.ast.lists.iter::<DeclId>(self.ast.indirect_field_chain(d)).collect();
        self.ast.decls.set_flags(d, DeclFlags::REFERENCED);
        let mut cur = self.build_decl_ref(chain[0], loc);
        for &f in &chain[1..] {
            let q = self.ast.types.effective_quals(self.ety(cur));
            cur = self.field_access(cur, f, q, false, loc);
        }
        cur
    }

    pub(crate) fn field_access(
        &mut self,
        base: ExprId,
        f: DeclId,
        quals: kiln_ast::types::Quals,
        arrow: bool,
        loc: Loc,
    ) -> ExprId {
        self.field_access_extra(base, f, quals, arrow, loc, None)
    }

    /// [`Self::field_access`] recording `extra` (the qualifier of
    /// `this->B::m`, which instantiation looks up in again: [class.qual]).
    pub(crate) fn field_access_extra(
        &mut self,
        base: ExprId,
        f: DeclId,
        quals: kiln_ast::types::Quals,
        arrow: bool,
        loc: Loc,
        extra: Option<kiln_ast::expr::DeclRefExtra>,
    ) -> ExprId {
        self.ast.decls.set_flags(f, DeclFlags::REFERENCED);
        let declared = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let mutable = self.ast.decls.field(f).is_some_and(kiln_ast::decl::FieldData::is_mutable);
        let ty = if self.ast.types.is_reference(declared) {
            self.non_reference(declared)
        } else {
            let mut q = quals;
            if mutable {
                q = q - kiln_ast::types::Quals::CONST;
            }
            if q.is_empty() { declared } else { self.ast.types.qualify(declared, q) }
        };
        self.build_member_expr(base, f, ty, arrow, loc, extra)
    }

    /// Builds the `MemberExpr` `base.m`; one built in an unevaluated operand
    /// is not an odr-use (clang's `getNonOdrUseReasonInCurrentContext` in
    /// `BuildMemberExpr`).
    pub(crate) fn build_member_expr(
        &mut self,
        base: ExprId,
        m: DeclId,
        ty: QualType,
        arrow: bool,
        loc: Loc,
        extra: Option<kiln_ast::expr::DeclRefExtra>,
    ) -> ExprId {
        let e = self.builder().member(base, m, ty, arrow, loc, extra);
        if self.unevaluated > 0 {
            let _ = self.ast.exprs.set_non_odr_use(e, NonOdrUse::Unevaluated);
        } else if self.reflection && !self.in_template() {
            self.note_consteval_only_member(e, m, ty);
        }
        e
    }

    // -- sizeof / alignof --------------------------------------------------------

    /// Enters an unevaluated operand ([expr.context]/1).
    pub fn enter_unevaluated(&mut self) {
        self.unevaluated += 1;
        self.push_cleanups();
    }

    /// Leaves an unevaluated operand: its temporaries are never created,
    /// so they add no cleanups to the enclosing full-expression (clang
    /// restores the parent's `CleanupInfo`).
    pub fn exit_unevaluated(&mut self) {
        self.unevaluated = self.unevaluated.saturating_sub(1);
        self.pop_cleanups();
    }

    fn trait_name(kind: TraitKind) -> &'static str {
        match kind {
            TraitKind::AlignOf => "alignof",
            TraitKind::PreferredAlignOf => "__alignof",
            TraitKind::DataSizeOf => "__datasizeof",
            _ => "sizeof",
        }
    }

    /// Checks the operand type of `sizeof`/`alignof` ([expr.sizeof]/1,
    /// [expr.alignof]/1). As clang `CheckUnaryExprOrTypeTraitOperand`,
    /// the diagnostics point at `loc` with `range`: an expression operand
    /// at its expression location with its range, a type operand at the
    /// keyword with the parenthesized type-id's range.
    fn check_trait_operand(
        &mut self,
        kind: TraitKind,
        ty: QualType,
        loc: Loc,
        range: Option<kiln_source::Span>,
    ) -> bool {
        self.check_trait_operand_named(kind, Self::trait_name(kind), ty, loc, range)
    }

    /// [`Self::check_trait_operand`] with the operator spelled `name` in
    /// the diagnostics (`alignas(type-id)`, [dcl.align]/3: as
    /// `alignas(alignof(type-id))`; clang `CheckAlignasTypeArgument`).
    pub(crate) fn check_trait_operand_named(
        &mut self,
        kind: TraitKind,
        name: &'static str,
        ty: QualType,
        loc: Loc,
        range: Option<kiln_source::Span>,
    ) -> bool {
        if self.is_error_type(ty) {
            return false;
        }
        // A dependent operand type is checked when it is substituted
        // ([temp.res.general]/6): `sizeof(void(T))` is a function type in
        // every instantiation, but clang `CreateUnaryExprOrTypeTraitExpr`
        // checks only a non-dependent type, so the uninstantiated
        // template is accepted.
        if self.is_dependent_type(ty) {
            return true;
        }
        let size_like = matches!(kind, TraitKind::SizeOf | TraitKind::DataSizeOf);
        let t = if size_like { self.non_reference(ty) } else { ty };
        // [expr.sizeof]/1, [expr.alignof]/1: in C++ a function type is an
        // error and `void` is an incomplete type; clang's GNU extensions
        // for both are C-only ("hard errors for SFINAE").
        if self.ast.types.is_function(t) {
            let msg = self.diag(DiagId::sema_sizeof_function_expr, loc).arg(name);
            self.emit(Self::with_range(msg, range));
            return false;
        }
        let check = if size_like {
            t
        } else {
            let mut x = self.non_reference(t);
            while let Some(e) = self.ast.types.element(x).filter(|_| self.ast.types.is_array(x)) {
                x = e;
            }
            x
        };
        if !self.require_complete(check, loc) {
            // clang names the checked type (`alignof`: the element type).
            let shown = if size_like { ty } else { check };
            let msg =
                self.diag(DiagId::sema_sizeof_incomplete, loc).arg(name).arg(self.ty_arg(shown));
            let msg = Self::with_range(msg, range);
            self.emit(self.incomplete_note(msg, shown));
            return false;
        }
        // clang `err_sizeof_alignof_typeof_sizeless`.
        if self.ast.types.is_sizeless(check) {
            let msg = self.diag(DiagId::sema_sizeof_sizeless, loc).arg(name).arg(self.ty_arg(ty));
            self.emit(Self::with_range(msg, range));
            return false;
        }
        true
    }

    fn with_range(d: Diag, range: Option<kiln_source::Span>) -> Diag {
        match range {
            Some(r) => d.range(r),
            None => d,
        }
    }

    /// The range `( type-id )` of the type operand of the keyword at
    /// `kw` closed by `rparen`: the `(` is the first non-blank after the
    /// keyword (none in a macro expansion, whose text is not at hand).
    fn paren_type_range(&self, kw: Loc, rparen: Loc) -> Option<kiln_source::Span> {
        let sm = self.diags.sm();
        let (file, pos) = sm.offset_of(kw)?;
        let text = sm.text(file).as_bytes();
        let mut i = pos as usize;
        while text.get(i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
            i += 1;
        }
        while text.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        if text.get(i) != Some(&b'(') {
            return None;
        }
        let open = sm.loc(file, u32::try_from(i).ok()?);
        Some(kiln_source::Span::new(open, self.token_end(rparen).unwrap_or(rparen)))
    }

    /// `sizeof(T)` / `alignof(T)`.
    pub fn act_on_trait_type(
        &mut self,
        kind: TraitKind,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let range = self.paren_type_range(loc, rparen);
        self.trait_type_in(kind, ty, loc, rparen, range)
    }

    /// [`Self::act_on_trait_type`] rebuilt by template instantiation:
    /// clang `RebuildUnaryExprOrTypeTrait` underlines the whole pattern
    /// expression, keyword to `)`.
    pub(crate) fn rebuild_trait_type(
        &mut self,
        kind: TraitKind,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let range = kiln_source::Span::new(loc, self.token_end(rparen).unwrap_or(rparen));
        self.trait_type_in(kind, ty, loc, rparen, Some(range))
    }

    fn trait_type_in(
        &mut self,
        kind: TraitKind,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
        range: Option<kiln_source::Span>,
    ) -> ExprId {
        if !self.check_trait_operand(kind, ty, loc, range) {
            return self.recover_err(&[], loc);
        }
        let st = self.size_type();
        self.builder().trait_of_type(kind, ty, st, loc, rparen)
    }

    /// `sizeof e` (and the `alignof e` extension). The operand was parsed
    /// between [`Self::enter_unevaluated`] and [`Self::exit_unevaluated`].
    /// `sizeof...(name)` ([expr.sizeof]/5): `name` must name a pack. M2
    /// declares no packs, so this always fails (clang's wording).
    pub fn act_on_sizeof_pack(&mut self, name: Symbol, name_loc: Loc, loc: Loc) -> ExprId {
        let text = format!("'{}'", self.ast.symbols().as_str(name));
        let msg = self.diag(DiagId::sema_sizeof_pack_no_pack, name_loc).arg(text);
        self.emit(msg);
        self.recover_err(&[], loc)
    }

    pub fn act_on_trait_expr(&mut self, kind: TraitKind, e: ExprId, loc: Loc) -> ExprId {
        // clang's `CreateUnaryExprOrTypeTraitExpr` checks placeholders
        // first: a bound member function is recovered by calling it.
        let e = self.recover_bound_member(e);
        if self.is_err_expr(e) {
            return self.recover_err(&[e], loc);
        }
        if self.is_bitfield(e) {
            // clang: at the operand's expression location, no range.
            let at = self.expr_loc(e);
            let msg = self.diag(DiagId::sema_sizeof_bitfield, at).arg(Self::trait_name(kind));
            self.emit(msg);
            return self.recover_err(&[e], loc);
        }
        if matches!(kind, TraitKind::AlignOf | TraitKind::PreferredAlignOf)
            && self.alignof_member_of_incomplete_class(e)
        {
            return self.recover_err(&[e], loc);
        }
        let e = self.resolve_single_overload_candidate(e);
        if self.check_overload_use(e) {
            return self.recover_err(&[e], loc);
        }
        self.check_unevaluated_side_effects(e);
        // A type-dependent operand whose type is not itself dependent (a
        // static data member of the current instantiation declared as an
        // array of unknown bound, [temp.dep.expr]/3) is checked at
        // instantiation (clang `CreateUnaryExprOrTypeTraitExpr`).
        if self.ast.exprs.get(e).is_type_dependent() && !self.is_dependent_type(self.ety(e)) {
            let st = self.size_type();
            return self.builder().trait_of_expr(kind, e, st, loc);
        }
        let e = self.complete_expr_array_bound(e);
        let ty = self.ety(e);
        // [expr.sizeof]/1: in C++ an expression of function type is an
        // error, not the GNU extension the type form gets (a dependent
        // one when it is substituted).
        if self.ast.types.is_function(ty) && !self.is_dependent_type(ty) {
            let msg = self.diag(DiagId::sema_sizeof_function_expr, self.eloc(e));
            self.emit(msg.arg(Self::trait_name(kind)));
            return self.recover_err(&[e], loc);
        }
        if !self.check_trait_operand(kind, ty, self.expr_loc(e), Some(self.expr_range(e))) {
            return self.recover_err(&[e], loc);
        }
        let st = self.size_type();
        self.builder().trait_of_expr(kind, e, st, loc)
    }

    /// clang `completeExprArrayBound`: a (parenthesized) reference to a
    /// static data member of a class template specialization declared as
    /// an array of unknown bound instantiates the member's definition,
    /// whose initializer or declarator gives the bound ([temp.inst]/4,
    /// [dcl.array]/3), and names that definition with its type, so that
    /// `sizeof(Ops)` sees the complete array. Anything else is returned
    /// unchanged.
    fn complete_expr_array_bound(&mut self, e: ExprId) -> ExprId {
        let inner = self.ignore_parens(e);
        let n = *self.ast.exprs.get(inner);
        if n.kind() != ExprKind::DeclRefExpr
            || n.is_type_dependent()
            || !matches!(self.ast.types.canonical_view(n.ty()), TypeView::IncompleteArray(_))
        {
            return e;
        }
        let v = self.ast.exprs.decl(inner);
        if self.kind_of(v) != DeclKind::Var
            || self.ast.instantiated_from(self.ast.first(v)).is_none()
            || self.inst_owner(self.ast.first(v)).is_none()
        {
            return e;
        }
        let loc = self.expr_loc(inner);
        self.instantiate_static_member_def(self.ast.first(v), loc);
        // The latest declaration that knows the bound: the instantiated
        // definition (clang names `getDefinition()`).
        let complete = |s: &Self, x: DeclId| {
            s.ast.decls.get(x).ty().filter(|&t| {
                !s.is_error_type(t)
                    && !matches!(s.ast.types.canonical_view(t), TypeView::IncompleteArray(_))
            })
        };
        let Some((def, dty)) =
            self.ast.redecls_of(v).find_map(|x| complete(self, x).map(|t| (x, t)))
        else {
            return e;
        };
        let extra = self.ast.exprs.decl_ref_extra(inner).copied();
        let odr = self.ast.exprs.non_odr_use(inner);
        let r = self.builder().decl_ref(def, dty, n.loc(), extra, odr, ExprFlags::NONE);
        if let Some(end) = self.ast.exprs.end_loc(inner) {
            let _ = self.ast.exprs.cold.end_loc.insert(r, end);
        }
        // Rewrap the parentheses around the new reference.
        let mut parens = Vec::new();
        let mut x = e;
        while x != inner && self.ast.exprs.get(x).kind() == ExprKind::ParenExpr {
            parens.push(x);
            x = self.ast.exprs.operand(x);
        }
        if x != inner {
            return e;
        }
        let mut out = r;
        for &p in parens.iter().rev() {
            let pn = *self.ast.exprs.get(p);
            let rparen = self.ast.exprs.end_loc(p).unwrap_or(pn.loc());
            out = self.builder().paren(out, pn.loc(), rparen);
        }
        out
    }

    /// clang `CheckAlignOfExpr`: `alignof` of a data member (named,
    /// ignoring parentheses) needs its class's layout, so the class must
    /// be complete (`struct U { int i; static_assert(alignof(i) == 4); }`
    /// is an error at the operand; [class.mem.general]/8: a
    /// `static_assert` is not a complete-class context).
    fn alignof_member_of_incomplete_class(&mut self, e: ExprId) -> bool {
        // clang reports at the operand's own exprLoc (a ParenExpr: its `(`).
        let inner = self.ignore_parens(e);
        let n = *self.ast.exprs.get(inner);
        if n.is_type_dependent()
            || !matches!(n.kind(), ExprKind::DeclRefExpr | ExprKind::MemberExpr)
        {
            return false;
        }
        let d = self.ast.exprs.decl(inner);
        if self.ast.decls.get(d).kind() != DeclKind::Field {
            return false;
        }
        let Some(parent) = self.ast.decls.get(d).ctx() else { return false };
        let incomplete = self
            .ast
            .decls
            .record(parent)
            .is_some_and(|r| !r.defn.contains(kiln_ast::decl::DefinitionBits::COMPLETE));
        if incomplete {
            let msg = self.diag(DiagId::sema_alignof_member_of_incomplete_class, self.eloc(e));
            self.emit(msg);
        }
        incomplete
    }

    // -- casts -------------------------------------------------------------------

    /// Marks the implicit conversions applied on top of `orig` (down to
    /// it) as part of an explicit cast.
    pub(crate) fn mark_explicit(&mut self, top: ExprId, orig: ExprId) {
        let mut cur = top;
        for _ in 0..64 {
            if cur == orig || self.ast.exprs.get(cur).kind() != ExprKind::ImplicitCastExpr {
                break;
            }
            self.ast.exprs.mark_part_of_explicit_cast(cur);
            cur = self.ast.exprs.operand(cur);
        }
    }

    /// [over.over]/1: the operand of a cast to `ty` that names an overload
    /// set resolves to the function `ty` selects; `None` after
    /// diagnosing.
    #[inline(never)]
    fn cast_overload_set(
        &mut self,
        style: CastStyle,
        ty: QualType,
        e: ExprId,
        loc: Loc,
    ) -> Option<ExprId> {
        use crate::overload::address::AddrCtx;
        let ctx = match style {
            CastStyle::Reinterpret => AddrCtx::Reinterpret,
            CastStyle::Static => AddrCtx::Cast(1),
            _ if self.ast.types.is_void(ty) => AddrCtx::Cast(0),
            _ => AddrCtx::Convert,
        };
        // clang reports a C-style cast at its type, just after the `(`.
        let at = if style == CastStyle::CStyle { loc.with_offset(1) } else { loc };
        self.resolve_address_of_overloaded(e, ty, true, ctx, at)
    }

    /// An explicit type conversion `style<ty>(e)` ([expr.cast],
    /// [expr.static.cast], [expr.reinterpret.cast], [expr.const.cast],
    /// [expr.type.conv]). `loc` is the cast's start, `rparen` its closing
    /// `)`.
    pub fn act_on_cast(
        &mut self,
        style: CastStyle,
        ty: QualType,
        e: ExprId,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        // A misaligned `&m` converted here is dropped now, as clang
        // `BuildCStyleCastExpr` / `BuildCXXNamedCast` /
        // `BuildCXXFunctionalCastExpr` do, before the rest of the
        // enclosing full-expression is parsed: a nested full-expression
        // after it (a template argument, a lambda body, an instantiated
        // body) reports what is still pending (WP-659).
        if !self.misaligned.is_empty() && style != CastStyle::Dynamic {
            self.discard_misaligned_member_address(ty, e);
        }
        let r = self.build_explicit_cast(style, ty, e, loc, rparen);
        self.note_cast_written_type(r, ty);
        r
    }

    /// Records the type as written `ty` of explicit cast `r` when the
    /// dump cannot rebuild it from the node (clang prints
    /// `getTypeAsWritten` for named and functional casts): the prvalue of
    /// a non-class cast drops the qualifiers and sugar of `CI` in `CI{1}`
    /// and `static_cast<CI>(x)`; a reference named by an alias.
    pub(crate) fn note_cast_written_type(&mut self, r: ExprId, ty: QualType) {
        let n = *self.ast.exprs.get(r);
        if !matches!(
            n.kind(),
            ExprKind::CXXFunctionalCastExpr
                | ExprKind::CXXStaticCastExpr
                | ExprKind::CXXConstCastExpr
                | ExprKind::CXXReinterpretCastExpr
                | ExprKind::CXXDynamicCastExpr
        ) || self.is_error_type(ty)
        {
            return;
        }
        // `const R` for a reference alias `R` is `R` ([dcl.ref]/1).
        let ty = if self.ast.types.is_reference(ty) { ty.without_quals() } else { ty };
        let rebuilt = match (self.ast.types.view(ty.ty()), n.value_category()) {
            (TypeView::LValueRef(p), ValueCategory::LValue)
            | (TypeView::RValueRef(p), ValueCategory::XValue) => p == n.ty(),
            (_, ValueCategory::PRValue) => ty == n.ty(),
            _ => false,
        };
        if !rebuilt {
            self.builder().set_cast_written_type(r, ty);
        }
    }

    #[allow(clippy::too_many_lines, reason = "one step per paragraph of [expr.cast]/4")]
    fn build_explicit_cast(
        &mut self,
        style: CastStyle,
        ty: QualType,
        e: ExprId,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        // `C(e)` ([dcl.type.class.deduct]/2, WP-071b).
        let ty = if style == CastStyle::Functional && !self.is_err_expr(e) {
            let ty = self.auto_cast_type(ty, &[e], None, loc);
            self.ctad_cast_type(ty, &[e], loc)
        } else {
            ty
        };
        let is_err = self.is_err_expr(e) || self.is_error_type(ty);
        if !is_err && (self.ast.types.is_dependent(ty) || self.expr_is_type_dependent(e)) {
            return self.dependent_cast(style, ty, e, loc, rparen);
        }
        // [expr.ref]/7.3.2: clang's `checkNonOverloadPlaceholders` recovers
        // a bound member function operand before any cast is checked.
        let (e, is_err) = if !is_err && self.ety(e) == QualType::BOUND_MEMBER {
            let e = self.recover_bound_member(e);
            (e, self.is_err_expr(e))
        } else {
            (e, is_err)
        };
        if style == CastStyle::Dynamic {
            return self.act_on_dynamic_cast(ty, e, loc, rparen);
        }
        if is_err {
            return self.recover(ty, &[e], loc);
        }
        // A class type is no [over.over] target: the cast
        // direct-initializes it through a constructor taking the set.
        let e = if self.ety(e) == QualType::OVERLOAD
            && !self.overload_set_converts_to_class(
                e,
                ty,
                crate::overload::ics::IcsOptions::ALLOW_EXPLICIT,
            ) {
            match self.cast_overload_set(style, ty, e, loc) {
                Some(x) => x,
                None => return self.recover(ty, &[e], loc),
            }
        } else {
            e
        };
        if self.ast.types.is_reference(ty) {
            return self.ref_cast(style, ty, e, loc, rparen);
        }
        // [expr.static.cast]/6: any expression can be converted to cv void.
        if self.ast.types.is_void(ty) {
            if style.allows_static() {
                if self.check_builtin_fn_use(e) {
                    return self.recover(ty, &[e], loc);
                }
                self.discard_assignment_ref(e);
                // [expr.static.cast]/6: the operand is a discarded-value
                // expression (clang `CastOperation::CheckToVoid`).
                let e = self.ignored_value_conversions(e);
                return self.explicit_cast(style, CastKind::ToVoid, e, ty, loc, rparen);
            }
            if style == CastStyle::Const {
                // clang `err_bad_const_cast_dest`: `void` is not a pointer.
                let msg = self
                    .diag(DiagId::sema_bad_const_cast_dest, loc)
                    .arg(style.select())
                    .arg(self.ty_arg(ty));
                self.emit(msg);
                return self.recover(ty, &[e], loc);
            }
            return self.bad_cast(style, e, ty, loc);
        }
        // clang `TryStaticImplicitCast`: a class target must be complete;
        // the use whose instantiation fails (a failed `static_assert` in
        // the class, [temp.inst]/2) fails quietly.
        if style.allows_static()
            && self.ast.types.as_record(ty).is_some()
            && self.instantiation_fails(ty, loc)
        {
            return self.recover(ty, &[e], loc);
        }
        // [expr.static.cast]/4 with a class target: direct-initialization
        // of a temporary from an object of the same or a derived class
        // ([dcl.init.general]/16.6.2; clang's `ConstructorConversion`).
        if style.allows_static()
            && self.ast.types.as_record(ty).is_some()
            && let Some(x) = {
                let d = self
                    .diag(DiagId::sema_ovl_deleted_conversion_in_cast, loc)
                    .arg(style.select())
                    .arg(self.ty_arg(self.ety(e)))
                    .arg(self.ty_arg(ty));
                self.construct_from_class(
                    e,
                    ty,
                    ConvCtx::Cast,
                    loc,
                    crate::class::DeletedAs::Cast(d, style.select()),
                )
            }
        {
            if self.is_err_expr(x) {
                return x;
            }
            if x == e {
                return self.explicit_cast(style, CastKind::NoOp, x, ty, loc, rparen);
            }
            // [class.abstract]/3: a temporary of an abstract class.
            if self.require_non_abstract_alloc(loc, ty) {
                return self.recover(ty, &[x], loc);
            }
            let x = self.maybe_bind_temporary(x);
            return self.explicit_cast(style, CastKind::ConstructorConversion, x, ty, loc, rparen);
        }
        // [expr.static.cast]/4: `T(e)`, `(T)e`, `static_cast<T>(e)` for
        // a class `T` direct-initialize a temporary by a constructor.
        if style.allows_static() && self.ast.types.as_record(ty).is_some() {
            let d = self
                .diag(DiagId::sema_ovl_deleted_conversion_in_cast, loc)
                .arg(style.select())
                .arg(self.ty_arg(self.ety(e)))
                .arg(self.ty_arg(ty));
            let site = crate::class::ConstructSite {
                loc,
                temp_end: None,
                rparen: None,
                // clang `InitializationKind::CreateFunctionalCast` /
                // `CreateCast`: the `)`; a C-style cast's range ends at its
                // operand's last token (WP-182).
                agg_end: Some(if style == CastStyle::CStyle {
                    self.expr_end_loc(e)
                } else {
                    rparen
                }),
                sites: crate::class::DiagSites {
                    access: loc,
                    deleted: loc,
                    deleted_as: crate::class::DeletedAs::Cast(d, style.select()),
                },
            };
            // The temporary has the written type, qualifiers and sugar
            // included ([expr.type.conv]/1, clang: `CV(1)` constructs a
            // `'CV':'const V'`).
            if let Some(x) = self.construct_class(ty, &[e], crate::class::InitStyle::Direct, site) {
                if self.is_err_expr(x) {
                    return x;
                }
                // A parenthesized aggregate initialization (P0960) is a
                // no-op cast of the initialized prvalue, as is the result
                // of a conversion function (CWG2327).
                if self.ast.exprs.get(x).kind() == ExprKind::ImplicitCastExpr {
                    self.mark_explicit(x, e);
                    return self.explicit_cast(style, CastKind::NoOp, x, ty, loc, rparen);
                }
                // [class.abstract]/3: a temporary of an abstract class
                // (clang's `checkAbstractType` for the constructor call,
                // at the cast's location).
                if self.require_non_abstract_alloc(loc, ty) {
                    return self.recover(ty, &[x], loc);
                }
                let kind = if self.ast.exprs.get(x).kind() == ExprKind::CXXParenListInitExpr {
                    CastKind::NoOp
                } else {
                    CastKind::ConstructorConversion
                };
                let x = self.maybe_bind_temporary(x);
                return self.explicit_cast(style, kind, x, ty, loc, rparen);
            }
        }
        if style.allows_static()
            && self.ast.types.as_record(ty).is_none()
            && self.ast.types.as_record(self.ety(e)).is_some()
        {
            return self.cast_user_conversion(style, e, ty, loc, rparen);
        }
        if self.ast.types.as_record(ty).is_some() {
            self.unsupported("conversions of class types", loc);
            return self.recover(ty, &[e], loc);
        }
        let to = conv::strip_cv(self, ty);
        // const_cast part (and the qualification-only C-style cast).
        if style.allows_const() {
            let x = conv::decay(self, e);
            let xt = self.ety(x);
            if style == CastStyle::Const && !self.ast.types.is_pointer(to) {
                let msg = self
                    .diag(DiagId::sema_bad_const_cast_dest, loc)
                    .arg(style.select())
                    .arg(self.ty_arg(ty));
                self.emit(msg);
                return self.recover(ty, &[e], loc);
            }
            if self.ast.types.is_pointer(to)
                && self.ast.types.is_pointer(xt)
                && self.similar_pointers(xt, to)
            {
                self.mark_explicit(x, e);
                return self.explicit_cast(style, CastKind::NoOp, x, to, loc, rparen);
            }
            if style == CastStyle::Const {
                return self.bad_cast(style, e, ty, loc);
            }
        }
        // static_cast: an implicit conversion sequence ([expr.static.cast]/4).
        if style.allows_static()
            && conv::standard_conversion(self, e, to, ConvCtx::Cast).is_some()
            && let Some(scs) = conv::standard_conversion(self, e, to, ConvCtx::Cast)
        {
            let was = std::mem::replace(&mut self.explicit_conv, true);
            let was_access =
                std::mem::replace(&mut self.cast_ignores_base_access, style.ignores_base_access());
            let r = conv::apply_standard_conversion(self, e, &scs, to);
            self.explicit_conv = was;
            self.cast_ignores_base_access = was_access;
            match r {
                Ok(x) => {
                    self.mark_explicit(x, e);
                    return self.explicit_cast(style, CastKind::NoOp, x, to, loc, rparen);
                }
                Err(err) => {
                    self.emit_conv_error(err, loc);
                    return self.recover(ty, &[e], loc);
                }
            }
        }
        let x = conv::decay(self, e);
        self.mark_explicit(x, e);
        let xt = self.ety(x);
        // static_cast: inverses of standard conversions ([expr.static.cast]/7-14).
        if style.allows_static()
            && let Some(ck) = self.static_inverse(xt, to, style)
        {
            return self.explicit_cast(style, ck, x, to, loc, rparen);
        }
        // static_cast: a pointer to a base class as a pointer to a derived
        // class ([expr.static.cast]/11).
        if style.allows_static()
            && let Some(c) = self.static_downcast(style, x, to, loc, rparen)
        {
            return c;
        }
        // static_cast: a pointer to member of a derived class as a pointer
        // to member of a base class ([expr.static.cast]/12).
        if style.allows_static()
            && let Some(c) = self.static_mp_upcast(style, x, to, loc, rparen)
        {
            return c;
        }
        // reinterpret_cast ([expr.reinterpret.cast]).
        if style.allows_reinterpret() {
            match self.reinterpret(xt, to, style, loc) {
                Ok(Some(ck)) => return self.explicit_cast(style, ck, x, to, loc, rparen),
                // Diagnosed (clang's `TC_Failed` with its own message): the
                // cast is erroneous, so a default argument or initializer
                // it appears in is invalid, with no follow-on diagnostic.
                Err(()) => return self.recover(ty, &[e], loc),
                Ok(None) => {}
            }
        }
        self.bad_cast(style, e, ty, loc)
    }

    /// The node of an explicit cast `style` that succeeded with kind `ck`:
    /// a prvalue of `to`. A functional cast records its `)` (`rparen`).
    fn explicit_cast(
        &mut self,
        style: CastStyle,
        ck: CastKind,
        sub: ExprId,
        to: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let vk = ValueCategory::PRValue;
        let flags = ExprFlags::NONE;
        if style == CastStyle::Functional {
            self.builder().functional_cast(ck, sub, to, vk, None, loc, rparen, flags)
        } else {
            self.builder().cast(style.kind(), ck, sub, to, vk, None, loc, flags)
        }
    }

    /// [expr.static.cast]/4 from a class to a non-class type `ty`: the
    /// user-defined conversion of the direct-initialization `ty t(e);`
    /// (explicit conversion functions are candidates), with clang's
    /// `tryDiagnoseOverloadedCast` errors.
    #[inline(never)]
    fn cast_user_conversion(
        &mut self,
        style: CastStyle,
        e: ExprId,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        use crate::overload::{Ics, IcsOptions};
        let to = conv::strip_cv(self, ty);
        let from = self.ety(e);
        let a = self.arg_info(e);
        let id = match self.try_implicit_conversion(&a, to, IcsOptions::ALLOW_EXPLICIT) {
            Ics::UserDefined(u) if !u.func.is_some_and(|f| self.ovl_deleted(f)) => {
                return match self.user_convert(e, to, ConvCtx::Cast, loc) {
                    Some(Ok(x)) if !self.is_err_expr(x) => {
                        self.mark_explicit(x, e);
                        self.explicit_cast(style, CastKind::NoOp, x, to, loc, rparen)
                    }
                    Some(Ok(_)) => self.recover(ty, &[e], loc),
                    Some(Err(err)) => {
                        self.emit_conv_error(err, loc);
                        self.recover(ty, &[e], loc)
                    }
                    None => {
                        self.unsupported("this user-defined conversion", loc);
                        self.recover(ty, &[e], loc)
                    }
                };
            }
            Ics::UserDefined(_) => DiagId::sema_ovl_deleted_conversion_in_cast,
            Ics::Ambiguous { .. } => DiagId::sema_ovl_ambiguous_conversion_in_cast,
            // clang's `TryStaticPointerDowncast` / `TryStaticMemberPointerUpcast`
            // diagnose a class operand of a cast to a pointer first.
            _ if self.ast.types.is_pointer(to) || self.is_member_pointer(to) => {
                let id = if self.ast.types.is_pointer(to) {
                    DiagId::sema_bad_static_cast_pointer_nonpointer
                } else {
                    DiagId::sema_bad_static_cast_member_pointer_nonmp
                };
                let msg = self.diag(id, loc).arg(self.ty_arg(from)).arg(self.ty_arg(ty));
                self.emit(msg);
                return self.recover(ty, &[e], loc);
            }
            _ if conv::has_conversion_functions(self, from) => {
                DiagId::sema_ovl_no_conversion_in_cast
            }
            _ => {
                let msg = self
                    .diag(DiagId::sema_nonviable_condition_no_candidates, loc)
                    .arg(self.ty_arg(from))
                    .arg(self.ty_arg(ty));
                self.emit(msg);
                return self.recover(ty, &[e], loc);
            }
        };
        let mut msg =
            self.diag(id, loc).arg(style.select()).arg(self.ty_arg(from)).arg(self.ty_arg(ty));
        let notes = self.udc_cast_notes(&a, to, id != DiagId::sema_ovl_no_conversion_in_cast);
        for n in notes {
            msg = msg.with_note(n);
        }
        self.emit(msg);
        self.recover(ty, &[e], loc)
    }

    /// clang `err_bad_cxx_cast_generic`. The target is not a reference, so
    /// the operand went through `DefaultFunctionArrayLvalueConversion`
    /// first and the message names the decayed type (`'int *'` for an
    /// `int[3]` lvalue, `'void (*)(int)'` for a function; [expr.cast]/4).
    fn bad_cast(&mut self, style: CastStyle, e: ExprId, ty: QualType, loc: Loc) -> ExprId {
        let from = self.ety(e);
        let from = if self.ast.types.is_array(from) {
            conv::array_decayed(self, from)
        } else if self.ast.types.is_function(from) {
            self.ast.types.pointer_to(from)
        } else {
            from
        };
        let msg = self
            .diag(DiagId::sema_bad_cxx_cast, loc)
            .arg(style.select())
            .arg(self.ty_arg(from))
            .arg(self.ty_arg(ty));
        self.emit(msg);
        self.recover(ty, &[e], loc)
    }

    /// Pointer types differing only in cv-qualification at any level
    /// ([conv.qual]/2 "similar", [expr.const.cast]/3): the levels below
    /// the first may be pointers, pointers to members of the same class
    /// or arrays of the same bound (`int A::**` and `const int A::**`).
    fn similar_pointers(&self, a: QualType, b: QualType) -> bool {
        conv::similar_same_levels(self, a, b)
    }

    /// True if converting pointer `from` to pointer `to` casts away
    /// constness ([expr.const.cast]/8), one level deep.
    fn casts_away_quals(&self, from: QualType, to: QualType) -> bool {
        match (self.ast.types.pointee(from), self.ast.types.pointee(to)) {
            (Some(f), Some(t)) => {
                let (fq, tq) =
                    (self.ast.types.effective_quals(f), self.ast.types.effective_quals(t));
                !tq.contains(fq)
            }
            _ => false,
        }
    }

    fn static_inverse(
        &mut self,
        from: QualType,
        to: QualType,
        style: CastStyle,
    ) -> Option<CastKind> {
        let types = &self.ast.types;
        // void* -> T* ([expr.static.cast]/13).
        if types.is_pointer(from) && types.is_pointer(to) {
            let fp = types.pointee(from)?;
            let tp = types.pointee(to)?;
            if types.is_void(fp) && !types.is_function(tp) {
                if !style.allows_const() && self.casts_away_quals(from, to) {
                    return None;
                }
                return Some(CastKind::BitCast);
            }
            return None;
        }
        let to_enum = types.as_enum(to).is_some();
        let from_enum = types.as_enum(from).is_some();
        // integral/enum/floating -> enum ([expr.static.cast]/10).
        if to_enum {
            if types.is_integral(from) || from_enum {
                // The value is first converted to a fixed underlying type,
                // so one of `bool` is a boolean conversion (clang
                // `TryStaticCast`: `CK_IntegralToBoolean`).
                let to_bool = conv::rank::enum_data(self, to).is_some_and(|d| {
                    d.is_fixed()
                        && d.int_type.is_some_and(|i| types.builtin(i) == Some(BuiltinType::Bool))
                });
                return Some(if to_bool {
                    CastKind::IntegralToBoolean
                } else {
                    CastKind::IntegralCast
                });
            }
            if types.is_floating(from) {
                return Some(CastKind::FloatingToIntegral);
            }
        }
        // scoped enum -> arithmetic ([expr.static.cast]/9).
        if from_enum {
            if types.builtin(to) == Some(BuiltinType::Bool) {
                return Some(CastKind::IntegralToBoolean);
            }
            if types.is_integral(to) {
                return Some(CastKind::IntegralCast);
            }
            if types.is_floating(to) {
                return Some(CastKind::IntegralToFloating);
            }
        }
        None
    }

    /// The `reinterpret_cast` part of cast `style` from `from` to `to`
    /// ([expr.reinterpret.cast]; clang `TryReinterpretCast`): the cast
    /// kind, `Ok(None)` when it does not apply (the caller reports the
    /// generic bad cast), or `Err(())` after a specific error (a pointer
    /// to a smaller integer, a vector of a different size: clang's
    /// `TC_Failed`), when the cast is erroneous.
    fn reinterpret(
        &mut self,
        from: QualType,
        to: QualType,
        style: CastStyle,
        loc: Loc,
    ) -> Result<Option<CastKind>, ()> {
        // clang `TryReinterpretCast`: a vector and a vector or integer of
        // the same data size (`areLaxCompatibleVectorTypes`) bit-cast.
        let (fv, tv) = (self.is_vector_type(from), self.is_vector_type(to));
        if fv || tv {
            if (!tv && !self.ast.types.is_integral(to))
                || (!fv && !self.ast.types.is_integral(from))
            {
                return Ok(None);
            }
            // `areLaxCompatibleVectorTypes`: never between a scalar and an
            // ext vector.
            let ext = |q| self.vector_parts(q).is_some_and(VecParts::is_ext);
            let scalar_ext = (!fv && ext(to)) || (!tv && ext(from));
            if !scalar_ext
                && self.lax_size(from).is_some()
                && self.lax_size(from) == self.lax_size(to)
            {
                return Ok(Some(CastKind::BitCast));
            }
            let id = if !tv {
                DiagId::sema_bad_cxx_cast_vector_to_scalar_different_size
            } else if !fv {
                DiagId::sema_bad_cxx_cast_scalar_to_vector_different_size
            } else {
                DiagId::sema_bad_cxx_cast_vector_to_vector_different_size
            };
            let msg =
                self.diag(id, loc).arg(style.select()).arg(self.ty_arg(from)).arg(self.ty_arg(to));
            self.emit(msg);
            return Err(());
        }
        let (fp, tp) = (self.ast.types.is_pointer(from), self.ast.types.is_pointer(to));
        if fp && tp {
            // [expr.reinterpret.cast]/2, [expr.const.cast]/8: not casting
            // away constness at any level (clang `CastsAwayConstness`;
            // between levels of different kinds clang only warns, an
            // extension kiln accepts silently).
            if style == CastStyle::Reinterpret
                && matches!(
                    conv::casts_away_constness(self, from, to),
                    conv::CastAway::Similar | conv::CastAway::SimilarKind
                )
            {
                let msg = self
                    .diag(DiagId::sema_bad_cast_qualifiers_away, loc)
                    .arg(style.select())
                    .arg(self.ty_arg(from))
                    .arg(self.ty_arg(to));
                self.emit(msg);
                return Err(());
            }
            return Ok(Some(CastKind::BitCast));
        }
        if tp && self.is_integral_or_enum(from) {
            return Ok(Some(CastKind::IntegralToPointer));
        }
        if fp && self.is_integral_or_enum(to) {
            let pw = u32::try_from(self.abi.pointer_size * 8).unwrap_or(64);
            if conv::int_width(self, to).is_some_and(|w| w < pw) {
                let msg =
                    self.diag(DiagId::sema_reinterpret_cast_small_int, loc).arg(self.ty_arg(to));
                self.emit(msg);
                return Err(());
            }
            return Ok(Some(CastKind::PointerToIntegral));
        }
        // [expr.reinterpret.cast]/4: a value of type `std::nullptr_t`
        // converts to an integral type (not an enumeration) with the
        // meaning and validity of `(void*)0`'s conversion (clang
        // `TryReinterpretCast`).
        if self.is_nullptr_type(from) && self.ast.types.is_integral(to) {
            let pw = u32::try_from(self.abi.pointer_size * 8).unwrap_or(64);
            if conv::int_width(self, to).is_some_and(|w| w < pw) {
                let msg =
                    self.diag(DiagId::sema_reinterpret_cast_small_int, loc).arg(self.ty_arg(to));
                self.emit(msg);
                return Err(());
            }
            return Ok(Some(CastKind::PointerToIntegral));
        }
        if style == CastStyle::Reinterpret
            && self.is_integral_or_enum(from)
            && self.ast.types.same_unqualified(from, to)
        {
            return Ok(Some(CastKind::NoOp));
        }
        Ok(None)
    }
}

/// "1st", "2nd", "3rd", "4th", ...
pub(crate) fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, x) if x != 11 => "st",
        (2, x) if x != 12 => "nd",
        (3, x) if x != 13 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// The operand kind of a rejected user-defined literal, which selects
/// The AST form of a literal's base.
pub(crate) const fn ast_radix(r: kiln_lex::literal::Radix) -> Radix {
    match r {
        kiln_lex::literal::Radix::Binary => Radix::Binary,
        kiln_lex::literal::Radix::Octal => Radix::Octal,
        kiln_lex::literal::Radix::Decimal => Radix::Decimal,
        kiln_lex::literal::Radix::Hex => Radix::Hex,
    }
}

/// clang's wording for the failed literal-operator lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UdOperand {
    Integer,
    Float,
    /// A character literal of the given type.
    Char(QualType),
    /// A string literal of the given element type.
    String(QualType),
}

/// The function a call names, for default arguments and diagnostics.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CallCallee {
    /// The function.
    pub(crate) decl: DeclId,
    /// A non-member function named by an id-expression: clang reports
    /// arity errors through overload resolution ("no matching function").
    pub(crate) overloadable: bool,
    /// Whether the call checks the function's `enable_if` conditions
    /// itself: not when overload resolution chose the function (it
    /// checked them, binding an explicit object member function's
    /// arguments as clang does, [`Sema::check_enable_if`]).
    pub(crate) enable_if: bool,
}

/// An overload set in the order clang lists it: an unqualified lookup
/// newest first, as for a non-template set (spec/sema-overload.md §2.1).
fn lookup_set_order(decls: &[DeclId], unqualified: bool) -> Vec<DeclId> {
    if unqualified { decls.iter().rev().copied().collect() } else { decls.to_vec() }
}

/// The operands of a non-dependent conditional expression being checked
/// ([expr.cond]): the original condition `c`, its converted form `c2`,
/// the arms and the location.
#[derive(Clone, Copy, Debug)]
struct CondOps {
    c: ExprId,
    c2: ExprId,
    t: ExprId,
    f: ExprId,
    loc: Loc,
}
