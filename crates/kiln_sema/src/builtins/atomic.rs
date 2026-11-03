//! The atomic builtins (WP-154; spec/sema.md §5.4): clang's
//! `Sema::BuildAtomicExpr` for the `__c11_atomic_*` and `__atomic_*`
//! operations, which become an `AtomicExpr`.
//!
//! The pointer operand gets the array/function/lvalue conversions and must
//! point to a complete, non-zero-sized object (an `_Atomic` one for the
//! C11 forms); the other operands are copy-initialized as parameters of
//! the types clang derives from the pointee (the value type, `ptrdiff_t`
//! for pointer arithmetic, a pointer to the value type for the GNU
//! by-address forms, `bool` for `weak`, `int` for the memory orders). A
//! constant memory order that is invalid for the operation is warned
//! about (`-Watomic-memory-ordering`).

use kiln_ast::decl::builtin::atomic::{self, Form};
use kiln_ast::types::{FloatFormat, TypeView};
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::{self, ConvCtx};

/// Which value types an `Arithmetic` operation allows besides integers
/// (clang's `ArithAllows`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Allows {
    /// Integers only (`and`, `or`, `xor`, `nand`).
    Int,
    /// And floating types (`max`, `min`).
    IntFp,
    /// And pointers and floating types (`add`, `sub`).
    IntPtrFp,
}

/// clang's `isValidOrderingForOp` for the success order of `op`.
fn valid_order(op: u32, order: i128) -> bool {
    if !(0..=5).contains(&order) {
        return false;
    }
    match atomic::form(op) {
        Some(Form::Copy) => !matches!(order, 1 | 2 | 4),
        Some(Form::Load | Form::LoadCopy) => !matches!(order, 3 | 4),
        _ => true,
    }
}

impl Sema<'_, '_> {
    /// A call of atomic operation `id` (see [`atomic::is_op`]): the
    /// `AtomicExpr`, or `None` after a diagnostic. `at` is where the
    /// builtin's name begins, `rparen` the closing `)`.
    pub(crate) fn builtin_atomic_op(
        &mut self,
        at: Loc,
        id: u32,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<ExprId> {
        let form = atomic::form(id)?;
        if !self.builtin_arg_count(args, form.num_args(), rparen) {
            return None;
        }
        let is_c11 = atomic::is_c11(id);
        let ptr = conv::decay(self, args[0]);
        if self.is_err_expr(ptr) {
            return None;
        }
        let pt = self.ety(ptr);
        let Some(atom) = self.ast.types.pointee(pt).filter(|_| self.ast.types.is_pointer(pt))
        else {
            let msg = self
                .diag(DiagId::sema_atomic_builtin_must_be_pointer, at)
                .arg(self.ty_arg(pt))
                .arg(0);
            self.emit(msg);
            return None;
        };
        let atom = self.ast.types.canonical(atom);
        let loads = matches!(form, Form::Load | Form::LoadCopy);
        let mut val = atom;
        if is_c11 {
            let Some(v) = self.ast.types.as_atomic(atom) else {
                let msg = self.diag(DiagId::sema_atomic_op_needs_atomic, at).arg(self.ty_arg(pt));
                self.emit(msg);
                return None;
            };
            if !loads && atom.quals().is_const() {
                let msg = self
                    .diag(DiagId::sema_atomic_op_needs_non_const_atomic, at)
                    .arg(0)
                    .arg(self.ty_arg(pt));
                self.emit(msg);
                return None;
            }
            val = self.ast.types.canonical(v);
        } else if !loads && atom.quals().is_const() {
            let msg =
                self.diag(DiagId::sema_atomic_op_needs_non_const_pointer, at).arg(self.ty_arg(pt));
            self.emit(msg);
            return None;
        }
        let ptr_at = self.begin_loc(ptr);
        if !self.require_complete(atom, ptr_at) {
            let msg =
                self.diag(DiagId::sema_incomplete_type_required, ptr_at).arg(self.ty_arg(atom));
            self.emit(self.incomplete_note(msg, atom));
            return None;
        }
        if self.size_align(atom).is_none_or(|(s, _)| s == 0) {
            let msg = self
                .diag(DiagId::sema_atomic_builtin_must_be_pointer, at)
                .arg(self.ty_arg(pt))
                .arg(1);
            self.emit(msg);
            return None;
        }
        let is_ptr = self.atomic_value_ok(id, form, at, pt, atom, val, ptr_at)?;
        // The pointee's cv-qualifiers do not reach the value type.
        let val = self.ast.types.unqualified(val);
        let result = match form {
            Form::Copy | Form::LoadCopy | Form::GnuXchg | Form::Init => QualType::VOID,
            Form::C11CmpXchg | Form::GnuCmpXchg => QualType::BOOL,
            _ => val,
        };
        let by_value = if atomic::by_value(id) { val } else { pt };
        let conv_args = self.atomic_convert_args(form, args, ptr, val, by_value, is_ptr)?;
        // clang's `SubExprs`: `ptr`, the order, then the values, the
        // failure order and `weak` as the form has them.
        let subs: Vec<ExprId> = {
            let a = &conv_args;
            match form {
                Form::Init | Form::Load => vec![a[0], a[1]],
                Form::LoadCopy | Form::Copy | Form::Arithmetic | Form::Xchg => {
                    vec![a[0], a[2], a[1]]
                }
                Form::GnuXchg => vec![a[0], a[3], a[1], a[2]],
                Form::C11CmpXchg => vec![a[0], a[3], a[1], a[4], a[2]],
                Form::GnuCmpXchg => vec![a[0], a[4], a[1], a[5], a[2], a[3]],
            }
        };
        self.atomic_check_orders(id, form, &subs);
        if matches!(self.ast.types.canonical_view(val), TypeView::BitInt { .. }) {
            let at = self.eloc(ptr);
            let msg = self.diag(DiagId::sema_atomic_builtin_bit_int_prohibited, at);
            self.emit(msg);
            return None;
        }
        Some(self.builder().atomic_expr(id, &subs, result, at, rparen))
    }

    /// The value-type constraints of clang `BuildAtomicExpr` on `atom`
    /// (the pointee of `pt`) and its value type `val`: whether `val` is a
    /// pointer, or `None` after a diagnostic.
    #[allow(clippy::too_many_arguments)]
    fn atomic_value_ok(
        &mut self,
        id: u32,
        form: Form,
        at: Loc,
        pt: QualType,
        atom: QualType,
        val: QualType,
        ptr_at: Loc,
    ) -> Option<bool> {
        let is_c11 = atomic::is_c11(id);
        let is_n =
            matches!(id, atomic::LOAD_N | atomic::STORE_N | atomic::EXCHANGE_N | atomic::CMPXCHG_N);
        let is_int = conv::rank::is_integral_or_unscoped_enum(self, val)
            || matches!(self.ast.types.canonical_view(val), TypeView::BitInt { .. });
        let is_ptr = self.ast.types.is_pointer(val);
        if form == Form::Arithmetic {
            let allows = match atomic::rmw(id).map(|r| r.0) {
                Some(atomic::Rmw::Add | atomic::Rmw::Sub) => Allows::IntPtrFp,
                Some(atomic::Rmw::Max | atomic::Rmw::Min) => Allows::IntFp,
                _ => Allows::Int,
            };
            let fp_ok = self.ast.types.is_floating(val)
                && !(self.ast.types.same_unqualified(val, QualType::LONGDOUBLE)
                    && self.target.long_double == FloatFormat::X87Extended);
            let ok = is_int
                || (is_ptr && allows == Allows::IntPtrFp)
                || (fp_ok && allows != Allows::Int);
            if !ok {
                let id = match allows {
                    Allows::IntPtrFp => DiagId::sema_atomic_op_needs_atomic_int_ptr_or_fp,
                    Allows::IntFp => DiagId::sema_atomic_op_needs_atomic_int_or_fp,
                    Allows::Int => DiagId::sema_atomic_op_needs_atomic_int,
                };
                let msg = self.diag(id, at).arg(u32::from(is_c11)).arg(self.ty_arg(pt));
                self.emit(msg);
                return None;
            }
            if is_c11
                && is_ptr
                && let Some(p) = self.ast.types.pointee(val)
                // A function type is not incomplete ([basic.types.general]/5):
                // `_Atomic(void (*)(int))` arithmetic is checked no further here.
                && !self.ast.types.is_function(p)
                && !self.require_complete(p, ptr_at)
            {
                let msg =
                    self.diag(DiagId::sema_incomplete_type_required, ptr_at).arg(self.ty_arg(p));
                self.emit(self.incomplete_note(msg, p));
                return None;
            }
        } else if is_n && !is_int && !is_ptr {
            let msg = self
                .diag(DiagId::sema_atomic_op_needs_atomic_int_or_ptr, at)
                .arg(u32::from(is_c11))
                .arg(self.ty_arg(pt));
            self.emit(msg);
            return None;
        }
        if !is_c11 && !self.is_trivially_copyable_type(atom) && !self.is_scalar(atom) {
            let msg = self.diag(DiagId::sema_atomic_op_needs_trivial_copy, at).arg(self.ty_arg(pt));
            self.emit(msg);
            return None;
        }
        Some(is_ptr)
    }

    /// The operands after `ptr` converted to the types the form takes
    /// (clang `BuildAtomicExpr`), in call order, or `None` after a
    /// diagnostic.
    fn atomic_convert_args(
        &mut self,
        form: Form,
        args: &[ExprId],
        ptr: ExprId,
        val: QualType,
        by_value: QualType,
        is_ptr: bool,
    ) -> Option<Vec<ExprId>> {
        let mut conv_args = Vec::with_capacity(args.len());
        conv_args.push(ptr);
        for (i, &a) in args.iter().enumerate().skip(1) {
            let ty = if i <= form.num_vals() {
                match i {
                    1 if form == Form::Arithmetic && is_ptr => {
                        QualType::unqual(self.target.ptrdiff_t)
                    }
                    1 if matches!(form, Form::Init | Form::Arithmetic) => val,
                    1 if matches!(form, Form::Copy | Form::Xchg) => by_value,
                    1 => self.ast.types.pointer_to(val),
                    2 => by_value,
                    _ => QualType::BOOL,
                }
            } else {
                QualType::INT
            };
            match conv::implicit_convert(self, a, ty, ConvCtx::Argument) {
                Ok(x) if !self.is_err_expr(x) => conv_args.push(x),
                Ok(_) => return None,
                Err(err) => {
                    let msg = err.diag(self, a);
                    self.emit(msg);
                    return None;
                }
            }
        }
        Some(conv_args)
    }

    /// clang 22 builds `__atomic_test_and_set` and `__atomic_clear` as
    /// atomic operations and checks a constant order (a clear is a
    /// store); clang 19 does not (spec/sema.md §1.1). `id` is the
    /// builtin called with `args`.
    pub(crate) fn atomic_flag_order_check(&mut self, id: u32, args: &[ExprId]) {
        if self.clang_compat < 22 || !matches!(id, atomic::CLEAR | atomic::TEST_AND_SET) {
            return;
        }
        if let [_, order] = args
            && let Some(o) = self.fold_int(*order)
            && (!(0..=5).contains(&o) || id == atomic::CLEAR && matches!(o, 1 | 2 | 4))
        {
            let at = self.begin_loc(*order);
            let msg = self.diag(DiagId::sema_atomic_op_invalid_memory_order, at).arg(0u32);
            self.emit(msg);
        }
    }

    /// `-Watomic-memory-ordering` for a constant order of `subs` (clang
    /// `BuildAtomicExpr`).
    fn atomic_check_orders(&mut self, id: u32, form: Form, subs: &[ExprId]) {
        let cmpxchg = matches!(form, Form::C11CmpXchg | Form::GnuCmpXchg);
        if form != Form::Init
            && let Some(o) = self.fold_int(subs[1])
            && !valid_order(id, o)
        {
            let at = self.begin_loc(subs[1]);
            let msg =
                self.diag(DiagId::sema_atomic_op_invalid_memory_order, at).arg(u32::from(cmpxchg));
            self.emit(msg);
        }
        if subs.len() >= 5
            && let Some(o) = self.fold_int(subs[3])
            && !matches!(o, 0 | 1 | 2 | 5)
        {
            let at = self.begin_loc(subs[3]);
            let msg = self.diag(DiagId::sema_atomic_op_invalid_memory_order, at).arg(2);
            self.emit(msg);
        }
    }
}
