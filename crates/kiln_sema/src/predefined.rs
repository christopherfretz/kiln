//! Predefined identifiers `__func__`, `__FUNCTION__` and
//! `__PRETTY_FUNCTION__` ([dcl.fct.def.general]/8; clang
//! `Sema::BuildPredefinedExpr` and `PredefinedExpr::ComputeName`).

use kiln_ast::consts::{PrintOptions as ValueOptions, PrintStyle};
use kiln_ast::decl::{DeclKind, DefinitionBits, FnStorage};
use kiln_ast::dump::AstNames;
use kiln_ast::expr::{PredefinedKind, StringKind};
use kiln_ast::strings::StringEncoding;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{
    ArgView, PrintOptions, RefQual, TemplateArg, TypeView, write_declarator, write_template_arg,
    write_type,
};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// A predefined identifier at `loc` (clang `BuildPredefinedExpr`):
    /// names the innermost enclosing function; outside one, warns and
    /// names the translation unit. In a dependent context the type is
    /// dependent and the name is computed at instantiation.
    pub fn act_on_predefined_expr(&mut self, kind: PredefinedKind, loc: Loc) -> ExprId {
        let func = self.enclosing_function(self.ctx);
        if func.is_none() {
            let msg = self.diag(DiagId::sema_predefined_identifier_outside_function, loc);
            self.emit(msg);
        }
        if let Some(f) = func
            && self.ctx_template_levels(f) > 0
        {
            return self.builder().predefined_expr(kind, QualType::DEPENDENT, None, loc);
        }
        let name = match func {
            Some(f) => self.compute_predefined_name(kind, f),
            None if kind == PredefinedKind::PrettyFunction => "top level".to_owned(),
            None => String::new(),
        };
        let lit = self.ast.strings.intern(StringEncoding::Ordinary, name.as_bytes());
        let ty = crate::literal::string_literal_type(
            self,
            kiln_lex::Encoding::Ordinary,
            name.len() as u64 + 1,
        );
        let sl = self.builder().string_literal(ty, StringKind::Ordinary, lit, loc);
        self.builder().predefined_expr(kind, ty, Some(sl), loc)
    }

    /// The innermost function enclosing context `ctx` (clang
    /// `getPredefinedExprDecl`).
    pub(crate) fn enclosing_function(&self, ctx: DeclId) -> Option<DeclId> {
        let mut cur = Some(ctx);
        let mut guard = 0;
        while let Some(d) = cur {
            if self.ast.decls.get(d).kind().is_function() {
                return Some(d);
            }
            guard += 1;
            if guard > 1024 {
                break;
            }
            cur = self.ast.decls.get(d).ctx();
        }
        None
    }

    /// The instantiation pattern of function `f` (clang
    /// `getTemplateInstantiationPattern`): the declaration its definition
    /// is instantiated from, through member templates; `f` itself for an
    /// explicit specialization or a non-template.
    pub(crate) fn function_pattern(&self, f: DeclId) -> DeclId {
        let mut cur = f;
        for _ in 0..64 {
            if let Some((_, s)) = self.ast.spec_info(cur)
                && s.kind == SpecKind::ExplicitSpecialization
            {
                break;
            }
            match self.ast.instantiated_from(cur) {
                Some(p) if p != cur && self.ast.decls.get(p).kind().is_function() => cur = p,
                _ => break,
            }
        }
        cur
    }

    /// clang `PredefinedExpr::ComputeName(kind, f)` for function `f`.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn compute_predefined_name(&self, kind: PredefinedKind, f: DeclId) -> String {
        let ast = &self.ast;
        let mut names = AstNames::new(ast);
        names.closure_as_lambda = self.clang_compat >= 22;
        if kind != PredefinedKind::PrettyFunction {
            let mut s = String::new();
            names.write_name(f, &mut s);
            return s;
        }
        let n = ast.decls.get(f);
        let mut out = String::new();
        let method = n.kind().is_function()
            && n.ctx().is_some_and(|c| ast.decls.get(c).kind() == DeclKind::CXXRecord)
            && matches!(
                n.kind(),
                DeclKind::CXXMethod
                    | DeclKind::CXXConstructor
                    | DeclKind::CXXDestructor
                    | DeclKind::CXXConversion
            );
        if method && let Some(fp) = ast.decls.function(f) {
            if fp.spec.is_virtual() {
                out.push_str("virtual ");
            }
            if fp.spec.storage() == FnStorage::Static {
                out.push_str("static ");
            }
        }
        let pattern = self.function_pattern(f);
        let fp = self.decl_sugared_proto(pattern);
        let mut proto = String::new();
        names.write_scope_prefix(f, true, &mut proto);
        match n.kind() {
            // A constructor or destructor name prints the class name
            // without arguments (clang `DeclarationName::print`).
            DeclKind::CXXConstructor | DeclKind::CXXDestructor => {
                if n.kind() == DeclKind::CXXDestructor {
                    proto.push('~');
                }
                if let Some(c) = n.ctx() {
                    names.write_name(c, &mut proto);
                }
            }
            _ => names.write_name(f, &mut proto),
        }
        proto.push('(');
        let params: Vec<QualType> = match ast.decls.function(pattern) {
            Some(x) if x.params != kiln_ast::ListId::EMPTY => {
                ast.lists.iter::<DeclId>(x.params).filter_map(|p| ast.decls.get(p).ty()).collect()
            }
            _ => fp.map(|p| ast.types.params(p).to_vec()).unwrap_or_default(),
        };
        for (i, &q) in params.iter().enumerate() {
            if i > 0 {
                proto.push_str(", ");
            }
            write_type(&ast.types, ast, q, PrintOptions::SUGARED, &mut proto);
        }
        if fp.is_some_and(kiln_ast::types::FuncProto::variadic) {
            if !params.is_empty() {
                proto.push_str(", ");
            }
            proto.push_str("...");
        }
        proto.push(')');
        if method && let Some(p) = fp {
            let info = p.info();
            if info.this_quals().is_const() {
                proto.push_str(" const");
            }
            if info.this_quals().is_volatile() {
                proto.push_str(" volatile");
            }
            match info.ref_qual() {
                RefQual::LValue => proto.push_str(" &"),
                RefQual::RValue => proto.push_str(" &&"),
                RefQual::None => {}
            }
        }
        let suffix = self.predefined_template_suffix(f);
        if !suffix.is_empty() {
            proto.push_str(" [");
            proto.push_str(&suffix);
            proto.push(']');
        }
        let lambda = n
            .ctx()
            .and_then(|c| ast.decls.record(c))
            .is_some_and(|r| r.defn.contains(DefinitionBits::LAMBDA));
        if lambda {
            out.push_str("auto ");
            out.push_str(&proto);
            return out;
        }
        if let Some(p) = fp
            && let TypeView::Decltype { underlying, .. } = ast.types.view(p.ret().ty())
        {
            write_declarator(&ast.types, ast, underlying, PrintOptions::SUGARED, &proto, &mut out);
            return out;
        }
        if matches!(n.kind(), DeclKind::CXXConstructor | DeclKind::CXXDestructor) {
            out.push_str(&proto);
            return out;
        }
        match fp {
            Some(p) => {
                write_declarator(&ast.types, ast, p.ret(), PrintOptions::SUGARED, &proto, &mut out);
            }
            None => out.push_str(&proto),
        }
        out
    }

    /// The declared function type of `f`, with its sugar.
    fn decl_sugared_proto(&self, f: DeclId) -> Option<&kiln_ast::types::FuncProto> {
        let q = self.ast.decls.get(f).ty()?;
        let q = self.ast.types.desugar_top(q);
        self.ast.types.proto_of(q.ty()).or_else(|| self.ast.types.proto(q))
    }

    /// The `P = A, ...` list of `__PRETTY_FUNCTION__`: the arguments of
    /// the enclosing implicit class template specializations, outermost
    /// first, then those of `f` when it is an implicit function template
    /// specialization (printed with their types).
    fn predefined_template_suffix(&self, f: DeclId) -> String {
        let ast = &self.ast;
        let mut specs = Vec::new();
        let mut cur = ast.decls.get(f).ctx();
        let mut guard = 0;
        while let Some(c) = cur {
            let k = ast.decls.get(c).kind();
            if k == DeclKind::TranslationUnit || guard > 1024 {
                break;
            }
            if k == DeclKind::CXXRecord
                && let Some((_, s)) = ast.spec_info(c)
                && s.kind != SpecKind::ExplicitSpecialization
            {
                specs.push(c);
            }
            guard += 1;
            cur = ast.decls.get(c).ctx();
        }
        let mut out = String::new();
        for &c in specs.iter().rev() {
            self.write_spec_args(c, false, &mut out);
        }
        if let Some((_, s)) = ast.spec_info(f)
            && s.kind != SpecKind::ExplicitSpecialization
        {
            self.write_spec_args(f, true, &mut out);
        }
        if out.len() >= 2 {
            out.truncate(out.len() - 2);
        }
        out
    }

    /// Appends `P = A, ` for each named parameter of specialization `d`.
    fn write_spec_args(&self, d: DeclId, include_type: bool, out: &mut String) {
        let ast = &self.ast;
        let Some((_, s)) = ast.spec_info(d) else { return };
        let Some(info) = ast.template_info(s.template) else { return };
        let params: Vec<DeclId> =
            ast.lists.iter::<DeclId>(ast.templates.param_list(info.params).params).collect();
        let args = ast.types.args(s.args);
        let names = AstNames::new(ast);
        for (i, &p) in params.iter().enumerate() {
            let Some(&a) = args.get(i) else { break };
            let mut pname = String::new();
            names.write_name(p, &mut pname);
            if pname.is_empty() {
                continue;
            }
            out.push_str(&pname);
            out.push_str(" = ");
            let typed = include_type || self.nttp_has_deduced_type(p);
            self.write_predefined_arg(a, typed, out);
            out.push_str(", ");
        }
    }

    /// Whether template parameter `p` is a non-type parameter with a
    /// placeholder type (clang `shouldIncludeTypeForArgument`).
    fn nttp_has_deduced_type(&self, p: DeclId) -> bool {
        if self.ast.decls.get(p).kind() != DeclKind::NonTypeTemplateParm {
            return false;
        }
        let Some(q) = self.ast.decls.get(p).ty() else { return false };
        let q = self.ast.types.desugar_top(q);
        matches!(
            self.ast.types.view(q.ty()),
            TypeView::Auto { .. } | TypeView::DeducedTemplateSpecialization { .. }
        )
    }

    /// clang `TemplateArgument::print(Policy, Out, IncludeType)`.
    fn write_predefined_arg(&self, a: TemplateArg, include_type: bool, out: &mut String) {
        let ast = &self.ast;
        let names = AstNames::new(ast);
        match a.view() {
            ArgView::Value { value, .. } if include_type => {
                let opts = ValueOptions {
                    include_type: true,
                    ..ValueOptions::with_style(PrintStyle::TemplateArg)
                };
                ast.consts.print(value, &ast.strings, &names, &opts, out);
            }
            // The specialization's arguments are printed canonical, as
            // clang stores them: full scopes, `(anonymous namespace)::`
            // included, and no typedef sugar.
            ArgView::Type(q) => write_type(&ast.types, &names, q, PrintOptions::CANONICAL, out),
            ArgView::Pack(l) => {
                out.push('<');
                for (i, &x) in ast.types.args(l).iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    self.write_predefined_arg(x, include_type, out);
                }
                out.push('>');
            }
            _ => write_template_arg(&ast.types, &names, a, out),
        }
    }
}
