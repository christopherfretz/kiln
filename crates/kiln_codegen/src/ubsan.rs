//! `UndefinedBehaviorSanitizer` checks (WP-675, docs/design/sanitizers.md
//! §1.4, spec/codegen.md "Sanitizers").
//!
//! Mirrors clang 19's `CodeGenFunction::EmitCheck` and its callers: each
//! check is an `i1` that holds when the operation is fine; the checks of
//! one operation are and-ed per mode (`-fsanitize-trap=` first, then the
//! recoverable and the fatal ones) and branch to `llvm.ubsantrap` or to a
//! `__ubsan_handle_*` call whose static data (source location, type
//! descriptors) the runtime prints. Every instruction a check emits
//! carries `!nosanitize !{}` (clang's `SanitizerScope`).

use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::sanitize::SanitizerSet as San;
use kiln_ast::types::{PrintOptions, TypeView, aka, print_type};
use kiln_ast::{ExprId, QualType};
use kiln_llvm::intrinsics::OverflowOp;
use kiln_llvm::{
    Attr, AttrKind, AttrList, BinOp, CallOpts, CastOp, Flags, GlobalId, GlobalSanitizer,
    IntPredicate, Linkage, MdKindId, MetadataId, TypeId as LTy, UnnamedAddr, UwTableKind, Value,
};
use kiln_source::Loc;

use crate::expr::LV;
use crate::fir::{Blk, Fir, Op, V};
use crate::module::{Cg, R, err};

/// clang's `SanitizerHandler`: the runtime entry point of a check and
/// the `llvm.ubsantrap` immediate (its index in clang's handler list).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UbHandler {
    AddOverflow,
    BuiltinUnreachable,
    DivremOverflow,
    FloatCastOverflow,
    InvalidBuiltin,
    LoadInvalidValue,
    MissingReturn,
    MulOverflow,
    NegateOverflow,
    NonnullArg,
    NonnullReturn,
    OutOfBounds,
    PointerOverflow,
    ShiftOutOfBounds,
    SubOverflow,
    TypeMismatch,
    VlaBoundNotPositive,
}

impl UbHandler {
    fn id(self) -> u8 {
        match self {
            UbHandler::AddOverflow => 0,
            UbHandler::BuiltinUnreachable => 1,
            UbHandler::DivremOverflow => 3,
            UbHandler::FloatCastOverflow => 5,
            UbHandler::InvalidBuiltin => 8,
            UbHandler::LoadInvalidValue => 10,
            UbHandler::MissingReturn => 11,
            UbHandler::MulOverflow => 12,
            UbHandler::NegateOverflow => 13,
            UbHandler::NonnullArg => 16,
            UbHandler::NonnullReturn => 17,
            UbHandler::OutOfBounds => 18,
            UbHandler::PointerOverflow => 19,
            UbHandler::ShiftOutOfBounds => 20,
            UbHandler::SubOverflow => 21,
            UbHandler::TypeMismatch => 22,
            UbHandler::VlaBoundNotPositive => 24,
        }
    }

    fn name(self) -> &'static str {
        match self {
            UbHandler::AddOverflow => "add_overflow",
            UbHandler::BuiltinUnreachable => "builtin_unreachable",
            UbHandler::DivremOverflow => "divrem_overflow",
            UbHandler::FloatCastOverflow => "float_cast_overflow",
            UbHandler::InvalidBuiltin => "invalid_builtin",
            UbHandler::LoadInvalidValue => "load_invalid_value",
            UbHandler::MissingReturn => "missing_return",
            UbHandler::MulOverflow => "mul_overflow",
            UbHandler::NegateOverflow => "negate_overflow",
            UbHandler::NonnullArg => "nonnull_arg",
            UbHandler::NonnullReturn => "nonnull_return_v1",
            UbHandler::OutOfBounds => "out_of_bounds",
            UbHandler::PointerOverflow => "pointer_overflow",
            UbHandler::ShiftOutOfBounds => "shift_out_of_bounds",
            UbHandler::SubOverflow => "sub_overflow",
            UbHandler::TypeMismatch => "type_mismatch_v1",
            UbHandler::VlaBoundNotPositive => "vla_bound_not_positive",
        }
    }
}

/// clang `CodeGenFunction::TypeCheckKind`: what a type check guards
/// (the last byte of the `type_mismatch` static data).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tck {
    Load = 0,
    Store = 1,
    ReferenceBinding = 2,
    MemberAccess = 3,
    MemberCall = 4,
    ConstructorCall = 5,
    DowncastPointer = 6,
    DowncastReference = 7,
    Upcast = 8,
    UpcastToVirtualBase = 9,
    DynamicOperation = 11,
}

impl Tck {
    /// clang `isNullPointerAllowed`.
    fn null_allowed(self) -> bool {
        matches!(
            self,
            Tck::DowncastPointer | Tck::Upcast | Tck::UpcastToVirtualBase | Tck::DynamicOperation
        )
    }
}

impl Cg<'_> {
    /// Whether the function being emitted checks `k` (clang `SanOpts`).
    pub(crate) fn ub_on(&self, k: San) -> bool {
        self.f.san.has_any(k)
    }

    /// Opens a check scope (clang `SanitizerScope`); returns the outer
    /// scope for [`Cg::ub_end`].
    pub(crate) fn ub_begin(&mut self) -> Option<(MdKindId, MetadataId)> {
        let k = self.m.md_kind("nosanitize");
        let md = self.m.md_tuple(&[]);
        self.f.fir.set_nosanitize(Some((k, md)))
    }

    /// Closes the scope [`Cg::ub_begin`] opened.
    pub(crate) fn ub_end(&mut self, outer: Option<(MdKindId, MetadataId)>) {
        self.f.fir.set_nosanitize(outer);
    }

    /// The sanitizer metadata of a global the sanitizers must leave
    /// alone (clang `SanitizerMetadata::disableSanitizerForGlobal`).
    fn ub_global_sanitizer(&self) -> GlobalSanitizer {
        let k = self.opts.sanitize.kinds;
        GlobalSanitizer {
            no_address: k.has_any(San::ADDRESS.union(San::KERNEL_ADDRESS)),
            no_hwaddress: k.has_any(San::HWADDRESS.union(San::KERNEL_HWADDRESS)),
            ..GlobalSanitizer::default()
        }
    }

    /// A private `unnamed_addr` global `@N` holding `init`.
    fn ub_global(&mut self, init: Value, constant: bool) -> GlobalId {
        let ty = self.m.const_type(init);
        let g = self.m.add_global("", ty);
        let _ = self.m.set_initializer(g, Some(init));
        let san = self.ub_global_sanitizer();
        let gv = self.m.global_mut(g);
        gv.linkage = Linkage::Private;
        gv.unnamed_addr = UnnamedAddr::Global;
        gv.is_constant = constant;
        gv.sanitizer = san;
        g
    }

    /// clang `EmitCheckSourceLocation`: `{ ptr file, i32 line, i32 col }`
    /// of `loc`'s presumed position (`{ null, 0, 0 }` without one).
    pub(crate) fn ub_loc(&mut self, loc: Option<Loc>) -> R<Value> {
        let p = match (self.sm, loc) {
            (Some(sm), Some(l)) => {
                sm.presumed_line_col(l).map(|p| (p.file_name.to_owned(), p.line, p.col))
            }
            _ => None,
        };
        let elems = match p {
            Some((name, line, col)) => {
                let mut bytes = name.into_bytes();
                bytes.push(0);
                let n = bytes.len() as u64;
                let g = self.string_global_named(&bytes, 1, n, ".src");
                let san = self.ub_global_sanitizer();
                self.m.global_mut(g).sanitizer = san;
                [
                    Value::global(g),
                    self.m.const_int(LTy::I32, i128::from(line)),
                    self.m.const_int(LTy::I32, i128::from(col)),
                ]
            }
            None => [
                self.m.const_null(LTy::PTR),
                self.m.const_int(LTy::I32, 0),
                self.m.const_int(LTy::I32, 0),
            ],
        };
        self.m.const_anon_struct(&elems, false).or_else(|_| err(loc, "bad UBSan source location"))
    }

    /// clang `EmitCheckTypeDescriptor`: `{ i16 kind, i16 info, [N x i8]
    /// name }`, once per type. An integer is kind 0 with info
    /// `log2(bits) << 1 | signed`, a floating type kind 1 with its width,
    /// anything else kind `0xffff`; the name is `q` as a diagnostic
    /// prints it (`'T'` or `'T' (aka 'U')`).
    pub(crate) fn ub_type(&mut self, q: QualType) -> R<Value> {
        self.ub_type_keyed(q, false)
    }

    /// [`Cg::ub_type`] keyed apart for a class type clang takes from
    /// `getRecordType` (`record`): its `TypeDescriptorMap` is keyed by
    /// the `QualType`, and the written (elaborated) spelling of a class is
    /// a different one, so each gets its own descriptor.
    pub(crate) fn ub_type_keyed(&mut self, q: QualType, record: bool) -> R<Value> {
        if let Some(&g) = self.ub_types.get(&(q, record)) {
            return Ok(Value::global(g));
        }
        let ast = self.ast;
        let (kind, info): (u16, u16) = match ast.types.canonical_view(q) {
            TypeView::Builtin(b) if b.is_integral() => {
                let bits = self.size_align(q).0 * 8;
                (0, ((bits.max(1).ilog2() as u16) << 1) | u16::from(self.is_signed(q)))
            }
            TypeView::Enum(d)
                if ast
                    .decls
                    .enumeration(ast.definition(d).unwrap_or(d))
                    .is_some_and(|e| !e.is_scoped() && e.int_type.is_some()) =>
            {
                let bits = self.size_align(q).0 * 8;
                (0, ((bits.max(1).ilog2() as u16) << 1) | u16::from(self.is_signed(q)))
            }
            TypeView::Builtin(b) if b.is_floating() => {
                (1, (self.size_align(q).0 * 8).min(u64::from(u16::MAX)) as u16)
            }
            _ => (0xffff, 0),
        };
        let s = print_type(&ast.types, ast, q, PrintOptions::SUGARED);
        let name = match aka(&ast.types, ast, q) {
            Some(a) => format!("'{s}' (aka '{a}')"),
            None => format!("'{s}'"),
        };
        let mut bytes = name.into_bytes();
        bytes.push(0);
        let elems = [
            self.m.const_int(LTy::I16, i128::from(kind)),
            self.m.const_int(LTy::I16, i128::from(info)),
            self.m.const_bytes(&bytes),
        ];
        let Ok(init) = self.m.const_anon_struct(&elems, false) else {
            return err(None, "bad UBSan type descriptor");
        };
        let g = self.ub_global(init, true);
        self.ub_types.insert((q, record), g);
        Ok(Value::global(g))
    }

    /// `a & b` as `IRBuilder::CreateAnd` (folded only when both are
    /// constants).
    fn ub_and(&mut self, a: V, b: V) -> V {
        self.bin(BinOp::And, Flags::NONE, a, b)
    }

    /// Whether checks need `nomerge` and their own trap blocks (clang 20+
    /// `NoMerge`; clang 19 only stops sharing trap blocks): at `-O0` or
    /// in an `optnone` function.
    fn ub_no_merge(&self) -> bool {
        !self.opts.optimize()
            || self.f.decl.is_some_and(|d| self.has_fn_attr(d, kiln_ast::attr::AttrKind::OPTNONE))
    }

    /// clang `EmitCheck`: branches on `checks` to the trap or the handler
    /// `h` called with the static data `statics` and the values `dynamics`
    /// (in a check scope).
    pub(crate) fn ub_check(
        &mut self,
        checks: &[(V, San)],
        h: UbHandler,
        statics: &[Value],
        dynamics: &[V],
    ) -> R<()> {
        let (trap_set, recover_set) = (self.opts.sanitize.trap, self.opts.sanitize.recover);
        let (mut trap, mut recover, mut fatal) = (None, None, None);
        for &(c, k) in checks {
            let slot = if trap_set.has_any(k) {
                &mut trap
            } else if recover_set.has_any(k) {
                &mut recover
            } else {
                &mut fatal
            };
            *slot = Some(match *slot {
                Some(p) => self.ub_and(p, c),
                None => c,
            });
        }
        // clang 20+ `-fsanitize-merge=` (default: the `undefined` group)
        // keeps the other checks' traps and calls apart.
        let kind_no_merge =
            self.opts.clang_compat >= 22 && checks.iter().any(|&(_, k)| !San::UNDEFINED.has_any(k));
        if let Some(t) = trap {
            self.ub_trap_check(t, h, kind_no_merge)?;
        }
        if recover.is_none() && fatal.is_none() {
            return Ok(());
        }
        let joint = match (fatal, recover) {
            (Some(f), Some(r)) => self.ub_and(f, r),
            (f, r) => f.or(r).expect("a fatal or recoverable check"),
        };
        let unrecoverable = matches!(h, UbHandler::MissingReturn | UbHandler::BuiltinUnreachable);
        let cont = self.f.fir.create_block();
        let handler = self.f.fir.create_block();
        self.ub_likely_br(joint, cont, handler)?;
        self.emit_block(handler, false);
        let mut args = Vec::with_capacity(dynamics.len() + 1);
        if !statics.is_empty() {
            let Ok(init) = self.m.const_anon_struct(statics, false) else {
                return err(None, "bad UBSan check data");
            };
            args.push(V::C(Value::global(self.ub_global(init, false))));
        }
        for &v in dynamics {
            args.push(self.ub_check_value(v));
        }
        match (fatal, recover) {
            (Some(f), Some(_)) => {
                let non_fatal = self.f.fir.create_block();
                let fatal_bb = self.f.fir.create_block();
                self.f.fir.push(Op::CondBr { c: f, t: non_fatal, f: fatal_bb }, LTy::VOID);
                self.emit_block(fatal_bb, false);
                self.ub_handler_call(
                    h,
                    args.clone(),
                    true,
                    unrecoverable,
                    non_fatal,
                    kind_no_merge,
                )?;
                self.emit_block(non_fatal, false);
                self.ub_handler_call(h, args, false, unrecoverable, cont, kind_no_merge)?;
            }
            (f, _) => {
                self.ub_handler_call(h, args, f.is_some(), unrecoverable, cont, kind_no_merge)?;
            }
        }
        self.emit_block(cont, false);
        Ok(())
    }

    /// `br i1 c, t, f, !prof !{"branch_weights", 1048575, 1}` (clang's
    /// likely branch to the continuation), `!nosanitize` after `!prof`.
    fn ub_likely_br(&mut self, c: V, t: Blk, f: Blk) -> R<()> {
        self.ub_br(c, t, f, true)
    }

    /// `br i1 c, t, f`, with the likely branch weights when `likely`.
    fn ub_br(&mut self, c: V, t: Blk, f: Blk, likely: bool) -> R<()> {
        if !likely {
            self.f.fir.push(Op::CondBr { c, t, f }, LTy::VOID);
            return Ok(());
        }
        let outer = self.f.fir.set_nosanitize(None);
        let br = self.f.fir.push(Op::CondBr { c, t, f }, LTy::VOID);
        let s = self.m.md_string(b"branch_weights");
        let w1 = self.m.const_int(LTy::I32, (1 << 20) - 1);
        let w2 = self.m.const_int(LTy::I32, 1);
        let (Ok(m1), Ok(m2)) = (self.m.md_const(w1), self.m.md_const(w2)) else {
            return err(None, "bad branch weights");
        };
        let md = self.m.md_tuple(&[Some(s), Some(m1), Some(m2)]);
        let k = self.m.md_kind("prof");
        let i = Fir::last_inst(br);
        self.f.fir.add_md(i, k, md);
        if let Some((nk, nmd)) = outer {
            self.f.fir.add_md(i, nk, nmd);
        }
        self.f.fir.set_nosanitize(outer);
        Ok(())
    }

    /// clang `EmitTrapCheck`: `br i1 c, cont, trap` (likely under clang
    /// 22) and a trap block calling `llvm.ubsantrap(i8 id)`, shared per
    /// handler above `-O0` unless `kind_no_merge`.
    fn ub_trap_check(&mut self, c: V, h: UbHandler, kind_no_merge: bool) -> R<()> {
        let no_merge = kind_no_merge || self.ub_no_merge();
        let likely = self.opts.clang_compat >= 22;
        let cont = self.f.fir.create_block();
        if !no_merge && let Some(&(_, tb)) = self.f.ub_traps.iter().find(|t| t.0 == h.id()) {
            self.ub_br(c, cont, tb, likely)?;
            self.emit_block(cont, false);
            return Ok(());
        }
        let tb = self.f.fir.create_block();
        self.ub_br(c, cont, tb, likely)?;
        self.emit_block(tb, false);
        let trap = kiln_llvm::intrinsics::ubsantrap(&mut self.m);
        let mut site = vec![Attr::Flag(AttrKind::NoReturn), Attr::Flag(AttrKind::NoUnwind)];
        if no_merge && self.opts.clang_compat >= 22 {
            site.insert(0, Attr::Flag(AttrKind::NoMerge));
        }
        let id = V::C(self.m.const_int(LTy::I8, i128::from(h.id())));
        self.ub_call(trap, vec![id], &site);
        self.f.fir.push(Op::Unreachable, LTy::VOID);
        if !no_merge {
            self.f.ub_traps.push((h.id(), tb));
        }
        self.emit_block(cont, false);
        Ok(())
    }

    /// A call of `f` with call-site function attributes `site`.
    fn ub_call(&mut self, f: kiln_llvm::FuncId, args: Vec<V>, site: &[Attr]) {
        let fn_ty = self.m.func(f).ty();
        let fa = self.m.attr_set(site);
        let opts =
            CallOpts { attrs: AttrList { func: fa, ..AttrList::default() }, ..CallOpts::default() };
        let c = self.f.fir.push(
            Op::Call { fn_ty, callee: V::C(Value::func(f)), args, opts: Box::new(opts) },
            LTy::VOID,
        );
        self.f.calls.push((Fir::last_inst(c), Some(f), true));
    }

    /// clang `emitCheckHandlerCall`: `__ubsan_handle_<name>[_abort]`,
    /// then `br cont` (recoverable) or `unreachable`.
    fn ub_handler_call(
        &mut self,
        h: UbHandler,
        args: Vec<V>,
        fatal: bool,
        unrecoverable: bool,
        cont: Blk,
        kind_no_merge: bool,
    ) -> R<()> {
        let abort = if fatal && !unrecoverable { "_abort" } else { "" };
        let name = format!("__ubsan_handle_{}{abort}", h.name());
        let may_return = !fatal;
        let f = if let Some(f) = self.m.get_function(&name) {
            f
        } else {
            {
                let params: Vec<LTy> = args.iter().map(|&a| self.f.fir.ty(&self.m, a)).collect();
                let ty = self.m.types.function(LTy::VOID, &params, false);
                let f = match self.m.add_function(&name, ty) {
                    Ok(f) => f,
                    Err(e) => return err(None, format!("cannot declare `{name}`: {e:?}")),
                };
                let mut fa = Vec::new();
                if !may_return {
                    fa.push(Attr::Flag(AttrKind::NoReturn));
                    fa.push(Attr::Flag(AttrKind::NoUnwind));
                }
                fa.push(Attr::UwTable(UwTableKind::Async));
                let a = self.m.attr_set(&fa);
                self.m.func_mut(f).attrs.func = a;
                f
            }
        };
        let mut site = Vec::new();
        if self.opts.clang_compat >= 22 && (kind_no_merge || self.ub_no_merge()) {
            site.push(Attr::Flag(AttrKind::NoMerge));
        }
        if !may_return {
            site.push(Attr::Flag(AttrKind::NoReturn));
        }
        site.push(Attr::Flag(AttrKind::NoUnwind));
        self.ub_call(f, args, &site);
        if may_return {
            self.f.fir.push(Op::Br { dest: cont }, LTy::VOID);
        } else {
            self.f.fir.push(Op::Unreachable, LTy::VOID);
        }
        Ok(())
    }

    /// clang `EmitCheckValue`: `v` as an `i64` handler argument (an
    /// integer or a float up to 64 bits by value, anything else by the
    /// address of a temporary).
    fn ub_check_value(&mut self, v: V) -> V {
        let t = self.f.fir.ty(&self.m, v);
        if t == LTy::I64 {
            return v;
        }
        let mut v = v;
        if self.is_float_ty(t) {
            let bits = self.m.types.float_width(t).unwrap_or(128);
            if bits <= 64 {
                let it = self.m.types.int(bits);
                v = self.cast(CastOp::BitCast, v, it);
            }
        }
        let t = self.f.fir.ty(&self.m, v);
        if self.m.types.int_width(t).is_some_and(|w| w <= 64) {
            return self.cast(CastOp::ZExt, v, LTy::I64);
        }
        if t != LTy::PTR {
            let bits = u64::from(self.m.types.int_width(t).unwrap_or(128));
            let align = (bits / 8).next_power_of_two().clamp(1, 16);
            let p = self.f.fir.alloca(t, align);
            self.store(v, p, align, false);
            v = p;
        }
        self.cast(CastOp::PtrToInt, v, LTy::I64)
    }

    // ---- checks ---------------------------------------------------------

    /// The operands of the binary or compound assignment `e`.
    fn ub_operands(&self, e: ExprId) -> Option<(ExprId, ExprId)> {
        let ast = self.ast;
        match ast.exprs.get(e).kind() {
            ExprKind::BinaryOperator => {
                let [l, r] = ast.exprs.operands(e);
                Some((l, r))
            }
            ExprKind::CompoundAssignOperator => {
                Some((self.sub(e), ast.exprs.compound_assign(e).rhs))
            }
            _ => None,
        }
    }

    /// clang `getUnwidenedIntegerType`: the promotable integer type `e`
    /// was implicitly widened from.
    fn ub_unwidened(&mut self, e: ExprId) -> Option<QualType> {
        let ast = self.ast;
        let mut b = e;
        while matches!(
            ast.exprs.get(b).kind(),
            ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr | ExprKind::ExprWithCleanups
        ) {
            b = self.sub(b);
        }
        if b == e {
            return None;
        }
        let bq = ast.exprs.get(b).ty();
        let promotable = match ast.types.canonical_view(bq) {
            TypeView::Builtin(t) => {
                use kiln_ast::types::BuiltinType as B;
                matches!(
                    t,
                    B::Bool
                        | B::Char
                        | B::SChar
                        | B::UChar
                        | B::Short
                        | B::UShort
                        | B::WChar
                        | B::Char8
                        | B::Char16
                        | B::Char32
                )
            }
            TypeView::Enum(d) => ast
                .decls
                .enumeration(ast.definition(d).unwrap_or(d))
                .is_some_and(|x| !x.is_scoped() && x.promotion_type.is_some()),
            _ => false,
        };
        let eq = ast.exprs.get(e).ty();
        (promotable && self.size_align(bq).0 < self.size_align(eq).0).then_some(bq)
    }

    /// clang `mayHaveIntegerOverflow` of `l op r` (`signed` arithmetic):
    /// only two constants can be proven not to overflow.
    fn ub_may_overflow(&self, op: BinaryOp, l: V, r: V, signed: bool) -> bool {
        let (Some((_, a, w)), Some((_, b, _))) = (self.cint(l), self.cint(r)) else {
            return true;
        };
        let sx = |x: u128| -> i128 {
            if w >= 128 { x as i128 } else { ((x << (128 - w)) as i128) >> (128 - w) }
        };
        let (sa, sb) = (sx(a), sx(b));
        let (smin, smax) = if w >= 128 {
            (i128::MIN, i128::MAX)
        } else {
            (-(1i128 << (w - 1)), (1i128 << (w - 1)) - 1)
        };
        let umax = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
        let s_ok = |x: Option<i128>| x.is_some_and(|x| (smin..=smax).contains(&x));
        let u_ok = |x: Option<u128>| x.is_some_and(|x| x <= umax);
        match op {
            BinaryOp::Add => {
                if signed {
                    !s_ok(sa.checked_add(sb))
                } else {
                    !u_ok(a.checked_add(b))
                }
            }
            BinaryOp::Sub => {
                if signed {
                    !s_ok(sa.checked_sub(sb))
                } else {
                    a < b
                }
            }
            BinaryOp::Mul => {
                if signed {
                    !s_ok(sa.checked_mul(sb))
                } else {
                    !u_ok(a.checked_mul(b))
                }
            }
            BinaryOp::Div | BinaryOp::Rem => signed && b != 0 && sa == smin && sb == -1,
            _ => true,
        }
    }

    /// clang `CanElideOverflowCheck` for the binary `e` (`op` on `l`,
    /// `r` in type `q`).
    fn ub_can_elide(&mut self, e: ExprId, op: BinaryOp, l: V, r: V, q: QualType) -> bool {
        if !self.ub_may_overflow(op, l, r, self.is_signed(q)) {
            return true;
        }
        let Some((le, re)) = self.ub_operands(e) else { return false };
        let (Some(lq), Some(rq)) = (self.ub_unwidened(le), self.ub_unwidened(re)) else {
            return false;
        };
        let unsigned = |c: &Self, t: QualType| c.ub_int(t) && !c.is_signed(t);
        if op != BinaryOp::Mul || !unsigned(self, lq) || !unsigned(self, rq) {
            return true;
        }
        let promoted = self.size_align(self.ast.exprs.get(e).ty()).0;
        2 * self.size_align(lq).0 < promoted || 2 * self.size_align(rq).0 < promoted
    }

    /// clang `isIntegerType` for a scalar (no vectors).
    pub(crate) fn ub_int(&self, q: QualType) -> bool {
        match self.ast.types.canonical_view(q) {
            TypeView::Builtin(b) => b.is_integral(),
            TypeView::Enum(_) | TypeView::BitInt { .. } => true,
            _ => false,
        }
    }

    /// The overflow check kind of integer arithmetic in `q`, if the
    /// function checks it (clang `EmitAdd`/`EmitSub`/`EmitMul`).
    pub(crate) fn ub_overflow_kind(&self, q: QualType) -> Option<San> {
        if !self.ub_int(q) || self.f.san.is_empty() {
            return None;
        }
        let k = if self.is_signed(q) {
            San::SIGNED_INTEGER_OVERFLOW
        } else {
            San::UNSIGNED_INTEGER_OVERFLOW
        };
        self.ub_on(k).then_some(k)
    }

    /// The flags of unchecked integer arithmetic in `q` (`nsw` when
    /// `signed`): a signed check the function could make but elided keeps
    /// `nsw` even under `-fwrapv` (clang `EmitAdd`'s fallthrough).
    pub(crate) fn ub_nsw(&self, q: QualType, signed: bool) -> Flags {
        if signed && self.ub_overflow_kind(q) == Some(San::SIGNED_INTEGER_OVERFLOW) {
            Flags::NSW
        } else {
            self.nsw_if(signed)
        }
    }

    /// The checked `l op r` (`+ - *` of the binary or compound assignment
    /// `e` in type `q`), or `None` when no check applies.
    pub(crate) fn ub_binary_overflow(
        &mut self,
        e: ExprId,
        op: BinaryOp,
        l: V,
        r: V,
        q: QualType,
    ) -> R<Option<V>> {
        let Some(k) = self.ub_overflow_kind(q) else { return Ok(None) };
        if self.ub_can_elide(e, op, l, r, q) {
            return Ok(None);
        }
        let h = match op {
            BinaryOp::Add => UbHandler::AddOverflow,
            BinaryOp::Sub => UbHandler::SubOverflow,
            _ => UbHandler::MulOverflow,
        };
        self.ub_overflow(e, op, l, r, q, k, h, &[l, r]).map(Some)
    }

    /// The checked `0 - v` of unary minus `e` (type `q`, `can_overflow`
    /// as sema computed), or `None`.
    pub(crate) fn ub_negate(&mut self, e: ExprId, v: V, q: QualType) -> R<Option<V>> {
        let Some(k) = self.ub_overflow_kind(q) else { return Ok(None) };
        let t = self.f.fir.ty(&self.m, v);
        let z = self.icn(t, 0);
        let can_overflow = !self.ast.exprs.get(e).has(kiln_ast::expr::ExprFlags::CANNOT_OVERFLOW);
        if !self.ub_may_overflow(BinaryOp::Sub, z, v, self.is_signed(q)) || !can_overflow {
            return Ok(None);
        }
        let h = UbHandler::NegateOverflow;
        self.ub_overflow(e, BinaryOp::Sub, z, v, q, k, h, &[v]).map(Some)
    }

    /// The checked increment or decrement `old ± 1` of `e`, or `None`.
    pub(crate) fn ub_inc_dec(&mut self, e: ExprId, old: V, inc: bool) -> R<Option<V>> {
        let q = self.ast.exprs.get(e).ty();
        let Some(k) = self.ub_overflow_kind(q) else { return Ok(None) };
        if self.ast.exprs.get(e).has(kiln_ast::expr::ExprFlags::CANNOT_OVERFLOW) {
            return Ok(None);
        }
        let t = self.f.fir.ty(&self.m, old);
        let one = self.icn(t, 1);
        let (op, h) = if inc {
            (BinaryOp::Add, UbHandler::AddOverflow)
        } else {
            (BinaryOp::Sub, UbHandler::SubOverflow)
        };
        self.ub_overflow(e, op, old, one, q, k, h, &[old, one]).map(Some)
    }

    /// clang `EmitOverflowCheckedBinOp`: `llvm.{s,u}{add,sub,mul}.with.
    /// overflow`, the check `!overflow` reported by `h` with type `q`.
    #[allow(clippy::too_many_arguments)]
    fn ub_overflow(
        &mut self,
        e: ExprId,
        op: BinaryOp,
        l: V,
        r: V,
        q: QualType,
        k: San,
        h: UbHandler,
        dynamics: &[V],
    ) -> R<V> {
        let signed = self.is_signed(q);
        let oop = match (op, signed) {
            (BinaryOp::Add, true) => OverflowOp::SAdd,
            (BinaryOp::Add, false) => OverflowOp::UAdd,
            (BinaryOp::Sub, true) => OverflowOp::SSub,
            (BinaryOp::Sub, false) => OverflowOp::USub,
            (_, true) => OverflowOp::SMul,
            (_, false) => OverflowOp::UMul,
        };
        let outer = self.ub_begin();
        let ty = self.f.fir.ty(&self.m, l);
        let f = kiln_llvm::intrinsics::with_overflow(&mut self.m, oop, ty);
        let rty = self.m.types.struct_literal(&[ty, LTy::I1], false);
        let pair = self.intrinsic_call_ty(f, vec![l, r], rty);
        let v = self.f.fir.push(Op::ExtractValue { agg: pair, idx: 0 }, ty);
        let ovf = self.f.fir.push(Op::ExtractValue { agg: pair, idx: 1 }, LTy::I1);
        let t = V::C(self.m.const_bool(true));
        let ok = self.bin(BinOp::Xor, Flags::NONE, ovf, t);
        let loc = self.ub_loc(Some(self.loc_of(e).unwrap_or_default()))?;
        let desc = self.ub_type(q)?;
        let r = self.ub_check(&[(ok, k)], h, &[loc, desc], dynamics);
        self.ub_end(outer);
        r.map(|()| v)
    }

    /// clang `EmitUndefinedBehaviorIntegerDivAndRemCheck` before `l / r`
    /// or `l % r` of `e` in type `q`.
    pub(crate) fn ub_divrem(&mut self, e: ExprId, op: BinaryOp, l: V, r: V, q: QualType) -> R<()> {
        let (zero_on, ovf_on) =
            (self.ub_on(San::INTEGER_DIVIDE_BY_ZERO), self.ub_on(San::SIGNED_INTEGER_OVERFLOW));
        if !(zero_on || ovf_on) || !self.ub_int(q) {
            return Ok(());
        }
        let signed = self.is_signed(q);
        let may_div0 = self.cint(r).is_none_or(|c| c.1 == 0);
        let may_ovf = self.ub_may_overflow(op, l, r, signed);
        if !may_div0 && !may_ovf {
            return Ok(());
        }
        let outer = self.ub_begin();
        let t = self.f.fir.ty(&self.m, l);
        let mut checks = Vec::new();
        if zero_on {
            let z = self.icn(t, 0);
            checks.push((self.icmp(IntPredicate::Ne, r, z), San::INTEGER_DIVIDE_BY_ZERO));
        }
        let widened = self.ub_operands(e).and_then(|(le, _)| self.ub_unwidened(le)).is_some();
        if ovf_on && signed && !widened && may_ovf {
            let w = self.m.types.int_width(t).unwrap_or(32);
            let min = self.icn(t, if w >= 128 { i128::MIN } else { -(1i128 << (w - 1)) });
            let neg1 = self.icn(t, -1);
            let a = self.icmp(IntPredicate::Ne, l, min);
            let b = self.icmp(IntPredicate::Ne, r, neg1);
            let ok = self.bin(BinOp::Or, Flags::NONE, a, b);
            checks.push((ok, San::SIGNED_INTEGER_OVERFLOW));
        }
        let res = if checks.is_empty() {
            Ok(())
        } else {
            let loc = self.ub_loc(self.loc_of(e))?;
            let desc = self.ub_type(q)?;
            self.ub_check(&checks, UbHandler::DivremOverflow, &[loc, desc], &[l, r])
        };
        self.ub_end(outer);
        res
    }

    /// clang `EmitShl`/`EmitShr` checks of `e` (`l` shifted by the
    /// unconverted `r`; `rp` is `r` converted to `l`'s type, `q` the
    /// computation type, `rq` the right operand's type).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ub_shift(
        &mut self,
        e: ExprId,
        shl: bool,
        l: V,
        r: V,
        rp: V,
        q: QualType,
        rq: QualType,
    ) -> R<()> {
        let lt = self.f.fir.ty(&self.m, l);
        let Some(w) = self.m.types.int_width(lt) else { return Ok(()) };
        let signed_base = self.ub_on(San::SHIFT_BASE)
            && self.is_signed(self.scalar_of(q))
            && !self.opts.wrapv
            && !self.opts.sanitize.cxx20;
        let unsigned_base = self.ub_on(San::UNSIGNED_SHIFT_BASE)
            && self.ub_int(self.scalar_of(q))
            && !self.is_signed(self.scalar_of(q));
        let base = shl && (signed_base || unsigned_base);
        let exponent = self.ub_on(San::SHIFT_EXPONENT);
        if !base && !exponent {
            return Ok(());
        }
        let outer = self.ub_begin();
        let r_signed = self.is_signed(self.scalar_of(rq));
        let rt = self.f.fir.ty(&self.m, r);
        let max = self.ub_max_shift(w, rt, r_signed);
        let valid = self.icmp(IntPredicate::Ule, r, max);
        let mut checks = Vec::new();
        if exponent {
            checks.push((valid, San::SHIFT_EXPONENT));
        }
        if base {
            let orig = self.f.fir.cur().expect("insertion point");
            let cont = self.f.fir.create_block();
            let check = self.f.fir.create_block();
            self.f.fir.push(Op::CondBr { c: valid, t: check, f: cont }, LTy::VOID);
            let pmax = if rp == r { max } else { self.ub_max_shift(w, lt, r_signed) };
            self.emit_block(check, false);
            let zeros = self.bin(BinOp::Sub, Flags::NUW.with(Flags::NSW), pmax, rp);
            let mut off = self.bin(BinOp::LShr, Flags::NONE, l, zeros);
            if unsigned_base || self.opts.sanitize.cplusplus {
                let one = self.icn(lt, 1);
                off = self.bin(BinOp::LShr, Flags::NONE, off, one);
            }
            let z = self.icn(lt, 0);
            let ok = self.icmp(IntPredicate::Eq, off, z);
            let from = self.f.fir.cur().expect("insertion point");
            self.emit_block(cont, false);
            let t = V::C(self.m.const_bool(true));
            let phi =
                self.f.fir.push(Op::Phi { ty: LTy::I1, inc: vec![(t, orig), (ok, from)] }, LTy::I1);
            checks
                .push((phi, if signed_base { San::SHIFT_BASE } else { San::UNSIGNED_SHIFT_BASE }));
        }
        let res = (|| {
            let loc = self.ub_loc(self.loc_of(e))?;
            let lq = self.ub_operands(e).map_or(q, |(le, _)| self.ast.exprs.get(le).ty());
            let ld = self.ub_type(lq)?;
            let rd = self.ub_type(rq)?;
            self.ub_check(&checks, UbHandler::ShiftOutOfBounds, &[loc, ld, rd], &[l, r])
        })();
        self.ub_end(outer);
        res
    }

    /// clang `GetMaximumShiftAmount`: `width - 1` in type `rt`, capped at
    /// `rt`'s maximum.
    fn ub_max_shift(&mut self, w: u32, rt: LTy, signed: bool) -> V {
        let rw = self.m.types.int_width(rt).unwrap_or(w);
        let rmax: u128 = if signed {
            (1u128 << (rw - 1)) - 1
        } else if rw >= 128 {
            u128::MAX
        } else {
            (1u128 << rw) - 1
        };
        let v = if rmax < u128::from(w) { rmax } else { u128::from(w - 1) };
        self.icn(rt, v as i128)
    }

    /// The `return` check at the fall-off end of a value-returning
    /// function (clang `EmitFunctionEpilog`'s `MissingReturn`), at the
    /// function name's location; the caller emits the `unreachable`.
    pub(crate) fn ub_missing_return(&mut self, loc: Option<Loc>) -> R<()> {
        let outer = self.ub_begin();
        let f = V::C(self.m.const_bool(false));
        let res = (|| {
            let l = self.ub_loc(loc)?;
            self.ub_check(&[(f, San::RETURN)], UbHandler::MissingReturn, &[l], &[])
        })();
        self.ub_end(outer);
        res
    }

    /// clang `EmitUnreachable`'s `BuiltinUnreachable` check of
    /// `__builtin_unreachable()` at `loc`; the caller emits the
    /// `unreachable`.
    pub(crate) fn ub_unreachable(&mut self, loc: Option<Loc>) -> R<()> {
        let outer = self.ub_begin();
        let f = V::C(self.m.const_bool(false));
        let res = (|| {
            let l = self.ub_loc(loc)?;
            self.ub_check(&[(f, San::UNREACHABLE)], UbHandler::BuiltinUnreachable, &[l], &[])
        })();
        self.ub_end(outer);
        res
    }

    /// clang `EmitVariablyModifiedType`'s `vla-bound` check of the bound
    /// `size` (expression `se`, type `sq`): `size > 0`.
    pub(crate) fn ub_vla_bound(&mut self, se: ExprId, size: V, sq: QualType) -> R<()> {
        if !self.ub_on(San::VLA_BOUND) {
            return Ok(());
        }
        let outer = self.ub_begin();
        let res = (|| {
            let t = self.f.fir.ty(&self.m, size);
            let z = self.icn(t, 0);
            let p = if self.is_signed(sq) { IntPredicate::Sgt } else { IntPredicate::Ugt };
            let ok = self.icmp(p, size, z);
            let loc = self.ub_loc(Some(self.ast.begin_loc(se)))?;
            let desc = self.ub_type(sq)?;
            self.ub_check(
                &[(ok, San::VLA_BOUND)],
                UbHandler::VlaBoundNotPositive,
                &[loc, desc],
                &[size],
            )
        })();
        self.ub_end(outer);
        res
    }

    // ---- type checks (null, alignment, object-size) ---------------------

    /// clang `sanitizePerformTypeCheck`.
    pub(crate) fn ub_type_check_on(&self) -> bool {
        self.ub_on(San::NULL.union(San::ALIGNMENT).union(San::OBJECT_SIZE))
    }

    /// `icmp ne ptr p, null` as clang's `CreateIsNotNull` with LLVM 19's
    /// constant folder: `true` for a non-`extern_weak` global or function
    /// and an `inbounds` constant GEP of one.
    fn ub_is_not_null(&mut self, p: V) -> V {
        if let V::C(c) = p
            && self.ub_const_nonnull(c)
        {
            return V::C(self.m.const_bool(true));
        }
        let z = V::C(self.m.const_null(LTy::PTR));
        self.f.fir.push(Op::ICmp { p: IntPredicate::Ne, l: p, r: z }, LTy::I1)
    }

    fn ub_const_nonnull(&self, c: Value) -> bool {
        use kiln_llvm::ValueRef;
        match c.get() {
            ValueRef::Global(g) => self.m.global(g).linkage != Linkage::ExternWeak,
            ValueRef::Func(f) => self.m.func(f).linkage != Linkage::ExternWeak,
            ValueRef::Const(k) => match self.m.const_data(k) {
                kiln_llvm::ConstData::Gep { flags, ops, .. } => {
                    flags.0 & kiln_llvm::GepFlags::INBOUNDS.0 != 0
                        && matches!(
                            ops.first().map(|b| b.get()),
                            Some(ValueRef::Global(_) | ValueRef::Func(_))
                        )
                        && self.ub_const_nonnull(ops[0])
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// LLVM `stripPointerCasts` then `dyn_cast<AllocaInst>`: the alignment
    /// of the `alloca` `p` is, through all-zero-index GEPs.
    fn ub_alloca_align(&self, p: V) -> Option<u64> {
        let mut p = p;
        loop {
            let V::I(i) = p else { return None };
            match self.f.fir.op(i) {
                Op::Alloca { align, .. } => return Some(*align),
                Op::Gep { base, idx, .. }
                    if idx.iter().all(|&x| self.cint(x).is_some_and(|c| c.1 == 0)) =>
                {
                    p = *base;
                }
                _ => return None,
            }
        }
    }

    /// clang `getNaturalTypeAlignment(q, forPointeeType)` for a complete
    /// `q`: an aligned typedef's alignment, else [`Cg::pointee_layout`].
    pub(crate) fn ub_natural_align(&mut self, q: QualType) -> u64 {
        if let Some(a) = self.typedef_align_attr(q) {
            return a;
        }
        self.pointee_layout(q).1
    }

    /// clang `EmitTypeCheck` (without `vptr`): the null, object-size and
    /// alignment checks of an access of kind `tck` to the `q` at `ptr`,
    /// aligned `align` (0: `q`'s natural pointee alignment), except the
    /// checks in `skip`; `array_size` multiplies the object size.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ub_type_check(
        &mut self,
        tck: Tck,
        loc: Option<Loc>,
        ptr: V,
        q: QualType,
        align: u64,
        skip: San,
        array_size: Option<V>,
    ) -> R<()> {
        if !self.ub_type_check_on() || self.is_volatile(q) {
            return Ok(());
        }
        let outer = self.ub_begin();
        let r = self.ub_type_check_in(tck, loc, ptr, q, align, skip, array_size);
        self.ub_end(outer);
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn ub_type_check_in(
        &mut self,
        tck: Tck,
        loc: Option<Loc>,
        ptr: V,
        q: QualType,
        align: u64,
        skip: San,
        array_size: Option<V>,
    ) -> R<()> {
        let mut checks: Vec<(V, San)> = Vec::new();
        let mut done = None;
        let alloca = self.ub_alloca_align(ptr);
        let guaranteed = skip.has_any(San::NULL) || alloca.is_some();
        let allow_null = tck.null_allowed();
        if (self.ub_on(San::NULL) || allow_null) && !guaranteed {
            let nn = self.ub_is_not_null(ptr);
            if self.cint(nn).is_none_or(|c| c.1 == 0) {
                if allow_null {
                    let d = self.f.fir.create_block();
                    let rest = self.f.fir.create_block();
                    self.f.fir.push(Op::CondBr { c: nn, t: rest, f: d }, LTy::VOID);
                    self.emit_block(rest, false);
                    done = Some(d);
                } else {
                    checks.push((nn, San::NULL));
                }
            }
        }
        let (size, _) = self.pointee_layout(q);
        let complete = size.is_some();
        if self.ub_on(San::OBJECT_SIZE)
            && !skip.has_any(San::OBJECT_SIZE)
            && let Some(sz) = size
        {
            let mut n = self.icn(LTy::I64, i128::from(sz));
            if let Some(a) = array_size {
                n = self.bin(BinOp::Mul, Flags::NONE, n, a);
            }
            if self.cint(n).is_none_or(|c| c.1 != 0) {
                let f = kiln_llvm::intrinsics::objectsize(&mut self.m, LTy::I64);
                let fl = V::C(self.m.const_bool(false));
                let os = self.intrinsic_call_ty(f, vec![ptr, fl, fl, fl], LTy::I64);
                checks.push((self.icmp(IntPredicate::Uge, os, n), San::OBJECT_SIZE));
            }
        }
        let mut align_val = None;
        let mut as_int = None;
        if self.ub_on(San::ALIGNMENT) && !skip.has_any(San::ALIGNMENT) {
            align_val = if align != 0 {
                Some(align)
            } else if complete {
                Some(self.ub_natural_align(q))
            } else {
                None
            };
            if let Some(a) = align_val
                && a > 1
                && alloca.is_none_or(|x| x < a)
            {
                let pi = self.cast(CastOp::PtrToInt, ptr, LTy::I64);
                let m = self.icn(LTy::I64, (a - 1) as i128);
                let low = self.bin(BinOp::And, Flags::NONE, pi, m);
                let z = self.icn(LTy::I64, 0);
                let ok = self.icmp(IntPredicate::Eq, low, z);
                as_int = Some(pi);
                if self.cint(ok).is_none_or(|c| c.1 == 0) {
                    checks.push((ok, San::ALIGNMENT));
                }
            }
        }
        if !checks.is_empty() {
            let l = self.ub_loc(loc)?;
            // clang takes the class type from `getRecordType` for these
            // kinds and through the type of `this` (the member checks it
            // skips for null and alignment).
            let record = matches!(
                tck,
                Tck::MemberCall | Tck::ConstructorCall | Tck::Upcast | Tck::UpcastToVirtualBase
            ) || (tck == Tck::MemberAccess && skip.has(San::ALIGNMENT.union(San::NULL)));
            let desc = self.ub_type_keyed(q, record)?;
            let lg = align_val.map_or(1, |a| a.max(1).ilog2());
            let lg = self.m.const_int(LTy::I8, i128::from(lg));
            let k = self.m.const_int(LTy::I8, tck as i128);
            self.ub_check(&checks, UbHandler::TypeMismatch, &[l, desc, lg, k], &[as_int
                .unwrap_or(ptr)])?;
        }
        if let Some(d) = done {
            self.f.fir.push(Op::Br { dest: d }, LTy::VOID);
            self.emit_block(d, false);
        }
        Ok(())
    }

    /// clang `IsWrappedCXXThis`: `e` is `this` under casts (not a
    /// `dynamic_cast`), parentheses and `__extension__`.
    pub(crate) fn ub_wrapped_this(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let mut b = e;
        loop {
            let k = ast.exprs.get(b).kind();
            b = match k {
                ExprKind::CXXThisExpr => return true,
                ExprKind::CXXDynamicCastExpr => return false,
                ExprKind::ParenExpr => ast.exprs.operand(b),
                ExprKind::UnaryOperator
                    if ast.exprs.unary_op(b) == kiln_ast::expr::UnaryOp::Extension =>
                {
                    ast.exprs.operand(b)
                }
                _ if k.is_cast() => ast.exprs.operand(b),
                _ => return false,
            };
        }
    }

    /// The checks a member access through the base `base` skips (clang
    /// `EmitMemberExpr`/`EmitCheckedLValue`): alignment and null on
    /// `this`, null on a named object.
    pub(crate) fn ub_member_skips(&self, base: ExprId) -> San {
        if self.ub_wrapped_this(base) {
            San::ALIGNMENT.union(San::NULL)
        } else if self.ast.exprs.get(base).kind() == ExprKind::DeclRefExpr {
            San::NULL
        } else {
            San::NONE
        }
    }

    /// clang `EmitCheckedLValue`: the lvalue `e`, type-checked as an
    /// access of kind `tck` unless it names a declaration, is a
    /// bit-field or a vector element.
    pub(crate) fn emit_checked_lvalue(&mut self, e: ExprId, tck: Tck) -> R<LV> {
        let lv = self.emit_lvalue(e)?;
        self.ub_check_lv(e, lv, tck)?;
        Ok(lv)
    }

    /// The type check [`Cg::emit_checked_lvalue`] makes of the lvalue `lv`
    /// of `e`.
    pub(crate) fn ub_check_lv(&mut self, e: ExprId, lv: LV, tck: Tck) -> R<()> {
        if !self.ub_type_check_on() {
            return Ok(());
        }
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        if n.kind() == ExprKind::DeclRefExpr || lv.bf.is_some() || lv.vidx.is_some() {
            return Ok(());
        }
        let skip = if n.kind() == ExprKind::MemberExpr {
            self.ub_member_skips(ast.exprs.operand(e))
        } else {
            San::NONE
        };
        self.ub_type_check(tck, self.ub_expr_loc(e), lv.addr.v, n.ty(), lv.addr.align, skip, None)
    }

    /// clang `Expr::getExprLoc` where kiln's expression location differs:
    /// a subscript is located at its base.
    /// clang's `Expr::getExprLoc`: the operator or member name of the
    /// nodes that override it, the subscript base's for `a[i]`, and the
    /// first token for every other node (casts and parentheses included).
    pub(crate) fn ub_expr_loc(&self, e: ExprId) -> Option<Loc> {
        let ast = self.ast;
        let mut e = e;
        loop {
            match ast.exprs.get(e).kind() {
                ExprKind::ArraySubscriptExpr => {
                    let [l, r] = ast.exprs.operands(e);
                    e = if self.ub_int(ast.exprs.get(r).ty()) { l } else { r };
                }
                ExprKind::CXXMemberCallExpr => e = ast.exprs.callee(e),
                ExprKind::MemberExpr
                | ExprKind::BinaryOperator
                | ExprKind::CompoundAssignOperator
                | ExprKind::UnaryOperator
                | ExprKind::CXXOperatorCallExpr => return self.loc_of(e),
                _ => return Some(ast.begin_loc(e)),
            }
        }
    }
}
