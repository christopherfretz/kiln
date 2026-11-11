//! Loop-hint pragmas on loop statements (`#pragma unroll`, `#pragma
//! clang loop`, ...; clang's `LoopHintAttr`, `handleLoopHintAttr`,
//! `CheckLoopHintExpr` and `CheckForIncompatibleAttributes`; spec/parse.md
//! §2.3).
//!
//! The parser maps each hint to its option and state
//! ([`kiln_ast::attr::LoopHint`]) and checks its value as clang's
//! `HandlePragmaLoopHint` does; [`Sema::handle_loop_hint`] checks it
//! against the statement it precedes and [`Sema::check_loop_hint_compat`]
//! checks the hints of one statement against each other.

use kiln_ast::attr::{
    AttrKind, AttrNode, AttrSyntax, LoopHint, LoopHintOption, LoopHintSpelling, LoopHintState,
};
use kiln_ast::stmt::StmtKind;
use kiln_ast::types::BuiltinType;
use kiln_ast::{AttrId, ExprId, RawId, StmtId};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;
use crate::conv;

impl Sema<'_, '_> {
    /// A loop-hint attribute `hint` at the pragma-name token `loc`, with
    /// the value `value` as written.
    pub fn loop_hint_attr(&mut self, hint: LoopHint, loc: Loc, value: Option<ExprId>) -> AttrId {
        self.ast.attrs.push(AttrNode {
            kind: AttrKind::LOOP_HINT,
            syntax: AttrSyntax::Pragma,
            flags: 0,
            loc,
            a: value.map_or(0, ExprId::raw),
            b: hint.pack(),
        })
    }

    /// Checks a loop hint's value (clang `CheckLoopHintExpr`): an integer
    /// constant expression of integer type (not `bool` or a narrow
    /// character type), positive (non-negative with `allow_zero`) and
    /// below 2^31. Value-dependent values pass. Returns the value (`None`
    /// after a diagnostic); `Some(None)` for a value-dependent one.
    pub fn check_loop_hint_expr(&mut self, e: ExprId, allow_zero: bool) -> Option<Option<i128>> {
        if self.expr_is_value_dependent(e) {
            return Some(None);
        }
        let loc = self.ast.exprs.get(e).loc();
        let ty = self.ast.exprs.get(e).ty();
        let narrow_char = matches!(
            self.ast.types.builtin(self.ast.types.canonical(ty).without_quals()),
            Some(BuiltinType::Bool | BuiltinType::Char | BuiltinType::SChar | BuiltinType::UChar)
        );
        if !conv::rank::is_integral_or_unscoped_enum(self, ty) || narrow_char {
            let msg =
                self.diag(DiagId::sema_pragma_loop_invalid_argument_type, loc).arg(self.ty_arg(ty));
            self.emit(msg);
            return None;
        }
        let c = conv::lvalue_to_rvalue(self, e);
        let c = self.act_on_constant_full_expr(c);
        match self.eval_int(c, EvalContext::new(EvalKind::Other, self.begin_loc(e))) {
            Ok(v) => {
                let positive = if allow_zero { v >= 0 } else { v > 0 };
                if !positive || v >= 1 << 31 {
                    let msg = self
                        .diag(DiagId::sema_requires_positive_value, loc)
                        .arg(v)
                        .arg(if positive { "1" } else { "0" });
                    self.emit(msg);
                    return None;
                }
                Some(Some(v))
            }
            Err(mut f) => {
                let at = Self::fold_lone_invalid(&mut f, loc);
                let d = self.not_ice_diag(DiagId::sema_expr_not_ice, at);
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                None
            }
        }
    }

    /// Applies loop hint `a` to statement `st` (clang `handleLoopHintAttr`):
    /// `st` must be a loop; a clang-loop count must be positive; `#pragma
    /// unroll 0` / `1` disables unrolling. Returns the attribute to keep.
    pub(crate) fn handle_loop_hint(&mut self, a: AttrId, st: StmtId) -> Option<AttrId> {
        let node = *self.ast.attrs.get(a);
        let mut hint = LoopHint::unpack(node.b);
        if !matches!(
            self.ast.stmts.get(st).kind(),
            StmtKind::DoStmt | StmtKind::ForStmt | StmtKind::CXXForRangeStmt | StmtKind::WhileStmt
        ) {
            let at = self.stmt_begin_loc(st);
            let msg = self
                .diag(DiagId::sema_pragma_loop_precedes_nonloop, at)
                .arg(format!("#pragma {}", hint.spelling.pragma()));
            self.emit(msg);
            return None;
        }
        let value = ExprId::from_raw(node.a);
        match (hint.spelling, hint.option, value) {
            (
                LoopHintSpelling::ClangLoop,
                LoopHintOption::VectorizeWidth
                | LoopHintOption::InterleaveCount
                | LoopHintOption::UnrollCount
                | LoopHintOption::PipelineInitiationInterval,
                Some(e),
            ) => {
                self.check_loop_hint_expr(e, false)?;
            }
            (LoopHintSpelling::Unroll, _, Some(e)) => {
                if let Some(v) = self.loop_hint_value(e)
                    && (v == 0 || v == 1)
                {
                    hint.option = LoopHintOption::Unroll;
                    hint.state = LoopHintState::Disable;
                    return Some(self.loop_hint_attr(hint, node.loc, value));
                }
            }
            _ => {}
        }
        Some(a)
    }

    /// Loop hint `a` with its dependent value substituted to `e` (clang
    /// `TransformLoopHintAttr`): `a` itself when `e` is invalid; a value
    /// of 0 or 1 makes it a hint that disables unrolling, whatever its
    /// option.
    pub(crate) fn instantiated_loop_hint(&mut self, a: AttrId, e: ExprId) -> AttrId {
        let node = *self.ast.attrs.get(a);
        let mut hint = LoopHint::unpack(node.b);
        let Some(v) = self.check_loop_hint_expr(e, hint.spelling == LoopHintSpelling::Unroll)
        else {
            return a;
        };
        if matches!(v, Some(0 | 1)) {
            hint.option = LoopHintOption::Unroll;
            hint.state = LoopHintState::Disable;
        }
        self.loop_hint_attr(hint, node.loc, Some(e))
    }

    /// The value of a checked loop hint value (`None` if it is
    /// value-dependent or not a constant).
    fn loop_hint_value(&mut self, e: ExprId) -> Option<i128> {
        if self.expr_is_value_dependent(e) {
            return None;
        }
        let c = conv::lvalue_to_rvalue(self, e);
        let c = self.act_on_constant_full_expr(c);
        self.eval_int(c, EvalContext::new(EvalKind::Other, self.begin_loc(e))).ok()
    }

    /// Checks the loop hints among `attrs` against each other (clang
    /// `CheckForIncompatibleAttributes`): one state and one numeric hint
    /// per category, and no numeric hint beside a disabling one (or, for
    /// unrolling, beside any state hint). Reported at the later hint;
    /// the hints are kept.
    pub(crate) fn check_loop_hint_compat(&mut self, attrs: &[AttrId]) {
        if attrs.len() < 2 {
            return;
        }
        // (state, numeric) per category.
        let mut slots: [(Option<AttrId>, Option<AttrId>); 7] = [(None, None); 7];
        for &a in attrs {
            let node = *self.ast.attrs.get(a);
            if node.kind != AttrKind::LOOP_HINT {
                continue;
            }
            let hint = LoopHint::unpack(node.b);
            let category = match hint.option {
                LoopHintOption::Vectorize | LoopHintOption::VectorizeWidth => 0,
                LoopHintOption::Interleave | LoopHintOption::InterleaveCount => 1,
                LoopHintOption::UnrollAndJam | LoopHintOption::UnrollAndJamCount => 2,
                LoopHintOption::PipelineDisabled | LoopHintOption::PipelineInitiationInterval => 3,
                LoopHintOption::Unroll | LoopHintOption::UnrollCount => 4,
                LoopHintOption::Distribute => 5,
                LoopHintOption::VectorizePredicate => 6,
            };
            let slot = &mut slots[category];
            let prev = if hint.option.is_state() { slot.0.replace(a) } else { slot.1.replace(a) };
            let (state, numeric) = *slot;
            if let Some(prev) = prev {
                let msg = self
                    .diag(DiagId::sema_pragma_loop_compatibility, node.loc)
                    .arg("1")
                    .arg(self.loop_hint_name(prev))
                    .arg(self.loop_hint_name(a));
                self.emit(msg);
            }
            if let (Some(state), Some(numeric)) = (state, numeric) {
                let disabled =
                    LoopHint::unpack(self.ast.attrs.get(state).b).state == LoopHintState::Disable;
                if category == 4 || category == 2 || disabled {
                    let msg = self
                        .diag(DiagId::sema_pragma_loop_compatibility, node.loc)
                        .arg("0")
                        .arg(self.loop_hint_name(state))
                        .arg(self.loop_hint_name(numeric));
                    self.emit(msg);
                }
            }
        }
    }

    /// A loop hint as diagnostics name it (clang
    /// `LoopHintAttr::getDiagnosticName`).
    fn loop_hint_name(&self, a: AttrId) -> String {
        let node = self.ast.attrs.get(a);
        let hint = LoopHint::unpack(node.b);
        let value = || self.loop_hint_value_string(node.a, hint.state);
        match hint.spelling {
            LoopHintSpelling::Nounroll => "#pragma nounroll".to_owned(),
            LoopHintSpelling::NounrollAndJam => "#pragma nounroll_and_jam".to_owned(),
            LoopHintSpelling::Unroll if hint.option == LoopHintOption::UnrollCount => {
                format!("#pragma unroll{}", value())
            }
            LoopHintSpelling::Unroll => "#pragma unroll".to_owned(),
            LoopHintSpelling::UnrollAndJam if hint.option == LoopHintOption::UnrollAndJamCount => {
                format!("#pragma unroll_and_jam{}", value())
            }
            LoopHintSpelling::UnrollAndJam => "#pragma unroll_and_jam".to_owned(),
            LoopHintSpelling::ClangLoop => format!("{}{}", hint.option.name(), value()),
        }
    }

    /// clang `LoopHintAttr::getValueString`: the argument in parentheses.
    fn loop_hint_value_string(&self, value: u32, state: LoopHintState) -> String {
        let value = ExprId::from_raw(value).map(|e| self.expr_text_as_written(e));
        let inner = match (state, value) {
            (LoopHintState::Numeric, v) => v.unwrap_or_default(),
            (LoopHintState::ScalableWidth, Some(v)) => format!("{v}, scalable"),
            (LoopHintState::FixedWidth, Some(v)) => v,
            (LoopHintState::ScalableWidth, None) => "scalable".to_owned(),
            (LoopHintState::FixedWidth, None) => "fixed".to_owned(),
            (LoopHintState::Enable, _) => "enable".to_owned(),
            (LoopHintState::Full, _) => "full".to_owned(),
            (LoopHintState::AssumeSafety, _) => "assume_safety".to_owned(),
            (LoopHintState::Disable, _) => "disable".to_owned(),
        };
        format!("({inner})")
    }
}
