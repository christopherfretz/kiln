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

use kiln_ast::expr::{BinaryOp, CastKind, ExprKind};
use kiln_ast::sanitize::SanitizerSet as San;
use kiln_ast::dump::AstNames;
use kiln_ast::types::{PrintOptions, Quals, TypeView, aka, print_type, vector_diag_arg};
use kiln_abi::mangle::Structor;
use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_ast::lambda::CaptureDefault;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_llvm::intrinsics::OverflowOp;
use kiln_llvm::{
    Attr, AttrKind, AttrList, BinOp, CallOpts, CastOp, Flags, FloatPredicate, GlobalId, GlobalSanitizer,
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

/// Which `QualType` clang names in a type check's descriptor; its
/// `TypeDescriptorMap` is keyed by the `QualType` object, so a class
/// type it builds with `getRecordType` and the written (elaborated) one
/// get separate descriptors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UbDesc {
    /// The type as written (an expression's type).
    Written,
    /// `getRecordType(class)`.
    Record,
    /// `MD->getThisType()` (`cv C *`, these `Quals` bits) of the class
    /// type checked: the `this` check of a member function's prologue.
    ThisPtr(u8),
}

/// One clang `EmitTypeCheck` call: the access kind, its location, the
/// type accessed, the alignment (0: natural), the checks skipped and the
/// element count of an array `new`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TypeCheck {
    pub tck: Tck,
    pub loc: Option<Loc>,
    pub q: QualType,
    pub desc: UbDesc,
    pub align: u64,
    pub skip: San,
    pub array_size: Option<V>,
}

impl TypeCheck {
    pub(crate) fn new(tck: Tck, loc: Option<Loc>, q: QualType) -> TypeCheck {
        TypeCheck { tck, loc, q, desc: UbDesc::Written, align: 0, skip: San::NONE, array_size: None }
    }
    pub(crate) fn align(self, align: u64) -> TypeCheck {
        TypeCheck { align, ..self }
    }
    pub(crate) fn skip(self, skip: San) -> TypeCheck {
        TypeCheck { skip, ..self }
    }
    pub(crate) fn desc(self, desc: UbDesc) -> TypeCheck {
        TypeCheck { desc, ..self }
    }
    pub(crate) fn array(self, n: V) -> TypeCheck {
        TypeCheck { array_size: Some(n), ..self }
    }
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
        self.ub_type_keyed(q, UbDesc::Written)
    }

    /// [`Cg::ub_type`] of the `QualType` `desc` names for `q`.
    pub(crate) fn ub_type_keyed(&mut self, q: QualType, desc: UbDesc) -> R<Value> {
        let key = match desc {
            UbDesc::Written => 0,
            UbDesc::Record => 1,
            UbDesc::ThisPtr(cv) => 2 + cv,
        };
        if let Some(&g) = self.ub_types.get(&(q, key)) {
            return Ok(Value::global(g));
        }
        let ast = self.ast;
        // clang 20+ (#64100): a signed `_BitInt` is kind 2 (`TK_BitInt`),
        // its exact width appended to the name as `\0`, a 32-bit
        // little-endian count and `\0`.
        let mut bit_int = None;
        let (kind, info): (u16, u16) = match ast.types.canonical_view(q) {
            TypeView::BitInt { signed, width } => {
                let bits = self.size_align(q).0 * 8;
                let k = if signed && self.opts.clang_compat >= 22 {
                    bit_int = Some(width);
                    2
                } else {
                    0
                };
                (k, ((bits.max(1).ilog2() as u16) << 1) | u16::from(signed))
            }
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
        let names = match self.sm {
            Some(sm) => AstNames::with_sources(ast, sm),
            None => AstNames::new(ast),
        };
        let s = print_type(&ast.types, &names, q, PrintOptions::SUGARED);
        let name = match (desc, aka(&ast.types, &names, q)) {
            (UbDesc::ThisPtr(cv), _) => {
                let cv = Quals::from_bits(cv);
                let mut p = String::new();
                for (on, w) in
                    [(cv.is_const(), "const "), (cv.is_volatile(), "volatile "), (cv.is_restrict(), "restrict ")]
                {
                    if on {
                        p.push_str(w);
                    }
                }
                format!("'{p}{s} *'")
            }
            (_, a) => match vector_diag_arg(&ast.types, &names, q, self.opts.clang_compat >= 22) {
                Some(v) => v,
                None => match a {
                    Some(a) => format!("'{s}' (aka '{a}')"),
                    None => format!("'{s}'"),
                },
            },
        };
        let (kind, info) = if matches!(desc, UbDesc::ThisPtr(_)) { (0xffff, 0) } else { (kind, info) };
        let nkey = (ast.types.canonical(q), key, name.clone().into_boxed_str());
        if let Some(&g) = self.ub_type_names.get(&nkey) {
            self.ub_types.insert((q, key), g);
            return Ok(Value::global(g));
        }
        let mut bytes = name.into_bytes();
        if let Some(w) = bit_int {
            bytes.push(0);
            bytes.extend_from_slice(&u32::try_from(w).unwrap_or(u32::MAX).to_le_bytes());
            bytes.push(0);
        }
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
        self.ub_types.insert((q, key), g);
        self.ub_type_names.insert(nkey, g);
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
    pub(crate) fn ub_operands(&self, e: ExprId) -> Option<(ExprId, ExprId)> {
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

    // ---- bool and enum loads ------------------------------------------

    /// clang `getRangeForType` with `StrictEnums`: the values `[min, end)`
    /// of a `bool` (or an enumeration with underlying type `bool`) or of
    /// an enumeration without a fixed underlying type
    /// (`EnumDecl::getValueRange`, [dcl.enum]/8); `None` for any other
    /// type (and for ranges beyond 64 bits).
    fn ub_value_range(&self, q: QualType) -> Option<(i128, i128)> {
        if self.has_bool_rep(q) {
            return Some((0, 2));
        }
        let ast = self.ast;
        let TypeView::Enum(d) = ast.types.canonical_view(q) else { return None };
        let d = ast.definition(d).unwrap_or(d);
        let data = ast.decls.enumeration(d)?;
        if data.flags & kiln_ast::decl::EnumData::FIXED != 0 {
            return None;
        }
        // Sema's `NumPositiveBits` (at least one for a zero enumerator)
        // and `NumNegativeBits`.
        let (mut pos, mut neg) = (0u32, 0u32);
        for m in ast.members(d) {
            let Some(v) = ast.decls.enumerator(m).and_then(|x| x.value) else { continue };
            let Some(kiln_ast::consts::Scalar::Int { bits, signed, .. }) = ast.consts.scalar(v)
            else {
                continue;
            };
            let v = if signed { bits.to_i128()? } else { i128::try_from(bits.to_u128()?).ok()? };
            if v >= 0 {
                pos = pos.max(128 - v.leading_zeros()).max(1);
            } else {
                neg = neg.max(129 - v.leading_ones());
            }
        }
        // [dcl.enum]/8: an empty enumerator-list acts as one enumerator 0.
        if pos == 0 && neg == 0 {
            pos = 1;
        }
        if neg == 0 {
            (pos < 64).then(|| (0, 1i128 << pos))
        } else {
            let n = (pos + 1).max(neg);
            (n <= 64).then(|| (-(1i128 << (n - 1)), 1i128 << (n - 1)))
        }
    }

    /// Whether a load of `q` is range-checked (clang
    /// `EmitScalarRangeCheck` returning true: no `!range` either).
    fn ub_range_wanted(&self, q: QualType) -> bool {
        (self.ub_on(San::BOOL) && self.has_bool_rep(q))
            || (self.ub_on(San::ENUM)
                && matches!(self.ast.types.canonical_view(q), TypeView::Enum(_)))
    }

    /// clang `EmitScalarRangeCheck`: `-fsanitize=bool` / `enum` on the
    /// value `v` of type `q` loaded at `loc` (in its memory form).
    pub(crate) fn ub_range_check(&mut self, v: V, q: QualType, loc: Option<Loc>) -> R<()> {
        if !self.ub_range_wanted(q) {
            return Ok(());
        }
        let t = self.f.fir.ty(&self.m, v);
        let is_bool = self.has_bool_rep(q);
        if is_bool && t == LTy::I1 {
            return Ok(());
        }
        let Some((min, end)) = self.ub_value_range(q) else { return Ok(()) };
        let outer = self.ub_begin();
        let res = (|| {
            let ok = if min == 0 {
                let c = self.icn(t, end - 1);
                self.icmp(IntPredicate::Ule, v, c)
            } else {
                let hi = self.icn(t, end - 1);
                let lo = self.icn(t, min);
                let u = self.icmp(IntPredicate::Sle, v, hi);
                let l = self.icmp(IntPredicate::Sge, v, lo);
                self.ub_and(u, l)
            };
            let sl = self.ub_loc(loc)?;
            let desc = self.ub_type(q)?;
            let enum_check =
                self.ub_on(San::ENUM) && matches!(self.ast.types.canonical_view(q), TypeView::Enum(_));
            let kind = if enum_check { San::ENUM } else { San::BOOL };
            // clang 19 converts the value (`EmitCheckValue`) before the
            // branch; clang 20+ passes it to `EmitCheck` raw.
            let cv = if self.opts.clang_compat >= 22 { v } else { self.ub_check_value(v) };
            self.ub_check(&[(ok, kind)], UbHandler::LoadInvalidValue, &[sl, desc], &[cv])
        })();
        self.ub_end(outer);
        res
    }

    /// clang `EmitLoadOfLValue(lv, loc)` for a scalar: [`Cg::load_lv`]
    /// with [`Cg::ub_range_check`] between the load and the conversion
    /// from its memory form.
    pub(crate) fn load_lv_at(&mut self, lv: LV, loc: Option<Loc>) -> R<V> {
        if !self.ub_range_wanted(lv.q) || lv.vidx.is_some() {
            return Ok(self.load_lv(lv));
        }
        if lv.bf.is_some() {
            let v = self.load_lv(lv);
            self.ub_range_check(v, lv.q, loc)?;
            return Ok(v);
        }
        let raw = self.load_scalar_mem(lv.addr, lv.q, lv.tb, false);
        self.ub_range_check(raw, lv.q, loc)?;
        Ok(self.emit_from_memory(raw, lv.q))
    }

    // ---- float-cast-overflow ---------------------------------------------

    /// clang `ScalarExprEmitter::EmitScalarConversion` of a floating
    /// value `v` of type `from` to the integer type `to` under
    /// `-fsanitize=float-cast-overflow`, with the check
    /// (`EmitFloatConversionCheck`) at `loc` between the `__fp16` → `float`
    /// extension and the `fptosi`/`fptoui`; every other conversion is
    /// [`Cg::conv`].
    pub(crate) fn conv_at(&mut self, v: V, from: QualType, to: QualType, loc: Option<Loc>) -> R<V> {
        let tt = self.lty(to);
        let ft = self.f.fir.ty(&self.m, v);
        if !self.ub_on(San::FLOAT_CAST_OVERFLOW)
            || self.is_bool(to)
            || !self.is_float_ty(ft)
            || self.m.types.int_width(tt).is_none()
            || self.ast.types.is_pointer(to)
        {
            return Ok(self.conv(v, from, to));
        }
        let src = if self.is_fp16(from) { self.cast(CastOp::FPExt, v, LTy::FLOAT) } else { v };
        let outer = self.ub_begin();
        let res = self.ub_float_cast_check(v, src, from, to, loc);
        self.ub_end(outer);
        res?;
        let op = if self.is_signed(to) { CastOp::FPToSI } else { CastOp::FPToUI };
        Ok(self.cast(op, src, tt))
    }

    /// clang `EmitFloatConversionCheck`: `src > MinSrc && src < MaxSrc`,
    /// the bounds being the largest too-small and smallest too-large
    /// values of the source format (`orig`'s, then widened to `src`'s),
    /// or an infinity where the integer bound does not fit the format.
    fn ub_float_cast_check(
        &mut self,
        orig: V,
        src: V,
        from: QualType,
        to: QualType,
        loc: Option<Loc>,
    ) -> R<()> {
        let tt = self.lty(to);
        let (ot, st) = (self.f.fir.ty(&self.m, orig), self.f.fir.ty(&self.m, src));
        let (Some(of), Some(sf)) = (fp_format(&self.m.types, ot), fp_format(&self.m.types, st))
        else {
            return Ok(());
        };
        let w = self.m.types.int_width(tt).unwrap_or(1);
        let unsigned = !self.is_signed(to);
        let one = 1u128 << (of.p - 1);
        // MinSrc: `Min - 1` rounded toward negative infinity.
        let min = if unsigned {
            Some((one, 0))
        } else {
            let k = w - 1;
            if k > of.emax {
                None
            } else if k < of.p {
                Some((((1u128 << k) + 1) << (of.p - 1 - k), k))
            } else {
                Some((one | 1, k))
            }
        };
        // MaxSrc: `Max + 1` rounded toward positive infinity, `2^k`.
        let k = if unsigned { w } else { w - 1 };
        let max = (k <= of.emax).then_some((one, k));
        let minc = V::C(self.m.const_float_bits(st, sf.encode(true, min, of.p)));
        let maxc = V::C(self.m.const_float_bits(st, sf.encode(false, max, of.p)));
        let ge = self.fcmp(FloatPredicate::Ogt, src, minc);
        let le = self.fcmp(FloatPredicate::Olt, src, maxc);
        let ok = self.ub_and(ge, le);
        let sl = self.ub_loc(loc)?;
        let fd = self.ub_type(from)?;
        let td = self.ub_type(to)?;
        self.ub_check(
            &[(ok, San::FLOAT_CAST_OVERFLOW)],
            UbHandler::FloatCastOverflow,
            &[sl, fd, td],
            &[orig],
        )
    }

    // ---- bounds ----------------------------------------------------------

    /// clang `Expr::isFlexibleArrayMemberLike` at the default
    /// `-fstrict-flex-arrays=0`: a zero-length array, or an array member
    /// that is the last field of its class (or any one- or zero-element
    /// array member of a union).
    fn ub_fam_like(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let e = self.skip_parens(e);
        let q = ast.exprs.get(e).ty();
        let size = match ast.types.canonical_view(q) {
            TypeView::ConstantArray(a) => Some(a.size),
            TypeView::IncompleteArray(_) => None,
            _ => return false,
        };
        if size == Some(0) {
            return true;
        }
        let d = match ast.exprs.get(e).kind() {
            ExprKind::MemberExpr | ExprKind::DeclRefExpr => ast.exprs.decl(e),
            _ => return false,
        };
        if ast.decls.get(d).kind() != DeclKind::Field {
            return false;
        }
        let Some(rd) = ast.decls.get(d).ctx() else { return false };
        if size == Some(1) && self.is_union(rd) {
            return true;
        }
        ast.decls.members(rd).filter(|&m| ast.decls.get(m).kind() == DeclKind::Field).last()
            == Some(d)
    }

    /// clang `getArrayIndexingBound`: the element count of the array
    /// `base` indexes (a vector, or a decayed array that is not
    /// flexible-array-member-like) and the indexed type.
    fn ub_array_bound(&mut self, base: ExprId) -> R<Option<(V, QualType)>> {
        let ast = self.ast;
        let bq = ast.exprs.get(base).ty();
        if let Some((_, n)) = self.vec_of(bq) {
            return Ok(Some((self.icn(LTy::I32, i128::from(n)), bq)));
        }
        let b = self.skip_parens(base);
        if !(ast.exprs.get(b).kind().is_cast()
            && ast.exprs.cast_kind(b) == CastKind::ArrayToPointerDecay)
        {
            // clang `LoadPassedObjectSize`: a `pass_object_size` parameter
            // (WP-871 follow-up).
            return Ok(None);
        }
        let sub = ast.exprs.operand(b);
        if self.ub_fam_like(sub) {
            return Ok(None);
        }
        let aq = ast.exprs.get(sub).ty();
        Ok(match ast.types.canonical_view(aq) {
            TypeView::ConstantArray(a) => Some((self.i64c(i128::from(a.size)), aq)),
            TypeView::VariableArray { .. } => Some((self.vla_size(aq)?.0, aq)),
            _ => None,
        })
    }

    /// clang `EmitBoundsCheck`: `-fsanitize=array-bounds` on the index
    /// `idx` (of type `iq`) into `base` at `e`; an `accessed` element must
    /// be in bounds, any other index may also be one past the end.
    pub(crate) fn ub_bounds(
        &mut self,
        e: ExprId,
        base: ExprId,
        idx: V,
        iq: QualType,
        accessed: bool,
    ) -> R<()> {
        if !self.ub_on(San::ARRAY_BOUNDS) {
            return Ok(());
        }
        let outer = self.ub_begin();
        let res = (|| {
            let Some((bound, indexed)) = self.ub_array_bound(base)? else {
                return Ok(());
            };
            // clang 19 compares in `size_t` and reports the raw index;
            // clang 21+ (`EmitBoundsCheckImpl`) compares in the wider of
            // the index type and `size_t` and reports the converted index.
            let it = self.f.fir.ty(&self.m, idx);
            let wide = self.opts.clang_compat >= 22
                && self.m.types.int_width(it).is_some_and(|w| w > 64);
            let ct = if wide { it } else { LTy::I64 };
            let iv = self.int_cast(idx, ct, self.is_signed(iq));
            let bv = self.int_cast(bound, ct, false);
            let arg = if self.opts.clang_compat >= 22 { iv } else { idx };
            let loc = self.ub_loc(self.ub_expr_loc(e))?;
            let d1 = self.ub_type(indexed)?;
            let d2 = self.ub_type(iq)?;
            let p = if accessed { IntPredicate::Ult } else { IntPredicate::Ule };
            let ok = self.icmp(p, iv, bv);
            self.ub_check(&[(ok, San::ARRAY_BOUNDS)], UbHandler::OutOfBounds, &[loc, d1, d2], &[arg])
        })();
        self.ub_end(outer);
        res
    }

    // ---- pointer-overflow ------------------------------------------------

    /// clang `EmitCheckedInBoundsGEP`: [`Cg::gep_arith`], then (under
    /// `-fsanitize=pointer-overflow`) the check that the address
    /// arithmetic did not wrap. `signed` is whether the indices are of
    /// signed type, `sub` whether the GEP subtracts (`p - n`, `--p`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn gep_checked(
        &mut self,
        src: LTy,
        base: V,
        idx: Vec<V>,
        nuw: bool,
        signed: bool,
        sub: bool,
        loc: Option<Loc>,
    ) -> R<V> {
        let check = !self.opts.wrapv_pointer && self.ub_on(San::POINTER_OVERFLOW);
        let indices = if check { idx.clone() } else { Vec::new() };
        let g = self.gep_arith(src, base, idx, nuw);
        if check {
            self.ub_pointer_overflow(src, base, g, &indices, signed, sub, loc)?;
        }
        Ok(g)
    }

    /// `l + r` or `l - r` on intptr values as `IRBuilder` folds them: a
    /// constant expression when both are constants.
    fn ub_ptr_bin(&mut self, op: BinOp, l: V, r: V) -> R<V> {
        if let (V::C(a), V::C(b)) = (l, r)
            && (self.cint(l).is_none() || self.cint(r).is_none())
        {
            return match self.m.const_binop(op, Flags::NONE, a, b) {
                Ok(c) => Ok(V::C(c)),
                Err(e) => err(None, format!("bad pointer-overflow offset: {e:?}")),
            };
        }
        Ok(self.bin(op, Flags::NONE, l, r))
    }

    /// clang `EmitGEPOffsetInBytes`'s checked `eval`: `l * r` or `l + r`
    /// in `i64` with `llvm.s{mul,add}.with.overflow`, folded when both are
    /// constants; `ovf` accumulates the overflow bit.
    fn ub_offset_op(&mut self, mul: bool, l: V, r: V, ovf: &mut V) -> V {
        if let (Some((_, a, _)), Some((_, b, _))) = (self.cint(l), self.cint(r)) {
            let (a, b) = (a as i64, b as i64);
            let (x, o) = if mul { a.overflowing_mul(b) } else { a.overflowing_add(b) };
            if o {
                *ovf = V::C(self.m.const_bool(true));
            }
            return self.i64c(i128::from(x));
        }
        let oop = if mul { OverflowOp::SMul } else { OverflowOp::SAdd };
        let f = kiln_llvm::intrinsics::with_overflow(&mut self.m, oop, LTy::I64);
        let rty = self.m.types.struct_literal(&[LTy::I64, LTy::I1], false);
        let pair = self.intrinsic_call_ty(f, vec![l, r], rty);
        let o = self.f.fir.push(Op::ExtractValue { agg: pair, idx: 1 }, LTy::I1);
        *ovf = self.bin(BinOp::Or, Flags::NONE, o, *ovf);
        self.f.fir.push(Op::ExtractValue { agg: pair, idx: 0 }, LTy::I64)
    }

    /// The check half of clang `EmitCheckedInBoundsGEP` for the GEP `g` of
    /// `base` by `idx` over `src`.
    #[allow(clippy::too_many_arguments)]
    fn ub_pointer_overflow(
        &mut self,
        src: LTy,
        base: V,
        g: V,
        idx: &[V],
        signed: bool,
        sub: bool,
        loc: Option<Loc>,
    ) -> R<()> {
        let null_check = !self.opts.null_pointer_is_valid;
        let overflow_check = !matches!(g, V::C(_));
        if !null_check && !overflow_check {
            return Ok(());
        }
        let outer = self.ub_begin();
        let res = (|| {
            let fls = V::C(self.m.const_bool(false));
            let mut ovf = fls;
            // `EmitGEPOffsetInBytes`.
            let total = if matches!(g, V::C(_)) {
                let gi = self.cast(CastOp::PtrToInt, g, LTy::I64);
                let bi = self.cast(CastOp::PtrToInt, base, LTy::I64);
                self.ub_ptr_bin(BinOp::Sub, gi, bi)?
            } else {
                // clang declares both intrinsics up front, used or not.
                kiln_llvm::intrinsics::with_overflow(&mut self.m, OverflowOp::SAdd, LTy::I64);
                kiln_llvm::intrinsics::with_overflow(&mut self.m, OverflowOp::SMul, LTy::I64);
                let mut total: Option<V> = None;
                let mut ty = src;
                for (k, &i) in idx.iter().enumerate() {
                    if k > 0 {
                        let Some((e, _)) = self.m.types.array_info(ty) else {
                            return err(loc, "pointer-overflow check of a non-array GEP");
                        };
                        ty = e;
                    }
                    let size = self.i64c(i128::from(self.alloc_size(ty)));
                    let is = self.int_cast(i, LTy::I64, true);
                    let local = self.ub_offset_op(true, size, is, &mut ovf);
                    total = Some(match total {
                        Some(t) if self.cint(t).is_none_or(|c| c.1 != 0) => {
                            self.ub_offset_op(false, t, local, &mut ovf)
                        }
                        _ => local,
                    });
                }
                total.unwrap_or_else(|| self.i64c(0))
            };
            if self.cint(total).is_some_and(|c| c.1 == 0) {
                return Ok(());
            }
            let int_ptr = self.cast(CastOp::PtrToInt, base, LTy::I64);
            let computed = self.ub_ptr_bin(BinOp::Add, int_ptr, total)?;
            let mut checks = Vec::with_capacity(2);
            if null_check {
                let base_nn = self.ub_is_not_null(base);
                let z = self.i64c(0);
                let res_nn = self.icmp(IntPredicate::Ne, computed, z);
                checks.push((self.icmp(IntPredicate::Eq, base_nn, res_nn), San::POINTER_OVERFLOW));
            }
            if overflow_check {
                let t = V::C(self.m.const_bool(true));
                let no_ovf = self.bin(BinOp::Xor, Flags::NONE, ovf, t);
                let valid = if signed {
                    let pos_valid = self.icmp(IntPredicate::Uge, computed, int_ptr);
                    let z = self.i64c(0);
                    let pos = self.icmp(IntPredicate::Sge, total, z);
                    let neg_valid = self.icmp(IntPredicate::Ult, computed, int_ptr);
                    // `IRBuilder::CreateSelect` folds only all-constant
                    // operands.
                    match (self.cint(pos), self.cint(pos_valid), self.cint(neg_valid)) {
                        (Some(c), Some(_), Some(_)) => {
                            if c.1 != 0 { pos_valid } else { neg_valid }
                        }
                        _ => self.f.fir.push(
                            Op::Select { c: pos, t: pos_valid, f: neg_valid },
                            LTy::I1,
                        ),
                    }
                } else if !sub {
                    self.icmp(IntPredicate::Uge, computed, int_ptr)
                } else {
                    self.icmp(IntPredicate::Ule, computed, int_ptr)
                };
                checks.push((self.ub_and(valid, no_ovf), San::POINTER_OVERFLOW));
            }
            let l = self.ub_loc(loc)?;
            self.ub_check(&checks, UbHandler::PointerOverflow, &[l], &[int_ptr, computed])
        })();
        self.ub_end(outer);
        res
    }

    // ---- type checks (null, alignment, object-size) ---------------------

    /// clang `sanitizePerformTypeCheck`.
    pub(crate) fn ub_type_check_on(&self) -> bool {
        self.ub_on(San::NULL.union(San::ALIGNMENT).union(San::OBJECT_SIZE))
    }

    /// The alignment half of clang `EmitBuiltinExpr`'s `EmitArgCheck` for
    /// the `memcpy` family: the argument's pointer type (after an implicit
    /// bit cast) is the checked type, as clang 19 passes it.
    pub(crate) fn ub_builtin_arg_check(
        &mut self,
        tck: Tck,
        v: V,
        align: u64,
        arg: ExprId,
    ) -> R<()> {
        if !self.ub_on(San::ALIGNMENT) {
            return Ok(());
        }
        let ast = self.ast;
        let loc = self.ub_expr_loc(arg);
        let mut a = arg;
        if ast.exprs.get(a).kind() == ExprKind::ImplicitCastExpr
            && ast.exprs.cast_kind(a) == CastKind::BitCast
        {
            a = ast.exprs.operand(a);
        }
        let q = ast.exprs.get(a).ty();
        let tc = TypeCheck::new(tck, loc, q).align(align).skip(San::ALL.without(San::ALIGNMENT));
        self.ub_type_check(v, tc)
    }

    /// Runs `f` with its aggregate slot marked as already checked (clang
    /// `AggValueSlot::IsSanitizerChecked`: member and delegating
    /// initializers, new-expression initializers).
    pub(crate) fn ub_checked<T>(&mut self, f: impl FnOnce(&mut Self) -> R<T>) -> R<T> {
        if !self.ub_type_check_on() {
            return f(self);
        }
        self.f.ub_checked = true;
        let r = f(self);
        self.f.ub_checked = false;
        r
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

    /// The alignment of `p` when it is a global variable itself (LLVM
    /// `GlobalValue::getPointerAlignment`), else 1.
    fn ub_global_align(&self, p: V) -> u64 {
        use kiln_llvm::ValueRef;
        match p {
            V::C(c) => match c.get() {
                ValueRef::Global(g) => self.m.global(g).align.unwrap_or(1),
                _ => 1,
            },
            _ => 1,
        }
    }

    /// LLVM `stripPointerCasts` then `dyn_cast<AllocaInst>`: the alignment
    /// of the `alloca` `p` is, through all-zero-index GEPs.
    fn ub_alloca_align(&self, p: V) -> Option<u64> {
        let mut p = p;
        loop {
            let V::I(i) = p else { return None };
            match self.f.fir.op(i) {
                Op::Alloca { align, .. } | Op::DynAlloca { align, .. } => return Some(*align),
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
    /// alignment checks `tc` of the access to `ptr`.
    pub(crate) fn ub_type_check(&mut self, ptr: V, tc: TypeCheck) -> R<()> {
        if !self.ub_type_check_on() || self.is_volatile(tc.q) {
            return Ok(());
        }
        let outer = self.ub_begin();
        let r = self.ub_type_check_in(ptr, tc);
        self.ub_end(outer);
        r
    }

    fn ub_type_check_in(&mut self, ptr: V, tc: TypeCheck) -> R<()> {
        let TypeCheck { tck, loc, q, desc, align, skip, array_size } = tc;
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
                // LLVM's constant folder: `and (ptrtoint @g), m` is zero
                // when `@g`'s alignment clears the bits of `m`.
                let low = if self.ub_global_align(ptr) >= a {
                    self.icn(LTy::I64, 0)
                } else {
                    self.bin(BinOp::And, Flags::NONE, pi, m)
                };
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
            let desc = self.ub_type_keyed(q, desc)?;
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
        if self.ast.exprs.get(e).kind() == ExprKind::ArraySubscriptExpr {
            self.f.ub_accessed = Some(e);
        }
        let lv = self.emit_lvalue(e);
        self.f.ub_accessed = None;
        let lv = lv?;
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
        let tc = TypeCheck::new(tck, self.ub_expr_loc(e), n.ty()).align(lv.addr.align).skip(skip);
        self.ub_type_check(lv.addr.v, tc)
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

    /// clang `EmitReferenceBindingToExpr`'s `TCK_ReferenceBinding` check
    /// of the glvalue `e` bound at `v` ([dcl.ref]/5, CWG453: it designates
    /// an object of suitable size and alignment).
    pub(crate) fn ub_ref_binding(&mut self, e: ExprId, v: V) -> R<()> {
        let q = self.ast.exprs.get(e).ty();
        if !self.ub_type_check_on() || self.ast.types.is_function(q) {
            return Ok(());
        }
        let loc = self.ub_expr_loc(e);
        self.ub_type_check(v, TypeCheck::new(Tck::ReferenceBinding, loc, q))
    }

    /// clang `StartFunction`'s check of `this` (`CXXABIThisValue`, the
    /// `this` loaded in member function `d`'s prologue, variant
    /// `structor`): `TCK_ConstructorCall` in a constructor, else
    /// `TCK_MemberCall`, of the type `MD->getThisType()`, aligned
    /// `CXXABIThisAlignment`, never object-size; never null either in
    /// the call operator of a captureless lambda (its static invoker
    /// calls it with a null `this`).
    pub(crate) fn ub_this_check(&mut self, d: DeclId, structor: Option<Structor>, this: V) -> R<()> {
        let ast = self.ast;
        if !self.ub_type_check_on() {
            return Ok(());
        }
        let Some(rd) = ast.decls.get(d).ctx() else { return Ok(()) };
        let kind = ast.decls.get(d).kind();
        let mut skip = San::OBJECT_SIZE;
        if let Some(l) = ast.call_op_lambda(d) {
            let data = ast.exprs.lambdas.get(l);
            if data.default == CaptureDefault::None && data.captures.is_empty() {
                skip = skip.union(San::NULL);
            }
        }
        // clang `CGCXXABI::buildThisParam`: the class's alignment when it
        // has no virtual bases, is effectively final or `this` is a
        // complete object, else its non-virtual alignment.
        let def = ast.definition(rd).unwrap_or(rd);
        let complete = matches!(structor, Some(Structor::Complete | Structor::Deleting));
        let align = if self.class_incomplete_now(def) {
            1
        } else {
            let full = !self.class_bits(def).contains(DefinitionBits::HAS_VIRTUAL_BASES)
                || self.class_is_final(def)
                || self.dtor_is_final(def)
                || complete;
            let l = self.tys.layouter().record(def);
            u64::from(if full { l.align } else { l.nv_align })
        };
        let cv = match ast.decls.get(d).ty().map(|q| ast.types.view(q.ty())) {
            Some(TypeView::Function(fp)) => fp.info().this_quals().bits(),
            _ => 0,
        };
        let tck = if kind == DeclKind::CXXConstructor { Tck::ConstructorCall } else { Tck::MemberCall };
        let loc = Some(ast.decls.get(d).loc());
        let cq = self.class_type(rd);
        let tc = TypeCheck::new(tck, loc, cq).align(align).skip(skip).desc(UbDesc::ThisPtr(cv));
        self.ub_type_check(this, tc)
    }
}

/// An IEEE-style binary floating-point format: precision `p` (with the
/// integer bit), maximum exponent `emax` (also the bias), exponent field
/// width, and whether the integer bit is stored (`x86_fp80`).
#[derive(Clone, Copy)]
struct FpFormat {
    p: u32,
    emax: u32,
    ebits: u32,
    explicit: bool,
}

fn fp_format(types: &kiln_llvm::TypeTable, t: LTy) -> Option<FpFormat> {
    use kiln_llvm::TypeData as T;
    let (p, emax, ebits, explicit) = match types.data(t) {
        T::Half => (11, 15, 5, false),
        T::BFloat => (8, 127, 8, false),
        T::Float => (24, 127, 8, false),
        T::Double => (53, 1023, 11, false),
        T::X86Fp80 => (64, 16383, 15, true),
        T::Fp128 => (113, 16383, 15, false),
        _ => return None,
    };
    Some(FpFormat { p, emax, ebits, explicit })
}

impl FpFormat {
    /// The encoding of `±m·2^(e-p+1)` (`m` a `p_src`-bit significand with
    /// its top bit set, exactly representable here), or of `±inf`.
    fn encode(self, neg: bool, v: Option<(u128, u32)>, p_src: u32) -> u128 {
        let frac = if self.explicit { self.p } else { self.p - 1 };
        let sign = u128::from(neg) << (frac + self.ebits);
        let (biased, m) = match v {
            None => ((1u128 << self.ebits) - 1, if self.explicit { 1u128 << (self.p - 1) } else { 0 }),
            Some((m, e)) => (u128::from(e + self.emax), m << (self.p - p_src)),
        };
        let m = if self.explicit { m } else { m & ((1u128 << frac) - 1) };
        sign | (biased << frac) | m
    }
}
