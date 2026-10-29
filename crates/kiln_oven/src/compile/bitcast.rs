//! Lowering of `__builtin_bit_cast` ([bit.cast]; spec/oven.md "Builtins").
//!
//! The operand (a glvalue) is evaluated first; then clang 19's static
//! checks run: `checkBitCastConstexprEligibilityType` on the destination
//! and then the source type (unions, pointers, member pointers, volatile
//! and reference members are not allowed), and the unsupported cases of
//! its buffer converters that depend on the types alone (a bit-field in
//! the source, types it has no byte image for). What remains is one
//! `Op::Builtin` (`bi::BIT_CAST`) the VM evaluates through the layout.

use kiln_ast::decl::{DeclKind, TagKind};
use kiln_ast::types::{PrintOptions, TypeKind};
use kiln_ast::{Ast, ExprId, QualType};

use super::{Access, Compiler};
use crate::Oven;
use crate::failure::Note;
use crate::kbc::{Op, bi};
use crate::ty::{OTy, TyKind};

/// clang's `note_constexpr_bit_cast_invalid_type` reasons.
const REASONS: [&str; 5] = ["union", "pointer", "member pointer", "volatile", "reference"];

/// clang's `checkBitCastConstexprEligibilityType`: the reason `q` cannot
/// be bit-cast in a constant expression, with the notes naming the
/// subobjects on the way to the offending type (innermost first).
fn ineligible(ast: &Ast, q: QualType, notes: &mut Vec<Note>) -> Option<usize> {
    let c = ast.types.canonical(q);
    let rec = ast.types.as_record(c).and_then(|d| ast.definition(d));
    if rec.and_then(|d| ast.decls.record(d)).is_some_and(|r| r.tag == TagKind::Union) {
        return Some(0);
    }
    match ast.types.canonical_kind(c) {
        TypeKind::Pointer => return Some(1),
        TypeKind::MemberPointer => return Some(2),
        _ => {}
    }
    if c.quals().is_volatile() {
        return Some(3);
    }
    if let Some(def) = rec {
        let owner = kiln_ast::types::print_type(&ast.types, ast, c, PrintOptions::CANONICAL);
        for b in ast.decls.bases(def, &ast.lists) {
            if let Some(r) = ineligible(ast, b.ty, notes) {
                let t = Oven::type_name(ast, b.ty);
                notes.push(Note::new(b.loc, format!("invalid type '{t}' is a base of '{owner}'")));
                return Some(r);
            }
        }
        for m in ast.members(def).filter(|&m| ast.decls.get(m).kind() == DeclKind::Field) {
            let n = ast.decls.get(m);
            let fq = n.ty().unwrap_or(QualType::INT);
            if ast.types.is_reference(fq) {
                return Some(4);
            }
            if let Some(r) = ineligible(ast, fq, notes) {
                let t = Oven::type_name(ast, fq);
                let loc = ast.decls.cold.begin_loc.get(m).copied().unwrap_or(n.loc());
                notes.push(Note::new(loc, format!("invalid type '{t}' is a member of '{owner}'")));
                return Some(r);
            }
        }
    }
    let mut e = c;
    while let Some(el) = ast.types.element(e).filter(|_| ast.types.is_array(e)) {
        e = el;
    }
    if e != c {
        return ineligible(ast, e, notes);
    }
    None
}

impl Compiler<'_> {
    /// The note for a type the Oven cannot bit-cast (clang's buffer
    /// converters' unsupported cases, plus classes with bases, which the
    /// Oven's object model does not lay out), the source checked first.
    fn bit_cast_unsupported(&self, t: OTy, src: bool) -> Option<String> {
        let types = &self.o.types;
        match types.kind(t) {
            TyKind::Opaque => {
                let n = self.o.tname(self.ast(), t);
                Some(format!("constexpr bit cast involving type '{n}' is not yet supported"))
            }
            TyKind::Array { elem, .. } => self.bit_cast_unsupported(elem, src),
            TyKind::Record { decl, .. } => {
                if self.ast().decls.bases(decl, &self.ast().lists).next().is_some() {
                    return Some(
                        "kiln: constant evaluation of __builtin_bit_cast of a class with base classes is not supported yet"
                            .to_owned(),
                    );
                }
                for f in types.fields(t) {
                    // The destination's bit-fields are reported when the
                    // walk reaches them (after earlier fields' faults).
                    if src && f.width.is_some() {
                        return Some(
                            "constexpr bit_cast involving bit-field is not yet supported"
                                .to_owned(),
                        );
                    }
                    if let Some(m) = self.bit_cast_unsupported(f.ty, src) {
                        return Some(m);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Only the read of the source object (`bi::READ_ONLY`), which clang
    /// performs before its type checks.
    fn bit_cast_read(&mut self, e: ExprId, src: u32, ts: OTy, td: OTy, loc: kiln_source::Loc) {
        let r = self.reg();
        let aux = bi::BIT_CAST | bi::READ_ONLY << 8;
        self.emit_site(Op::Builtin, aux, &[r, src, src, src, ts.0, td.0], e, loc);
    }

    /// `__builtin_bit_cast` `e`: into the object `mem` points to when
    /// given (the result may then hold indeterminate bytes), else as the
    /// scalar value in `dst`.
    pub(super) fn bit_cast(&mut self, e: ExprId, dst: u32, mem: Option<u32>) {
        let op = self.ast().exprs.operand(e);
        let src = self.lv_mem(op, Access::Read);
        let loc = self.begin(e);
        let (qs, qd) = (self.node(op).ty(), self.node(e).ty());
        // clang reads the source (as `const`) before checking the types.
        if qs.quals().is_volatile() {
            let t = Oven::type_name(self.ast(), qs);
            let t = if qs.quals().is_const() { t } else { format!("const {t}") };
            let msg = format!(
                "read of volatile-qualified type '{t}' is not allowed in a constant expression"
            );
            self.trap_loc(loc, msg);
            return;
        }
        let (ts, td) = (self.ty(qs), self.ty(qd));
        let mut notes = Vec::new();
        let bad = ineligible(self.ast(), qd, &mut notes)
            .map(|r| (r, "to"))
            .or_else(|| ineligible(self.ast(), qs, &mut notes).map(|r| (r, "from")));
        if let Some((r, dir)) = bad {
            let (with, what) = if r == 4 { ("type with a ", "member") } else { ("", "type") };
            let msg = format!(
                "bit_cast {dir} a {with}{} {what} is not allowed in a constant expression",
                REASONS[r]
            );
            self.bit_cast_read(e, src, ts, td, loc);
            self.trap_notes(loc, msg, notes);
            return;
        }
        if let Some(m) =
            self.bit_cast_unsupported(ts, true).or_else(|| self.bit_cast_unsupported(td, false))
        {
            self.bit_cast_read(e, src, ts, td, loc);
            self.trap_loc(loc, m);
            return;
        }
        if mem.is_none() && self.o.types.is_aggregate(td) {
            self.unsupported(e);
            return;
        }
        let (b, sub) = mem.map_or((src, 0), |p| (p, bi::TO_MEM));
        self.emit_site(
            Op::Builtin,
            bi::BIT_CAST | sub << 8,
            &[dst, src, b, src, ts.0, td.0],
            e,
            loc,
        );
    }
}
