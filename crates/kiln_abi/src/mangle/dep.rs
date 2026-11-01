//! Dependent signature types and template-argument expressions
//! (spec/abi.md §8.14): what a function template's signature needs when
//! its return or parameter types name members of dependent types
//! (`typename rr<T>::type`), dependent template specializations and
//! value-dependent template arguments (`en<is_i<T>::value, int>`).
//! Follows clang's `CXXNameMangler` (`manglePrefix(NestedNameSpecifier)`,
//! `mangleTemplatePrefix(TemplateName)`, `mangleUnresolvedPrefix`,
//! `mangleExpression`).

use super::{Mangler, Structor, Sub};
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, ExprDeps, ExprKind, TraitKind, UnaryOp};
use kiln_ast::ids::{ArgListId, DeclId, ExprId, NnsId, QualType, TemplateNameId};
use kiln_ast::names::DeclNameView;
use kiln_ast::types::{ArgView, Nns, TagKeyword, TemplateName, TypeView};
use kiln_source::symbol::Symbol;

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
                self.template_args(args);
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
        match ast.types.canonical_view(q) {
            TypeView::TemplateSpecialization { name, args, .. } => {
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
        let ast = self.ast;
        let list: Vec<_> = ast.types.args(args).to_vec();
        let pack_at = ast.template_info(ast.first(t)).and_then(|i| {
            let params = ast.templates.param_list(i.params).params;
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
        let key = Sub::TemplateTemplateParm(depth, index);
        if !self.try_sub(key) {
            self.template_param(index);
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
            if self.expression(e) {
                self.out.push('E');
                return;
            }
        }
        self.out.truncate(len);
        self.subs.truncate(subs);
        self.out.push_str("u7unknown");
    }

    // ---- template-argument expressions -----------------------------------

    /// A value-dependent template argument (clang `mangleExpression` with
    /// `AsTemplateArg`): a primary expression as is, anything else
    /// wrapped in `X ... E`. Unsupported forms fall back to `XLi0EE`
    /// (spec/abi.md §8.14).
    pub(super) fn template_arg_expr(&mut self, e: Option<ExprId>) {
        let (len, subs) = (self.out.len(), self.subs.len());
        if let Some(e) = e {
            let e = self.skip_transparent(e);
            let primary = self.is_primary(e);
            if !primary {
                self.out.push('X');
            }
            if self.expression(e) {
                if !primary {
                    self.out.push('E');
                }
                return;
            }
        }
        self.out.truncate(len);
        self.subs.truncate(subs);
        self.out.push_str("XLi0EE");
    }

    /// Parentheses, implicit casts, `ConstantExpr` and substituted
    /// parameters do not appear in the mangling.
    fn skip_transparent(&self, mut e: ExprId) -> ExprId {
        let ex = &self.ast.exprs;
        loop {
            e = match ex.get(e).kind() {
                ExprKind::ParenExpr | ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr => {
                    ex.operand(e)
                }
                ExprKind::SubstNonTypeTemplateParmExpr => ex.subst_replacement(e),
                _ => return e,
            };
        }
    }

    fn is_primary(&self, e: ExprId) -> bool {
        let ast = self.ast;
        match ast.exprs.get(e).kind() {
            ExprKind::IntegerLiteral
            | ExprKind::CXXBoolLiteralExpr
            | ExprKind::CharacterLiteral
            | ExprKind::CXXNullPtrLiteralExpr => true,
            // A template parameter reference is wrapped (`XT_E`).
            _ => false,
        }
    }

    /// One expression; `false` when it (or a subexpression) has no
    /// supported encoding.
    fn expression(&mut self, e: ExprId) -> bool {
        let ast = self.ast;
        let e = self.skip_transparent(e);
        let n = ast.exprs.get(e);
        match n.kind() {
            ExprKind::IntegerLiteral => {
                let Some(v) = ast.exprs.int_value(e) else { return false };
                self.out.push('L');
                self.ty(n.ty());
                self.out.push_str(&v.to_string());
                self.out.push('E');
            }
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
            ExprKind::DeclRefExpr => {
                let d = ast.exprs.decl(e);
                if ast.decls.get(d).kind() != DeclKind::NonTypeTemplateParm {
                    return false;
                }
                let idx = ast.template_parm(d).map_or(0, |p| u32::from(p.index));
                self.template_param(idx);
            }
            ExprKind::DependentScopeDeclRefExpr => {
                let Some(nns) = NnsId::try_new(n.a()) else { return false };
                let DeclNameView::Identifier(name) =
                    ast.names.view(ast.dependent_scope_ref_name(e))
                else {
                    return false;
                };
                if !self.unresolved_prefix(nns, false) {
                    return false;
                }
                self.symbol(name);
                if let Some(&args) = ast.templates.expr_targs.get(e) {
                    self.template_args(args);
                }
            }
            ExprKind::UnresolvedLookupExpr => {
                let DeclNameView::Identifier(name) =
                    ast.names.view(kiln_ast::names::DeclName::from_raw(n.a()).unwrap_or_default())
                else {
                    return false;
                };
                if let Some(&nns) = ast.templates.expr_nns.get(e)
                    && !self.unresolved_prefix(nns, false)
                {
                    return false;
                }
                self.symbol(name);
                if let Some(&args) = ast.templates.expr_targs.get(e) {
                    self.template_args(args);
                }
            }
            ExprKind::CallExpr => {
                self.out.push_str("cl");
                if !self.expression(ast.exprs.callee(e)) {
                    return false;
                }
                let args: Vec<ExprId> = ast.lists.iter::<ExprId>(ast.exprs.call_args(e)).collect();
                for a in args {
                    if !self.expression(a) {
                        return false;
                    }
                }
                self.out.push('E');
            }
            ExprKind::UnaryOperator => {
                let code = match ast.exprs.unary_op(e) {
                    UnaryOp::PostInc | UnaryOp::PreInc => "pp",
                    UnaryOp::PostDec | UnaryOp::PreDec => "mm",
                    UnaryOp::AddrOf => "ad",
                    UnaryOp::Deref => "de",
                    UnaryOp::Plus => "ps",
                    UnaryOp::Minus => "ng",
                    UnaryOp::Not => "co",
                    UnaryOp::LNot => "nt",
                    _ => return false,
                };
                self.out.push_str(code);
                return self.expression(ast.exprs.operand(e));
            }
            ExprKind::BinaryOperator => {
                let Some(code) = binary_code(ast.exprs.binary_op(e)) else { return false };
                self.out.push_str(code);
                let [l, r] = ast.exprs.operands(e);
                return self.expression(l) && self.expression(r);
            }
            ExprKind::ConditionalOperator => {
                let Some(c) = ExprId::try_new(n.a()) else { return false };
                let [t, f] = ast.exprs.cond_arms(e);
                self.out.push_str("qu");
                return self.expression(c) && self.expression(t) && self.expression(f);
            }
            ExprKind::UnaryExprOrTypeTraitExpr => {
                // A non-dependent operand is encoded as the integer result,
                // which needs the evaluator: not supported here.
                if !n.deps().intersects(ExprDeps::VALUE_INST) {
                    return false;
                }
                let (t, x) = match ast.exprs.trait_kind(e) {
                    TraitKind::SizeOf => ("st", "sz"),
                    TraitKind::AlignOf => ("at", "az"),
                    _ => return false,
                };
                if let Some(q) = ast.exprs.trait_type_arg(e) {
                    self.out.push_str(t);
                    self.ty(q);
                } else {
                    let Some(&sub) = ast.exprs.children(e, &ast.lists).first() else {
                        return false;
                    };
                    self.out.push_str(x);
                    return self.expression(sub);
                }
            }
            ExprKind::PackExpansionExpr => {
                self.out.push_str("sp");
                return self.expression(ast.exprs.operand(e));
            }
            ExprKind::SizeOfPackExpr => {
                let Some(d) = DeclId::try_new(n.a()) else { return false };
                let Some(p) = ast.template_parm(d) else { return false };
                self.out.push_str("sZ");
                self.template_param(u32::from(p.index));
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
            ExprKind::CStyleCastExpr
            | ExprKind::CXXStaticCastExpr
            | ExprKind::CXXConstCastExpr
            | ExprKind::CXXReinterpretCastExpr
            | ExprKind::CXXDynamicCastExpr => {
                self.out.push_str(match n.kind() {
                    ExprKind::CStyleCastExpr => "cv",
                    ExprKind::CXXStaticCastExpr => "sc",
                    ExprKind::CXXConstCastExpr => "cc",
                    ExprKind::CXXReinterpretCastExpr => "rc",
                    _ => "dc",
                });
                self.ty(n.ty());
                return self.expression(ast.exprs.operand(e));
            }
            _ => return false,
        }
        true
    }

    /// clang `mangleUnresolvedPrefix`: `sr` levels of a dependent
    /// qualifier as written, closed by `E` unless the innermost level is
    /// an unresolved type (a template parameter or `decltype`).
    fn unresolved_prefix(&mut self, nns: NnsId, recursive: bool) -> bool {
        let ast = self.ast;
        let lead = |m: &mut Self, prefix: Option<NnsId>| match prefix {
            Some(p) => m.unresolved_prefix(p, true),
            None => {
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
            Nns::Namespace { prefix, ns } | Nns::NamespaceAlias { prefix, ns, .. } => {
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
    fn unresolved_type_or_simple_id(&mut self, ty: QualType, recursive: bool) -> Option<bool> {
        let ast = self.ast;
        let q = ast.types.canonical(ty);
        match ast.types.canonical_view(q) {
            TypeView::TemplateTypeParm(_)
            | TypeView::CanonTemplateTypeParm { .. }
            | TypeView::DependentDecltype(_) => {
                if recursive {
                    self.out.push('N');
                }
                self.ty(q);
                Some(true)
            }
            TypeView::TemplateSpecialization { name, args, .. } => {
                let n = ast.types.canonical_template_name(name);
                let TemplateName::Template(t) = ast.types.template_name_view(n) else {
                    return None;
                };
                let DeclNameView::Identifier(s) = ast.names.view(ast.decls.get(t).name()) else {
                    return None;
                };
                self.symbol(s);
                self.template_args(args);
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
                Some(false)
            }
            _ => None,
        }
    }
}

/// <operator-name> of a binary operator in an expression.
const fn binary_code(op: BinaryOp) -> Option<&'static str> {
    Some(match op {
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
        _ => return None,
    })
}
