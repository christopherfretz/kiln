//! Builtin functions (spec/sema.md §5.4): clang's `LazilyCreateBuiltin`.
//!
//! A builtin function is declared the first time an unqualified lookup of
//! its name finds nothing: an implicit `extern "C"` `Function` decl in the
//! translation unit (inside an implicit `LinkageSpec`, as clang dumps it)
//! carrying its builtin id in the `builtin_id` cold data, an implicit
//! `BuiltinAttr` and, for the `noexcept` ones, an implicit `NoThrowAttr`.
//! Its name has the placeholder type `<builtin fn type>`; a call converts
//! it with `BuiltinFnToFnPtr` ([`Sema::act_on_call`]), and any other use is
//! an error.

use crate::lookup::LookupKind;
use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::consts::{PtrBase, Scalar};
use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind, FnSpec, FnStorage, LinkageLang, builtin};
use kiln_ast::expr::{CastKind, ExprFlags, ExprKind, ValueCategory};
use kiln_ast::expr::{SourceLocKind, SourceLocValues};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::TargetTypes;
use kiln_ast::types::{BuiltinType, ExcSpec, FuncInfo, NoexceptValue, Quals, VectorKind};
use kiln_ast::types::{Nns, TagKeyword};
use kiln_ast::{Ast, AttrId, DeclHead, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::conv::{self, ConvCtx};

mod atomic;

/// The implicit typedefs `__int128_t` and `__uint128_t` for `__int128`
/// and `unsigned __int128` (clang's `Sema::Initialize`: declared in the
/// translation unit when the target has a 128-bit integer type, i.e. on
/// 64-bit targets). They are ordinary `Typedef` decls marked `IMPLICIT`,
/// so a program's `typedef __int128 __int128_t;` redeclares them and
/// `-ast-dump` skips them, as clang.
pub(crate) fn declare_int128_typedefs(ast: &mut Ast, target: &TargetTypes) {
    if target.pointer_bytes < 8 {
        return;
    }
    let tu = ast.tu;
    for (name, ty) in [("__int128_t", QualType::INT128), ("__uint128_t", QualType::UINT128)] {
        let n = DeclName::ident(ast.symbols().intern(name));
        let h = DeclHead::new(tu, n, Loc::INVALID).flags(DeclFlags::IMPLICIT);
        let _ = ast.builder().typedef(h, DeclKind::Typedef, ty, None);
    }
}

impl Sema<'_, '_> {
    /// The aliases behind [`Sema::size_type`], [`Sema::signed_size_type`]
    /// and [`Sema::ptrdiff_type`] under `-fclang-compat=22`: clang 22
    /// prints `size_t`, its signed counterpart and `ptrdiff_t` as written
    /// by the implementation (`PredefinedSugarType`: `__size_t`,
    /// `__signed_size_t`, `__ptrdiff_t`) while no declaration of those
    /// names is visible, so the aliases are implicit and linked into no
    /// lookup table (`__size_t x;` stays an unknown type name, as clang).
    pub(crate) fn declare_size_t_sugar(&mut self) {
        if self.target.pointer_bytes == 0 {
            return;
        }
        let size_t = QualType::unqual(self.target.size_t);
        let signed = match size_t {
            QualType::UINT => QualType::INT,
            QualType::ULONGLONG => QualType::LONGLONG,
            _ => QualType::LONG,
        };
        let ptrdiff_t = QualType::unqual(self.target.ptrdiff_t);
        let mk = |s: &mut Self, name: &str, ty: QualType| {
            let tu = s.ast.tu;
            let n = DeclName::ident(s.ast.symbols().intern(name));
            let h = DeclHead::new(tu, n, Loc::INVALID).flags(DeclFlags::IMPLICIT);
            let d = s.ast.builder().unlinked_alias(h, ty);
            s.ast.decls.get(d).ty().unwrap_or(ty)
        };
        self.size_t_sugar = mk(self, "__size_t", size_t);
        self.signed_size_t_sugar = mk(self, "__signed_size_t", signed);
        self.ptrdiff_t_sugar = mk(self, "__ptrdiff_t", ptrdiff_t);
    }

    /// The implicit typedef `__builtin_va_list` (clang's `Sema::Initialize`
    /// and `CreateVaListDecl`), declared in the translation unit:
    /// - `AArch64` (AAPCS64 §10.1.5): `typedef std::__va_list
    ///   __builtin_va_list;` with `struct __va_list { void *__stack;
    ///   void *__gr_top; void *__vr_top; int __gr_offs; int __vr_offs; }`
    ///   in an implicit `namespace std` that lookup does not see;
    /// - x86-64 (`SysV` psABI §3.5.7): `typedef __va_list_tag
    ///   __builtin_va_list[1];` with `struct __va_list_tag { unsigned
    ///   gp_offset; unsigned fp_offset; void *overflow_arg_area; void
    ///   *reg_save_area; }`, a record lookup does not see.
    ///
    /// All are `IMPLICIT`, so `-ast-dump` skips them, as clang.
    pub(crate) fn declare_builtin_va_list(&mut self) {
        use kiln_abi::target::Arch;
        use kiln_ast::decl::{Access, TagKind};
        if self.target.pointer_bytes == 0 {
            return;
        }
        let tu = self.ast.tu;
        let void_ptr = self.ast.types.pointer_to(QualType::VOID);
        let (ctx, tag_name, fields): (DeclId, &str, [(&str, QualType); 5]) =
            match self.abi.triple.arch {
                Arch::AArch64 => {
                    let std = DeclName::ident(self.ast.symbols().intern("std"));
                    let ns = self.builder().hidden_namespace(tu, std);
                    let f = [
                        ("__stack", void_ptr),
                        ("__gr_top", void_ptr),
                        ("__vr_top", void_ptr),
                        ("__gr_offs", QualType::INT),
                        ("__vr_offs", QualType::INT),
                    ];
                    (ns, "__va_list", f)
                }
                Arch::X86_64 => {
                    let f = [
                        ("gp_offset", QualType::UINT),
                        ("fp_offset", QualType::UINT),
                        ("overflow_arg_area", void_ptr),
                        ("reg_save_area", void_ptr),
                        ("", QualType::VOID),
                    ];
                    (tu, "__va_list_tag", f)
                }
            };
        // Created unnamed and named afterwards, so the tag is not entered
        // in lookup.
        let h = DeclHead::new(ctx, DeclName::EMPTY, Loc::INVALID).flags(DeclFlags::IMPLICIT);
        let r = self.builder().record(h, TagKind::Struct, None);
        let tag = DeclName::ident(self.ast.symbols().intern(tag_name));
        self.ast.decls.set_name(r, tag);
        let saved = self.ctx;
        self.act_on_start_record_definition(r);
        for (name, ty) in fields.into_iter().filter(|f| !f.0.is_empty()) {
            let n = DeclName::ident(self.ast.symbols().intern(name));
            let f = self.builder().field(DeclHead::new(r, n, Loc::INVALID), ty, None, 0);
            self.ast.decls.set_access(f, Access::Public);
        }
        self.act_on_finish_record_definition(r, false);
        self.ctx = saved;
        let rty = self.tag_type(r);
        let ty = match self.abi.triple.arch {
            Arch::AArch64 => rty,
            Arch::X86_64 => self.ast.types.constant_array(rty, 1, None),
        };
        let n = DeclName::ident(self.ast.symbols().intern("__builtin_va_list"));
        let h = DeclHead::new(tu, n, Loc::INVALID).flags(DeclFlags::IMPLICIT);
        let td = self.builder().typedef(h, DeclKind::Typedef, ty, None);
        self.va_list_type = Some(self.ast.types.typedef_type(td, ty).into());
    }

    /// The parameter type a `Builtins.td` `A` spells (clang's
    /// `DecodeTypeFromStr`): `__builtin_va_list &`, or the decayed pointer
    /// when the target's `va_list` is an array (x86-64).
    fn builtin_va_list_param(&mut self) -> QualType {
        let Some(t) = self.va_list_type else { return QualType::ERROR };
        if self.ast.types.is_array(t) {
            conv::array_decayed(self, t)
        } else {
            self.ast.types.lvalue_ref_to(t)
        }
    }

    /// The parameter type a `Builtins.td` `a` spells: `__builtin_va_list`,
    /// decayed when the target's `va_list` is an array (clang
    /// `GetBuiltinType` decays array parameters).
    fn builtin_va_list_value(&mut self) -> QualType {
        let Some(t) = self.va_list_type else { return QualType::ERROR };
        if self.ast.types.is_array(t) { conv::array_decayed(self, t) } else { t }
    }

    /// The type a `Builtins.td` `P` spells (clang `ASTContext::getFILEType`):
    /// the type a translation-unit-scope typedef named `FILE` names, or
    /// the error type when there is none (clang `GE_Missing_stdio`).
    fn file_type(&mut self) -> QualType {
        let name = self.intern("FILE");
        let found = self.lookup_in(self.ast.tu, name, LookupKind::Ordinary);
        // clang `LookupNecessaryTypesForBuiltin`: any type declaration,
        // a typedef or (WP-186) a tag such as `struct FILE;`.
        let typedef = found
            .decls
            .iter()
            .find(|&&d| matches!(self.kind_of(d), DeclKind::Typedef | DeclKind::TypeAlias));
        if let Some(&d) = typedef {
            return self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        }
        match found.decls.iter().find(|&&d| crate::lookup::is_tag_kind(self.kind_of(d))) {
            Some(&d) => self.tag_type(d),
            None => QualType::ERROR,
        }
    }

    /// clang's library-builtin recognition (`ActOnFunctionDeclarator`)
    /// and `AddKnownFunctionAttributes` (spec/sema.md §5.4): the first
    /// declaration of a [`builtin::LIB`] function, in a namespace-scope
    /// context with C language linkage and with the row's type ignoring
    /// the exception specification, gets clang's implicit `NoThrowAttr`,
    /// `ConstAttr` / `PureAttr`, appended to `attrs`. Redeclarations
    /// inherit them (the attributes are read across the redeclaration
    /// chain). `NoThrowAttr` makes a call non-throwing for `noexcept`
    /// ([`Sema::can_throw`]) and `nounwind` in IR, as in clang; no
    /// `BuiltinAttr` is attached, so a call stays an ordinary call.
    ///
    /// WP-419: the declaration also records the row's `__builtin_` twin
    /// (`ceil` -> `__builtin_ceil`), which redeclarations inherit. Codegen
    /// lowers a call through the twin where clang's `EmitBuiltinExpr`
    /// does (the errno-free rounding, `fabs`, `copysign`, `fma`, `fmax` /
    /// `fmin`, `fmaximum_num` / `fminimum_num` functions); everywhere
    /// else the call stays an ordinary call.
    pub(crate) fn add_known_function_attrs(&mut self, f: DeclId, attrs: &mut Vec<AttrId>) {
        if let Some(p) = self.ast.prev(f) {
            if self.kind_of(f) == DeclKind::Function
                && let Some(id) = self.ast.decls.lib_builtin(p)
            {
                let _ = self.ast.decls.cold.lib_builtin.insert(f, id);
            }
            return;
        }
        let Some(info) = self.lib_builtin_info(f) else { return };
        if let Some(id) =
            builtin::by_name_at(&format!("__builtin_{}", info.name), self.clang_compat)
        {
            let _ = self.ast.decls.cold.lib_builtin.insert(f, id);
        }
        let loc = self.ast.decls.get(f).loc();
        // WP-186: the printf family's implicit `format`, first (clang
        // order), unless one is written.
        if let Some((fmt, first)) = builtin::lib_printf_format(info.name)
            && !attrs.iter().any(|&a| self.ast.attrs.get(a).kind == AttrKind::FORMAT)
        {
            let b = u32::from(fmt) | u32::from(first) << 16;
            let n = AttrNode {
                kind: AttrKind::FORMAT,
                syntax: AttrSyntax::Gnu,
                flags: 1,
                loc,
                a: 0,
                b,
            };
            attrs.push(self.ast.attrs.push(n));
        }
        for (bit, kind) in [
            (builtin::attr::NOTHROW, AttrKind::NOTHROW),
            (builtin::attr::CONST, AttrKind::CONST),
            (builtin::attr::PURE, AttrKind::PURE),
        ] {
            if info.attrs & bit != 0 {
                let n = AttrNode { kind, syntax: AttrSyntax::Gnu, flags: 1, loc, a: 0, b: 0 };
                attrs.push(self.ast.attrs.push(n));
            }
        }
    }

    /// GCC's implicit library builtins (WP-380, spec/sema.md §5.4): under
    /// [`Sema::gnu_lib_builtins`] the first declaration of a
    /// [`builtin::LIB`] function recognized as clang's library builtin
    /// whose `__builtin_` twin GCC and the Oven fold
    /// ([`builtin::gnu_lib_twin`]) records that twin; redeclarations
    /// inherit it. A call stays an ordinary call everywhere but in the
    /// Oven, which evaluates it as the twin.
    pub(crate) fn record_gnu_lib_twin(&mut self, f: DeclId) {
        if !self.gnu_lib_builtins || self.kind_of(f) != DeclKind::Function {
            return;
        }
        let twin = match self.ast.prev(f) {
            Some(p) => self.ast.decls.gnu_lib_twin(p),
            None => match self.lib_builtin_info(f) {
                Some(info) => builtin::gnu_lib_twin(info.name),
                None => None,
            },
        };
        if let Some(id) = twin {
            let _ = self.ast.decls.cold.gnu_lib_twin.insert(f, id);
        }
    }

    /// clang's `BuiltinAttr` on a written declaration (spec/sema.md §5.4,
    /// "One declaration per builtin"): a redeclaration of a builtin is
    /// the builtin (its id is inherited along the redeclaration chain,
    /// so `extern "C" void *__builtin_memcpy(...)` after a call still
    /// calls the builtin); and the first declaration of a non-library
    /// builtin name (`builtin::by_name`) in a namespace-scope context
    /// with C language linkage and with the builtin's type ignoring the
    /// exception specification is the builtin (clang
    /// `ActOnFunctionDeclarator`; implicit `Builtin` attribute appended
    /// to `attrs`). Any other declaration of the name is an ordinary
    /// function.
    pub(crate) fn recognize_builtin_decl(&mut self, f: DeclId, attrs: &mut Vec<AttrId>) {
        if self.kind_of(f) != DeclKind::Function {
            return;
        }
        if let Some(p) = self.ast.prev(f) {
            if let Some(id) = self.ast.decls.builtin_id(p) {
                if !self.ast.decls.get(f).flags().contains(DeclFlags::INVALID)
                    && let Some(info) = builtin::info(id)
                    && !self.check_builtin_redecl_exc_spec(f, &info, self.ast.decls.get(p).loc())
                {
                    return;
                }
                let _ = self.ast.decls.cold.builtin_id.insert(f, id);
            }
            return;
        }
        let Some(name) = self.ast.decls.get(f).name().as_ident() else { return };
        let Some(id) = builtin::by_name_at(self.ast.symbols().as_str(name), self.clang_compat)
        else {
            return;
        };
        // Another target's builtin name (`_mm_pause` on `AArch64`) is an
        // ordinary function.
        if !self.target_has_builtin(id) {
            return;
        }
        let Some(info) = builtin::info(id) else { return };
        if !self.c_linkage_file_scope_fn_of_type(f, info.sig) {
            self.check_builtin_redecl(f, &info);
            return;
        }
        if info.attrs & builtin::attr::CUSTOM != 0 {
            self.check_builtin_redecl(f, &info);
            return;
        }
        let loc = self.ast.decls.get(f).loc();
        if !self.check_builtin_redecl_exc_spec(f, &info, loc) {
            return;
        }
        let _ = self.ast.decls.cold.builtin_id.insert(f, id);
        // clang `CheckEquivalentExceptionSpec`: a redeclaration of a
        // `NoThrow` builtin without a specification gets `throw()`, so a
        // later `noexcept` redeclaration matches.
        if info.attrs & builtin::attr::NOTHROW != 0
            && let Some(ty) = self.ast.decls.get(f).ty()
            && self.written_exc_spec(ty) == ExcSpec::None
        {
            let ty = self.with_exc_spec(ty, ExcSpec::DynamicNone);
            self.ast.decls.set_ty(f, ty);
        }
        let n = AttrNode {
            kind: AttrKind::BUILTIN,
            syntax: AttrSyntax::Gnu,
            flags: 1,
            loc: Loc::INVALID,
            a: id,
            b: 0,
        };
        attrs.push(self.ast.attrs.push(n));
    }

    /// The type of the builtin `info` (clang's `GetBuiltinType`): the
    /// type its `Builtins.td` signature spells, `noreturn` and `noexcept`
    /// as its attributes say; `None` when the signature names a type the
    /// target lacks.
    fn builtin_fn_type(&mut self, info: &builtin::Info) -> Option<QualType> {
        let (ret, params, variadic) = self.builtin_signature(info.sig);
        if ret == QualType::ERROR || params.contains(&QualType::ERROR) {
            return None;
        }
        let has = |a: u8| info.attrs & a != 0;
        let fi =
            FuncInfo::new().with_variadic(variadic).with_noreturn(has(builtin::attr::NORETURN));
        let exc = if has(builtin::attr::NOTHROW) { ExcSpec::BasicNoexcept } else { ExcSpec::None };
        Some(self.ast.types.function(ret, &params, fi, exc))
    }

    /// clang's "'X' is a builtin with type 'T'" note
    /// (`note_previous_builtin_declaration`) at `loc`.
    pub(crate) fn builtin_type_note(
        &mut self,
        msg: kiln_diag::Diag,
        info: &builtin::Info,
        loc: Loc,
    ) -> kiln_diag::Diag {
        let Some(ty) = self.builtin_fn_type(info) else { return msg };
        let note = self
            .diag(DiagId::note_previous_builtin_declaration, loc)
            .arg(info.name)
            .arg(self.ty_arg(ty));
        msg.with_note(note)
    }

    /// The first declaration `f` of the non-library builtin `info`'s name
    /// at file scope with C language linkage that is not the builtin
    /// (spec/sema.md §5.4, "Redeclaring a builtin"; clang
    /// `MergeFunctionDecl` against the implicit builtin declaration): a
    /// custom-typechecked builtin cannot be redeclared at all ("cannot
    /// redeclare builtin function"); any other declaration whose type
    /// differs from the builtin's (ignoring the exception specification)
    /// is "conflicting types", or at `-fclang-compat=19`, when only the
    /// return type differs, "functions that differ only in their return
    /// type cannot be overloaded" (clang 19 checks overloading first).
    /// Both carry the builtin-type note at `f` and make `f` invalid. A
    /// declaration in a namespace or with C++ linkage is an ordinary
    /// function (clang's redeclaration lookup does not find the builtin).
    fn check_builtin_redecl(&mut self, f: DeclId, info: &builtin::Info) {
        if !self.file_scope_extern_c_fn(f) {
            return;
        }
        let Some(ty) = self.ast.decls.get(f).ty() else { return };
        let Some(bty) = self.builtin_fn_type(info) else { return };
        let loc = self.ast.decls.get(f).loc();
        let name = info.name;
        let msg = if info.attrs & builtin::attr::CUSTOM != 0 {
            self.diag(DiagId::sema_builtin_redeclare, loc).arg(name)
        } else {
            let same_params = match (self.ast.types.proto(bty), self.ast.types.proto(ty)) {
                (Some(a), Some(b)) => {
                    let (a, b) = (*a, *b);
                    let (pa, pb) =
                        (self.ast.types.params(&a).to_vec(), self.ast.types.params(&b).to_vec());
                    pa.len() == pb.len()
                        && pa.iter().zip(&pb).all(|(x, y)| self.ast.types.same_type(*x, *y))
                        && a.info().variadic() == b.info().variadic()
                }
                _ => false,
            };
            if same_params && self.clang_compat < 22 {
                self.diag(DiagId::sema_ovl_diff_return_type, loc)
            } else {
                self.diag(DiagId::sema_conflicting_types, loc).arg(name)
            }
        };
        let msg = self.builtin_type_note(msg, info, loc);
        self.emit(msg);
        self.ast.decls.set_flags(f, DeclFlags::INVALID);
    }

    /// Whether `f` is a `Function` whose redeclaration context is the
    /// translation unit and that has C language linkage: the declarations
    /// clang's redeclaration lookup matches against an implicit builtin.
    fn file_scope_extern_c_fn(&self, f: DeclId) -> bool {
        self.kind_of(f) == DeclKind::Function
            && kiln_abi::mangle::is_extern_c(&self.ast, f)
            && self.ast.decls.function(f).is_some_and(|x| x.spec.storage() != FnStorage::Static)
            && self.ast.decls.get(f).ctx().is_some_and(|c| {
                let r = self.ast.redecl_context(c);
                self.kind_of(r) == DeclKind::TranslationUnit
            })
    }

    /// [except.spec]/5 against the implicit declaration of the `NoThrow`
    /// builtin `info` (clang `CheckEquivalentExceptionSpec`; its note
    /// "previous declaration is here" is at `prev_loc`: the previous
    /// declaration of `f`, or for the first one `f` itself, where clang
    /// puts the implicit builtin declaration): a potentially
    /// throwing specification as written on `f` (`noexcept(false)`,
    /// `throw(T)`) is "exception specification in declaration does not
    /// match previous declaration", and `f` is invalid. False when it was
    /// reported.
    fn check_builtin_redecl_exc_spec(
        &mut self,
        f: DeclId,
        info: &builtin::Info,
        prev_loc: Loc,
    ) -> bool {
        let Some(ty) = self.ast.decls.get(f).ty() else { return true };
        let spec = self.written_exc_spec(ty);
        if info.attrs & builtin::attr::NOTHROW == 0
            || !self.file_scope_extern_c_fn(f)
            || matches!(spec, ExcSpec::None | ExcSpec::Deferred(_))
            || spec.is_nothrow()
            || matches!(spec, ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(_), .. })
            || matches!(spec, ExcSpec::DependentNoexcept(_))
        {
            return true;
        }
        let loc = self.ast.decls.get(f).loc();
        let msg = self.diag(DiagId::sema_exception_spec_mismatch, loc);
        let msg = msg.with_note(self.diag(DiagId::note_previous_declaration, prev_loc));
        self.emit(msg);
        self.ast.decls.set_flags(f, DeclFlags::INVALID);
        false
    }

    /// Whether `f` is a `Function` in a namespace-scope context (through
    /// linkage specifications) with C language linkage whose type is the
    /// one the `Builtins.td` signature `sig` spells, ignoring the
    /// exception specification.
    fn c_linkage_file_scope_fn_of_type(&mut self, f: DeclId, sig: &str) -> bool {
        if self.kind_of(f) != DeclKind::Function {
            return false;
        }
        let file_ctx = self.ast.decls.get(f).ctx().is_some_and(|c| {
            let r = self.ast.redecl_context(c);
            matches!(self.kind_of(r), DeclKind::TranslationUnit | DeclKind::Namespace)
        });
        if !file_ctx || !kiln_abi::mangle::is_extern_c(&self.ast, f) {
            return false;
        }
        let Some(ty) = self.ast.decls.get(f).ty() else { return false };
        let (ret, params, variadic) = self.builtin_signature(sig);
        if ret == QualType::ERROR || params.contains(&QualType::ERROR) {
            return false;
        }
        let want = self.ast.types.function(
            ret,
            &params,
            FuncInfo::new().with_variadic(variadic),
            ExcSpec::None,
        );
        let have = self.with_exc_spec(ty, ExcSpec::None);
        self.ast.types.same_type(want, have)
    }

    /// The [`builtin::LIB`] row declaration `f` is recognized as (clang's
    /// library-builtin recognition in `ActOnFunctionDeclarator`, which
    /// gives it a builtin id): a `Function` in a namespace-scope context
    /// with C language linkage, named in the table and with the row's type
    /// ignoring the exception specification; WP-419: and declared at the
    /// `-fclang-compat` level and in the dialect ([`builtin::lib_min_compat`],
    /// [`builtin::lib_gnu_only`]).
    pub(crate) fn lib_builtin_info(&mut self, f: DeclId) -> Option<builtin::Info> {
        if self.kind_of(f) != DeclKind::Function {
            return None;
        }
        let name = self.ast.decls.get(f).name().as_ident()?;
        let info = builtin::lib_by_name(self.ast.symbols().as_str(name)).copied()?;
        if builtin::lib_min_compat(info.id) > self.clang_compat
            || (builtin::lib_gnu_only(info.id) && !self.gnu)
        {
            return None;
        }
        self.c_linkage_file_scope_fn_of_type(f, info.sig).then_some(info)
    }

    /// Declares the builtin function `name` (first used at `loc`) if it
    /// is one kiln knows, from its [`builtin::Info`] row: the type its
    /// `Builtins.td` signature spells (`noexcept` for the `NoThrow` ones,
    /// clang's `GetBuiltinType`), one implicit parameter per parameter
    /// type, and the implicit attributes in clang's order (`BuiltinAttr`,
    /// then `NoThrowAttr`, `ConstAttr` / `PureAttr`).
    pub(crate) fn lazily_create_builtin(&mut self, name: Symbol, loc: Loc) -> Option<DeclId> {
        let id = builtin::by_name_at(self.ast.symbols().as_str(name), self.clang_compat)?;
        if !self.target_has_builtin(id) {
            return None;
        }
        let info = builtin::info(id)?;
        let (ret, params, variadic) = self.builtin_signature(info.sig);
        let has = |a: u8| info.attrs & a != 0;
        let fi =
            FuncInfo::new().with_variadic(variadic).with_noreturn(has(builtin::attr::NORETURN));
        let exc = if has(builtin::attr::NOTHROW) { ExcSpec::BasicNoexcept } else { ExcSpec::None };
        let ty = self.ast.types.function(ret, &params, fi, exc);
        let tu = self.ast.tu;
        let lh = DeclHead::new(tu, DeclName::EMPTY, loc).flags(DeclFlags::IMPLICIT);
        let ls = self.builder().linkage_spec_head(lh, LinkageLang::C);
        let h = DeclHead::new(ls, DeclName::ident(name), loc).flags(DeclFlags::IMPLICIT);
        let mut spec = FnSpec::new().with_storage(FnStorage::Extern);
        if has(builtin::attr::CONSTEVAL) {
            spec = spec.with_constexpr_kind(ConstexprKind::Consteval);
        }
        let f = self.builder().function(h, DeclKind::Function, ty, spec, None);
        let _ = self.ast.decls.cold.builtin_id.insert(f, id);
        let implicit =
            |kind, loc, a| AttrNode { kind, syntax: AttrSyntax::Gnu, flags: 1, loc, a, b: 0 };
        let mut pds = Vec::with_capacity(params.len());
        for (i, &pty) in params.iter().enumerate() {
            let idx = u16::try_from(i).unwrap_or(u16::MAX);
            let ph = DeclHead::new(f, DeclName::EMPTY, Loc::INVALID).flags(DeclFlags::IMPLICIT);
            let p = self.builder().param_head(ph, pty, idx, 0);
            if id == builtin::ADDRESSOF {
                // clang: an implicit `[[clang::lifetimebound]]` parameter.
                let lb = self.ast.attrs.push(implicit(AttrKind::LIFETIMEBOUND, loc, 0));
                let _ = self.builder().attach_attrs(p, &[lb]);
            }
            pds.push(p);
        }
        if !pds.is_empty() {
            let _ = self.builder().set_params(f, &pds);
        }
        let mut attrs = vec![self.ast.attrs.push(implicit(AttrKind::BUILTIN, Loc::INVALID, id))];
        if let Some((fmt, first)) = builtin::printf_format(id) {
            let b = u32::from(fmt) | u32::from(first) << 16;
            let node = AttrNode { b, ..implicit(AttrKind::FORMAT, loc, 0) };
            attrs.push(self.ast.attrs.push(node));
        }
        let early = has(builtin::attr::CONST_EARLY);
        for (bit, kind) in [
            (builtin::attr::CONST_EARLY, AttrKind::CONST),
            (builtin::attr::NOTHROW, AttrKind::NOTHROW),
            (builtin::attr::CONST, AttrKind::CONST),
            (builtin::attr::PURE, AttrKind::PURE),
        ] {
            if has(bit) && !(early && bit == builtin::attr::CONST) {
                attrs.push(self.ast.attrs.push(implicit(kind, loc, 0)));
            }
        }
        let _ = self.builder().attach_attrs(f, &attrs);
        // WP-630: clang `LazilyCreateBuiltin` declares a header builtin
        // (C++ has no predefined library functions, but these are not
        // ones), with `ext_implicit_lib_function_decl` and its header.
        if let Some(h) = builtin::x86::header_of(id)
            && loc != Loc::INVALID
        {
            let ty = self.ty_arg(ty);
            let msg =
                self.diag(DiagId::sema_implicit_lib_function_decl, loc).arg(info.name).arg(ty);
            let note = self.diag(DiagId::note_include_header_or_declare, loc).arg(h).arg(info.name);
            self.emit(msg.with_note(note));
        }
        Some(f)
    }

    /// The return type, parameter types and variadic bit a `Builtins.td`
    /// signature spells (clang's `DecodeTypeFromStr`; see
    /// [`builtin::Info::sig`]).
    fn builtin_signature(&mut self, sig: &str) -> (QualType, Vec<QualType>, bool) {
        let mut s = sig.as_bytes();
        let mut tys = Vec::new();
        let mut variadic = false;
        while let Some(&c) = s.first() {
            if c == b'.' {
                variadic = true;
                break;
            }
            let (t, rest) = self.builtin_sig_type(s);
            tys.push(t);
            s = rest;
        }
        let ret = if tys.is_empty() { QualType::VOID } else { tys.remove(0) };
        (ret, tys, variadic)
    }

    /// One type of a `Builtins.td` signature and the rest of the string.
    fn builtin_sig_type<'s>(&mut self, mut s: &'s [u8]) -> (QualType, &'s [u8]) {
        let (mut unsigned, mut longs, mut complex) = (false, 0, false);
        let (mut signed, mut vector) = (false, 0u32);
        while let Some(&c) = s.first() {
            match c {
                // `_Complex` of the type that follows (`__builtin_cexp`'s
                // "XdXd").
                b'X' => complex = true,
                b'U' => unsigned = true,
                b'L' => longs += 1,
                // WP-558 (the NEON prototypes): `S` signed (`Sc` is
                // `signed char`); `W` `int64_t` (`long` on both LP64
                // targets); `V<N>` a GNU vector of `N` elements.
                b'S' => signed = true,
                b'W' => longs = 1,
                b'V' => {
                    let digits = s[1..].iter().take_while(|d| d.is_ascii_digit()).count();
                    vector = std::str::from_utf8(&s[1..=digits])
                        .ok()
                        .and_then(|d| d.parse().ok())
                        .unwrap_or(1);
                    s = &s[digits..];
                }
                // An integer constant expression argument (checked by
                // `check_builtin_ice_args`); WP-724 (the ACLE
                // prototypes): `Z` `int32_t` (`Zi` is `int`, `ZUi`
                // `unsigned`).
                b'I' | b'Z' => {}
                _ => break,
            }
            s = &s[1..];
        }
        let base = s.first().copied().unwrap_or(b'v');
        s = s.get(1..).unwrap_or_default();
        let mut t = match (base, unsigned, longs) {
            (b'b', ..) => QualType::BOOL,
            (b'c', false, _) if signed => QualType::SCHAR,
            (b'c', false, _) => QualType::CHAR,
            (b'c', true, _) => QualType::UCHAR,
            (b'w', ..) => QualType::WCHAR,
            (b's', false, _) => QualType::SHORT,
            (b's', true, _) => QualType::USHORT,
            (b'i', false, 0) => QualType::INT,
            (b'i', true, 0) => QualType::UINT,
            (b'i', false, 1) => QualType::LONG,
            (b'i', true, 1) => QualType::ULONG,
            (b'i', false, 2) => QualType::LONGLONG,
            (b'i', true, 2) => QualType::ULONGLONG,
            (b'i', false, _) => QualType::INT128,
            (b'i', true, _) => QualType::UINT128,
            (b'f', ..) => QualType::FLOAT,
            (b'd', _, 0) => QualType::DOUBLE,
            (b'd', _, 1) => QualType::LONGDOUBLE,
            // `LLd`: the `*f128` builtins' type. GCC (whose libstdc++
            // `<cmath>` calls them with `_Float128` and `_Float128*`
            // arguments on x86-64) types them `_Float128`; clang 19 has
            // `__float128` (x86-64) or `long double` (AArch64), both of
            // which convert to `_Float128` by value (WP-361).
            (b'd', ..) => QualType::STD_FLOAT128,
            // `h`: `__fp16`; `x`: `_Float16`.
            (b'h', ..) => QualType::HALF,
            (b'x', ..) => QualType::FLOAT16,
            // WP-361: kiln's `k`: `_Float32`; `K`: `_Float64`.
            (b'k', ..) => QualType::FLOAT32,
            (b'K', ..) => QualType::FLOAT64,
            // `y`: `__bf16`.
            (b'y', ..) => QualType::BFLOAT16,
            (b'z', ..) => self.size_type(),
            (b'A', ..) => self.builtin_va_list_param(),
            (b'a', ..) => self.builtin_va_list_value(),
            (b'P', ..) => self.file_type(),
            _ => QualType::VOID,
        };
        if complex {
            t = self.ast.types.complex(t);
        }
        if vector != 0 {
            t = self.ast.types.vector(t, vector, VectorKind::Generic);
        }
        while let Some(&c) = s.first() {
            t = match c {
                b'C' => self.ast.types.qualify(t, Quals::CONST),
                b'D' => self.ast.types.qualify(t, Quals::VOLATILE),
                b'R' => self.ast.types.qualify(t, Quals::RESTRICT),
                b'*' => self.ast.types.pointer_to(t),
                b'&' => self.ast.types.lvalue_ref_to(t),
                _ => break,
            };
            s = &s[1..];
        }
        (t, s)
    }

    /// A call of a builtin with custom type-checking (clang's
    /// `CustomTypeChecking` builtins, checked in `CheckBuiltinFunctionCall`):
    /// the arguments are not converted to parameter types; each builtin
    /// converts and checks them and decides the call's type. Returns the
    /// call, or a recovery expression after a diagnostic.
    #[allow(clippy::too_many_arguments, reason = "one per part of the call")]
    pub(crate) fn builtin_custom_call(
        &mut self,
        callee: ExprId,
        b: DeclId,
        id: u32,
        args: &[ExprId],
        braced: bool,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut kids = vec![callee];
        kids.extend_from_slice(args);
        if id == builtin::ADDRESSOF {
            return match self.builtin_addressof_call(callee, b, args, braced, loc, rparen) {
                Some(e) => e,
                None => self.recover_err(&kids, loc),
            };
        }
        if braced {
            self.unsupported("a braced argument to a builtin function", loc);
            return self.recover_err(&kids, loc);
        }
        if id == builtin::SHUFFLEVECTOR {
            return self.act_on_shufflevector(args, self.begin_loc(callee), rparen);
        }
        if builtin::sync::op(id).is_some() {
            return match self.builtin_sync_call(callee, b, id, args, loc, rparen) {
                Some(e) => e,
                None => self.recover_err(&kids, loc),
            };
        }
        if builtin::atomic::is_op(id) {
            // WP-154: an `AtomicExpr` (no call, no callee).
            return match self.builtin_atomic_op(self.begin_loc(callee), id, args, rparen) {
                Some(e) => e,
                None => self.recover_err(&kids, loc),
            };
        }
        let r = match id {
            builtin::CLZG | builtin::CTZG => self.builtin_count_zeros_generic(args, rparen),
            builtin::POPCOUNTG => self.builtin_popcountg(args, rparen),
            // WP-270: the generic checked-arithmetic builtins.
            builtin::ADD_OVERFLOW | builtin::SUB_OVERFLOW | builtin::MUL_OVERFLOW => {
                self.builtin_overflow(args, rparen)
            }
            builtin::ISFINITE
            | builtin::ISINF
            | builtin::ISNAN
            | builtin::ISNORMAL
            | builtin::SIGNBIT
            | builtin::ISINF_SIGN
            | builtin::ISSUBNORMAL
            | builtin::ISZERO
            | builtin::ISSIGNALING => self.builtin_fp_classification(args, rparen),
            builtin::FPCLASSIFY => self.builtin_fpclassify(args, rparen),
            builtin::ISFPCLASS => self.builtin_isfpclass(callee, args, rparen),
            builtin::ISGREATER
            | builtin::ISGREATEREQUAL
            | builtin::ISLESS
            | builtin::ISLESSEQUAL
            | builtin::ISLESSGREATER
            | builtin::ISUNORDERED => self.builtin_unordered_compare(args, rparen),
            builtin::LAUNDER => self.builtin_launder(callee, args, rparen),
            builtin::IS_WITHIN_LIFETIME => self.builtin_is_within_lifetime(args, rparen),
            builtin::IS_STRING_LITERAL => self.builtin_is_string_literal(args, rparen),
            builtin::ASSUME_ALIGNED => self.builtin_assume_aligned(callee, args, rparen),
            builtin::COMPLEX => self.builtin_complex(callee, args, rparen),
            builtin::CONSTANT_P => self.builtin_constant_p(args, rparen),
            // WP-419: clang `CheckBuiltinFunctionCall`: one argument, kept
            // as written (no conversion; the call never evaluates it). A
            // bound member function or builtin name is diagnosed as in any
            // call argument; an overload set is kept (clang classifies its
            // placeholder type as an unspecified value, kiln as -1).
            builtin::CLASSIFY_TYPE => (self.builtin_arg_count(args, 1, rparen)
                && !self.check_builtin_fn_use(args[0]))
            .then(|| (args.to_vec(), QualType::INT)),
            builtin::VA_START => self.builtin_va_start(callee, args, rparen),
            // WP-156: the vector reductions (vector_ops.rs).
            builtin::REDUCE_ADD
            | builtin::REDUCE_MUL
            | builtin::REDUCE_AND
            | builtin::REDUCE_OR
            | builtin::REDUCE_XOR
            | builtin::REDUCE_MIN
            | builtin::REDUCE_MAX => self.builtin_reduce(id, args, rparen),
            // WP-416: the alignment and element-wise math builtins.
            builtin::ALIGN_UP | builtin::ALIGN_DOWN | builtin::IS_ALIGNED => {
                self.builtin_align(id, args, rparen)
            }
            builtin::ELEMENTWISE_ABS => self.builtin_elementwise_abs(args, rparen),
            builtin::ELEMENTWISE_MAX | builtin::ELEMENTWISE_MIN => {
                self.builtin_elementwise_minmax(args, rparen)
            }
            builtin::ELEMENTWISE_ADD_SAT | builtin::ELEMENTWISE_SUB_SAT => {
                self.builtin_elementwise_sat(args, rparen)
            }
            builtin::OPERATOR_NEW | builtin::OPERATOR_DELETE => {
                return self.builtin_operator_new_delete(callee, b, id, args, loc, rparen);
            }
            // WP-630.
            builtin::NONTEMPORAL_STORE | builtin::NONTEMPORAL_LOAD => {
                self.builtin_nontemporal(callee, id == builtin::NONTEMPORAL_STORE, args, rparen)
            }
            builtin::NONDETERMINISTIC_VALUE => self.builtin_nondeterministic_value(args, rparen),
            _ if let Some(load) = builtin::arm::exclusive_load(id) => {
                self.builtin_arm_exclusive(callee, load, args, rparen)
            }
            _ => {
                self.unsupported("this builtin function", loc);
                None
            }
        };
        let Some((args, ty)) = r else { return self.recover_err(&kids, loc) };
        let f = self.builtin_fn_to_ptr(callee, b);
        let call =
            self.builder().call(f, &args, ty, ValueCategory::PRValue, loc, rparen, ExprFlags::NONE);
        // A consteval builtin (`__builtin_is_within_lifetime`): an
        // immediate invocation like any other ([expr.const]/16).
        let r = self.ignore_parens(callee);
        self.check_immediate_invocation(call, b, Some(r))
    }

    /// clang's `checkArgCount`: exactly `n` arguments, else `too few`
    /// at the `)` or `too many` at the first extra argument.
    pub(crate) fn builtin_arg_count(&mut self, args: &[ExprId], n: usize, rparen: Loc) -> bool {
        if args.len() == n {
            return true;
        }
        let (id, at) = if args.len() < n {
            (DiagId::sema_call_too_few_args, rparen)
        } else {
            (DiagId::sema_call_too_many_args, self.begin_loc(args[n]))
        };
        let msg = self.diag(id, at).arg(n).arg(args.len());
        self.emit(msg);
        false
    }

    /// clang's `isUnsignedIntegerType`: `bool`, an unsigned (or
    /// unsigned-character) integer type, an unsigned `_BitInt`, or a
    /// complete unscoped enumeration with an unsigned underlying type.
    pub(crate) fn is_unsigned_integer_type(&self, q: QualType) -> bool {
        if self.ast.types.as_enum(q).is_some() {
            return conv::rank::is_unscoped_enum(self, q)
                && conv::rank::int_width(self, q).is_some()
                && !conv::rank::int_signed(self, q);
        }
        self.ast.types.is_integral(q) && !conv::rank::int_signed(self, q)
    }

    /// `__builtin_clzg` / `__builtin_ctzg` (clang's
    /// `BuiltinCountZeroBitsGeneric`): one unsigned integer argument and
    /// an optional `int` fallback for a zero value.
    fn builtin_count_zeros_generic(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        match args.len() {
            0 => {
                let msg = self.diag(DiagId::sema_call_too_few_args, rparen).arg(1).arg(0);
                self.emit(msg);
                return None;
            }
            1 | 2 => {}
            n => {
                let msg = self.diag(DiagId::sema_call_too_many_args_at_most, rparen).arg(2).arg(n);
                self.emit(msg);
                return None;
            }
        }
        let a = conv::lvalue_to_rvalue(self, args[0]);
        let t = self.ety(a);
        if !self.is_unsigned_integer_type(t) {
            let msg = self
                .diag(DiagId::sema_builtin_invalid_arg_type, self.begin_loc(a))
                .arg(0)
                .arg(0)
                .arg(self.ty_arg(t));
            self.emit(msg);
            return None;
        }
        let mut out = vec![a];
        if let Some(&fb) = args.get(1) {
            let fb = conv::usual_unary_conversions(self, fb);
            let ft = self.ety(fb);
            if !self.ast.types.same_unqualified(ft, QualType::INT) {
                let msg = self
                    .diag(DiagId::sema_builtin_invalid_arg_type, self.begin_loc(fb))
                    .arg(1)
                    .arg(1)
                    .arg(self.ty_arg(ft));
                self.emit(msg);
                return None;
            }
            out.push(fb);
        }
        Some((out, QualType::INT))
    }

    /// `__builtin_popcountg` (clang's `BuiltinPopcountg`): one unsigned
    /// integer argument.
    fn builtin_popcountg(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 1, rparen) {
            return None;
        }
        let a = conv::lvalue_to_rvalue(self, args[0]);
        let t = self.ety(a);
        if !self.is_unsigned_integer_type(t) {
            let msg = self
                .diag(DiagId::sema_builtin_invalid_arg_type, self.begin_loc(a))
                .arg(0)
                .arg(0)
                .arg(self.ty_arg(t));
            self.emit(msg);
            return None;
        }
        Some((vec![a], QualType::INT))
    }

    /// `__builtin_align_up` / `__builtin_align_down` /
    /// `__builtin_is_aligned` (clang's `BuiltinAlignment`; spec/sema.md
    /// §5.4): the source is an integer (not `bool` or an enumeration) or a
    /// non-function pointer (an array decays), else `operand of type T
    /// where arithmetic or pointer type is required`; the alignment is
    /// such an integer, else `used type T where integer is required`. A
    /// constant alignment must be in [1, 2^(width(T)-1)] and a power of
    /// two; 1 warns (`-Wtautological-compare`). The result has the
    /// (decayed, unqualified) source type, or `bool` for `is_aligned`.
    fn builtin_align(
        &mut self,
        id: u32,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 2, rparen) {
            return None;
        }
        let (src, al) = (args[0], args[1]);
        let valid_int = |s: &Self, q: QualType| {
            s.ast.types.is_integral(q) && s.ast.types.builtin(q) != Some(BuiltinType::Bool)
        };
        let st0 = self.ety(src);
        let st = if self.ast.types.is_array(st0) { conv::array_decayed(self, st0) } else { st0 };
        let ptr = self.ast.types.is_pointer(st);
        let fn_ptr =
            ptr && self.ast.types.pointee(st).is_some_and(|p| self.ast.types.is_function(p));
        if (!ptr && !valid_int(self, st)) || fn_ptr {
            let d = self
                .diag(DiagId::sema_typecheck_expect_scalar_operand, self.eloc(src))
                .arg(self.ty_arg(st));
            self.emit(d);
            return None;
        }
        let at = self.ety(al);
        if !valid_int(self, at) {
            let d = self.diag(DiagId::sema_expect_int, self.eloc(al)).arg(self.ty_arg(at));
            self.emit(d);
            return None;
        }
        let is_aligned = id == builtin::IS_ALIGNED;
        if !self.expr_is_value_dependent(al)
            && let Some(v) = self.fold_int(al)
        {
            let bits = if ptr {
                u32::try_from(self.abi.pointer_size * 8).unwrap_or(64)
            } else {
                conv::rank::int_width(self, st).unwrap_or(64)
            };
            let max = (bits <= 128).then(|| 1u128 << (bits - 1));
            let loc = self.eloc(al);
            let err = if v < 1 {
                Some(self.diag(DiagId::sema_alignment_too_small, loc).arg(1))
            } else if let Some(m) = max.filter(|&m| v.cast_unsigned() > m) {
                Some(self.diag(DiagId::sema_alignment_too_big_bits, loc).arg(m.to_string()))
            } else if v.count_ones() != 1 {
                Some(self.diag(DiagId::sema_alignment_not_power_of_two, loc))
            } else {
                None
            };
            if let Some(d) = err {
                self.emit(d);
                return None;
            }
            if v == 1 {
                let d = self
                    .diag(DiagId::sema_alignment_builtin_useless, loc)
                    .arg(usize::from(is_aligned));
                self.emit(d);
            }
        }
        let src = conv::decay(self, src);
        let al = conv::lvalue_to_rvalue(self, al);
        let ty = if is_aligned { QualType::BOOL } else { conv::strip_cv(self, st) };
        Some((vec![src, al], ty))
    }

    /// `__builtin_isfinite` & co. (clang's `BuiltinFPClassification`;
    /// C23 7.12.3): one real floating argument.
    fn builtin_fp_classification(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 1, rparen) {
            return None;
        }
        let a = conv::decay(self, args[0]);
        let t = self.ety(a);
        if !self.is_real_floating(t) {
            let msg =
                self.diag(DiagId::sema_builtin_fp_class_arg, self.begin_loc(a)).arg(self.ty_arg(t));
            self.emit(msg);
            return None;
        }
        Some((vec![a], QualType::INT))
    }

    /// The probability argument of `__builtin_expect_with_probability`
    /// (already converted to `double`): a constant expression in
    /// [0.0, 1.0] (clang `Sema::BuiltinExpectWithProbability`). A
    /// value-dependent argument is checked at instantiation.
    pub(crate) fn check_expect_probability(&mut self, p: ExprId) -> bool {
        if self.expr_is_value_dependent(p) {
            return true;
        }
        let at = self.begin_loc(p);
        let v = match self.eval_const(p, EvalContext::new(EvalKind::Other, at)) {
            Ok(v) => v,
            Err(f) => {
                let d = self.diag(DiagId::sema_probability_not_constant_float, at);
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                return false;
            }
        };
        let x = match self.ast.consts.scalar(v) {
            Some(Scalar::Float { kind, bits, .. }) if kind.bits() == 64 => {
                f64::from_bits(bits as u64)
            }
            _ => {
                let d = self.diag(DiagId::sema_probability_not_constant_float, at);
                self.emit(d);
                return false;
            }
        };
        if !(0.0..=1.0).contains(&x) {
            let d = self.diag(DiagId::sema_probability_out_of_range, at);
            self.emit(d);
            return false;
        }
        true
    }

    /// The arguments of a builtin call that must be integer constant
    /// expressions (clang `CheckBuiltinFunctionCall` -> `BuiltinConstantArg`:
    /// `argument to '<name>' must be a constant integer` at the call's
    /// start). Value-dependent arguments wait for instantiation.
    pub(crate) fn check_builtin_ice_args(
        &mut self,
        id: u32,
        callee: ExprId,
        args: &[ExprId],
    ) -> bool {
        if id == builtin::VERBOSE_TRAP {
            return self.check_verbose_trap(args);
        }
        let mask = builtin::ice_params(id);
        let x86 = builtin::x86::row(id);
        if mask == 0 && x86.is_none_or(|r| r.imms.is_empty()) {
            return true;
        }
        for (i, &a) in args.iter().enumerate().take(32) {
            if mask & (1 << i) == 0 || self.expr_is_value_dependent(a) {
                continue;
            }
            if self.fold_int(a).is_none() {
                let name = builtin::info(id).map_or("", |e| e.name);
                let at = self.begin_loc(callee);
                let d = self.diag(DiagId::sema_constant_integer_arg_type, at).arg(name.to_string());
                self.emit(d);
                return false;
            }
        }
        if let Some(r) = x86 {
            return self.check_x86_imms(r.imms, callee, args);
        }
        if id == builtin::ALLOCA_WITH_ALIGN
            && let Some(&a) = args.get(1)
            && !self.expr_is_value_dependent(a)
        {
            return self.check_alloca_align(callee, a);
        }
        if matches!(id, builtin::OBJECT_SIZE | builtin::DYNAMIC_OBJECT_SIZE)
            && let Some(&a) = args.get(1)
            && !self.expr_is_value_dependent(a)
            && let Some(v) = self.fold_int(a)
            && !(0..=3).contains(&v)
        {
            // clang `BuiltinConstantArgRange(TheCall, 1, 0, 3)`.
            let d = self
                .diag(DiagId::sema_argument_invalid_range, self.begin_loc(callee))
                .arg(v.to_string())
                .arg("0".to_string())
                .arg("3".to_string());
            self.emit(d);
            return false;
        }
        if matches!(id, builtin::FRAME_ADDRESS | builtin::RETURN_ADDRESS)
            && let Some(&a) = args.first()
            && !self.expr_is_value_dependent(a)
            && let Some(v) = self.fold_int(a)
        {
            // clang `BuiltinConstantArgRange(TheCall, 0, 0, 0xFFFF)`, then
            // `-Wframe-address` for a nonzero level.
            let at = self.begin_loc(callee);
            if !(0..=0xFFFF).contains(&v) {
                let d = self
                    .diag(DiagId::sema_argument_invalid_range, at)
                    .arg(v.to_string())
                    .arg("0".to_string())
                    .arg("65535".to_string());
                self.emit(d);
                return false;
            }
            if v != 0 {
                let name = if id == builtin::FRAME_ADDRESS {
                    "__builtin_frame_address"
                } else {
                    "__builtin_return_address"
                };
                let d = self.diag(DiagId::sema_frame_address, at).arg(name);
                self.emit(d);
            }
        }
        true
    }

    /// WP-610: `__builtin_prefetch(addr [, rw [, locality]])` (clang
    /// `Sema::BuiltinPrefetch`, after the arguments are converted): at most
    /// three arguments (`too many arguments to function call, expected at
    /// most 3, have N` at the `)`), then `rw` and `locality` integer
    /// constant expressions in [0, 1] and [0, 3] (clang
    /// `BuiltinConstantArgRange`: `argument to '__builtin_prefetch' must
    /// be a constant integer` / `argument value V is outside the valid
    /// range [0, M]` at the call's start). Value-dependent arguments wait
    /// for instantiation.
    pub(crate) fn check_prefetch(&mut self, callee: ExprId, args: &[ExprId], rparen: Loc) -> bool {
        if args.len() > 3 {
            let d =
                self.diag(DiagId::sema_call_too_many_args_at_most, rparen).arg(3).arg(args.len());
            self.emit(d);
            return false;
        }
        let at = self.begin_loc(callee);
        for (i, &a) in args.iter().enumerate().skip(1) {
            if self.expr_is_value_dependent(a) {
                continue;
            }
            let max: i128 = if i == 1 { 1 } else { 3 };
            let Some(v) = self.fold_int(a) else {
                let d =
                    self.diag(DiagId::sema_constant_integer_arg_type, at).arg("__builtin_prefetch");
                self.emit(d);
                return false;
            };
            if !(0..=max).contains(&v) {
                let d = self
                    .diag(DiagId::sema_argument_invalid_range, at)
                    .arg(v.to_string())
                    .arg("0".to_string())
                    .arg(max.to_string());
                self.emit(d);
                return false;
            }
        }
        true
    }

    /// WP-630: clang `SemaX86::CheckBuiltinFunctionCall` for the x86
    /// builtin call with the converted arguments `args`, once the integer
    /// constant arguments are known to be constants: the 32-bit-only
    /// builtins, then the rounding / SAE, gather-scatter scale and tile
    /// checks, then the ranges, each reported at the call's start. A
    /// value-dependent argument waits for instantiation. A range outside
    /// the AMX tile numbers is `-Wargument-outside-range`, an error by
    /// default that does not make the call invalid (clang
    /// `BuiltinConstantArgRange` with `RangeIsError = false`). False after
    /// an error.
    fn check_x86_imms(
        &mut self,
        imms: &[builtin::x86::Imm],
        callee: ExprId,
        args: &[ExprId],
    ) -> bool {
        use builtin::x86::Imm;
        let at = self.begin_loc(callee);
        let value = |s: &mut Self, i: u8| {
            let a = *args.get(usize::from(i))?;
            if s.expr_is_value_dependent(a) { None } else { s.fold_int(a) }
        };
        let fail = |s: &mut Self, id: DiagId| {
            let d = s.diag(id, at);
            s.emit(d);
            false
        };
        if imms.contains(&Imm::Only32Bit) {
            return fail(self, DiagId::sema_x86_builtin_32_bit_only);
        }
        for &imm in imms {
            let (i, rc) = match imm {
                Imm::Rounding(i) => (i, true),
                Imm::Sae(i) => (i, false),
                _ => continue,
            };
            // clang: 4 (current direction) or 8 (no exceptions), with
            // rounding control also 9 to 11, without it also 12.
            let ok = |v: i128| v == 4 || v == 8 || if rc { (9..=11).contains(&v) } else { v == 12 };
            if value(self, i).is_some_and(|v| !ok(v)) {
                return fail(self, DiagId::sema_x86_invalid_rounding);
            }
        }
        for &imm in imms {
            if let Imm::Scale(i) = imm
                && value(self, i).is_some_and(|v| !matches!(v, 1 | 2 | 4 | 8))
            {
                return fail(self, DiagId::sema_x86_invalid_scale);
            }
        }
        let ranges = |imm: &Imm| match *imm {
            Imm::Range(i, lo, hi) => Some((i, lo, hi, false)),
            Imm::RangeError(i, lo, hi) => Some((i, lo, hi, true)),
            _ => None,
        };
        // clang `CheckBuiltinTileArguments`: the tile numbers' ranges, then
        // `CheckBuiltinTileDuplicate`.
        let mut tiles = Vec::new();
        for (i, lo, hi, hard) in imms.iter().filter_map(ranges) {
            let Some(v) = value(self, i) else { continue };
            if !(i128::from(lo)..=i128::from(hi)).contains(&v) {
                let id = if hard {
                    DiagId::sema_argument_invalid_range
                } else {
                    DiagId::sema_argument_outside_range
                };
                let d = self.diag(id, at).arg(v.to_string());
                let d = d.arg(lo.to_string()).arg(hi.to_string());
                self.emit(d);
                if hard {
                    return false;
                }
            }
            if hard {
                tiles.push(v);
            }
        }
        if imms.contains(&Imm::DistinctTiles)
            && tiles.iter().enumerate().any(|(k, t)| tiles[..k].contains(t))
        {
            return fail(self, DiagId::sema_x86_tile_duplicate);
        }
        true
    }

    /// clang `checkBuiltinVerboseTrap`: each argument must evaluate to a
    /// constant string without a `$`, else `argument to
    /// __builtin_verbose_trap must {be a pointer to a constant string|not
    /// contain $}` at the argument. A string literal (through the
    /// decay) is read directly; any other argument must constant-fold,
    /// and its characters are not inspected (spec/sema.md §5.4).
    fn check_verbose_trap(&mut self, args: &[ExprId]) -> bool {
        let mut ok = true;
        for &a in args {
            if self.expr_is_value_dependent(a) {
                continue;
            }
            let inner = self.ignore_paren_imp_casts(a);
            let kind = if let Some(lit) = self.string_lit_of(inner) {
                let sid = self.ast.exprs.string_lit(lit);
                let bytes = self.ast.strings.bytes(sid);
                // The stored bytes include the trailing NUL only.
                bytes.contains(&b'$').then_some(1)
            } else if self.try_fold(a).is_some() {
                None
            } else {
                Some(0)
            };
            if let Some(k) = kind {
                let d = self.diag(DiagId::sema_builtin_verbose_trap_arg, self.begin_loc(a)).arg(k);
                self.emit(d);
                ok = false;
            }
        }
        ok
    }

    /// `__builtin_assume_aligned(p, align[, offset])` (clang
    /// `BuiltinAssumeAligned`): 2 or 3 arguments; `p` decays and must
    /// convert to `const void *` (it keeps its own type); `align` is an
    /// integer (or unscoped enumeration) constant power of 2 (left
    /// unconverted), warned about above 2^32
    /// (`-Wbuiltin-assume-aligned-alignment`); `offset` converts to
    /// `size_t`. The call is a `void *` prvalue. Alignment errors are at
    /// the call's start.
    fn builtin_assume_aligned(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if args.len() < 2 {
            let d = self.diag(DiagId::sema_call_too_few_args, rparen).arg(2).arg(args.len());
            self.emit(d);
            return None;
        }
        if args.len() > 3 {
            // clang `checkArgCountAtMost`: at the call's end.
            let d =
                self.diag(DiagId::sema_call_too_many_args_at_most, rparen).arg(3).arg(args.len());
            self.emit(d);
            return None;
        }
        let cvp = self.ast.types.qualify(QualType::VOID, Quals::CONST);
        let cvp = self.ast.types.pointer_to(cvp);
        let p = args[0];
        // clang 22: the decayed first argument must be a pointer (not
        // `std::nullptr_t` nor a class converting to one).
        if self.clang_compat >= 22 {
            let dp = conv::decay(self, p);
            if !self.ast.types.is_pointer(self.ety(dp)) && !self.is_err_expr(dp) {
                let d = self
                    .diag(DiagId::sema_builtin_assume_aligned_non_pointer, self.begin_loc(callee))
                    .range(self.expr_range(callee));
                self.emit(d);
                return None;
            }
        }
        if let Err(e) = conv::implicit_convert(self, p, cvp, ConvCtx::Argument) {
            let d = e.diag_at(self, self.begin_loc(p));
            self.emit(d);
            return None;
        }
        let p = conv::decay(self, p);
        let al = args[1];
        if !self.expr_is_value_dependent(al) {
            let at = self.begin_loc(callee);
            // clang `BuiltinConstantArg`: an integral or unscoped enumeration
            // constant (LLVM's `PickAlignment<T, A>::value` enumerator).
            let v = if conv::rank::is_integral_or_unscoped_enum(self, self.ety(al)) {
                self.fold_int(al)
            } else {
                None
            };
            let Some(v) = v else {
                let d = self
                    .diag(DiagId::sema_constant_integer_arg_type, at)
                    .arg("__builtin_assume_aligned".to_string());
                self.emit(d);
                return None;
            };
            if v <= 0 || (v & (v - 1)) != 0 {
                self.error(DiagId::sema_alignment_not_power_of_two, at);
                return None;
            }
            if v > 1 << 32 {
                let d = self
                    .diag(DiagId::sema_builtin_assume_aligned_too_big, at)
                    .arg((1u64 << 32).to_string());
                self.emit(d);
            }
        }
        let mut out = vec![p, al];
        if let Some(&off) = args.get(2) {
            let sz = self.size_type();
            match conv::implicit_convert(self, off, sz, ConvCtx::Argument) {
                Ok(x) => out.push(x),
                Err(e) => {
                    let d = e.diag_at(self, self.begin_loc(off));
                    self.emit(d);
                    return None;
                }
            }
        }
        let vp = self.ast.types.pointer_to(QualType::VOID);
        Some((out, vp))
    }

    /// The alignment of `__builtin_alloca_with_align`, in bits (clang
    /// `BuiltinAllocaWithAlign`): a power of 2, at least `CHAR_BIT`, at
    /// most `INT32_MAX`; reported at the call's start.
    fn check_alloca_align(&mut self, callee: ExprId, a: ExprId) -> bool {
        let Some(v) = self.fold_int(a) else { return true };
        let id = if v <= 0 || (v & (v - 1)) != 0 {
            (DiagId::sema_alignment_not_power_of_two, None)
        } else if v < 8 {
            (DiagId::sema_alignment_too_small, Some("8"))
        } else if v > i128::from(i32::MAX) {
            (DiagId::sema_alignment_too_big_bits, Some("2147483647"))
        } else {
            return true;
        };
        let mut d = self.diag(id.0, self.begin_loc(callee));
        if let Some(n) = id.1 {
            d = d.arg(n.to_string());
        }
        self.emit(d);
        false
    }

    /// `__builtin_{add,sub,mul}_overflow(a, b, r)` (clang's
    /// `BuiltinOverflow`; WP-270): `a` and `b` integer (`bool`, an
    /// unscoped enumeration) after the decay / lvalue-to-rvalue
    /// conversions and without promotion, `r` a pointer to a non-const
    /// integer type; the call is a `bool` prvalue. Each failure is
    /// reported at the argument's start; the first one stops the check.
    fn builtin_overflow(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 3, rparen) {
            return None;
        }
        let mut out = Vec::with_capacity(3);
        for (i, &a) in args.iter().enumerate() {
            let a = conv::decay(self, a);
            let t = self.ety(a);
            if t == QualType::ERROR {
                return None;
            }
            let (ok, id) = if i < 2 {
                (self.is_integer_type(t), DiagId::sema_overflow_builtin_int)
            } else {
                let p = self.ast.types.pointee(t).filter(|_| self.ast.types.is_pointer(t));
                let ok = p.is_some_and(|p| {
                    self.is_integer_type(p) && !self.ast.types.effective_quals(p).is_const()
                });
                (ok, DiagId::sema_overflow_builtin_ptr_int)
            };
            if !ok {
                let msg = self.diag(id, self.begin_loc(a)).arg(self.ty_arg(t));
                self.emit(msg);
                return None;
            }
            out.push(a);
        }
        Some((out, QualType::BOOL))
    }

    /// clang's `isIntegerType`: an integral type (`bool`, the character
    /// types, `_BitInt`) or a complete unscoped enumeration.
    pub(crate) fn is_integer_type(&self, q: QualType) -> bool {
        if self.ast.types.as_enum(q).is_some() {
            return conv::rank::is_unscoped_enum(self, q)
                && conv::rank::int_width(self, q).is_some();
        }
        self.ast.types.is_integral(q)
    }

    /// `__builtin_isfpclass(x, mask)` (clang's `BuiltinFPClassification`
    /// with `IsFPClass`): `x` real floating, `mask` an integer constant
    /// in [0, 1023] (`llvm::fcAllFlags`), left unconverted; reported at
    /// the call's start (clang `BuiltinConstantArgRange`).
    fn builtin_isfpclass(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 2, rparen) {
            return None;
        }
        let (mut out, _) = self.builtin_fp_classification(&args[..1], rparen)?;
        let m = args[1];
        if !self.expr_is_value_dependent(m) {
            let at = self.begin_loc(callee);
            let v = if self.ast.types.is_integral(self.ety(m)) { self.fold_int(m) } else { None };
            let Some(v) = v else {
                let msg = self
                    .diag(DiagId::sema_constant_integer_arg_type, at)
                    .arg("__builtin_isfpclass".to_string());
                self.emit(msg);
                return None;
            };
            if !(0..=1023).contains(&v) {
                let msg = self
                    .diag(DiagId::sema_argument_invalid_range, at)
                    .arg(v.to_string())
                    .arg("0".to_string())
                    .arg("1023".to_string());
                self.emit(msg);
                return None;
            }
        }
        out.push(m);
        Some((out, QualType::INT))
    }

    /// `__builtin_fpclassify(nan, inf, normal, subnormal, zero, x)`
    /// (clang's `BuiltinFPClassification` with six arguments; C23
    /// 7.12.3.1): the five values are converted to `int` as arguments,
    /// `x` must be real floating.
    fn builtin_fpclassify(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 6, rparen) {
            return None;
        }
        let mut out = Vec::with_capacity(6);
        for &a in &args[..5] {
            if let Ok(x) = conv::implicit_convert(self, a, QualType::INT, ConvCtx::Argument) {
                out.push(x);
            } else {
                // clang `PerformImplicitConversion(.., AA_Passing)`:
                // `DiagnoseAssignmentResult`'s incompatible case.
                let t = self.ety(a);
                let msg = self
                    .diag(DiagId::sema_builtin_passing_incompatible, self.begin_loc(a))
                    .arg(self.ty_arg(t))
                    .arg(self.ty_arg(QualType::INT));
                self.emit(msg);
                return None;
            }
        }
        let (mut x, _) = self.builtin_fp_classification(&args[5..], rparen)?;
        out.append(&mut x);
        Some((out, QualType::INT))
    }

    /// `__builtin_isgreater` & co. (clang's `BuiltinUnorderedCompare`;
    /// C23 7.12.17): the usual arithmetic conversions to a real floating
    /// common type.
    fn builtin_unordered_compare(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 2, rparen) {
            return None;
        }
        let l = conv::usual_unary_conversions(self, args[0]);
        let r = conv::usual_unary_conversions(self, args[1]);
        let (lt, rt) = (self.ety(l), self.ety(r));
        let arith = |s: &Self, q| conv::rank::is_arithmetic_or_unscoped_enum(s, q);
        let (l, r, common) = if arith(self, lt) && arith(self, rt) {
            match conv::usual_arithmetic_conversions_kind(
                self,
                l,
                r,
                conv::ArithConvKind::Comparison,
            ) {
                Ok(x) => {
                    let at = self.ast.exprs.get(l).loc();
                    self.report_arith_warning(at);
                    x
                }
                Err(e) => {
                    let msg = e.diag(self, l);
                    self.emit(msg);
                    return None;
                }
            }
        } else {
            (l, r, QualType::ERROR)
        };
        if !self.is_real_floating(common) {
            let msg = self
                .diag(DiagId::sema_builtin_ordered_compare, self.begin_loc(l))
                .arg(self.ty_arg(self.ety(l)))
                .arg(self.ty_arg(self.ety(r)));
            self.emit(msg);
            return None;
        }
        Some((vec![l, r], QualType::INT))
    }

    /// A real (not complex) floating type.
    fn is_real_floating(&self, q: QualType) -> bool {
        self.ast.types.builtin(q).is_some_and(BuiltinType::is_floating)
    }

    /// `__builtin_launder` ([ptr.launder]; clang's `BuiltinLaunder`): one
    /// argument of pointer-to-complete-object type (an array or function
    /// argument decays), copy-initialized to that type, which is the
    /// call's.
    /// `__builtin_constant_p(e)` (clang `CheckBuiltinFunctionCall`): exactly one argument, which undergoes
    /// `DefaultFunctionArrayLvalueConversion`; the result is `int`.
    fn builtin_constant_p(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 1, rparen) {
            return None;
        }
        let a = conv::decay(self, args[0]);
        Some((vec![a], QualType::INT))
    }

    fn builtin_launder(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 1, rparen) {
            return None;
        }
        let at = self.begin_loc(callee);
        let a = args[0];
        let t = self.ety(a);
        let pt = if self.ast.types.is_array(t) {
            conv::array_decayed(self, t)
        } else if self.ast.types.is_function(t) {
            self.ast.types.pointer_to(t)
        } else {
            t
        };
        let pointee = self.ast.types.pointee(pt).filter(|_| self.ast.types.is_pointer(pt));
        let bad = match pointee {
            None => Some(0),
            Some(p) if self.ast.types.is_function(p) => Some(1),
            Some(p) if self.ast.types.is_void(p) => Some(2),
            Some(_) => None,
        };
        if let Some(which) = bad {
            let msg = self.diag(DiagId::sema_builtin_launder_invalid_arg, at).arg(which);
            self.emit(msg);
            return None;
        }
        let pointee = pointee.unwrap_or(QualType::ERROR);
        if !self.require_complete(pointee, at) {
            let msg =
                self.diag(DiagId::sema_incomplete_type_required, at).arg(self.ty_arg(pointee));
            self.emit(self.incomplete_note(msg, pointee));
            return None;
        }
        let pt = conv::strip_cv(self, pt);
        match conv::implicit_convert(self, a, pt, ConvCtx::Argument) {
            Ok(x) => Some((vec![x], pt)),
            Err(err) => {
                let msg = err.diag(self, a);
                self.emit(msg);
                None
            }
        }
    }

    /// `__builtin_is_within_lifetime(p)` ([meta.const.eval]/4; clang's
    /// `BuiltinIsWithinLifetime`): one argument, decayed and converted to
    /// an rvalue, of object or `void` pointer type (LWG4138: not a
    /// function pointer), diagnosed at the argument's expression
    /// location; the call is `bool`. The argument keeps its type.
    fn builtin_is_within_lifetime(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 1, rparen) {
            return None;
        }
        let a = conv::decay(self, args[0]);
        let t = self.ety(a);
        if t == QualType::ERROR {
            return None;
        }
        let pointee = self.ast.types.pointee(t).filter(|_| self.ast.types.is_pointer(t));
        let bad = match pointee {
            None => Some(0),
            Some(p) if self.ast.types.is_function(p) => Some(1),
            Some(_) => None,
        };
        if let Some(which) = bad {
            let at = self.expr_loc(a);
            let msg = self.diag(DiagId::sema_builtin_is_within_lifetime_invalid_arg, at).arg(which);
            self.emit(msg);
            return None;
        }
        Some((vec![a], QualType::BOOL))
    }

    /// `__builtin_nontemporal_store(v, p)` / `__builtin_nontemporal_load(p)`
    /// (clang `BuiltinNontemporalOverloaded`; WP-630): `p` decayed must be
    /// a pointer to an integer, floating, pointer or vector type (else an
    /// error at the callee); a store converts `v` to the unqualified
    /// pointee and is `void`, a load has the unqualified pointee's type.
    fn builtin_nontemporal(
        &mut self,
        callee: ExprId,
        store: bool,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        let n = if store { 2 } else { 1 };
        if !self.builtin_arg_count(args, n, rparen) {
            return None;
        }
        let at = self.begin_loc(callee);
        let p = conv::decay(self, args[n - 1]);
        let pt = self.ety(p);
        if pt == QualType::ERROR {
            return None;
        }
        let Some(vt) = self.ast.types.pointee(pt).filter(|_| self.ast.types.is_pointer(pt)) else {
            let msg = self.diag(DiagId::sema_nontemporal_not_pointer, at).arg(self.ty_arg(pt));
            self.emit(msg);
            return None;
        };
        let vt = conv::strip_cv(self, vt);
        let t = &self.ast.types;
        let vector = matches!(t.canonical_view(vt), kiln_ast::types::TypeView::Vector { .. });
        if !(t.is_integral(vt) || t.is_pointer(vt) || t.is_floating(vt) || vector) {
            let msg = self.diag(DiagId::sema_nontemporal_bad_pointee, at).arg(self.ty_arg(pt));
            self.emit(msg);
            return None;
        }
        if !store {
            return Some((vec![p], vt));
        }
        let v = args[0];
        match conv::implicit_convert(self, v, vt, ConvCtx::Argument) {
            Ok(x) => Some((vec![x, p], QualType::VOID)),
            Err(err) => {
                let msg = err.diag(self, v);
                self.emit(msg);
                None
            }
        }
    }

    /// `__builtin_arm_ldrex(p)` / `ldaex` and `__builtin_arm_strex(v, p)` /
    /// `stlex` (clang `CheckARMBuiltinExclusiveCall`; WP-724): `p` decayed
    /// must be a pointer (else an error at the callee) to an integer,
    /// floating or pointer type (else an error naming the address as clang
    /// converts it, `const volatile T *` / `volatile T *`); a load has the
    /// pointee's type, qualifiers included; a store converts `v` to the
    /// unqualified pointee and is `int`.
    fn builtin_arm_exclusive(
        &mut self,
        callee: ExprId,
        load: bool,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        let n = if load { 1 } else { 2 };
        if !self.builtin_arg_count(args, n, rparen) {
            return None;
        }
        let at = self.begin_loc(callee);
        let p = conv::decay(self, args[n - 1]);
        let pt = self.ety(p);
        if pt == QualType::ERROR {
            return None;
        }
        let Some(vt) = self.ast.types.pointee(pt).filter(|_| self.ast.types.is_pointer(pt)) else {
            let msg = self
                .diag(DiagId::sema_atomic_builtin_must_be_pointer, at)
                .arg(self.ty_arg(pt))
                .arg(0);
            self.emit(msg);
            return None;
        };
        let qvt = vt;
        let vt = conv::strip_cv(self, vt);
        let t = &self.ast.types;
        if !(t.is_integral(vt) || t.is_pointer(vt) || t.is_floating(vt)) {
            // clang names the address as converted: a pointer to the
            // volatile (and, for a load, const) unqualified pointee.
            let q = if load { Quals::CONST | Quals::VOLATILE } else { Quals::VOLATILE };
            let at_ty = self.ast.types.qualify(vt, q);
            let at_ty = self.ast.types.pointer_to(at_ty);
            let msg = self
                .diag(DiagId::sema_atomic_builtin_must_be_pointer_intfltptr, at)
                .arg(self.ty_arg(at_ty));
            self.emit(msg);
            return None;
        }
        if load {
            return Some((vec![p], qvt));
        }
        let v = args[0];
        match conv::implicit_convert(self, v, vt, ConvCtx::Argument) {
            Ok(x) => Some((vec![x, p], QualType::INT)),
            Err(err) => {
                let msg = err.diag(self, v);
                self.emit(msg);
                None
            }
        }
    }

    /// `__builtin_nondeterministic_value(x)` (clang
    /// `BuiltinNonDeterministicValue`; WP-630): one argument of a builtin
    /// (not a pointer or enumeration) or vector type, kept as written; the
    /// call has its type.
    fn builtin_nondeterministic_value(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 1, rparen) {
            return None;
        }
        let a = args[0];
        let t = self.ety(a);
        if t == QualType::ERROR {
            return None;
        }
        let view = self.ast.types.canonical_view(t);
        let vector = matches!(view, kiln_ast::types::TypeView::Vector { .. });
        if !(self.ast.types.builtin(t).is_some() || vector) {
            let id = if self.clang_compat >= 22 {
                DiagId::sema_nondeterministic_value_arg
            } else {
                DiagId::sema_nondeterministic_value_arg_19
            };
            let msg = self.diag(id, self.begin_loc(a)).arg(self.ty_arg(t));
            self.emit(msg);
            return None;
        }
        Some((vec![a], self.non_reference(t)))
    }

    /// `__builtin_is_string_literal(p)` (GCC 16, WP-484): one pointer to a
    /// (cv) character type, else `sema_builtin_is_string_literal_arg`.
    fn builtin_is_string_literal(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        use kiln_ast::types::BuiltinType as B;
        if !self.builtin_arg_count(args, 1, rparen) {
            return None;
        }
        let a = conv::decay(self, args[0]);
        let t = self.ety(a);
        if t == QualType::ERROR {
            return None;
        }
        let ok = self.ast.types.is_pointer(t)
            && self.ast.types.pointee(t).is_some_and(|p| {
                let p = self.ast.types.canonical(p);
                matches!(
                    self.ast.types.builtin(p),
                    Some(B::Char | B::WChar | B::Char8 | B::Char16 | B::Char32)
                )
            });
        if !ok {
            let at = self.expr_loc(a);
            let msg = self.diag(DiagId::sema_builtin_is_string_literal_arg, at);
            self.emit(msg);
            return None;
        }
        Some((vec![a], QualType::BOOL))
    }

    /// `__builtin_va_start(ap, last)` ([cstdarg.syn]/1, C 7.16.1.4; clang's
    /// `BuiltinVAStart`): two arguments; `ap` binds to the
    /// `__builtin_va_list` parameter; the call must be in a variadic
    /// function, else `'va_start' used in function with fixed args` (or
    /// `cannot be used outside a function`) at the callee. `last` is kept
    /// as written; one that is not the last named parameter, or that
    /// names a parameter of reference or promotable type, gets a
    /// `-Wvarargs` warning (undefined behavior, [cstdarg.syn]/1).
    fn builtin_va_start(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 2, rparen) {
            return None;
        }
        let pt = self.builtin_va_list_param();
        let ap = match conv::implicit_convert(self, args[0], pt, ConvCtx::Argument) {
            Ok(x) => x,
            Err(err) => {
                let msg = err.diag(self, args[0]);
                self.emit(msg);
                return None;
            }
        };
        let at = self.begin_loc(callee);
        let f = self.ctx;
        if !self.ast.decls.get(f).kind().is_function() {
            let msg = self.diag(DiagId::sema_va_start_outside_function, at);
            self.emit(msg);
            return None;
        }
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        if !self.ast.types.proto(fty).is_some_and(|p| p.info().variadic()) {
            let msg = self.diag(DiagId::sema_va_start_fixed_function, at);
            self.emit(msg);
            return None;
        }
        let last =
            self.ast.decls.function(f).and_then(|x| self.ast.lists.iter::<DeclId>(x.params).last());
        let x = self.ignore_paren_imp_casts(args[1]);
        let named = Some(x)
            .filter(|&x| self.ast.exprs.get(x).kind() == ExprKind::DeclRefExpr)
            .map(|x| self.ast.exprs.decl(x))
            .filter(|&d| self.ast.decls.get(d).kind() == DeclKind::ParmVar);
        let arg_at = self.begin_loc(args[1]);
        match named {
            Some(p) if Some(p) == last => {
                let t = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                let promotable = self.ast.types.canonical(t).without_quals() == QualType::FLOAT
                    || conv::rank::promoted_type(self, t, None).is_some_and(|q| {
                        self.ast.types.canonical(q) != self.ast.types.canonical(t)
                    });
                let reason = if self.ast.types.is_reference(t) {
                    Some(1)
                } else if promotable {
                    Some(0)
                } else {
                    None
                };
                if let Some(r) = reason {
                    let msg = self.diag(DiagId::sema_va_start_type_is_undefined, arg_at).arg(r);
                    let pl = self.ast.decls.get(p).loc();
                    let note =
                        self.diag(DiagId::note_va_start_parameter_type, pl).arg(self.ty_arg(t));
                    self.emit(msg.with_note(note));
                }
            }
            _ => {
                let msg = self.diag(DiagId::sema_va_start_not_last_named_param, arg_at);
                self.emit(msg);
            }
        }
        Some((vec![ap, args[1]], QualType::VOID))
    }

    /// `__builtin_va_arg(list, T)` ([cstdarg.syn]/1, C 7.16.1.1; clang's
    /// `BuildVAArgExpr`). The `va_list` operand: on a target whose
    /// `va_list` is an array (x86-64) it decays (the usual unary
    /// conversions) and must then have the decayed type, else `first
    /// argument to 'va_arg' is of type 'T' and not 'va_list'`; on one
    /// whose `va_list` is a class (`AArch64`) it is copy-initialized to a
    /// `__builtin_va_list &` parameter (so a mismatch is a reference
    /// binding error). A non-dependent `T` must be complete and not
    /// abstract; a non-POD `T` (a reference included) is a
    /// `-Wnon-pod-varargs` error that does not stop the expression; a
    /// promotable `T` (an integer type of rank below `int`, an unscoped
    /// enumeration whose promoted type is not its underlying type, or
    /// `float`) gets the `-Wvarargs` undefined-behavior warning as a
    /// runtime-behavior diagnostic (`diag_runtime_behavior`). Every check
    /// reports at `ty_loc`, where `T` begins. The result is a prvalue of
    /// `T`'s non-lvalue expression type (clang's `getNonLValueExprType`:
    /// a reference's referent; cv dropped for a non-class non-array).
    pub fn act_on_va_arg(
        &mut self,
        e: ExprId,
        ty: QualType,
        ty_loc: Loc,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        if self.is_err_expr(e) || self.is_error_type(ty) {
            return self.recover_err(&[e], loc);
        }
        let Some(vl) = self.va_list_type else { return self.recover_err(&[e], loc) };
        let mut list = e;
        if !self.expr_is_type_dependent(e) {
            if self.ast.types.is_array(vl) {
                let orig = self.ety(e);
                list = conv::usual_unary_conversions(self, e);
                let want = conv::array_decayed(self, vl);
                if self.ast.types.canonical(self.ety(list)) != self.ast.types.canonical(want) {
                    let at = self.begin_loc(e);
                    let msg = self.diag(DiagId::sema_va_arg_not_va_list, at).arg(self.ty_arg(orig));
                    self.emit(msg);
                    return self.recover_err(&[list], loc);
                }
            } else {
                let pt = self.ast.types.lvalue_ref_to(vl);
                list = match conv::implicit_convert(self, e, pt, ConvCtx::Argument) {
                    Ok(x) => x,
                    Err(err) => {
                        let msg = err.diag(self, e);
                        self.emit(msg);
                        return self.recover_err(&[e], loc);
                    }
                };
            }
        }
        if !self.ast.types.is_dependent(ty) {
            if !self.require_complete(ty, ty_loc) {
                let msg = self.diag(DiagId::sema_va_arg_incomplete, ty_loc).arg(self.ty_arg(ty));
                self.emit(self.incomplete_note(msg, ty));
                return self.recover_err(&[list], loc);
            }
            if let Some(c) = self.abstract_class(ty) {
                let msg = self.diag(DiagId::sema_va_arg_abstract, ty_loc).arg(self.ty_arg(ty));
                let msg = self.pure_virtual_notes(msg, c);
                self.emit(msg);
                return self.recover_err(&[list], loc);
            }
            if self.ast.types.is_reference(ty) || !self.is_pod_type(ty) {
                let msg = self.diag(DiagId::sema_va_arg_not_pod, ty_loc).arg(self.ty_arg(ty));
                self.emit(msg);
            }
            if let Some(p) = self.va_arg_promotion(ty) {
                let msg = self
                    .diag(DiagId::sema_va_arg_never_compatible, ty_loc)
                    .arg(self.ty_arg(ty))
                    .arg(self.ty_arg(p));
                self.diag_runtime_behavior(msg);
            }
        }
        let mut rty = self.non_reference(ty);
        if !self.ast.types.is_reference(ty)
            && self.ast.types.as_record(rty).is_none()
            && !self.ast.types.is_array(rty)
        {
            rty = conv::strip_cv(self, rty);
        }
        self.builder().va_arg_expr(list, ty, rty, ty_loc, loc, rparen)
    }

    /// The type a variadic argument read as `t` was promoted to when `t`
    /// is not that type (clang `BuildVAArgExpr`'s `PromoteType`): an
    /// integral promotion ([conv.prom]) whose result is not `t` (for an
    /// unscoped enumeration: not its underlying type), and `double` for
    /// `float` ([expr.call]/13). clang's second test, a promoted type
    /// differing from the underlying type only in signedness, never
    /// holds for a standard promotable type, so it is not repeated.
    fn va_arg_promotion(&mut self, t: QualType) -> Option<QualType> {
        let canon = |s: &Self, q: QualType| s.ast.types.canonical(q).without_quals();
        if canon(self, t) == QualType::FLOAT {
            return Some(QualType::DOUBLE);
        }
        let p = conv::rank::promoted_type(self, t, None)?;
        let under = if self.ast.types.as_enum(t).is_some() {
            conv::rank::enum_data(self, t).and_then(|e| e.int_type).unwrap_or(t)
        } else {
            t
        };
        (canon(self, p) != canon(self, under)).then_some(p)
    }

    /// `__builtin_operator_new` / `__builtin_operator_delete` (clang's
    /// `BuiltinOperatorNewDeleteOverloaded`): overload resolution among
    /// the global `operator new` / `operator delete` functions
    /// ([basic.stc.dynamic.allocation], [basic.stc.dynamic.deallocation])
    /// that must select a replaceable one ([new.delete]); the call has its
    /// return type, the arguments are copy-initialized to its parameters,
    /// and the callee's `BuiltinFnToFnPtr` cast has its function type.
    fn builtin_operator_new_delete(
        &mut self,
        callee: ExprId,
        b: DeclId,
        id: u32,
        args: &[ExprId],
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut kids = vec![callee];
        kids.extend_from_slice(args);
        let delete = id == builtin::OPERATOR_DELETE;
        let op = if delete { OverloadedOperator::Delete } else { OverloadedOperator::New };
        self.declare_global_new_delete();
        let at = self.begin_loc(callee);
        let tu = self.ast.tu;
        let cands = self.ast.lookup_local(tu, DeclName::operator(op)).to_vec();
        let mut best: Option<(DeclId, usize)> = None;
        for &f in &cands {
            if self.ast.decls.get(f).kind() != DeclKind::Function {
                continue;
            }
            let Some((ps, variadic)) = self.fn_params(f) else { continue };
            if ps.len() > args.len() || (ps.len() < args.len() && !variadic) {
                continue;
            }
            let mut exact = 0;
            let ok = args.iter().zip(&ps).all(|(&a, &p)| match self.placement_arg_match(a, p) {
                Some(e) => {
                    exact += usize::from(e);
                    true
                }
                None => false,
            });
            if ok && best.is_none_or(|(_, e)| exact > e) {
                best = Some((f, exact));
            }
        }
        let name = format!("operator {}", if delete { "delete" } else { "new" });
        let Some((f, _)) = best else {
            let msg = self.diag(DiagId::sema_ovl_no_viable_function_in_call, at).arg(&name);
            let msg = self.builtin_new_delete_notes(msg, &cands, args);
            self.emit(msg);
            return self.recover_err(&kids, loc);
        };
        if !self.is_replaceable_global_alloc(f, delete) {
            let msg =
                self.diag(DiagId::sema_builtin_new_delete_not_usual, at).arg(usize::from(delete));
            let fl = self.ast.decls.get(f).loc();
            let note = self
                .diag(DiagId::note_builtin_non_usual_declared_here, fl)
                .arg(format!("'{name}'"));
            self.emit(msg.with_note(note));
            return self.recover_err(&kids, loc);
        }
        let Some((ps, _)) = self.fn_params(f) else { return self.recover_err(&kids, loc) };
        let mut out = Vec::with_capacity(args.len());
        for (&a, &p) in args.iter().zip(&ps) {
            let p = conv::strip_cv(self, p);
            match conv::implicit_convert(self, a, p, ConvCtx::Argument) {
                Ok(x) => out.push(x),
                Err(err) => {
                    let msg = err.diag(self, a);
                    self.emit(msg);
                    return self.recover_err(&kids, loc);
                }
            }
        }
        self.mark_used(f);
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let _ = b;
        let cal = conv::raw_cast(
            self,
            CastKind::BuiltinFnToFnPtr,
            callee,
            fty,
            ValueCategory::PRValue,
            None,
        );
        let ret =
            self.ast.types.proto(fty).map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
        let ret = conv::strip_cv(self, ret);
        self.builder().call(cal, &out, ret, ValueCategory::PRValue, loc, rparen, ExprFlags::NONE)
    }

    /// clang's `isReplaceableGlobalAllocationFunction` for a global
    /// `operator new` (`delete`) `f`: `(std::size_t` (`void*`), then
    /// optionally a `std::size_t` size (deallocation only), a
    /// `std::align_val_t` and a `const std::nothrow_t&`, in that order.
    pub(crate) fn is_replaceable_global_alloc(&self, f: DeclId, delete: bool) -> bool {
        let Some((ps, variadic)) = self.fn_params(f) else { return false };
        if variadic || ps.is_empty() {
            return false;
        }
        let t = &self.ast.types;
        let size_t = self.size_type();
        let first_ok = if delete {
            t.is_pointer(ps[0]) && t.pointee(ps[0]).is_some_and(|p| t.same_type(p, QualType::VOID))
        } else {
            t.same_type(ps[0], size_t)
        };
        if !first_ok {
            return false;
        }
        let mut rest = &ps[1..];
        if delete && rest.first().is_some_and(|&p| t.same_type(p, size_t)) {
            rest = &rest[1..];
        }
        if let (Some(&p), Some(al)) = (rest.first(), self.global_ops.align_val_t)
            && t.same_type(p, al)
        {
            rest = &rest[1..];
        }
        if let Some(&p) = rest.first()
            && self.is_nothrow_t_ref(p)
        {
            rest = &rest[1..];
        }
        rest.is_empty()
    }

    /// `const std::nothrow_t &`.
    fn is_nothrow_t_ref(&self, p: QualType) -> bool {
        let t = &self.ast.types;
        if !t.is_reference(p) {
            return false;
        }
        let Some(r) = t.pointee(p).and_then(|x| t.as_record(x)) else { return false };
        let d = self.ast.decls.get(r);
        d.name().as_ident().is_some_and(|n| self.ast.symbols().as_str(n) == "nothrow_t")
    }

    /// clang's candidate notes for a failed builtin `operator new` /
    /// `operator delete` resolution (the bad conversions first, then the
    /// arity mismatches, closest first).
    fn builtin_new_delete_notes(
        &mut self,
        mut msg: kiln_diag::Diag,
        cands: &[DeclId],
        args: &[ExprId],
    ) -> kiln_diag::Diag {
        let have = args.len();
        let mut bad: Vec<(bool, Loc, String)> = Vec::new();
        let mut arity: Vec<(usize, bool, Loc, String)> = Vec::new();
        for &f in cands {
            let Some((ps, variadic)) = self.fn_params(f) else { continue };
            let implicit = self.ast.decls.get(f).flags().contains(DeclFlags::IMPLICIT);
            let fl = if implicit { Loc::INVALID } else { self.ast.decls.get(f).loc() };
            let n = ps.len();
            if have < n || (have > n && !variadic) {
                let detail = self.arity_detail(f, n, n, have, variadic);
                arity.push((n.abs_diff(have), implicit, fl, detail));
                continue;
            }
            for (i, (&a, &p)) in args.iter().zip(&ps).enumerate() {
                if self.placement_arg_match(a, p).is_some() {
                    continue;
                }
                let detail = format!(
                    "no known conversion from {} to {} for {} argument",
                    self.ty_arg(self.ety(a)),
                    self.ty_arg(p),
                    crate::expr::ordinal(i + 1)
                );
                bad.push((implicit, fl, detail));
                break;
            }
        }
        bad.sort_by_key(|b| b.0);
        arity.sort_by_key(|a| (a.0, !a.1));
        let notes: Vec<(Loc, String)> = bad
            .into_iter()
            .map(|(_, l, d)| (l, d))
            .chain(arity.into_iter().map(|(_, _, l, d)| (l, d)))
            .collect();
        for (l, d) in notes {
            let note = self.diag(DiagId::note_ovl_candidate_not_viable, l).arg(d);
            msg = msg.with_note(note);
        }
        msg
    }

    /// A call of `__builtin_addressof` (clang's custom type-checking,
    /// `SemaBuiltinAddressof`): exactly one argument, which must be an
    /// lvalue that is not a bit-field ([specialized.addressof]); the type
    /// is a pointer to the argument's type and the argument is not
    /// converted. `None` after a diagnostic.
    pub(crate) fn builtin_addressof_call(
        &mut self,
        callee: ExprId,
        b: DeclId,
        args: &[ExprId],
        braced: bool,
        loc: Loc,
        rparen: Loc,
    ) -> Option<ExprId> {
        let at = self.begin_loc(callee);
        match args {
            [] => {
                let msg = self.diag(DiagId::sema_call_too_few_args, rparen).arg(1).arg(0);
                self.emit(msg);
                return None;
            }
            [_] => {}
            [_, extra, ..] => {
                let msg = self
                    .diag(DiagId::sema_call_too_many_args, self.begin_loc(*extra))
                    .arg(1)
                    .arg(args.len());
                self.emit(msg);
                return None;
            }
        }
        if braced {
            self.unsupported("a braced argument to __builtin_addressof", loc);
            return None;
        }
        let ty = self.check_addressable(args[0], at)?;
        let f = self.builtin_fn_to_ptr(callee, b);
        let ty = self.ast.types.pointer_to(ty);
        Some(self.builder().call(
            f,
            &[args[0]],
            ty,
            ValueCategory::PRValue,
            loc,
            rparen,
            ExprFlags::NONE,
        ))
    }

    /// Whether unqualified lookup of `name` would declare a builtin
    /// function (clang's `LookupBuiltin`): the parser then treats a call of
    /// it in a template as naming a function, not as an ADL-only name.
    pub fn is_builtin_name(&self, name: Symbol) -> bool {
        builtin::by_name_at(self.ast.symbols().as_str(name), self.clang_compat)
            .is_some_and(|id| builtin::info(id).is_some() && self.target_has_builtin(id))
    }

    /// Whether the target declares builtin `id`: the NEON builtins are
    /// `AArch64` target builtins (WP-558), the x86 ones x86-64 target
    /// builtins (WP-610, WP-630); every other builtin is
    /// target-independent.
    pub(crate) fn target_has_builtin(&self, id: u32) -> bool {
        use kiln_abi::target::Arch;
        let arch = self.abi.triple.arch;
        if builtin::neon::is_neon(id) || builtin::arm::is_arm(id) {
            return arch == Arch::AArch64;
        }
        !builtin::x86::is_x86(id) || arch == Arch::X86_64
    }

    /// The builtin function `callee` (through parentheses) names, if it
    /// names one.
    pub(crate) fn builtin_callee(&self, callee: ExprId) -> Option<DeclId> {
        let x = self.ignore_parens(callee);
        let n = self.ast.exprs.get(x);
        if n.kind() != ExprKind::DeclRefExpr {
            return None;
        }
        let d = self.ast.exprs.decl(x);
        self.ast.decls.builtin_id(d).map(|_| d)
    }

    /// The callee of a call to the builtin `d`: its name converted to a
    /// pointer to its function type (clang's `BuiltinFnToFnPtr`).
    pub(crate) fn builtin_fn_to_ptr(&mut self, callee: ExprId, d: DeclId) -> ExprId {
        let fty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let p = self.ast.types.pointer_to(fty);
        conv::raw_cast(self, CastKind::BuiltinFnToFnPtr, callee, p, ValueCategory::PRValue, None)
    }

    /// Whether `e` is a builtin function's name used other than as a
    /// callee; reports `builtin functions must be directly called`.
    pub(crate) fn check_builtin_fn_use(&mut self, e: ExprId) -> bool {
        if self.ast.exprs.get(e).ty() == QualType::BOUND_MEMBER {
            // [expr.ref]/7.3.2: a bound member function can only be called.
            self.diagnose_bound_member(e);
            return true;
        }
        if self.ast.exprs.get(e).ty() != QualType::BUILTIN_FN {
            return false;
        }
        let msg = self.diag(DiagId::sema_builtin_fn_use, self.eloc(e));
        self.emit(msg);
        true
    }
}

impl Sema<'_, '_> {
    /// `__builtin_bit_cast(ty, e)` at `loc` ([bit.cast]; clang
    /// `BuildBuiltinBitCastExpr` and `CastOperation::CheckBuiltinBitCast`):
    /// both types complete, a prvalue operand materialized, equal sizes,
    /// and both types trivially copyable. The result is a prvalue of `ty`
    /// (cv-unqualified for a non-class type) with cast kind
    /// `LValueToRValueBitCast`. The Oven evaluates it through the ABI
    /// layout (spec/oven.md "Builtins").
    pub fn act_on_builtin_bit_cast(
        &mut self,
        ty: QualType,
        e: ExprId,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        use kiln_ast::expr::{ExprFlags, StorageDuration, ValueCategory};
        let _ = rparen;
        if self.is_err_expr(e) || self.is_error_type(ty) {
            return self.recover(self.non_reference(ty), &[e], loc);
        }
        if self.ast.types.is_dependent(ty) || self.expr_is_type_dependent(e) {
            let to = self.non_reference(ty);
            return self.builder().cast(
                ExprKind::BuiltinBitCastExpr,
                CastKind::Dependent,
                e,
                to,
                ValueCategory::PRValue,
                None,
                loc,
                ExprFlags::NONE,
            );
        }
        let dest = self.non_reference(ty);
        if !self.require_complete(dest, loc) {
            let msg = self.diag(DiagId::sema_cast_to_incomplete, loc).arg(self.ty_arg(dest));
            self.emit(self.incomplete_note(msg, dest));
            return self.recover_err(&[e], loc);
        }
        let src = self.ety(e);
        if !self.require_complete(src, loc) {
            let msg = self.diag(DiagId::sema_incomplete_type_required, loc).arg(self.ty_arg(src));
            self.emit(self.incomplete_note(msg, src));
            return self.recover_err(&[e], loc);
        }
        let mut sub = e;
        if self.ast.exprs.get(e).value_category() == ValueCategory::PRValue {
            // clang's `CreateMaterializeTemporaryExpr` (its full-expression
            // needs cleanups, without side effects).
            sub = self.builder().materialize_temporary(
                e,
                src,
                false,
                StorageDuration::FullExpression,
                None,
            );
            self.cleanup.needs = true;
        }
        // A reference destination has the size of a pointer (clang's
        // `getTypeSizeInChars` of a reference type).
        let size_of = |s: &mut Self, t: QualType| {
            let t = if s.ast.types.is_reference(t) { s.ast.types.pointer_to(t) } else { t };
            crate::target::size_align(&s.ast, t, &s.abi).map_or(0, |x| x.0)
        };
        let (ss, ds) = (size_of(self, src), size_of(self, ty));
        if ss != ds {
            // clang 22 names both types; clang 19 only the sizes.
            let msg = if self.clang_compat >= 22 {
                self.diag(DiagId::sema_bit_cast_size_mismatch_types, loc)
                    .arg(self.ty_arg(src))
                    .arg(self.ty_arg(ty))
            } else {
                self.diag(DiagId::sema_bit_cast_size_mismatch, loc)
            }
            .arg(ss)
            .arg(ds);
            self.emit(msg);
            return self.recover_err(&[sub], loc);
        }
        for (t, which) in [(ty, "1"), (src, "0")] {
            // `std::meta::info` is scalar, hence trivially copyable, but it
            // has no value representation to cast through (P2996
            // [basic.types.general]: a consteval-only type).
            if self.ast.types.is_reference(t)
                || !self.is_trivially_copyable_type(t)
                || self.is_meta_info(t)
            {
                let msg = self.diag(DiagId::sema_bit_cast_not_trivially_copyable, loc).arg(which);
                self.emit(msg);
                return self.recover_err(&[sub], loc);
            }
        }
        let to = if self.ast.types.as_record(dest).is_some() || self.ast.types.is_array(dest) {
            dest
        } else {
            conv::strip_cv(self, dest)
        };
        self.builder().cast(
            ExprKind::BuiltinBitCastExpr,
            CastKind::LValueToRValueBitCast,
            sub,
            to,
            ValueCategory::PRValue,
            None,
            loc,
            ExprFlags::NONE,
        )
    }
}

// ---- source-location builtins -------------------------------------------

/// Per-TU state of the source-location builtins (clang
/// `StdSourceLocationImplDecl` and `UniqueStringLiterals`).
#[derive(Debug, Default)]
pub(crate) struct SrcLocCache {
    /// `std::source_location::__impl` once found and checked.
    impl_decl: Option<DeclId>,
    /// The string literal object every source-location string with this
    /// content points into: one `StringLiteral` expression per content
    /// (clang `getPredefinedStringLiteralFromCache`).
    strings: kiln_source::fx::FxHashMap<kiln_ast::StringLitId, ExprId>,
    /// Which builtins a parameter's default argument evaluates,
    /// transitively through the default arguments of the calls in it
    /// (bit `1 << kind`; computed once per parameter).
    param_kinds: kiln_source::fx::FxHashMap<DeclId, u8>,
    /// Uses whose `__builtin_source_location()` value holds the pretty
    /// name of a function whose return type is not deduced yet: (use,
    /// function), refined at the end of that function's body
    /// ([`Sema::finish_source_locs`]).
    deferred: Vec<(ExprId, DeclId)>,
}

/// The bit of `SourceLocKind::SourceLocStruct` in a kinds mask.
const SRC_LOC_STRUCT_BIT: u8 = 1 << SourceLocKind::SourceLocStruct as u8;

impl Sema<'_, '_> {
    /// `__builtin_FILE()`, `__builtin_FILE_NAME()`, `__builtin_FUNCTION()`,
    /// `__builtin_LINE()`, `__builtin_COLUMN()` and
    /// `__builtin_source_location()` (clang `ActOnSourceLocExpr`;
    /// [support.srcloc.cons]): a prvalue of type `const char *`,
    /// `unsigned int` or `const std::source_location::__impl *`. The values
    /// at the expression itself are computed now (§9.5); a use of a default
    /// argument holding one gets the values of the call
    /// ([`Self::note_default_arg_use`]).
    pub fn act_on_source_loc_expr(&mut self, kind: SourceLocKind, loc: Loc, rparen: Loc) -> ExprId {
        let ty = match kind {
            SourceLocKind::File | SourceLocKind::FileName | SourceLocKind::Function => {
                self.const_char_ptr(QualType::CHAR)
            }
            SourceLocKind::Line | SourceLocKind::Column => QualType::UINT,
            SourceLocKind::SourceLocStruct => match self.std_source_location_impl(loc) {
                Some(t) => {
                    let c = self.ast.types.qualify(t, Quals::CONST);
                    self.ast.types.pointer_to(c)
                }
                None => return self.recover_err(&[], loc),
            },
        };
        let ctx = self.ctx;
        let e = self.builder().source_loc_expr(kind, ty, ctx, loc, rparen);
        self.record_source_loc(e, loc, ctx, kind == SourceLocKind::SourceLocStruct);
        if matches!(kind, SourceLocKind::Function | SourceLocKind::SourceLocStruct) {
            self.mark_function_name_dependent(e, ctx);
        }
        e
    }

    /// The type `std::source_location::__impl`, spelled with its scope as
    /// clang prints it (clang `LookupStdSourceLocationImpl`): it must be a
    /// complete standard-layout class without bases whose only fields are
    /// `const char *_M_file_name`, `const char *_M_function_name` and
    /// integral `_M_line` and `_M_column`.
    fn std_source_location_impl(&mut self, loc: Loc) -> Option<QualType> {
        let found = match self.srcloc.impl_decl {
            Some(d) => Some(d),
            None => self.lookup_source_location_impl(),
        };
        let Some(d) = found else {
            self.error(DiagId::sema_std_source_location_impl_not_found, loc);
            return None;
        };
        if self.srcloc.impl_decl.is_none() {
            if !self.source_location_impl_ok(d) {
                self.error(DiagId::sema_std_source_location_impl_malformed, loc);
                return None;
            }
            self.srcloc.impl_decl = Some(d);
        }
        let rec = self.ast.types.canonical(self.tag_type(d));
        let outer = self.ast.decls.get(d).ctx()?;
        let ns = self.ast.decls.get(outer).ctx()?;
        let outer_ty = self.tag_type(outer);
        let nns_ns = self.ast.types.nns(Nns::Namespace { prefix: None, ns: self.ast.first(ns) });
        let nns = self.ast.types.nns(Nns::Type {
            prefix: Some(nns_ns),
            ty: outer_ty,
            template_kw: false,
        });
        Some(self.ast.types.elaborated(TagKeyword::None, Some(nns), rec))
    }

    /// The complete class `std::source_location::__impl`, if declared.
    fn lookup_source_location_impl(&self) -> Option<DeclId> {
        let std = self.intern("std");
        let ns = self
            .lookup_in(self.ast.tu, std, LookupKind::Nested)
            .single()
            .filter(|&d| self.ast.decls.get(d).kind() == DeclKind::Namespace)?;
        let sl = self.intern("source_location");
        let outer = self.lookup_qualified(ns, sl, LookupKind::Tag).single()?;
        let outer = self.ast.definition(outer)?;
        let imp = self.intern("__impl");
        let d = self.lookup_qualified(outer, imp, LookupKind::Tag).single()?;
        let d = self.ast.definition(d)?;
        self.ast.decls.record(d)?;
        Some(d)
    }

    /// Whether `__impl` definition `d` has the shape clang requires.
    fn source_location_impl_ok(&mut self, d: DeclId) -> bool {
        let Some(r) = self.ast.decls.record(d) else { return false };
        if self.is_union(d)
            || !r.defn.contains(kiln_ast::decl::DefinitionBits::STANDARD_LAYOUT)
            || self.ast.decls.bases(d, &self.ast.lists).next().is_some()
        {
            return false;
        }
        let char_ptr = self.const_char_ptr(QualType::CHAR);
        let char_ptr = self.ast.types.canonical(char_ptr);
        let fields: Vec<DeclId> = self
            .ast
            .members(d)
            .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
        let mut count = 0;
        for f in fields {
            let n = *self.ast.decls.get(f);
            let Some(t) = n.ty() else { return false };
            let t = self.ast.types.canonical(t);
            let name = match n.name().as_ident() {
                Some(s) => self.ast.symbols().as_str(s).to_owned(),
                None => return false,
            };
            let ok = match name.as_str() {
                "_M_file_name" | "_M_function_name" => t == char_ptr,
                "_M_line" | "_M_column" => self.ast.types.is_integral(t),
                _ => return false,
            };
            if !ok {
                return false;
            }
            count += 1;
        }
        count == 4
    }

    /// Records the values of every source-location builtin for a use `e`
    /// at `loc` in context `ctx` (clang `SourceLocExpr::EvaluateInContext`
    /// with `getExpansionRange(Loc).getEnd()`, kiln's expansion location);
    /// `location` builds the `__impl` object too.
    fn record_source_loc(&mut self, e: ExprId, loc: Loc, ctx: DeclId, location: bool) {
        // The file as `-fkiln-source-path-style=` spells it (clang's as
        // written by default, GCC 16.1's simplified absolute path under
        // `gcc`; spec/reflection.md §3.9), then `-fmacro-prefix-map=`
        // applies (clang `SourceLocExpr::EvaluateInContext`).
        let sm = self.diags.sm();
        let (file, line, column) = match sm.presumed_line_col(loc) {
            Some(p) => (sm.remap_macro_path(&sm.presumed_file_name(&p, loc)), p.line, p.col),
            None => (String::new(), 0, 0),
        };
        let file_name = file.rsplit('/').next().unwrap_or(&file).to_owned();
        let (plain, pretty) = self.predefined_function_names(ctx);
        let Some(file_p) = self.srcloc_string(&file) else { return };
        let Some(file_name_p) = self.srcloc_string(&file_name) else { return };
        let Some(function_p) = self.srcloc_string(&plain) else { return };
        let location = if location {
            let Some(pretty_p) = self.srcloc_string(&pretty) else { return };
            self.source_location_object(file_p, pretty_p, line, column)
        } else {
            None
        };
        let v = SourceLocValues {
            file: file_p,
            file_name: file_name_p,
            function: function_p,
            line,
            column,
            location,
        };
        let _ = self.ast.exprs.set_source_loc_values(e, v);
        if v.location.is_some()
            && self
                .func
                .as_ref()
                .is_some_and(|f| f.decl == ctx && f.auto_ret.is_some() && !f.deduced)
        {
            self.srcloc.deferred.push((e, ctx));
        }
    }

    /// The return type of `f` is deduced (or its body ends): the
    /// source-location objects built in it before then take the pretty
    /// name with the deduced type (clang computes a `SourceLocExpr`'s value
    /// when it is evaluated: `const char *f()` for `auto f()` from codegen
    /// or from an immediate invocation in the deducing `return`). A
    /// constant evaluated before then keeps the name it saw.
    pub(crate) fn finish_source_locs(&mut self, f: DeclId) {
        if self.srcloc.deferred.is_empty() {
            return;
        }
        let (mine, rest): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.srcloc.deferred).into_iter().partition(|&(_, c)| c == f);
        self.srcloc.deferred = rest;
        let (_, pretty) = self.predefined_function_names(f);
        let Some(pretty_p) = self.srcloc_string(&pretty) else { return };
        for (e, _) in mine {
            let Some(mut v) = self.ast.exprs.source_loc_values(e).copied() else { continue };
            v.location = self.source_location_object(v.file, pretty_p, v.line, v.column);
            if v.location.is_some() {
                self.ast.exprs.refine_source_loc_values(e, v);
            }
        }
    }

    /// The `const char *` value pointing to the string literal object
    /// holding `s` (one object per content).
    pub(crate) fn srcloc_string(&mut self, s: &str) -> Option<kiln_ast::ConstValueId> {
        self.frozen_string(s, false)
    }

    /// The `const char *` (`const char8_t *` when `u8`) value pointing to
    /// the string literal object holding `s` (one object per content and
    /// encoding).
    pub(crate) fn frozen_string(&mut self, s: &str, u8: bool) -> Option<kiln_ast::ConstValueId> {
        use kiln_ast::expr::StringKind;
        use kiln_ast::strings::StringEncoding;
        let (enc, lex, kind, elem) = if u8 {
            (StringEncoding::Utf8, kiln_lex::Encoding::Utf8, StringKind::Utf8, QualType::CHAR8)
        } else {
            (
                StringEncoding::Ordinary,
                kiln_lex::Encoding::Ordinary,
                StringKind::Ordinary,
                QualType::CHAR,
            )
        };
        let lit = self.ast.strings.intern(enc, s.as_bytes());
        let origin = if let Some(&e) = self.srcloc.strings.get(&lit) {
            e
        } else {
            let ty = crate::literal::string_literal_type(self, lex, s.len() as u64 + 1);
            let e = self.builder().string_literal(ty, kind, lit, Loc::INVALID);
            self.srcloc.strings.insert(lit, e);
            e
        };
        let ty = self.const_char_ptr(elem);
        let ty = self.ast.types.canonical(ty);
        let base = PtrBase::StringLit { lit, origin };
        self.ast.consts.push_scalar(Scalar::Pointer { ty, base, offset: 0, designated: None }).ok()
    }

    /// The `const std::source_location::__impl *` value of a
    /// `source_location` for `loc` in a function named `function`
    /// (`std::meta::source_location_of`), or `None` when `__impl` is not
    /// declared with the shape [`Self::std_source_location_impl`] checks
    /// (quietly: the caller materializes a null `source_location`).
    pub(crate) fn meta_source_location(
        &mut self,
        loc: Loc,
        function: &str,
    ) -> Option<kiln_ast::ConstValueId> {
        if self.srcloc.impl_decl.is_none() {
            let d = self.lookup_source_location_impl()?;
            if !self.source_location_impl_ok(d) {
                return None;
            }
            self.srcloc.impl_decl = Some(d);
        }
        // The file as `-fkiln-source-path-style=` spells it (clang's as
        // written by default, GCC 16.1's simplified absolute path under
        // `gcc`; spec/reflection.md §3.9), then `-fmacro-prefix-map=`
        // applies (clang `SourceLocExpr::EvaluateInContext`).
        let sm = self.diags.sm();
        let (file, line, column) = match sm.presumed_line_col(loc) {
            Some(p) => (sm.remap_macro_path(&sm.presumed_file_name(&p, loc)), p.line, p.col),
            None => (String::new(), 0, 0),
        };
        let file = self.srcloc_string(&file)?;
        let function = self.srcloc_string(function)?;
        self.source_location_object(file, function, line, column)
    }

    /// The `const std::source_location::__impl *` value pointing to the
    /// frozen `__impl` object `{file, function, line, column}` (clang's
    /// `UnnamedGlobalConstantDecl`): equal values share one object.
    fn source_location_object(
        &mut self,
        file: kiln_ast::ConstValueId,
        function: kiln_ast::ConstValueId,
        line: u32,
        column: u32,
    ) -> Option<kiln_ast::ConstValueId> {
        let d = self.srcloc.impl_decl?;
        let rec = self.ast.types.canonical(self.tag_type(d));
        let fields: Vec<DeclId> = self
            .ast
            .members(d)
            .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
        let layout = kiln_abi::ensure_layout(&mut self.ast, d, &self.abi)?;
        let size = layout.size as usize;
        let offsets: Vec<u64> = layout.fields.to_vec();
        let mut bytes = vec![0u8; size];
        let mut covered = vec![false; size];
        let mut relocs = Vec::new();
        let ptr_bytes = usize::from(self.ast.consts.pointer_bytes());
        for (f, off) in fields.into_iter().zip(offsets) {
            let at = (off / 8) as usize;
            let n = self.ast.decls.get(f);
            let name = n.name().as_ident().map(|s| self.ast.symbols().as_str(s).to_owned());
            let fty = n.ty().unwrap_or(QualType::UINT);
            if let Some(n @ ("_M_file_name" | "_M_function_name")) = name.as_deref() {
                let value = if n == "_M_file_name" { file } else { function };
                relocs.push(kiln_ast::consts::Reloc { at: at as u32, value });
                covered.get_mut(at..at + ptr_bytes)?.fill(true);
            } else {
                let v = if name.as_deref() == Some("_M_line") { line } else { column };
                let (w, _) = crate::target::size_align(&self.ast, fty, &self.abi)?;
                let w = usize::try_from(w).ok()?;
                let le = u64::from(v).to_le_bytes();
                for i in 0..w {
                    *bytes.get_mut(at + i)? = le.get(i).copied().unwrap_or(0);
                }
                covered.get_mut(at..at + w)?.fill(true);
            }
        }
        relocs.sort_by_key(|r| r.at);
        let mut padding = Vec::new();
        let mut i = 0;
        while i < size {
            if covered[i] {
                i += 1;
                continue;
            }
            let start = i;
            while i < size && !covered[i] {
                i += 1;
            }
            padding
                .push(kiln_ast::consts::ByteRange { start: start as u32, len: (i - start) as u32 });
        }
        let img = kiln_ast::consts::ByteImage {
            ty: rec,
            bytes: &bytes,
            relocs: &relocs,
            padding: &padding,
            unions: &[],
        };
        let obj = self.ast.consts.push_aggregate(img).ok()?;
        let cq = self.ast.types.qualify(rec, Quals::CONST);
        let pty = self.ast.types.pointer_to(cq);
        let pty = self.ast.types.canonical(pty);
        self.ast
            .consts
            .push_scalar(Scalar::Pointer {
                ty: pty,
                base: PtrBase::Const(obj),
                offset: 0,
                designated: None,
            })
            .ok()
    }

    /// clang `PredefinedExpr::ComputeName` of context `ctx` for
    /// `__builtin_FUNCTION` (`Function`: the unqualified name) and
    /// `_M_function_name` (`PrettyFunction`); both empty outside a
    /// function.
    fn predefined_function_names(&self, ctx: DeclId) -> (String, String) {
        if !self.ast.decls.get(ctx).kind().is_function() {
            return (String::new(), String::new());
        }
        (
            self.compute_predefined_name(kiln_ast::expr::PredefinedKind::Func, ctx),
            self.compute_predefined_name(kiln_ast::expr::PredefinedKind::PrettyFunction, ctx),
        )
    }

    /// Records the source-location values at a use of `param`'s default
    /// argument (the `CXXDefaultArgExpr` `e`, in the current context, for
    /// the call beginning at `call_begin`) when the argument evaluates a
    /// source-location builtin ([support.srcloc.cons]/2: the location of
    /// the call; clang `CXXDefaultArgExpr::getUsedLocation`).
    pub(crate) fn note_default_arg_use(&mut self, e: ExprId, param: DeclId, call_begin: Loc) {
        let kinds = self.default_arg_srcloc_kinds(param);
        if kinds != 0 {
            let ctx = self.ctx;
            self.record_source_loc(e, call_begin, ctx, kinds & SRC_LOC_STRUCT_BIT != 0);
            // An immediate invocation in the argument is evaluated where
            // the call is built (clang's rewritten default argument), so
            // it sees the function name as it is now.
            if self.default_arg_has_immediate(param, 0) {
                self.srcloc.deferred.retain(|&(x, _)| x != e);
            }
            if kinds & (SRC_LOC_STRUCT_BIT | 1 << SourceLocKind::Function as u8) != 0 {
                self.mark_function_name_dependent(e, ctx);
            }
        }
    }

    /// Whether `param`'s default argument holds an immediate invocation
    /// (a `ConstantExpr`), through nested default arguments.
    /// Records the source-location values at a use of `field`'s default
    /// member initializer (the `CXXDefaultInitExpr` `e`, built at `loc`:
    /// the constructor's name, or the `}` / `)` of an aggregate
    /// initialization, in context `ctx`) when the initializer evaluates a
    /// source-location builtin ([support.srcloc.cons]/2; clang
    /// `CXXDefaultInitExpr::getUsedLocation` / `getUsedContext`).
    pub(crate) fn note_default_init_use(
        &mut self,
        e: ExprId,
        field: DeclId,
        loc: Loc,
        ctx: DeclId,
    ) {
        let kinds = self.default_arg_srcloc_kinds(field);
        if kinds != 0 {
            self.record_source_loc(e, loc, ctx, kinds & SRC_LOC_STRUCT_BIT != 0);
            if self.default_arg_has_immediate(field, 0) {
                self.srcloc.deferred.retain(|&(x, _)| x != e);
            }
            if kinds & (SRC_LOC_STRUCT_BIT | 1 << SourceLocKind::Function as u8) != 0 {
                self.mark_function_name_dependent(e, ctx);
            }
        }
    }

    fn default_arg_has_immediate(&self, param: DeclId, depth: u32) -> bool {
        let Some(init) = self.default_init_of(param) else {
            return false;
        };
        if depth > 32 {
            return false;
        }
        let mut stack = vec![init];
        while let Some(x) = stack.pop() {
            match self.ast.exprs.get(x).kind() {
                ExprKind::ConstantExpr => return true,
                ExprKind::CXXDefaultArgExpr => {
                    let p = self.ast.exprs.default_arg_param(x);
                    if self.default_arg_has_immediate(p, depth + 1) {
                        return true;
                    }
                }
                ExprKind::CXXDefaultInitExpr => {
                    let f = self.ast.exprs.default_init_field(x);
                    if self.default_arg_has_immediate(f, depth + 1) {
                        return true;
                    }
                }
                _ => self.ast.exprs.children_into(x, &self.ast.lists, &mut stack),
            }
        }
        false
    }

    /// In a dependent context the function name a source-location value
    /// holds changes per specialization (`[T = int]`): `e` is marked
    /// instantiation-dependent so instantiation rebuilds it (clang 19's
    /// `SourceLocExpr::MayBeDependent`, `TransformCXXDefaultArgExpr`).
    fn mark_function_name_dependent(&mut self, e: ExprId, ctx: DeclId) {
        if self.ast.decls.get(ctx).kind().is_function() && self.ctx_template_levels(ctx) > 0 {
            self.ast.exprs.mark_instantiation_dependent(e);
        }
    }

    /// The default argument of parameter `d` or the default member
    /// initializer of field `d`.
    fn default_init_of(&self, d: DeclId) -> Option<ExprId> {
        match self.ast.decls.param(d) {
            Some(p) => p.default_arg,
            None => self.ast.decls.field(d).and_then(|f| f.init),
        }
    }

    /// The source-location builtins `param`'s default argument (or field
    /// `param`'s default member initializer) evaluates (a mask of
    /// `1 << kind`), through nested default arguments and initializers.
    fn default_arg_srcloc_kinds(&mut self, param: DeclId) -> u8 {
        if let Some(&k) = self.srcloc.param_kinds.get(&param) {
            return k;
        }
        // Guards recursion through `void f(int = f())`.
        self.srcloc.param_kinds.insert(param, 0);
        let Some(init) = self.default_init_of(param) else {
            return 0;
        };
        let mut kinds = 0u8;
        let mut stack = vec![init];
        while let Some(x) = stack.pop() {
            match self.ast.exprs.get(x).kind() {
                ExprKind::SourceLocExpr => {
                    kinds |= 1 << self.ast.exprs.source_loc_kind(x) as u8;
                }
                ExprKind::CXXDefaultArgExpr => {
                    let p = self.ast.exprs.default_arg_param(x);
                    kinds |= self.default_arg_srcloc_kinds(p);
                }
                ExprKind::CXXDefaultInitExpr => {
                    let f = self.ast.exprs.default_init_field(x);
                    kinds |= self.default_arg_srcloc_kinds(f);
                }
                _ => self.ast.exprs.children_into(x, &self.ast.lists, &mut stack),
            }
        }
        self.srcloc.param_kinds.insert(param, kinds);
        kinds
    }
}
