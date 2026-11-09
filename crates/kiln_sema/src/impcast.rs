//! Implicit-conversion warnings over a completed full-expression
//! (clang's `CheckImplicitConversions` / `AnalyzeImplicitConversions`).
//!
//! Implemented: `-Wliteral-conversion`, a floating literal (possibly
//! negated or parenthesized) implicitly converted to an integer type
//! whose value changes, or is out of range (clang's
//! `DiagnoseFloatingImpCast` with `IsLiteral`); and `-Wconstant-conversion`,
//! an integral constant implicitly converted to a narrower integer type
//! whose value changes (clang's `warn_impcast_integer_precision_constant`);
//! `-Wimplicit-int-float-conversion` and
//! `-Wimplicit-const-int-float-conversion`, an integer whose likely range
//! (`int_range.rs`) does not fit a floating type's significand;
//! and `-Wnull-conversion` for GNU `__null` and `nullptr` (`gnu_null.rs`).

use kiln_ast::consts::FloatKind;
use kiln_ast::expr::{BinaryOp, ExprDeps, ExprKind, UnaryOp};
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::conv::narrow::float_kind;
use crate::conv::rank::{int_signed, int_width, is_integral_or_unscoped_enum};
use crate::conv::soft;
use crate::null_compare::NullUse;

/// Bound on the nodes looked at in one full-expression.
const MAX_NODES: usize = 1_000_000;

impl Sema<'_, '_> {
    /// Walks full-expression `root`, whose conversion context is `cc`
    /// (the declared variable, `return`, ...; clang's `CC`), diagnosing
    /// its implicit conversions. Children of an expression take its own
    /// location as their context, as in clang.
    pub(crate) fn check_implicit_conversions(&mut self, root: ExprId, cc: Loc) {
        // clang's `CheckImplicitConversions`: nothing for a value- or
        // type-dependent full-expression; each instantiation checks it.
        if self.expr_is_value_dependent(root) {
            return;
        }
        if !self.inst.stack.is_empty() {
            self.recheck_enum_constants_in_bool(root);
        }
        let mut work = std::mem::take(&mut self.conv_walk_stack);
        work.clear();
        work.push((root, cc, false));
        let mut seen = 0usize;
        while let Some((e, cc, list)) = work.pop() {
            seen += 1;
            if seen > MAX_NODES {
                break;
            }
            // clang's `IsListInit`: everything under a braced list.
            let list = list || self.ast.exprs.get(e).kind() == ExprKind::InitListExpr;
            let s = self.strip_impcast(e);
            if s != e {
                self.check_conversion(e, s, cc, list);
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
                    work.push((self.strip_impcast(r), at, list));
                    work.push((l, at, list));
                    continue;
                }
                k if k.is_explicit_cast() => {
                    let mut sub = self.strip_impcast(self.ast.exprs.operand(s));
                    // clang 22 also skips the conversion of the one element
                    // of a braced functional cast (`int{6.5}`, `T{4.5}` of
                    // an aggregate `T`): its narrowing is diagnosed instead
                    // (WP-465). clang 19 checks it.
                    if self.clang_compat >= 22
                        && self.ast.exprs.get(sub).kind() == ExprKind::InitListExpr
                    {
                        let mut it = self.ast.lists.iter(self.ast.exprs.init_list_inits(sub));
                        if let (Some(only), None) = (it.next(), it.next()) {
                            sub = self.strip_impcast(only);
                        }
                    }
                    work.push((sub, cc, list));
                    continue;
                }
                _ => {}
            }
            let at = match n.kind() {
                ExprKind::CallExpr | ExprKind::CXXMemberCallExpr => self.begin_loc(s),
                _ => n.loc(),
            };
            // clang's work list pops the last child first.
            let mut kids = self.walk_stack.take();
            kids.clear();
            self.ast.exprs.children_into(s, &self.ast.lists, &mut kids);
            work.extend(kids.iter().map(|&k| (k, at, list)));
            self.put_walk_stack(kids);
        }
        self.conv_walk_stack = work;
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
    fn check_conversion(&mut self, e: ExprId, src: ExprId, cc: Loc, list: bool) {
        let to = self.ast.exprs.get(e).ty();
        let from = self.ast.exprs.get(src).ty();
        self.null_conversion(src, to, cc, 0);
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
        // clang `CheckImplicitConversion`: a scalar converted to a vector
        // (an ext-vector splat) is checked against the element type, and
        // the diagnostics name the vector.
        let shown = to;
        let to = match self.vector_parts(to) {
            Some(_) if self.vector_parts(from).is_some() => return,
            Some(v) => v.elem,
            None => to,
        };
        if let Some(tk) = self.ast.types.builtin(to).and_then(|b| float_kind(self, b)) {
            if !list
                && let Some(fb) = self.ast.types.builtin(from)
                && fb.is_integral()
            {
                self.int_float_imp_cast(src, tk, shown, cc);
            }
            return;
        }
        let bit_int = matches!(self.ast.types.canonical_view(to), TypeView::BitInt { .. });
        if !bit_int && self.ast.types.builtin(to).is_none_or(|b| !b.is_integral()) {
            return;
        }
        if self.ast.types.builtin(from).and_then(|b| float_kind(self, b)).is_some() {
            self.floating_imp_cast(src, shown, cc);
            return;
        }
        self.integer_constant_imp_cast(src, to, shown, cc);
    }

    /// `-Wnull-conversion` for `src` (implicit casts stripped) converted
    /// to `to`: `__null` or a `std::nullptr_t` operand, or, as clang's
    /// `CheckConditionalOperator`, each such arm of a conditional.
    fn null_conversion(&mut self, src: ExprId, to: QualType, cc: Loc, depth: u32) {
        let n = *self.ast.exprs.get(src);
        if n.kind() == ExprKind::ConditionalOperator && depth < 64 {
            for arm in self.ast.exprs.cond_arms(src) {
                let a = self.strip_impcast(arm);
                self.null_conversion(a, to, cc, depth + 1);
            }
        } else if n.kind() == ExprKind::GNUNullExpr
            || self.is_nullptr_type(self.ast.types.canonical(n.ty()))
        {
            self.check_null_conversion(src, to, cc);
        }
    }

    /// clang's `DiagnoseAlwaysNonNullPointer` for the conversion `cast`
    /// of `e` (parentheses and implicit casts stripped) to `bool`
    /// (`-Wpointer-bool-conversion`, `-Wundefined-bool-conversion`;
    /// spec/sema.md §5.27). Not in unevaluated operands.
    fn always_non_null_pointer(&mut self, e: ExprId, cc: Loc) {
        if self.unevaluated > 0 {
            return;
        }
        if self.expr_loc(e) == self.cond_typo_at {
            return;
        }
        let Some((msg, _)) = self.always_non_null(e, NullUse::Bool { cc }) else {
            return;
        };
        // An instantiation checks its full-expressions again and warns
        // again.
        let msg = if self.inst.stack.is_empty() { msg } else { msg.allow_duplicate() };
        self.emit_after_typos(e, msg);
    }

    /// clang `tryExprAsCall` for function `d`, then whether the call is
    /// `bool`: every parameter has a default argument.
    pub(crate) fn zero_arg_call_returns_bool(&self, d: DeclId) -> bool {
        let Some(t) = self.ast.decls.get(d).ty() else { return false };
        let t = self.ast.types.canonical(t);
        let TypeView::Function(p) = self.ast.types.view(t.ty()) else {
            return false;
        };
        self.ast.types.builtin(self.ast.types.canonical(p.ret())) == Some(BuiltinType::Bool)
            && self.param_decls(d).iter().all(|&q| self.param_has_default(q))
    }

    /// clang's `IsInAnyMacroBody`: whether `loc`, walked outward through
    /// macro arguments to where they were written, is in a macro's
    /// replacement list.
    pub(crate) fn in_any_macro_body(&self, mut loc: Loc) -> bool {
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
    /// conditional are checked separately. The first is a runtime-behavior
    /// diagnostic, the second is not.
    fn integer_constant_imp_cast(&mut self, e: ExprId, to: QualType, shown: QualType, cc: Loc) {
        let n = *self.ast.exprs.get(e);
        if n.kind() == ExprKind::ConditionalOperator {
            for arm in self.ast.exprs.cond_arms(e) {
                let a = self.strip_impcast(arm);
                if self.ast.exprs.get(a).ty() != to {
                    self.integer_constant_imp_cast(a, to, shown, cc);
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
        // clang's `DiagRuntimeBehavior` for a value wider than `to`: not in
        // an unevaluated or a constant-evaluated context (a
        // constant-expression). The same-width conversion is a plain
        // diagnostic (`_BitInt(7)` case value `100`).
        let runtime = !same_width;
        if runtime
            && (self.unevaluated > 0
                || self.immediate > 0
                || self.constant_full_expr > 0
                || self.in_constant_evaluated())
        {
            return;
        }
        let converted = truncate_to(v, tw, t_signed);
        let range = self.expr_range(e);
        let ctx = Span::new(cc, self.token_end(cc).unwrap_or(cc));
        let d = self
            .diag(DiagId::sema_constant_conversion, n.loc())
            .arg(self.ty_arg(from))
            .arg(self.ty_arg(shown))
            .arg(v.to_string())
            .arg(converted.to_string())
            .range(range)
            .range(ctx);
        if runtime {
            self.diag_if_reachable(d, e);
        } else {
            self.emit(d);
        }
    }

    /// clang's `CheckImplicitConversion` for integer `e` converted to
    /// floating type `to` (`kind`) outside a braced list: when the likely
    /// range of `e` (`GetExprRange`) needs more bits than `to`'s
    /// significand, a constant whose value changes is diagnosed
    /// (`-Wimplicit-const-int-float-conversion`, a runtime-behavior
    /// diagnostic), and any other value may lose precision
    /// (`-Wimplicit-int-float-conversion`).
    fn int_float_imp_cast(&mut self, e: ExprId, kind: FloatKind, shown: QualType, cc: Loc) {
        let constant = self.diags.is_enabled(DiagId::sema_impcast_integer_float_precision_constant);
        let lossy = self.diags.is_enabled(DiagId::sema_impcast_integer_float_precision);
        if !constant && !lossy {
            return;
        }
        let n = *self.ast.exprs.get(e);
        // clang's `CheckConditionalOperand`: each integer arm separately.
        let arms = match n.kind() {
            ExprKind::ConditionalOperator => Some(self.ast.exprs.cond_arms(e)),
            ExprKind::BinaryConditionalOperator => {
                let [common, _, _, f] =
                    self.ast.exprs.binary_conditional_operands(e, &self.ast.lists);
                Some([common, f])
            }
            _ => None,
        };
        if let Some(arms) = arms {
            for arm in arms {
                let a = self.strip_impcast(arm);
                if self
                    .ast
                    .types
                    .builtin(self.ast.exprs.get(a).ty())
                    .is_some_and(BuiltinType::is_integral)
                {
                    self.int_float_imp_cast(a, kind, shown, cc);
                }
            }
            return;
        }
        let from = n.ty();
        if n.deps().intersects(ExprDeps::VALUE.with(ExprDeps::TYPE).with(ExprDeps::ERROR)) {
            return;
        }
        let prec = soft::params(kind).0;
        let width = self.int_range_of_type(from).width;
        // A range is at most one bit wider than its type.
        if width < prec {
            return;
        }
        if self.expr_int_range(e, width).width <= prec {
            return;
        }
        let range = self.expr_range(e);
        let ctx = Span::new(cc, self.token_end(cc).unwrap_or(cc));
        let Some(v) = self.fold_int(e) else {
            if lossy {
                let d = self
                    .diag(DiagId::sema_impcast_integer_float_precision, n.loc())
                    .arg(self.ty_arg(from))
                    .arg(self.ty_arg(shown))
                    .range(range)
                    .range(ctx);
                // An instantiation checks its full-expressions again and
                // warns again.
                let d = if self.inst.stack.is_empty() { d } else { d.allow_duplicate() };
                self.emit(d);
            }
            return;
        };
        if !constant || self.immediate > 0 || self.runtime_diag_quiet() {
            return;
        }
        let signed = int_signed(self, from);
        #[allow(clippy::cast_sign_loss, reason = "an unsigned value's bits")]
        let (neg, mag) = if signed { (v < 0, v.unsigned_abs()) } else { (false, v as u128) };
        let r = soft::from_int(kind, neg, mag);
        if !r.inexact && !r.overflow {
            return;
        }
        let shown_v = if neg { format!("-{mag}") } else { mag.to_string() };
        let d = self
            .diag(DiagId::sema_impcast_integer_float_precision_constant, n.loc())
            .arg(shown_v)
            .arg(soft::apfloat_string(kind, r.bits, prec))
            .arg(self.ty_arg(from))
            .arg(self.ty_arg(shown))
            .range(range)
            .range(ctx);
        let d = if self.inst.stack.is_empty() { d } else { d.allow_duplicate() };
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
        let (w, signed) = if let Some(v) = self.vector_parts(to) {
            // A vector target: clang's `getIntWidth` is the whole
            // vector's size, so only a fractional part is reported.
            let bits = crate::target::size_align(&self.ast, to, self.abi()).map_or(64, |s| s.0 * 8);
            (u32::try_from(bits).unwrap_or(u32::MAX), int_signed(self, v.elem))
        } else if is_bool {
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
    fn float_literal_value(&self, e: ExprId) -> Option<(FloatKind, u128)> {
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

    /// The parser split a one-character `>` token at `at` off a `>>` or
    /// `>>=` ([temp.names]/4): [`Self::token_end`] measures it as one
    /// character. A tentative parse that is rolled back may leave one
    /// recorded; only a diagnostic range ending at a `>>` operator at
    /// the same location would notice.
    pub fn note_split_greater(&mut self, at: Loc) {
        self.split_greaters.insert(at);
    }

    /// The end of the identifier, keyword, number or punctuator token at
    /// `at`, from the source text. For a macro location the token is
    /// measured where it is spelled and the end is that far past `at` in
    /// its expansion entry (clang `Lexer::getLocForEndOfToken`); `None`
    /// if it would leave the entry.
    pub(crate) fn token_end(&self, at: Loc) -> Option<Loc> {
        let sm = self.diags.sm();
        if self.split_greaters.contains(&at) {
            return Some(at.with_offset(1));
        }
        if at.is_macro() {
            let sp = sm.spelling_loc(at);
            let len = sm.same_entry_distance(sp, self.token_end(sp)?)?;
            let end = at.with_offset(len);
            return sm.same_entry_distance(at, end).map(|_| end);
        }
        let (file, pos) = sm.offset_of(at)?;
        let text = sm.text(file).as_bytes();
        let start = pos as usize;
        // The end-of-file location (a `)` missing at EOF, `alignof(int`)
        // starts no token (WP-972: measuring one stepped past the buffer).
        if start >= text.len() {
            return None;
        }
        let mut i = start;
        let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'\'';
        if text.get(i).is_some_and(|&c| word(c)) {
            while let Some(&c) = text.get(i) {
                // An exponent sign continues a number (`1.5e+3`).
                let exp_sign = (c == b'+' || c == b'-')
                    && i > start
                    && matches!(text[i - 1], b'e' | b'E' | b'p' | b'P')
                    && (text[start].is_ascii_digit() || text[start] == b'.');
                // `.` continues a number (`1.5`), not a name (`a.b`).
                let ident_dot = c == b'.' && !(text[start].is_ascii_digit() || text[start] == b'.');
                if !(word(c) || exp_sign) || ident_dot {
                    break;
                }
                i += 1;
            }
        } else {
            // A punctuator, longest match first ([lex.operators]). A `>`
            // the parser split off a `>>` or `>>=` closing a template
            // argument list was measured above (clang measures the split
            // token).
            const PUNCT: [&[u8]; 24] = [
                b"<=>", b"<<=", b">>=", b"->*", b"::", b"->", b"++", b"--", b"<<", b">>", b"<=",
                b">=", b"==", b"!=", b"&&", b"||", b"+=", b"-=", b"*=", b"/=", b"%=", b"&=", b"|=",
                b"^=",
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kiln_ast::Ast;
    use kiln_ast::types::TypeContext;
    use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
    use kiln_source::SourceManager;
    use kiln_source::symbol::Interner;

    use crate::Sema;
    use crate::target::{Arch, Target};

    /// [lex.operators]: `>>` and `>>=` are one token each unless the
    /// parser split a `>` off them closing a template argument list
    /// ([temp.names]/4).
    #[test]
    fn token_end_measures_shift_tokens_and_split_greaters() {
        let src = "a >> b >>= c >= d > e";
        let sm = SourceManager::new();
        let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
        let diags = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
        let target = Target::for_arch(Arch::X86_64);
        let ast = Ast::new(Arc::new(Interner::new()), TypeContext::new(target.types.clone()));
        let mut s = Sema::for_target(ast, &diags, target);
        let len = |s: &Sema<'_, '_>, at: u32| {
            let end = s.token_end(sm.loc(file, at)).expect("measurable");
            sm.offset_of(end).expect("file location").1 - at
        };
        assert_eq!(len(&s, 2), 2, ">>");
        assert_eq!(len(&s, 7), 3, ">>=");
        assert_eq!(len(&s, 13), 2, ">=");
        assert_eq!(len(&s, 18), 1, ">");
        s.note_split_greater(sm.loc(file, 2));
        s.note_split_greater(sm.loc(file, 3));
        assert_eq!(len(&s, 2), 1, "split first '>'");
        assert_eq!(len(&s, 3), 1, "split second '>'");
    }
}
