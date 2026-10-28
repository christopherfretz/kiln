//! The M2 constant folder (spec/sema.md §8): evaluates integral, boolean
//! and floating constant expressions over the typed AST for array bounds,
//! bit-field widths, enumerator values, `case` labels, `static_assert`
//! and `constexpr` variable initializers.
//!
//! It is a stand-in for the Oven (`kiln_oven`): sema only reaches it
//! through [`ConstEvaluator`] and [`evaluator`], so switching to the Oven
//! is a one-line change to [`evaluator`]. The folder executes no function
//! calls (`constexpr` functions are the Oven's, M3).

use kiln_ast::consts::{FloatKind, Scalar};
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, CastKind, ExprFlags, ExprKind, TraitKind, UnaryOp};
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType, RawId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::target::{TargetLayout, layout};

/// A folded value.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ConstVal {
    /// An integral or enumeration value (bool is 0/1), sign- or
    /// zero-extended from its type's width.
    Int(i128),
    /// A floating value.
    Float(f64),
    /// A null pointer (`nullptr` or a null pointer conversion).
    Null,
}

impl ConstVal {
    /// The value as an integer, if it is one.
    pub fn as_int(self) -> Option<i128> {
        match self {
            ConstVal::Int(v) => Some(v),
            _ => None,
        }
    }

    /// True if the value is "true" in a boolean context.
    pub fn truthy(self) -> bool {
        match self {
            ConstVal::Int(v) => v != 0,
            ConstVal::Float(f) => f != 0.0,
            ConstVal::Null => false,
        }
    }
}

/// Why an expression is not a constant expression: the note clang
/// attaches (`note_constexpr_*`), if any.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct NotConstant {
    /// `(id, location, arguments)` of the explanatory note.
    pub note: Option<(DiagId, Loc, Vec<String>)>,
    /// A secondary note (`declared here`), if any.
    pub decl_note: Option<(DiagId, Loc, Vec<String>)>,
}

impl NotConstant {
    fn plain() -> Self {
        NotConstant::default()
    }

    fn note(id: DiagId, loc: Loc, args: Vec<String>) -> Self {
        NotConstant { note: Some((id, loc, args)), decl_note: None }
    }
}

/// A constant evaluator (spec/sema.md §8).
pub trait ConstEvaluator {
    /// Evaluates `e` as a core constant expression.
    fn evaluate(&self, ast: &Ast, e: ExprId) -> Result<ConstVal, NotConstant>;
}

/// The evaluator sema uses. Swap for the Oven here.
pub fn evaluator() -> impl ConstEvaluator {
    Folder
}

/// Folds `e` to an integer (for callers without a `Sema`, e.g. layout).
pub fn fold_int_expr(ast: &Ast, e: ExprId) -> Option<i128> {
    evaluator().evaluate(ast, e).ok().and_then(ConstVal::as_int)
}

/// The M2 folder.
#[derive(Clone, Copy, Debug, Default)]
pub struct Folder;

/// `(width, signed)` of an integral or enumeration type.
pub fn int_info(ast: &Ast, ty: QualType) -> Option<(u32, bool)> {
    let t = &ast.types;
    let c = t.canonical(ty);
    match t.view(c.ty()) {
        TypeView::Builtin(b) => {
            if b == BuiltinType::Bool {
                return Some((1, false));
            }
            if !b.is_integral() {
                return None;
            }
            let bytes = t.target().builtin_size_align(b).0;
            let signed = match b {
                BuiltinType::Char => t.target().char_signed,
                BuiltinType::WChar => t.target().wchar_signed,
                _ => b.is_signed_integer(),
            };
            Some((u32::from(bytes) * 8, signed))
        }
        TypeView::Enum(d) => {
            let e = ast.decls.enumeration(ast.definition(d).unwrap_or(d))?;
            int_info(ast, e.int_type?)
        }
        _ => None,
    }
}

/// Truncates `v` to `(width, signed)`, wrapping.
pub fn wrap(v: i128, width: u32, signed: bool) -> i128 {
    if width >= 128 {
        return v;
    }
    let mask = (1i128 << width) - 1;
    let u = v & mask;
    if signed && width > 0 && (u >> (width - 1)) & 1 == 1 { u - (1i128 << width) } else { u }
}

/// True if `v` is representable in `(width, signed)`.
pub fn fits(v: i128, width: u32, signed: bool) -> bool {
    wrap(v, width, signed) == v
}

const DEPTH_LIMIT: u32 = 512;

impl ConstEvaluator for Folder {
    fn evaluate(&self, ast: &Ast, e: ExprId) -> Result<ConstVal, NotConstant> {
        Eval { ast, depth: 0 }.eval(e)
    }
}

struct Eval<'s> {
    ast: &'s Ast,
    depth: u32,
}

impl Eval<'_> {
    /// `q` quoted as a diagnostic argument.
    fn ty_name(&self, q: QualType) -> String {
        let names = crate::names::SemaNames::of_ast(self.ast);
        let s = kiln_ast::types::print_type(
            &self.ast.types,
            &names,
            q,
            kiln_ast::types::PrintOptions::SUGARED,
        );
        format!("'{s}'")
    }

    fn name(&self, d: DeclId) -> String {
        let n = self.ast.decls.get(d).name();
        self.ast.names.simple_spelling(n, self.ast.symbols()).unwrap_or_default()
    }

    fn scalar(&self, id: ConstValueId) -> Result<ConstVal, NotConstant> {
        match self.ast.consts.scalar(id) {
            Some(Scalar::Int { signed, bits, .. }) => {
                let v = if signed { bits.to_i128() } else { bits.to_u128().map(|u| u as i128) };
                v.map(ConstVal::Int).ok_or_else(NotConstant::plain)
            }
            Some(Scalar::Bool { value, .. }) => Ok(ConstVal::Int(i128::from(value))),
            Some(Scalar::Float { kind, bits, .. }) => match kind {
                FloatKind::Double => Ok(ConstVal::Float(f64::from_bits(bits as u64))),
                FloatKind::Single => Ok(ConstVal::Float(f64::from(f32::from_bits(bits as u32)))),
                // TODO(M3): half, bfloat16 and quad arithmetic (the Oven).
                _ => Err(NotConstant::plain()),
            },
            Some(Scalar::Null { .. }) => Ok(ConstVal::Null),
            _ => Err(NotConstant::plain()),
        }
    }

    fn to_type(&self, v: ConstVal, ty: QualType, loc: Loc) -> Result<ConstVal, NotConstant> {
        let t = &self.ast.types;
        if let Some((w, s)) = int_info(self.ast, ty) {
            return match v {
                ConstVal::Int(i) => {
                    if w == 1 {
                        Ok(ConstVal::Int(i128::from(i != 0)))
                    } else {
                        Ok(ConstVal::Int(wrap(i, w, s)))
                    }
                }
                ConstVal::Float(f) => {
                    if w == 1 {
                        return Ok(ConstVal::Int(i128::from(f != 0.0)));
                    }
                    let tr = f.trunc();
                    #[allow(clippy::cast_precision_loss, reason = "range check only")]
                    let ok = tr.is_finite() && tr >= -(2f64.powi(127)) && tr < 2f64.powi(127);
                    #[allow(clippy::cast_possible_truncation, reason = "range checked")]
                    let i = if ok { tr as i128 } else { 0 };
                    if ok && fits(i, w, s) {
                        Ok(ConstVal::Int(i))
                    } else {
                        Err(NotConstant::note(
                            DiagId::note_constexpr_overflow,
                            loc,
                            vec![format_float(f), self.ty_name(ty)],
                        ))
                    }
                }
                ConstVal::Null => Ok(ConstVal::Int(0)),
            };
        }
        if t.is_floating(ty) {
            let single = t.builtin(t.canonical(ty)) == Some(BuiltinType::Float);
            #[allow(clippy::cast_precision_loss, reason = "integral-to-floating conversion")]
            let f = match v {
                ConstVal::Int(i) => i as f64,
                ConstVal::Float(f) => f,
                ConstVal::Null => return Err(NotConstant::plain()),
            };
            #[allow(clippy::cast_possible_truncation, reason = "rounding to float")]
            let f = if single { f64::from(f as f32) } else { f };
            return Ok(ConstVal::Float(f));
        }
        if t.is_pointer(ty) || t.builtin(t.canonical(ty)) == Some(BuiltinType::NullPtr) {
            return match v {
                ConstVal::Null => Ok(ConstVal::Null),
                ConstVal::Int(0) => Ok(ConstVal::Null),
                _ => Err(NotConstant::plain()),
            };
        }
        Err(NotConstant::plain())
    }

    fn eval(&mut self, e: ExprId) -> Result<ConstVal, NotConstant> {
        self.depth += 1;
        if self.depth > DEPTH_LIMIT {
            return Err(NotConstant::plain());
        }
        let r = self.eval_inner(e);
        self.depth -= 1;
        r
    }

    #[allow(clippy::too_many_lines, reason = "one arm per expression kind")]
    fn eval_inner(&mut self, e: ExprId) -> Result<ConstVal, NotConstant> {
        let n = *self.ast.exprs.get(e);
        let ty = n.ty();
        match n.kind() {
            ExprKind::IntegerLiteral => {
                if n.has(ExprFlags::WIDE) {
                    self.scalar(ConstValueId::new(n.a()))
                } else {
                    let v = u64::from(n.a()) | (u64::from(n.b()) << 32);
                    self.to_type(ConstVal::Int(i128::from(v)), ty, n.loc())
                }
            }
            ExprKind::CharacterLiteral => {
                self.to_type(ConstVal::Int(i128::from(n.a())), ty, n.loc())
            }
            ExprKind::CXXBoolLiteralExpr => Ok(ConstVal::Int(i128::from(n.a() != 0))),
            ExprKind::FloatingLiteral => self.scalar(ConstValueId::new(n.a())),
            ExprKind::CXXNullPtrLiteralExpr => Ok(ConstVal::Null),
            ExprKind::ParenExpr => self.eval(ExprId::new(n.a())),
            ExprKind::ConstantExpr => {
                if n.b() != 0 {
                    self.scalar(ConstValueId::new(n.b()))
                } else {
                    self.eval(ExprId::new(n.a()))
                }
            }
            ExprKind::ImplicitCastExpr
            | ExprKind::CStyleCastExpr
            | ExprKind::CXXFunctionalCastExpr
            | ExprKind::CXXStaticCastExpr
            | ExprKind::CXXConstCastExpr
            | ExprKind::CXXReinterpretCastExpr => self.cast(e),
            ExprKind::CXXScalarValueInitExpr | ExprKind::ImplicitValueInitExpr => {
                self.to_type(ConstVal::Int(0), ty, n.loc())
            }
            ExprKind::DeclRefExpr => self.decl_ref(DeclId::new(n.a()), n.loc()),
            ExprKind::UnaryOperator => self.unary(e),
            ExprKind::BinaryOperator => self.binary(e),
            ExprKind::ConditionalOperator => {
                let [then, els] = self.ast.exprs.cond_arms(e);
                let c = self.eval(ExprId::new(n.a()))?;
                if c.truthy() { self.eval(then) } else { self.eval(els) }
            }
            ExprKind::UnaryExprOrTypeTraitExpr => {
                let kind = TraitKind::from_u8(n.aux()).ok_or_else(NotConstant::plain)?;
                let arg = if n.has(ExprFlags::ARG_IS_TYPE) {
                    QualType::from_raw(n.a()).ok_or_else(NotConstant::plain)?
                } else {
                    self.ast.exprs.get(ExprId::new(n.a())).ty()
                };
                let (s, a) = layout().size_align(self.ast, arg).ok_or_else(NotConstant::plain)?;
                let v = match kind {
                    TraitKind::SizeOf => s,
                    _ => a,
                };
                Ok(ConstVal::Int(i128::from(v)))
            }
            ExprKind::CallExpr => {
                // TODO(M3): constexpr function calls are evaluated by the Oven.
                let callee = self.callee_decl(ExprId::new(n.a()));
                Err(match callee {
                    Some(f) => {
                        let mut nc = NotConstant::note(
                            DiagId::note_constexpr_invalid_function,
                            n.loc(),
                            vec![self.name(f)],
                        );
                        nc.decl_note = Some((
                            DiagId::note_declared_at,
                            self.ast.decls.get(f).loc(),
                            vec![],
                        ));
                        nc
                    }
                    None => NotConstant::plain(),
                })
            }
            _ => Err(NotConstant::plain()),
        }
    }

    fn callee_decl(&self, mut e: ExprId) -> Option<DeclId> {
        loop {
            let n = self.ast.exprs.get(e);
            match n.kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ParenExpr => e = ExprId::new(n.a()),
                ExprKind::DeclRefExpr => return Some(DeclId::new(n.a())),
                _ => return None,
            }
        }
    }

    fn decl_ref(&mut self, d: DeclId, loc: Loc) -> Result<ConstVal, NotConstant> {
        let node = *self.ast.decls.get(d);
        match node.kind() {
            DeclKind::EnumConstant => {
                let data = self.ast.decls.enumerator(d).ok_or_else(NotConstant::plain)?;
                match (data.value, data.init) {
                    (Some(v), _) => self.scalar(v),
                    (None, Some(i)) => self.eval(i),
                    (None, None) => Err(NotConstant::plain()),
                }
            }
            DeclKind::Var => {
                let v = self.ast.decls.var(d).ok_or_else(NotConstant::plain)?;
                let ty = node.ty().unwrap_or(QualType::INT);
                let usable = v.spec.is_constexpr()
                    || (self.ast.types.effective_quals(ty).is_const()
                        && !self.ast.types.effective_quals(ty).is_volatile()
                        && int_info(self.ast, ty).is_some());
                match (usable, v.init) {
                    (true, Some(init)) => {
                        let r = self.eval(init)?;
                        self.to_type(r, ty, loc)
                    }
                    _ => {
                        let id = if usable {
                            DiagId::note_constexpr_var_init_unknown
                        } else if int_info(self.ast, ty).is_some()
                            && !self.ast.types.effective_quals(ty).is_const()
                        {
                            DiagId::note_constexpr_ltor_non_const_int
                        } else {
                            DiagId::note_constexpr_ltor_non_constexpr
                        };
                        let mut nc = NotConstant::note(id, loc, vec![self.name(d)]);
                        nc.decl_note = Some((DiagId::note_declared_at, node.loc(), vec![]));
                        Err(nc)
                    }
                }
            }
            DeclKind::ParmVar => Err(NotConstant::note(
                DiagId::note_constexpr_function_param_value_unknown,
                loc,
                vec![self.name(d)],
            )),
            _ => Err(NotConstant::plain()),
        }
    }

    fn cast(&mut self, e: ExprId) -> Result<ConstVal, NotConstant> {
        let n = *self.ast.exprs.get(e);
        let sub = ExprId::new(n.a());
        let ck = CastKind::from_u8(n.aux()).ok_or_else(NotConstant::plain)?;
        match ck {
            CastKind::LValueToRValue | CastKind::NoOp => self.eval(sub),
            CastKind::IntegralCast
            | CastKind::IntegralToBoolean
            | CastKind::IntegralToFloating
            | CastKind::FloatingToIntegral
            | CastKind::FloatingCast
            | CastKind::FloatingToBoolean
            | CastKind::BooleanToSignedIntegral => {
                let v = self.eval(sub)?;
                self.to_type(v, n.ty(), n.loc())
            }
            CastKind::NullToPointer => Ok(ConstVal::Null),
            CastKind::PointerToBoolean => match self.eval(sub)? {
                ConstVal::Null => Ok(ConstVal::Int(0)),
                _ => Err(NotConstant::plain()),
            },
            _ => Err(NotConstant::plain()),
        }
    }

    fn unary(&mut self, e: ExprId) -> Result<ConstVal, NotConstant> {
        let n = *self.ast.exprs.get(e);
        let op = UnaryOp::from_u8(n.aux()).ok_or_else(NotConstant::plain)?;
        let v = match op {
            UnaryOp::Plus | UnaryOp::Minus | UnaryOp::Not | UnaryOp::LNot => {
                self.eval(ExprId::new(n.a()))?
            }
            _ => return Err(NotConstant::plain()),
        };
        match (op, v) {
            (UnaryOp::Plus, v) => Ok(v),
            (UnaryOp::Minus, ConstVal::Float(f)) => Ok(ConstVal::Float(-f)),
            (UnaryOp::Minus, ConstVal::Int(i)) => self.checked(i.wrapping_neg(), n.ty(), n.loc()),
            (UnaryOp::Not, ConstVal::Int(i)) => self.to_type(ConstVal::Int(!i), n.ty(), n.loc()),
            (UnaryOp::LNot, v) => Ok(ConstVal::Int(i128::from(!v.truthy()))),
            _ => Err(NotConstant::plain()),
        }
    }

    /// `v` in type `ty`, diagnosing signed overflow.
    fn checked(&self, v: i128, ty: QualType, loc: Loc) -> Result<ConstVal, NotConstant> {
        let (w, s) = int_info(self.ast, ty).ok_or_else(NotConstant::plain)?;
        if s && !fits(v, w, s) {
            return Err(NotConstant::note(
                DiagId::note_constexpr_overflow,
                loc,
                vec![v.to_string(), self.ty_name(ty)],
            ));
        }
        Ok(ConstVal::Int(wrap(v, w, s)))
    }

    #[allow(clippy::too_many_lines, reason = "one arm per operator")]
    fn binary(&mut self, e: ExprId) -> Result<ConstVal, NotConstant> {
        let n = *self.ast.exprs.get(e);
        let op = BinaryOp::from_u8(n.aux()).ok_or_else(NotConstant::plain)?;
        let (l, r) = (ExprId::new(n.a()), ExprId::new(n.b()));
        match op {
            BinaryOp::LAnd => {
                if !self.eval(l)?.truthy() {
                    return Ok(ConstVal::Int(0));
                }
                return Ok(ConstVal::Int(i128::from(self.eval(r)?.truthy())));
            }
            BinaryOp::LOr => {
                if self.eval(l)?.truthy() {
                    return Ok(ConstVal::Int(1));
                }
                return Ok(ConstVal::Int(i128::from(self.eval(r)?.truthy())));
            }
            BinaryOp::Comma => {
                self.eval(l)?;
                return self.eval(r);
            }
            op if op.is_assignment() || op.is_ptr_mem() => return Err(NotConstant::plain()),
            _ => {}
        }
        let lv = self.eval(l)?;
        let rv = self.eval(r)?;
        let lty = self.ast.exprs.get(l).ty();
        let loc = n.loc();
        let cmp = |o: std::cmp::Ordering| -> bool {
            use std::cmp::Ordering::{Equal, Greater, Less};
            match op {
                BinaryOp::LT => o == Less,
                BinaryOp::GT => o == Greater,
                BinaryOp::LE => o != Greater,
                BinaryOp::GE => o != Less,
                BinaryOp::EQ => o == Equal,
                _ => o != Equal,
            }
        };
        match (lv, rv) {
            (ConstVal::Int(a), ConstVal::Int(b)) => {
                let unsigned = int_info(self.ast, lty).is_some_and(|(_, s)| !s);
                let ucmp = |a: i128, b: i128| -> std::cmp::Ordering {
                    if unsigned { (a as u128).cmp(&(b as u128)) } else { a.cmp(&b) }
                };
                let v = match op {
                    BinaryOp::Mul => a.wrapping_mul(b),
                    BinaryOp::Add => a.wrapping_add(b),
                    BinaryOp::Sub => a.wrapping_sub(b),
                    BinaryOp::Div | BinaryOp::Rem => {
                        if b == 0 {
                            return Err(NotConstant::note(
                                DiagId::note_expr_divide_by_zero,
                                loc,
                                vec![],
                            ));
                        }
                        let (q, rem) = if unsigned {
                            ((a as u128 / b as u128) as i128, (a as u128 % b as u128) as i128)
                        } else {
                            (a.wrapping_div(b), a.wrapping_rem(b))
                        };
                        if op == BinaryOp::Div { q } else { rem }
                    }
                    BinaryOp::Shl | BinaryOp::Shr => {
                        let (w, _) = int_info(self.ast, lty).ok_or_else(NotConstant::plain)?;
                        if b < 0 {
                            return Err(NotConstant::note(
                                DiagId::note_constexpr_negative_shift,
                                loc,
                                vec![b.to_string()],
                            ));
                        }
                        if b >= i128::from(w) {
                            return Err(NotConstant::note(
                                DiagId::note_constexpr_large_shift,
                                loc,
                                vec![
                                    b.to_string(),
                                    self.ty_name(lty),
                                    w.to_string(),
                                    if w == 1 { String::new() } else { "s".into() },
                                ],
                            ));
                        }
                        if op == BinaryOp::Shl {
                            // C++20: modular, but the result must be representable
                            // as a shifted value; wrapping is the defined result.
                            return self.to_type(ConstVal::Int(a << b), n.ty(), loc);
                        }
                        if unsigned { ((a as u128) >> b) as i128 } else { a >> b }
                    }
                    BinaryOp::And => a & b,
                    BinaryOp::Or => a | b,
                    BinaryOp::Xor => a ^ b,
                    BinaryOp::LT
                    | BinaryOp::GT
                    | BinaryOp::LE
                    | BinaryOp::GE
                    | BinaryOp::EQ
                    | BinaryOp::NE => return Ok(ConstVal::Int(i128::from(cmp(ucmp(a, b))))),
                    _ => return Err(NotConstant::plain()),
                };
                if matches!(op, BinaryOp::Mul | BinaryOp::Add | BinaryOp::Sub | BinaryOp::Div) {
                    // Exact results need more than i128 only for 128-bit operands.
                    let exact = match op {
                        BinaryOp::Mul => a.checked_mul(b),
                        BinaryOp::Add => a.checked_add(b),
                        BinaryOp::Sub => a.checked_sub(b),
                        _ => a.checked_div(b),
                    };
                    return match exact {
                        Some(x) => self.checked(x, n.ty(), loc),
                        None => self.to_type(ConstVal::Int(v), n.ty(), loc),
                    };
                }
                self.to_type(ConstVal::Int(v), n.ty(), loc)
            }
            (ConstVal::Float(a), ConstVal::Float(b)) => {
                let v = match op {
                    BinaryOp::Mul => a * b,
                    BinaryOp::Div => a / b,
                    BinaryOp::Add => a + b,
                    BinaryOp::Sub => a - b,
                    BinaryOp::LT
                    | BinaryOp::GT
                    | BinaryOp::LE
                    | BinaryOp::GE
                    | BinaryOp::EQ
                    | BinaryOp::NE => {
                        return Ok(ConstVal::Int(i128::from(match a.partial_cmp(&b) {
                            Some(o) => cmp(o),
                            None => op == BinaryOp::NE,
                        })));
                    }
                    _ => return Err(NotConstant::plain()),
                };
                self.to_type(ConstVal::Float(v), n.ty(), loc)
            }
            (ConstVal::Null, ConstVal::Null) => match op {
                BinaryOp::EQ => Ok(ConstVal::Int(1)),
                BinaryOp::NE => Ok(ConstVal::Int(0)),
                _ => Err(NotConstant::plain()),
            },
            _ => Err(NotConstant::plain()),
        }
    }
}

/// Formats `f` as clang's `APFloat::toString` does for notes (shortest
/// round-trip form).
pub fn format_float(f: f64) -> String {
    let s = format!("{f}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        s + ".0"
    }
}

