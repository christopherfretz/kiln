//! The builtin templates `__make_integer_seq`, `__type_pack_element` and
//! `__builtin_common_type` (spec/sema-templates.md §9.3). Each is an
//! implicit `BuiltinTemplate`
//! declarations entered in the translation unit's lookup table (not its
//! member list) when `Sema` is created; a template-id naming one is
//! checked here instead of through a template parameter list.

use kiln_ast::Ast;
use kiln_ast::expr::ValueCategory;
use kiln_ast::ids::TemplateNameId;
use kiln_ast::names::DeclName;
use kiln_ast::types::{ArgView, Quals, TemplateArg, UnaryTransformKind};
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_oven::EvalContext;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::{self, ConvCtx};
use crate::templ::actions::TemplateIdRef;
use crate::templ::stack::FrameKind;

/// The inline payload of `__make_integer_seq`'s declaration.
pub(crate) const MAKE_INTEGER_SEQ: u32 = 0;
/// The inline payload of `__type_pack_element`'s declaration.
pub(crate) const TYPE_PACK_ELEMENT: u32 = 1;
/// The inline payload of `__builtin_common_type`'s declaration.
pub(crate) const COMMON_TYPE: u32 = 2;

/// The longest sequence `__make_integer_seq` builds (kiln's limit; the
/// arguments are built eagerly, one per element).
pub(crate) const MAX_SEQ_LEN: i128 = 1 << 20;

/// Declares the builtin templates in `ast`'s translation unit.
pub(crate) fn declare_builtin_templates(ast: &mut Ast) {
    let tu = ast.tu;
    for (name, which) in [
        ("__make_integer_seq", MAKE_INTEGER_SEQ),
        ("__type_pack_element", TYPE_PACK_ELEMENT),
        ("__builtin_common_type", COMMON_TYPE),
    ] {
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
            TYPE_PACK_ELEMENT => self.type_pack_element(d, args, &at, loc)?,
            COMMON_TYPE => self.builtin_common_type(d, args, &at, loc)?,
            w => unreachable!(
                "builtin template payload {w}: declare_builtin_templates declares only the three"
            ),
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

    /// `__builtin_common_type<Base, HasType, NoType, Ts...>` (clang 20
    /// `builtinCommonTypeImpl`; libc++ 20+ derives `std::common_type`
    /// from it): `HasType<C>` when `Ts...` have a common type `C` by
    /// [meta.trans.other]/3 (Note A), user specializations of the
    /// two-type case reached through `Base<T1, T2>`, else `NoType`.
    fn builtin_common_type(
        &mut self,
        d: DeclId,
        args: &[TemplateArg],
        at: &dyn Fn(usize) -> Loc,
        loc: Loc,
    ) -> Result<QualType, ()> {
        if args.len() < 3 {
            self.builtin_template_arity(d, false, loc);
            return Err(());
        }
        let (ArgView::Template(base), ArgView::Template(has)) = (args[0].view(), args[1].view())
        else {
            let k = usize::from(matches!(args[0].view(), ArgView::Template(_)));
            self.error(DiagId::sema_template_arg_must_be_template, at(k));
            return Err(());
        };
        let mut types = Vec::with_capacity(args.len() - 2);
        for (k, &a) in args.iter().enumerate().skip(2) {
            let ArgView::Type(t) = a.view() else {
                self.error(DiagId::sema_template_arg_must_be_type, at(k));
                return Err(());
            };
            types.push(t);
        }
        let (none, ts) = (types[0], &types[1..]);
        let Some(c) = self.common_type_of(base, ts, loc) else { return Ok(none) };
        match self.act_template_id(has, &[TemplateArg::ty(c)], loc, loc)? {
            TemplateIdRef::Type(q) => Ok(q),
            TemplateIdRef::Value(_) | TemplateIdRef::Overload(_) => {
                self.error(DiagId::sema_template_arg_must_be_template, at(1));
                Err(())
            }
        }
    }

    /// The common type of `ts` ([meta.trans.other]/3, Note A): none for
    /// no type; `common_type_t<T0, T0>` for one; the two-type rule for
    /// two; else `common_type_t<common_type_t<T1, T2>, R...>`.
    fn common_type_of(
        &mut self,
        base: TemplateNameId,
        ts: &[QualType],
        loc: Loc,
    ) -> Option<QualType> {
        match *ts {
            [] => None,
            [t] => self.lookup_common_type(base, t, t, loc),
            [t1, t2] => self.common_type_2(base, t1, t2, loc),
            [t1, ref rest @ ..] => {
                let mut c = t1;
                for &t in rest {
                    c = self.lookup_common_type(base, c, t, loc)?;
                }
                Some(c)
            }
        }
    }

    /// `common_type_t<T1, T2>` through `Base<T1, T2>`, which finds a user
    /// specialization ([meta.trans.other]/4), as a substitution in the
    /// translation unit that fails quietly. Two builtin types cannot have
    /// one, so their common type is computed directly (clang).
    fn lookup_common_type(
        &mut self,
        base: TemplateNameId,
        t1: QualType,
        t2: QualType,
        loc: Loc,
    ) -> Option<QualType> {
        if self.ast.types.builtin(t1).is_some() && self.ast.types.builtin(t2).is_some() {
            return self.common_type_2(base, t1, t2, loc);
        }
        let args = [TemplateArg::ty(t1), TemplateArg::ty(t2)];
        let tu = self.ast.tu;
        let saved = std::mem::replace(&mut self.ctx, tu);
        let r = self.in_sfinae(FrameKind::ConstraintSubstitution, tu, None, loc, |s| {
            s.enter_unevaluated();
            let r = s.act_template_id(base, &args, loc, loc);
            s.exit_unevaluated();
            match r {
                Ok(TemplateIdRef::Type(q)) if !s.is_error_type(q) => Some(q),
                _ => None,
            }
        });
        self.ctx = saved;
        match r {
            Some((Some(q), Ok(()))) => Some(q),
            _ => None,
        }
    }

    /// [meta.trans.other]/3.3: with `D1`, `D2` the decayed `T1`, `T2`,
    /// `common_type_t<D1, D2>` unless both are already decayed; then
    /// `decay_t<decltype(false ? declval<D1>() : declval<D2>())>`, and
    /// from C++20 `decay_t<COND-RES(CREF(D1), CREF(D2))>`.
    fn common_type_2(
        &mut self,
        base: TemplateNameId,
        t1: QualType,
        t2: QualType,
        loc: Loc,
    ) -> Option<QualType> {
        let d1 = self.apply_unary_transform(UnaryTransformKind::Decay, t1, loc)?;
        let d2 = self.apply_unary_transform(UnaryTransformKind::Decay, t2, loc)?;
        if !self.ast.types.same_type(t1, d1) || !self.ast.types.same_type(t2, d2) {
            return self.lookup_common_type(base, d1, d2, loc);
        }
        if let Some(c) = self.conditional_common_type(d1, d2, false, loc) {
            return Some(c);
        }
        if self.std < kiln_lex::Std::Cxx20 {
            return None;
        }
        self.conditional_common_type(d1, d2, true, loc)
    }

    /// The decayed type of `b ? x : y` on a `bool` and operands of types
    /// `d1` and `d2`: prvalues, or `const` lvalues for `cref`, built as
    /// an unevaluated operand in the translation unit that fails
    /// quietly.
    fn conditional_common_type(
        &mut self,
        d1: QualType,
        d2: QualType,
        cref: bool,
        loc: Loc,
    ) -> Option<QualType> {
        let (q1, q2, vc) = if cref {
            let c = |s: &mut Self, q| s.ast.types.qualify(q, Quals::CONST);
            (c(self, d1), c(self, d2), ValueCategory::LValue)
        } else {
            (d1, d2, ValueCategory::PRValue)
        };
        let tu = self.ast.tu;
        let saved = std::mem::replace(&mut self.ctx, tu);
        let r = self.in_sfinae(FrameKind::ConstraintSubstitution, tu, None, loc, |s| {
            s.enter_unevaluated();
            let c = s.builder().opaque_value_of(QualType::BOOL, ValueCategory::PRValue, loc);
            let x = s.builder().opaque_value_of(q1, vc, loc);
            let y = s.builder().opaque_value_of(q2, vc, loc);
            let e = s.act_on_conditional(c, x, y, loc);
            s.exit_unevaluated();
            (!s.is_err_expr(e)).then(|| s.ety(e))
        });
        self.ctx = saved;
        match r {
            Some((Some(q), Ok(()))) => {
                self.apply_unary_transform(UnaryTransformKind::Decay, q, loc)
            }
            _ => None,
        }
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
            // A `-Wc++11-narrowing` error by default; clang goes on with
            // the converted value (`__type_pack_element<-1, int>` then
            // also reports the index out of bounds).
            self.emit(msg);
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
