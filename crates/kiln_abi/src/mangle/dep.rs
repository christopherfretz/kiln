//! Dependent signature types and template-argument expressions
//! (spec/abi.md §8.15): what a function template's signature needs when
//! its return or parameter types name members of dependent types
//! (`typename rr<T>::type`), dependent template specializations and
//! value-dependent template arguments (`en<is_i<T>::value, int>`).
//! Follows clang's `CXXNameMangler` (`manglePrefix(NestedNameSpecifier)`,
//! `mangleTemplatePrefix(TemplateName)`, `mangleUnresolvedPrefix`,
//! `mangleExpression`).

use super::{Mangler, Structor, Sub};
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, ExprDeps, ExprKind, TraitKind, UnaryOp};
use kiln_ast::ids::{
    ArgListId, DeclId, ExprId, NnsId, QualType, TemplateNameId, TemplateParamListId,
};
use kiln_ast::names::DeclNameView;
use kiln_ast::types::{ArgView, ExprOperand, Nns, TagKeyword, TemplateName, TypeView};
use kiln_source::symbol::Symbol;

/// clang `UnknownArity`: an operator name outside a call.
pub(super) const UNKNOWN_ARITY: usize = usize::MAX;

impl Mangler<'_, '_> {
    /// A dependent `TemplateSpecialization` type (clang `mangleType(const
    /// TemplateSpecializationType *)`): the unscoped template name at
    /// namespace scope or directly in `std`, else a nested name; a
    /// template template parameter is `T_` (a template substitution).
    pub(super) fn dependent_tst(&mut self, name: TemplateNameId, args: ArgListId) {
        let ast = self.ast;
        let n = ast.types.canonical_template_name(name);
        match ast.types.template_name_view(n) {
            TemplateName::Template(t) => {
                let parent = self.parent(t);
                let top =
                    parent.is_none_or(|p| ast.decls.get(p).kind() == DeclKind::TranslationUnit);
                let std = parent.is_some_and(|p| self.is_std(p));
                if top || std {
                    self.unscoped_template(t, std);
                    self.converted_args(t, args);
                } else {
                    self.out.push('N');
                    self.template_prefix(t, None, Structor::Complete);
                    self.converted_args(t, args);
                    self.out.push('E');
                }
            }
            TemplateName::TemplateTemplateParm { depth, index, .. }
            | TemplateName::CanonTemplateTemplateParm { depth, index, .. } => {
                self.template_template_parm(depth, index);
                self.grouped_args(self.ttp_head(depth, index), args);
            }
            TemplateName::DependentIdentifier { nns, name } => {
                self.dependent_template_spec(nns, name, args);
            }
            _ => self.out.push_str("u7unknown"),
        }
    }

    /// A `DependentName` type: `[Ts|Tu|Te] N <prefix> <source-name> E`.
    pub(super) fn dependent_name(&mut self, keyword: TagKeyword, nns: NnsId, name: Symbol) {
        match keyword {
            TagKeyword::Struct | TagKeyword::Class | TagKeyword::Interface => {
                self.out.push_str("Ts");
            }
            TagKeyword::Union => self.out.push_str("Tu"),
            TagKeyword::Enum => self.out.push_str("Te"),
            _ => {}
        }
        self.out.push('N');
        self.nns_prefix(nns);
        self.symbol(name);
        self.out.push('E');
    }

    /// A `DependentTemplateSpecialization` type:
    /// `N <dependent template prefix> <template-args> E`.
    pub(super) fn dependent_template_spec(&mut self, nns: NnsId, name: Symbol, args: ArgListId) {
        self.out.push('N');
        self.dependent_template_prefix(nns, name);
        self.template_args(args);
        self.out.push('E');
    }

    /// `<prefix> <source-name>` of the dependent template name
    /// `nns::template name`, a substitution candidate (clang ≥ 12).
    fn dependent_template_prefix(&mut self, nns: NnsId, name: Symbol) {
        let key = Sub::DepTemplate(self.ast.types.canonical_nns(nns), name);
        if self.try_sub(key) {
            return;
        }
        self.nns_prefix(nns);
        self.symbol(name);
        self.subs.push(key);
    }

    /// <prefix> for a nested-name-specifier (clang
    /// `manglePrefix(NestedNameSpecifier *)`).
    fn nns_prefix(&mut self, nns: NnsId) {
        let ast = self.ast;
        let nns = ast.types.canonical_nns(nns);
        match ast.types.nns_view(nns) {
            Nns::Global => {}
            Nns::Namespace { ns, .. } | Nns::NamespaceAlias { ns, .. } => self.prefix(ns, None),
            Nns::Type { ty, .. } => self.type_prefix(ty),
            Nns::Identifier { prefix, name } => {
                // Substitutable since clang 15.
                let key = Sub::Nns(nns);
                if self.try_sub(key) {
                    return;
                }
                if let Some(p) = prefix {
                    self.nns_prefix(p);
                }
                self.symbol(name);
                self.subs.push(key);
            }
            Nns::Splice { .. } => self.out.push_str("u7unknown"),
        }
    }

    /// clang `manglePrefix(QualType)`: a template specialization is its
    /// template prefix and arguments (the specialization type being the
    /// candidate), anything else the type itself.
    fn type_prefix(&mut self, ty: QualType) {
        let ast = self.ast;
        let q = ast.types.canonical(ty);
        // The creating spelling's expressions, as in `Mangler::compound`.
        let origin = ast.types.arg_origin(q.ty());
        match ast.types.canonical_view(q) {
            TypeView::TemplateSpecialization { name, args, .. } => {
                let args = origin.unwrap_or(args);
                let key = Sub::Type(q);
                if self.try_sub(key) {
                    return;
                }
                let n = ast.types.canonical_template_name(name);
                match ast.types.template_name_view(n) {
                    TemplateName::Template(t) => {
                        self.template_prefix(t, None, Structor::Complete);
                        self.converted_args(t, args);
                        self.subs.push(key);
                        return;
                    }
                    TemplateName::TemplateTemplateParm { depth, index, .. }
                    | TemplateName::CanonTemplateTemplateParm { depth, index, .. } => {
                        self.template_template_parm(depth, index);
                        self.grouped_args(self.ttp_head(depth, index), args);
                        self.subs.push(key);
                        return;
                    }
                    TemplateName::DependentIdentifier { nns, name } => {
                        self.dependent_template_prefix(nns, name);
                    }
                    _ => self.out.push_str("u7unknown"),
                }
                self.template_args(args);
                self.subs.push(key);
            }
            TypeView::DependentTemplateSpecialization { nns, name, args, .. } => {
                let args = origin.unwrap_or(args);
                let key = Sub::Type(q);
                if self.try_sub(key) {
                    return;
                }
                self.dependent_template_prefix(nns, name);
                self.template_args(args);
                self.subs.push(key);
            }
            _ => self.ty(q),
        }
    }

    /// The arguments of a dependent specialization of `t` as clang's
    /// converted (canonical) list has them: those for a trailing
    /// parameter pack are grouped into one `J ... E` argument pack.
    fn converted_args(&mut self, t: DeclId, args: ArgListId) {
        let params = self.ast.template_info(self.ast.first(t)).map(|i| i.params);
        self.grouped_args(params, args);
    }

    /// [`Self::converted_args`] against the parameter list `params`.
    fn grouped_args(&mut self, params: Option<TemplateParamListId>, args: ArgListId) {
        let ast = self.ast;
        let list: Vec<_> = ast.types.args(args).to_vec();
        let pack_at = params.and_then(|pl| {
            let params = ast.templates.param_list(pl).params;
            ast.lists
                .iter::<DeclId>(params)
                .position(|p| ast.template_parm(p).is_some_and(|x| x.pack))
        });
        let Some(k) = pack_at.filter(|&k| {
            k <= list.len() && !list.get(k).is_some_and(|a| matches!(a.view(), ArgView::Pack(_)))
        }) else {
            self.template_args(args);
            return;
        };
        self.out.push('I');
        for a in &list[..k] {
            self.template_arg(a.view());
        }
        self.out.push('J');
        for a in &list[k..] {
            self.template_arg(a.view());
        }
        self.out.push_str("EE");
    }

    /// A template template parameter as a template prefix: `T_`, a
    /// substitution candidate.
    fn template_template_parm(&mut self, depth: u32, index: u32) {
        let key = Sub::TemplateParm(depth, index);
        if !self.try_sub(key) {
            self.template_param_at(depth, index);
            self.subs.push(key);
        }
    }

    fn symbol(&mut self, s: Symbol) {
        let name = self.ast.symbols().as_str(s);
        self.source_name(name);
    }

    /// A dependent `decltype(e)`: `Dt <expression> E` for an
    /// id-expression or member access, else `DT <expression> E`.
    pub(super) fn dependent_decltype(&mut self, e: Option<ExprId>) {
        let (len, subs) = (self.out.len(), self.subs.len());
        if let Some(e) = e {
            let id = matches!(
                self.ast.exprs.get(e).kind(),
                ExprKind::DeclRefExpr
                    | ExprKind::MemberExpr
                    | ExprKind::UnresolvedLookupExpr
                    | ExprKind::DependentScopeDeclRefExpr
                    | ExprKind::CXXDependentScopeMemberExpr
                    | ExprKind::UnresolvedMemberExpr
            );
            self.out.push_str(if id { "Dt" } else { "DT" });
            let saved_failed = self.failed.take();
            if self.expression(e) {
                self.out.push('E');
                self.failed = saved_failed;
                return;
            }
            // As in a template argument: a hard error, never a shared
            // placeholder name (WP-593).
            let at = self.failed.unwrap_or(e);
            self.anon.unmangleable.push(at);
            self.failed = saved_failed;
        }
        self.out.truncate(len);
        self.subs.truncate(subs);
        self.out.push_str("u7unknown");
    }

    /// The `A <expression>` head of a dependent-bound array type; an
    /// unencodable bound is a hard error (the vendor placeholder keeps the
    /// name well formed).
    pub(super) fn dependent_array_bound(&mut self, size: ExprOperand) {
        let (len, subs) = (self.out.len(), self.subs.len());
        let e = match size {
            ExprOperand::Expr(e) => Some(e),
            ExprOperand::Profile(p) => self.ast.types.profile_expr(p),
        };
        self.out.push('A');
        if let Some(e) = e {
            let saved_failed = self.failed.take();
            if self.expression(e) {
                self.failed = saved_failed;
                return;
            }
            let at = self.failed.unwrap_or(e);
            self.anon.unmangleable.push(at);
            self.failed = saved_failed;
        }
        self.out.truncate(len);
        self.subs.truncate(subs);
        self.out.push_str("Au7unknown");
    }

    /// `DB <expression> _` (`_BitInt(N)`) or `DU <expression> _`
    /// (`unsigned _BitInt(N)`) of a dependent width. A width expression
    /// the mangler cannot spell falls back to `u7unknown` in its place,
    /// keeping the signed and unsigned forms apart.
    pub(super) fn dependent_bitint(&mut self, signed: bool, width: Option<ExprId>) {
        let (len, subs) = (self.out.len(), self.subs.len());
        let tag = if signed { "DB" } else { "DU" };
        self.out.push_str(tag);
        if !width.is_some_and(|e| self.expression(e)) {
            self.out.truncate(len);
            self.subs.truncate(subs);
            self.out.push_str(tag);
            self.out.push_str("u7unknown");
        }
        self.out.push('_');
    }

    // ---- template-argument expressions -----------------------------------

    /// A value-dependent template argument (clang `mangleExpression` with
    /// `AsTemplateArg`): a primary expression as is, anything else
    /// wrapped in `X ... E`. An expression with no encoding is a hard
    /// error at its innermost unsupported subexpression (clang's "cannot
    /// yet mangle expression type", WP-593): two overloads differing only
    /// in such an expression would otherwise share one name. The
    /// placeholder `XLi0EE` keeps the name well formed (spec/abi.md
    /// §8.15).
    pub(super) fn template_arg_expr(&mut self, e: Option<ExprId>) {
        let (len, subs) = (self.out.len(), self.subs.len());
        if let Some(e) = e {
            if self.substituted_value(e) {
                return;
            }
            let saved_failed = self.failed.take();
            let e = self.skip_transparent(e);
            let primary = self.is_primary(e);
            if !primary {
                self.out.push('X');
            }
            if self.expression(e) {
                if !primary {
                    self.out.push('E');
                }
                self.failed = saved_failed;
                return;
            }
            let at = self.failed.unwrap_or(e);
            self.anon.unmangleable.push(at);
            self.failed = saved_failed;
        }
        self.out.truncate(len);
        self.subs.truncate(subs);
        self.out.push_str("XLi0EE");
    }

    /// A substituted non-type template parameter whose replacement is an
    /// integral or enumeration constant mangles as that template argument,
    /// `L <parameter type> <value> E` (clang `mangleExpression`,
    /// `SubstNonTypeTemplateParmExprClass`): `LS2_2E`, not the cast
    /// `XcvS2_Lj2EE` the replacement spells. Returns false, writing
    /// nothing, for any other expression.
    fn substituted_value(&mut self, mut e: ExprId) -> bool {
        let ast = self.ast;
        let ex = &ast.exprs;
        loop {
            if ast.templates.pack_index_origin.contains(e) {
                return false;
            }
            e = match ex.get(e).kind() {
                ExprKind::ParenExpr | ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr => {
                    ex.operand(e)
                }
                ExprKind::SubstNonTypeTemplateParmExpr => break,
                _ => return false,
            };
        }
        let ty = ex.get(e).ty();
        if ast.types.as_enum(ty).is_none() {
            return false;
        }
        let mut r = ex.subst_replacement(e);
        let value = loop {
            r = match ex.get(r).kind() {
                ExprKind::ParenExpr
                | ExprKind::ImplicitCastExpr
                | ExprKind::ConstantExpr
                | ExprKind::CStyleCastExpr
                | ExprKind::CXXStaticCastExpr => ex.operand(r),
                ExprKind::IntegerLiteral => match ex.int_value(r) {
                    Some(v) if i64::try_from(v).is_ok() => break v,
                    _ => return false,
                },
                _ => return false,
            };
        };
        self.out.push('L');
        self.ty(ty);
        self.out.push_str(&value.to_string());
        self.out.push('E');
        true
    }

    /// The type of the innermost implicit conversion among the
    /// transparent nodes over `e` (clang `ImplicitlyConvertedToType`).
    fn implicitly_converted_to(&self, mut e: ExprId) -> Option<QualType> {
        let ex = &self.ast.exprs;
        let mut ty = None;
        loop {
            e = match ex.get(e).kind() {
                ExprKind::ImplicitCastExpr => {
                    ty = Some(ex.get(e).ty());
                    ex.operand(e)
                }
                ExprKind::ParenExpr | ExprKind::ConstantExpr => ex.operand(e),
                ExprKind::SubstNonTypeTemplateParmExpr => ex.subst_replacement(e),
                _ => return ty,
            };
        }
    }

    /// The integer literal a non-dependent `sizeof`/`alignof` stands for
    /// (clang `mangleExpression`, `UnaryExprOrTypeTraitExprClass`).
    fn trait_value(&mut self, e: ExprId, converted: Option<QualType>) -> bool {
        let ast = self.ast;
        let operand = match ast.exprs.trait_type_arg(e) {
            Some(t) => t,
            None => match ast.exprs.first_child(e, &ast.lists) {
                Some(sub) if ast.exprs.trait_kind(e) == TraitKind::SizeOf => {
                    ast.exprs.get(sub).ty()
                }
                _ => return false,
            },
        };
        // `sizeof` and `alignof` of a reference are those of the referee.
        let c = ast.types.canonical(operand);
        let operand = match ast.types.view(c.ty()) {
            TypeView::LValueRef(r) | TypeView::RValueRef(r) => r,
            _ => operand,
        };
        let layout = crate::layout::type_size_align(
            ast,
            operand,
            &crate::Target::for_types(ast.types.target()),
        );
        let value = match ast.exprs.trait_kind(e) {
            TraitKind::SizeOf => layout.size,
            TraitKind::AlignOf => layout.align,
            _ => return false,
        };
        let ty = converted
            .filter(|&t| ast.types.is_integral(t))
            .unwrap_or_else(|| ast.exprs.get(e).ty());
        self.out.push('L');
        self.ty(ty);
        self.out.push_str(&value.to_string());
        self.out.push('E');
        true
    }

    /// Parentheses, implicit casts, `ConstantExpr`, temporaries (clang
    /// `MaterializeTemporaryExpr`, `CXXBindTemporaryExpr`,
    /// `ExprWithCleanups`) and substituted parameters do not appear in
    /// the mangling.
    fn skip_transparent(&self, mut e: ExprId) -> ExprId {
        let ex = &self.ast.exprs;
        loop {
            // A substituted pack index stops here: it writes nothing.
            if self.ast.templates.pack_index_origin.contains(e) {
                return e;
            }
            e = match ex.get(e).kind() {
                ExprKind::ParenExpr
                | ExprKind::ImplicitCastExpr
                | ExprKind::ConstantExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ExprWithCleanups => ex.operand(e),
                ExprKind::SubstNonTypeTemplateParmExpr => ex.subst_replacement(e),
                _ => return e,
            };
        }
    }

    fn is_primary(&self, e: ExprId) -> bool {
        let ast = self.ast;
        if ast.templates.pack_index_origin.contains(e) {
            return true;
        }
        match ast.exprs.get(e).kind() {
            // A substituted `sizeof...` is `sZ`, not a literal.
            ExprKind::IntegerLiteral => !ast.templates.size_of_pack_origin.contains(e),
            ExprKind::CXXBoolLiteralExpr
            | ExprKind::CharacterLiteral
            | ExprKind::FloatingLiteral
            | ExprKind::ImaginaryLiteral
            | ExprKind::StringLiteral
            | ExprKind::CXXNullPtrLiteralExpr
            | ExprKind::GNUNullExpr
            // A reflection is a literal (`L <info-type> <reflection> E`).
            | ExprKind::CXXReflectExpr
            // A pack index has no encoding and no `X…E` wrapper.
            | ExprKind::PackIndexingExpr => true,
            // An enumerator is its value's literal.
            ExprKind::DeclRefExpr => matches!(
                ast.decls.get(ast.exprs.decl(e)).kind(),
                DeclKind::EnumConstant
                    | DeclKind::Var
                    | DeclKind::VarTemplateSpecialization
                    | DeclKind::Function
                    | DeclKind::CXXMethod
                    | DeclKind::TemplateParamObject
            ),
            // A template parameter reference is wrapped (`XT_E`).
            _ => false,
        }
    }

    /// A `DeclRefExpr` operand: a template or function parameter, an
    /// enumerator's value or a builtin function's name.
    fn decl_ref_expression(&mut self, e: ExprId) -> bool {
        let ast = self.ast;
        let d = ast.exprs.decl(e);
        match ast.decls.get(d).kind() {
            DeclKind::NonTypeTemplateParm => {
                let (depth, idx) = ast
                    .template_parm(d)
                    .map_or((0, 0), |p| (u32::from(p.depth), u32::from(p.index)));
                self.template_param_at(depth, idx);
            }
            DeclKind::ParmVar => return self.function_param_ref(d),
            // `L <enum-type> <value> E` (clang `mangleDeclRefExpr`).
            DeclKind::EnumConstant => {
                let Some(v) = ast.decls.enumerator(d).and_then(|x| x.value) else {
                    return false;
                };
                let Some(ty) = ast.decls.get(d).ty() else { return false };
                self.value_arg(ty, v);
            }
            // A builtin function called in a dependent call (WP-726,
            // clang `mangleDeclRefExpr`): `L_Z <source-name> E`.
            DeclKind::Function if ast.decls.builtin_id(d).is_some() => {
                let Some(name) = ast.decls.get(d).name().as_ident() else {
                    return false;
                };
                self.out.push_str("L_Z");
                self.symbol(name);
                self.out.push('E');
            }
            // Any other named entity, a variable, function, static data
            // member or non-static data member (`&R<int>::m`): `L
            // <mangled-name> E` (clang `mangleDeclRefExpr`'s external
            // name, its `default` case; the substitution table is shared).
            DeclKind::Field
            | DeclKind::Var
            | DeclKind::VarTemplateSpecialization
            | DeclKind::Function
            | DeclKind::CXXMethod
            | DeclKind::TemplateParamObject => {
                self.out.push('L');
                self.entity(d);
                self.out.push('E');
            }
            _ => return false,
        }
        true
    }

    /// One expression; `false` when it (or a subexpression) has no
    /// supported encoding.
    pub(super) fn expression(&mut self, e: ExprId) -> bool {
        self.expression_arity(e, UNKNOWN_ARITY)
    }

    /// One expression whose operator names, if any, are mangled for
    /// `arity` operands (clang `mangleExpression`'s `Arity`: a callee's
    /// argument count, [`UNKNOWN_ARITY`] elsewhere).
    fn expression_arity(&mut self, e: ExprId, arity: usize) -> bool {
        let ok = self.expression_node(e, arity);
        if !ok && self.failed.is_none() {
            self.failed = Some(self.skip_transparent(e));
        }
        ok
    }

    /// `L <type> <value> E`, or `sZ <param>` for a substituted
    /// `sizeof...` (a non-dependent `SizeOfPackExpr` in clang, which still
    /// names its pack).
    fn integer_literal(&mut self, e: ExprId) -> bool {
        let ast = self.ast;
        let pack = ast.templates.size_of_pack_origin.get(e);
        if let Some(p) = pack.and_then(|&d| ast.template_parm(d)) {
            self.out.push_str("sZ");
            self.template_param_at(u32::from(p.depth), u32::from(p.index));
            return true;
        }
        let ty = ast.exprs.get(e).ty();
        let Some(v) = ast.exprs.int_value(e) else {
            // A literal outside the 64-bit signed range
            // (`18446744073709551615ull`) keeps its value as a constant:
            // `L <type> <decimal> E` all the same (WP-980).
            let Some(c) = ast.exprs.literal_const(e) else { return false };
            self.value_in_arg(ty, c, false);
            return true;
        };
        self.out.push('L');
        self.ty(ty);
        self.out.push_str(&v.to_string());
        self.out.push('E');
        true
    }

    /// A GNU imaginary literal `2i` / `2.0i` (clang `mangleExpression`,
    /// `ImaginaryLiteralClass`, "mangle as if a complex literal"):
    /// `L <complex type> <real> _ <imag> E`. The real part is a zero of
    /// the element type: `0` for an integer, the hex encoding of `+0.0`
    /// for a floating type; the imaginary part is the literal's number
    /// (decimal) or encoding (hex). `t + 2i` is `plfp_LCi0_2E`.
    fn imaginary_literal(&mut self, e: ExprId) -> bool {
        let ast = self.ast;
        let sub = ast.exprs.operand(e);
        let ty = ast.exprs.get(e).ty();
        let mut imag = String::new();
        let mut zero_width = None;
        match ast.exprs.get(sub).kind() {
            ExprKind::FloatingLiteral => {
                let Some(c) = ast.exprs.literal_const(sub) else { return false };
                let kiln_ast::consts::ConstValue::Scalar(kiln_ast::consts::Scalar::Float {
                    kind,
                    bits,
                    ..
                }) = ast.consts.get(c)
                else {
                    return false;
                };
                zero_width = Some(kind.bits());
                let saved = std::mem::take(&mut self.out);
                self.float_hex(kind.bits(), bits);
                imag = std::mem::replace(&mut self.out, saved);
            }
            ExprKind::IntegerLiteral => {
                if let Some(v) = ast.exprs.int_value(sub) {
                    imag = v.to_string();
                } else {
                    let Some(c) = ast.exprs.literal_const(sub) else { return false };
                    let kiln_ast::consts::ConstValue::Scalar(kiln_ast::consts::Scalar::Int {
                        bits,
                        ..
                    }) = ast.consts.get(c)
                    else {
                        return false;
                    };
                    // A literal is never negative: its value as unsigned.
                    bits.write_decimal(false, &mut imag);
                }
            }
            _ => return false,
        }
        self.out.push('L');
        self.ty(ty);
        match zero_width {
            Some(w) => self.float_hex(w, 0),
            None => self.out.push('0'),
        }
        self.out.push('_');
        self.out.push_str(&imag);
        self.out.push('E');
        true
    }

    fn expression_node(&mut self, e: ExprId, arity: usize) -> bool {
        let ast = self.ast;
        if self.substituted_value(e) {
            return true;
        }
        let converted = self.implicitly_converted_to(e);
        let e = self.skip_transparent(e);
        // The element a substituted `pack...[i]` selected: clang keeps a
        // `PackIndexingExpr` and writes nothing for it (WP-1060).
        if ast.templates.pack_index_origin.contains(e) {
            return true;
        }
        let n = ast.exprs.get(e);
        match n.kind() {
            ExprKind::IntegerLiteral => return self.integer_literal(e),
            ExprKind::ImaginaryLiteral => return self.imaginary_literal(e),
            // P2662 `pack...[i]`: clang 19 and 22 write nothing for it, in
            // a `decltype` (`DTE`, not `Dt`), an operand (`DTplfp_E`) and
            // a template argument (`1SIE`, without `X…E`), dependent or
            // substituted. The Itanium ABI has no production for it yet;
            // kiln matches clang's names (spec/landed/abi/WP-1029.md).
            ExprKind::PackIndexingExpr => {}
            ExprKind::CharacterLiteral => {
                self.out.push('L');
                self.ty(n.ty());
                self.out.push_str(&ast.exprs.char_value(e).to_string());
                self.out.push('E');
            }
            ExprKind::CXXBoolLiteralExpr => {
                self.out.push_str(if ast.exprs.bool_value(e) { "Lb1E" } else { "Lb0E" });
            }
            ExprKind::CXXNullPtrLiteralExpr => self.out.push_str("LDnE"),
            // `L <type> <hex bits> E` (clang `mangleFloat`: the encoding,
            // most significant nibble first, zero-padded to its width).
            ExprKind::FloatingLiteral => {
                let Some(c) = ast.exprs.literal_const(e) else { return false };
                let kiln_ast::consts::ConstValue::Scalar(kiln_ast::consts::Scalar::Float {
                    kind,
                    bits,
                    ..
                }) = ast.consts.get(c)
                else {
                    return false;
                };
                self.float_literal(n.ty(), kind.bits(), bits);
            }
            // `L <array type> E`: a string literal names only its type.
            ExprKind::StringLiteral => {
                self.out.push('L');
                self.ty(n.ty());
                self.out.push('E');
            }
            ExprKind::CXXReflectExpr => return self.reflect_expr(e),
            // clang mangles `__null` as the integer literal 0 of its type.
            ExprKind::GNUNullExpr => {
                self.out.push('L');
                self.ty(n.ty());
                self.out.push_str("0E");
            }
            ExprKind::DeclRefExpr => return self.decl_ref_expression(e),
            ExprKind::ConceptSpecializationExpr => return self.concept_id_expr(e),
            ExprKind::CXXFoldExpr => return self.fold_expr(e),
            ExprKind::RequiresExpr => return self.requires_expr(e),
            ExprKind::DependentScopeDeclRefExpr => {
                let Some(nns) = NnsId::try_new(n.a()) else { return false };
                if !self.unresolved_prefix(nns, false) {
                    return false;
                }
                let args = ast.templates.expr_targs.get(e).copied();
                return self.base_unresolved_name(ast.dependent_scope_ref_name(e), args, arity);
            }
            ExprKind::UnresolvedLookupExpr => {
                let name = kiln_ast::names::DeclName::from_raw(n.a()).unwrap_or_default();
                if let Some(&nns) = ast.templates.expr_nns.get(e)
                    && !self.unresolved_prefix(nns, false)
                {
                    return false;
                }
                let args = ast.templates.expr_targs.get(e).copied();
                return self.base_unresolved_name(name, args, arity);
            }
            ExprKind::CXXThisExpr => self.out.push_str("fpT"),
            ExprKind::CXXDependentScopeMemberExpr | ExprKind::MemberExpr => {
                return self.member_expression(e, arity);
            }
            ExprKind::SizeOfPackExpr => {
                let Some(d) = DeclId::try_new(n.a()) else { return false };
                let Some(p) = ast.template_parm(d) else { return false };
                self.out.push_str("sZ");
                self.template_param_at(u32::from(p.depth), u32::from(p.index));
            }
            // A vendor trait: `u <source-name> <type>* E`.
            ExprKind::TypeTraitExpr => {
                self.out.push('u');
                self.source_name(ast.exprs.type_trait(e).spelling());
                let tys: Vec<QualType> =
                    ast.lists.iter::<QualType>(ast.exprs.type_trait_args(e)).collect();
                for q in tys {
                    self.ty(q);
                }
                self.out.push('E');
            }
            _ => return self.operator_expression(e, converted),
        }
        true
    }

    /// clang `CallExprClass`: `cl <callee> <arg>* E`; the callee's
    /// operator name takes the argument count as its arity (unknown with
    /// a pack expansion).
    fn call_expression(&mut self, expr: ExprId) -> bool {
        let ast = self.ast;
        self.out.push_str("cl");
        let args: Vec<ExprId> = ast.lists.iter::<ExprId>(ast.exprs.call_args(expr)).collect();
        let packs = args.iter().any(|&a| ast.exprs.get(a).kind() == ExprKind::PackExpansionExpr);
        let arity = if packs { UNKNOWN_ARITY } else { args.len() };
        if !self.expression_arity(ast.exprs.callee(expr), arity) {
            return false;
        }
        for arg in args {
            if !self.expression(arg) {
                return false;
            }
        }
        self.out.push('E');
        true
    }

    /// `sizeof`/`alignof` of a type (`st`/`at`) or an expression
    /// (`sz`/`az`). `converted` is the type `expr` is implicitly
    /// converted to.
    fn trait_expression(&mut self, expr: ExprId, converted: Option<QualType>) -> bool {
        let ast = self.ast;
        let node = ast.exprs.get(expr);
        // A non-instantiation-dependent operand is encoded as the
        // integer result, typed as the integer type it is
        // implicitly converted to, else as its own `size_t`
        // ([abi] 5.1.6.1; `Lm4E` for `sizeof(wchar_t) == 2`).
        if !node.deps().intersects(ExprDeps::VALUE_INST) {
            return self.trait_value(expr, converted);
        }
        let (type_code, expr_code) = match ast.exprs.trait_kind(expr) {
            TraitKind::SizeOf => ("st", "sz"),
            TraitKind::AlignOf => ("at", "az"),
            _ => return false,
        };
        if let Some(arg) = ast.exprs.trait_type_arg(expr) {
            self.out.push_str(type_code);
            self.ty(arg);
        } else {
            let Some(sub) = ast.exprs.first_child(expr, &ast.lists) else {
                return false;
            };
            self.out.push_str(expr_code);
            return self.expression(sub);
        }
        true
    }

    /// `noexcept(e)` (`nx <expression>`), `throw e` / `throw`
    /// (`tw <expression>` / `tr`) and `typeid(type)` / `typeid(expr)`
    /// (`ti <type>` / `te <expression>`).
    fn keyword_expression(&mut self, expr: ExprId) -> bool {
        let ast = self.ast;
        let node = ast.exprs.get(expr);
        if node.kind() == ExprKind::CXXNoexceptExpr {
            let Some(sub) = ExprId::try_new(node.a()) else { return false };
            self.out.push_str("nx");
            return self.expression(sub);
        }
        if node.kind() == ExprKind::CXXThrowExpr {
            let Some(sub) = ast.exprs.throw_operand(expr) else {
                self.out.push_str("tr");
                return true;
            };
            self.out.push_str("tw");
            return self.expression(sub);
        }
        match ast.exprs.typeid_operand(expr) {
            kiln_ast::expr::TypeidOperand::Type(t) => {
                self.out.push_str("ti");
                self.ty(t);
                true
            }
            kiln_ast::expr::TypeidOperand::Expr(sub) => {
                self.out.push_str("te");
                self.expression(sub)
            }
        }
    }

    /// An expression splice `[: r :]` (`template [: r :]<args>`): GCC 16's
    /// `DS <expression> [<template-args>] E` (P2996 has no Itanium
    /// encoding yet). A member splice `s.[: r :]` / `p->[: r :]` is the
    /// member access `dt` / `pt <expression>` with the splice as its
    /// name (GCC 16: `dtfp_DST_E`).
    fn splice_expression(&mut self, expr: ExprId) -> bool {
        let ast = self.ast;
        if let Some((base, arrow)) = ast.exprs.splice_base(expr) {
            self.out.push_str(if arrow { "pt" } else { "dt" });
            if !self.expression(base) {
                return false;
            }
        }
        self.out.push_str("DS");
        if !self.expression(ast.exprs.splice_operand(expr)) {
            return false;
        }
        if let Some(args) = ast.exprs.splice_template_args(expr) {
            self.template_args(args);
        }
        self.out.push('E');
        true
    }

    /// An operator, call, cast, `sizeof`/`alignof` or pack expansion.
    /// `converted` is the type `expr` is implicitly converted to.
    fn operator_expression(&mut self, expr: ExprId, converted: Option<QualType>) -> bool {
        let ast = self.ast;
        let node = ast.exprs.get(expr);
        match node.kind() {
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr => return self.call_expression(expr),
            ExprKind::UnaryOperator
            | ExprKind::BinaryOperator
            | ExprKind::CompoundAssignOperator => return self.operator_operands(expr),
            ExprKind::ConditionalOperator => {
                let Some(cond) = ExprId::try_new(node.a()) else { return false };
                let [then, els] = ast.exprs.cond_arms(expr);
                self.out.push_str("qu");
                return self.expression(cond) && self.expression(then) && self.expression(els);
            }
            ExprKind::UnaryExprOrTypeTraitExpr => return self.trait_expression(expr, converted),
            ExprKind::ArraySubscriptExpr => {
                self.out.push_str("ix");
                let [lhs, rhs] = ast.exprs.operands(expr);
                return self.expression(lhs) && self.expression(rhs);
            }
            ExprKind::CXXPseudoDestructorExpr
            | ExprKind::CXXUnresolvedConstructExpr
            | ExprKind::CXXNewExpr => return self.member_or_construct_expression(expr),
            // clang `CXXOperatorCallExprClass`: the operator for its operand
            // count, then the operands (`->` is the enclosing member
            // access's).
            ExprKind::CXXOperatorCallExpr => {
                let args: Vec<ExprId> =
                    ast.lists.iter::<ExprId>(ast.exprs.call_args(expr)).collect();
                let op = ast.exprs.operator_call_op(expr);
                if op != kiln_ast::names::OverloadedOperator::Arrow {
                    self.out.push_str(super::operator_code(op, args.len()));
                }
                for a in args {
                    if !self.expression(a) {
                        return false;
                    }
                }
            }
            // A braced-init-list as an operand (a call argument,
            // `f({1, V...})`): `il <expression>* E` (clang
            // `mangleExpression` `InitListExprClass`).
            ExprKind::InitListExpr => return self.init_list(expr),
            ExprKind::CXXThrowExpr | ExprKind::CXXTypeidExpr | ExprKind::CXXNoexceptExpr => {
                return self.keyword_expression(expr);
            }
            ExprKind::CXXScalarValueInitExpr => {
                self.out.push_str("cv");
                self.ty(node.ty());
                self.out.push_str("_E");
            }
            ExprKind::CXXFunctionalCastExpr
            | ExprKind::CXXTemporaryObjectExpr
            | ExprKind::CXXConstructExpr => return self.construct_expression(expr),
            // An expression splice `[: r :]` (`template [: r :]<args>`):
            // GCC 16's `DS <expression> [<template-args>] E` (P2996 has no
            // Itanium encoding yet).
            ExprKind::CXXSpliceExpr => return self.splice_expression(expr),
            ExprKind::PackExpansionExpr => {
                self.out.push_str("sp");
                return self.expression(ast.exprs.operand(expr));
            }
            ExprKind::CStyleCastExpr
            | ExprKind::CXXStaticCastExpr
            | ExprKind::CXXConstCastExpr
            | ExprKind::CXXReinterpretCastExpr
            | ExprKind::CXXDynamicCastExpr => {
                self.out.push_str(match node.kind() {
                    ExprKind::CStyleCastExpr => "cv",
                    ExprKind::CXXStaticCastExpr => "sc",
                    ExprKind::CXXConstCastExpr => "cc",
                    ExprKind::CXXReinterpretCastExpr => "rc",
                    _ => "dc",
                });
                self.ty(node.ty());
                return self.expression(ast.exprs.operand(expr));
            }
            ExprKind::CXXDeleteExpr => return self.delete_expression(expr),
            _ => return false,
        }
        true
    }

    /// A built-in unary or binary operator, compound assignments
    /// included: `<operator-name> <expression>+`.
    fn operator_operands(&mut self, expr: ExprId) -> bool {
        let ast = self.ast;
        let node = ast.exprs.get(expr);
        if node.kind() == ExprKind::UnaryOperator {
            let code = match ast.exprs.unary_op(expr) {
                UnaryOp::PostInc | UnaryOp::PreInc => "pp",
                UnaryOp::PostDec | UnaryOp::PreDec => "mm",
                UnaryOp::AddrOf => "ad",
                UnaryOp::Deref => "de",
                UnaryOp::Plus => "ps",
                UnaryOp::Minus => "ng",
                UnaryOp::Not => "co",
                UnaryOp::LNot => "nt",
                // clang has no operator name for these and mangles the
                // operand alone (WP-980).
                UnaryOp::Real | UnaryOp::Imag | UnaryOp::Extension => "",
                UnaryOp::Coawait => return false,
            };
            self.out.push_str(code);
            return self.expression(ast.exprs.operand(expr));
        }
        self.out.push_str(binary_code(ast.exprs.binary_op(expr)));
        // A compound assignment is clang's `CompoundAssignOperator`, a
        // `BinaryOperator` to the mangler (`pL`, `mI`, ...; WP-980): its
        // left operand is `a`, the right in the side record.
        let [lhs, rhs] = if node.kind() == ExprKind::CompoundAssignOperator {
            let Some(lhs) = ExprId::try_new(node.a()) else { return false };
            [lhs, ast.exprs.compound_assign(expr).rhs]
        } else {
            ast.exprs.operands(expr)
        };
        self.expression(lhs) && self.expression(rhs)
    }

    /// `[gs] dl|da <expression>` (clang `CXXDeleteExprClass`; WP-980).
    fn delete_expression(&mut self, expr: ExprId) -> bool {
        let ast = self.ast;
        let node = ast.exprs.get(expr);
        if node.has(kiln_ast::expr::ExprFlags::GLOBAL) {
            self.out.push_str("gs");
        }
        self.out.push_str(if node.has(kiln_ast::expr::ExprFlags::ARRAY_FORM) {
            "da"
        } else {
            "dl"
        });
        self.expression(ast.exprs.delete_operand(expr))
    }

    /// The member-access, pseudo-destructor, functional-cast and
    /// `new` forms of [`Self::operator_expression`].
    fn member_or_construct_expression(&mut self, expr: ExprId) -> bool {
        let ast = self.ast;
        let node = ast.exprs.get(expr);
        match node.kind() {
            // `p.~T`: `dt <expression> dn <destructor-name>`.
            ExprKind::CXXPseudoDestructorExpr => {
                let (base, arrow, t) = ast.exprs.pseudo_destructor(expr);
                self.out.push_str(if arrow { "pt" } else { "dt" });
                if !self.expression(base) {
                    return false;
                }
                self.out.push_str("dn");
                self.ty(t);
            }
            // `T(args)` is `cv <type> <expression>` for one argument, else
            // `cv <type> _ <expression>* E`; `T{args}` is
            // `tl <type> <braced-expression>* E`.
            ExprKind::CXXUnresolvedConstructExpr => {
                let mut args: Vec<ExprId> = kiln_ast::ids::ListId::try_new(node.a())
                    .map(|l| ast.lists.iter::<ExprId>(l).collect())
                    .unwrap_or_default();
                let list = node.aux() != 0;
                // `T{a, b}` spells the braced list's elements.
                if list
                    && let [one] = args[..]
                    && ast.exprs.get(one).kind() == ExprKind::InitListExpr
                {
                    args = ast.lists.iter::<ExprId>(ast.exprs.init_list_inits(one)).collect();
                }
                self.out.push_str(if list { "tl" } else { "cv" });
                // A class template to deduce with dependent arguments is
                // its template name (clang leaves the deduced type of a
                // dependent `DeducedTemplateSpecializationType` null; kiln
                // records `DEPENDENT`, which the canonical type keeps),
                // without the pattern's <abi-tags>: clang mangles the
                // `ClassTemplateDecl` (`NS0_15drop_while_viewE_`).
                match ast.types.view(node.ty().ty()) {
                    TypeView::DeducedTemplateSpecialization { name, deduced }
                        if deduced.is_none_or(|d| d == QualType::DEPENDENT) =>
                    {
                        let saved = self.untagged.take();
                        self.untagged = ast.types.template_decl(name);
                        self.template_name(name);
                        self.untagged = saved;
                    }
                    _ => self.ty(node.ty()),
                }
                let wrap = !list && args.len() != 1;
                if wrap {
                    self.out.push('_');
                }
                for a in args {
                    if !self.expression(a) {
                        return false;
                    }
                }
                if list || wrap {
                    self.out.push('E');
                }
            }
            ExprKind::CXXNewExpr => return self.new_expression(expr),
            _ => return false,
        }
        true
    }

    /// clang `CXXNewExprClass`: `[gs] nw|na <expression>* _ <type>
    /// (pi <expression>* E | il <braced-expression>* E | E)`; an array
    /// new mangles its element type, not the bound (as clang 19).
    fn new_expression(&mut self, expr: ExprId) -> bool {
        let ast = self.ast;
        let node = ast.exprs.get(expr);
        let x = *ast.exprs.new_extra(expr);
        let style = ast.exprs.new_init_style(expr);
        if node.has(kiln_ast::expr::ExprFlags::GLOBAL) {
            self.out.push_str("gs");
        }
        self.out.push_str(if x.array_size.is_some() { "na" } else { "nw" });
        let placement: Vec<ExprId> =
            x.placement.map(|l| ast.lists.iter::<ExprId>(l).collect()).unwrap_or_default();
        for p in placement {
            if !self.expression(p) {
                return false;
            }
        }
        self.out.push('_');
        self.ty(x.allocated_ty);
        if style == kiln_ast::expr::NewInitStyle::Parens {
            self.out.push_str("pi");
            let args: Vec<ExprId> = match x.init {
                Some(i) if ast.exprs.get(i).kind() == ExprKind::ParenListExpr => {
                    kiln_ast::ids::ListId::try_new(ast.exprs.get(i).a())
                        .map(|l| ast.lists.iter::<ExprId>(l).collect())
                        .unwrap_or_default()
                }
                Some(i) => vec![i],
                None => Vec::new(),
            };
            for a in args {
                if !self.expression(a) {
                    return false;
                }
            }
            self.out.push('E');
        } else if style == kiln_ast::expr::NewInitStyle::Braces {
            // `new T{a, b}` (clang `mangleInitListElements`, WP-231).
            match x.init {
                Some(i) if ast.exprs.get(i).kind() == ExprKind::InitListExpr => {
                    if !self.init_list(i) {
                        return false;
                    }
                }
                _ => return false,
            }
        } else {
            self.out.push('E');
        }
        true
    }

    /// clang `mangleMemberExpr`: `dt`/`pt <expression> <unresolved-name>`;
    /// a member of the implicit object is `dtdefpT` (as GCC), one named
    /// by implicit access in a dependent class has no base at all.
    fn member_expression(&mut self, expr: ExprId, arity: usize) -> bool {
        let ast = self.ast;
        let node = ast.exprs.get(expr);
        let base = ExprId::try_new(node.a());
        let (arrow, nns, name, args) = if node.kind() == ExprKind::MemberExpr {
            let extra = ast.exprs.decl_ref_extra(expr);
            (
                node.has(kiln_ast::expr::ExprFlags::ARROW),
                extra.and_then(|x| x.qualifier),
                ast.decls.get(ast.exprs.decl(expr)).name(),
                extra.and_then(|x| x.template_args),
            )
        } else {
            (
                node.aux() & 1 != 0,
                ast.templates.expr_nns.get(expr).copied(),
                ast.dependent_member_name(expr),
                ast.templates.expr_targs.get(expr).copied(),
            )
        };
        // Behind an explicit object of dependent type, the qualifier's
        // first name cannot be looked up in the object's class yet
        // ([basic.lookup.qual.general]/2), so clang keeps it as the
        // identifier written: `t.T::f()` is `dtfp_sr1TE1f`, not
        // `dtfp_srT_1f` (spec/landed/abi/WP-1029.md).
        let mut spelled = false;
        if let Some(b) = base {
            let b = self.skip_transparent(b);
            let bn = ast.exprs.get(b);
            if bn.kind() == ExprKind::CXXThisExpr && ast.exprs.this_is_implicit(b) {
                self.out.push_str("dtdefpT");
            } else {
                spelled = bn.is_type_dependent();
                self.out.push_str(if arrow { "pt" } else { "dt" });
                if !self.expression(b) {
                    return false;
                }
            }
        }
        if let Some(nns) = nns
            && !self.unresolved_prefix_in(nns, false, spelled)
        {
            return false;
        }
        self.base_unresolved_name(name, args, arity)
    }

    /// clang `mangleUnresolvedName`'s <base-unresolved-name> and its
    /// optional template arguments: `<simple-id>`, `on <operator-name>`
    /// (an operator for `arity` operands, a conversion `cv <type>`, a
    /// literal operator `li <source-name>`) or `dn <destructor-name>`.
    fn base_unresolved_name(
        &mut self,
        name: kiln_ast::names::DeclName,
        args: Option<ArgListId>,
        arity: usize,
    ) -> bool {
        let ast = self.ast;
        match ast.names.view(name) {
            DeclNameView::Identifier(s) => self.symbol(s),
            DeclNameView::Operator(op) => {
                self.out.push_str("on");
                self.out.push_str(super::operator_code(op, arity));
            }
            DeclNameView::Conversion(q) => {
                self.out.push_str("oncv");
                self.ty(q);
            }
            DeclNameView::LiteralOperator(s) => {
                self.out.push_str("onli");
                self.symbol(s);
            }
            DeclNameView::Destructor(t) => {
                self.out.push_str("dn");
                if self.unresolved_type_or_simple_id(QualType::unqual(t), false).is_none() {
                    return false;
                }
            }
            _ => return false,
        }
        if let Some(args) = args {
            self.template_args(args);
        }
        true
    }

    /// clang's `CXXFunctionalCastExprClass`, `CXXTemporaryObjectExprClass`
    /// and `CXXConstructExprClass`: `T(a)` is `cv <type> <expression>`,
    /// `T(a, b)` `cv <type> _ <expression>* E`, `T{a, b}` `tl <type>
    /// <braced-expression>* E`; an implicit construction is its operand.
    fn construct_expression(&mut self, expr: ExprId) -> bool {
        let ast = self.ast;
        let node = ast.exprs.get(expr);
        match node.kind() {
            ExprKind::CXXFunctionalCastExpr => {
                let mut sub = self.skip_implicit(ast.exprs.operand(expr));
                if ast.exprs.get(sub).kind() == ExprKind::CXXConstructExpr
                    && !ast.exprs.construct_info(sub).list
                    && let Some(a) = ast.lists.iter::<ExprId>(ast.exprs.construct_args(sub)).next()
                {
                    sub = self.skip_implicit(a);
                }
                if ast.exprs.get(sub).kind() == ExprKind::CXXStdInitializerListExpr {
                    sub = self.skip_implicit(ast.exprs.operand(sub));
                }
                if ast.exprs.get(sub).kind() == ExprKind::InitListExpr {
                    self.out.push_str("tl");
                    self.ty(node.ty());
                    return self.braced_elements(sub);
                }
                self.out.push_str("cv");
                self.ty(node.ty());
                self.expression(sub)
            }
            ExprKind::CXXTemporaryObjectExpr => {
                let info = ast.exprs.construct_info(expr);
                let args: Vec<ExprId> =
                    ast.lists.iter::<ExprId>(ast.exprs.construct_args(expr)).collect();
                self.out.push_str(if info.list { "tl" } else { "cv" });
                self.ty(node.ty());
                let wrap = !info.list && args.len() != 1;
                if wrap {
                    self.out.push('_');
                }
                if info.std_init_list
                    && let Some(&a) = args.first()
                {
                    let mut il = self.skip_implicit(a);
                    if ast.exprs.get(il).kind() == ExprKind::CXXStdInitializerListExpr {
                        il = self.skip_implicit(ast.exprs.operand(il));
                    }
                    if ast.exprs.get(il).kind() != ExprKind::InitListExpr {
                        return false;
                    }
                    for e in ast.lists.iter::<ExprId>(ast.exprs.init_list_inits(il)) {
                        if !self.braced_element(e) {
                            return false;
                        }
                    }
                } else {
                    for a in args {
                        if !self.expression(a) {
                            return false;
                        }
                    }
                }
                if info.list || wrap {
                    self.out.push('E');
                }
                true
            }
            _ => {
                let info = ast.exprs.construct_info(expr);
                let args: Vec<ExprId> =
                    ast.lists.iter::<ExprId>(ast.exprs.construct_args(expr)).collect();
                if !info.list || info.std_init_list {
                    return args.first().is_some_and(|&a| self.expression(a));
                }
                self.out.push_str("il");
                for a in args {
                    if !self.expression(a) {
                        return false;
                    }
                }
                self.out.push('E');
                true
            }
        }
    }

    /// clang `IgnoreImplicit`: implicit casts, temporaries and cleanups.
    fn skip_implicit(&self, mut e: ExprId) -> ExprId {
        let ex = &self.ast.exprs;
        loop {
            e = match ex.get(e).kind() {
                ExprKind::ImplicitCastExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::ConstantExpr => ex.operand(e),
                _ => return e,
            };
        }
    }

    /// `<braced-expression>* E` of braced list `list` (clang
    /// `mangleInitListElements`).
    fn braced_elements(&mut self, list: ExprId) -> bool {
        let ast = self.ast;
        for e in ast.lists.iter::<ExprId>(ast.exprs.init_list_inits(list)) {
            if !self.braced_element(e) {
                return false;
            }
        }
        self.out.push('E');
        true
    }

    fn braced_element(&mut self, e: ExprId) -> bool {
        if self.ast.exprs.get(e).kind() == ExprKind::InitListExpr {
            self.init_list(e)
        } else {
            self.expression(e)
        }
    }

    /// `il <braced-expression>* E`: a braced-init-list's elements.
    fn init_list(&mut self, list: ExprId) -> bool {
        let ast = self.ast;
        self.out.push_str("il");
        for e in ast.lists.iter::<ExprId>(ast.exprs.init_list_inits(list)) {
            let ok = if ast.exprs.get(e).kind() == ExprKind::InitListExpr {
                self.init_list(e)
            } else {
                self.expression(e)
            };
            if !ok {
                return false;
            }
        }
        self.out.push('E');
        true
    }

    /// clang `mangleUnresolvedPrefix`: `sr` levels of a dependent
    /// qualifier as written, closed by `E` unless the innermost level is
    /// an unresolved type (a template parameter or `decltype`).
    pub(super) fn unresolved_prefix(&mut self, nns: NnsId, recursive: bool) -> bool {
        self.unresolved_prefix_in(nns, recursive, false)
    }

    /// [`Self::unresolved_prefix`]; with `spelled`, a first level that
    /// names a type by a plain identifier is that identifier (the
    /// qualifier of a member access on a dependent object).
    fn unresolved_prefix_in(&mut self, nns: NnsId, recursive: bool, spelled: bool) -> bool {
        let ast = self.ast;
        if spelled && let Some(s) = ast.spelled_member_qualifier(nns) {
            self.out.push_str("sr");
            self.symbol(s);
            if !recursive {
                self.out.push('E');
            }
            return true;
        }
        let lead = |m: &mut Self, prefix: Option<NnsId>| {
            if let Some(p) = prefix {
                m.unresolved_prefix_in(p, true, spelled)
            } else {
                m.out.push_str("sr");
                true
            }
        };
        match ast.types.nns_view(nns) {
            Nns::Global => {
                self.out.push_str("gs");
                if recursive {
                    self.out.push_str("sr");
                }
                return true;
            }
            // A namespace alias keeps its own name, as clang's
            // `mangleUnresolvedPrefix` does: `na::P::` is `sr2na1PE`.
            Nns::Namespace { prefix, ns } | Nns::NamespaceAlias { prefix, alias: ns, .. } => {
                if !lead(self, prefix) {
                    return false;
                }
                let DeclNameView::Identifier(s) = ast.names.view(ast.decls.get(ns).name()) else {
                    return false;
                };
                self.symbol(s);
            }
            Nns::Type { prefix, ty, .. } => {
                if !lead(self, prefix) {
                    return false;
                }
                match self.unresolved_type_or_simple_id(ty, recursive) {
                    None => return false,
                    Some(true) => return true,
                    Some(false) => {}
                }
            }
            Nns::Identifier { prefix, name } => {
                if !lead(self, prefix) {
                    return false;
                }
                self.symbol(name);
            }
            Nns::Splice { .. } => return false,
        }
        if !recursive {
            self.out.push('E');
        }
        true
    }

    /// clang `mangleUnresolvedTypeOrSimpleId`: `Some(true)` after an
    /// unresolved type (no closing `E` follows), `Some(false)` after a
    /// simple id, `None` when unsupported.
    /// The template head of the template template parameter that `ty`,
    /// as written, specializes.
    fn written_ttp_params(&self, ty: QualType) -> Option<TemplateParamListId> {
        let ast = self.ast;
        let TypeView::TemplateSpecialization { name, .. } = ast.types.view(ty.ty()) else {
            return None;
        };
        let TemplateName::TemplateTemplateParm { decl, .. } = ast.types.template_name_view(name)
        else {
            return None;
        };
        ast.template_parm(decl).and_then(|x| x.params)
    }

    fn unresolved_type_or_simple_id(&mut self, ty: QualType, recursive: bool) -> Option<bool> {
        let ast = self.ast;
        // An alias template specialization keeps its name as written
        // (clang mangles the qualifier's sugared type): `_And<T>::value`
        // is `sr4_AndIT_E5value`, not its expansion (WP-818).
        if let Some((t, args)) = self.written_alias(ty) {
            let DeclNameView::Identifier(s) = ast.names.view(ast.decls.get(t).name()) else {
                return None;
            };
            self.symbol(s);
            self.template_args(args);
            return Some(false);
        }
        let q = ast.types.canonical(ty);
        // clang mangles a substituted template parameter (`T::m` in a
        // member of `C<P>`) as the type it became: an unresolved type
        // (`srS0_1m`), not the simple-id `1P` (WP-1029).
        let mut written = ty;
        while let TypeView::Elaborated { named, .. } = ast.types.view(written.ty()) {
            written = named;
        }
        // A typedef qualifier keeps its own name, as clang's
        // `case Type::Typedef` does: `P::Self::f` is `sr1P4SelfE1f`, not
        // the class it names (WP-1029). A using-declaration's type is its
        // target's.
        match ast.types.view(written.ty()) {
            TypeView::Typedef { decl, .. } => {
                let DeclNameView::Identifier(s) = ast.names.view(ast.decls.get(decl).name()) else {
                    return None;
                };
                self.symbol(s);
                return Some(false);
            }
            TypeView::Using { .. } => {
                return self
                    .unresolved_type_or_simple_id(ast.types.desugar_once(written), recursive);
            }
            _ => {}
        }
        let substituted =
            matches!(ast.types.view(written.ty()), TypeView::SubstTemplateTypeParm { .. });
        match ast.types.canonical_view(q) {
            _ if substituted => {
                if recursive {
                    self.out.push('N');
                }
                self.ty(q);
                Some(true)
            }
            TypeView::TemplateTypeParm(_)
            | TypeView::CanonTemplateTypeParm { .. }
            | TypeView::PackIndexing { .. }
            | TypeView::DependentDecltype(_) => {
                if recursive {
                    self.out.push('N');
                }
                self.ty(q);
                Some(true)
            }
            TypeView::TemplateSpecialization { name, args, .. } => {
                let n = ast.types.canonical_template_name(name);
                let t = match ast.types.template_name_view(n) {
                    TemplateName::Template(t) => t,
                    // A template template parameter's specialization is an
                    // unresolved type: `srT_IJ5is_ccIiEEE5value`.
                    // Its arguments are grouped as clang's converted list
                    // against the parameter's own template head.
                    TemplateName::TemplateTemplateParm { depth, index, .. }
                    | TemplateName::CanonTemplateTemplateParm { depth, index, .. } => {
                        if recursive {
                            self.out.push('N');
                        }
                        let key = Sub::Type(q);
                        if !self.try_sub(key) {
                            self.template_template_parm(depth, index);
                            let origin = ast.types.arg_origin(q.ty());
                            let params =
                                self.written_ttp_params(ty).or_else(|| self.ttp_head(depth, index));
                            self.grouped_args(params, origin.unwrap_or(args));
                            self.subs.push(key);
                        }
                        return Some(true);
                    }
                    _ => return None,
                };
                let DeclNameView::Identifier(s) = ast.names.view(ast.decls.get(t).name()) else {
                    return None;
                };
                self.symbol(s);
                let origin = ast.types.arg_origin(q.ty());
                self.template_args(self.written_args(ty).or(origin).unwrap_or(args));
                Some(false)
            }
            TypeView::DependentTemplateSpecialization { name, args, .. } => {
                self.symbol(name);
                self.template_args(args);
                Some(false)
            }
            TypeView::Record(d) | TypeView::Enum(d) => {
                let DeclNameView::Identifier(s) = ast.names.view(ast.decls.get(d).name()) else {
                    return None;
                };
                self.symbol(s);
                // A specialization is its template-id: `sr1BIiEE1f`.
                if let Some((_, args)) = self.spec_of(d) {
                    self.template_args(self.written_args(ty).unwrap_or(args));
                }
                Some(false)
            }
            _ => None,
        }
    }
}

impl Mangler<'_, '_> {
    /// The arguments of the dependent class template specialization `ty`
    /// as its qualifier spells them: clang's `mangleUnresolvedTypeOrSimpleId`
    /// mangles the `TemplateSpecializationType` of the qualifier as
    /// written, so `R<T>::value` with `template <class, class = void>
    /// struct R` is `sr1RIT_EE5value`, without the defaulted `void` the
    /// canonical type carries ([temp.arg.general]/9; WP-783).
    fn written_args(&self, ty: QualType) -> Option<ArgListId> {
        let types = &self.ast.types;
        let mut q = ty;
        loop {
            if let TypeView::TemplateSpecialization { args, alias: false, .. } = types.view(q.ty())
            {
                return Some(args);
            }
            let n = types.desugar_once(q);
            if n == q {
                return None;
            }
            q = n;
        }
    }
}

impl Mangler<'_, '_> {
    /// The alias template and written arguments of qualifier type `ty`
    /// when its outermost template specialization, under elaboration,
    /// parentheses and typedefs, names an alias template.
    fn written_alias(&self, ty: QualType) -> Option<(DeclId, ArgListId)> {
        let types = &self.ast.types;
        let mut q = ty;
        loop {
            match types.view(q.ty()) {
                TypeView::TemplateSpecialization { name, args, alias: true, .. } => {
                    let n = types.canonical_template_name(name);
                    let TemplateName::Template(t) = types.template_name_view(n) else {
                        return None;
                    };
                    return Some((t, args));
                }
                TypeView::Elaborated { .. }
                | TypeView::Paren(_)
                | TypeView::Typedef { .. }
                | TypeView::Using { .. } => q = types.desugar_once(q),
                _ => return None,
            }
        }
    }
}

/// <operator-name> of a binary operator in an expression.
pub(super) const fn binary_code(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::PtrMemD => "ds",
        BinaryOp::PtrMemI => "pm",
        BinaryOp::Mul => "ml",
        BinaryOp::Div => "dv",
        BinaryOp::Rem => "rm",
        BinaryOp::Add => "pl",
        BinaryOp::Sub => "mi",
        BinaryOp::Shl => "ls",
        BinaryOp::Shr => "rs",
        BinaryOp::Cmp => "ss",
        BinaryOp::LT => "lt",
        BinaryOp::GT => "gt",
        BinaryOp::LE => "le",
        BinaryOp::GE => "ge",
        BinaryOp::EQ => "eq",
        BinaryOp::NE => "ne",
        BinaryOp::And => "an",
        BinaryOp::Xor => "eo",
        BinaryOp::Or => "or",
        BinaryOp::LAnd => "aa",
        BinaryOp::LOr => "oo",
        BinaryOp::Comma => "cm",
        BinaryOp::Assign => "aS",
        BinaryOp::MulAssign => "mL",
        BinaryOp::DivAssign => "dV",
        BinaryOp::RemAssign => "rM",
        BinaryOp::AddAssign => "pL",
        BinaryOp::SubAssign => "mI",
        BinaryOp::ShlAssign => "lS",
        BinaryOp::ShrAssign => "rS",
        BinaryOp::AndAssign => "aN",
        BinaryOp::XorAssign => "eO",
        BinaryOp::OrAssign => "oR",
    }
}
