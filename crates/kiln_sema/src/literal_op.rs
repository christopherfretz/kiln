//! Literal operators ([over.literal]) and user-defined literals
//! ([lex.ext]), spec/sema-expr.md §2.6 (clang
//! `CheckLiteralOperatorDeclaration`, `BuildLiteralOperatorCall`).

use kiln_ast::decl::DeclKind;
use kiln_ast::types::{BuiltinType as B, TemplateArg, TypeView};
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;

use kiln_ast::ExprId;
use kiln_ast::expr::{ExprKind, Radix, StringKind};
use kiln_ast::names::DeclName;
use kiln_ast::strings::StringEncoding;
use kiln_lex::literal::{FloatLiteral, FloatSuffix, IntegerLiteral};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::expr::{UdOperand, ast_radix};
use crate::literal;
use crate::lookup::LookupKind;

/// A literal operator template's form ([over.literal]/5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TemplForm {
    /// `template<char...>`: a numeric literal operator template.
    CharPack,
    /// `template<C c>` with a class type `C` (C++20): a string literal
    /// operator template.
    ClassNttp,
    /// `template<class C, C...>`: the GNU string literal operator
    /// template.
    GnuPack,
}

/// What one declaration found for `operator""X` accepts ([lex.ext]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LitOpForm {
    /// A function with one parameter of this (canonical, unqualified)
    /// type.
    One(QualType),
    /// A function with two parameters, the first of this (canonical,
    /// unqualified) type.
    Two(QualType),
    /// A literal operator template.
    Templ(TemplForm),
    /// Anything else (an invalid literal operator).
    Other,
}

impl Sema<'_, '_> {
    /// True for the character types a literal operator accepts
    /// ([over.literal]/3): `char`, `wchar_t`, `char8_t`, `char16_t`,
    /// `char32_t`.
    pub(crate) fn is_literal_char_type(&self, t: QualType) -> bool {
        matches!(
            self.ast.types.builtin(self.ast.types.canonical(t).without_quals()),
            Some(B::Char | B::WChar | B::Char8 | B::Char16 | B::Char32)
        )
    }

    /// The [over.literal] rules for literal operator `f`: reports the
    /// first violation and returns false, mirroring clang
    /// `CheckLiteralOperatorDeclaration` (the `extern "C"` check is not
    /// modelled). Warns for reserved suffixes ([usrlit.suffix]).
    pub(crate) fn check_literal_operator_decl(&mut self, f: DeclId) -> bool {
        let loc = self.ast.decls.get(f).loc();
        let name = self.ast.decls.get(f).name();
        let spelled = self.decl_name_str(name);
        if self.kind_of(f) == DeclKind::CXXMethod {
            let m = self
                .diag(DiagId::sema_literal_operator_outside_namespace, loc)
                .arg(format!("'{spelled}'"));
            self.emit(m);
            return false;
        }
        if kiln_abi::mangle::is_extern_c(&self.ast, f) {
            let mut c = self.ast.decls.get(f).ctx();
            while let Some(x) = c
                && self.kind_of(x) != DeclKind::LinkageSpec
            {
                c = self.ast.decls.get(x).ctx();
            }
            let mut m = self.diag(DiagId::sema_literal_operator_extern_c, loc);
            if let Some(ls) = c {
                m = m.with_note(
                    self.diag(DiagId::note_extern_c_begins_here, self.ast.decls.get(ls).loc()),
                );
            }
            self.emit(m);
            return false;
        }
        let params: Vec<DeclId> = self
            .ast
            .decls
            .function(f)
            .map(|fd| self.ast.lists.iter::<DeclId>(fd.params).collect())
            .unwrap_or_default();
        if let Some(head) = self.templ.cur {
            let list = *self.ast.templates.param_list(head);
            let tps: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
            if !tps.is_empty() {
                if !params.is_empty() {
                    self.error(DiagId::sema_literal_operator_template_with_params, loc);
                    return false;
                }
                match self.literal_template_form(&tps) {
                    Some(TemplForm::GnuPack) => {
                        let m = self.diag(DiagId::sema_string_literal_operator_template, loc);
                        self.emit(m);
                    }
                    Some(_) => {}
                    None => {
                        self.error(DiagId::sema_literal_operator_template, list.template_loc);
                        return false;
                    }
                }
                return self.check_literal_operator_tail(f, &params);
            }
        }
        if !self.check_literal_operator_params(f, &params) {
            return false;
        }
        self.check_literal_operator_tail(f, &params)
    }

    /// The parameter types of non-template literal operator `f`
    /// ([over.literal]/3).
    fn check_literal_operator_params(&mut self, f: DeclId, params: &[DeclId]) -> bool {
        let loc = self.ast.decls.get(f).loc();
        let ptys: Vec<QualType> =
            params.iter().map(|&p| self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR)).collect();
        if ptys.iter().any(|&t| self.is_error_type(t)) {
            return true;
        }
        let ploc = |s: &Self, p: DeclId| {
            s.ast.decls.cold.begin_loc.get(p).copied().unwrap_or(s.ast.decls.get(p).loc())
        };
        match ptys.as_slice() {
            [t] => {
                let u = self.ast.types.canonical(*t).without_quals();
                if u == QualType::ULONGLONG
                    || u == QualType::LONGDOUBLE
                    || self.is_literal_char_type(u)
                {
                } else if let Some(pt) = self.ast.types.pointee(u) {
                    let q = self.ast.types.effective_quals(pt);
                    let pc = self.ast.types.canonical(pt).without_quals();
                    if !(pc == QualType::CHAR && q.is_const() && !q.is_volatile()) {
                        let m = self
                            .diag(DiagId::sema_literal_operator_param, ploc(self, params[0]))
                            .arg(format!("'{}'", self.ty_str(*t)))
                            .arg("'const char *'");
                        self.emit(m);
                        return false;
                    }
                } else if self.ast.types.is_floating(u) || self.ast.types.is_integral(u) {
                    let want = if self.ast.types.is_floating(u) {
                        "'long double'"
                    } else {
                        "'unsigned long long'"
                    };
                    let m = self
                        .diag(DiagId::sema_literal_operator_param, ploc(self, params[0]))
                        .arg(format!("'{}'", self.ty_str(*t)))
                        .arg(want);
                    self.emit(m);
                    return false;
                } else {
                    self.error(DiagId::sema_literal_operator_invalid_param, ploc(self, params[0]));
                    return false;
                }
            }
            [a, b] => {
                let first_ok = self.ast.types.pointee(*a).is_some_and(|pt| {
                    let q = self.ast.types.effective_quals(pt);
                    q.is_const() && !q.is_volatile() && self.is_literal_char_type(pt)
                });
                if !first_ok {
                    let m = self
                        .diag(DiagId::sema_literal_operator_param, ploc(self, params[0]))
                        .arg(format!("'{}'", self.ty_str(*a)))
                        .arg("'const char *'");
                    self.emit(m);
                    return false;
                }
                let st = self.size_type();
                if self.ast.types.canonical(*b).without_quals() != self.ast.types.canonical(st) {
                    let m = self
                        .diag(DiagId::sema_literal_operator_param, ploc(self, params[1]))
                        .arg(format!("'{}'", self.ty_str(*b)))
                        .arg(format!("'{}'", self.ty_str(st)));
                    self.emit(m);
                    return false;
                }
            }
            _ => {
                self.error(DiagId::sema_literal_operator_bad_param_count, loc);
                return false;
            }
        }
        true
    }

    /// The literal operator template form of template parameters `tps`
    /// ([over.literal]/5), if they form one: one `char` pack, one
    /// non-pack class-type NTTP (C++20 string literal operator template),
    /// or the GNU `class C, C...` form.
    fn literal_template_form(&self, tps: &[DeclId]) -> Option<TemplForm> {
        let nttp_ty = |d: DeclId| {
            (self.kind_of(d) == DeclKind::NonTypeTemplateParm)
                .then(|| self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR))
        };
        match tps {
            [p] => {
                let t = nttp_ty(*p)?;
                let pack = self.is_pack_decl(*p);
                let c = self.ast.types.canonical(t).without_quals();
                if pack && c == QualType::CHAR {
                    return Some(TemplForm::CharPack);
                }
                let deduced = matches!(
                    self.ast.types.view(c.ty()),
                    TypeView::DeducedTemplateSpecialization { .. }
                );
                (!pack && (self.ast.types.as_record(c).is_some() || deduced))
                    .then_some(TemplForm::ClassNttp)
            }
            [a, b] => {
                if self.kind_of(*a) != DeclKind::TemplateTypeParm || self.is_pack_decl(*a) {
                    return None;
                }
                let t = nttp_ty(*b)?;
                if !self.is_pack_decl(*b) {
                    return None;
                }
                let y = self.ast.template_parm(*a)?;
                let (depth, index) = match self.ast.types.view(t.without_quals().ty()) {
                    TypeView::TemplateTypeParm(d) => {
                        let x = self.ast.template_parm(d)?;
                        (u32::from(x.depth), u32::from(x.index))
                    }
                    TypeView::CanonTemplateTypeParm { depth, index, .. } => (depth, index),
                    _ => return None,
                };
                (depth == u32::from(y.depth) && index == u32::from(y.index))
                    .then_some(TemplForm::GnuPack)
            }
            _ => None,
        }
    }

    /// The checks shared by every literal operator form: no default
    /// arguments ([over.literal]/7), then the reserved-suffix warning.
    fn check_literal_operator_tail(&mut self, f: DeclId, params: &[DeclId]) -> bool {
        if let Some(&p) = params.iter().find(|&&p| self.param_has_default(p)) {
            let at = self
                .ast
                .decls
                .param(p)
                .and_then(|pd| pd.default_arg)
                .map_or_else(|| self.ast.decls.get(p).loc(), |e| self.begin_loc(e));
            self.error(DiagId::sema_literal_operator_default_argument, at);
            return false;
        }
        let Some(sfx) = self.ast.decls.get(f).name().as_literal_operator() else { return true };
        let s = crate::decl::sym_str(self, sfx);
        let status = if !s.starts_with('_') {
            Some("1")
        } else if s.contains("__") {
            Some("2")
        } else {
            None
        };
        if let Some(status) = status {
            // clang `StringLiteralParser::isValidUDSuffix`: the numeric
            // library suffixes and `sv` (sema has no -std; C++20's set).
            let valid = s.starts_with('_')
                || s == "sv"
                || kiln_lex::literal::is_valid_ud_suffix(&s, kiln_lex::Std::Cxx26);
            let loc = self.ast.decls.get(f).loc();
            let m = self.diag(DiagId::sema_user_literal_reserved, loc).arg(status).arg(if valid {
                "1"
            } else {
                "0"
            });
            self.emit(m);
        }
        true
    }

    // -- user-defined literals ([lex.ext]) ----------------------------------------

    /// The declarations unqualified lookup finds for `operator""sfx`, with
    /// what each accepts (clang `LookupLiteralOperator`).
    fn literal_op_forms(&self, sfx: Symbol) -> Vec<(DeclId, LitOpForm)> {
        let found = self.lookup_name_dn(DeclName::literal_operator(sfx), LookupKind::Ordinary);
        found
            .decls
            .iter()
            .map(|&d| {
                if self.kind_of(d) == DeclKind::FunctionTemplate {
                    let tps = self.template_params(d);
                    let form =
                        self.literal_template_form(&tps).map_or(LitOpForm::Other, LitOpForm::Templ);
                    return (d, form);
                }
                let Some(fd) = self.ast.decls.function(d) else { return (d, LitOpForm::Other) };
                let ps: Vec<DeclId> = self.ast.lists.iter::<DeclId>(fd.params).collect();
                let pty = |p: DeclId| {
                    let t = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                    self.ast.types.canonical(t).without_quals()
                };
                let form = match ps.as_slice() {
                    [a] => LitOpForm::One(pty(*a)),
                    [a, _] => LitOpForm::Two(pty(*a)),
                    _ => LitOpForm::Other,
                };
                (d, form)
            })
            .collect()
    }

    /// True if `t` (canonical) is `const elem *`.
    fn is_const_ptr_to(&self, t: QualType, elem: QualType) -> bool {
        self.ast.types.pointee(t).is_some_and(|p| {
            let q = self.ast.types.effective_quals(p);
            q.is_const() && !q.is_volatile() && self.ast.types.canonical(p).without_quals() == elem
        })
    }

    /// `operator""sfx(args)`, resolved by ordinary overload resolution
    /// over the declarations unqualified lookup finds. kiln builds a plain
    /// `CallExpr` (clang: `UserDefinedLiteral`, a `CallExpr` subclass);
    /// spec/sema-expr.md §2.6.
    fn udl_call(&mut self, sfx: Symbol, args: &[ExprId], loc: Loc, ud_loc: Loc) -> ExprId {
        let callee = self.act_on_call_name_dn(DeclName::literal_operator(sfx), ud_loc);
        self.act_on_call(callee, args, loc, loc)
    }

    /// `operator""sfx<targs>()` for the literal operator templates
    /// `templates`.
    fn udl_template_call(
        &mut self,
        sfx: Symbol,
        templates: &[DeclId],
        targs: &[TemplateArg],
        loc: Loc,
        ud_loc: Loc,
    ) -> ExprId {
        let list = self.ast.types.arg_list(targs);
        let name = DeclName::literal_operator(sfx);
        let ule =
            self.builder().unresolved_lookup(name, templates, false, None, Some(list), ud_loc);
        self.act_on_call(ule, &[], loc, loc)
    }

    /// The templates of `forms` of form `want`.
    fn templates_of(forms: &[(DeclId, LitOpForm)], want: TemplForm) -> Vec<DeclId> {
        forms.iter().filter(|f| f.1 == LitOpForm::Templ(want)).map(|f| f.0).collect()
    }

    /// An integer literal with a ud-suffix ([lex.ext]/3): `raw` is its
    /// spelling without the suffix.
    pub fn act_on_integer_udl(
        &mut self,
        lit: &IntegerLiteral<'_>,
        raw: &str,
        loc: Loc,
        ud_loc: Loc,
    ) -> ExprId {
        let Some(sfx_str) = lit.ud_suffix else {
            return self.act_on_integer_literal(lit, loc, ud_loc);
        };
        let sfx = self.intern(sfx_str);
        let forms = self.literal_op_forms(sfx);
        if forms.iter().any(|f| f.1 == LitOpForm::One(QualType::ULONGLONG)) {
            let Some(v) = lit.value().and_then(|v| u64::try_from(v).ok()) else {
                self.error(DiagId::sema_int_literal_too_large, loc);
                return self.recover_err(&[], loc);
            };
            let arg = self.builder().int_literal(QualType::ULONGLONG, v, ast_radix(lit.radix), loc);
            return self.udl_call(sfx, &[arg], loc, ud_loc);
        }
        if lit.imaginary && !self.has_raw_form(&forms) {
            // clang: lookup failure is not fatal for `1i`; the GNU
            // imaginary constant remains.
            let plain = IntegerLiteral { ud_suffix: None, ..lit.clone() };
            return self.act_on_integer_literal(&plain, loc, ud_loc);
        }
        self.raw_udl(sfx, sfx_str, &forms, raw, UdOperand::Integer, loc, ud_loc)
    }

    /// A floating literal with a ud-suffix ([lex.ext]/4).
    pub fn act_on_float_udl(
        &mut self,
        lit: &FloatLiteral<'_>,
        raw: &str,
        loc: Loc,
        ud_loc: Loc,
    ) -> ExprId {
        let Some(sfx_str) = lit.ud_suffix else {
            return self.act_on_float_literal(lit, loc, ud_loc);
        };
        let sfx = self.intern(sfx_str);
        let forms = self.literal_op_forms(sfx);
        if forms.iter().any(|f| f.1 == LitOpForm::One(QualType::LONGDOUBLE)) {
            let cooked = FloatLiteral {
                suffix: FloatSuffix::L,
                ud_suffix: None,
                imaginary: false,
                ..lit.clone()
            };
            let arg = self.act_on_float_literal(&cooked, loc, ud_loc);
            if self.is_err_expr(arg) {
                return arg;
            }
            return self.udl_call(sfx, &[arg], loc, ud_loc);
        }
        if lit.imaginary && !self.has_raw_form(&forms) {
            let plain = FloatLiteral { ud_suffix: None, ..lit.clone() };
            return self.act_on_float_literal(&plain, loc, ud_loc);
        }
        self.raw_udl(sfx, sfx_str, &forms, raw, UdOperand::Float, loc, ud_loc)
    }

    /// Whether `forms` has a raw literal operator or a numeric literal
    /// operator template ([lex.ext]/3-4).
    fn has_raw_form(&self, forms: &[(DeclId, LitOpForm)]) -> bool {
        forms
            .iter()
            .any(|f| matches!(f.1, LitOpForm::One(t) if self.is_const_ptr_to(t, QualType::CHAR)))
            || !Self::templates_of(forms, TemplForm::CharPack).is_empty()
    }

    /// The raw literal operator or numeric literal operator template call
    /// for a numeric literal spelled `raw` ([lex.ext]/3-4).
    #[allow(clippy::too_many_arguments, reason = "the literal's parts")]
    fn raw_udl(
        &mut self,
        sfx: Symbol,
        sfx_str: &str,
        forms: &[(DeclId, LitOpForm)],
        raw: &str,
        operand: UdOperand,
        loc: Loc,
        ud_loc: Loc,
    ) -> ExprId {
        if forms
            .iter()
            .any(|f| matches!(f.1, LitOpForm::One(t) if self.is_const_ptr_to(t, QualType::CHAR)))
        {
            let id = self.ast.strings.intern(StringEncoding::Ordinary, raw.as_bytes());
            let ty = literal::string_literal_type(
                self,
                kiln_lex::Encoding::Ordinary,
                raw.len() as u64 + 1,
            );
            let arg = self.builder().string_literal(ty, StringKind::Ordinary, id, loc);
            return self.udl_call(sfx, &[arg], loc, ud_loc);
        }
        let templates = Self::templates_of(forms, TemplForm::CharPack);
        if !templates.is_empty() {
            let targs: Vec<TemplateArg> = raw
                .bytes()
                .map(|b| {
                    let v = self.int_const(QualType::CHAR, i128::from(b));
                    TemplateArg::value(QualType::CHAR, v)
                })
                .collect();
            return self.udl_template_call(sfx, &templates, &targs, loc, ud_loc);
        }
        self.reject_ud_suffix(Some(sfx_str), ud_loc, operand);
        self.recover_err(&[], loc)
    }

    /// A character literal `e` with ud-suffix `sfx_str` ([lex.ext]/6).
    pub(crate) fn char_udl(&mut self, e: ExprId, sfx_str: &str, loc: Loc, ud_loc: Loc) -> ExprId {
        if self.is_err_expr(e) {
            return e;
        }
        let ty = self.ast.types.canonical(self.ety(e)).without_quals();
        let sfx = self.intern(sfx_str);
        let forms = self.literal_op_forms(sfx);
        if forms.iter().any(|f| f.1 == LitOpForm::One(ty)) {
            return self.udl_call(sfx, &[e], loc, ud_loc);
        }
        self.reject_ud_suffix(Some(sfx_str), ud_loc, UdOperand::Char(ty));
        self.recover_err(&[], loc)
    }

    /// A string literal `e` (element type `elem`) with ud-suffix
    /// `sfx_str` ([lex.ext]/5).
    pub(crate) fn string_udl(
        &mut self,
        e: ExprId,
        sfx_str: &str,
        elem: QualType,
        loc: Loc,
        ud_loc: Loc,
    ) -> ExprId {
        if self.is_err_expr(e) || self.ast.exprs.get(e).kind() != ExprKind::StringLiteral {
            return e;
        }
        let sfx = self.intern(sfx_str);
        let forms = self.literal_op_forms(sfx);
        let templates = Self::templates_of(&forms, TemplForm::ClassNttp);
        if !templates.is_empty() {
            return self.udl_template_call(
                sfx,
                &templates,
                &[TemplateArg::expr(e, None)],
                loc,
                ud_loc,
            );
        }
        let (enc, bytes) = self.ast.strings.get(self.ast.exprs.string_lit(e));
        let width = match enc {
            StringEncoding::Utf16 | StringEncoding::Wide16 => 2,
            StringEncoding::Utf32 | StringEncoding::Wide32 => 4,
            _ => 1,
        };
        let units: Vec<u32> = bytes
            .chunks(width)
            .map(|c| c.iter().rev().fold(0u32, |v, &b| (v << 8) | u32::from(b)))
            .collect();
        let templates = Self::templates_of(&forms, TemplForm::GnuPack);
        if !templates.is_empty() {
            let mut targs = vec![TemplateArg::ty(elem)];
            for u in &units {
                let v = self.int_const(elem, i128::from(*u));
                targs.push(TemplateArg::value(elem, v));
            }
            return self.udl_template_call(sfx, &templates, &targs, loc, ud_loc);
        }
        if forms.iter().any(|f| matches!(f.1, LitOpForm::Two(t) if self.is_const_ptr_to(t, elem))) {
            let st = self.size_type();
            let len = self.builder().int_literal(st, units.len() as u64, Radix::Decimal, loc);
            return self.udl_call(sfx, &[e, len], loc, ud_loc);
        }
        self.reject_ud_suffix(Some(sfx_str), ud_loc, UdOperand::String(elem));
        self.recover_err(&[], loc)
    }
}
