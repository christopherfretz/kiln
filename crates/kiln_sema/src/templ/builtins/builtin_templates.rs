//! The builtin templates `__make_integer_seq` and `__type_pack_element`
//! (spec/sema-templates.md §9.3). Both are implicit `BuiltinTemplate`
//! declarations entered in the translation unit's lookup table (not its
//! member list) when `Sema` is created; a template-id naming one is
//! checked here instead of through a template parameter list.

use kiln_ast::Ast;
use kiln_ast::ids::TemplateNameId;
use kiln_ast::names::DeclName;
use kiln_ast::types::{ArgView, TemplateArg};
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_oven::EvalContext;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::{self, ConvCtx};
use crate::templ::actions::TemplateIdRef;

/// The inline payload of `__make_integer_seq`'s declaration.
pub(crate) const MAKE_INTEGER_SEQ: u32 = 0;
/// The inline payload of `__type_pack_element`'s declaration.
pub(crate) const TYPE_PACK_ELEMENT: u32 = 1;

/// The longest sequence `__make_integer_seq` builds (kiln's limit; the
/// arguments are built eagerly, one per element).
const MAX_SEQ_LEN: i128 = 1 << 20;

/// Declares the builtin templates in `ast`'s translation unit.
pub(crate) fn declare_builtin_templates(ast: &mut Ast) {
    let tu = ast.tu;
    for (name, which) in
        [("__make_integer_seq", MAKE_INTEGER_SEQ), ("__type_pack_element", TYPE_PACK_ELEMENT)]
    {
        let n = DeclName::ident(ast.symbols().intern(name));
        let _ = ast.builder().builtin_template(tu, n, which);
    }
}

impl Sema<'_, '_> {
    /// `name<args>` where `name` is builtin template `d` (§9.3).
    /// Dependent arguments give a dependent specialization; otherwise
    /// the result is the computed type under `TemplateSpecialization`
    /// sugar (clang).
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub(crate) fn builtin_template_id(
        &mut self,
        name: TemplateNameId,
        d: DeclId,
        args: &[TemplateArg],
        locs: &[Loc],
        loc: Loc,
    ) -> Result<TemplateIdRef, ()> {
        let list = self.ast.types.arg_list(args);
        let flags = self.ast.types.arg_list_flags(list);
        if flags.intersects(kiln_ast::types::TypeFlags::DEPENDENT_INSTANTIATION)
            || args.iter().any(|&a| self.is_pack_expansion_arg(a))
        {
            return Ok(TemplateIdRef::Type(
                self.ast.types.template_specialization(name, list, None, false),
            ));
        }
        let at = |i: usize| locs.get(i).copied().unwrap_or(loc);
        let result = match self.ast.decls.get(d).data() {
            MAKE_INTEGER_SEQ => self.make_integer_seq(d, args, &at, loc)?,
            _ => self.type_pack_element(d, args, &at, loc)?,
        };
        Ok(TemplateIdRef::Type(self.ast.types.template_specialization(
            name,
            list,
            Some(result),
            false,
        )))
    }

    /// `too few/many template arguments for template '__x'`.
    fn builtin_template_arity(&self, d: DeclId, too_many: bool, loc: Loc) {
        let name = self.qualified_decl_name(d);
        let msg = self
            .diag(DiagId::sema_template_arg_list_different_arity, loc)
            .arg(u32::from(too_many))
            .arg(6u32)
            .arg(name);
        self.emit(msg);
    }

    /// `__make_integer_seq<Seq, T, N>`: `Seq<T, 0, ..., N-1>`.
    fn make_integer_seq(
        &mut self,
        d: DeclId,
        args: &[TemplateArg],
        at: &dyn Fn(usize) -> Loc,
        loc: Loc,
    ) -> Result<QualType, ()> {
        if args.len() != 3 {
            self.builtin_template_arity(d, args.len() > 3, loc);
            return Err(());
        }
        let ArgView::Template(seq) = args[0].view() else {
            self.error(DiagId::sema_template_arg_must_be_template, at(0));
            return Err(());
        };
        let ArgView::Type(t) = args[1].view() else {
            self.error(DiagId::sema_template_arg_must_be_type, at(1));
            return Err(());
        };
        let n = self.builtin_nttp_value(args[2], t, at(2))?;
        // [intseq.intseq]/1: T shall be an integer type.
        let canon = self.ast.types.canonical(t);
        if !self.ast.types.is_integral(canon) {
            self.error(DiagId::sema_integer_seq_integral_element_type, at(1));
            return Err(());
        }
        // [intseq.make]/1: N shall not be negative.
        if n < 0 {
            self.error(DiagId::sema_integer_seq_negative_length, at(2));
            return Err(());
        }
        if n > MAX_SEQ_LEN {
            let msg = self
                .diag(DiagId::sema_integer_seq_too_long, at(2))
                .arg(n.to_string())
                .arg(MAX_SEQ_LEN.to_string());
            self.emit(msg);
            return Err(());
        }
        let mut synth = Vec::with_capacity(usize::try_from(n).unwrap_or(0) + 1);
        synth.push(TemplateArg::ty(t));
        for i in 0..n {
            let v = self.int_const(canon, i);
            synth.push(TemplateArg::value(canon, v));
        }
        match self.act_template_id(seq, &synth, loc, loc)? {
            TemplateIdRef::Type(q) => Ok(q),
            TemplateIdRef::Value(_) | TemplateIdRef::Overload(_) => {
                self.error(DiagId::sema_template_arg_must_be_template, at(0));
                Err(())
            }
        }
    }

    /// `__type_pack_element<I, Ts...>`: `Ts...[I]` (shares §7.4's
    /// bounds rule).
    fn type_pack_element(
        &mut self,
        d: DeclId,
        args: &[TemplateArg],
        at: &dyn Fn(usize) -> Loc,
        loc: Loc,
    ) -> Result<QualType, ()> {
        if args.is_empty() {
            self.builtin_template_arity(d, false, loc);
            return Err(());
        }
        let size_t = self.size_type();
        let i = self.builtin_nttp_value(args[0], size_t, at(0))?;
        let mut types = Vec::with_capacity(args.len() - 1);
        for (k, &a) in args.iter().enumerate().skip(1) {
            let ArgView::Type(t) = a.view() else {
                self.error(DiagId::sema_template_arg_must_be_type, at(k));
                return Err(());
            };
            types.push(t);
        }
        if let Some(&t) = usize::try_from(i).ok().and_then(|i| types.get(i)) {
            return Ok(t);
        }
        self.error(DiagId::sema_pack_index_out_of_bounds, at(0));
        Err(())
    }

    /// The value of non-type argument `a` converted to `ty` as a
    /// converted constant expression ([temp.arg.nontype]/2).
    fn builtin_nttp_value(&mut self, a: TemplateArg, ty: QualType, at: Loc) -> Result<i128, ()> {
        let e = match a.view() {
            ArgView::Value { value, .. } => return self.const_int(value).ok_or(()),
            ArgView::Expr { expr, .. } => expr,
            _ => {
                self.error(DiagId::sema_template_arg_must_be_expr, at);
                return Err(());
            }
        };
        if self.is_err_expr(e) || self.is_error_type(ty) {
            return Err(());
        }
        let pty = conv::strip_cv(self, ty);
        let ety = self.ast.exprs.get(e).ty();
        let c = conv::lvalue_to_rvalue(self, e);
        let cty = conv::strip_cv(self, self.ast.exprs.get(c).ty());
        let t = &self.ast.types;
        let int_like = |q| t.is_integral(q) || t.as_enum(q).is_some();
        if (int_like(pty) && t.is_floating(cty)) || (t.is_floating(pty) && int_like(cty)) {
            let msg = self
                .diag(DiagId::sema_cce_conversion_not_allowed, at)
                .arg(self.ty_arg(ety))
                .arg(self.ty_arg(pty));
            self.emit(msg);
            return Err(());
        }
        if t.is_integral(pty)
            && int_like(cty)
            && let Some(raw) = self.fold_int(c)
            && !self.int_fits(pty, raw)
        {
            let msg = self
                .diag(DiagId::sema_cce_narrowing_value, at)
                .arg("non-type template argument")
                .arg(raw.to_string())
                .arg(self.ty_arg(pty));
            self.emit(msg);
            return Err(());
        }
        let c = match conv::implicit_convert(self, c, pty, ConvCtx::Initialization) {
            Ok(c) => c,
            Err(err) => {
                self.emit_conv_error(err, at);
                return Err(());
            }
        };
        let c = self.act_on_full_expr(c);
        match self.eval_const(c, EvalContext::converted(at)) {
            Ok(v) => self.const_int(v).ok_or(()),
            Err(f) => {
                let msg =
                    self.diag(DiagId::sema_expr_not_cce, at).arg("non-type template argument");
                let msg = Self::with_failure_notes(msg, f);
                self.emit(msg);
                Err(())
            }
        }
    }
}
