//! Constructor, destructor, conversion-function and member `operator=`
//! declarators (spec/sema-class.md §4.7; clang's
//! `CheckConstructorDeclarator`, `CheckDestructorDeclarator`,
//! `CheckConversionDeclarator`, `CheckConstructor`) and the
//! `explicit` specifier ([dcl.fct.spec]/3).

use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind, FnSpec};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::BuiltinType;
use kiln_ast::types::{ExcSpec, FuncInfo, RefQual};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use super::SpecialKind;
use crate::Sema;
use crate::declspec::{
    Chunk, DeclSpec, Declarator, DeclaratorContext, ParamInfo, SpecialName, StorageSpec,
};
use crate::lookup::LookupKind;

/// What a special declarator declares.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Ctor,
    Dtor,
    Conv(QualType),
    Assign,
}

impl Shape {
    /// The `%select` index of `constructor|destructor|conversion function`.
    fn sel(self) -> &'static str {
        match self {
            Shape::Ctor => "0",
            Shape::Dtor => "1",
            Shape::Conv(_) | Shape::Assign => "2",
        }
    }

    fn kind(self) -> DeclKind {
        match self {
            Shape::Ctor => DeclKind::CXXConstructor,
            Shape::Dtor => DeclKind::CXXDestructor,
            Shape::Conv(_) => DeclKind::CXXConversion,
            Shape::Assign => DeclKind::CXXMethod,
        }
    }
}

impl Sema<'_, '_> {
    /// Whether [`Sema::act_on_special_declarator`] declares `d`:
    /// constructors, destructors, conversion functions and member
    /// `operator=` (other operator functions and friend declarations of
    /// special members are WP-068b).
    pub(crate) fn handles_special_declarator(&self, ds: &DeclSpec, d: &Declarator) -> bool {
        if ds.friend.is_some() {
            return false;
        }
        match d.special {
            Some((
                SpecialName::Constructor(_)
                | SpecialName::Destructor(_)
                | SpecialName::Conversion(_),
                _,
            )) => true,
            Some((SpecialName::Operator(OverloadedOperator::Equal), _)) => {
                self.declarator_class(d).is_some()
            }
            _ => false,
        }
    }

    /// The class a declarator declares a member of: the class being
    /// defined, or the class named by its qualifier.
    fn declarator_class(&self, d: &Declarator) -> Option<DeclId> {
        let (ctx, _) = self.decl_contexts(d);
        (self.kind_of(ctx) == DeclKind::CXXRecord).then(|| self.ast.definition(ctx).unwrap_or(ctx))
    }

    /// The `explicit` specifier of a declarator ([dcl.fct.spec]/3; clang
    /// `ActOnFunctionDeclarator`, `DiagnoseFunctionSpecifiers`): only on
    /// a constructor or conversion function declared in its class.
    /// Returns whether the declaration is explicit (written, and
    /// `explicit(expr)` not known false).
    pub(crate) fn check_explicit_spec(
        &mut self,
        ds: &DeclSpec,
        d: &Declarator,
        function: bool,
        ctor_or_conv: bool,
    ) -> bool {
        let Some(loc) = ds.explicit else { return false };
        if !function {
            self.error(DiagId::sema_explicit_non_function, loc);
        } else if d.ctx != DeclaratorContext::Member {
            self.error(DiagId::sema_explicit_out_of_class, loc);
        } else if !ctor_or_conv {
            self.error(DiagId::sema_explicit_non_ctor_or_conv, loc);
        } else {
            return true;
        }
        false
    }

    /// The value of `explicit(expr)` ([dcl.fct.spec]/3: a contextually
    /// converted constant expression of type `bool`): `None` when it is
    /// value-dependent or not a constant.
    fn explicit_value(&mut self, e: kiln_ast::ExprId) -> Option<bool> {
        let x = self.ast.exprs.get(e);
        if x.is_value_dependent() || x.is_type_dependent() {
            return None;
        }
        let ty = x.ty();
        if self.ast.types.builtin(ty) != Some(BuiltinType::Bool) {
            // todo(WP-068a): converted constant expressions of other
            // types.
            self.unsupported("'explicit' with a non-bool expression", self.begin_loc(e));
            return None;
        }
        let r = crate::conv::lvalue_to_rvalue(self, e);
        self.fold_int(r).map(|v| v != 0)
    }

    /// A constructor, destructor, conversion-function or member
    /// `operator=` declarator ([class.ctor], [class.dtor],
    /// [class.conv.fct], [class.copy.assign]; spec/sema-class.md §4.7).
    /// `ty` is the declarator's type (return type `void` unless one was
    /// written); `invalid` whether an error was already reported. Returns
    /// the function, or `None` when the declaration is dropped.
    #[allow(clippy::too_many_lines, reason = "clang's declarator checks in order")]
    pub(crate) fn act_on_special_declarator(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        invalid: &mut bool,
    ) -> Option<DeclId> {
        let (sp, sloc) = d.special?;
        let Some(class) = self.declarator_class(d) else {
            match sp {
                SpecialName::Conversion(_) => self.error(DiagId::sema_conv_not_member, sloc),
                SpecialName::Destructor(_) => self.error(DiagId::sema_dtor_not_member, sloc),
                SpecialName::Constructor(_) => self.unsupported("constructors", sloc),
                SpecialName::Operator(_) => self.unsupported("overloaded operators", sloc),
            }
            return None;
        };
        if !self.ast.types.is_function(ty) {
            if !*invalid {
                self.error(DiagId::parse_expected_unqualified_id, sloc);
            }
            return None;
        }
        let shape = match sp {
            SpecialName::Constructor(_) => Shape::Ctor,
            SpecialName::Destructor(n) => {
                if !self.dtor_name_matches(class, n, d.qualifier, sloc) {
                    return None;
                }
                Shape::Dtor
            }
            SpecialName::Conversion(t) => Shape::Conv(t),
            SpecialName::Operator(_) => Shape::Assign,
        };
        let in_class = d.qualifier.is_none();
        let is_static = matches!(ds.storage, Some((StorageSpec::Static, _)));
        let static_loc = match ds.storage {
            Some((StorageSpec::Static, l)) => l,
            _ => sloc,
        };
        let _ = static_loc;
        let Some(Chunk::Function { params, variadic, rparen, quals, ref_qual, .. }) =
            d.chunks.first()
        else {
            return None;
        };
        let (params, variadic, rparen, quals, ref_qual) =
            (params.clone(), *variadic, *rparen, *quals, *ref_qual);
        let mut ds_virt = None;
        match shape {
            Shape::Ctor => {
                if ds.has_type_specifier() {
                    self.error(DiagId::sema_ctor_return_type, sloc);
                    return None;
                }
                if ds.virtual_.is_some() {
                    if !*invalid {
                        let m = self
                            .diag(DiagId::sema_ctor_dtor_cannot_be, sloc)
                            .arg("0")
                            .arg("virtual");
                        self.emit(m);
                    }
                    *invalid = true;
                    let mut c = ds.clone();
                    c.virtual_ = None;
                    ds_virt = Some(c);
                }
                if is_static {
                    if !*invalid {
                        let m = self
                            .diag(DiagId::sema_ctor_dtor_cannot_be, sloc)
                            .arg("0")
                            .arg("static");
                        self.emit(m);
                    }
                    *invalid = true;
                }
                self.check_ctor_dtor_quals(rparen, quals, ref_qual, "0", invalid);
            }
            Shape::Dtor => {
                if is_static {
                    if !*invalid {
                        let m = self
                            .diag(DiagId::sema_ctor_dtor_cannot_be, sloc)
                            .arg("1")
                            .arg("static");
                        self.emit(m);
                    }
                    // clang `CheckDestructorDeclarator` drops `static`
                    // without invalidating the declarator.
                }
                if !*invalid && ds.has_type_specifier() {
                    self.error(DiagId::sema_dtor_return_type, sloc);
                }
                self.check_ctor_dtor_quals(rparen, quals, ref_qual, "1", invalid);
                if !params.is_empty() {
                    self.error(DiagId::sema_dtor_params, sloc);
                    *invalid = true;
                }
                if variadic {
                    self.error(DiagId::sema_dtor_variadic, sloc);
                    *invalid = true;
                }
            }
            Shape::Conv(_) => {
                let id = if is_static {
                    Some(DiagId::sema_conv_not_member)
                } else if ds.has_type_specifier() {
                    Some(DiagId::sema_conv_return_type)
                } else if !params.is_empty() {
                    Some(DiagId::sema_conv_params)
                } else if variadic {
                    Some(DiagId::sema_conv_variadic)
                } else {
                    None
                };
                if let Some(id) = id {
                    if !*invalid {
                        self.error(id, sloc);
                    }
                    return None;
                }
            }
            Shape::Assign => {
                if is_static {
                    // todo(WP-068b): `operator=` declared `static`
                    // ([over.ass]/1).
                    self.unsupported("a static 'operator='", sloc);
                    return None;
                }
            }
        }
        // The function type: constructors and destructors return `void`
        // and keep no qualifiers once diagnosed; a destructor has no
        // parameters; a conversion function returns its target.
        let ty = match shape {
            Shape::Ctor => self.rebuild_fn_type(ty, QualType::VOID, None, true),
            Shape::Dtor => self.rebuild_fn_type(ty, QualType::VOID, Some(&[]), true),
            Shape::Conv(t) => self.rebuild_fn_type(ty, t, None, false),
            Shape::Assign => ty,
        };
        let class_ty = self.class_type(class);
        let canon = self.ast.types.canonical(class_ty).ty();
        let name = match shape {
            Shape::Ctor => self.ast.names.constructor(canon),
            Shape::Dtor => self.ast.names.destructor(canon),
            Shape::Conv(t) => {
                let c = self.ast.types.canonical(t);
                self.ast.names.conversion(c)
            }
            Shape::Assign => DeclName::operator(OverloadedOperator::Equal),
        };
        if let Shape::Conv(t) = shape
            && in_class
            && !self.is_dependent_ctx(class)
        {
            self.warn_class_conversion(class, t, sloc);
        }
        let (ctx, lexical) = self.decl_contexts(d);
        let ctx = self.ast.definition(ctx).unwrap_or(ctx);
        let fkind = shape.kind();
        // Redeclarations ([class.mem.general]/5, [class.mfct]/3).
        if !in_class && !self.is_dependent_ctx(class) {
            match shape {
                Shape::Ctor => {
                    for k in
                        [SpecialKind::DefaultCtor, SpecialKind::CopyCtor, SpecialKind::MoveCtor]
                    {
                        self.ensure_implicit(class, k);
                    }
                }
                Shape::Dtor => self.ensure_implicit(class, SpecialKind::Dtor),
                Shape::Assign => {
                    self.ensure_implicit(class, SpecialKind::CopyAssign);
                    self.ensure_implicit(class, SpecialKind::MoveAssign);
                }
                Shape::Conv(_) => {}
            }
        }
        let templ = self.templ.cur.is_some();
        let cands: Vec<DeclId> = self.ast.lookup_local(class, name).iter().collect();
        let mut matched = None;
        for c in cands {
            let c = if templ {
                match self.pattern_of(c) {
                    Some(p) => self.ast.latest(p),
                    None => continue,
                }
            } else {
                c
            };
            if self.kind_of(c) != fkind || (!templ && self.ast.described_template(c).is_some()) {
                continue;
            }
            let cty = self.ast.decls.get(c).ty().unwrap_or(QualType::ERROR);
            if self.same_ignoring_exc_spec(cty, ty) {
                matched = Some(c);
                break;
            }
        }
        let mut prev_ok = None;
        let mut ty = ty;
        let mut redecl_invalid = false;
        if in_class {
            if let Some(p) = matched
                && !*invalid
            {
                if self.ast.decls.get(p).flags().contains(DeclFlags::INVALID) {
                    redecl_invalid = true;
                } else {
                    let m = match shape {
                        Shape::Assign => self.diag(DiagId::sema_member_redeclared, sloc),
                        _ => self.diag(DiagId::sema_special_redeclared, sloc).arg(shape.sel()),
                    };
                    let m = self.note_prev(m, p, false);
                    self.emit(m);
                    redecl_invalid = true;
                }
            }
        } else {
            match matched {
                Some(p) if !self.is_user_declared(p) => {
                    let kind = self.special_kind_of(p).unwrap_or(SpecialKind::DefaultCtor);
                    let m = self.diag(DiagId::sema_define_implicit_member, sloc).arg(kind.sel());
                    self.emit(m);
                    return None;
                }
                Some(p) => {
                    prev_ok = Some(p);
                    let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                    if !self.ast.types.same_type(pty, ty) {
                        let (pe, ne) = (self.written_exc_spec(pty), self.written_exc_spec(ty));
                        if ne == ExcSpec::None || matches!(pe, ExcSpec::Deferred(_)) {
                            // An unwritten specification is inherited
                            // ([except.spec]/5, the implicit one of a
                            // destructor, [except.spec]/8).
                            if ne == ExcSpec::None {
                                ty = pty;
                            }
                        } else {
                            let m = self.diag(DiagId::sema_exception_spec_mismatch, sloc);
                            let m = self.note_prev(m, p, false);
                            self.emit(m);
                        }
                    }
                }
                None => {
                    let what = if templ { None } else { Some(self.decl_name_str(name)) };
                    let what = what.unwrap_or_else(|| self.decl_name_str(name));
                    let m = self
                        .diag(DiagId::sema_member_def_no_match, sloc)
                        .arg(what)
                        .arg(self.qualified_decl_name(ctx));
                    self.emit(m);
                    redecl_invalid = true;
                }
            }
        }
        let mut spec = FnSpec::new();
        if ds.inline.is_some() {
            spec = spec.with_inline(true);
        }
        if ds.constexpr.is_some() {
            spec = spec.with_constexpr_kind(ConstexprKind::Constexpr).with_implicit_inline(true);
        }
        spec = spec.with_variadic(variadic && shape != Shape::Dtor);
        if let Some(p) = prev_ok
            && self.ast.decls.function(p).is_some_and(|x| x.spec.is_inline())
        {
            spec = spec.with_implicit_inline(true);
        }
        if is_static && !in_class {
            self.error(DiagId::sema_static_out_of_class, static_loc_of(ds));
        }
        let explicit =
            self.check_explicit_spec(ds, d, true, matches!(shape, Shape::Ctor | Shape::Conv(_)));
        let explicit_value = match (explicit, ds.explicit_expr) {
            (true, Some(e)) => self.explicit_value(e),
            (e, _) => Some(e),
        };
        if explicit_value != Some(false) && explicit {
            spec = spec.with_explicit(true);
        }
        let fparams: Vec<ParamInfo> =
            if shape == Shape::Dtor { Vec::new() } else { params.clone() };
        let (defaults, redefined) = self.merge_default_args(prev_ok, false, &fparams);
        if redefined {
            prev_ok = None;
            redecl_invalid = true;
        }
        let mut h = DeclHead::new(ctx, name, sloc);
        h.lexical = Some(lexical);
        self.mark_pattern_next();
        let site = crate::class::VirtualSite {
            class: ctx,
            method: true,
            in_class,
            friend: false,
            prev: prev_ok,
            name,
        };
        let vds = ds_virt.as_ref().unwrap_or(ds);
        let virt = self.virtual_decl(vds, d, ty, site, &mut spec);
        let f = self.builder().function(h, fkind, ty, spec, prev_ok);
        self.finish_virtual_decl(f, &virt);
        if redecl_invalid || *invalid {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
        if explicit && let Some(e) = ds.explicit_expr {
            let _ = self.ast.decls.cold.explicit_expr.insert(f, e);
        }
        self.create_params(f, &fparams, &defaults);
        // A destructor without an exception specification gets the
        // implicit one ([except.spec]/8), computed when needed.
        if shape == Shape::Dtor
            && prev_ok.is_none()
            && d.late_noexcept.is_none()
            && self.written_exc_spec(ty) == ExcSpec::None
        {
            let t = self.with_exc_spec(ty, ExcSpec::Deferred(f));
            self.ast.decls.set_ty(f, t);
        }
        if shape == Shape::Ctor && !*invalid && !redecl_invalid {
            self.check_copy_ctor_by_value(f, class, &fparams);
        }
        Some(f)
    }

    /// The cv- and ref-qualifier checks of a constructor (`sel` "0") or
    /// destructor ("1") ([class.ctor.general]/3, [class.dtor]/2).
    fn check_ctor_dtor_quals(
        &mut self,
        rparen: Loc,
        quals: kiln_ast::types::Quals,
        ref_qual: RefQual,
        sel: &str,
        invalid: &mut bool,
    ) {
        if quals.is_empty() && ref_qual == RefQual::None {
            return;
        }
        let written = self.written_fn_quals(rparen);
        for (q, sp) in [
            (kiln_ast::types::Quals::CONST, "const"),
            (kiln_ast::types::Quals::VOLATILE, "volatile"),
        ] {
            if quals.contains(q) {
                let loc = written.iter().find(|w| w.0 == sp).map_or(rparen, |w| w.1);
                let m = self.diag(DiagId::sema_ctor_dtor_qualifier, loc).arg(sp).arg(sel);
                self.emit(m);
                *invalid = true;
            }
        }
        if ref_qual != RefQual::None {
            let sp = if ref_qual == RefQual::LValue { "&" } else { "&&" };
            let loc = written.iter().find(|w| w.0 == sp).map_or(rparen, |w| w.1);
            let m = self.diag(DiagId::sema_ctor_dtor_ref_qualifier, loc).arg(sp).arg(sel);
            self.emit(m);
            *invalid = true;
        }
    }

    /// The qualifier tokens (`const`, `volatile`, `&`, `&&`) written
    /// after the `)` at `rparen`, with their locations (the declarator
    /// keeps the qualifiers, not where they were written).
    fn written_fn_quals(&self, rparen: Loc) -> Vec<(&'static str, Loc)> {
        let mut out = Vec::new();
        let sm = self.diags.sm();
        let Some((file, off)) = sm.offset_of(rparen) else { return out };
        let text = sm.text(file).as_bytes();
        let mut i = off as usize + 1;
        while i < text.len() {
            while i < text.len() && text[i].is_ascii_whitespace() {
                i += 1;
            }
            let start = i;
            let word: &'static str = if text[i..].starts_with(b"&&") {
                i += 2;
                "&&"
            } else if text[i..].starts_with(b"&") {
                i += 1;
                "&"
            } else {
                while i < text.len() && (text[i].is_ascii_alphanumeric() || text[i] == b'_') {
                    i += 1;
                }
                match &text[start..i] {
                    b"const" => "const",
                    b"volatile" => "volatile",
                    _ => break,
                }
            };
            let Ok(o) = u32::try_from(start) else { break };
            out.push((word, sm.loc(file, o)));
        }
        out
    }

    /// The location of the first token after `loc`'s (one byte long)
    /// token: the class name after a destructor's `~`.
    fn next_token_loc(&self, loc: Loc) -> Loc {
        let sm = self.diags.sm();
        let Some((file, off)) = sm.offset_of(loc) else { return loc };
        let text = sm.text(file).as_bytes();
        let mut i = off as usize + 1;
        while i < text.len() && text[i].is_ascii_whitespace() {
            i += 1;
        }
        u32::try_from(i).map_or(loc, |o| sm.loc(file, o))
    }

    /// [class.dtor]/1: the class-name after `~` names the class
    /// (clang's `ActOnDestructorName`). Reports and returns false
    /// otherwise.
    fn dtor_name_matches(
        &mut self,
        class: DeclId,
        n: Symbol,
        qualifier: Option<DeclId>,
        tilde: Loc,
    ) -> bool {
        let mut found = if qualifier.is_some() {
            self.lookup_in_class(class, n, LookupKind::Ordinary).decls
        } else {
            Vec::new()
        };
        if found.is_empty() {
            found = self.lookup_name(n, LookupKind::Ordinary).decls;
        }
        let names_class = |s: &Self, x: DeclId| {
            let k = s.kind_of(x);
            (crate::lookup::is_tag_kind(k) || k == DeclKind::Typedef || k == DeclKind::TypeAlias)
                && s.ast.decls.get(x).ty().is_some_and(|t| s.is_class_type_of(t, class))
        };
        if found.iter().any(|&x| names_class(self, x)) || self.is_dependent_ctx(class) {
            return true;
        }
        let loc = self.next_token_loc(tilde);
        if found.is_empty() {
            let m = self
                .diag(DiagId::sema_dtor_undeclared_name, loc)
                .arg(self.ast.symbols().as_str(n).to_owned());
            self.emit(m);
        } else {
            self.error(DiagId::sema_dtor_class_name, loc);
        }
        false
    }

    /// `-Wclass-conversion` ([class.conv.fct]/1 note: a conversion to the
    /// class itself, a base or `void` is never used).
    fn warn_class_conversion(&mut self, class: DeclId, target: QualType, loc: Loc) {
        let t = self.non_reference(target);
        let t = self.ast.types.canonical(t).without_quals();
        let class_ty = self.class_type(class);
        let cls = self.ty_arg(class_ty);
        if let Some(r) = self.ast.types.as_record(t) {
            if self.ast.first(r) == self.ast.first(class) {
                let m = self.diag(DiagId::sema_conv_to_self, loc).arg(cls);
                self.emit(m);
            } else if crate::conv::bases::is_derived_from(self, class, r) {
                let m = self.diag(DiagId::sema_conv_to_base, loc).arg(cls).arg(self.ty_arg(t));
                self.emit(m);
            }
        } else if self.ast.types.is_void(t) {
            let m = self.diag(DiagId::sema_conv_to_void, loc).arg(cls).arg(self.ty_arg(t));
            self.emit(m);
        }
    }

    /// [class.copy.ctor]/5: a constructor whose first parameter is the
    /// class itself (by value) and whose others have defaults is
    /// ill-formed (clang `CheckConstructor`).
    fn check_copy_ctor_by_value(&mut self, f: DeclId, class: DeclId, params: &[ParamInfo]) {
        let Some(first) = params.first() else { return };
        if params[1..].iter().any(|p| p.default_arg == crate::declspec::DefaultArg::None) {
            return;
        }
        let t = self.ast.types.canonical(first.ty).without_quals();
        if self.ast.types.as_record(t).is_some_and(|r| self.ast.first(r) == self.ast.first(class))
            && !self.ast.types.is_reference(first.ty)
        {
            let loc = match first.name {
                Some((_, l)) => l,
                None if first.default_arg == crate::declspec::DefaultArg::None
                    && first.range.1.is_valid() =>
                {
                    first.range.1
                }
                None => first.loc,
            };
            self.error(DiagId::sema_copy_ctor_by_value, loc);
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
    }

    /// Function type `ty` with return type `ret`, parameters `params`
    /// (kept when `None`) and, with `strip`, no cv- or ref-qualifiers.
    pub(crate) fn rebuild_fn_type(
        &mut self,
        ty: QualType,
        ret: QualType,
        params: Option<&[QualType]>,
        strip: bool,
    ) -> QualType {
        let t = &self.ast.types;
        let Some(p) = t.proto_of(ty.ty()).or_else(|| t.proto(ty)).copied() else { return ty };
        let ps = match params {
            Some(ps) => ps.to_vec(),
            None => self.ast.types.params(&p).to_vec(),
        };
        let mut info = p.info();
        if strip {
            info = info.with_this_quals(kiln_ast::types::Quals::NONE).with_ref_qual(RefQual::None);
        }
        if params.is_some() {
            info = info.with_variadic(false);
        }
        let exc = self.written_exc_spec(ty);
        let _: FuncInfo = info;
        self.ast.types.function(ret, &ps, info, exc)
    }

    /// Function type `ty` with exception specification `exc`.
    pub(crate) fn with_exc_spec(&mut self, ty: QualType, exc: ExcSpec) -> QualType {
        let t = &self.ast.types;
        let Some(p) = t.proto_of(ty.ty()).or_else(|| t.proto(ty)).copied() else { return ty };
        let ps = self.ast.types.params(&p).to_vec();
        self.ast.types.function(p.ret(), &ps, p.info(), exc)
    }
}

/// Where `static` was written.
fn static_loc_of(ds: &DeclSpec) -> Loc {
    match ds.storage {
        Some((_, l)) => l,
        None => ds.loc,
    }
}
