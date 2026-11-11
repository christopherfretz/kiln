//! C declarations: redeclarations and tentative definitions
//! (spec/landed/sema-conv/WP-1286.md).
//!
//! - A GNU89 `extern inline` definition is inline-only: a later
//!   definition replaces it (clang `canRedefineFunction`).
//! - Two compatible declarations of one function form the composite type
//!   (C23 6.2.7p3): `int g(int (*)[]); int g(int (*p)[3]) {...}`.
//! - A wide string literal initializes an array whose element type is
//!   compatible with the literal's (`wchar_t`, `char16_t` and `char32_t`
//!   are typedefs in C; C23 6.7.10p16, clang `IsStringInit`).
//! - GNU: an array is initialized from a compound literal of the same
//!   array type (clang `InitializationSequence`'s `SK_ArrayInit`).
//! - A file-scope object declared without an initializer and without
//!   `extern` is a tentative definition (C23 6.9.2p2). Its type may be
//!   incomplete until the end of the translation unit, where an array of
//!   unknown bound becomes `[1]` (6.9.2p5) and any other incomplete type
//!   is an error (clang `ActOnEndOfTranslationUnit`).
//! - A tag first declared in a parameter list has function prototype
//!   scope (C23 6.2.1p4): later parameters of that list find it, the
//!   function's body does too when the declarator is a definition, and
//!   nothing else does. It is clang's `-Wvisibility` warning.

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, FnStorage, VarStorage};
use kiln_ast::expr::ExprKind;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::fx::FxHashSet;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::conv;
use crate::declspec::{DeclSpec, StorageSpec};
use crate::init::StringInit;
use crate::literal::c_char_type;

/// The recursion bound of the composite type (types nest no deeper in
/// practice; past it the first type is kept).
const DEPTH: u32 = 64;

impl Sema<'_, '_> {
    /// A C declaration with no declarators outside a structure (C23
    /// 6.7p2, clang `ParsedFreeStandingDeclSpec`): with `typedef` it is
    /// "typedef requires a name"; an unnamed structure or union declares
    /// no anonymous member there (6.7.2.1p15 makes one only a member), so
    /// it declares nothing. True if diagnosed here.
    pub(crate) fn c_decl_spec_only(&mut self, ds: &DeclSpec) -> bool {
        if !self.is_c() || ds.invalid || self.kind_of(self.ctx) == DeclKind::CXXRecord {
            return false;
        }
        if matches!(ds.storage, Some((StorageSpec::Typedef, _))) {
            self.emit(self.diag(DiagId::sema_typedef_without_a_name, ds.loc));
            return true;
        }
        let anonymous = ds.tag.is_some_and(|t| {
            ds.tag_declares
                && self
                    .ast
                    .decls
                    .record(t)
                    .is_some_and(|r| r.defn.contains(DefinitionBits::ANONYMOUS))
        });
        if anonymous {
            self.emit(self.diag(DiagId::sema_no_declarators, ds.loc));
        }
        anonymous
    }

    /// True if a tag declared now has C function prototype scope (C23
    /// 6.2.1p4): the innermost parameter-declaration-clause is open.
    pub(crate) fn c_in_proto_tag_scope(&self) -> bool {
        self.is_c() && self.in_prototype_scope() && !self.prototype_params.is_empty()
    }

    /// The tag `n` declared earlier in the innermost open parameter list.
    pub(crate) fn c_proto_tag(&self, n: Symbol) -> Option<DeclId> {
        let f = self.prototype_params.last()?;
        f.tags.iter().rev().find(|t| t.0 == n).map(|t| t.1)
    }

    /// Gives the new tag `d` named `n` prototype scope: hidden from
    /// file-scope lookup, found by the rest of the parameter list.
    pub(crate) fn c_declare_proto_tag(&mut self, n: Symbol, d: DeclId) {
        self.builder().unregister_name(d);
        if let Some(f) = self.prototype_params.last_mut() {
            f.tags.push((n, d));
        }
    }

    /// The outermost parameter list just closed declared `tags`: kept for
    /// the function body, if the declarator is a definition (clang
    /// `ActOnStartOfFunctionDef` re-adds the `DeclsInPrototype`).
    pub(crate) fn c_end_proto_tags(&mut self, tags: Vec<(Symbol, DeclId)>) {
        if self.is_c() && self.prototype_params.is_empty() {
            self.c_param_tags = tags;
        }
    }

    /// The function body scope just opened: the tags of its parameter
    /// list are in scope there.
    pub(crate) fn c_inject_param_tags(&mut self) {
        if !self.is_c() {
            return;
        }
        for (n, d) in std::mem::take(&mut self.c_param_tags) {
            self.push_local(n, d);
        }
    }

    /// clang `canRedefineFunction`: in C, a GNU89 (`-fgnu89-inline` or
    /// `gnu_inline`) `extern inline` definition `f` is inline-only, and a
    /// later definition, even a `static` one, replaces it.
    pub(crate) fn c_can_redefine_function(&self, f: DeclId) -> bool {
        self.is_c()
            && self
                .ast
                .decls
                .function(f)
                .is_some_and(|x| x.spec.is_inline() && x.spec.storage() == FnStorage::Extern)
            && (self.gnu89_inline || self.find_attr(f, AttrKind::GNU_INLINE).is_some())
    }

    /// The composite of function types `pty` (a previous declaration's)
    /// and `ty` when both have prototypes, differ, and are compatible
    /// (C23 6.2.7p3, 6.7.6.3p14): `None` otherwise. Only the parameter and
    /// return types may differ; the rest of the function type (calling
    /// convention, `noreturn`, exception specification) must agree.
    pub(crate) fn c_proto_composite(&mut self, pty: QualType, ty: QualType) -> Option<QualType> {
        let types = &self.ast.types;
        if types.same_type(pty, ty) {
            return None;
        }
        let (a, b) = (types.proto(pty).copied()?, types.proto(ty).copied()?);
        if a.info().no_proto()
            || b.info().no_proto()
            || a.info() != b.info()
            || types.exc_spec(&a) != types.exc_spec(&b)
        {
            return None;
        }
        // GNU: a transparent union parameter merges with a member's type.
        let (pty, ty) = self.c_transparent_param_merge(pty, ty).unwrap_or((pty, ty));
        if !conv::c_assign::compatible(self, pty, ty) {
            return None;
        }
        Some(self.c_composite_at(ty, pty, DEPTH))
    }

    /// The composite of the compatible types `a` and `b` (C23 6.2.7p3):
    /// an array takes a known size from either, and the rule applies
    /// through pointers, arrays, and function return and parameter types.
    /// The sugar of `a` is kept where nothing changes.
    fn c_composite_at(&mut self, a: QualType, b: QualType, depth: u32) -> QualType {
        let types = &self.ast.types;
        if depth == 0 || types.same_type(a, b) {
            return a;
        }
        let quals = types.canonical(a).quals();
        let (va, vb) = (types.canonical_view(a), types.canonical_view(b));
        let t = match (va, vb) {
            (TypeView::Pointer(x), TypeView::Pointer(y)) => {
                let p = self.c_composite_at(x, y, depth - 1);
                self.ast.types.pointer_to(p)
            }
            (
                TypeView::ConstantArray(_) | TypeView::IncompleteArray(_),
                TypeView::ConstantArray(_) | TypeView::IncompleteArray(_),
            ) => {
                let (Some(x), Some(y)) = (types.element(a), types.element(b)) else { return a };
                let size = match (va, vb) {
                    (TypeView::ConstantArray(c), _) | (_, TypeView::ConstantArray(c)) => {
                        Some(c.size)
                    }
                    _ => None,
                };
                let e = self.c_composite_at(x, y, depth - 1);
                match size {
                    Some(n) => self.ast.types.constant_array(e, n, None),
                    None => self.ast.types.incomplete_array(e),
                }
            }
            (TypeView::Function(f), TypeView::Function(g))
                if f.num_params() == g.num_params()
                    && !f.info().no_proto()
                    && !g.info().no_proto() =>
            {
                let ret = self.c_composite_at(f.ret(), g.ret(), depth - 1);
                let (pf, pg) =
                    (self.ast.types.params(&f).to_vec(), self.ast.types.params(&g).to_vec());
                let params: Vec<QualType> = pf
                    .iter()
                    .zip(&pg)
                    .map(|(&x, &y)| self.c_composite_at(x, y, depth - 1))
                    .collect();
                let exc = self.ast.types.exc_spec(&f);
                self.ast.types.function(ret, &params, f.info(), exc)
            }
            _ => return a,
        };
        self.ast.types.qualify(t, quals)
    }

    /// clang `ActOnUninitializedDecl` for a C tentative definition `v`
    /// (C23 6.9.2p2): its type may stay incomplete until the end of the
    /// translation unit; a `static` one of incomplete non-array type is
    /// warned at its first declaration (GCC accepts `static struct s x;`
    /// completed later, 6.9.2p3). The definition is recorded for
    /// [`Self::c_complete_tentative_definitions`]. False (the ordinary
    /// checks apply) for a `constexpr` object or an invalid one.
    pub(crate) fn c_tentative_uninit(&mut self, v: DeclId) -> bool {
        let vn = *self.ast.decls.get(v);
        let Some(spec) = self.ast.decls.var(v).map(|x| x.spec) else { return false };
        if !self.c_tentative(v) || spec.is_constexpr() || vn.has(DeclFlags::INVALID) {
            return false;
        }
        let Some(ty) = vn.ty() else { return false };
        if self.is_error_type(ty) {
            return true;
        }
        let array = matches!(self.ast.types.canonical_view(ty), TypeView::IncompleteArray(_));
        if !array
            && spec.storage() == VarStorage::Static
            && self.ast.first(v) == v
            && !self.ast.types.is_void(ty)
            && !self.is_complete_now(ty)
        {
            let msg = self
                .diag(DiagId::sema_tentative_static_incomplete_type, vn.loc())
                .arg(self.ty_arg(ty));
            self.emit(self.incomplete_note(msg, ty));
        }
        self.c_tentatives.push(v);
        true
    }

    /// Whether `ty` is complete, completing nothing and reporting nothing.
    fn is_complete_now(&self, ty: QualType) -> bool {
        match self.ast.types.as_record(ty) {
            Some(r) => self.ast.definition(r).is_some_and(|d| !self.defining.contains(&d)),
            None => self.ast.types.as_enum(ty).is_none_or(|e| self.ast.definition(e).is_some()),
        }
    }

    /// clang `ActOnEndOfTranslationUnit`'s tentative definitions (C23
    /// 6.9.2p2, p5): for each object whose declarations include a
    /// tentative definition and no definition with an initializer, the
    /// last tentative definition is its definition. An array of unknown
    /// bound there becomes an array of one element (warned,
    /// `-Wtentative-definition-array`); another type still incomplete is
    /// an error, and the object is invalid.
    pub(crate) fn c_complete_tentative_definitions(&mut self) {
        let list = std::mem::take(&mut self.c_tentatives);
        let mut seen = FxHashSet::default();
        for v in list {
            let Some(acting) = self.c_acting_definition(v) else { continue };
            if !seen.insert(acting) || self.ast.decls.get(acting).has(DeclFlags::INVALID) {
                continue;
            }
            let vn = *self.ast.decls.get(acting);
            let Some(ty) = vn.ty() else { continue };
            if self.is_error_type(ty) {
                continue;
            }
            if let TypeView::IncompleteArray(_) = self.ast.types.canonical_view(ty) {
                let Some(elem) = self.ast.types.element(ty) else { continue };
                let msg = self.diag(DiagId::sema_tentative_incomplete_array, vn.loc());
                self.emit(msg);
                let one = self.ast.types.constant_array(elem, 1, None);
                self.ast.decls.set_ty(acting, one);
            } else if !self.require_complete(ty, vn.loc()) {
                let msg = self
                    .diag(DiagId::sema_tentative_def_incomplete_type, vn.loc())
                    .arg(self.ty_arg(ty));
                self.emit(self.incomplete_note(msg, ty));
                self.ast.decls.set_flags(acting, DeclFlags::INVALID);
            }
        }
    }

    /// clang `VarDecl::getActingDefinition`: `None` when a declaration of
    /// `v`'s object has an initializer, else its last tentative
    /// definition.
    fn c_acting_definition(&self, v: DeclId) -> Option<DeclId> {
        let mut last = None;
        for r in self.ast.redecls_of(v) {
            if self.ast.decls.var(r).is_some_and(|x| x.init.is_some()) {
                return None;
            }
            if last.is_none() && self.c_tentative(r) {
                last = Some(r);
            }
        }
        last
    }

    /// clang `IsStringInit` in C (C23 6.7.10p15-16): a character string
    /// literal initializes an array of a character type, and a wide one
    /// (`L`, `u`, `U`) an array whose unqualified element type is
    /// compatible with the literal's element type (`wchar_t`,
    /// `char16_t`, `char32_t` are their underlying integer types in C).
    pub(crate) fn c_string_init_compat(&self, elem: QualType, s: ExprId) -> StringInit {
        use kiln_ast::expr::StringKind as K;
        use kiln_ast::types::BuiltinType as B;
        let u = self.ast.types.canonical(elem).without_quals();
        let b = self.ast.types.builtin(u);
        let char_like = matches!(b, Some(B::Char | B::SChar | B::UChar));
        let char_or_uchar = matches!(b, Some(B::Char | B::UChar));
        // clang `IsWideCharCompatible`: `char16_t` and `char32_t` count
        // from C11.
        let wide_compat = u == c_char_type(self, QualType::WCHAR)
            || (self.c_at_least(kiln_lex::CStd::C11)
                && (u == c_char_type(self, QualType::CHAR16)
                    || u == c_char_type(self, QualType::CHAR32)));
        let own = |lit: QualType| {
            if u == c_char_type(self, lit) {
                StringInit::Ok
            } else if char_or_uchar {
                StringInit::WideIntoChar
            } else if wide_compat {
                StringInit::IncompatWide
            } else {
                StringInit::Other
            }
        };
        match self.ast.exprs.string_kind(s) {
            K::Ordinary | K::Utf8 | K::Unevaluated if char_like => StringInit::Ok,
            K::Ordinary | K::Utf8 | K::Unevaluated if wide_compat => StringInit::NarrowIntoWide,
            K::Ordinary | K::Utf8 | K::Unevaluated => StringInit::Other,
            K::Wide => own(QualType::WCHAR),
            K::Utf16 => own(QualType::CHAR16),
            K::Utf32 => own(QualType::CHAR32),
        }
    }

    /// GNU: `T a[] = (T[]){...};` initializes the array `ty` from the
    /// compound literal `e` (through parentheses) of an array type with
    /// the same element type (clang `InitializationSequence`'s GNU
    /// `SK_ArrayInit`, warned `-Wgnu-compound-literal-initializer` under
    /// `-pedantic`): the literal's initializer list initializes the array.
    /// A bound that differs, a different element type, or a literal with
    /// side effects is an error. `None` when `e` is not such a literal.
    pub(crate) fn c_array_init_from_compound_literal(
        &mut self,
        ty: QualType,
        e: ExprId,
        at: Loc,
    ) -> Option<ExprId> {
        let lit = self.ignore_parens(e);
        if self.ast.exprs.get(lit).kind() != ExprKind::CompoundLiteralExpr {
            return None;
        }
        let src = self.ast.exprs.get(lit).ty();
        let types = &self.ast.types;
        let (Some(de), Some(se)) = (types.element(ty), types.element(src)) else { return None };
        let (dv, sv) = (types.canonical_view(ty), types.canonical_view(src));
        // clang `hasCompatibleArrayTypes`.
        let compatible = types.same_type(ty, src)
            || (types.same_type(de, se)
                && matches!(dv, TypeView::IncompleteArray(_))
                && matches!(sv, TypeView::ConstantArray(_)));
        let range = self.expr_range(e);
        let id = if !compatible {
            DiagId::sema_array_init_different_type
        } else if self.has_possible_side_effects(lit) {
            DiagId::sema_array_init_non_constant_array
        } else {
            DiagId::sema_array_init_compound_literal_gnu
        };
        let msg = self.diag(id, at).arg(self.ty_arg(ty)).arg(self.ty_arg(src)).range(range);
        self.emit(msg);
        if id != DiagId::sema_array_init_compound_literal_gnu {
            return Some(self.recover(ty, &[e], at));
        }
        Some(self.ast.exprs.compound_literal_init(lit))
    }
}

impl Sema<'_, '_> {
    /// C23 6.7.5p3 (function specifiers): an inline definition of a
    /// function with external linkage shall not reference an identifier
    /// with internal linkage. clang `diagnoseUseOfInternalDeclInInlineFunction`
    /// (`-Wstatic-in-inline`, C only): `d` named at `loc` in the current
    /// function. The warning is pedantic-only when the use is in the main
    /// file, or names an inline or `const` function.
    pub(crate) fn c_check_static_in_inline(&mut self, d: DeclId, loc: Loc) {
        if !self.is_c() || !matches!(self.kind_of(d), DeclKind::Var | DeclKind::Function) {
            return;
        }
        let Some(cur) = self.func.as_ref().map(|f| f.decl) else { return };
        let inline = |s: &Self, f: DeclId| {
            s.ast.decls.function(f).is_some_and(|x| x.spec.is_inline())
                || s.ast.decls.function(s.ast.first(f)).is_some_and(|x| x.spec.is_inline())
        };
        if !inline(self, cur) || !self.has_external_linkage(cur) || !self.has_internal_linkage(d) {
            return;
        }
        let is_fn = self.kind_of(d) == DeclKind::Function;
        let quiet = self.in_main_file(loc)
            || (is_fn && (inline(self, d) || self.find_attr(d, AttrKind::CONST).is_some()));
        let id = if quiet {
            DiagId::sema_internal_in_extern_inline_quiet
        } else {
            DiagId::sema_internal_in_extern_inline
        };
        let name = format!("'{}'", self.decl_name(d));
        let mut msg = self.diag(id, loc).arg(u8::from(!is_fn)).arg(name.clone());
        if let Some(note) = self.c_suggest_static(cur) {
            msg = msg.with_note(note);
        }
        let decl_at = self.ast.decls.get(self.ast.first(d)).loc();
        let note = self.diag(DiagId::note_entity_declared_at, decl_at).arg(name);
        self.emit(msg.with_note(note));
    }

    /// clang `MaybeSuggestAddingStaticToDecl`: the "use 'static'" note on
    /// inline function `cur`, at its first declaration's start, when that
    /// declaration has no storage class written.
    fn c_suggest_static(&self, cur: DeclId) -> Option<kiln_diag::Diag> {
        let first = self.ast.first(cur);
        if !self.ast.decls.function(first).is_some_and(|x| x.spec.storage() == FnStorage::None) {
            return None;
        }
        let at = self
            .ast
            .decls
            .cold
            .begin_loc
            .get(first)
            .copied()
            .unwrap_or(self.ast.decls.get(first).loc());
        let fname = format!("'{}'", self.decl_name(cur));
        Some(
            self.diag(DiagId::note_convert_inline_to_static, at)
                .arg(fname)
                .fixit(kiln_diag::FixIt::insert(at, "static ")),
        )
    }

    /// C23 6.7.5p3: an inline definition of a function with external
    /// linkage shall not contain a definition of a modifiable object with
    /// static storage duration. clang `warn_static_local_in_extern_inline`
    /// (`-Wstatic-local-in-inline`): a block-scope `static` object of a
    /// type that is not `const`, written at `storage_loc`, in a function
    /// whose definition here is discarded (clang
    /// `isFunctionDefinitionDiscarded`: another translation unit provides
    /// the external definition).
    pub(crate) fn c_check_static_local_in_inline(&mut self, ty: QualType, storage_loc: Loc) {
        if !self.is_c() || self.ast.types.effective_quals(ty).is_const() {
            return;
        }
        let Some(cur) = self.func.as_ref().map(|f| f.decl) else { return };
        if !self.c_inline_definition_discarded(cur) {
            return;
        }
        let mut msg = self.diag(DiagId::sema_static_local_in_extern_inline, storage_loc);
        if let Some(note) = self.c_suggest_static(cur) {
            msg = msg.with_note(note);
        }
        self.emit(msg);
    }

    /// Whether the definition of function `f` (being defined) is an
    /// inline definition that is not externally visible (clang
    /// `GetGVALinkageForFunction == GVA_AvailableExternally`, by the
    /// declarations seen so far). C99 (C23 6.7.5p7): every file-scope
    /// declaration is `inline` and none is `extern`. GNU89
    /// (`-fgnu89-inline`, or `gnu_inline`): the definition is `extern
    /// inline` and no declaration is `inline` without `extern`. The
    /// codegen rule is `kiln_codegen`'s `c_inline_definition_only`.
    fn c_inline_definition_discarded(&self, f: DeclId) -> bool {
        let ast = &self.ast;
        let spec = |r: DeclId| ast.decls.function(r).map(|x| x.spec);
        let first = ast.first(f);
        if !ast.redecls_of(first).any(|r| spec(r).is_some_and(kiln_ast::decl::FnSpec::is_inline))
            || !self.has_external_linkage(f)
        {
            return false;
        }
        let ext = |s: kiln_ast::decl::FnSpec| s.storage() == FnStorage::Extern;
        if self.gnu89_inline || self.find_attr(f, AttrKind::GNU_INLINE).is_some() {
            return spec(f).is_some_and(|s| s.is_inline() && ext(s))
                && !ast
                    .redecls_of(first)
                    .any(|r| spec(r).is_some_and(|s| s.is_inline() && !ext(s)));
        }
        let file_scope = |r: DeclId| {
            ast.decls
                .lexical_ctx(r)
                .is_some_and(|c| ast.decls.get(c).kind() == DeclKind::TranslationUnit)
        };
        !ast.redecls_of(first)
            .filter(|&r| file_scope(r))
            .any(|r| spec(r).is_some_and(|s| !s.is_inline() || ext(s)))
    }
}
