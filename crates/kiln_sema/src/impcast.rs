//! Implicit-conversion warnings over a completed full-expression
//! (clang's `CheckImplicitConversions` / `AnalyzeImplicitConversions`).
//!
//! Implemented: `-Wliteral-conversion`, a floating literal (possibly
//! negated or parenthesized) implicitly converted to an integer type
//! whose value changes, or is out of range (clang's
//! `DiagnoseFloatingImpCast` with `IsLiteral`); and `-Wconstant-conversion`,
//! an integral constant implicitly converted to a narrower integer type
//! whose value changes (clang's `warn_impcast_integer_precision_constant`);
//! and `-Wnull-conversion` for GNU `__null` (`gnu_null.rs`).

use kiln_ast::expr::{BinaryOp, ExprDeps, ExprKind, UnaryOp};
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::conv::narrow::float_kind;
use crate::conv::rank::{int_signed, int_width, is_integral_or_unscoped_enum};
use crate::conv::soft;

/// Bound on the nodes looked at in one full-expression.
const MAX_NODES: usize = 1_000_000;

impl Sema<'_, '_> {
    /// Walks full-expression `root`, whose conversion context is `cc`
    /// (the declared variable, `return`, ...; clang's `CC`), diagnosing
    /// its implicit conversions. Children of an expression take its own
    /// location as their context, as in clang.
    pub(crate) fn check_implicit_conversions(&mut self, root: ExprId, cc: Loc) {
        let mut work = vec![(root, cc)];
        let mut seen = 0usize;
        while let Some((e, cc)) = work.pop() {
            seen += 1;
            if seen > MAX_NODES {
                break;
            }
            let s = self.strip_impcast(e);
            if s != e {
                self.check_conversion(e, s, cc);
            }
            let n = *self.ast.exprs.get(s);
            match n.kind() {
                // Not evaluated, or analyzed as statements of their own.
                ExprKind::UnaryExprOrTypeTraitExpr | ExprKind::StmtExpr | ExprKind::LambdaExpr => {
                    continue;
                }
                // clang's `AnalyzeAssignment`: a constant truncated by the
                // bit-field assigned (`-Wbitfield-constant-conversion`)
                // skips the conversions of the right operand.
                ExprKind::BinaryOperator
                    if self.ast.exprs.binary_op(s) == BinaryOp::Assign && {
                        let [l, r] = self.ast.exprs.operands(s);
                        self.check_bit_field_assignment(l, r, n.loc())
                    } =>
                {
                    let [l, r] = self.ast.exprs.operands(s);
                    let at = n.loc();
                    work.push((self.strip_impcast(r), at));
                    work.push((l, at));
                    continue;
                }
                k if k.is_explicit_cast() => {
                    let sub = self.strip_impcast(self.ast.exprs.operand(s));
                    work.push((sub, cc));
                    continue;
                }
                _ => {}
            }
            let at = match n.kind() {
                ExprKind::CallExpr | ExprKind::CXXMemberCallExpr => self.begin_loc(s),
                _ => n.loc(),
            };
            let kids = self.ast.exprs.children(s, &self.ast.lists);
            // clang's work list pops the last child first.
            work.extend(kids.into_iter().map(|k| (k, at)));
        }
    }

    /// clang's `IgnoreParenImpCasts`.
    pub(crate) fn strip_impcast(&self, mut e: ExprId) -> ExprId {
        for _ in 0..256 {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ParenExpr
                | ExprKind::ImplicitCastExpr
                | ExprKind::ConstantExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::MaterializeTemporaryExpr => e = self.ast.exprs.operand(e),
                ExprKind::SubstNonTypeTemplateParmExpr => e = self.ast.exprs.subst_replacement(e),
                _ => break,
            }
        }
        e
    }

    /// The conversion of `src` (implicit casts and parentheses stripped)
    /// to the type of `e`.
    fn check_conversion(&mut self, e: ExprId, src: ExprId, cc: Loc) {
        let to = self.ast.exprs.get(e).ty();
        let from = self.ast.exprs.get(src).ty();
        if self.ast.exprs.get(src).kind() == ExprKind::GNUNullExpr {
            self.check_null_conversion(src, to, cc);
        }
        if self.ast.types.builtin(to) == Some(BuiltinType::Bool) {
            let from = self.ast.types.canonical(from);
            if self.ast.types.is_pointer(from)
                || self.ast.types.is_array(from)
                || self.ast.types.is_function(from)
            {
                self.always_non_null_pointer(src, cc);
                return;
            }
            self.diagnose_int_in_bool_context(src);
        }
        let bit_int = matches!(self.ast.types.canonical_view(to), TypeView::BitInt { .. });
        if !bit_int && self.ast.types.builtin(to).is_none_or(|b| !b.is_integral()) {
            return;
        }
        if self.ast.types.builtin(from).and_then(|b| float_kind(self, b)).is_some() {
            self.floating_imp_cast(src, to, cc);
            return;
        }
        self.integer_constant_imp_cast(src, to, cc);
    }

    /// clang's `DiagnoseAlwaysNonNullPointer` for the conversion of `e`
    /// (parentheses and implicit casts stripped) to `bool`: a function, an
    /// array, or the address of an object, named by a `DeclRefExpr` or
    /// `MemberExpr`, is never null (`-Wpointer-bool-conversion`; a
    /// function adds the `&` and, when it is callable with no arguments
    /// and returns `bool`, the `()` fix-it notes); `this` and the address
    /// of a reference are null only after undefined behaviour
    /// (`-Wundefined-bool-conversion`). Not in unevaluated operands, nor
    /// when `e` or the context `cc` comes from a macro body; a weak
    /// declaration may be null.
    fn always_non_null_pointer(&mut self, e: ExprId, cc: Loc) {
        if self.unevaluated > 0 {
            return;
        }
        let at = self.expr_loc(e);
        if at == self.cond_typo_at {
            return;
        }
        if at.is_macro() && (self.in_any_macro_body(at) || self.in_any_macro_body(cc)) {
            return;
        }
        let root = e;
        let mut e = e;
        let kind = self.ast.exprs.get(e).kind();
        if kind == ExprKind::CXXThisExpr {
            let msg = self.diag(DiagId::sema_this_bool_conversion, at);
            self.emit_after_typos(root, msg);
            return;
        }
        let addr_of = kind == ExprKind::UnaryOperator;
        if addr_of {
            if self.ast.exprs.unary_op(e) != UnaryOp::AddrOf {
                return;
            }
            e = self.ast.exprs.operand(e);
        }
        let named = |s: &Self, x: ExprId| {
            matches!(s.ast.exprs.get(x).kind(), ExprKind::DeclRefExpr | ExprKind::MemberExpr)
                .then(|| s.ast.exprs.decl(x))
        };
        if addr_of {
            // clang `CheckForReference` (a reference-returning call is
            // left alone: kiln does not model its note).
            let x = self.strip_impcast(e);
            if let Some(d) = named(self, x) {
                if self.ast.decls.get(d).ty().is_some_and(|t| self.ast.types.is_reference(t)) {
                    let at = self.expr_loc(x);
                    let msg = self.diag(DiagId::sema_address_of_reference_bool_conversion, at);
                    self.emit_after_typos(root, msg);
                    return;
                }
            } else if matches!(
                self.ast.exprs.get(x).kind(),
                ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr
            ) {
                return;
            }
        }
        let Some(d) = named(self, e) else {
            return;
        };
        if self.find_attr(d, kiln_ast::attr::AttrKind::WEAK).is_some() {
            return;
        }
        let Some(t) = self.ast.decls.get(d).ty() else {
            return;
        };
        let t = self.ast.types.canonical(t);
        let (array, function) = (self.ast.types.is_array(t), self.ast.types.is_function(t));
        if (addr_of && function) || (!addr_of && !function && !array) {
            return;
        }
        let what = if addr_of {
            0u32
        } else if function {
            1
        } else {
            2
        };
        let at = self.expr_loc(e);
        let text = self.expr_text_as_written(e);
        // clang streams the context `SourceRange(CC)`, not `e`'s range
        // (it decides the macro-expansion notes, spec/diag.md §3.3).
        let range = Span::new(cc, self.token_end(cc).unwrap_or(cc));
        let mut msg =
            self.diag(DiagId::sema_impcast_pointer_to_bool, at).arg(what).arg(text).range(range);
        if function {
            let begin = self.begin_loc(e);
            msg = msg.with_note(
                self.diag(DiagId::note_function_warning_silence, at)
                    .fixit(kiln_diag::FixIt::insert(begin, "&")),
            );
            if self.zero_arg_call_returns_bool(d, t) {
                let end = self.expr_end_loc(e);
                let after = self.token_end(end).unwrap_or(end);
                msg = msg.with_note(
                    self.diag(DiagId::note_function_to_function_call, at)
                        .fixit(kiln_diag::FixIt::insert(after, "()")),
                );
            }
        }
        self.emit_after_typos(root, msg);
    }

    /// clang `tryExprAsCall` for function `d` of type `t`, then whether
    /// the call is `bool`: every parameter has a default argument.
    fn zero_arg_call_returns_bool(&self, d: DeclId, t: QualType) -> bool {
        let kiln_ast::types::TypeView::Function(p) = self.ast.types.view(t.ty()) else {
            return false;
        };
        self.ast.types.builtin(self.ast.types.canonical(p.ret())) == Some(BuiltinType::Bool)
            && self.param_decls(d).iter().all(|&q| self.param_has_default(q))
    }

    /// clang's `IsInAnyMacroBody`: whether `loc`, walked outward through
    /// macro arguments to where they were written, is in a macro's
    /// replacement list.
    fn in_any_macro_body(&self, mut loc: Loc) -> bool {
        let sm = self.diags.sm();
        for _ in 0..256 {
            if !loc.is_macro() {
                return false;
            }
            match sm.expansion_info(loc) {
                None => return false,
                Some(x) if matches!(x.kind, kiln_source::ExpansionKind::Macro { .. }) => {
                    return true;
                }
                Some(_) => loc = sm.immediate_macro_caller_loc(loc),
            }
        }
        false
    }

    /// clang's `CheckImplicitConversion` for an integral constant `e`
    /// converted to integer type `to` (`-Wconstant-conversion`): the value
    /// needs more bits than `to` has, or a non-negative signed value of
    /// exactly `to`'s width turns negative (`isSameWidthConstantConversion`;
    /// not for a hexadecimal, octal or binary literal). The arms of a
    /// conditional are checked separately. A runtime-behavior diagnostic.
    fn integer_constant_imp_cast(&mut self, e: ExprId, to: QualType, cc: Loc) {
        let n = *self.ast.exprs.get(e);
        if n.kind() == ExprKind::ConditionalOperator {
            for arm in self.ast.exprs.cond_arms(e) {
                let a = self.strip_impcast(arm);
                if self.ast.exprs.get(a).ty() != to {
                    self.integer_constant_imp_cast(a, to, cc);
                }
            }
            return;
        }
        let from = n.ty();
        if self.ast.types.builtin(to) == Some(BuiltinType::Bool)
            || !is_integral_or_unscoped_enum(self, from)
            || self.ast.types.is_dependent(from)
            || self.ast.types.canonical(from).without_quals()
                == self.ast.types.canonical(to).without_quals()
            || n.deps().intersects(ExprDeps::VALUE.with(ExprDeps::TYPE).with(ExprDeps::ERROR))
        {
            return;
        }
        // clang's `DiagRuntimeBehavior`: not in an unevaluated or a
        // constant-evaluated context (a constant-expression).
        if self.unevaluated > 0
            || self.immediate > 0
            || self.constant_full_expr > 0
            || self.in_constant_evaluated()
        {
            return;
        }
        let (Some(tw), Some(fw)) = (int_width(self, to), int_width(self, from)) else { return };
        let t_signed = int_signed(self, to);
        // Only a conversion that can change a value of `from`.
        if fw < tw || (fw == tw && (t_signed || !int_signed(self, from))) {
            return;
        }
        let Some(v) = self.fold_int(e) else { return };
        // clang's `GetValueRange` (the approximate `LikelySourceRange`).
        let likely = if v < 0 { 129 - (!v).leading_zeros() } else { 128 - v.leading_zeros() };
        let same_width = likely == tw
            && t_signed
            && v >= 0
            && int_signed(self, from)
            && !self.non_decimal_literal(e);
        if likely <= tw && !same_width {
            return;
        }
        let converted = truncate_to(v, tw, t_signed);
        let range = self.expr_range(e);
        let ctx = Span::new(cc, self.token_end(cc).unwrap_or(cc));
        let d = self
            .diag(DiagId::sema_constant_conversion, n.loc())
            .arg(self.ty_arg(from))
            .arg(self.ty_arg(to))
            .arg(v.to_string())
            .arg(converted.to_string())
            .range(range)
            .range(ctx);
        self.diag_if_reachable(d, e);
    }

    /// Whether `e` is an integer literal spelled in hexadecimal, octal or
    /// binary.
    fn non_decimal_literal(&self, e: ExprId) -> bool {
        if self.ast.exprs.get(e).kind() != ExprKind::IntegerLiteral {
            return false;
        }
        let sm = self.diags.sm();
        let at = sm.spelling_loc(self.ast.exprs.get(e).loc());
        let Some((file, pos)) = sm.offset_of(at) else { return false };
        let text = sm.text(file).as_bytes();
        let pos = pos as usize;
        text.get(pos) == Some(&b'0') && text.get(pos + 1).is_some_and(u8::is_ascii_alphanumeric)
    }

    /// clang's `CheckConditionalOperand` / `DiagnoseFloatingImpCast` for
    /// operand `e` converted to integer type `to`.
    fn floating_imp_cast(&mut self, e: ExprId, to: QualType, cc: Loc) {
        let n = *self.ast.exprs.get(e);
        if n.kind() == ExprKind::ConditionalOperator {
            for arm in self.ast.exprs.cond_arms(e) {
                let a = self.strip_impcast(arm);
                if self
                    .ast
                    .types
                    .builtin(self.ast.exprs.get(a).ty())
                    .and_then(|b| float_kind(self, b))
                    .is_some()
                {
                    self.floating_imp_cast(a, to, cc);
                }
            }
            return;
        }
        let Some((kind, bits)) = self.float_literal_value(e) else { return };
        let is_bool = self.ast.types.builtin(to) == Some(BuiltinType::Bool);
        let (w, signed) = if is_bool {
            (1, false)
        } else {
            (int_width(self, to).unwrap_or(64), int_signed(self, to))
        };
        // A negative zero converts inexactly (`APFloat::convertToInteger`).
        let int = soft::to_int(kind, bits)
            .map(|(neg, mag, exact)| (neg, mag, exact && !(neg && mag == 0)));
        let fits = int.is_some_and(|(neg, mag, _)| representable(neg, mag, w, signed));
        if fits && int.is_some_and(|(_, _, exact)| exact) {
            return;
        }
        let from = self.ty_arg(n.ty());
        let target = self.ty_arg(to.without_quals());
        let at = n.loc();
        let range = Span::new(self.begin_loc(e), self.lit_expr_end(e).unwrap_or(at));
        let ctx = Span::new(cc, self.token_end(cc).unwrap_or(cc));
        let d = if !is_bool && !fits {
            self.diag(DiagId::sema_literal_conversion_out_of_range, at).arg(from).arg(target)
        } else {
            let value = soft::apfloat_diag_string(kind, bits);
            let result = if is_bool {
                if soft::is_zero_value(kind, bits) {
                    "false".to_string()
                } else {
                    "true".to_string()
                }
            } else {
                let (neg, mag, _) = int.unwrap_or((false, 0, false));
                if neg && mag != 0 { format!("-{mag}") } else { mag.to_string() }
            };
            self.diag(DiagId::sema_literal_conversion, at)
                .arg(from)
                .arg(target)
                .arg(value)
                .arg(result)
        };
        let d = d.range(range).range(ctx);
        self.emit(d);
    }

    /// The value of a floating literal, possibly under a unary `+`/`-`
    /// (clang's `IsLiteral` case).
    fn float_literal_value(&self, e: ExprId) -> Option<(kiln_ast::consts::FloatKind, u128)> {
        let n = self.ast.exprs.get(e);
        let (lit, negate) = match n.kind() {
            ExprKind::FloatingLiteral => (e, false),
            ExprKind::UnaryOperator => {
                let op = self.ast.exprs.unary_op(e);
                if !matches!(op, UnaryOp::Plus | UnaryOp::Minus) {
                    return None;
                }
                let inner = self.strip_impcast(self.ast.exprs.operand(e));
                if self.ast.exprs.get(inner).kind() != ExprKind::FloatingLiteral {
                    return None;
                }
                (inner, op == UnaryOp::Minus)
            }
            _ => return None,
        };
        let v = self.ast.exprs.literal_const(lit)?;
        let kiln_ast::consts::Scalar::Float { kind, bits, .. } = self.ast.consts.scalar(v)? else {
            return None;
        };
        // The literal's type is the operand's; a cast in between (none in
        // practice) would change the encoding.
        let tk = self.ast.types.builtin(n.ty()).and_then(|b| float_kind(self, b))?;
        if tk != kind {
            return None;
        }
        Some((kind, if negate { bits ^ soft::sign_bit(kind) } else { bits }))
    }

    /// The end of the last token of literal-shaped `e` (a literal under
    /// parentheses and unary operators).
    fn lit_expr_end(&self, e: ExprId) -> Option<Loc> {
        let n = self.ast.exprs.get(e);
        match n.kind() {
            ExprKind::ParenExpr => self.ast.exprs.end_loc(e).map(|r| r.with_offset(1)),
            ExprKind::UnaryOperator | ExprKind::ImplicitCastExpr => {
                self.lit_expr_end(self.ast.exprs.operand(e))
            }
            _ => self.token_end(n.loc()),
        }
    }

    /// The end of the identifier, keyword, number or punctuator token at
    /// `at`, from the source text. For a macro location the token is
    /// measured where it is spelled and the end is that far past `at` in
    /// its expansion entry (clang `Lexer::getLocForEndOfToken`); `None`
    /// if it would leave the entry.
    pub(crate) fn token_end(&self, at: Loc) -> Option<Loc> {
        let sm = self.diags.sm();
        if at.is_macro() {
            let sp = sm.spelling_loc(at);
            let len = sm.same_entry_distance(sp, self.token_end(sp)?)?;
            let end = at.with_offset(len);
            return sm.same_entry_distance(at, end).map(|_| end);
        }
        let (file, pos) = sm.offset_of(at)?;
        let text = sm.text(file).as_bytes();
        let start = pos as usize;
        let mut i = start;
        let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'\'';
        if text.get(i).is_some_and(|&c| word(c)) {
            while let Some(&c) = text.get(i) {
                // An exponent sign continues a number (`1.5e+3`).
                let exp_sign = (c == b'+' || c == b'-')
                    && i > start
                    && matches!(text[i - 1], b'e' | b'E' | b'p' | b'P')
                    && (text[start].is_ascii_digit() || text[start] == b'.');
                if !(word(c) || exp_sign) {
                    break;
                }
                i += 1;
            }
        } else {
            // A punctuator, longest match first ([lex.operators]). `>>`
            // and `>>=` are left out: a `>` closing a template argument
            // list may be the first half of one (clang measures the split
            // token).
            const PUNCT: [&[u8]; 22] = [
                b"<=>", b"<<=", b"->*", b"::", b"->", b"++", b"--", b"<<", b"<=", b">=", b"==",
                b"!=", b"&&", b"||", b"+=", b"-=", b"*=", b"/=", b"%=", b"&=", b"|=", b"^=",
            ];
            let rest = text.get(i..).unwrap_or_default();
            i += PUNCT.iter().find(|p| rest.starts_with(p)).map_or(1, |p| p.len());
        }
        u32::try_from(i).ok().map(|o| sm.loc(file, o))
    }
}

/// True if `neg`/`mag` is a value of a `w`-bit integer of signedness
/// `signed`.
fn representable(neg: bool, mag: u128, w: u32, signed: bool) -> bool {
    if w == 0 {
        return mag == 0;
    }
    if signed {
        let lim = 1u128 << (w.min(128) - 1);
        if neg { mag <= lim } else { mag < lim }
    } else {
        (!neg || mag == 0) && (w >= 128 || mag < (1u128 << w))
    }
}

/// `v` truncated to a `w`-bit integer of signedness `signed` (clang's
/// `PrettyPrintInRange`).
pub(crate) fn truncate_to(v: i128, w: u32, signed: bool) -> i128 {
    if w == 0 || w >= 128 {
        return if w == 0 { 0 } else { v };
    }
    let sh = 128 - w;
    if signed {
        (v << sh) >> sh
    } else {
        // The low `w` bits, as a non-negative value.
        v & ((1i128 << w) - 1)
    }
}
