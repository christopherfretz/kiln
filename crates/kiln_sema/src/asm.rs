//! GNU asm statements and file-scope asm declarations ([dcl.asm]; GNU
//! extended asm; spec/sema.md §asm, WP-140): constraint validation,
//! operand checks and the template analysis (clang's `ActOnGCCAsmStmt`).
//! The target rules live in `kiln_abi::asm`.

use kiln_abi::asm::{self as tasm, AsmOperandCounts, AsmPiece, AsmStringError, ConstraintInfo};
use kiln_ast::expr::{ExprDeps, ValueCategory};
use kiln_ast::stmt::{AsmData, AsmOperand, asm_bits};
use kiln_ast::types::TypeKind;
use kiln_ast::{DeclId, ExprId, ListId, QualType, StmtId, StringLitId};
use kiln_diag::{DiagId, FixIt};
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::conv;

/// A string literal and its first token's location.
pub type AsmStr = (StringLitId, Loc);

impl Sema<'_, '_> {
    /// `asm qualifiers ( template : outputs : inputs : clobbers )`
    /// (clang's `ActOnGCCAsmStmt`). Errors in constraints, clobbers or
    /// the template are reported and still build the statement, as
    /// clang does; invalid operands yield `None`.
    #[allow(clippy::too_many_arguments)]
    pub fn act_on_gcc_asm_stmt(
        &mut self,
        loc: Loc,
        bits: u16,
        string: AsmStr,
        outputs: &[AsmOperand],
        inputs: &[AsmOperand],
        clobbers: &[AsmStr],
        rparen: Loc,
    ) -> Option<StmtId> {
        self.act_on_gcc_asm_goto_stmt(loc, bits, string, outputs, inputs, clobbers, &[], rparen)
    }

    /// [`Self::act_on_gcc_asm_stmt`] with the `asm goto` labels (clang's
    /// `ActOnGCCAsmStmt`; `labels` are `Label` declarations).
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn act_on_gcc_asm_goto_stmt(
        &mut self,
        loc: Loc,
        mut bits: u16,
        string: AsmStr,
        outputs: &[AsmOperand],
        inputs: &[AsmOperand],
        clobbers: &[AsmStr],
        labels: &[(DeclId, Loc)],
        rparen: Loc,
    ) -> Option<StmtId> {
        if bits & asm_bits::BASIC != 0 {
            // A basic asm is implicitly volatile (clang's parser).
            bits |= asm_bits::VOLATILE;
        }
        let arch = self.abi.triple.arch;
        let mut ops: Vec<AsmOperand> = outputs.iter().chain(inputs).copied().collect();
        let nout = outputs.len();
        let build = |s: &mut Self, ops: &[AsmOperand]| -> Option<StmtId> {
            Some(s.build_asm(loc, bits, string, ops, nout, clobbers, labels, rparen))
        };
        let mut out_infos = Vec::with_capacity(nout);
        for op in &ops[..nout] {
            let c = self.asm_str(op.constraint);
            let name = self.asm_name(op);
            let mut info = ConstraintInfo::new(&c, &name);
            if !tasm::validate_output_constraint(arch, &mut info) {
                let d =
                    self.diag(DiagId::sema_asm_invalid_output_constraint, op.constraint_loc).arg(c);
                self.emit(d);
                return build(self, &ops);
            }
            out_infos.push(info);
            if self.check_naked_parm_ref(op.expr) {
                return None;
            }
            let e = op.expr;
            let t = self.ast.exprs.get(e).ty();
            if self.type_dependent(e) || self.is_error_type(t) {
                continue;
            }
            if !self.asm_output_lvalue_ok(e) {
                let b = self.begin_loc(e);
                if !self.is_complete(t)
                    && !self.ast.types.is_array(t)
                    && !self.ast.types.is_void(t)
                    && !self.require_complete(t, b)
                {
                    self.report_incomplete_operand(t, b);
                    return None;
                }
                self.error(DiagId::sema_asm_invalid_lvalue_in_output, b);
                return None;
            }
        }
        let mut in_infos = Vec::with_capacity(inputs.len());
        for i in nout..ops.len() {
            let op = ops[i];
            let c = self.asm_str(op.constraint);
            let name = self.asm_name(&op);
            let mut info = ConstraintInfo::new(&c, &name);
            if !tasm::validate_input_constraint(arch, &mut out_infos, &mut info) {
                let d =
                    self.diag(DiagId::sema_asm_invalid_input_constraint, op.constraint_loc).arg(c);
                self.emit(d);
                return build(self, &ops);
            }
            let mut e = op.expr;
            let t = self.ast.exprs.get(e).ty();
            let b = self.begin_loc(e);
            if self.is_error_type(t) {
                in_infos.push(info);
                continue;
            }
            if self.ast.types.canonical_kind(t) == TypeKind::MemberPointer {
                self.error(DiagId::sema_asm_pmf_through_constraint, b);
                return None;
            }
            if self.check_naked_parm_ref(e) {
                return None;
            }
            if info.allows_memory() && !info.allows_register() {
                if !self.type_dependent(e)
                    && self.ast.exprs.get(e).value_category() != ValueCategory::LValue
                {
                    let d = self.diag(DiagId::sema_asm_invalid_lvalue_in_input, b).arg(c);
                    self.emit(d);
                    return None;
                }
            } else {
                e = conv::decay_dependent(self, e);
                ops[i].expr = e;
            }
            let t = self.ast.exprs.get(e).ty();
            if info.allows_register() && self.ast.types.is_void(t) {
                let d =
                    self.diag(DiagId::sema_asm_invalid_type_in_input, b).arg(self.ty_arg(t)).arg(c);
                self.emit(d);
                return None;
            }
            in_infos.push(info);
            if self.type_dependent(e) {
                continue;
            }
            if (!self.ast.types.is_void(t)
                || !in_infos.last().is_some_and(ConstraintInfo::allows_memory))
                && !self.require_complete(t, b)
            {
                self.report_incomplete_operand(t, b);
                return None;
            }
        }
        for &(c, cloc) in clobbers {
            let name = self.asm_str(c);
            if !tasm::is_valid_clobber(arch, &name) {
                let d = self.diag(DiagId::sema_asm_unknown_register_name, cloc).arg(name);
                self.emit(d);
                return build(self, &ops);
            }
        }
        // The template ([dcl.asm]/1; GNU operand references).
        let mut names: Vec<String> = ops.iter().map(|o| self.asm_name(o)).collect();
        for &(l, _) in labels {
            let n = self.ast.decls.get(l).name();
            names.push(
                n.as_ident().map_or_else(String::new, |s| self.ast.symbols().as_str(s).to_owned()),
            );
        }
        let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let plus = out_infos.iter().filter(|i| i.is_read_write()).count() as u32;
        let counts = AsmOperandCounts {
            outputs: nout as u32,
            plus,
            inputs: inputs.len() as u32,
            labels: labels.len() as u32,
            names: &name_refs,
        };
        let text = self.ast.strings.bytes(string.0).to_vec();
        let simple = bits & asm_bits::BASIC != 0;
        let pieces = match tasm::analyze_asm_string(arch, &text, simple, &counts) {
            Ok(p) => p,
            Err((err, off)) => {
                let id = match err {
                    AsmStringError::InvalidEscape => DiagId::sema_asm_invalid_escape,
                    AsmStringError::InvalidOperandNumber => DiagId::sema_asm_invalid_operand_number,
                    AsmStringError::UnterminatedName => {
                        DiagId::sema_asm_unterminated_symbolic_operand_name
                    }
                    AsmStringError::EmptyName => DiagId::sema_asm_empty_symbolic_operand_name,
                    AsmStringError::UnknownName => DiagId::sema_asm_unknown_symbolic_operand_name,
                };
                let at = self.string_byte_loc(string.1, off);
                self.error(id, at);
                return build(self, &ops);
            }
        };
        let total = ops.len() as u32;
        // Operand widths against the printed register (clang's
        // `validateConstraintModifier`).
        for p in &pieces {
            let AsmPiece::Operand { no, modifier, percent, end } = *p else { continue };
            let mut idx = no;
            // Labels are last; they have no constraint to check.
            if bits & asm_bits::GOTO != 0 && idx >= total {
                continue;
            }
            if idx >= total {
                let mut cnt = idx - total;
                match out_infos.iter().position(|i| {
                    i.is_read_write() && {
                        let hit = cnt == 0;
                        cnt = cnt.wrapping_sub(1);
                        hit
                    }
                }) {
                    Some(o) => idx = o as u32,
                    None => continue,
                }
            }
            let e = ops[idx as usize].expr;
            let t = self.ast.exprs.get(e).ty();
            if self.type_dependent(e) {
                continue;
            }
            let Some((size, _)) = crate::target::size_align(&self.ast, t, &self.abi) else {
                continue;
            };
            let c = self.asm_str(ops[idx as usize].constraint);
            if let Err(suggest) = tasm::validate_constraint_modifier(arch, &c, modifier, size * 8) {
                let b = self.begin_loc(e);
                let mut d =
                    self.diag(DiagId::sema_asm_mismatched_size_modifier, b).allow_duplicate();
                if let Some(m) = suggest {
                    let at = self.string_byte_loc(string.1, percent);
                    let stop = self.string_byte_loc(string.1, end);
                    let range = Span::new(at, stop);
                    let skip = percent as usize + 1 + usize::from(modifier != 0);
                    let operand = String::from_utf8_lossy(&text[skip..end as usize]);
                    let n = self
                        .diag(DiagId::note_asm_missing_constraint_modifier, at)
                        .arg(m)
                        .range(range)
                        .fixit(FixIt::replace(range, format!("%{m}{operand}")));
                    d = d.with_note(n);
                }
                self.emit(d);
            }
        }
        // Every operand has the same number of alternatives.
        let mut alts: Option<usize> = None;
        for (k, info) in out_infos.iter().chain(&in_infos).enumerate() {
            let n = info.constraint.matches(',').count() + 1;
            match alts {
                None => alts = Some(n),
                Some(a) if a != n => {
                    let b = self.begin_loc(ops[k].expr);
                    let d = self
                        .diag(DiagId::sema_asm_unexpected_constraint_alternatives, b)
                        .arg(a.to_string())
                        .arg(n.to_string());
                    self.emit(d);
                    return build(self, &ops);
                }
                Some(_) => {}
            }
        }
        // Tied operands: one input per output, compatible types.
        let mut matched: Vec<Option<usize>> = vec![None; nout];
        for (i, info) in in_infos.iter().enumerate() {
            let Some(tied) = info.tied_operand() else { continue };
            let tied = tied as usize;
            let in_e = ops[nout + i].expr;
            if let Some(first) = matched[tied] {
                let b = self.begin_loc(in_e);
                let fb = self.begin_loc(ops[nout + first].expr);
                let n = self.diag(DiagId::note_asm_input_duplicate_first, fb).arg(tied.to_string());
                let d = self
                    .diag(DiagId::sema_asm_input_duplicate_match, b)
                    .arg(tied.to_string())
                    .with_note(n);
                self.emit(d);
                return build(self, &ops);
            }
            matched[tied] = Some(i);
            let out_e = ops[tied].expr;
            if self.type_dependent(in_e) || self.type_dependent(out_e) {
                continue;
            }
            let in_t = self.ast.exprs.get(in_e).ty();
            let out_t = self.ast.exprs.get(out_e).ty();
            if self.ast.types.same_type(in_t, out_t)
                || self.is_error_type(in_t)
                || self.is_error_type(out_t)
            {
                continue;
            }
            let in_dom = self.asm_domain(in_t);
            let out_dom = self.asm_domain(out_t);
            let size = |s: &Self, t| {
                crate::target::size_align(&s.ast, t, &s.abi).map_or(0, |(b, _)| b * 8)
            };
            let (in_size, out_size) = (size(self, in_t), size(self, out_t));
            if in_size == out_size && in_dom == out_dom && in_dom != Domain::Other {
                continue;
            }
            let mentioned = |n: usize| {
                pieces
                    .iter()
                    .any(|p| matches!(p, AsmPiece::Operand { no, .. } if *no as usize == n))
            };
            let smaller_mentioned = (mentioned(nout + i) && in_size < out_size)
                || (mentioned(tied) && out_size < in_size);
            if !smaller_mentioned
                && in_dom != Domain::Other
                && out_infos[tied].allows_register()
                && out_size <= 64
            {
                continue;
            }
            if in_dom == Domain::Int
                && out_dom == Domain::Int
                && !mentioned(nout + i)
                && !self.ast.exprs.get(in_e).deps().intersects(ExprDeps::VALUE)
                && self.is_constant_operand(in_e)
            {
                let out_u = conv::strip_cv(self, out_t);
                if let Ok(c) =
                    conv::implicit_convert(self, in_e, out_u, conv::ConvCtx::Initialization)
                {
                    ops[nout + i].expr = c;
                }
                continue;
            }
            let b = self.begin_loc(in_e);
            let d = self
                .diag(DiagId::sema_asm_tying_incompatible_types, b)
                .arg(self.ty_arg(in_t))
                .arg(self.ty_arg(out_t));
            self.emit(d);
            return build(self, &ops);
        }
        // Operand names are unique (clang sorts them and reports the
        // first adjacent pair).
        let mut by_name: Vec<(&str, usize)> = names
            .iter()
            .enumerate()
            .filter(|(_, n)| !n.is_empty())
            .map(|(k, n)| (n.as_str(), k))
            .collect();
        by_name.sort_by(|a, b| a.0.cmp(b.0));
        if let Some(w) = by_name.windows(2).find(|w| w[0].0 == w[1].0) {
            let (name, first, second) = (w[0].0.to_owned(), w[0].1, w[1].1);
            // A label's location is its name in the label list.
            let at = |s: &Self, k: usize| {
                if k < ops.len() { s.begin_loc(ops[k].expr) } else { labels[k - ops.len()].1 }
            };
            let (b, fb) = (at(self, second), at(self, first));
            let n = self.diag(DiagId::note_duplicate_asm_operand_name, fb).arg(name.clone());
            let d = self.diag(DiagId::sema_duplicate_asm_operand_name, b).arg(name).with_note(n);
            self.emit(d);
            return None;
        }
        build(self, &ops)
    }

    /// A namespace-scope `asm ( string-literal ) ;` ([dcl.asm]; clang's
    /// `FileScopeAsmDecl`).
    pub fn act_on_file_scope_asm(&mut self, string: ExprId, loc: Loc) -> DeclId {
        let ctx = self.ctx;
        self.builder().file_scope_asm(ctx, loc, string)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_asm(
        &mut self,
        loc: Loc,
        bits: u16,
        string: AsmStr,
        ops: &[AsmOperand],
        nout: usize,
        clobbers: &[AsmStr],
        labels: &[(DeclId, Loc)],
        rparen: Loc,
    ) -> StmtId {
        let cl: Vec<StringLitId> = clobbers.iter().map(|c| c.0).collect();
        let clobbers = self.ast.lists.push(&cl);
        let labels: Vec<DeclId> = labels.iter().map(|l| l.0).collect();
        let labels = self.ast.lists.push(&labels);
        let srclocs = self.asm_srclocs(string);
        // Each operand ends a full-expression ([intro.execution]/5): its
        // potential odr-uses are resolved here (clang marks them `used`).
        let ops: Vec<AsmOperand> =
            ops.iter().map(|o| AsmOperand { expr: self.act_on_full_expr(o.expr), ..*o }).collect();
        let ops = &ops[..];
        let data = AsmData {
            string: string.0,
            string_loc: string.1,
            operands: 0,
            num_outputs: nout as u32,
            num_inputs: (ops.len() - nout) as u32,
            clobbers,
            labels,
            rparen,
            srclocs,
        };
        self.builder().gcc_asm_stmt(data, ops, bits, loc)
    }

    /// clang's `getAsmSrcLocInfo`: the location of each line of a
    /// multi-line template (a `\n` before the last byte starts a line).
    /// clang's `!srcloc` values for the template: the literal's begin
    /// location, then the first byte of every later line. A value is
    /// clang's raw encoding of a main-file location (byte offset + 2);
    /// kiln uses the same formula for every file, so locations in headers
    /// or macro expansions do not match clang's numbering. Macro parity
    /// needs clang's unified offset space (spec/sema.md §7.7, "Deferred
    /// (WP-160 item 4)").
    fn asm_srclocs(&mut self, string: AsmStr) -> ListId {
        let enc = |s: &Self, l: Loc| {
            let sm = s.diags.sm();
            sm.offset_of(sm.expansion_loc(l)).map_or(0, |(_, o)| o.saturating_add(2))
        };
        let mut vals = vec![enc(self, string.1)];
        let b = self.ast.strings.bytes(string.0).to_vec();
        if b.len() >= 2 {
            for (i, &c) in b[..b.len() - 1].iter().enumerate() {
                if c == b'\n' {
                    let l = self.string_byte_loc(string.1, i as u32 + 1);
                    vals.push(enc(self, l));
                }
            }
        }
        self.ast.lists.push_raw(&vals)
    }

    fn asm_str(&self, s: StringLitId) -> String {
        String::from_utf8_lossy(self.ast.strings.bytes(s)).into_owned()
    }

    fn asm_name(&self, op: &AsmOperand) -> String {
        op.name.map_or_else(String::new, |n| self.ast.symbols().as_str(n).to_owned())
    }

    fn type_dependent(&self, e: ExprId) -> bool {
        self.ast.exprs.get(e).deps().contains(ExprDeps::TYPE)
    }

    /// clang's `isModifiableLvalue` result is `MLV_Valid` or
    /// `MLV_ArrayType`.
    fn asm_output_lvalue_ok(&self, e: ExprId) -> bool {
        let n = self.ast.exprs.get(e);
        let t = n.ty();
        if n.value_category() != ValueCategory::LValue || self.ast.types.is_function(t) {
            return false;
        }
        if self.ast.types.is_array(t) {
            return true;
        }
        !self.ast.types.effective_quals(t).is_const()
            && self.is_complete(t)
            && !self.ast.types.is_void(t)
    }

    fn report_incomplete_operand(&self, t: QualType, at: Loc) {
        let d = self.diag(DiagId::sema_dereference_incomplete_type, at).arg(self.ty_arg(t));
        let d = self.incomplete_note(d, t);
        self.emit(d);
    }

    fn asm_domain(&self, t: QualType) -> Domain {
        if self.ast.types.is_integral(t)
            || self.ast.types.is_pointer(t)
            || conv::rank::is_unscoped_enum(self, t)
        {
            Domain::Int
        } else if self.ast.types.is_floating(t) {
            Domain::Fp
        } else {
            Domain::Other
        }
    }

    /// clang's `isEvaluatable` for a tied integer input, approximated by
    /// the integer-literal forms.
    fn is_constant_operand(&self, e: ExprId) -> bool {
        use kiln_ast::expr::ExprKind;
        let e = self.ignore_parens(e);
        matches!(
            self.ast.exprs.get(e).kind(),
            ExprKind::IntegerLiteral | ExprKind::CharacterLiteral | ExprKind::CXXBoolLiteralExpr
        )
    }

    /// clang's `StringLiteral::getLocationOfByte`: the source location of
    /// byte `off` of the (possibly concatenated) ordinary literal whose
    /// first token is at `start`; an escape maps to its backslash.
    pub(crate) fn string_byte_loc(&self, start: Loc, off: u32) -> Loc {
        let sm = self.diags.sm();
        let Some((file, pos)) = sm.offset_of(start) else { return start };
        let text = sm.text(file).as_bytes();
        let at = |i: usize| u32::try_from(i).map_or(start, |o| sm.loc(file, o));
        let mut i = pos as usize;
        let mut left = off as usize;
        loop {
            // One literal token.
            if text.get(i) == Some(&b'R') && text.get(i + 1) == Some(&b'"') {
                let open = i + 2 + text[i + 2..].iter().position(|&c| c == b'(').unwrap_or(0);
                let delim = &text[i + 2..open];
                let body = open + 1;
                let mut close = body;
                while close < text.len()
                    && !(text[close] == b')'
                        && text[close + 1..].starts_with(delim)
                        && text.get(close + 1 + delim.len()) == Some(&b'"'))
                {
                    close += 1;
                }
                if left < close - body {
                    return at(body + left);
                }
                left -= close - body;
                i = close + 2 + delim.len();
            } else if text.get(i) == Some(&b'"') {
                i += 1;
                while i < text.len() && text[i] != b'"' {
                    let (len, units) = escape_len(&text[i..]);
                    if units > 0 && left < units {
                        return at(i);
                    }
                    left -= units.min(left);
                    i += len.max(1);
                }
                i += 1;
            } else {
                return at(i);
            }
            // Whitespace and comments up to the next literal.
            loop {
                match text.get(i) {
                    Some(c) if c.is_ascii_whitespace() => i += 1,
                    Some(b'/') if text.get(i + 1) == Some(&b'/') => {
                        while i < text.len() && text[i] != b'\n' {
                            i += 1;
                        }
                    }
                    Some(b'/') if text.get(i + 1) == Some(&b'*') => {
                        i += 2;
                        while i + 1 < text.len() && !(text[i] == b'*' && text[i + 1] == b'/') {
                            i += 1;
                        }
                        i += 2;
                    }
                    _ => break,
                }
            }
            if i >= text.len() {
                return at(text.len().saturating_sub(1));
            }
        }
    }
}

/// clang's `AsmDomain`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Domain {
    Int,
    Fp,
    Other,
}

/// The source length and the number of string bytes of the character or
/// escape at the start of `t` (inside an ordinary literal).
fn escape_len(t: &[u8]) -> (usize, usize) {
    if t[0] != b'\\' || t.len() < 2 {
        return (1, 1);
    }
    match t[1] {
        b'\n' => (2, 0),
        b'\r' if t.get(2) == Some(&b'\n') => (3, 0),
        b'x' => {
            if t.get(2) == Some(&b'{') {
                let n = t.iter().position(|&c| c == b'}').map_or(t.len(), |p| p + 1);
                return (n, 1);
            }
            let n = t[2..].iter().take_while(|c| c.is_ascii_hexdigit()).count();
            (2 + n, 1)
        }
        b'0'..=b'7' => {
            let n = t[1..].iter().take(3).take_while(|c| (b'0'..=b'7').contains(c)).count();
            (1 + n, 1)
        }
        b'o' | b'N' if t.get(2) == Some(&b'{') => {
            let n = t.iter().position(|&c| c == b'}').map_or(t.len(), |p| p + 1);
            let units = if t[1] == b'o' { 1 } else { 3 };
            (n, units)
        }
        b'u' | b'U' => {
            let digits = if t[1] == b'u' { 4 } else { 8 };
            let hex = t.get(2..2 + digits).and_then(|h| std::str::from_utf8(h).ok());
            let cp = hex.and_then(|h| u32::from_str_radix(h, 16).ok()).unwrap_or(0);
            let units = match cp {
                0..=0x7f => 1,
                0x80..=0x7ff => 2,
                0x800..=0xffff => 3,
                _ => 4,
            };
            (2 + digits, units)
        }
        _ => (2, 1),
    }
}

// -- naked functions (WP-405) ---------------------------------------------------

impl Sema<'_, '_> {
    /// The `naked` attribute on some declaration of function `f` (clang
    /// `getAttr<NakedAttr>()`, inherited by redeclarations). A naked
    /// function gets no fall-off or noreturn check (clang
    /// `ActOnFinishFunctionBody`; `Sema::end_function_body`).
    pub(crate) fn naked_attr(&self, f: DeclId) -> Option<kiln_ast::attr::AttrNode> {
        use kiln_ast::attr::AttrKind;
        self.ast.decls.function(f)?;
        self.ast.redecls_of(f).find_map(|r| {
            self.ast
                .lists
                .iter::<kiln_ast::AttrId>(self.ast.attrs.of_decl(r))
                .map(|a| *self.ast.attrs.get(a))
                .find(|n| n.kind == AttrKind::NAKED)
        })
    }

    /// A definition of a function some declaration of which carries
    /// `aarch64_vector_pcs`: kiln would give it the base calling
    /// convention, which preserves fewer registers than its callers
    /// assume, so it is the class-A error (WP-805; WP-808 implements it).
    pub(crate) fn check_vector_pcs_definition(&mut self, f: DeclId) {
        use kiln_ast::attr::AttrKind;
        let loc = self.ast.redecls_of(f).find_map(|r| {
            self.ast
                .lists
                .iter::<kiln_ast::AttrId>(self.ast.attrs.of_decl(r))
                .map(|a| *self.ast.attrs.get(a))
                .find(|n| n.kind == AttrKind::AARCH64_VECTOR_PCS_DECL)
                .map(|n| n.loc)
        });
        if let Some(loc) = loc {
            let msg = self
                .diag(kiln_diag::DiagId::sema_attribute_unsupported_by_kiln, loc)
                .arg("aarch64_vector_pcs");
            self.emit(msg);
        }
    }

    /// A `naked` function's body holds only asm statements, null
    /// statements and local register variables without an initializer
    /// (clang `ActOnFinishFunctionBody`): the first other statement is
    /// an error, with a note at the attribute, and invalidates `f`.
    pub(crate) fn check_naked_body(&mut self, f: DeclId, body: StmtId) {
        use kiln_ast::attr::AttrKind;
        use kiln_ast::stmt::StmtKind;
        let Some(naked) = self.naked_attr(f) else { return };
        let stmts: Vec<StmtId> = if self.ast.stmts.get(body).kind() == StmtKind::CompoundStmt {
            let l = self.ast.stmts.compound_body(body);
            self.ast.lists.iter::<StmtId>(l).collect()
        } else {
            vec![body]
        };
        for s in stmts {
            let kind = self.ast.stmts.get(s).kind();
            if matches!(kind, StmtKind::GCCAsmStmt | StmtKind::NullStmt) {
                continue;
            }
            if kind == StmtKind::DeclStmt {
                let l = self.ast.stmts.decl_stmt_decls(s);
                let register_vars = self.ast.lists.iter::<DeclId>(l).all(|d| {
                    self.ast.decls.var(d).is_none_or(|v| {
                        v.init.is_none()
                            && self
                                .ast
                                .lists
                                .iter::<kiln_ast::AttrId>(self.ast.attrs.of_decl(d))
                                .any(|a| self.ast.attrs.get(a).kind == AttrKind::ASM_LABEL)
                    })
                });
                if register_vars {
                    continue;
                }
            }
            let at = self.stmt_begin_loc(s);
            let note = self.diag(DiagId::note_attribute, naked.loc);
            let msg = self.diag(DiagId::sema_naked_non_asm_stmt, at).with_note(note);
            self.emit(msg);
            self.ast.decls.set_flags(f, kiln_ast::decl::DeclFlags::INVALID);
            return;
        }
    }

    /// An asm operand of a `naked` function may not name a parameter or
    /// `this` (clang `CheckNakedParmReference`): reports one in `e`, in
    /// clang's work-list order, and returns true.
    fn check_naked_parm_ref(&mut self, e: ExprId) -> bool {
        use kiln_ast::decl::DeclKind;
        use kiln_ast::expr::ExprKind;
        let Some(f) = self.func.as_ref().map(|s| s.decl) else { return false };
        let Some(naked) = self.naked_attr(f) else { return false };
        let mut work = vec![e];
        while let Some(e) = work.pop() {
            let id = match self.ast.exprs.get(e).kind() {
                ExprKind::CXXThisExpr => Some(DiagId::sema_asm_naked_this_ref),
                ExprKind::DeclRefExpr
                    if self.ast.decls.get(self.ast.exprs.decl(e)).kind() == DeclKind::ParmVar =>
                {
                    Some(DiagId::sema_asm_naked_parm_ref)
                }
                _ => None,
            };
            if let Some(id) = id {
                let at = self.begin_loc(e);
                let note = self.diag(DiagId::note_attribute, naked.loc);
                let msg = self.diag(id, at).with_note(note);
                self.emit(msg);
                return true;
            }
            let kids = self.ast.exprs.children(e, &self.ast.lists);
            // clang's work list: last child first.
            work.extend(kids);
        }
        false
    }
}
