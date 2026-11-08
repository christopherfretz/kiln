//! Vector types (spec/types.md §1 `Vector` / `DependentVector`, spec/sema.md
//! §5.8): clang's `ext_vector_type(N)` and GNU `vector_size(N)` attributes
//! on a typedef or alias-declaration (libc++ `__algorithm/simd_utils.h`
//! `using __simd_vector __attribute__((__ext_vector_type__(_Np))) = _Tp;`).
//!
//! Both attributes are consumed into the declared type and never attached
//! to the declaration (clang's AST keeps only the type). A value-dependent
//! size, or a dependent element of a `vector_size` vector, gives a
//! `DependentVector` whose operand is the attribute's expression as
//! written (the element count for `ext_vector_type`, the byte size for
//! `vector_size`, as clang's `DependentVectorType` /
//! `DependentSizedExtVectorType`); substitution rebuilds it through
//! [`Sema::build_vector_type`]. Among the decl-specifiers the attributes
//! modify the decl-specifier type (`__attribute__((vector_size(8))) short
//! f();` returns a vector); after a declarator they modify the whole
//! declared type, and an invalid one leaves the type unchanged (clang
//! `processTypeAttrs`, `HandleVectorSizeAttr`). Operations on vector
//! values are in `vector_ops.rs` (WP-156); their constant evaluation is M4.

use std::ops::ControlFlow;

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::ids::RawId;
use kiln_ast::types::{FloatFormat, Quals, TypeView, VectorKind};
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;
use crate::conv;
use crate::decl::AttrArg;
use crate::declspec::DeclSpec;

/// The kinds of a GNU `mode` ([`AttrKind::MODE`] `a >> 8 & 7`): an
/// integer mode, a floating-point one by width, `KF` (IEEE quad) and `IF`
/// (IBM double-double).
const MODE_INT: u32 = 0;
const MODE_FLOAT: u32 = 1;
const MODE_FLOAT128: u32 = 2;
const MODE_IBM128: u32 = 3;

/// Where the attributes given to [`Sema::apply_vector_attrs`] were
/// written: among a type's specifiers, or after a declarator (where a
/// `[[clang::]]` NEON vector attribute appertains to the declaration,
/// not the type: clang slides it onto the declared type).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VectorAttrPos {
    /// Among the decl-specifiers; those before the `Loc`, when given, are
    /// a leading attribute-specifier-seq, which appertains to the
    /// declaration ([dcl.pre]/5): as for `Decl`.
    Type(Option<Loc>),
    /// A variable, parameter, field or alias-declaration: clang's
    /// `warn_type_attribute_deprecated_on_decl`.
    Decl,
    /// A typedef: clang's `err_attribute_invalid_on_decl`.
    Typedef,
}

impl Sema<'_, '_> {
    /// `ext_vector_type(N)` / `vector_size(N)` as written: exactly one
    /// expression argument, kept unevaluated until the attribute is
    /// applied to its declaration.
    pub(crate) fn vector_attribute(
        &mut self,
        kind: AttrKind,
        syntax: AttrSyntax,
        raw: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let n = kiln_ast::attr::normalize_attr_name(raw).to_owned();
        let e = match args {
            [AttrArg::Expr(e)] => *e,
            [AttrArg::Type(_)] => {
                self.error(DiagId::parse_expected_expression, loc);
                return None;
            }
            _ => {
                // clang 22 checks `ext_vector_type`'s arguments as a
                // declaration attribute and again as a type attribute:
                // the error twice (spec/sema.md §1.1).
                let times = if kind == AttrKind::EXT_VECTOR_TYPE && self.clang_compat >= 22 {
                    2
                } else {
                    1
                };
                for _ in 0..times {
                    let msg = self
                        .diag(DiagId::parse_attribute_wrong_arg_count, loc)
                        .arg(n.clone())
                        .arg("1");
                    self.emit(if times == 2 { msg.allow_duplicate() } else { msg });
                }
                return None;
            }
        };
        let node = AttrNode { kind, syntax, flags: 0, loc, a: e.raw(), b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// The attributes `act_on_attribute` hands off whole: the vector
    /// attributes and the GNU allocation attributes (`Break` with the
    /// result); `Continue` for any other kind.
    pub(crate) fn attribute_with_args(
        &mut self,
        kind: AttrKind,
        syntax: AttrSyntax,
        raw: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> ControlFlow<Option<AttrId>> {
        match kind {
            AttrKind::MODE => ControlFlow::Break(self.mode_attribute(syntax, args, loc)),
            AttrKind::ARM_SVE_VECTOR_BITS => {
                ControlFlow::Break(self.sve_bits_attribute(syntax, raw, args, loc))
            }
            k if k.is_vector_type() => {
                ControlFlow::Break(self.vector_attribute(kind, syntax, raw, args, loc))
            }
            AttrKind::ALLOC_SIZE | AttrKind::ALLOC_ALIGN => {
                ControlFlow::Break(self.alloc_attribute(kind, syntax, args, loc))
            }
            AttrKind::NONNULL => ControlFlow::Break(self.nonnull_attribute(syntax, args, loc)),
            AttrKind::FORMAT => ControlFlow::Break(self.format_attribute(syntax, args, loc)),
            AttrKind::CONSTRUCTOR | AttrKind::DESTRUCTOR => {
                ControlFlow::Break(Some(self.ctor_dtor_attribute(kind, syntax, args, loc)))
            }
            AttrKind::CLEANUP => {
                ControlFlow::Break(Some(self.cleanup_attribute(syntax, args, loc)))
            }
            _ => ControlFlow::Continue(()),
        }
    }

    /// GNU `mode(M)` as written (clang `handleModeAttr` /
    /// `parseModeAttrArg`; spec/sema.md "The `mode` attribute"): one
    /// identifier, `__M__` normalized to `M`. `QI`/`HI`/`SI`/`DI`/`XI`/`TI`
    /// are 8/16/32/64/96/128-bit integers; `HF`/`SF`/`DF`/`XF`/`TF`
    /// floating-point of 16/32/64/96/128 bits, `KF` the IEEE quad type and
    /// `IF` the IBM double-double one; the `C` forms (`SC`, `DC`, `XC`,
    /// `TC`, ..) their complex types; `byte` 8, `word`/`pointer`/
    /// `unwind_word` the pointer width. `V<N>M` (N a power of two) is a
    /// vector of N elements of mode M, deprecated with a warning. Any other
    /// name is an unknown mode. Whether the target has the type is decided
    /// by [`Self::apply_mode`]. Encoded as for [`AttrKind::MODE`].
    fn mode_attribute(&mut self, syntax: AttrSyntax, args: &[AttrArg], loc: Loc) -> Option<AttrId> {
        let name = "'mode'";
        if args.len() != 1 {
            let msg =
                self.diag(DiagId::sema_attribute_wrong_number_arguments, loc).arg(name).arg("1");
            self.emit(msg);
            return None;
        }
        let AttrArg::Ident(sym, _) = args[0] else {
            let msg = self.diag(DiagId::sema_attribute_param_not_ident, loc).arg(name).arg("1");
            self.emit(msg);
            return None;
        };
        let written = self.ast.symbols().as_str(sym).to_owned();
        let m = match written.strip_prefix("__").and_then(|w| w.strip_suffix("__")) {
            Some(w) if written.len() >= 4 => w,
            _ => written.as_str(),
        };
        // `V<N>`: the digits after `V`, a power of two (clang ignores the
        // prefix otherwise, so `V3SI` is an unknown mode). kiln keeps N up
        // to 2^16.
        let digits = m.bytes().skip(1).take_while(u8::is_ascii_digit).count();
        let lanes = (m.len() >= 4 && m.starts_with('V') && digits > 0)
            .then(|| m[1..=digits].parse::<u32>().ok())
            .flatten()
            .filter(|n| n.is_power_of_two() && *n <= 1 << 16);
        let base = if lanes.is_some() { &m[digits + 1..] } else { m };
        if lanes.is_some() {
            self.error(DiagId::sema_vector_mode_deprecated, loc);
        }
        let b = base.as_bytes();
        let word = u32::from(self.target.pointer_bytes) * 8;
        // (width, MODE_FLOAT.. kind, complex)
        let (width, kind, complex) = match (base, b.len()) {
            ("byte", _) => (8, MODE_INT, false),
            ("word" | "pointer" | "unwind_word", _) => (word, MODE_INT, false),
            (_, 2) => {
                let (w, kind) = match b[0] {
                    b'Q' => (8, MODE_FLOAT),
                    b'H' => (16, MODE_FLOAT),
                    b'S' => (32, MODE_FLOAT),
                    b'D' => (64, MODE_FLOAT),
                    b'X' => (96, MODE_FLOAT),
                    b'T' => (128, MODE_FLOAT),
                    b'K' => (128, MODE_FLOAT128),
                    b'I' => (128, MODE_IBM128),
                    _ => (0, MODE_INT),
                };
                match b[1] {
                    b'I' if kind == MODE_FLOAT => (w, MODE_INT, false),
                    b'F' => (w, kind, false),
                    b'C' => (w, kind, true),
                    _ => (0, MODE_INT, false),
                }
            }
            _ => (0, MODE_INT, false),
        };
        if width == 0 {
            self.machine_mode_error(loc, true, m);
            return None;
        }
        let log2 = lanes.map_or(0, |n| n.trailing_zeros() + 1);
        let node = AttrNode {
            kind: AttrKind::MODE,
            syntax,
            flags: 0,
            loc,
            a: width | kind << 8 | u32::from(complex) << 11 | log2 << 12,
            b: sym.index(),
        };
        Some(self.ast.attrs.push(node))
    }

    /// "unknown/unsupported machine mode 'M'" (clang `err_machine_mode`).
    fn machine_mode_error(&mut self, loc: Loc, unknown: bool, m: &str) {
        let msg = self
            .diag(DiagId::sema_machine_mode, loc)
            .arg(if unknown { "0" } else { "1" })
            .arg(format!("'{m}'"));
        self.emit(msg);
    }

    /// The real floating-point type of a `width`-bit mode of `kind` (clang
    /// `TargetInfo::getRealTypeByWidth`): 16/32/64 bits `__fp16`, `float`,
    /// `double`; 96 bits the x87 `long double`; 128 bits `__float128` for
    /// `KF`, none for `IF`, otherwise an IEEE quad `long double` or else
    /// `__float128` where the target has it (x86-64).
    fn mode_real_type(&self, width: u32, kind: u32) -> Option<QualType> {
        let x87 = self.target.long_double == FloatFormat::X87Extended;
        let quad = self.target.long_double == FloatFormat::IeeeQuad;
        match (width, kind) {
            (16, _) => Some(QualType::HALF),
            (32, _) => Some(QualType::FLOAT),
            (64, _) => Some(QualType::DOUBLE),
            (96, _) if x87 => Some(QualType::LONGDOUBLE),
            (128, MODE_FLOAT128) => x87.then_some(QualType::FLOAT128),
            (128, MODE_FLOAT) if quad => Some(QualType::LONGDOUBLE),
            (128, MODE_FLOAT) if x87 => Some(QualType::FLOAT128),
            _ => None,
        }
    }

    /// The underlying type an enumeration gets from its `mode` attribute
    /// `node`, `old` being the type it is applied to (the fixed type, else
    /// `int`); `None` (diagnosed) for a mode other than an integer one.
    pub(crate) fn enum_mode_type(&mut self, old: QualType, node: AttrNode) -> Option<QualType> {
        if node.a >> 12 & 0x1f != 0 {
            let m = self.ast.symbols().as_str(kiln_source::symbol::Symbol::from_index(node.b));
            let m = format!("'{}'", m.trim_start_matches("__").trim_end_matches("__"));
            let msg = self.diag(DiagId::sema_mode_enum_vector, node.loc).arg(m);
            self.emit(msg);
            return None;
        }
        let t = self.apply_mode(old, node);
        conv::rank::int_width(self, t).is_some().then_some(t)
    }

    /// `ty` with the `mode` attribute `node` applied (clang
    /// `Sema::AddModeAttr`), on `ty` or, for a vector `ty`, its element
    /// type: an integral or enumeration type becomes the integer type of
    /// the mode's width with its signedness; a floating-point or complex
    /// floating-point type the mode's floating-point type, or for a complex
    /// mode its complex type. A vector mode makes a GNU vector of that
    /// type; a scalar mode on a vector type keeps its size and kind. All
    /// without cv-qualifiers (clang sets the declaration's type to the
    /// unqualified mode type). A complex integer type is not primitive.
    /// As in clang, a type that does not match the mode is diagnosed and
    /// the mode's type still installed. A dependent type is left as it is.
    pub(crate) fn apply_mode(&mut self, ty: QualType, node: AttrNode) -> QualType {
        if self.is_error_type(ty) || self.ast.types.is_dependent(ty) {
            return ty;
        }
        let (width, kind) = (node.a & 0xff, node.a >> 8 & 7);
        let (complex, log2) = (node.a >> 11 & 1 == 1, node.a >> 12 & 0x1f);
        let old_vec = match self.ast.types.canonical_view(ty) {
            TypeView::Vector { elem, count, kind } => Some((elem, count, kind)),
            _ => None,
        };
        let elem = old_vec.map_or(ty, |v| v.0);
        let name = || {
            let m = self.ast.symbols().as_str(kiln_source::symbol::Symbol::from_index(node.b));
            m.trim_start_matches("__").trim_end_matches("__").to_owned()
        };
        if log2 != 0 && self.ast.types.as_enum(elem).is_some() {
            let msg =
                self.diag(DiagId::sema_mode_enum_vector, node.loc).arg(format!("'{}'", name()));
            self.emit(msg);
            return ty;
        }
        let integral = conv::rank::int_width(self, elem).is_some()
            && !matches!(self.ast.types.canonical_view(elem), TypeView::BitInt { .. });
        let is_complex =
            self.ast.types.as_complex(elem).is_some_and(|e| self.ast.types.is_floating(e));
        let floating = self.ast.types.is_floating(elem) || is_complex;
        let wrong = if kind == MODE_INT {
            !integral
        } else if complex {
            !is_complex
        } else {
            !floating
        };
        if !integral && !floating {
            self.error(DiagId::sema_mode_not_primitive, node.loc);
        } else if wrong {
            self.error(DiagId::sema_mode_wrong_type, node.loc);
        }
        let new = if kind == MODE_INT {
            let signed = integral && conv::rank::int_signed(self, elem);
            match (width, signed) {
                (8, true) => Some(QualType::SCHAR),
                (8, false) => Some(QualType::UCHAR),
                (16, true) => Some(QualType::SHORT),
                (16, false) => Some(QualType::USHORT),
                (32, true) => Some(QualType::INT),
                (32, false) => Some(QualType::UINT),
                (64, true) => Some(QualType::LONG),
                (64, false) => Some(QualType::ULONG),
                (128, true) => Some(QualType::INT128),
                (128, false) => Some(QualType::UINT128),
                _ => None,
            }
        } else {
            self.mode_real_type(width, kind)
        };
        let Some(mut new) = new else {
            let m = name();
            self.machine_mode_error(node.loc, false, &m);
            return ty;
        };
        if complex {
            new = self.ast.types.complex(new);
        }
        if log2 != 0 {
            return self.ast.types.vector(new, 1 << (log2 - 1), VectorKind::Generic);
        }
        let Some((old_elem, count, vkind)) = old_vec else { return new };
        if complex {
            self.error(DiagId::sema_mode_complex_vector_base, node.loc);
            return ty;
        }
        let size = |s: &Self, t| crate::target::size_align(&s.ast, t, s.abi()).map_or(0, |x| x.0);
        let (old_size, new_size) = (size(self, old_elem), size(self, new));
        if new_size == 0 {
            return ty;
        }
        let lanes = old_size * u64::from(count) / new_size;
        self.ast.types.vector(new, u32::try_from(lanes).unwrap_or(u32::MAX), vkind)
    }

    /// GNU `format(archetype, F, A)` as written (WP-186; clang
    /// `handleFormatAttr`): exactly three arguments, an identifier then
    /// two integer constants. The archetype (`__x__` normalized to `x`)
    /// is stored in `a`: [`FORMAT_PRINTF`] (checked at calls),
    /// [`FORMAT_STRFTIME`] or [`FORMAT_OTHER`] (a format clang supports
    /// that kiln does not check); an unknown one is ignored with a
    /// warning, and GCC's internal `gcc_*diag` silently. `b` = `F | A <<
    /// 16`, each saturated at `0xfffe` (a negative value is out of
    /// bounds); the indices are checked against the declaration in
    /// `check_decl_attrs`.
    pub(crate) fn format_attribute(
        &mut self,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let name = "'format'";
        if args.len() != 3 {
            let msg =
                self.diag(DiagId::sema_attribute_wrong_number_arguments, loc).arg(name).arg("3");
            self.emit(msg);
            return None;
        }
        let AttrArg::Ident(sym, _) = args[0] else {
            let msg = self.diag(DiagId::sema_attribute_param_not_ident, loc).arg(name).arg("1");
            self.emit(msg);
            return None;
        };
        let written = self.ast.symbols().as_str(sym).to_owned();
        let arch = match written.strip_prefix("__").and_then(|w| w.strip_suffix("__")) {
            Some(w) if written.len() >= 4 => w,
            _ => written.as_str(),
        };
        // clang 22 maps GCC's `gnu_printf` / `gnu_scanf` to the plain
        // archetypes; clang 19 does not know them.
        let gnu22 = self.clang_compat >= 22;
        let a = match arch {
            "printf" => FORMAT_PRINTF,
            "gnu_printf" if gnu22 => FORMAT_PRINTF,
            "strftime" => FORMAT_STRFTIME,
            "scanf" | "printf0" | "strfmon" | "cmn_err" | "vcmn_err" | "zcmn_err" | "kprintf"
            | "freebsd_kprintf" | "os_trace" | "os_log" | "NSString" | "CFString" => FORMAT_OTHER,
            "gnu_scanf" if gnu22 => FORMAT_OTHER,
            "gcc_diag" | "gcc_cdiag" | "gcc_cxxdiag" | "gcc_tdiag" => return None,
            _ => {
                let msg =
                    self.diag(DiagId::sema_attribute_arg_not_supported, loc).arg(name).arg(arch);
                self.emit(msg);
                return None;
            }
        };
        let mut idx = [0u32; 2];
        for (i, arg) in args[1..].iter().enumerate() {
            let v = match arg {
                AttrArg::Expr(e) => self.fold_int(*e),
                AttrArg::Type(_) | AttrArg::Ident(..) => None,
            };
            let Some(v) = v else {
                let msg = self
                    .diag(DiagId::sema_attribute_param_not_ice, loc)
                    .arg(name)
                    .arg((i + 2).to_string());
                self.emit(msg);
                return None;
            };
            idx[i] = u32::try_from(v).map_or(0xfffe, |v| v.min(0xfffe));
        }
        let node =
            AttrNode { kind: AttrKind::FORMAT, syntax, flags: 0, loc, a, b: idx[0] | idx[1] << 16 };
        Some(self.ast.attrs.push(node))
    }

    /// GNU `alloc_size(n[, m])` / `alloc_align(n)` as written: integer
    /// constants, each kept as an `IntegerLiteral` at its argument (zero
    /// for a negative value, which is out of bounds) and checked against
    /// the declaration in `check_decl_attrs` (clang `handleAllocSizeAttr`
    /// / `AddAllocAlignAttr`).
    pub(crate) fn alloc_attribute(
        &mut self,
        kind: AttrKind,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let size = kind == AttrKind::ALLOC_SIZE;
        let name = if size { "'alloc_size'" } else { "'alloc_align'" };
        if size && (args.is_empty() || args.len() > 2) {
            let msg =
                self.diag(DiagId::sema_alloc_size_arg_count, loc).arg(u32::from(!args.is_empty()));
            self.emit(msg);
            return None;
        }
        if !size && args.len() != 1 {
            let msg =
                self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg("alloc_align").arg("1");
            self.emit(msg);
            return None;
        }
        let mut idx = [0u32; 2];
        for (i, arg) in args.iter().enumerate() {
            let v = match arg {
                AttrArg::Expr(e) => self.fold_int(*e).map(|v| (v, self.ast.exprs.get(*e).loc())),
                AttrArg::Type(_) | AttrArg::Ident(..) => None,
            };
            let Some((v, at)) = v else {
                let msg = self
                    .diag(DiagId::sema_attribute_param_not_ice, loc)
                    .arg(name)
                    .arg((i + 1).to_string());
                self.emit(msg);
                return None;
            };
            let v = u64::try_from(v).unwrap_or(0);
            let ty = self.size_type();
            idx[i] = self.builder().int_literal(ty, v, kiln_ast::expr::Radix::Decimal, at).raw();
        }
        let node = AttrNode { kind, syntax, flags: 0, loc, a: idx[0], b: idx[1] };
        Some(self.ast.attrs.push(node))
    }

    /// Applies (and removes from `attrs`) the vector attributes of a
    /// declaration of type `ty`. `typedef` is true for a typedef or
    /// alias-declaration, the only declarations `ext_vector_type` applies
    /// to at clang 19 (clang 22 takes any); `vector_size` also applies to
    /// the type of a variable, field or parameter (on a function clang applies it to the return type: not
    /// supported yet). `pos` says where `attrs` were written. Returns the
    /// declared type.
    pub(crate) fn apply_vector_attrs(
        &mut self,
        ty: QualType,
        attrs: &mut Vec<AttrId>,
        typedef: bool,
        pos: VectorAttrPos,
    ) -> QualType {
        let mut ty = ty;
        let mut i = 0;
        while i < attrs.len() {
            let node = *self.ast.attrs.get(attrs[i]);
            if !node.kind.is_vector_type() {
                i += 1;
                continue;
            }
            // A `mode` on a dependent type stays on the declaration,
            // applied at instantiation ([`Self::apply_pattern_mode`]).
            if node.kind == AttrKind::MODE && self.ast.types.is_dependent(ty) {
                self.ast.attrs.set_flags(attrs[i], AttrNode::MODE_DEFERRED);
                i += 1;
                continue;
            }
            attrs.remove(i);
            if node.kind == AttrKind::MODE {
                ty = self.apply_mode(ty, node);
                continue;
            }
            if node.kind == AttrKind::ARM_SVE_VECTOR_BITS {
                if self.is_error_type(ty) {
                    continue;
                }
                let Some(bits) = ExprId::from_raw(node.a) else { continue };
                ty = self.build_sve_fixed_type(ty, bits, typedef, node.loc);
                continue;
            }
            if matches!(node.kind, AttrKind::NEON_VECTOR_TYPE | AttrKind::NEON_POLYVECTOR_TYPE) {
                if self.is_error_type(ty) {
                    continue;
                }
                let poly = node.kind == AttrKind::NEON_POLYVECTOR_TYPE;
                let Some(count) = ExprId::from_raw(node.a) else { continue };
                let shown = self.attr_shown(&node, neon_attr_name(poly));
                let v = self.build_neon_vector_type(ty, count, &shown, poly, node.loc);
                if v != QualType::ERROR {
                    ty = v;
                    let pos = match pos {
                        VectorAttrPos::Type(Some(end)) if node.loc < end => VectorAttrPos::Decl,
                        p => p,
                    };
                    if node.syntax == AttrSyntax::Cxx11 && !matches!(pos, VectorAttrPos::Type(_)) {
                        self.neon_attr_on_decl(node, pos);
                    }
                }
                continue;
            }
            let ext = node.kind == AttrKind::EXT_VECTOR_TYPE;
            // clang 22 applies `ext_vector_type` to any declared type
            // (spec/sema.md §1.1).
            if !typedef && ext && self.clang_compat < 22 {
                self.error(DiagId::sema_ext_vector_type_on_non_typedef, node.loc);
                continue;
            }
            if self.is_error_type(ty) {
                continue;
            }
            let kind = if ext { VectorKind::Ext } else { VectorKind::Generic };
            let Some(size) = ExprId::from_raw(node.a) else { continue };
            // clang `HandleVectorSizeAttr` / `HandleExtVectorTypeAttr`
            // leave the type unchanged after a diagnostic.
            let v = self.build_vector_type(ty, size, kind, node.loc);
            if v != QualType::ERROR {
                ty = v;
            }
        }
        ty
    }

    /// A `[[clang::neon_vector_type(N)]]` (or `neon_polyvector_type`)
    /// `node` after a declarator, applied to its type: an error on a
    /// typedef, deprecated elsewhere (clang `ProcessDeclAttributeList`'s
    /// legacy slide of a type attribute to the decl-specifiers).
    fn neon_attr_on_decl(&mut self, node: AttrNode, pos: VectorAttrPos) {
        let poly = node.kind == AttrKind::NEON_POLYVECTOR_TYPE;
        let shown = self.attr_shown(&node, neon_attr_name(poly));
        let id = if pos == VectorAttrPos::Typedef {
            DiagId::sema_stmt_attribute_on_decl
        } else {
            DiagId::sema_type_attribute_deprecated_on_decl
        };
        let msg = self.diag(id, node.loc).arg(shown);
        self.emit(msg);
    }

    /// `ty`, the substituted type of pattern declaration `d`, with the
    /// GNU `mode` kept on `d` for its dependent type applied (clang
    /// `instantiateDependentModeAttr`).
    pub(crate) fn apply_pattern_mode(&mut self, d: DeclId, ty: QualType) -> QualType {
        let ast = &self.ast;
        let modes: Vec<AttrNode> = ast
            .lists
            .iter::<AttrId>(ast.attrs.of_decl(d))
            .map(|a| *ast.attrs.get(a))
            .filter(|n| n.kind == AttrKind::MODE)
            .collect();
        modes.into_iter().fold(ty, |ty, node| self.apply_mode(ty, node))
    }

    /// `base` with the vector attributes among `ds`'s decl-specifiers
    /// applied: they modify the decl-specifier type, not the declarator's
    /// (clang `processTypeAttrs(TAL_DeclSpec)`); the declarator's own
    /// trailing attributes apply to the whole declared type.
    pub(crate) fn apply_declspec_vector_attrs(
        &mut self,
        base: QualType,
        ds: &DeclSpec,
    ) -> QualType {
        self.apply_declspec_vector_attrs_as(base, ds, ds.is_typedef(), false)
    }

    /// [`Self::apply_declspec_vector_attrs`] (`typedef` as for
    /// [`Self::apply_vector_attrs`]). clang `ConvertDeclSpecToType`
    /// processes the decl-specifier attributes before it applies the
    /// cv-qualifiers written among the decl-specifiers, so those qualify
    /// the vector, not its element: `__attribute__((vector_size(16)))
    /// double const *p` points to a `const` vector of two `double`
    /// (printed `__attribute__((__vector_size__(2 * sizeof(double))))
    /// double const *`), as in GCC.
    fn apply_declspec_vector_attrs_as(
        &mut self,
        base: QualType,
        ds: &DeclSpec,
        typedef: bool,
        type_id: bool,
    ) -> QualType {
        let mut attrs = ds.attrs.clone();
        if type_id {
            attrs.retain(|&a| self.ast.attrs.get(a).kind != AttrKind::MODE);
        }
        if !attrs.iter().any(|&a| self.ast.attrs.get(a).kind.is_vector_type()) {
            return base;
        }
        let cv = ds.cv.bits() & base.quals_bits();
        let elem = QualType::new(base.ty(), base.quals_bits() & !cv);
        let lead = ds.lead_cxx11.map(|_| ds.loc);
        let v = self.apply_vector_attrs(elem, &mut attrs, typedef, VectorAttrPos::Type(lead));
        if v == elem {
            return base;
        }
        self.ast.types.qualify(v, Quals::from_bits(cv))
    }

    /// `base` with the vector attributes among the decl-specifiers of a
    /// type-id applied (`sizeof(__attribute__((vector_size(16))) int)`,
    /// a cast, a template argument): clang `processTypeAttrs` handles
    /// them as type attributes, so `ext_vector_type` is accepted here at
    /// clang 19 too (there is no declaration to restrict it to a typedef).
    /// A GNU `mode` is a declaration attribute, so a type-id drops it
    /// (clang `ActOnTypeName`'s `checkUnusedDeclAttributes`), with a
    /// warning when `warn_mode` (not for a named cast or a new-expression,
    /// whose type clang builds without that check).
    pub fn apply_type_id_vector_attrs(
        &mut self,
        base: QualType,
        ds: &DeclSpec,
        warn_mode: bool,
    ) -> QualType {
        if warn_mode {
            self.warn_type_id_modes(&ds.attrs);
        }
        self.apply_declspec_vector_attrs_as(base, ds, true, true)
    }

    /// clang `warn_attribute_ignored_when_parsing_type` for each GNU
    /// `mode` among `attrs`, the attributes of a type-id.
    pub(crate) fn warn_type_id_modes(&mut self, attrs: &[AttrId]) {
        for &a in attrs {
            let node = *self.ast.attrs.get(a);
            if node.kind == AttrKind::MODE {
                let msg =
                    self.diag(DiagId::sema_attribute_ignored_parsing_type, node.loc).arg("'mode'");
                self.emit(msg);
            }
        }
    }

    /// `attrs` without the vector attributes (applied separately by
    /// [`Self::apply_declspec_vector_attrs`]), except a `mode` deferred to
    /// instantiation.
    pub(crate) fn non_vector_attrs(&self, attrs: &[AttrId]) -> Vec<AttrId> {
        let kept = |n: &AttrNode| {
            !n.kind.is_vector_type()
                || n.flags & AttrNode::MODE_DEFERRED != 0 && n.kind == AttrKind::MODE
        };
        attrs.iter().copied().filter(|&a| kept(self.ast.attrs.get(a))).collect()
    }

    /// The vector of `elem` sized by `size` (the element count for
    /// `VectorKind::Ext`, the byte size otherwise), clang's
    /// `BuildExtVectorType` / `BuildVectorType`; `ERROR` after a
    /// diagnostic.
    pub(crate) fn build_vector_type(
        &mut self,
        elem: QualType,
        size: ExprId,
        kind: VectorKind,
        loc: Loc,
    ) -> QualType {
        let ext = kind == VectorKind::Ext;
        let t = &self.ast.types;
        let elem_dep = t.is_dependent(elem);
        if !elem_dep {
            let ok = if ext {
                // An integer (`bool` included) or real floating type.
                t.is_integral(elem) || t.is_floating(elem)
            } else {
                !t.is_array(elem)
                    && t.builtin(elem).is_some()
                    && t.canonical(elem).ty() != QualType::BOOL.ty()
                    && (t.is_integral(elem) || t.is_floating(elem))
            };
            let already = matches!(t.canonical_view(elem), TypeView::Vector { .. });
            if !ok || already {
                let msg =
                    self.diag(DiagId::sema_vector_invalid_element_type, loc).arg(self.ty_arg(elem));
                self.emit(msg);
                return QualType::ERROR;
            }
        }
        if self.expr_is_value_dependent(size) || (!ext && elem_dep) {
            let p = self.dependent_profile(size);
            self.inst.vector_attr_locs.insert(size, loc);
            return self.ast.types.dependent_vector(elem, size, p, kind);
        }
        let name = if ext { "ext_vector_type" } else { "vector_size" };
        let Some(v) = self.vector_size_value(size, name, loc) else {
            return QualType::ERROR;
        };
        if v == 0 {
            self.error(DiagId::sema_vector_zero_size, loc);
            return QualType::ERROR;
        }
        let count = if ext {
            v
        } else {
            let esize = crate::target::size_align(&self.ast, elem, self.abi()).map_or(0, |s| s.0);
            if esize == 0 || v % u128::from(esize) != 0 {
                self.error(DiagId::sema_vector_size_not_multiple, loc);
                return QualType::ERROR;
            }
            v / u128::from(esize)
        };
        let Ok(count) = u32::try_from(count) else {
            self.error(DiagId::sema_vector_size_too_large, loc);
            return QualType::ERROR;
        };
        self.ast.types.vector(elem, count, kind)
    }

    /// `neon_vector_type(N)` / `neon_polyvector_type(N)` on `elem` (clang
    /// `HandleNeonVectorTypeAttr`, WP-558): `N` is an element count, an
    /// integral constant; the element must be a type the target's Neon
    /// vectors permit (`isPermittedNeonBaseType`, seen through typedefs:
    /// plain `char` and anything dependent are not); the vector must be
    /// 64 or 128 bits. `ERROR` after a diagnostic.
    fn build_neon_vector_type(
        &mut self,
        elem: QualType,
        count: ExprId,
        name: &str,
        poly: bool,
        loc: Loc,
    ) -> QualType {
        // clang quotes the NEON attributes' names here, not the others'.
        let name = &format!("'{name}'");
        if self.expr_is_value_dependent(count) {
            let msg = self.diag(DiagId::sema_attribute_requires_integer_constant, loc).arg(name);
            self.emit(msg);
            return QualType::ERROR;
        }
        let Some(n) = self.vector_size_value(count, name, loc) else {
            return QualType::ERROR;
        };
        let aarch64 = self.abi().triple.arch == kiln_abi::target::Arch::AArch64;
        let wide = self.abi().pointer_size == 8;
        let b = self.ast.types.builtin(elem);
        let ok = b.is_some_and(|b| {
            use kiln_ast::types::BuiltinType as B;
            if poly {
                if aarch64 {
                    matches!(b, B::UChar | B::UShort | B::ULong | B::ULongLong)
                } else {
                    matches!(b, B::SChar | B::Short | B::LongLong)
                }
            } else {
                matches!(
                    b,
                    B::SChar
                        | B::UChar
                        | B::Short
                        | B::UShort
                        | B::Int
                        | B::UInt
                        | B::Long
                        | B::ULong
                        | B::LongLong
                        | B::ULongLong
                        | B::Float
                        | B::Half
                        | B::BFloat16
                ) || (wide && b == B::Double)
            }
        });
        if !ok {
            let msg =
                self.diag(DiagId::sema_vector_invalid_element_type, loc).arg(self.ty_arg(elem));
            self.emit(msg);
            return QualType::ERROR;
        }
        let esize = crate::target::size_align(&self.ast, elem, self.abi()).map_or(0, |s| s.0);
        let bits = n.saturating_mul(u128::from(esize)).saturating_mul(8);
        let Ok(count) = u32::try_from(n) else {
            self.error(DiagId::sema_neon_vector_bad_size, loc);
            return QualType::ERROR;
        };
        if bits != 64 && bits != 128 {
            self.error(DiagId::sema_neon_vector_bad_size, loc);
            return QualType::ERROR;
        }
        let kind = if poly { VectorKind::NeonPoly } else { VectorKind::Neon };
        self.ast.types.vector(elem, count, kind)
    }

    /// The value of the size operand `e` of attribute `name`: an integral
    /// constant expression (clang `err_attribute_argument_type`).
    pub(crate) fn vector_size_value(&mut self, e: ExprId, name: &str, loc: Loc) -> Option<u128> {
        let ty = self.ast.exprs.get(e).ty();
        if self.is_error_type(ty) {
            return None;
        }
        if !self.is_integral_or_enum(ty) {
            let msg = self.diag(DiagId::sema_attribute_requires_integer_constant, loc).arg(name);
            self.emit(msg);
            return None;
        }
        let c = conv::lvalue_to_rvalue(self, e);
        let c = self.act_on_full_expr(c);
        match self.eval_int(c, EvalContext::new(EvalKind::Other, self.begin_loc(e))) {
            Ok(v) if v < 0 => {
                self.error(DiagId::sema_vector_size_too_large, loc);
                None
            }
            Ok(v) => u128::try_from(v).ok(),
            Err(_) => {
                let msg =
                    self.diag(DiagId::sema_attribute_requires_integer_constant, loc).arg(name);
                self.emit(msg);
                None
            }
        }
    }
}

/// [`AttrKind::FORMAT`]'s `a`: a `printf` format, checked at calls.
pub(crate) const FORMAT_PRINTF: u32 = 0;
/// [`AttrKind::FORMAT`]'s `a`: a format clang supports that kiln does not
/// check (`scanf`, `strfmon`, ...).
pub(crate) const FORMAT_OTHER: u32 = 1;
/// [`AttrKind::FORMAT`]'s `a`: `strftime` (no data arguments).
pub(crate) const FORMAT_STRFTIME: u32 = 2;

/// The name of the NEON vector attribute (`neon_polyvector_type` when
/// `poly`).
fn neon_attr_name(poly: bool) -> &'static str {
    if poly { "neon_polyvector_type" } else { "neon_vector_type" }
}
