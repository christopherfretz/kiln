//! Expressions in the definition hash: every [`ExprKind`], each arm
//! naming its area. Structural implicit nodes are transparent (O-10:
//! implicit conversions, temporaries, cleanups, constant wrappers,
//! parentheses, the replacement of a template parameter); of sema's
//! implicit work only the resolved callee of each call hashes, and it
//! does as the callee subexpression. No cached value, odr-use mark or
//! overload-candidate flag of a node is read.

use kiln_ast::consts::Scalar;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{ExprFlags, ExprKind as K};
use kiln_ast::ids::ListId;
use kiln_ast::{DeclId, ExprId};
use kiln_source::Loc;

use super::{
    Area, DefHasher, Item, Site, T_ENTITY, T_ENUMERATOR, T_FIELD, T_LOCAL, T_TU_PLACEHOLDER,
    T_VALUE_PLACEHOLDER, Work,
};

impl DefHasher<'_> {
    /// Whether the token at `loc` was produced by a builtin macro
    /// (`__LINE__`, `__FILE__`, `__COUNTER__`, `__DATE__`, `__TIME__`) or
    /// by `#`/`##`: such tokens are spelled in the scratch space.
    fn in_scratch(&self, loc: Loc) -> bool {
        loc.is_macro() && self.sm.is_written_in_scratch_space(self.sm.spelling_loc(loc))
    }

    fn value_placeholder(&mut self, k: K, detail: u8) {
        self.out.extend_from_slice(&[T_VALUE_PLACEHOLDER, super::E_BASE + k as u8, detail]);
    }

    /// A count and the expressions of list `l`.
    fn exprs(&mut self, l: ListId) {
        self.count(self.ast.lists.len(l));
        self.stack.extend(self.ast.lists.iter::<ExprId>(l).map(Work::Expr));
    }

    /// A reference to declaration `d`: a local by index, an enumerator as
    /// (enumeration, name, value), a data member as (class, name), a
    /// function or variable by its mangled name.
    fn decl_ref(&mut self, d: DeclId) {
        let ast = self.ast;
        if let Some(i) = self.local(d) {
            self.byte(T_LOCAL);
            return self.u32(i);
        }
        let k = ast.decls.get(d).kind();
        match k {
            DeclKind::Function
            | DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion
            | DeclKind::Var
            | DeclKind::VarTemplateSpecialization => match self.refs.entity(d, &mut self.stats) {
                Some(h) => {
                    self.byte(T_ENTITY);
                    self.out.extend_from_slice(&h.to_le_bytes());
                }
                None => self.byte(T_TU_PLACEHOLDER),
            },
            DeclKind::EnumConstant => {
                self.byte(T_ENUMERATOR);
                self.owner(d);
                self.name(d);
                let value = ast
                    .decls
                    .enumerator(d)
                    .and_then(|x| x.value)
                    .and_then(|v| ast.consts.scalar(v))
                    .and_then(|v| match v {
                        Scalar::Int { bits, .. } => bits.to_i128(),
                        Scalar::Bool { value, .. } => Some(i128::from(value)),
                        _ => None,
                    });
                match value {
                    Some(v) => self.out.extend_from_slice(&v.to_le_bytes()),
                    None => self.partial(Area::Misc, Site::Decl(k)),
                }
            }
            DeclKind::Field => {
                self.byte(T_FIELD);
                self.owner(d);
                self.name(d);
            }
            _ => self.partial(Area::Misc, Site::Decl(k)),
        }
    }

    /// The class or enumeration enclosing `d` as a type item.
    fn owner(&mut self, d: DeclId) {
        let r = self.ast.decls.get(d).ctx().and_then(|c| self.refs.tag(c, &mut self.stats));
        match r {
            Some(h) => {
                self.byte(super::T_TYPE);
                self.out.extend_from_slice(&h.to_le_bytes());
            }
            None => self.byte(T_TU_PLACEHOLDER),
        }
    }

    /// The identifier of `d` (empty for an unnamed one).
    fn name(&mut self, d: DeclId) {
        let ast = self.ast;
        let s = ast.decls.get(d).name().as_ident().map_or("", |s| ast.symbols().as_str(s));
        self.str(s);
    }

    #[allow(
        clippy::too_many_lines,
        clippy::many_single_char_names,
        reason = "one arm per expression kind"
    )]
    pub(super) fn expr(&mut self, e: ExprId) {
        let ast = self.ast;
        let x = &ast.exprs;
        let n = *x.get(e);
        let k = n.kind();
        let id = ExprId::try_new;
        match k {
            // ----- transparent (O-10's structural implicit nodes; the
            // parentheses: the tree already shows the grouping).
            K::ImplicitCastExpr
            | K::ConstantExpr
            | K::MaterializeTemporaryExpr
            | K::CXXBindTemporaryExpr
            | K::ExprWithCleanups
            | K::ParenExpr => self.stack.push(Work::OptExpr(id(n.a()))),
            K::SubstNonTypeTemplateParmExpr => self.stack.push(Work::Expr(x.subst_replacement(e))),
            K::OpaqueValueExpr => match x.opaque_source(e) {
                Some(src) => self.stack.push(Work::Expr(src)),
                None => self.partial(Area::Misc, Site::Expr(k)),
            },

            // ----- area: expr. Literals by value and type, never by
            // spelling; a token from a builtin macro is a placeholder.
            K::IntegerLiteral => {
                if self.in_scratch(n.loc()) {
                    return self.value_placeholder(k, 0);
                }
                self.expr_tag(k);
                self.ty(n.ty());
                if let Some(v) = x.int_value(e) {
                    self.byte(0);
                    self.u64(v);
                } else if let Some(Scalar::Int { bits, .. }) =
                    x.literal_const(e).and_then(|c| ast.consts.scalar(c))
                {
                    self.byte(1);
                    self.count(bits.words().len());
                    for &w in bits.words() {
                        self.u64(w);
                    }
                } else {
                    self.partial(Area::Misc, Site::Item(Item::WideLiteral));
                }
            }
            K::FloatingLiteral => {
                self.expr_tag(k);
                self.ty(n.ty());
                match x.literal_const(e).and_then(|c| ast.consts.scalar(c)) {
                    Some(Scalar::Float { bits, .. }) => {
                        self.out.extend_from_slice(&bits.to_le_bytes());
                    }
                    _ => self.partial(Area::Misc, Site::Item(Item::WideLiteral)),
                }
            }
            K::CharacterLiteral => {
                if self.in_scratch(n.loc()) {
                    return self.value_placeholder(k, 0);
                }
                self.expr_tag(k);
                self.ty(n.ty());
                self.u32(x.char_value(e));
            }
            K::StringLiteral => {
                // A builtin-macro or `#`/`##` piece, or a literal
                // concatenated from pieces (kiln keeps only the first and
                // last piece's location): the whole-literal placeholder
                // (X17; mortar's ruling b).
                let concatenated = x.end_loc(e).is_some_and(|l| l != n.loc());
                if concatenated || self.in_scratch(n.loc()) {
                    return self.value_placeholder(k, 0);
                }
                self.expr_tag(k);
                self.byte(n.aux());
                self.ty(n.ty());
                let (_, bytes) = ast.strings.get(x.string_lit(e));
                self.count(bytes.len());
                self.out.extend_from_slice(bytes);
            }
            K::CXXBoolLiteralExpr => {
                self.expr_tag(k);
                self.byte(u8::from(x.bool_value(e)));
            }
            K::CXXNullPtrLiteralExpr | K::CXXThisExpr => self.expr_tag(k),
            K::GNUNullExpr => {
                self.expr_tag(k);
                self.ty(n.ty());
            }
            // `__func__`-like identifiers and the source-location builtins:
            // one placeholder per kind, never the value.
            K::PredefinedExpr => self.value_placeholder(k, x.predefined_kind(e) as u8),
            K::SourceLocExpr => self.value_placeholder(k, x.source_loc_kind(e) as u8),

            // ----- area: expr. Names.
            K::DeclRefExpr => {
                self.expr_tag(k);
                self.decl_ref(x.decl(e));
            }
            K::MemberExpr => {
                // `.` or `->` as written; whether a qualified name
                // suppresses a virtual call (`p->B::f()`); the member; the
                // object (an implicit `this` hashes as `this`: `m` and
                // `this->m` are equal).
                self.expr_tag(k);
                let d = x.decl(e);
                let qualified = x.decl_ref_extra(e).is_some_and(|r| r.qualifier.is_some());
                let virt = ast.decls.function(d).is_some_and(|f| f.spec.is_virtual());
                self.byte(u8::from(n.has(ExprFlags::ARROW)) | u8::from(qualified && virt) << 1);
                self.decl_ref(d);
                self.stack.push(Work::OptExpr(id(n.a())));
            }

            // ----- area: expr. Operators by operator.
            K::UnaryOperator => {
                self.expr_tag(k);
                self.byte(n.aux());
                self.stack.push(Work::OptExpr(id(n.a())));
            }
            K::BinaryOperator | K::ArraySubscriptExpr => {
                self.expr_tag(k);
                self.byte(if k == K::BinaryOperator { n.aux() } else { 0 });
                self.stack.extend([Work::OptExpr(id(n.a())), Work::OptExpr(id(n.b()))]);
            }
            K::CompoundAssignOperator => {
                self.expr_tag(k);
                self.byte(n.aux());
                self.stack.extend([Work::OptExpr(id(n.a())), Work::Expr(x.compound_assign(e).rhs)]);
            }
            K::ConditionalOperator => {
                self.expr_tag(k);
                let [t, f] = x.cond_arms(e);
                self.stack.extend([Work::OptExpr(id(n.a())), Work::Expr(t), Work::Expr(f)]);
            }
            // Calls: the callee subexpression (it names the resolved
            // function), then the arguments as sema built them.
            K::CallExpr | K::CXXMemberCallExpr | K::CXXOperatorCallExpr => {
                self.expr_tag(k);
                self.byte(if k == K::CXXOperatorCallExpr { n.aux() } else { 0 });
                self.stack.push(Work::OptExpr(id(n.a())));
                self.exprs(ListId::new(n.b()));
            }
            // `a != b` as `!(a == b)` and the like: the written operator,
            // then the semantic form (it holds the resolved call).
            K::CXXRewrittenBinaryOperator => {
                self.expr_tag(k);
                self.byte(x.rewritten_written(e).0 as u8);
                self.stack.push(Work::Expr(x.rewritten_semantic(e)));
            }
            // `sizeof`/`alignof` (and the other unary traits) of a type
            // or of an (unevaluated) expression.
            K::UnaryExprOrTypeTraitExpr => {
                self.expr_tag(k);
                self.byte(n.aux());
                if n.has(ExprFlags::ARG_IS_TYPE) {
                    self.byte(1);
                    match x.trait_type_arg(e) {
                        Some(t) => self.ty(t),
                        None => self.byte(super::T_NONE),
                    }
                } else {
                    self.byte(0);
                    self.stack.push(Work::OptExpr(id(n.a())));
                }
            }
            // Casts written by the user: the cast kind (a C-style cast
            // becomes `static_cast` or `reinterpret_cast` depending on
            // what sema saw), the value category and type, the operand.
            K::CStyleCastExpr
            | K::CXXFunctionalCastExpr
            | K::CXXStaticCastExpr
            | K::CXXConstCastExpr
            | K::CXXReinterpretCastExpr
            | K::CXXDynamicCastExpr
            | K::BuiltinBitCastExpr => {
                self.expr_tag(k);
                self.byte(n.aux());
                self.byte(n.value_category() as u8);
                self.ty(n.ty());
                self.stack.push(Work::OptExpr(id(n.a())));
            }
            K::CXXPseudoDestructorExpr => {
                let (base, arrow, destroyed) = x.pseudo_destructor(e);
                self.expr_tag(k);
                self.byte(u8::from(arrow));
                self.ty(destroyed);
                self.stack.push(Work::Expr(base));
            }

            // ----- area: init (K-3c): initialization and object
            // expressions, new/delete, traits, typeid, throw.
            K::InitListExpr
            | K::ImplicitValueInitExpr
            | K::CXXConstructExpr
            | K::CXXTemporaryObjectExpr
            | K::CXXInheritedCtorInitExpr
            | K::CXXDefaultArgExpr
            | K::CXXDefaultInitExpr
            | K::CXXScalarValueInitExpr
            | K::CXXParenListInitExpr
            | K::ArrayInitLoopExpr
            | K::ArrayInitIndexExpr
            | K::CXXStdInitializerListExpr
            | K::CompoundLiteralExpr
            | K::NoInitExpr
            | K::DesignatedInitUpdateExpr
            | K::CXXNewExpr
            | K::CXXDeleteExpr
            | K::CXXThrowExpr
            | K::CXXNoexceptExpr
            | K::CXXTypeidExpr
            | K::TypeTraitExpr
            | K::ArrayTypeTraitExpr
            | K::ExpressionTraitExpr => self.partial(Area::Init, Site::Expr(k)),

            // ----- area: lambda (K-3d): lambdas, coroutine operators.
            K::LambdaExpr | K::CoawaitExpr | K::CoyieldExpr => {
                self.partial(Area::Lambda, Site::Expr(k));
            }

            // ----- area: misc (K-3d): GNU and builtin forms, reflection,
            // literal operators, constraints, recovery.
            K::UserDefinedLiteral
            | K::ImaginaryLiteral
            | K::EmbedExpr
            | K::BinaryConditionalOperator
            | K::OffsetOfExpr
            | K::SizeOfPackExpr
            | K::RequiresExpr
            | K::ConceptSpecializationExpr
            | K::PackIndexingExpr
            | K::StmtExpr
            | K::AddrLabelExpr
            | K::ChooseExpr
            | K::VAArgExpr
            | K::AtomicExpr
            | K::ShuffleVectorExpr
            | K::ConvertVectorExpr
            | K::ExtVectorElementExpr
            | K::CXXReflectExpr
            | K::CXXSpliceExpr
            | K::PseudoObjectExpr
            | K::GenericSelectionExpr
            | K::RecoveryExpr => self.partial(Area::Misc, Site::Expr(k)),

            // ----- dependent only: a function template's own body is never
            // hashed; these wait for modules (K-6).
            K::UnresolvedLookupExpr
            | K::UnresolvedMemberExpr
            | K::DependentScopeDeclRefExpr
            | K::CXXDependentScopeMemberExpr
            | K::SubstNonTypeTemplateParmPackExpr
            | K::FunctionParmPackExpr
            | K::ParenListExpr
            | K::DesignatedInitExpr
            | K::PackExpansionExpr
            | K::CXXFoldExpr
            | K::CXXUnresolvedConstructExpr
            | K::DependentCoawaitExpr
            | K::CXXExpansionSelectExpr => self.partial(Area::Dependent, Site::Expr(k)),
        }
    }
}
