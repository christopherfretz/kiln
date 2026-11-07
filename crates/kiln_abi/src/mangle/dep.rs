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
use kiln_ast::ids::{ArgListId, DeclId, ExprId, NnsId, QualType, TemplateNameId};
use kiln_ast::names::DeclNameView;
use kiln_ast::types::{ArgView, ExprOperand, Nns, TagKeyword, TemplateName, TypeView};
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
            if self.expression(e) {
                self.out.push('E');
                return;
            }
        }
        self.out.truncate(len);
        self.subs.truncate(subs);
        self.out.push_str("u7unknown");
    }

    /// The `A <expression>` head of a dependent-bound array type; an
    /// unencodable bound is the vendor placeholder.
    pub(super) fn dependent_array_bound(&mut self, size: ExprOperand) {
        let (len, subs) = (self.out.len(), self.subs.len());
        let e = match size {
            ExprOperand::Expr(e) => Some(e),
            ExprOperand::Profile(p) => self.ast.types.profile_expr(p),
        };
        self.out.push('A');
        if e.is_some_and(|e| self.expression(e)) {
            return;
        }
        self.out.truncate(len);
        self.subs.truncate(subs);
        self.out.push_str("Au7unknown");
    }

    // ---- template-argument expressions -----------------------------------

    /// A value-dependent template argument (clang `mangleExpression` with
    /// `AsTemplateArg`): a primary expression as is, anything else
    /// wrapped in `X ... E`. Unsupported forms fall back to `XLi0EE`
    /// (spec/abi.md §8.15).
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
            | ExprKind::CXXNullPtrLiteralExpr
            | ExprKind::GNUNullExpr
            // A reflection is a literal (`L <info-type> <reflection> E`).
            | ExprKind::CXXReflectExpr => true,
            // An enumerator is its value's literal.
            ExprKind::DeclRefExpr => {
                ast.decls.get(ast.exprs.decl(e)).kind() == DeclKind::EnumConstant
            }
            // A template parameter reference is wrapped (`XT_E`).
            _ => false,
        }
    }

    /// One expression; `false` when it (or a subexpression) has no
    /// supported encoding.
    pub(super) fn expression(&mut self, e: ExprId) -> bool {
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
            ExprKind::CXXReflectExpr => return self.reflect_expr(e),
            // clang mangles `__null` as the integer literal 0 of its type.
            ExprKind::GNUNullExpr => {
                self.out.push('L');
                self.ty(n.ty());
                self.out.push_str("0E");
            }
            ExprKind::DeclRefExpr => {
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
                    _ => return false,
                }
            }
            ExprKind::ConceptSpecializationExpr => return self.concept_id_expr(e),
            ExprKind::CXXFoldExpr => return self.fold_expr(e),
            ExprKind::RequiresExpr => return self.requires_expr(e),
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
            _ => return self.operator_expression(e),
        }
        true
    }

    /// An operator, call, cast, `sizeof`/`alignof` or pack expansion.
    fn operator_expression(&mut self, expr: ExprId) -> bool {
        let ast = self.ast;
        let node = ast.exprs.get(expr);
        match node.kind() {
            ExprKind::CallExpr => {
                self.out.push_str("cl");
                if !self.expression(ast.exprs.callee(expr)) {
                    return false;
                }
                let args: Vec<ExprId> =
                    ast.lists.iter::<ExprId>(ast.exprs.call_args(expr)).collect();
                for arg in args {
                    if !self.expression(arg) {
                        return false;
                    }
                }
                self.out.push('E');
            }
            ExprKind::UnaryOperator => {
                let code = match ast.exprs.unary_op(expr) {
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
                return self.expression(ast.exprs.operand(expr));
            }
            ExprKind::BinaryOperator => {
                let code = binary_code(ast.exprs.binary_op(expr));
                self.out.push_str(code);
                let [lhs, rhs] = ast.exprs.operands(expr);
                return self.expression(lhs) && self.expression(rhs);
            }
            ExprKind::ConditionalOperator => {
                let Some(cond) = ExprId::try_new(node.a()) else { return false };
                let [then, els] = ast.exprs.cond_arms(expr);
                self.out.push_str("qu");
                return self.expression(cond) && self.expression(then) && self.expression(els);
            }
            ExprKind::UnaryExprOrTypeTraitExpr => {
                // A non-dependent operand is encoded as the integer result,
                // which needs the evaluator: not supported here.
                if !node.deps().intersects(ExprDeps::VALUE_INST) {
                    return false;
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
            }
            ExprKind::ArraySubscriptExpr => {
                self.out.push_str("ix");
                let [lhs, rhs] = ast.exprs.operands(expr);
                return self.expression(lhs) && self.expression(rhs);
            }
            ExprKind::CXXDependentScopeMemberExpr
            | ExprKind::CXXPseudoDestructorExpr
            | ExprKind::CXXUnresolvedConstructExpr
            | ExprKind::CXXNewExpr => return self.member_or_construct_expression(expr),
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
            _ => return false,
        }
        true
    }

    /// The member-access, pseudo-destructor, functional-cast and
    /// `new` forms of [`Self::operator_expression`].
    fn member_or_construct_expression(&mut self, expr: ExprId) -> bool {
        let ast = self.ast;
        let node = ast.exprs.get(expr);
        match node.kind() {
            // clang `mangleMemberExpr`: `dt`/`pt <expression>
            // <unresolved-name>`.
            ExprKind::CXXDependentScopeMemberExpr => {
                let Some(base) = ExprId::try_new(node.a()) else { return false };
                self.out.push_str(if node.aux() & 1 != 0 { "pt" } else { "dt" });
                if !self.expression(base) {
                    return false;
                }
                if let Some(&nns) = ast.templates.expr_nns.get(expr)
                    && !self.unresolved_prefix(nns, false)
                {
                    return false;
                }
                let DeclNameView::Identifier(name) =
                    ast.names.view(ast.dependent_member_name(expr))
                else {
                    return false;
                };
                self.symbol(name);
                if let Some(&args) = ast.templates.expr_targs.get(expr) {
                    self.template_args(args);
                }
            }
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
                self.ty(node.ty());
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
        let ast = self.ast;
        let lead = |m: &mut Self, prefix: Option<NnsId>| {
            if let Some(p) = prefix {
                m.unresolved_prefix(p, true)
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
