//! Calling-convention attributes (clang `handleCallConvAttr`,
//! `handleFunctionTypeAttr`; spec/types.md §5.4, WP-808): `ms_abi`,
//! `preserve_most`, `regcall`, `aarch64_vector_pcs`, ... written on a
//! declaration are function type attributes. Sema parses each into an
//! [`AttrKind::CALLING_CONV`] node, then consumes it into the declared
//! type's [`FuncInfo`]: the convention takes part in type identity
//! (function pointer conversions, overloading, redeclaration), mangling
//! and the IR calling convention.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax, CC_ATTR_SPELLINGS};
use kiln_ast::types::{CallConv, FuncInfo, TypeView};
use kiln_ast::{AttrId, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::decl::AttrArg;

/// `a` of a convention the target does not support (clang
/// `warn_cconv_unsupported`, emitted once the subject is known to be a
/// function type).
const UNSUPPORTED: u32 = 0xff;

/// Whether spelling `idx` of [`CC_ATTR_SPELLINGS`] is a GCC one
/// (`__attribute__` and `[[gnu::]]`); the others are clang's own
/// (`__attribute__` and `[[clang::]]`), as clang 19's `Attr.td` spells
/// them.
const fn gcc_spelled(idx: usize) -> bool {
    matches!(idx, 0..=3 | 13..=15 | 17)
}

/// The convention spelling `idx` means on an x86-64 (`x86`) or `AArch64`
/// Linux target (clang `X86_64TargetInfo` / `AArch64TargetInfo`
/// `checkCallingConvention`): None when the target ignores it. A
/// convention that is the target's default (`sysv_abi`, `cdecl`) is
/// `Default`.
const fn target_cc(idx: usize, x86: bool) -> Option<CallConv> {
    Some(match idx {
        0 => CallConv::Win64,
        1 | 2 => CallConv::Default,
        3 if x86 => CallConv::RegCall,
        4 if x86 => CallConv::VectorCall,
        5 => CallConv::PreserveMost,
        6 => CallConv::PreserveAll,
        7 => CallConv::PreserveNone,
        8 if !x86 => CallConv::AArch64VectorCall,
        9 if !x86 => CallConv::AArch64SveCall,
        10 if x86 => CallConv::IntelOclBicc,
        11 => CallConv::Swift,
        12 => CallConv::SwiftAsync,
        _ => return None,
    })
}

/// The kind of calling-convention attribute `n` (its `__` affixes
/// removed) written with `syntax` and normalized scope `scope` on x86-64
/// (`x86`) or `AArch64`, for the attribute triage (WP-805): None for any
/// other name, a spelling clang does not give it (`[[clang::ms_abi]]`,
/// `[[gnu::preserve_most]]`), and for `swiftcall` / `swiftasynccall`,
/// which kiln does not lower (class A, `__has_attribute` 0). x86
/// `interrupt` is class A too.
pub(crate) fn cc_attr_kind(
    n: &str,
    syntax: AttrSyntax,
    scope: Option<&str>,
    x86: bool,
) -> Option<AttrKind> {
    let bare = syntax == AttrSyntax::Gnu && scope.is_none();
    let gnu = bare || scope == Some("gnu");
    let clang = bare || scope == Some("clang");
    // x86 `no_caller_saved_registers` (GCC spellings, clang
    // `TargetSpecificAttr<TargetAnyX86>`): unknown elsewhere.
    if n == "no_caller_saved_registers" {
        return (x86 && gnu).then_some(AttrKind::NO_CALLER_SAVED_REGS);
    }
    if n == "regparm" {
        return gnu.then_some(AttrKind::REGPARM);
    }
    let idx = CC_ATTR_SPELLINGS.iter().position(|&s| s == n)?;
    let spelled = if gcc_spelled(idx) { gnu } else { clang };
    (spelled && !matches!(idx, 11 | 12)).then_some(AttrKind::CALLING_CONV)
}

/// How clang names convention `cc` in `err_attributes_are_not_compatible`
/// (a target-default one as `cdecl`).
fn cc_name(cc: CallConv) -> &'static str {
    cc.attribute().unwrap_or("cdecl")
}

impl Sema<'_, '_> {
    /// A calling-convention attribute `n` (the name without `__`
    /// wrappers), `regparm` or x86 `no_caller_saved_registers`, one
    /// [`cc_attr_kind`] accepts: `Some(node)` when it is one (None after a
    /// diagnostic), None for any other name.
    #[allow(clippy::option_option)]
    pub(crate) fn cc_attribute(
        &mut self,
        syntax: AttrSyntax,
        scope: Option<&str>,
        n: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<Option<AttrId>> {
        let x86 = self.abi.triple.arch == kiln_abi::target::Arch::X86_64;
        let bare = syntax == AttrSyntax::Gnu && scope.is_none();
        let gnu = bare || scope == Some("gnu");
        let clang = bare || scope == Some("clang");
        if n == "no_caller_saved_registers" {
            if !(x86 && gnu) {
                return None;
            }
            if !args.is_empty() {
                let msg = self
                    .diag(DiagId::parse_attribute_wrong_arg_count, loc)
                    .arg(n.to_owned())
                    .arg("0");
                self.emit(msg);
                return Some(None);
            }
            let node = AttrNode {
                kind: AttrKind::NO_CALLER_SAVED_REGS,
                syntax,
                flags: 0,
                loc,
                a: 0,
                b: 0,
            };
            return Some(Some(self.ast.attrs.push(node)));
        }
        if gnu && n == "regparm" {
            return Some(self.regparm_attribute(syntax, x86, args, loc));
        }
        let idx = CC_ATTR_SPELLINGS.iter().position(|&s| s == n)?;
        if !(if gcc_spelled(idx) { gnu } else { clang }) {
            return None;
        }
        let mut cc = target_cc(idx, x86).map_or(UNSUPPORTED, |c| u32::from(c as u8));
        if n == "pcs" {
            // clang `handleCallConvAttr`, `AT_Pcs`: one string argument,
            // `aapcs` or `aapcs-vfp`, ARM-only.
            let [arg] = args else {
                let msg =
                    self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg("pcs").arg("1");
                self.emit(msg);
                return Some(None);
            };
            let lit = match arg {
                AttrArg::Expr(e) => self.string_lit_of(*e).map(|l| self.ast.exprs.string_lit(l)),
                AttrArg::Type(_) | AttrArg::Ident(..) => None,
            };
            let Some(lit) = lit else {
                let at = match arg {
                    AttrArg::Expr(e) => self.ast.exprs.get(*e).loc(),
                    AttrArg::Ident(_, l) => *l,
                    AttrArg::Type(_) => loc,
                };
                let msg = self.diag(DiagId::parse_attribute_requires_string, at).arg("pcs");
                self.emit(msg);
                return Some(None);
            };
            if !matches!(self.ast.strings.bytes(lit), b"aapcs" | b"aapcs-vfp") {
                self.error(DiagId::sema_invalid_pcs, loc);
                return Some(None);
            }
            cc = UNSUPPORTED;
        } else if !args.is_empty() {
            let msg =
                self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg(n.to_owned()).arg("0");
            self.emit(msg);
            return Some(None);
        }
        #[allow(clippy::cast_possible_truncation, reason = "idx < 19")]
        let node =
            AttrNode { kind: AttrKind::CALLING_CONV, syntax, flags: 0, loc, a: cc, b: idx as u32 };
        Some(Some(self.ast.attrs.push(node)))
    }

    /// GNU `regparm(N)` (clang `checkRegparm`): one integer constant
    /// argument; x86 only (an error elsewhere); N at most 6 on x86-64.
    fn regparm_attribute(
        &mut self,
        syntax: AttrSyntax,
        x86: bool,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let [arg] = args else {
            let msg =
                self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg("regparm").arg("1");
            self.emit(msg);
            return None;
        };
        let n = match arg {
            AttrArg::Expr(e) => self.attr_u32_value(*e),
            AttrArg::Type(_) | AttrArg::Ident(..) => None,
        };
        let Some(n) = n else {
            let msg =
                self.diag(DiagId::sema_attribute_requires_integer_constant, loc).arg("'regparm'");
            self.emit(msg);
            return None;
        };
        if !x86 {
            self.error(DiagId::sema_regparm_wrong_platform, loc);
            return None;
        }
        if n > 6 {
            let msg = self.diag(DiagId::sema_regparm_invalid_number, loc).arg("6");
            self.emit(msg);
            return None;
        }
        let node = AttrNode { kind: AttrKind::REGPARM, syntax, flags: 0, loc, a: n, b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// The value of the integer constant attribute argument `e` (clang
    /// `checkUInt32Argument`), None when it is not one.
    fn attr_u32_value(&mut self, e: kiln_ast::ExprId) -> Option<u32> {
        let ty = self.ast.exprs.get(e).ty();
        if self.is_error_type(ty)
            || !self.is_integral_or_enum(ty)
            || self.expr_is_value_dependent(e)
        {
            return None;
        }
        let c = crate::conv::lvalue_to_rvalue(self, e);
        let c = self.act_on_full_expr(c);
        let at = self.begin_loc(e);
        let v = self.eval_int(c, kiln_oven::EvalContext::new(kiln_oven::EvalKind::Other, at));
        v.ok().and_then(|v| u32::try_from(v).ok())
    }

    /// The calling-convention attributes among `attrs` applied to the
    /// declared type `ty` (a function type, or a pointer, reference or
    /// member pointer to one) and removed: clang's `'X' only applies to
    /// function types; type here is 'T'` on any other type,
    /// `'X' calling convention is not supported for this target` for one
    /// the target ignores, and `X and Y attributes are not compatible`
    /// for a second, different convention (spec/types.md §5.4). `decl`:
    /// the subject is a declaration other than a typedef, where
    /// `preserve_none` (a clang `DeclOrTypeAttr` with a `FunctionLike`
    /// subject list) also warns that it only applies to functions and
    /// function pointers.
    pub(crate) fn apply_cc_attrs(
        &mut self,
        ty: QualType,
        attrs: &mut Vec<AttrId>,
        decl: bool,
    ) -> QualType {
        let is_cc = |s: &Self, a: AttrId| s.ast.attrs.get(a).kind.is_fn_type_attr();
        if !attrs.iter().any(|&a| is_cc(self, a)) {
            return ty;
        }
        let mut nodes = Vec::new();
        attrs.retain(|&a| {
            if is_cc(self, a) {
                nodes.push(*self.ast.attrs.get(a));
                false
            } else {
                true
            }
        });
        if self.is_error_type(ty) {
            return ty;
        }
        let name = |n: &AttrNode| {
            if n.kind == AttrKind::NO_CALLER_SAVED_REGS {
                "no_caller_saved_registers"
            } else if n.kind == AttrKind::REGPARM {
                "regparm"
            } else {
                CC_ATTR_SPELLINGS.get(n.b as usize).copied().unwrap_or("cdecl")
            }
        };
        let Some(info) = self.fn_info_through(ty) else {
            let ts = format!("'{}'", self.ty_str(ty));
            for n in &nodes {
                let msg = self
                    .diag(DiagId::sema_type_attribute_wrong_type, n.loc)
                    .arg(format!("'{}'", name(n)))
                    .arg(ts.clone());
                self.emit(msg);
                if decl && n.kind == AttrKind::CALLING_CONV && name(n) == "preserve_none" {
                    let msg = self
                        .diag(DiagId::sema_attribute_wrong_decl_type, n.loc)
                        .arg("'preserve_none'")
                        .arg("functions and function pointers");
                    self.emit(msg);
                }
            }
            return ty;
        };
        let mut cc = info.cc();
        let mut written = false;
        let mut no_csr = info.no_caller_saved_regs();
        let mut regparm = info.regparm();
        for n in &nodes {
            if n.kind == AttrKind::NO_CALLER_SAVED_REGS {
                no_csr = true;
                continue;
            }
            if n.kind == AttrKind::REGPARM {
                #[allow(clippy::cast_possible_truncation, reason = "checked at most 6")]
                let v = n.a as u8;
                regparm = Some(v);
                continue;
            }
            if n.a == UNSUPPORTED {
                let msg = self.diag(DiagId::sema_cconv_unsupported, n.loc).arg(name(n));
                self.emit(msg);
                continue;
            }
            #[allow(clippy::cast_possible_truncation, reason = "a CallConv discriminant")]
            let new = CallConv::from_u8(n.a as u8);
            if new != cc && (written || cc != CallConv::Default) {
                let msg = self
                    .diag(DiagId::sema_cconv_incompatible, n.loc)
                    .arg(cc_name(new))
                    .arg(cc_name(cc));
                self.emit(msg);
                continue;
            }
            cc = new;
            written = true;
        }
        let f = move |i: FuncInfo| {
            i.with_cc(cc).with_no_caller_saved_regs(no_csr).with_regparm(regparm)
        };
        self.with_fn_info(ty, &f, true).unwrap_or(ty)
    }

    /// [`Sema::apply_cc_attrs`] over a borrowed list (the attributes stay
    /// with their owner).
    pub(crate) fn apply_cc_attrs_of(&mut self, ty: QualType, attrs: &[AttrId]) -> QualType {
        let mut v = attrs.to_vec();
        self.apply_cc_attrs(ty, &mut v, false)
    }

    /// The [`FuncInfo`] of the function type `ty` is, or points, refers
    /// or member-points to.
    pub(crate) fn fn_info_through(&self, ty: QualType) -> Option<FuncInfo> {
        let t = &self.ast.types;
        let c = t.canonical(ty);
        let inner = match t.view(c.ty()) {
            TypeView::Function(p) => return Some(p.info()),
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => p,
            TypeView::MemberPointer { pointee, .. } => pointee,
            _ => return None,
        };
        t.proto(inner).map(kiln_ast::types::FuncProto::info)
    }

    /// The calling convention and `no_caller_saved_registers` of a
    /// redeclaration's type `ty` against its previous declaration's `pty`
    /// (clang `MergeFunctionDecl`): a redeclaration that names none
    /// inherits them; one naming a different convention is
    /// `function declared 'X' here was previously declared ...`, and one
    /// adding `no_caller_saved_registers` is
    /// `err_function_attribute_mismatch`. Returns the merged `ty`.
    pub(crate) fn merge_cc(
        &mut self,
        ty: QualType,
        pty: QualType,
        prev: DeclId,
        at: Loc,
    ) -> QualType {
        let (Some(new), Some(old)) = (
            self.ast.types.proto(ty).map(kiln_ast::types::FuncProto::info),
            self.ast.types.proto(pty).map(kiln_ast::types::FuncProto::info),
        ) else {
            return ty;
        };
        let mut cc = new.cc();
        if cc != old.cc() {
            if cc != CallConv::Default {
                // clang names the first declaration's convention, and puts
                // the note there, "since it is the one that matters".
                let first = self.ast.first(prev);
                let fcc = self
                    .ast
                    .decls
                    .get(first)
                    .ty()
                    .and_then(|t| self.ast.types.proto(t))
                    .map_or(old.cc(), |p| p.info().cc());
                let was = match fcc.attribute() {
                    Some(a) => format!("'{a}'"),
                    None => "without calling convention".to_owned(),
                };
                let msg = self.diag(DiagId::sema_cconv_change, at).arg(cc_name(cc)).arg(was);
                let msg = self.note_prev(msg, first, false);
                self.emit(msg);
            }
            cc = old.cc();
        }
        if new.no_caller_saved_regs() && !old.no_caller_saved_regs() {
            let msg = self
                .diag(DiagId::sema_function_attribute_mismatch, at)
                .arg("no_caller_saved_registers");
            let msg = self.note_prev(msg, prev, false);
            self.emit(msg);
        }
        let no_csr = old.no_caller_saved_regs();
        // clang `MergeFunctionDecl`: `err_regparm_mismatch`.
        if let Some(n) = new.regparm()
            && new.regparm() != old.regparm()
        {
            let was = match old.regparm() {
                Some(o) => format!("with the regparm({o})"),
                None => "without the regparm".to_owned(),
            };
            let msg = self.diag(DiagId::sema_regparm_mismatch, at).arg(n.to_string()).arg(was);
            let msg = self.note_prev(msg, prev, false);
            self.emit(msg);
        }
        let regparm = old.regparm();
        let f = move |i: FuncInfo| {
            i.with_cc(cc).with_no_caller_saved_regs(no_csr).with_regparm(regparm)
        };
        self.with_fn_info(ty, &f, false).unwrap_or(ty)
    }
}
