//! Calling-convention attributes (clang `handleCallConvAttr`,
//! `handleFunctionTypeAttr`; spec/types.md §5.4, WP-808): `ms_abi`,
//! `preserve_most`, `regcall`, `aarch64_vector_pcs`, ... written on a
//! declaration are function type attributes. Sema parses each into an
//! [`AttrKind::CALLING_CONV`] node, then consumes it into the declared
//! type's [`FuncInfo`]: the convention takes part in type identity
//! (function pointer conversions, overloading, redeclaration), mangling
//! and the IR calling convention.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax, CC_ATTR_SPELLINGS};
use kiln_ast::types::{CallConv, FuncInfo, TypeAttrKind, TypeView};
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
/// `[[gnu::preserve_most]]`). `swiftcall` / `swiftasynccall` lower
/// through `kiln_abi::swift` (WP-1745).
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
    spelled.then_some(AttrKind::CALLING_CONV)
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
            if let AttrArg::Ident(_, l) = arg {
                // clang `checkStringLiteralArgumentAttr` (WP-1881): the
                // parser keeps a leading identifier as one; the attribute
                // is still the calling convention no target here has.
                let shown = match self.attr_spellings.get(&loc) {
                    Some(&(sc, nm)) => self.attr_shown_parts(syntax, sc, nm),
                    None => n.to_owned(),
                };
                let msg =
                    self.diag(DiagId::sema_attribute_requires_string, *l).arg(format!("'{shown}'"));
                self.emit(msg);
            } else if let Some(lit) = lit {
                if !matches!(self.ast.strings.bytes(lit), b"aapcs" | b"aapcs-vfp") {
                    self.error(DiagId::sema_invalid_pcs, loc);
                    return Some(None);
                }
            } else {
                let at = match arg {
                    AttrArg::Expr(e) => self.ast.exprs.get(*e).loc(),
                    AttrArg::Ident(_, l) => *l,
                    AttrArg::Type(_) => loc,
                };
                let msg = self.diag(DiagId::parse_attribute_requires_string, at).arg("pcs");
                self.emit(msg);
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

    /// clang `supportsVariadicCall`: the callee-cleanup and Swift
    /// conventions take no `...`; false (after the error at `loc`) drops
    /// convention attribute `shown` from the variadic prototype `info`.
    fn cconv_varargs_ok(&mut self, cc: CallConv, info: FuncInfo, loc: Loc, shown: &str) -> bool {
        let no_va = matches!(
            cc,
            CallConv::RegCall | CallConv::VectorCall | CallConv::Swift | CallConv::SwiftAsync
        );
        if no_va && info.variadic() && !info.no_proto() {
            let msg = self.diag(DiagId::sema_cconv_varargs, loc).arg(shown);
            self.emit(msg);
            return false;
        }
        true
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
        let in_chunk = |s: &Self, a: AttrId| {
            let n = s.ast.attrs.get(a);
            n.kind.is_fn_type_attr() && n.flags & AttrNode::IN_CHUNK != 0
        };
        let ty = if attrs.iter().any(|&a| in_chunk(self, a)) {
            // Those written at a pointer, member pointer or grouping
            // chunk apply after the function type they reach was built
            // and its parameter ABIs checked (WP-1827).
            let mut chunk = Vec::new();
            attrs.retain(|&a| {
                let c = in_chunk(self, a);
                if c {
                    chunk.push(a);
                }
                !c
            });
            let ty = self.apply_cc_attrs_inner(ty, attrs, decl);
            self.param_abis_before_chunk_attrs(ty);
            self.apply_cc_attrs_inner(ty, &mut chunk, decl)
        } else {
            self.apply_cc_attrs_inner(ty, attrs, decl)
        };
        // The Swift parameter ABIs need the convention (WP-1745); a
        // declaration's attributes are its last (a type-id's and a
        // typedef's may be followed by the alias's).
        self.param_abis_after_cc(ty, decl);
        ty
    }

    #[allow(clippy::too_many_lines, reason = "clang's per-attribute checks, in its order")]
    fn apply_cc_attrs_inner(
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
        self.drop_decl_position_sme(&mut nodes);
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
                if n.kind == AttrKind::SME_TYPE {
                    let msg = self
                        .diag(DiagId::sema_sme_attr_wrong_type, n.loc)
                        .arg(Self::sme_keyword_shown(n))
                        .arg(ts.clone());
                    self.emit(msg);
                    continue;
                }
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
        // A convention already written on the type (clang
        // `getCallingConvAttributedType`): a typedef's `sysv_abi`.
        let mut written = self.cc_attributed(ty);
        // The spelling of a written convention that is the target's
        // default (or one it ignores): kept as sugar (WP-1858).
        let mut default_spelling = None;
        let mut no_csr = info.no_caller_saved_regs();
        let mut regparm = info.regparm();
        let sme = self.merge_sme_attrs(info, &nodes);
        for n in &nodes {
            if n.kind == AttrKind::SME_TYPE {
                continue;
            }
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
            // clang `CheckCallingConvAttr`: one the target ignores warns
            // and is the default convention, still written.
            let new = if n.a == UNSUPPORTED {
                // As written: `__pcs__`, clang 22's `gnu::pcs` (WP-1881).
                let msg = self
                    .diag(DiagId::sema_cconv_unsupported, n.loc)
                    .arg(self.attr_shown(n, name(n)));
                self.emit(msg);
                CallConv::Default
            } else {
                #[allow(clippy::cast_possible_truncation, reason = "a CallConv discriminant")]
                let v = n.a as u8;
                CallConv::from_u8(v)
            };
            if new != cc && (written || cc != CallConv::Default) {
                let msg = self
                    .diag(DiagId::sema_cconv_incompatible, n.loc)
                    .arg(cc_name(new))
                    .arg(cc_name(cc));
                self.emit(msg);
                continue;
            }
            if !self.cconv_varargs_ok(new, info, n.loc, name(n)) {
                continue;
            }
            cc = new;
            written = true;
            default_spelling = (cc == CallConv::Default).then_some(n.b as usize);
        }
        let f = move |i: FuncInfo| {
            i.with_cc(cc).with_no_caller_saved_regs(no_csr).with_regparm(regparm).with_sme(sme)
        };
        let ty = self.with_fn_info(ty, &f, true).unwrap_or(ty);
        match default_spelling {
            Some(sp) => self.wrap_fn_type(ty, &|t, f| t.attributed_cc(Some(sp), f)).unwrap_or(ty),
            None => ty,
        }
    }

    /// Whether the function type `ty` is, through its sugar, has a
    /// calling-convention attribute written (clang
    /// `getCallingConvAttributedType`): a convention other than the
    /// default always is; the default one (`sysv_abi`, `cdecl`, one the
    /// target ignores) is when its [`kiln_ast::types::TypeAttrKind::CallingConv`]
    /// sugar is there (WP-1858).
    pub(crate) fn cc_attributed(&self, ty: QualType) -> bool {
        let t = &self.ast.types;
        let mut q = ty;
        for _ in 0..1024 {
            if let TypeView::Attributed { kind: TypeAttrKind::CallingConv, .. } = t.view(q.ty()) {
                return true;
            }
            let n = t.desugar_once(q);
            if n == q {
                break;
            }
            q = n;
        }
        false
    }

    /// The function type `ty` rebuilt from `of`, with the sugar of a
    /// default calling convention written on `of` (its top-level sugar,
    /// [`Self::cc_attributed`]); `ty` when `of` has none.
    pub(crate) fn with_cc_sugar_of(&mut self, ty: QualType, of: QualType) -> QualType {
        let t = &self.ast.types;
        let mut q = of;
        for _ in 0..1024 {
            if let TypeView::Attributed { kind: TypeAttrKind::CallingConv, .. } = t.view(q.ty()) {
                let sp = t.cc_attr_spelling(q.ty());
                return self.ast.types.attributed_cc(sp, ty);
            }
            let n = t.desugar_once(q);
            if n == q {
                break;
            }
            q = n;
        }
        ty
    }

    /// `ty` (a function type, or one reached through pointers,
    /// references, member pointers and arrays, as [`Sema::with_fn_info`])
    /// with that function type, sugar included, wrapped by `f`; None when
    /// it reaches none.
    fn wrap_fn_type(
        &mut self,
        ty: QualType,
        f: &dyn Fn(&mut kiln_ast::types::TypeContext, QualType) -> QualType,
    ) -> Option<QualType> {
        if self.ast.types.is_function(ty) {
            let q = ty.quals();
            let w = f(&mut self.ast.types, ty.without_quals());
            return Some(self.ast.types.qualify(w, q));
        }
        let built = match self.ast.types.view(self.ast.types.desugar_top(ty).ty()) {
            TypeView::Pointer(p) => {
                let g = self.wrap_fn_type(p, f)?;
                self.ast.types.pointer_to(g)
            }
            TypeView::LValueRef(p) => {
                let g = self.wrap_fn_type(p, f)?;
                self.ast.types.lvalue_ref_to(g)
            }
            TypeView::RValueRef(p) => {
                let g = self.wrap_fn_type(p, f)?;
                self.ast.types.rvalue_ref_to(g)
            }
            TypeView::MemberPointer { pointee, class } => {
                let g = self.wrap_fn_type(pointee, f)?;
                self.ast.types.member_pointer(g, class)
            }
            TypeView::ConstantArray(a) => {
                let g = self.wrap_fn_type(a.elem, f)?;
                self.ast.types.constant_array(g, a.size, a.size_expr)
            }
            TypeView::IncompleteArray(e) => {
                let g = self.wrap_fn_type(e, f)?;
                self.ast.types.incomplete_array(g)
            }
            _ => return None,
        };
        Some(self.ast.types.qualify(built, ty.quals()))
    }

    /// [`Sema::apply_cc_attrs`] over a borrowed list (the attributes stay
    /// with their owner).
    pub(crate) fn apply_cc_attrs_of(&mut self, ty: QualType, attrs: &[AttrId]) -> QualType {
        let mut v = attrs.to_vec();
        self.apply_cc_attrs(ty, &mut v, false)
    }

    /// The [`FuncInfo`] of the function type `ty` is, or points, refers
    /// or member-points to, through any number of those and of arrays
    /// (clang `FunctionTypeUnwrapper`).
    pub(crate) fn fn_info_through(&self, ty: QualType) -> Option<FuncInfo> {
        let f = self.fn_type_through(ty)?;
        self.ast.types.proto(f).map(kiln_ast::types::FuncProto::info)
    }

    /// The (canonical) function type `ty` is, or reaches through pointers,
    /// references, member pointers and arrays of known or unknown bound
    /// (clang `FunctionTypeUnwrapper`, which a function type attribute
    /// applies through).
    pub(crate) fn fn_type_through(&self, ty: QualType) -> Option<QualType> {
        let t = &self.ast.types;
        let mut c = t.canonical(ty);
        for _ in 0..1024 {
            c = match t.view(c.ty()) {
                TypeView::Function(_) => return Some(c),
                TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => p,
                TypeView::MemberPointer { pointee, .. } => pointee,
                TypeView::ConstantArray(a) => a.elem,
                TypeView::IncompleteArray(e) => e,
                _ => return None,
            };
            c = t.canonical(c);
        }
        None
    }

    /// Function type `ty` with the calling convention of function type
    /// `of` (clang `adjustCCAndNoReturn`'s convention half); `ty` when
    /// either is not a function type.
    pub(crate) fn with_cc_of(&mut self, ty: QualType, of: QualType) -> QualType {
        let Some(cc) = self.ast.types.proto(of).map(|p| p.info().cc()) else { return ty };
        if self.ast.types.proto(ty).is_none_or(|p| p.info().cc() == cc) {
            return ty;
        }
        let f = move |i: FuncInfo| i.with_cc(cc);
        self.with_fn_info(ty, &f, false).unwrap_or(ty)
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
        self.merge_cc_noted(ty, pty, prev, at, None)
    }

    /// [`Self::merge_cc`], its notes at `note_at` when given: a function
    /// template's specialization first declared by `at`'s explicit
    /// specialization is "previously declared" there (clang creates it at
    /// the specialization's location; WP-1843).
    pub(crate) fn merge_cc_noted(
        &mut self,
        ty: QualType,
        pty: QualType,
        prev: DeclId,
        at: Loc,
        note_at: Option<Loc>,
    ) -> QualType {
        let note = |s: &Self, msg: kiln_diag::Diag, prev: DeclId| match note_at {
            Some(l) => msg.with_note(s.diag(DiagId::note_previous_declaration, l)),
            None => s.note_prev(msg, prev, false),
        };
        let (Some(new), Some(old)) = (
            self.ast.types.proto(ty).map(kiln_ast::types::FuncProto::info),
            self.ast.types.proto(pty).map(kiln_ast::types::FuncProto::info),
        ) else {
            return ty;
        };
        let mut cc = new.cc();
        if cc != old.cc() {
            // A redeclaration naming no convention inherits it; one naming
            // any, the target's default included (`sysv_abi`, `cdecl`;
            // clang `getCallingConvAttributedType`, WP-1858), must agree.
            if cc != CallConv::Default || self.cc_attributed(ty) {
                // clang names the first declaration's convention, and puts
                // the note there, "since it is the one that matters".
                let first = self.ast.first(prev);
                let fty = self.ast.decls.get(first).ty();
                let fcc =
                    fty.and_then(|t| self.ast.types.proto(t)).map_or(old.cc(), |p| p.info().cc());
                let was = if fcc != CallConv::Default || fty.is_some_and(|t| self.cc_attributed(t))
                {
                    format!("'{}'", cc_name(fcc))
                } else {
                    "without calling convention".to_owned()
                };
                let msg = self.diag(DiagId::sema_cconv_change, at).arg(cc_name(cc)).arg(was);
                let msg = note(self, msg, first);
                self.emit(msg);
            }
            cc = old.cc();
        }
        if new.no_caller_saved_regs() && !old.no_caller_saved_regs() {
            let msg = self
                .diag(DiagId::sema_function_attribute_mismatch, at)
                .arg("no_caller_saved_registers");
            let msg = note(self, msg, prev);
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
            let msg = note(self, msg, prev);
            self.emit(msg);
        }
        let regparm = old.regparm();
        // A redeclaration with other SME attributes is an error; it takes
        // the previous ones, so nothing else is said about it (WP-1426).
        self.check_sme_redecl(ty, pty, prev, at);
        let sme = old.sme();
        let f = move |i: FuncInfo| {
            i.with_cc(cc).with_no_caller_saved_regs(no_csr).with_regparm(regparm).with_sme(sme)
        };
        self.with_fn_info(ty, &f, false).unwrap_or(ty)
    }
}
