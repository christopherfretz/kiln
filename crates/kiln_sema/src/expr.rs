//! Expressions ([expr], spec/sema.md §5): names, literals, operators,
//! calls, member access, `sizeof`/`alignof`, casts, and the end of a
//! full-expression (the odr-use pass, [basic.def.odr]).
//!
//! Each `act_on_*` builds the typed node clang builds, with the implicit
//! conversions of [`crate::conv`] spelled out as `ImplicitCastExpr`s.

use kiln_ast::attr::AttrKind;
use kiln_ast::consts::{FloatKind, IntBits, NullKind, Scalar};
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{
    BinaryOp, CastKind, CharKind, ExprFlags, ExprKind, NonOdrUse, ObjectKind, Radix, StringKind,
    TraitKind, UnaryOp, ValueCategory,
};
use kiln_ast::ids::RawId;
use kiln_ast::strings::StringEncoding;
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{Ast, AttrId, ConstValueId, DeclId, ExprId, ListId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_lex::Encoding;
use kiln_lex::literal::{CharLiteral, CodeUnits, FloatLiteral, IntegerLiteral, StringLiteral};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::constfold::{self, ConstEvaluator, ConstVal, NotConstant, int_info};
use crate::conv::{self, ArithConvKind, ConvCtx, ConvError};
use crate::literal;
use crate::lookup::LookupKind;

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

    fn kind(self) -> ExprKind {
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
}

impl Sema<'_, '_> {
    // -- small helpers ---------------------------------------------------------

    fn ety(&self, e: ExprId) -> QualType {
        self.ast.exprs.get(e).ty()
    }

    fn eloc(&self, e: ExprId) -> Loc {
        self.ast.exprs.get(e).loc()
    }

    fn is_err_expr(&self, e: ExprId) -> bool {
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
    fn non_reference(&self, t: QualType) -> QualType {
        if self.ast.types.is_reference(t) {
            self.pointee_sugared(t).unwrap_or(QualType::ERROR)
        } else {
            t
        }
    }

    fn is_integral_or_enum(&self, t: QualType) -> bool {
        self.ast.types.is_integral(t) || self.ast.types.as_enum(t).is_some()
    }

    fn is_scalar(&self, t: QualType) -> bool {
        self.ast.types.is_arithmetic(t)
            || self.ast.types.as_enum(t).is_some()
            || self.ast.types.is_pointer(t)
            || self.is_nullptr_type(t)
    }

    pub(crate) fn is_nullptr_type(&self, t: QualType) -> bool {
        self.ast.types.builtin(t) == Some(BuiltinType::NullPtr)
    }

    fn size_type(&self) -> QualType {
        QualType::unqual(self.target.size_t)
    }

    /// A `RecoveryExpr` of type `ty` over `kids`.
    pub(crate) fn recover(&mut self, ty: QualType, kids: &[ExprId], loc: Loc) -> ExprId {
        self.builder().recovery(Some(ty), ValueCategory::PRValue, kids, loc)
    }

    /// A `RecoveryExpr` of error type over `kids`.
    fn recover_err(&mut self, kids: &[ExprId], loc: Loc) -> ExprId {
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

    /// True if an integer type `t` is narrower than `int`: arithmetic on
    /// it is done in `int` and cannot overflow (clang's
    /// `UnaryOperator::canOverflow` rule).
    fn narrower_than_int(&self, t: QualType) -> bool {
        if !self.is_integral_or_enum(t) {
            return false;
        }
        let int_w = conv::int_width(self, QualType::INT).unwrap_or(32);
        conv::int_width(self, t).is_some_and(|w| w < int_w)
    }

    // -- constants ---------------------------------------------------------------

    /// Evaluates `e` as a constant expression (through the
    /// [`ConstEvaluator`] seam, spec/sema.md §8).
    ///
    /// # Errors
    /// If `e` is not a constant expression; the notes explain why.
    pub fn evaluate(&self, e: ExprId) -> Result<ConstVal, NotConstant> {
        constfold::evaluator(&self.abi).evaluate(&self.ast, e)
    }

    /// `d` with the notes explaining a failed evaluation.
    pub(crate) fn with_not_constant_notes(&self, d: Diag, nc: NotConstant) -> Diag {
        let mut d = d;
        for (id, loc, args) in [nc.note, nc.decl_note].into_iter().flatten() {
            let n = args.into_iter().fold(self.diag(id, loc), Diag::arg);
            d = d.with_note(n);
        }
        d
    }

    /// Emits a conversion error at `loc`.
    pub(crate) fn emit_conv_error(&self, err: ConvError, loc: Loc) {
        let d = err.args.into_iter().fold(self.diag(err.id, loc), Diag::arg);
        self.emit(d);
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
            Scalar::Int { ty: t, signed, bits: IntBits::from_i128(w.clamp(1, 128), v) }
        };
        self.ast.consts.push_scalar(scalar).expect("integer constants intern")
    }

    /// Interns the floating constant `f` of type `t` (exact for `float`
    /// and `double`; other formats are not cached).
    fn float_value(&mut self, t: QualType, f: f64) -> Option<ConstValueId> {
        let b = self.ast.types.builtin(t)?;
        let kind = conv::narrow::float_kind(self, b)?;
        let bits = match kind {
            FloatKind::Double => u128::from(f.to_bits()),
            #[allow(clippy::cast_possible_truncation, reason = "rounding to the target format")]
            FloatKind::Single => u128::from((f as f32).to_bits()),
            _ => return None,
        };
        self.ast.consts.push_scalar(Scalar::Float { ty: t, kind, bits }).ok()
    }

    /// Interns the value `v` of an object of type `ty`, if representable.
    pub(crate) fn const_value(&mut self, ty: QualType, v: ConstVal) -> Option<ConstValueId> {
        let t = conv::strip_cv(self, ty);
        match v {
            ConstVal::Int(i) if int_info(&self.ast, t).is_some() => Some(self.int_const(t, i)),
            #[allow(clippy::cast_precision_loss, reason = "integral value of a floating object")]
            ConstVal::Int(i) if self.ast.types.is_floating(t) => self.float_value(t, i as f64),
            ConstVal::Float(f) if self.ast.types.is_floating(t) => self.float_value(t, f),
            ConstVal::Null if self.ast.types.is_pointer(t) || self.is_nullptr_type(t) => {
                self.ast.consts.push_scalar(Scalar::Null { ty: t, kind: NullKind::Pointer }).ok()
            }
            _ => None,
        }
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
                ExprKind::IntegerLiteral
                | ExprKind::FloatingLiteral
                | ExprKind::CharacterLiteral
                | ExprKind::CXXBoolLiteralExpr
                | ExprKind::CXXNullPtrLiteralExpr
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
                _ => break,
            }
        }
        e
    }

    fn ignore_parens(&self, e: ExprId) -> ExprId {
        let mut e = e;
        for _ in 0..256 {
            if self.ast.exprs.get(e).kind() == ExprKind::ParenExpr {
                e = self.ast.exprs.operand(e);
            } else {
                break;
            }
        }
        e
    }

    /// The operator and operands of a failed `static_assert` condition
    /// worth explaining (clang's `DiagnoseStaticAssertDetails`).
    pub(crate) fn comparison_parts(&self, e: ExprId) -> Option<(&'static str, ExprId, ExprId)> {
        let e = self.ignore_paren_imp_casts(e);
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

    /// The location of the first token of `e` (clang's `getBeginLoc`):
    /// a node whose own location is an operator or a name after its first
    /// operand begins where that operand does.
    pub(crate) fn begin_loc(&self, mut e: ExprId) -> Loc {
        loop {
            let n = self.ast.exprs.get(e);
            let first = match n.kind() {
                ExprKind::ImplicitCastExpr
                | ExprKind::ConstantExpr
                | ExprKind::BinaryOperator
                | ExprKind::CompoundAssignOperator
                | ExprKind::ConditionalOperator
                | ExprKind::ArraySubscriptExpr
                | ExprKind::CallExpr
                | ExprKind::MemberExpr => true,
                ExprKind::UnaryOperator => self.ast.exprs.unary_op(e).is_postfix(),
                _ => false,
            };
            let kids = if first { self.ast.exprs.children(e, &self.ast.lists) } else { Vec::new() };
            match kids.first() {
                Some(&k) => e = k,
                None => return n.loc(),
            }
        }
    }

    // -- full-expressions --------------------------------------------------------

    /// The end of a full-expression ([intro.execution]/5): resolves the
    /// potential odr-uses it contains ([basic.def.odr]/5, spec/sema.md
    /// §5.2). A variable usable in constant expressions whose name is the
    /// operand of an lvalue-to-rvalue conversion is not odr-used
    /// (`non_odr_use_constant`); every other named variable is `used`.
    pub fn act_on_full_expr(&mut self, e: ExprId) -> ExprId {
        if self.pending_odr.is_empty() {
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
            stack.extend(self.ast.exprs.children(x, &self.ast.lists));
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
        }
        e
    }

    /// The `DeclRefExpr`s among the potential results of `e`
    /// ([basic.def.odr]/3).
    fn potential_results(&self, e: ExprId, out: &mut Vec<ExprId>) {
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
    fn usable_in_constant_expressions(&self, d: DeclId) -> bool {
        let n = self.ast.decls.get(d);
        if n.kind() != DeclKind::Var {
            return false;
        }
        let Some(v) = self.ast.decls.var(d) else { return false };
        let Some(init) = v.init else { return false };
        let ty = n.ty().unwrap_or(QualType::ERROR);
        if self.ast.types.is_reference(ty) {
            return self.is_static_lvalue(init);
        }
        let q = self.ast.types.effective_quals(ty);
        let ok = v.spec.is_constexpr()
            || (q.is_const()
                && !q.is_volatile()
                && self.is_integral_or_enum(ty)
                && !self.ast.types.is_reference(ty));
        ok && self.evaluate(init).is_ok()
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
    fn has_static_storage(&self, d: DeclId) -> bool {
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

    /// Converts `e` to `ty` as in `ctx`, diagnosing failure (the result
    /// is then a `RecoveryExpr`).
    pub(crate) fn convert_for_init(&mut self, e: ExprId, ty: QualType, ctx: ConvCtx) -> ExprId {
        match conv::implicit_convert(self, e, ty, ctx) {
            Ok(x) => x,
            Err(err) => {
                let d = err.diag(self, e);
                self.emit(d);
                let loc = self.eloc(e);
                self.recover(ty, &[e], loc)
            }
        }
    }

    /// clang's `DiagnoseUnusedExprResult` for an expression statement.
    pub(crate) fn diagnose_unused(&mut self, e: ExprId) {
        if self.is_err_expr(e) {
            return;
        }
        if let Some((id, loc, arg)) = self.unused_warning(e) {
            let d = arg.into_iter().fold(self.diag(id, loc), Diag::arg);
            self.emit(d);
        }
    }

    fn unused_warning(&self, e: ExprId) -> Option<(DiagId, Loc, Vec<String>)> {
        let x = self.ignore_parens(e);
        let n = *self.ast.exprs.get(x);
        let loc = n.loc();
        match n.kind() {
            ExprKind::CallExpr => {
                let callee = self.ignore_paren_imp_casts(self.ast.exprs.callee(x));
                if self.ast.exprs.get(callee).kind() == ExprKind::DeclRefExpr {
                    let f = self.ast.exprs.decl(callee);
                    // A nodiscard class or enumeration returned by value
                    // counts too ([dcl.attr.nodiscard]/2).
                    let by_type = || {
                        let t = n.ty();
                        let tag = self.ast.types.as_record(t).or_else(|| self.ast.types.as_enum(t));
                        tag.and_then(|d| self.find_attr(d, AttrKind::NODISCARD))
                    };
                    if let Some(a) = self.find_attr(f, AttrKind::NODISCARD).or_else(by_type) {
                        // clang points at the callee and appends the
                        // attribute's message ([dcl.attr.nodiscard]/4).
                        let at = self.begin_loc(callee);
                        let attr = *self.ast.attrs.get(a);
                        let text = ExprId::from_raw(attr.a).and_then(|m| self.string_text(m));
                        let name = "'nodiscard'".to_owned();
                        return Some(match text {
                            Some(t) => (DiagId::sema_unused_result_msg, at, vec![name, t]),
                            None => (DiagId::sema_unused_result, at, vec![name]),
                        });
                    }
                }
                None
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
                    BinaryOp::Comma => self.unused_warning(self.ast.exprs.operands(x)[1]),
                    BinaryOp::LAnd | BinaryOp::LOr => {
                        // `a && f()` is control flow when the right side has effects.
                        let r = self.ast.exprs.operands(x)[1];
                        self.unused_warning(r).map(|_| (DiagId::sema_unused_expr, loc, Vec::new()))
                    }
                    _ if op.is_assignment() => None,
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
                let [t, f] = self.ast.exprs.cond_arms(x);
                (self.unused_warning(t).is_some() && self.unused_warning(f).is_some()).then_some((
                    DiagId::sema_unused_expr,
                    loc,
                    Vec::new(),
                ))
            }
            ExprKind::CStyleCastExpr | ExprKind::CXXFunctionalCastExpr => (self
                .ast
                .exprs
                .cast_kind(x)
                != CastKind::ToVoid)
                .then_some((DiagId::sema_unused_expr, loc, Vec::new())),
            ExprKind::ImplicitCastExpr => self.unused_warning(self.ast.exprs.operand(x)),
            ExprKind::DeclRefExpr | ExprKind::MemberExpr | ExprKind::ArraySubscriptExpr => {
                if self.ast.types.effective_quals(n.ty()).is_volatile() {
                    None
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
            | ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::CXXStaticCastExpr
            | ExprKind::CXXReinterpretCastExpr
            | ExprKind::CXXConstCastExpr => Some((DiagId::sema_unused_expr, loc, Vec::new())),
            _ => None,
        }
    }

    fn find_attr(&self, d: DeclId, kind: AttrKind) -> Option<AttrId> {
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

    /// An id-expression naming `name` ([expr.prim.id]), optionally
    /// qualified by the context `qualifier`.
    pub fn act_on_id_expression(
        &mut self,
        name: Symbol,
        loc: Loc,
        qualifier: Option<DeclId>,
    ) -> ExprId {
        let found = match qualifier {
            Some(q) => self.lookup_qualified(q, name, LookupKind::Ordinary),
            None => self.lookup_name(name, LookupKind::Ordinary),
        };
        let Some(&d) = found.decls.first() else {
            let text = self.ast.symbols().as_str(name).to_owned();
            let msg = match qualifier {
                Some(q) => {
                    self.diag(DiagId::sema_no_member, loc).arg(text).arg(self.qualifier_desc(q))
                }
                None => self.diag(DiagId::sema_undeclared_var_use, loc).arg(text),
            };
            self.emit(msg);
            return self.recover_err(&[], loc);
        };
        if found.decls.len() > 1 {
            if found.decls.iter().all(|&x| self.ast.decls.get(x).kind().is_function()) {
                self.unsupported("overloaded functions", loc);
                return self.recover_err(&[], loc);
            }
            // Distinct entities found via using-directives ([namespace.udir]/6).
            let text = self.ast.symbols().as_str(name).to_owned();
            let mut msg = self.diag(DiagId::sema_ambiguous_reference, loc).arg(text);
            for &c in &found.decls {
                let n = self
                    .diag(DiagId::note_ambiguous_candidate, self.ast.decls.get(c).loc())
                    .arg(self.qualified_decl_name(c));
                msg = msg.with_note(n);
            }
            self.emit(msg);
            return self.recover_err(&[], loc);
        }
        self.build_decl_ref(d, loc)
    }

    /// A `DeclRefExpr` naming the value `d`.
    pub fn build_decl_ref(&mut self, d: DeclId, loc: Loc) -> ExprId {
        let n = *self.ast.decls.get(d);
        match n.kind() {
            DeclKind::Var | DeclKind::ParmVar | DeclKind::Function | DeclKind::EnumConstant => {}
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
            DeclKind::Field | DeclKind::IndirectField => {
                self.unsupported("implicit member access", loc);
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
        let declared = n.ty().unwrap_or(QualType::ERROR);
        let ty = self.non_reference(declared);
        self.ast.decls.set_flags(d, DeclFlags::REFERENCED);
        self.diagnose_deprecated(d, loc);
        let nou = if self.unevaluated > 0 { NonOdrUse::Unevaluated } else { NonOdrUse::None };
        let e = self.builder().decl_ref(d, ty, loc, None, nou, ExprFlags::NONE);
        if self.unevaluated == 0 {
            match n.kind() {
                DeclKind::Function => {
                    let own = self
                        .func
                        .as_ref()
                        .is_some_and(|f| self.ast.first(f.decl) == self.ast.first(d));
                    if !own {
                        self.mark_used(d);
                    }
                }
                DeclKind::Var | DeclKind::ParmVar => self.pending_odr.push(e),
                _ => {}
            }
        }
        e
    }

    /// `[[deprecated]]` ([dcl.attr.deprecated]).
    fn diagnose_deprecated(&self, d: DeclId, loc: Loc) {
        let Some(a) = self.find_attr(d, AttrKind::DEPRECATED) else { return };
        let attr = *self.ast.attrs.get(a);
        let name = self.decl_name(d);
        let text = ExprId::from_raw(attr.a).and_then(|m| self.string_text(m));
        let msg = match text {
            Some(t) => self.diag(DiagId::sema_deprecated_msg, loc).arg(name.clone()).arg(t),
            None => self.diag(DiagId::sema_deprecated, loc).arg(name.clone()),
        };
        let note = self.diag(DiagId::note_marked_deprecated, attr.loc).arg(name);
        self.emit(msg.with_note(note));
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
    fn reject_ud_suffix(&mut self, ud: Option<&str>, ud_loc: Loc, operand: UdOperand) -> bool {
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
    fn const_char_ptr(&mut self, elem: QualType) -> QualType {
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
        let radix = match lit.radix {
            kiln_lex::literal::Radix::Binary => Radix::Binary,
            kiln_lex::literal::Radix::Octal => Radix::Octal,
            kiln_lex::literal::Radix::Decimal => Radix::Decimal,
            kiln_lex::literal::Radix::Hex => Radix::Hex,
        };
        match literal::classify_integer_literal(self, lit) {
            Ok(t) => {
                if let Some(w) = &t.warning {
                    let d = w.diag(self, loc);
                    self.emit(d);
                }
                if let Ok(v) = u64::try_from(t.value) {
                    self.builder().int_literal(t.ty, v, radix, loc)
                } else {
                    #[allow(clippy::cast_possible_wrap, reason = "bit pattern")]
                    let cv = self.int_const(t.ty, t.value as i128);
                    self.builder().wide_int_literal(t.ty, cv, radix, loc)
                }
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
                self.builder().float_literal(t.ty, cv, loc)
            }
            Err(err) => {
                let d = err.diag(self, loc);
                self.emit(d);
                self.recover_err(&[], loc)
            }
        }
    }

    /// A character literal ([lex.ccon]).
    pub fn act_on_char_literal(&mut self, lit: &CharLiteral<'_>, loc: Loc, ud_loc: Loc) -> ExprId {
        if lit.ud_suffix.is_some() {
            let ty = literal::type_char_literal(self, lit).map_or(QualType::CHAR, |x| x.0);
            self.reject_ud_suffix(lit.ud_suffix, ud_loc, UdOperand::Char(ty));
            return self.recover_err(&[], loc);
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
            let elem = match enc {
                Encoding::Ordinary => QualType::CHAR,
                Encoding::Wide => QualType::WCHAR,
                Encoding::Utf8 => QualType::CHAR8,
                Encoding::Utf16 => QualType::CHAR16,
                Encoding::Utf32 => QualType::CHAR32,
            };
            self.reject_ud_suffix(Some(ud), ud_loc, UdOperand::String(elem));
            return self.recover_err(&[], loc);
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
        self.builder().paren(e, lparen, rparen)
    }

    // -- assignment checks -------------------------------------------------------

    /// [expr.assign]/1, [expr.pre.incr]: the operand must be a modifiable
    /// lvalue. Diagnoses and returns false otherwise.
    fn check_modifiable(&mut self, e: ExprId, loc: Loc) -> bool {
        let n = *self.ast.exprs.get(e);
        let t = n.ty();
        if n.value_category() != ValueCategory::LValue
            || self.ast.types.is_array(t)
            || self.ast.types.is_function(t)
        {
            self.error(DiagId::sema_not_assignable, loc);
            return false;
        }
        if self.ast.types.effective_quals(t).is_const() {
            let base = self.ignore_parens(e);
            if self.ast.exprs.get(base).kind() == ExprKind::DeclRefExpr {
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
            } else {
                self.error(DiagId::sema_read_only_not_assignable, loc);
            }
            return false;
        }
        if self.ast.types.as_record(t).is_some() {
            self.unsupported("assignment of class objects", loc);
            return false;
        }
        true
    }

    fn mark_bitfield_if(&mut self, result: ExprId, from: ExprId) {
        if self.is_bitfield(from) && self.ast.exprs.get(result).value_category().is_glvalue() {
            self.ast.exprs.mark_bitfield(result);
        }
    }

    // -- unary operators ---------------------------------------------------------

    /// A prefix unary operator, or postfix `++`/`--` ([expr.unary.op],
    /// [expr.pre.incr], [expr.post.incr]).
    pub fn act_on_unary(&mut self, op: UnaryOp, e: ExprId, loc: Loc) -> ExprId {
        if self.is_err_expr(e) {
            return self.recover_err(&[e], loc);
        }
        let t0 = self.ety(e);
        let bad = |s: &mut Self, e: ExprId| {
            let msg = s.diag(DiagId::sema_invalid_unary_operand, loc).arg(s.ty_arg(t0));
            s.emit(msg);
            s.recover_err(&[e], loc)
        };
        match op {
            UnaryOp::Plus | UnaryOp::Minus => {
                let ok = self.ast.types.is_arithmetic(t0)
                    || conv::is_unscoped_enum(self, t0)
                    || (op == UnaryOp::Plus
                        && (self.ast.types.is_pointer(t0) || self.ast.types.is_array(t0)));
                if !ok {
                    return bad(self, e);
                }
                let x = conv::usual_unary_conversions(self, e);
                let ty = self.ety(x);
                let flags =
                    if op == UnaryOp::Plus || self.narrower_than_int(conv::strip_cv(self, t0)) {
                        ExprFlags::CANNOT_OVERFLOW
                    } else {
                        ExprFlags::NONE
                    };
                self.builder().unary(op, x, ty, loc, flags)
            }
            UnaryOp::Not => {
                if !(self.ast.types.is_integral(t0) || conv::is_unscoped_enum(self, t0)) {
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
        let n = *self.ast.exprs.get(e);
        if n.object_kind() == ObjectKind::BitField {
            self.error(DiagId::sema_addrof_bitfield, loc);
            return self.recover_err(&[e], loc);
        }
        if n.value_category() != ValueCategory::LValue {
            let msg = self.diag(DiagId::sema_addrof_rvalue, loc).arg(self.ty_arg(n.ty()));
            self.emit(msg);
            return self.recover_err(&[e], loc);
        }
        let ty = self.ast.types.pointer_to(n.ty());
        self.builder().unary(UnaryOp::AddrOf, e, ty, loc, ExprFlags::CANNOT_OVERFLOW)
    }

    fn inc_dec(&mut self, op: UnaryOp, e: ExprId, loc: Loc) -> ExprId {
        let t = self.ety(e);
        let inc = matches!(op, UnaryOp::PreInc | UnaryOp::PostInc);
        if self.ast.types.builtin(t) == Some(BuiltinType::Bool) {
            let msg = self.diag(DiagId::sema_incdec_bool, loc).arg(if inc { "0" } else { "1" });
            self.emit(msg);
            return self.recover_err(&[e], loc);
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
            if !self.check_pointer_arith(t, false, loc) {
                return self.recover_err(&[e], loc);
            }
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
        let flags =
            if self.narrower_than_int(t) { ExprFlags::CANNOT_OVERFLOW } else { ExprFlags::NONE };
        let ty = if op.is_postfix() { conv::strip_cv(self, t) } else { t };
        let r = self.builder().unary(op, e, ty, loc, flags);
        self.mark_bitfield_if(r, e);
        r
    }

    /// Pointer arithmetic needs a pointer to a complete object type
    /// ([expr.add]/1); `void` and function pointees are GNU extensions.
    fn check_pointer_arith(&mut self, pt: QualType, both: bool, loc: Loc) -> bool {
        let Some(p) = self.pointee_sugared(pt) else { return true };
        let sel = if both { "1" } else { "0" };
        if self.ast.types.is_void(p) {
            let msg = self.diag(DiagId::sema_arith_void_pointer, loc).arg(sel);
            self.emit(msg);
            return true;
        }
        if self.ast.types.is_function(p) {
            let msg =
                self.diag(DiagId::sema_arith_function_pointer, loc).arg(sel).arg(self.ty_arg(p));
            self.emit(msg);
            return true;
        }
        if !self.is_complete(p) {
            let msg = self.diag(DiagId::sema_arith_incomplete_pointee, loc).arg(self.ty_arg(p));
            self.emit(self.incomplete_note(msg, p));
            return false;
        }
        true
    }

    // -- binary operators --------------------------------------------------------

    fn invalid_operands(&mut self, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        let msg = self
            .diag(DiagId::sema_invalid_operands, loc)
            .arg(self.ty_arg(self.ety(l)))
            .arg(self.ty_arg(self.ety(r)));
        self.emit(msg);
        self.recover_err(&[l, r], loc)
    }

    fn conv_failed(&mut self, err: &ConvError, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        self.emit_conv_error(err.clone(), loc);
        self.recover_err(&[l, r], loc)
    }

    /// A binary operator ([expr.mul] .. [expr.comma]).
    pub fn act_on_binary(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        if self.is_err_expr(l) || self.is_err_expr(r) {
            return self.recover_err(&[l, r], loc);
        }
        match op {
            BinaryOp::Assign => self.assign(l, r, loc),
            _ if op.is_compound_assignment() => self.compound_assign(op, l, r, loc),
            BinaryOp::Comma => {
                let ty = self.ety(r);
                let res = self.builder().binary(op, l, r, ty, loc);
                self.mark_bitfield_if(res, r);
                res
            }
            BinaryOp::LAnd | BinaryOp::LOr => {
                let (Ok(l2), Ok(r2)) = (
                    conv::contextually_convert_to_bool(self, l),
                    conv::contextually_convert_to_bool(self, r),
                ) else {
                    return self.invalid_operands(l, r, loc);
                };
                self.builder().binary(op, l2, r2, QualType::BOOL, loc)
            }
            BinaryOp::Add | BinaryOp::Sub => self.additive(op, l, r, loc),
            BinaryOp::Shl | BinaryOp::Shr => {
                let (lt, rt) = (self.ety(l), self.ety(r));
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
                let (lt, rt) = (self.ety(l), self.ety(r));
                let arith = |s: &Self, t: QualType| {
                    s.ast.types.is_arithmetic(t) || conv::is_unscoped_enum(s, t)
                };
                if !arith(self, lt) || !arith(self, rt) {
                    return self.invalid_operands(l, r, loc);
                }
                match conv::usual_arithmetic_conversions_kind(self, l, r, kind) {
                    Ok((l2, r2, ty)) => {
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
            _ => {
                self.unsupported(&format!("operator '{}'", op.spelling()), loc);
                self.recover_err(&[l, r], loc)
            }
        }
    }

    /// clang's `-Wdivision-by-zero`: an integer divisor that folds to 0
    /// ([expr.mul]/4), outside unevaluated operands.
    fn diagnose_division_by_zero(&mut self, op: BinaryOp, r: ExprId, loc: Loc) {
        if self.unevaluated > 0 || !self.ast.types.is_integral(self.ety(r)) {
            return;
        }
        if matches!(self.evaluate(r), Ok(ConstVal::Int(0))) {
            let what = if matches!(op, BinaryOp::Div | BinaryOp::DivAssign) {
                "division"
            } else {
                "remainder"
            };
            let msg = self.diag(DiagId::sema_division_by_zero, loc).arg(what);
            self.emit(msg);
        }
    }

    /// clang's `-Wshift-count-negative`/`-Wshift-count-overflow`: a
    /// constant shift count outside `[0, width)` of the promoted left
    /// operand type `lty` ([expr.shift]/1).
    fn diagnose_shift_count(&mut self, r: ExprId, lty: QualType, loc: Loc) {
        if self.unevaluated > 0 {
            return;
        }
        let Some((width, _)) = int_info(&self.ast, lty) else { return };
        let Ok(ConstVal::Int(v)) = self.evaluate(r) else { return };
        let id = if v < 0 {
            DiagId::sema_shift_negative
        } else if v >= i128::from(width) {
            DiagId::sema_shift_gt_typewidth
        } else {
            return;
        };
        let msg = self.diag(id, loc);
        self.emit(msg);
    }

    fn assign(&mut self, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        if !self.check_modifiable(l, loc) {
            return self.recover_err(&[l, r], loc);
        }
        let lt = self.ety(l);
        let to = conv::strip_cv(self, lt);
        let r2 = match conv::implicit_convert(self, r, to, ConvCtx::Assignment) {
            Ok(x) => x,
            Err(err) => {
                let d = err.diag(self, r);
                self.emit(d);
                return self.recover_err(&[l, r], loc);
            }
        };
        let res = self.builder().binary(BinaryOp::Assign, l, r2, lt, loc);
        self.mark_bitfield_if(res, l);
        res
    }

    fn compound_assign(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        if !self.check_modifiable(l, loc) {
            return self.recover_err(&[l, r], loc);
        }
        let lt = self.ety(l);
        let rt = self.ety(r);
        let (r2, lhs_ty, res_ty) = match op {
            BinaryOp::AddAssign | BinaryOp::SubAssign if self.ast.types.is_pointer(lt) => {
                if !self.is_integral_or_enum(rt) || conv::is_scoped_enum(self, rt) {
                    return self.invalid_operands(l, r, loc);
                }
                if !self.check_pointer_arith(lt, false, loc) {
                    return self.recover_err(&[l, r], loc);
                }
                let r2 = conv::usual_unary_conversions(self, r);
                let p = conv::strip_cv(self, lt);
                (r2, p, p)
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
                (r2, p, p)
            }
            _ => {
                let arith = |s: &Self, t: QualType| {
                    s.ast.types.is_arithmetic(t) || conv::is_unscoped_enum(s, t)
                };
                if !arith(self, lt) || !arith(self, rt) {
                    return self.invalid_operands(l, r, loc);
                }
                match conv::usual_arithmetic_conversions_kind(
                    self,
                    l,
                    r,
                    ArithConvKind::CompoundAssign,
                ) {
                    Ok((_, r2, ty)) => {
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
                        (r2, ty, ty)
                    }
                    Err(err) => return self.conv_failed(&err, l, r, loc),
                }
            }
        };
        let res = self.builder().compound_assign(op, l, r2, lt, lhs_ty, res_ty, loc);
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
            if !same {
                let msg = self
                    .diag(DiagId::sema_sub_ptr_compatible, loc)
                    .arg(self.ty_arg(lt))
                    .arg(self.ty_arg(rt));
                self.emit(msg);
                return self.recover_err(&[l2, r2], loc);
            }
            if !self.check_pointer_arith(lt, true, loc) {
                return self.recover_err(&[l2, r2], loc);
            }
            let ty = QualType::unqual(self.target.ptrdiff_t);
            return self.builder().binary(op, l2, r2, ty, loc);
        }
        if lp && int_like(self, rt) {
            if !self.check_pointer_arith(lt, false, loc) {
                return self.recover_err(&[l2, r2], loc);
            }
            let r3 = conv::usual_unary_conversions(self, r2);
            return self.builder().binary(op, l2, r3, lt, loc);
        }
        if rp && int_like(self, lt) && op == BinaryOp::Add {
            if !self.check_pointer_arith(rt, false, loc) {
                return self.recover_err(&[l2, r2], loc);
            }
            let l3 = conv::usual_unary_conversions(self, l2);
            return self.builder().binary(op, l3, r2, rt, loc);
        }
        if lp || rp {
            return self.invalid_operands(l, r, loc);
        }
        let arith =
            |s: &Self, t: QualType| s.ast.types.is_arithmetic(t) || conv::is_unscoped_enum(s, t);
        if !arith(self, lt) || !arith(self, rt) {
            return self.invalid_operands(l, r, loc);
        }
        match conv::usual_arithmetic_conversions_kind(self, l, r, ArithConvKind::Arithmetic) {
            Ok((l3, r3, ty)) => self.builder().binary(op, l3, r3, ty, loc),
            Err(err) => self.conv_failed(&err, l, r, loc),
        }
    }

    /// The composite pointer type of two pointer operands
    /// ([expr.type]/4), converting the operands to it.
    fn composite_pointer(&mut self, l: ExprId, r: ExprId) -> Option<(ExprId, ExprId, QualType)> {
        let (lt, rt) = (self.ety(l), self.ety(r));
        let (lp, rp) = (
            self.ast.types.is_pointer(lt) || self.is_nullptr_type(lt),
            self.ast.types.is_pointer(rt) || self.is_nullptr_type(rt),
        );
        let (lnull, rnull) =
            (conv::is_null_pointer_constant(self, l), conv::is_null_pointer_constant(self, r));
        if self.is_nullptr_type(lt) && self.is_nullptr_type(rt) {
            return Some((l, r, lt));
        }
        if self.ast.types.is_pointer(lt) && (rnull || self.is_nullptr_type(rt)) {
            let r2 = conv::implicit_convert(self, r, lt, ConvCtx::Comparison).ok()?;
            return Some((l, r2, lt));
        }
        if self.ast.types.is_pointer(rt) && (lnull || self.is_nullptr_type(lt)) {
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
        if !(lp && rp) {
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
        None
    }

    fn comparison(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        let l2 = conv::decay(self, l);
        let r2 = conv::decay(self, r);
        let (lt, rt) = (self.ety(l2), self.ety(r2));
        let ptr_like = |s: &Self, t: QualType| s.ast.types.is_pointer(t) || s.is_nullptr_type(t);
        if ptr_like(self, lt) || ptr_like(self, rt) {
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
            let msg = self.diag(id, loc).arg(self.ty_arg(lt)).arg(self.ty_arg(rt));
            self.emit(msg);
            return self.recover_err(&[l2, r2], loc);
        }
        if !self.is_scalar(lt) || !self.is_scalar(rt) {
            return self.invalid_operands(l, r, loc);
        }
        match conv::usual_arithmetic_conversions_kind(self, l, r, ArithConvKind::Comparison) {
            Ok((l3, r3, _)) => self.builder().binary(op, l3, r3, QualType::BOOL, loc),
            Err(err) => self.conv_failed(&err, l, r, loc),
        }
    }

    /// `c ? t : f` ([expr.cond]).
    pub fn act_on_conditional(&mut self, c: ExprId, t: ExprId, f: ExprId, loc: Loc) -> ExprId {
        if self.is_err_expr(c) || self.is_err_expr(t) || self.is_err_expr(f) {
            return self.recover_err(&[c, t, f], loc);
        }
        let c2 = match conv::contextually_convert_to_bool(self, c) {
            Ok(x) => x,
            Err(err) => {
                let d = err.diag(self, c);
                self.emit(d);
                return self.recover_err(&[c, t, f], loc);
            }
        };
        let (tn, fnn) = (*self.ast.exprs.get(t), *self.ast.exprs.get(f));
        let (tt, ft) = (tn.ty(), fnn.ty());
        let (tv, fv) = (self.ast.types.is_void(tt), self.ast.types.is_void(ft));
        if tv && fv {
            return self.builder().conditional(c2, t, f, QualType::VOID, loc);
        }
        if tn.value_category().is_glvalue()
            && tn.value_category() == fnn.value_category()
            && self.ast.types.same_type(tt, ft)
            && !tv
        {
            let res = self.builder().conditional(c2, t, f, tt, loc);
            if self.is_bitfield(t) || self.is_bitfield(f) {
                self.ast.exprs.mark_bitfield(res);
            }
            return res;
        }
        // [expr.cond]/4.1: an lvalue of a less cv-qualified type binds to a
        // reference to the other operand's type.
        if tn.value_category() == ValueCategory::LValue
            && fnn.value_category() == ValueCategory::LValue
            && !tv
            && !fv
            && self.ast.types.as_record(tt).is_none()
            && self.ast.types.same_unqualified(tt, ft)
        {
            let (tq, fq) = (self.ast.types.effective_quals(tt), self.ast.types.effective_quals(ft));
            let lv = ValueCategory::LValue;
            let pair = if tq.contains(fq) {
                Some((t, conv::raw_cast(self, CastKind::NoOp, f, tt, lv, None), tt))
            } else if fq.contains(tq) {
                Some((conv::raw_cast(self, CastKind::NoOp, t, ft, lv, None), f, ft))
            } else {
                None
            };
            if let Some((t2, f2, ty)) = pair {
                let res = self.builder().conditional(c2, t2, f2, ty, loc);
                if self.is_bitfield(t) || self.is_bitfield(f) {
                    self.ast.exprs.mark_bitfield(res);
                }
                return res;
            }
        }
        let arith = |s: &Self, q: QualType| {
            s.ast.types.is_arithmetic(q) || s.ast.types.as_enum(q).is_some()
        };
        // [expr.cond]/7: after the standard conversions, operands of the
        // same type give that type (no promotion: `b ? s1 : s2` is
        // `short`).
        if !tv && !fv && arith(self, tt) && arith(self, ft) {
            let t2 = conv::decay(self, t);
            let f2 = conv::decay(self, f);
            let (t2ty, f2ty) = (self.ety(t2), self.ety(f2));
            if self.ast.types.same_type(t2ty, f2ty) {
                return self.builder().conditional(c2, t2, f2, t2ty, loc);
            }
        }
        if arith(self, tt) && arith(self, ft) {
            return match conv::usual_arithmetic_conversions_kind(
                self,
                t,
                f,
                ArithConvKind::Conditional,
            ) {
                Ok((t2, f2, ty)) => self.builder().conditional(c2, t2, f2, ty, loc),
                Err(err) => self.conv_failed(&err, t, f, loc),
            };
        }
        if !tv && !fv {
            let t2 = conv::decay(self, t);
            let f2 = conv::decay(self, f);
            if let Some((t3, f3, ty)) = self.composite_pointer(t2, f2) {
                return self.builder().conditional(c2, t3, f3, ty, loc);
            }
            if self.ast.types.as_record(tt).is_some() && self.ast.types.same_unqualified(tt, ft) {
                let ty = conv::strip_cv(self, tt);
                return self.builder().conditional(c2, t2, f2, ty, loc);
            }
        }
        let msg = self
            .diag(DiagId::sema_incompatible_cond_operands, loc)
            .arg(self.ty_arg(tt))
            .arg(self.ty_arg(ft));
        self.emit(msg);
        self.recover_err(&[c, t, f], loc)
    }

    // -- postfix -----------------------------------------------------------------

    /// The function prototype of `q` as written: sugar is looked through
    /// only down to the first function type, so parameter types keep
    /// their typedef names (clang converts arguments to them).
    fn sugared_proto(&self, q: QualType) -> Option<kiln_ast::types::FuncProto> {
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

    /// `e1[e2]` ([expr.sub]).
    pub fn act_on_subscript(&mut self, base: ExprId, idx: ExprId, loc: Loc) -> ExprId {
        if self.is_err_expr(base) || self.is_err_expr(idx) {
            return self.recover_err(&[base, idx], loc);
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
        if !self.ast.types.is_function(elem) && !self.is_complete(elem) {
            let msg = self.diag(DiagId::sema_arith_incomplete_pointee, loc).arg(self.ty_arg(elem));
            self.emit(self.incomplete_note(msg, elem));
            return self.recover_err(&[b, i], loc);
        }
        self.builder().subscript(b, i, elem, loc)
    }

    /// Reports argument `a` (the `i`th, 0-based) not converting to its
    /// parameter type `pt`: clang's "no matching function" with the
    /// candidate's note for a named callee, else the conversion error.
    fn bad_argument(
        &mut self,
        named: Option<DeclId>,
        callee: ExprId,
        a: ExprId,
        pt: QualType,
        i: usize,
        err: &ConvError,
    ) {
        let Some(d) = named else {
            let msg = err.diag(self, a);
            self.emit(msg);
            return;
        };
        let msg = self
            .diag(DiagId::sema_ovl_no_viable_function_in_call, self.begin_loc(callee))
            .arg(self.decl_name(d));
        let detail = format!(
            "no known conversion from {} to {} for {} argument",
            self.ty_arg(self.ety(a)),
            self.ty_arg(pt),
            ordinal(i + 1)
        );
        let fl = self.ast.decls.get(d).loc();
        let note = self.diag(DiagId::note_ovl_candidate_not_viable, fl).arg(detail);
        self.emit(msg.with_note(note));
    }

    /// A function call `f(args)` ([expr.call]). `loc` is the callee's
    /// location; `rparen` the closing parenthesis.
    pub fn act_on_call(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut kids = vec![callee];
        kids.extend_from_slice(args);
        if self.is_err_expr(callee) || args.iter().any(|&a| self.is_err_expr(a)) {
            return self.recover_err(&kids, loc);
        }
        let named = {
            let x = self.ignore_parens(callee);
            (self.ast.exprs.get(x).kind() == ExprKind::DeclRefExpr).then(|| self.ast.exprs.decl(x))
        };
        let f = conv::decay(self, callee);
        let ft = self.ety(f);
        let fn_ty = self.pointee_sugared(ft).filter(|p| self.ast.types.is_function(*p));
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
        let params: Vec<QualType> = self.ast.types.params(&proto).to_vec();
        let ret = proto.ret();
        let variadic = proto.info().variadic();
        let n = params.len();
        let have = args.len();
        if have < n || (have > n && !variadic) {
            let few = have < n;
            if let Some(d) = named {
                let msg = self
                    .diag(DiagId::sema_ovl_no_viable_function_in_call, self.begin_loc(callee))
                    .arg(self.decl_name(d));
                let detail = self.arity_detail(d, n, have, variadic);
                let fl = self.ast.decls.get(d).loc();
                let note = self.diag(DiagId::note_ovl_candidate_not_viable, fl).arg(detail);
                self.emit(msg.with_note(note));
            } else {
                let id = match (few, variadic) {
                    (true, true) => DiagId::sema_call_too_few_args_at_least,
                    (true, false) => DiagId::sema_call_too_few_args,
                    (false, _) => DiagId::sema_call_too_many_args,
                };
                let msg = self.diag(id, loc).arg(n).arg(have);
                self.emit(msg);
            }
            return self.recover_err(&kids, loc);
        }
        let mut conv_args = Vec::with_capacity(have);
        for (i, &a) in args.iter().enumerate() {
            if i < n {
                let pt = conv::strip_cv(self, params[i]);
                let to = if self.ast.types.is_reference(params[i]) { params[i] } else { pt };
                match conv::implicit_convert(self, a, to, ConvCtx::Argument) {
                    Ok(x) => conv_args.push(x),
                    Err(err) => {
                        self.bad_argument(named, callee, a, params[i], i, &err);
                        return self.recover_err(&kids, loc);
                    }
                }
            } else {
                conv_args.push(self.default_argument_promotion(a));
            }
        }
        let (ty, vk) = if self.ast.types.is_reference(ret) {
            let lvalue = matches!(self.ast.types.canonical_view(ret), TypeView::LValueRef(_));
            (
                self.non_reference(ret),
                if lvalue { ValueCategory::LValue } else { ValueCategory::XValue },
            )
        } else {
            (conv::strip_cv(self, ret), ValueCategory::PRValue)
        };
        if !self.ast.types.is_void(ty)
            && self.ast.types.as_record(ty).is_some()
            && !self.is_complete(ty)
        {
            let msg =
                self.diag(DiagId::sema_typecheck_decl_incomplete_type, loc).arg(self.ty_arg(ty));
            self.emit(self.incomplete_note(msg, ty));
        }
        self.builder().call(f, &conv_args, ty, vk, loc, rparen, ExprFlags::NONE)
    }

    /// clang's "candidate function not viable" detail for an arity
    /// mismatch.
    fn arity_detail(&self, d: DeclId, n: usize, have: usize, variadic: bool) -> String {
        let provided = match have {
            0 => "no arguments were provided".to_owned(),
            1 => "1 was provided".to_owned(),
            k => format!("{k} were provided"),
        };
        let pname = self
            .ast
            .decls
            .function(d)
            .and_then(|f| self.ast.lists.at::<DeclId>(f.params, 0))
            .map(|p| self.decl_name(p))
            .unwrap_or_default();
        if n == 1 && !variadic && !pname.is_empty() {
            return format!("requires single argument '{pname}', but {provided}");
        }
        let need = if variadic { format!("at least {n}") } else { n.to_string() };
        let plural = if n == 1 { "argument" } else { "arguments" };
        format!("requires {need} {plural}, but {provided}")
    }

    /// The default argument promotions for a variadic argument
    /// ([expr.call]/12).
    fn default_argument_promotion(&mut self, a: ExprId) -> ExprId {
        let x = conv::decay(self, a);
        let t = self.ety(x);
        if self.ast.types.builtin(t) == Some(BuiltinType::Float) {
            return conv::implicit_convert(self, x, QualType::DOUBLE, ConvCtx::Argument)
                .unwrap_or(x);
        }
        if self.is_integral_or_enum(t) && !conv::is_scoped_enum(self, t) {
            return conv::integral_promotion(self, x);
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
        if self.is_err_expr(base) {
            return self.recover_err(&[base], loc);
        }
        let (b, obj_ty) = if arrow {
            let b = conv::decay(self, base);
            let bt = self.ety(b);
            match self.pointee_sugared(bt).filter(|_| self.ast.types.is_pointer(bt)) {
                Some(p) if self.ast.types.as_record(p).is_some() => (b, p),
                _ => {
                    let msg = if self.ast.types.as_record(bt).is_some() {
                        self.diag(DiagId::sema_member_reference_suggestion, op_loc)
                            .arg(self.ty_arg(bt))
                            .arg("1")
                    } else {
                        self.diag(DiagId::sema_member_reference_struct_union, op_loc)
                            .arg(self.ty_arg(bt))
                    };
                    self.emit(msg);
                    return self.recover_err(&[base], loc);
                }
            }
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
                return self.recover_err(&[base], loc);
            }
            (base, bt)
        };
        let rec = self.ast.types.as_record(obj_ty).expect("checked above");
        let Some(def) = self.ast.definition(rec).filter(|_| self.is_complete(obj_ty)) else {
            let msg =
                self.diag(DiagId::sema_member_reference_incomplete, loc).arg(self.ty_arg(obj_ty));
            self.emit(self.incomplete_note(msg, obj_ty));
            return self.recover_err(&[base], loc);
        };
        let found = self.lookup_in(def, name, LookupKind::Ordinary);
        let Some(&m) = found.decls.first() else {
            let msg = self
                .diag(DiagId::sema_no_member, loc)
                .arg(self.ast.symbols().as_str(name).to_owned())
                .arg(self.ty_arg(obj_ty));
            self.emit(msg);
            return self.recover_err(&[base], loc);
        };
        let quals = self.ast.types.effective_quals(obj_ty);
        match self.ast.decls.get(m).kind() {
            DeclKind::Field => Some(self.field_access(b, m, quals, arrow, loc)),
            DeclKind::IndirectField => {
                let list = ListId::from_raw(self.ast.decls.get(m).data());
                let chain: Vec<DeclId> =
                    list.map(|l| self.ast.lists.iter::<DeclId>(l).collect()).unwrap_or_default();
                self.ast.decls.set_flags(m, DeclFlags::REFERENCED);
                let mut cur = b;
                let mut arrow_now = arrow;
                let mut q = quals;
                for f in chain {
                    cur = self.field_access(cur, f, q, arrow_now, loc);
                    q = self.ast.types.effective_quals(self.ety(cur));
                    arrow_now = false;
                }
                Some(cur)
            }
            _ => None,
        }
        .unwrap_or_else(|| {
            self.unsupported("this kind of member access", loc);
            self.recover_err(&[base], loc)
        })
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

    fn field_access(
        &mut self,
        base: ExprId,
        f: DeclId,
        quals: kiln_ast::types::Quals,
        arrow: bool,
        loc: Loc,
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
        self.builder().member(base, f, ty, arrow, loc, None)
    }

    // -- sizeof / alignof --------------------------------------------------------

    /// Enters an unevaluated operand ([expr.context]/1).
    pub fn enter_unevaluated(&mut self) {
        self.unevaluated += 1;
    }

    /// Leaves an unevaluated operand.
    pub fn exit_unevaluated(&mut self) {
        self.unevaluated = self.unevaluated.saturating_sub(1);
    }

    fn trait_name(kind: TraitKind) -> &'static str {
        match kind {
            TraitKind::AlignOf => "alignof",
            TraitKind::PreferredAlignOf => "__alignof",
            _ => "sizeof",
        }
    }

    /// Checks the operand type of `sizeof`/`alignof` ([expr.sizeof]/1,
    /// [expr.alignof]/1).
    fn check_trait_operand(&mut self, kind: TraitKind, ty: QualType, loc: Loc) -> bool {
        let name = Self::trait_name(kind);
        if self.is_error_type(ty) {
            return false;
        }
        let t = if kind == TraitKind::SizeOf { self.non_reference(ty) } else { ty };
        if self.ast.types.is_function(t) {
            let msg = self.diag(DiagId::sema_sizeof_function, loc).arg(name);
            self.emit(msg);
            return true;
        }
        if self.ast.types.is_void(t) {
            let msg = self.diag(DiagId::sema_sizeof_void, loc).arg(name);
            self.emit(msg);
            return true;
        }
        let check = if kind == TraitKind::SizeOf {
            t
        } else {
            let mut x = self.non_reference(t);
            while let Some(e) = self.ast.types.element(x).filter(|_| self.ast.types.is_array(x)) {
                x = e;
            }
            x
        };
        if !self.is_complete(check) {
            let msg = self.diag(DiagId::sema_sizeof_incomplete, loc).arg(name).arg(self.ty_arg(ty));
            self.emit(self.incomplete_note(msg, ty));
            return false;
        }
        true
    }

    /// `sizeof(T)` / `alignof(T)`.
    pub fn act_on_trait_type(
        &mut self,
        kind: TraitKind,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        if !self.check_trait_operand(kind, ty, loc) {
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
        if self.is_err_expr(e) {
            return self.recover_err(&[e], loc);
        }
        if self.is_bitfield(e) {
            let msg = self.diag(DiagId::sema_sizeof_bitfield, loc).arg(Self::trait_name(kind));
            self.emit(msg);
            return self.recover_err(&[e], loc);
        }
        let ty = self.ety(e);
        // [expr.sizeof]/1: in C++ an expression of function type is an
        // error, not the GNU extension the type form gets.
        if self.ast.types.is_function(ty) {
            let msg = self.diag(DiagId::sema_sizeof_function_expr, self.eloc(e));
            self.emit(msg.arg(Self::trait_name(kind)));
            return self.recover_err(&[e], loc);
        }
        if !self.check_trait_operand(kind, ty, loc) {
            return self.recover_err(&[e], loc);
        }
        let st = self.size_type();
        self.builder().trait_of_expr(kind, e, st, loc)
    }

    // -- casts -------------------------------------------------------------------

    /// Marks the implicit conversions applied on top of `orig` (down to
    /// it) as part of an explicit cast.
    fn mark_explicit(&mut self, top: ExprId, orig: ExprId) {
        let mut cur = top;
        for _ in 0..64 {
            if cur == orig || self.ast.exprs.get(cur).kind() != ExprKind::ImplicitCastExpr {
                break;
            }
            self.ast.exprs.mark_part_of_explicit_cast(cur);
            cur = self.ast.exprs.operand(cur);
        }
    }

    /// An explicit type conversion `style<ty>(e)` ([expr.cast],
    /// [expr.static.cast], [expr.reinterpret.cast], [expr.const.cast],
    /// [expr.type.conv]). `loc` is the cast's start.
    #[allow(clippy::too_many_lines, reason = "one step per paragraph of [expr.cast]/4")]
    pub fn act_on_cast(&mut self, style: CastStyle, ty: QualType, e: ExprId, loc: Loc) -> ExprId {
        if self.is_err_expr(e) || self.is_error_type(ty) {
            return self.recover(ty, &[e], loc);
        }
        if style == CastStyle::Dynamic {
            self.unsupported("dynamic_cast", loc);
            return self.recover(ty, &[e], loc);
        }
        if self.ast.types.is_reference(ty) {
            self.unsupported("casts to reference types", loc);
            return self.recover(ty, &[e], loc);
        }
        if self.ast.types.as_record(ty).is_some() || self.ast.types.as_record(self.ety(e)).is_some()
        {
            self.unsupported("conversions of class types", loc);
            return self.recover(ty, &[e], loc);
        }
        let kind = style.kind();
        let to = conv::strip_cv(self, ty);
        let flags = ExprFlags::NONE;
        // [expr.static.cast]/6: any expression can be converted to cv void.
        if self.ast.types.is_void(ty) {
            if style.allows_static() {
                return self.builder().cast(
                    kind,
                    CastKind::ToVoid,
                    e,
                    ty,
                    ValueCategory::PRValue,
                    None,
                    loc,
                    flags,
                );
            }
            return self.bad_cast(style, e, ty, loc);
        }
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
                return self.builder().cast(
                    kind,
                    CastKind::NoOp,
                    x,
                    to,
                    ValueCategory::PRValue,
                    None,
                    loc,
                    flags,
                );
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
            match conv::apply_standard_conversion(self, e, &scs, to) {
                Ok(x) => {
                    self.mark_explicit(x, e);
                    return self.builder().cast(
                        kind,
                        CastKind::NoOp,
                        x,
                        to,
                        ValueCategory::PRValue,
                        None,
                        loc,
                        flags,
                    );
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
            return self.builder().cast(kind, ck, x, to, ValueCategory::PRValue, None, loc, flags);
        }
        // reinterpret_cast ([expr.reinterpret.cast]).
        if style.allows_reinterpret()
            && let Some(ck) = self.reinterpret(xt, to, style, loc)
        {
            return self.builder().cast(kind, ck, x, to, ValueCategory::PRValue, None, loc, flags);
        }
        self.bad_cast(style, e, ty, loc)
    }

    fn bad_cast(&mut self, style: CastStyle, e: ExprId, ty: QualType, loc: Loc) -> ExprId {
        let from = self.ety(e);
        let msg = self
            .diag(DiagId::sema_bad_cxx_cast, loc)
            .arg(style.select())
            .arg(self.ty_arg(from))
            .arg(self.ty_arg(ty));
        self.emit(msg);
        self.recover(ty, &[e], loc)
    }

    /// Pointer types differing only in cv-qualification at any level
    /// ([conv.qual]/2 "similar").
    fn similar_pointers(&self, a: QualType, b: QualType) -> bool {
        let (mut a, mut b) = (a, b);
        for _ in 0..64 {
            match (self.ast.types.pointee(a), self.ast.types.pointee(b)) {
                (Some(x), Some(y))
                    if self.ast.types.is_pointer(a) && self.ast.types.is_pointer(b) =>
                {
                    a = x;
                    b = y;
                }
                _ => return self.ast.types.same_unqualified(a, b),
            }
        }
        false
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
                return Some(CastKind::IntegralCast);
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

    fn reinterpret(
        &mut self,
        from: QualType,
        to: QualType,
        style: CastStyle,
        loc: Loc,
    ) -> Option<CastKind> {
        let (fp, tp) = (self.ast.types.is_pointer(from), self.ast.types.is_pointer(to));
        if fp && tp {
            if style == CastStyle::Reinterpret && self.casts_away_quals(from, to) {
                return None;
            }
            return Some(CastKind::BitCast);
        }
        if tp && self.is_integral_or_enum(from) {
            return Some(CastKind::IntegralToPointer);
        }
        if fp && self.is_integral_or_enum(to) {
            let pw = u32::try_from(self.abi.pointer_size * 8).unwrap_or(64);
            if conv::int_width(self, to).is_some_and(|w| w < pw) {
                let msg =
                    self.diag(DiagId::sema_reinterpret_cast_small_int, loc).arg(self.ty_arg(to));
                self.emit(msg);
            }
            return Some(CastKind::PointerToIntegral);
        }
        if style == CastStyle::Reinterpret
            && self.is_integral_or_enum(from)
            && self.ast.types.same_unqualified(from, to)
        {
            return Some(CastKind::NoOp);
        }
        None
    }
}

/// "1st", "2nd", "3rd", "4th", ...
fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, x) if x != 11 => "st",
        (2, x) if x != 12 => "nd",
        (3, x) if x != 13 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// The operand kind of a rejected user-defined literal, which selects
/// clang's wording for the failed literal-operator lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UdOperand {
    Integer,
    Float,
    /// A character literal of the given type.
    Char(QualType),
    /// A string literal of the given element type.
    String(QualType),
}
