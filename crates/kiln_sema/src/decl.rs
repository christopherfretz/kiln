//! Declarations ([dcl], [basic.def], spec/sema.md §4): decl-specifiers and
//! declarators to types, variables, functions, fields, typedefs, tags,
//! enumerators, namespaces, linkage specifications, `static_assert` and
//! attributes.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{
    ConstexprKind, DeclFlags, DeclKind, DeclMisc, DefinitionBits, EnumData, FieldData, FnSpec,
    FnStorage, FriendKind, LinkageLang, TagKind, TlsKind, VarSpec, VarStorage,
};
use kiln_ast::expr::{CastKind, ExprKind};
use kiln_ast::ids::RawId;
use kiln_ast::lookup::LookupFlags;
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{
    AutoKeyword, BuiltinType, ExcSpec, FuncInfo, Quals, RefQual, TagKeyword, TypeFlags, TypeView,
};
use kiln_ast::{AttrId, DeclHead, DeclId, ExprId, QualType, StmtId, StringLitId};
use kiln_diag::{DiagId, FixIt};
use kiln_source::symbol::Symbol;
use kiln_source::{Loc, Span};

use crate::class::ClassBuild;
use crate::constfold;
use crate::conv::{self, ConvCtx};
use crate::declspec::{
    Chunk, DeclSite, DeclSpec, Declarator, DeclaratorContext, DefaultArg, ParamInfo, Sign,
    SpecialName, StorageSpec, TypeSpecKind, Width,
};
use crate::init::Initializer;
use crate::kind_conflict::NewKind;
use crate::local_friend::LocalFriend;
use crate::lookup::{LookupKind, is_tag_kind, is_type_kind};
use crate::overload::redecl::NewFn;
use crate::qualified_decl::QualifiedDecl;
use crate::scope::{Ns, Scope, ScopeKind};
use crate::type_diff::DiffArgs as _;
use crate::{Sema, target};
use kiln_ast::templates::is_template_kind;
use kiln_oven::{EvalContext, EvalKind};

/// How a class or enumeration name is used ([dcl.type.elab],
/// [class.pre], [dcl.enum]).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TagUse {
    /// `struct S *p;`: refers to (or forward-declares) `S`.
    Reference,
    /// `struct S;` or `enum E : int;`: declares `S` in the current scope.
    Declaration,
    /// `struct S { ... }`: defines `S`.
    Definition,
    /// `friend class S;` ([dcl.type.elab]/4, [class.friend]/11): refers to
    /// `S`, else declares it in the innermost enclosing namespace without
    /// making it visible (WP-066b; until then reported unsupported and
    /// treated as [`TagUse::Reference`]).
    Friend,
}

/// What a name denotes, for the parser's decisions
/// ([basic.lookup], clang's `Sema::ClassifyName`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NameClass {
    /// A type-name (typedef, class or enumeration) with its type.
    Type(QualType, DeclId),
    /// A namespace.
    Namespace(DeclId),
    /// A variable, function, enumerator, ...
    Value(DeclId),
    /// Nothing was found.
    Undeclared,
    /// A class template, alias template or template template parameter
    /// ([temp.names]/3): `<` starts its template arguments.
    TypeTemplate(DeclId),
    /// A variable template.
    VarTemplate(DeclId),
    /// A function template (one of the declarations found is one).
    FunctionTemplate(DeclId),
    /// A concept ([temp.concept]).
    Concept(DeclId),
    /// An undeclared name the parser treats as a template name
    /// ([temp.names]/3.3: followed by `<` in a function call). Produced
    /// by the parser, never by [`Sema::classify_name`].
    UndeclaredTemplate,
    /// A name after a dependent qualifier ([temp.dep.type]/6): its
    /// meaning is known only at instantiation.
    Dependent,
}

/// An attribute argument.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AttrArg {
    /// An expression (or string literal).
    Expr(ExprId),
    /// A type-id (`alignas(T)`).
    Type(QualType),
    /// The leading identifier of an attribute whose first parameter is
    /// one (`enum_extensibility(open)`; clang `attributeHasIdentifierArg`).
    Ident(Symbol, Loc),
}

/// The state of an enumeration whose body is being processed.
#[derive(Clone, Debug)]
pub struct EnumBuild {
    decl: DeclId,
    /// The fixed underlying type, if any.
    fixed: Option<QualType>,
    /// The value of the previous enumerator.
    last: Option<i128>,
    enumerators: Vec<(DeclId, i128)>,
    /// The type of each enumerator before the closing brace
    /// ([dcl.enum]/5): a reference to an enumerator inside the body has
    /// it.
    pre_types: Vec<(DeclId, QualType)>,
}

/// The parameters declared so far in a parameter-declaration-clause
/// being parsed ([`Sema::act_on_start_prototype`]).
#[derive(Clone, Debug, Default)]
pub struct PrototypeFrame {
    /// The scope depth when the clause opened: its parameters are found
    /// before the scopes below it ([basic.scope.param]).
    pub(crate) depth: usize,
    /// The named parameters, in order.
    pub(crate) params: Vec<(Symbol, DeclId)>,
    /// The number of parameters declared so far.
    pub(crate) index: usize,
}

pub(crate) fn sym_str(s: &Sema<'_, '_>, sym: Symbol) -> String {
    s.ast.symbols().as_str(sym).to_string()
}

impl Sema<'_, '_> {
    // -- types ----------------------------------------------------------------

    /// The type named by the type-specifiers of `ds` ([dcl.type.simple]),
    /// cv-qualified.
    pub fn type_from_declspec(&mut self, ds: &DeclSpec) -> QualType {
        self.type_from_declspec_at(ds, ds.loc)
    }

    /// [`Self::type_from_declspec`], reporting a missing type specifier
    /// at `loc` (clang uses the first declarator's name).
    /// `[unsigned] short|long|long long [int]` ([dcl.type.simple]).
    fn int_declspec_type(width: Width, unsigned: bool) -> QualType {
        match (width, unsigned) {
            (Width::Short, false) => QualType::SHORT,
            (Width::Short, true) => QualType::USHORT,
            (Width::Long, false) => QualType::LONG,
            (Width::Long, true) => QualType::ULONG,
            (Width::LongLong, false) => QualType::LONGLONG,
            (Width::LongLong, true) => QualType::ULONGLONG,
            (Width::None, false) => QualType::INT,
            (Width::None, true) => QualType::UINT,
        }
    }

    pub fn type_from_declspec_at(&mut self, ds: &DeclSpec, loc: Loc) -> QualType {
        let unsigned = ds.sign == Sign::Unsigned;
        let base = match ds.base {
            TypeSpecKind::None
                if ds.complex.is_some() && ds.width == Width::None && ds.sign == Sign::None =>
            {
                // clang `DeclSpec::Finish`: `ext_plain_complex`.
                self.error(DiagId::sema_plain_complex, ds.complex.unwrap_or(loc));
                QualType::DOUBLE
            }
            TypeSpecKind::None if !ds.has_type_specifier() => {
                if !ds.invalid {
                    self.error(DiagId::parse_missing_type_specifier, loc);
                }
                QualType::INT
            }
            TypeSpecKind::None | TypeSpecKind::Int => Self::int_declspec_type(ds.width, unsigned),
            TypeSpecKind::Void => QualType::VOID,
            TypeSpecKind::Bool => QualType::BOOL,
            TypeSpecKind::Char => match ds.sign {
                Sign::None => QualType::CHAR,
                Sign::Signed => QualType::SCHAR,
                Sign::Unsigned => QualType::UCHAR,
            },
            TypeSpecKind::WChar => QualType::WCHAR,
            TypeSpecKind::Char8 => QualType::CHAR8,
            TypeSpecKind::Char16 => QualType::CHAR16,
            TypeSpecKind::Char32 => QualType::CHAR32,
            TypeSpecKind::Int128 => {
                if unsigned {
                    QualType::UINT128
                } else {
                    QualType::INT128
                }
            }
            TypeSpecKind::Float => QualType::FLOAT,
            TypeSpecKind::Double => {
                if ds.width == Width::Long {
                    QualType::LONGDOUBLE
                } else {
                    QualType::DOUBLE
                }
            }
            TypeSpecKind::Float128 => QualType::FLOAT128,
            TypeSpecKind::Float16 => QualType::FLOAT16,
            TypeSpecKind::Float32 => QualType::FLOAT32,
            TypeSpecKind::Float64 => QualType::FLOAT64,
            TypeSpecKind::StdFloat128 => QualType::STD_FLOAT128,
            TypeSpecKind::Float32x => QualType::FLOAT32X,
            TypeSpecKind::Float64x => QualType::FLOAT64X,
            TypeSpecKind::BFloat16 => QualType::BFLOAT16,
            TypeSpecKind::Fp16 => QualType::HALF,
            // [dcl.type.auto.deduct]/4 (clang
            // `err_decltype_auto_cannot_be_combined`).
            TypeSpecKind::DecltypeAuto if !ds.cv.is_empty() => {
                self.error(
                    DiagId::sema_decltype_auto_cannot_be_combined,
                    ds.type_loc.unwrap_or(loc),
                );
                return QualType::ERROR;
            }
            TypeSpecKind::Auto | TypeSpecKind::DecltypeAuto => {
                let kw = if ds.base == TypeSpecKind::Auto {
                    AutoKeyword::Auto
                } else {
                    AutoKeyword::DecltypeAuto
                };
                let (c, args) = match &ds.auto_constraint {
                    Some((c, a)) if a.is_empty() => (Some(*c), None),
                    Some((c, a)) => (Some(*c), Some(self.ast.types.arg_list(a))),
                    None => (None, None),
                };
                self.ast.types.auto_type(kw, None, c, args, false)
            }
            TypeSpecKind::Named => ds.named.unwrap_or(QualType::ERROR),
            // [dcl.type.simple] (C23 6.7.2, a clang extension in C++).
            TypeSpecKind::BitInt => match ds.bit_int_width {
                Some((w, _)) => self.build_bit_int_type(unsigned, w, ds.loc),
                None => QualType::ERROR,
            },
        };
        let base = match ds.complex {
            Some(cloc) => self.complex_of_declspec(ds, base, cloc),
            None => base,
        };
        // clang `BuildQualifiedType`: the `_Atomic` qualifier makes the
        // unqualified type atomic and keeps its qualifiers outside.
        let base = match ds.atomic_qual {
            Some(aloc) if self.ast.types.as_atomic(base).is_none() => {
                let q = self.ast.types.effective_quals(base);
                let un = if q.is_empty() { base } else { self.ast.types.unqualified(base) };
                match self.act_on_atomic_type(un, aloc) {
                    Some(t) if q.is_empty() => t,
                    Some(t) => self.ast.types.qualify(t, q),
                    None => QualType::ERROR,
                }
            }
            _ => base,
        };
        let cv = self.declspec_quals(ds, base);
        let ty = if cv.is_empty() { base } else { self.ast.types.qualify(base, cv) };
        if ds.nullability.is_empty() { ty } else { self.declspec_nullability(ds, ty) }
    }

    /// The cv-qualifiers of `ds` for the type `base`. C99 6.7.3p2 (clang
    /// `BuildQualifiedType`, `err_typecheck_invalid_restrict_not_pointer`):
    /// only a pointer or reference may be restrict-qualified; the
    /// qualifier is dropped (`__restrict int h();`, WP-371).
    fn declspec_quals(&mut self, ds: &DeclSpec, base: QualType) -> Quals {
        let cv = ds.cv;
        self.declspec_restrict_error = None;
        if let Some(rloc) = ds.cv_locs[2]
            && cv.contains(Quals::RESTRICT)
            && !self.restrict_ok(base)
        {
            let msg = self.diag(DiagId::sema_typecheck_invalid_restrict_not_pointer, rloc);
            let msg = msg.arg(self.ty_arg(base));
            self.emit(msg);
            self.declspec_restrict_error = Some((rloc, base));
            return cv - Quals::RESTRICT;
        }
        cv
    }

    /// The `__restrict` error the decl-specifier-seq whose type was formed
    /// last reported, if any: the parser repeats it
    /// ([`Sema::report_restrict_not_pointer`]) for each later declarator
    /// of the group (`__restrict int a, b;` is reported twice, as clang
    /// does; WP-379).
    pub fn take_declspec_restrict_error(&mut self) -> Option<(Loc, QualType)> {
        self.declspec_restrict_error.take()
    }

    /// "restrict requires a pointer or reference ('T' is invalid)" again
    /// at the `__restrict` of a decl-specifier-seq of type `T`, for a later
    /// declarator of its group (not deduplicated).
    pub fn report_restrict_not_pointer(&mut self, (rloc, base): (Loc, QualType)) {
        let msg = self.diag(DiagId::sema_typecheck_invalid_restrict_not_pointer, rloc);
        let msg = msg.arg(self.ty_arg(base)).allow_duplicate();
        self.emit(msg);
    }

    /// True if `restrict` may qualify `ty`: a pointer, reference or
    /// member pointer, or a type not known yet (dependent, a placeholder,
    /// an error).
    fn restrict_ok(&self, ty: QualType) -> bool {
        self.is_error_type(ty)
            || self.is_dependent_type(ty)
            || self.is_undeduced_auto(ty)
            || self.ast.types.is_pointer(ty)
            || self.ast.types.is_reference(ty)
            || self.ast.types.canonical_kind(ty) == kiln_ast::types::TypeKind::MemberPointer
    }

    /// `_Complex T` for the type `base` named by the other specifiers of
    /// `ds` (clang `DeclSpec::Finish`: a real floating or integer type).
    fn complex_of_declspec(&mut self, ds: &DeclSpec, base: QualType, cloc: Loc) -> QualType {
        let ok = matches!(
            ds.base,
            TypeSpecKind::None
                | TypeSpecKind::Int
                | TypeSpecKind::Char
                | TypeSpecKind::Float
                | TypeSpecKind::Double
                | TypeSpecKind::Float128
                | TypeSpecKind::Float16
                | TypeSpecKind::Float32
                | TypeSpecKind::Float64
                | TypeSpecKind::StdFloat128
                | TypeSpecKind::Float32x
                | TypeSpecKind::Float64x
        );
        // WP-365: GCC's `_Complex __bf16`, spelled directly or through a
        // type-name (libstdc++ `<complex>`'s `__complex__ decltype(0.0bf16)`
        // under `__STDCPP_BFLOAT16_T__`, which kiln predefines as GCC
        // does); clang 19 rejects both.
        let ok = ok
            || (matches!(ds.base, TypeSpecKind::BFloat16 | TypeSpecKind::Named)
                && self.ast.types.builtin(self.ast.types.canonical(base))
                    == Some(BuiltinType::BFloat16));
        if !ok {
            if !ds.invalid && !self.is_error_type(base) {
                // clang `DeclSpec::getSpecifierName(TST_typename)`.
                let name = match ds.base {
                    TypeSpecKind::Named => "type-name",
                    TypeSpecKind::Fp16 => "half",
                    b => b.spelling(),
                };
                let msg = self.diag(DiagId::sema_invalid_complex_spec, cloc).arg(name);
                self.emit(msg);
            }
            // clang drops the `_Complex` (`TSC_unspecified`) and goes on
            // with the base type.
            return base;
        }
        self.ast.types.complex(base)
    }

    /// True if `q` is the error type.
    pub fn is_error_type(&self, q: QualType) -> bool {
        self.ast.types.is_error(q)
    }

    /// True if `q` is an undeduced `auto`.
    pub fn is_undeduced_auto(&self, q: QualType) -> bool {
        matches!(self.ast.types.view(self.ast.types.canonical(q).ty()), TypeView::Auto { .. })
    }

    /// The base element type of `q` if it is an array of known bound,
    /// else `q` (clang's `getBaseElementType` in the variable and field
    /// completeness checks).
    pub(crate) fn bounded_array_element(&self, q: QualType) -> QualType {
        if matches!(self.ast.types.canonical_view(q), TypeView::ConstantArray(_)) {
            self.base_element(q)
        } else {
            q
        }
    }

    /// The type a non-static data member of type `q` requires complete
    /// (clang `CheckFieldDecl`): the element type of a bounded array, and
    /// of an array of unknown bound, which is a flexible array member
    /// (spec/sema.md §5.8, checked when the class is complete).
    pub(crate) fn field_complete_type(&self, q: QualType) -> QualType {
        match self.ast.types.canonical_view(q) {
            TypeView::IncompleteArray(e) => self.bounded_array_element(e),
            _ => self.bounded_array_element(q),
        }
    }

    /// True if `q` is a complete object type here (a class is incomplete
    /// until its closing brace).
    pub fn is_complete(&self, q: QualType) -> bool {
        // A dependent type is checked when instantiated ([temp.res.general]/6).
        if self.is_dependent_type(q) {
            return true;
        }
        if let Some(r) = self.ast.types.as_record(q) {
            let f = self.ast.first(r);
            if self.defining.iter().any(|&d| self.ast.first(d) == f) {
                return false;
            }
        }
        if let Some(elem) = self.ast.types.element(q) {
            // A variable length array is complete (GNU extension).
            let vla = matches!(self.ast.types.canonical_view(q), TypeView::VariableArray { .. });
            if self.ast.types.is_array(q) && self.ast.types.array_size(q).is_none() && !vla {
                return false;
            }
            return self.is_complete(elem);
        }
        target::is_complete_object(&self.ast, q)
    }

    /// An operator-function-id (literal-operator-id) declarator written
    /// with the `SpecialName::Operator` (`LiteralOperator`) form,
    /// rewritten into the `op_name` (`lit_suffix`) form every operator
    /// function other than a member `operator=` uses (`name` holds
    /// `operator`). `None` for any other declarator.
    pub(crate) fn operator_declarator(&self, d: &Declarator) -> Option<Declarator> {
        let mut c = d.clone();
        match d.special {
            Some((SpecialName::Operator(op), loc)) => {
                c.op_name = Some(op);
                c.name = Some((self.intern("operator"), loc));
            }
            Some((SpecialName::LiteralOperator(s), loc)) => {
                c.lit_suffix = Some(s);
                c.name = Some((self.intern("operator"), loc));
            }
            _ => return None,
        }
        c.special = None;
        Some(c)
    }

    fn declarator_name(&self, d: &Declarator) -> String {
        if let Some((sp, _)) = d.special {
            return match sp {
                SpecialName::Constructor(n) => sym_str(self, n),
                SpecialName::Destructor(n) => format!("~{}", sym_str(self, n)),
                SpecialName::Operator(op) => format!("operator{}", op.spelling()),
                SpecialName::LiteralOperator(x) => format!("operator\"\"{}", sym_str(self, x)),
                SpecialName::Conversion(t) => format!("operator {}", self.ty_arg(t)),
                SpecialName::DeductionGuide(t) => self.decl_name_str(self.ast.decls.get(t).name()),
            };
        }
        d.name.map_or_else(|| "type name".to_owned(), |(n, _)| sym_str(self, n))
    }

    /// The `(` of a parameter-declaration-clause. Until the matching
    /// [`Sema::act_on_end_prototype`] (after the trailing return type),
    /// each parameter declared ([`Sema::act_on_prototype_param`]) is in
    /// scope ([basic.scope.param]): unqualified lookup finds it before
    /// the scope the declarator appears in ([`Sema::lookup_name`]).
    pub fn act_on_start_prototype(&mut self) {
        self.prototype_params.push(PrototypeFrame {
            depth: self.scopes.len(),
            params: Vec::new(),
            index: 0,
        });
    }

    /// Parameter `p` of the innermost parameter-declaration-clause, at its
    /// point of declaration (before its default argument,
    /// [basic.scope.pdecl]/1): creates its declaration (`p.decl`), which
    /// the function adopts ([`Sema::create_params`]).
    pub fn act_on_prototype_param(&mut self, p: &mut ParamInfo) {
        let Some(frame) = self.prototype_params.last() else { return };
        let idx = u16::try_from(frame.index).unwrap_or(u16::MAX);
        let name = p.name.map_or(DeclName::EMPTY, |(n, _)| DeclName::ident(n));
        let pack = matches!(self.ast.types.view(p.ty.ty()), TypeView::PackExpansion { .. });
        let mut flags = if pack { kiln_ast::decl::ParamData::PACK } else { 0 };
        if p.explicit_object.is_some() {
            flags |= kiln_ast::decl::ParamData::EXPLICIT_OBJECT;
        }
        // The prototype scopes enclosing this one (`ParamData::scope_depth`).
        let depth = self.prototype_params.len().saturating_sub(1) + self.requires_param_scopes;
        flags |= kiln_ast::decl::ParamData::scope_depth_flags(depth);
        let ctx = self.ctx;
        let pd = self.builder().prototype_param(ctx, name, p.loc, p.ty, idx, flags);
        if p.invalid {
            self.ast.decls.set_flags(pd, DeclFlags::INVALID);
        }
        self.unowned_params.insert(pd);
        p.decl = Some(pd);
        if let Some(frame) = self.prototype_params.last_mut() {
            frame.index += 1;
            if let Some((n, _)) = p.name {
                frame.params.push((n, pd));
            }
        }
    }

    /// The innermost parameter named `name` of the
    /// parameter-declaration-clauses opened at scope depth `depth`
    /// ([basic.scope.param]).
    pub(crate) fn prototype_param_named(&self, depth: usize, name: Symbol) -> Option<DeclId> {
        self.prototype_params
            .iter()
            .rev()
            .filter(|f| f.depth == depth)
            .find_map(|f| f.params.iter().rev().find(|e| e.0 == name).map(|e| e.1))
    }

    /// The exception specification or trailing return type of a function
    /// declarator with cv-qualifiers `quals`, in a member-declaration
    /// when `member`: there `this` may appear, with type "pointer to cv
    /// `C`" ([expr.prim.this]/2; clang's `CXXThisScopeRAII`). Returns
    /// whether a `this` scope was entered, for
    /// [`Sema::act_on_end_this_scope`].
    pub fn act_on_start_this_scope(&mut self, member: bool, quals: Quals) -> bool {
        let class = if member && self.kind_of(self.ctx) == DeclKind::CXXRecord {
            self.ctx
        } else if let Some(&(c, depth, _)) = self.declarator_records.last()
            && depth == self.scopes.len()
            && self.kind_of(c) == DeclKind::CXXRecord
        {
            // After the declarator-id of an out-of-line member
            // (`auto S::f() -> decltype(this->v)`).
            c
        } else {
            return false;
        };
        let t = self.this_class_type(class);
        let t = if quals.is_empty() { t } else { self.ast.types.qualify(t, quals) };
        let p = self.ast.types.pointer_to(t);
        self.nsdmi_this.push((class, p));
        true
    }

    /// Leaves the scope [`Sema::act_on_start_this_scope`] entered.
    pub fn act_on_end_this_scope(&mut self, entered: bool) {
        if entered {
            self.nsdmi_this.pop();
        }
    }

    /// The end of the declarator part a parameter-declaration-clause
    /// scopes (see [`Sema::act_on_start_prototype`]).
    pub fn act_on_end_prototype(&mut self) {
        self.prototype_params.pop();
    }

    /// After a qualified declarator-id `N::name` (clang's
    /// `ActOnCXXEnterDeclaratorScope`): the rest of the declarator, its
    /// parameters, array bounds and trailing return type, is looked up in
    /// the scope of `N` and the contexts between it and the current one
    /// ([basic.lookup.unqual]/1, [dcl.meaning]). Returns what
    /// [`Sema::act_on_exit_declarator_scope`] leaves.
    pub fn act_on_enter_declarator_scope(&mut self, qual: DeclId) -> usize {
        if !matches!(self.kind_of(qual), DeclKind::CXXRecord | DeclKind::Namespace) {
            return 0;
        }
        let n = self.push_scopes_between(qual, self.ctx);
        if n > 0 {
            // The current context is the qualifier's
            // (clang's `EnterDeclaratorContext`): `X<P...>::` names the
            // current instantiation there ([temp.dep.type]/1).
            let c = self.ast.definition(qual).unwrap_or(qual);
            self.declarator_records.push((c, self.scopes.len(), self.ctx));
            self.ctx = c;
        }
        n
    }

    /// The type of the decl-specifiers `base` of a declaration whose
    /// declarator-id is qualified by `d.qualifier` (clang's
    /// `RebuildDeclaratorInCurrentInstantiation`): a `typename X<P...>::m`
    /// written before the declarator-id of a member of the class template
    /// `X` names the member of the current instantiation
    /// ([temp.dep.type]/6), as it does inside the class.
    pub fn rebuild_in_current_instantiation(&mut self, base: QualType, d: &Declarator) -> QualType {
        let Some(qual) = d.qualifier else { return base };
        if self.kind_of(qual) != DeclKind::CXXRecord {
            return base;
        }
        self.rebuild_ci_type(base, qual, d.loc, 0)
    }

    /// [`Sema::rebuild_ci_type`] through an elaborated type and through
    /// `*`, `&` and `&&` (`pair<typename X<T>::ptr&, bool>`); `None` when
    /// `view` is none of those.
    fn rebuild_ci_derived(
        &mut self,
        base: QualType,
        view: TypeView,
        qual: DeclId,
        loc: Loc,
        depth: u32,
    ) -> Option<QualType> {
        if let TypeView::Elaborated { keyword, nns, named } = view {
            let r = self.rebuild_ci_type(named, qual, loc, depth + 1);
            if r == named {
                return Some(base);
            }
            let t = self.ast.types.elaborated(keyword, nns, r);
            return Some(self.ast.types.qualify(t, base.quals()));
        }
        let (TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p)) = view else {
            return None;
        };
        let r = self.rebuild_ci_type(p, qual, loc, depth + 1);
        if r == p {
            return Some(base);
        }
        let t = match view {
            TypeView::Pointer(_) => self.ast.types.pointer_to(r),
            TypeView::LValueRef(_) => self.ast.types.lvalue_ref_to(r),
            _ => self.ast.types.rvalue_ref_to(r),
        };
        Some(self.ast.types.qualify(t, base.quals()))
    }

    /// [`Sema::rebuild_in_current_instantiation`] of `base`, also through
    /// the type arguments of a template-id (libc++'s
    /// `pair<typename __hash_table<...>::iterator, bool>
    /// __hash_table<...>::__node_insert_unique(...)`).
    #[allow(
        clippy::too_many_lines,
        reason = "one branch per dependent type form: template-id, dependent template-id, dependent name"
    )]
    fn rebuild_ci_type(&mut self, base: QualType, qual: DeclId, loc: Loc, depth: u32) -> QualType {
        if depth > 32 || !self.is_dependent_type(base) {
            return base;
        }
        let view = self.ast.types.view(base.ty());
        if let Some(t) = self.rebuild_ci_derived(base, view, qual, loc, depth) {
            return t;
        }
        if let TypeView::TemplateSpecialization { name, args, alias, .. } = view {
            let list = self.ast.types.args(args).to_vec();
            let mut out = Vec::with_capacity(list.len());
            let mut changed = false;
            for a in list {
                if let kiln_ast::types::ArgView::Type(t) = a.view() {
                    let r = self.rebuild_ci_type(t, qual, loc, depth + 1);
                    changed |= r != t;
                    out.push(kiln_ast::types::TemplateArg::ty(r));
                } else {
                    out.push(a);
                }
            }
            if !changed {
                // `DequeIterator<V, 1>` before the declarator-id
                // `DequeIterator<V, 1>::operator+` of a member of that partial
                // specialization: its current instantiation.
                if !alias && let Some(tmpl) = self.ast.types.template_decl(name) {
                    let entered = self.act_on_enter_declarator_scope(qual);
                    let c = self.partial_current_instantiation_canon(tmpl, base, loc);
                    self.act_on_exit_declarator_scope(entered);
                    if let Some(c) = c {
                        let t = self.ast.types.template_specialization(name, args, Some(c), false);
                        return self.ast.types.qualify(t, base.quals());
                    }
                }
                return base;
            }
            if alias {
                // An alias template specialization is substituted again
                // with the rebuilt arguments
                // (`remove_extent_t<typename X<T>::elem>`, [temp.alias]/2).
                let Ok(crate::templ::actions::TemplateIdRef::Type(t)) =
                    self.act_template_id(name, &out, loc, loc)
                else {
                    return base;
                };
                return self.ast.types.qualify(t, base.quals());
            }
            let l = self.ast.types.arg_list(&out);
            // Once the rebuilt arguments no longer depend on the
            // template's parameters (`pair<typename X<T>::ptr, int>` with a
            // non-dependent member `ptr`), this is the class template
            // specialization the in-class declaration named
            // ([temp.over.link]/5): formed from the canonical arguments
            // (the written `typename X<T>::` qualifier keeps the sugar
            // instantiation-dependent), kept as the written sugar over it.
            let canon: Vec<_> = out
                .iter()
                .map(|&a| match a.view() {
                    kiln_ast::types::ArgView::Type(t) => {
                        kiln_ast::types::TemplateArg::ty(self.ast.types.canonical(t))
                    }
                    _ => a,
                })
                .collect();
            let cl = self.ast.types.arg_list(&canon);
            let still_dependent =
                self.ast.types.arg_list_flags(cl).intersects(TypeFlags::DEPENDENT_INSTANTIATION);
            let args = if still_dependent { &out } else { &canon };
            let Ok(crate::templ::actions::TemplateIdRef::Type(t)) =
                self.act_template_id(name, args, loc, loc)
            else {
                return base;
            };
            let t = if still_dependent {
                t
            } else {
                let under = self.ast.types.canonical(t);
                self.ast.types.template_specialization(name, l, Some(under), false)
            };
            return self.ast.types.qualify(t, base.quals());
        }
        if let TypeView::DependentTemplateSpecialization { keyword, nns, .. } = view {
            // `typename X<P...>::template M<A...>` itself.
            let Some(t) = self.rebuild_ci_member_template_id(base, qual, loc) else {
                return base;
            };
            let t = self.ast.types.elaborated(keyword, Some(nns), t);
            return self.ast.types.qualify(t, base.quals());
        }
        let TypeView::DependentName { keyword, nns, name } = view else {
            return base;
        };
        // `typename X<P...>::template M<A...>::m`: the qualifier names the
        // member template `M` of the current instantiation (libstdc++
        // `valarray<_Tp>::template _UnaryOp<_Name>::_Rt`).
        if let kiln_ast::types::Nns::Type { prefix, ty, template_kw } = self.ast.types.nns_view(nns)
            && let Some(r) = self.rebuild_ci_member_template_id(ty, qual, loc)
        {
            let n = self.ast.types.nns(kiln_ast::types::Nns::Type { prefix, ty: r, template_kw });
            let t = self.ast.types.dependent_name(keyword, n, name);
            return self.ast.types.qualify(t, base.quals());
        }
        let Some(pat) = self.rebuild_ci_nns_record(nns, qual, loc, depth) else {
            // The qualifier is no class of the current instantiation, but
            // may still be rebuilt in it: a member typedef
            // (`typename list<V, A>::AllocTraits::size_type`) or a
            // template-id over its members
            // (`typename add_lvalue_reference<typename shared_ptr<E>::element_type>::type`).
            let Some(n) = self.rebuild_ci_nns(nns, qual, loc, depth) else { return base };
            let t = self.ast.types.dependent_name(keyword, n, name);
            return self.ast.types.qualify(t, base.quals());
        };
        let found = self.lookup_qualified(pat, name, LookupKind::Ordinary);
        let Some(&m) = found.decls.first() else { return base };
        if !matches!(
            self.kind_of(m),
            DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::CXXRecord | DeclKind::Enum
        ) {
            return base;
        }
        let t = match self.kind_of(m) {
            DeclKind::Typedef | DeclKind::TypeAlias => {
                self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR)
            }
            _ => self.tag_type(m),
        };
        let t = self.ast.types.elaborated(keyword, Some(nns), t);
        self.ast.types.qualify(t, base.quals())
    }

    /// Nested-name-specifier `nns` (not naming a class of the current
    /// instantiation, [`Sema::rebuild_ci_nns_record`]) rebuilt in the
    /// current instantiation: a type component through
    /// [`Sema::rebuild_ci_type`], or `X<P...>::td` with `td` a member
    /// typedef as the type `td` aliases. `None` when nothing changes.
    fn rebuild_ci_nns(
        &mut self,
        nns: kiln_ast::ids::NnsId,
        qual: DeclId,
        loc: Loc,
        depth: u32,
    ) -> Option<kiln_ast::ids::NnsId> {
        if depth > 32 {
            return None;
        }
        match self.ast.types.nns_view(nns) {
            kiln_ast::types::Nns::Type { prefix, ty, template_kw } => {
                let r = self.rebuild_ci_type(ty, qual, loc, depth + 1);
                (r != ty).then(|| {
                    self.ast.types.nns(kiln_ast::types::Nns::Type { prefix, ty: r, template_kw })
                })
            }
            kiln_ast::types::Nns::Identifier { prefix: Some(prefix), name } => {
                let outer = self.rebuild_ci_nns_record(prefix, qual, loc, depth + 1)?;
                let found = self.lookup_qualified(outer, name, LookupKind::Ordinary);
                let &[m] = found.decls.as_slice() else { return None };
                if !matches!(self.kind_of(m), DeclKind::Typedef | DeclKind::TypeAlias) {
                    return None;
                }
                let ty = self.ast.decls.get(m).ty()?;
                Some(self.ast.types.nns(kiln_ast::types::Nns::Type {
                    prefix: None,
                    ty,
                    template_kw: false,
                }))
            }
            _ => None,
        }
    }

    /// The class of the current instantiation that nested-name-specifier
    /// `nns`, written before the declarator-id of a member of the class
    /// template `X`, names ([temp.dep.type]/1): `X<P...>::` (the primary
    /// template's own parameters or a partial specialization's
    /// arguments) and, through it, a member class
    /// (`typename A::B<P>::C::`, [temp.dep.type]/6).
    fn rebuild_ci_nns_record(
        &mut self,
        nns: kiln_ast::ids::NnsId,
        qual: DeclId,
        loc: Loc,
        depth: u32,
    ) -> Option<DeclId> {
        if depth > 32 {
            return None;
        }
        match self.ast.types.nns_view(nns) {
            kiln_ast::types::Nns::Type { ty, .. } => {
                let TypeView::TemplateSpecialization { name: tn, .. } =
                    self.ast.types.view(ty.ty())
                else {
                    return None;
                };
                let tmpl = self.ast.types.template_decl(tn)?;
                let entered = self.act_on_enter_declarator_scope(qual);
                let pat = self.current_instantiation_at(tmpl, ty, loc);
                self.act_on_exit_declarator_scope(entered);
                pat
            }
            kiln_ast::types::Nns::Identifier { prefix: Some(prefix), name } => {
                let outer = self.rebuild_ci_nns_record(prefix, qual, loc, depth + 1)?;
                let found = self.lookup_qualified(outer, name, LookupKind::Ordinary);
                let &[m] = found.decls.as_slice() else { return None };
                (self.kind_of(m) == DeclKind::CXXRecord)
                    .then(|| self.ast.definition(m).unwrap_or(m))
            }
            _ => None,
        }
    }

    /// `X<P...>::template M<A...>` written before the declarator-id of a
    /// member of class template `X`, rebuilt as the specialization of the
    /// member template `M` of the current instantiation, as `M<A...>`
    /// names it inside the class ([temp.dep.type]/6). `None` when `ty` is
    /// not that form or `M` is not a member class or alias template.
    fn rebuild_ci_member_template_id(
        &mut self,
        ty: QualType,
        qual: DeclId,
        loc: Loc,
    ) -> Option<QualType> {
        let TypeView::DependentTemplateSpecialization { nns, name, args, .. } =
            self.ast.types.view(ty.ty())
        else {
            return None;
        };
        let kiln_ast::types::Nns::Type { ty: cty, .. } = self.ast.types.nns_view(nns) else {
            return None;
        };
        let TypeView::TemplateSpecialization { name: tn, .. } = self.ast.types.view(cty.ty())
        else {
            return None;
        };
        let tmpl = self.ast.types.template_decl(tn)?;
        let entered = self.act_on_enter_declarator_scope(qual);
        let pat = self.current_instantiation_at(tmpl, cty, loc);
        self.act_on_exit_declarator_scope(entered);
        let found = self.lookup_qualified(pat?, name, LookupKind::Ordinary);
        let &[m] = found.decls.as_slice() else { return None };
        if !matches!(self.kind_of(m), DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate) {
            return None;
        }
        let tn = self.template_name_for(m);
        let list = self.ast.types.args(args).to_vec();
        let Ok(crate::templ::actions::TemplateIdRef::Type(t)) =
            self.act_template_id(tn, &list, loc, loc)
        else {
            return None;
        };
        Some(self.ast.types.qualify(t, ty.quals()))
    }

    /// Before the initializer of `v`, declared with a qualified
    /// declarator-id (clang's `ActOnCXXEnterDeclInitializer`): the scopes
    /// of its context, as for the rest of its declarator. Returns what
    /// [`Sema::act_on_exit_declarator_scope`] leaves.
    pub fn act_on_enter_decl_initializer(&mut self, v: DeclId) -> usize {
        match self.ast.decls.get(v).ctx() {
            Some(c) if c != self.ctx => self.act_on_enter_declarator_scope(c),
            _ => 0,
        }
    }

    /// Leaves the scopes [`Sema::act_on_enter_declarator_scope`] entered.
    pub fn act_on_exit_declarator_scope(&mut self, entered: usize) {
        if entered == 0 {
            return;
        }
        if let Some(&(_, d, ctx)) = self.declarator_records.last()
            && d == self.scopes.len()
        {
            self.declarator_records.pop();
            self.ctx = ctx;
        }
        for _ in 0..entered {
            self.scopes.pop();
        }
    }

    /// The trailing requires-clause of function declarator `d`: its
    /// parameters are in scope again ([basic.scope.param]/1), and in a
    /// member-declaration so is `this` ([expr.prim.this]/2); after a
    /// qualified declarator-id, so is the scope of its qualifier. Returns
    /// what [`Sema::act_on_end_trailing_requires`] leaves.
    pub fn act_on_start_trailing_requires(&mut self, d: &Declarator) -> (usize, bool) {
        let Some(Chunk::Function { params, quals, .. }) = d.chunks.first() else {
            return (0, false);
        };
        let quals = *quals;
        let scopes = d.qualifier.map_or(0, |q| self.act_on_enter_declarator_scope(q));
        let mut frame =
            PrototypeFrame { depth: self.scopes.len(), params: Vec::new(), index: params.len() };
        for p in params {
            if let (Some((n, _)), Some(pd)) = (p.name, p.decl) {
                frame.params.push((n, pd));
            }
        }
        self.prototype_params.push(frame);
        // A friend is no member: no `this` (clang's
        // `InitCXXThisScopeForDeclaratorIfRelevant`).
        (
            scopes,
            !d.friend && self.act_on_start_this_scope(d.ctx == DeclaratorContext::Member, quals),
        )
    }

    /// Leaves the scopes [`Sema::act_on_start_trailing_requires`] entered.
    pub fn act_on_end_trailing_requires(&mut self, (scopes, this): (usize, bool)) {
        self.act_on_end_this_scope(this);
        self.prototype_params.pop();
        self.act_on_exit_declarator_scope(scopes);
    }

    /// Normalizes a parameter list ([dcl.fct]/2): a lone unnamed `void`
    /// parameter means "no parameters"; any other `void` parameter is an
    /// error.
    pub fn act_on_param_list(&mut self, params: Vec<ParamInfo>) -> Vec<ParamInfo> {
        let is_void = |s: &Self, p: &ParamInfo| s.ast.types.is_void(p.ty);
        if params.len() == 1 && params[0].name.is_none() && is_void(self, &params[0]) {
            if !self.ast.types.effective_quals(params[0].ty).is_empty() {
                self.error(DiagId::sema_param_with_void_type, params[0].loc);
            }
            // [dcl.attr.annotation]/1: not on a `void` parameter.
            self.reject_annotations_in(&params[0].attrs, Some("a parameter of type 'void'"));
            return Vec::new();
        }
        let mut out = Vec::with_capacity(params.len());
        for mut p in params {
            if is_void(self, &p) {
                if p.name.is_none() {
                    self.error(DiagId::sema_void_only_param, p.loc);
                } else {
                    self.error(DiagId::sema_param_with_void_type, p.name.map_or(p.loc, |n| n.1));
                }
                p.ty = QualType::ERROR;
                p.invalid = true;
            }
            out.push(p);
        }
        out
    }

    /// `ty` wrapped in `ParenType` sugar for each pair of grouping
    /// parentheses of `d` around exactly `inside` chunks (clang's
    /// `DeclaratorChunk::Paren`, WP-191). Parentheses around no chunk (the
    /// outermost type, `int (x)`) and around a function or array type
    /// (`void (*f)(int)`, `int (*p)[3]`) get no node: clang's printer
    /// shows neither, so the sugar would only cost a node.
    fn declarator_parens(&mut self, mut ty: QualType, d: &Declarator, inside: usize) -> QualType {
        if inside == 0 {
            return ty;
        }
        for &(n, _, _) in &d.parens {
            if n != inside {
                continue;
            }
            let k = self.ast.types.kind(ty.ty());
            if k == kiln_ast::types::TypeKind::Function
                || k.is_array()
                || k == kiln_ast::types::TypeKind::QualifiedArray
                || self.is_error_type(ty)
            {
                return ty;
            }
            ty = self.ast.types.paren_type(ty);
        }
        ty
    }

    /// [dcl.fct]/6 (CWG1417): a function type with a cv-qualifier-seq or
    /// ref-qualifier that `d` (whose decl-specifiers are `ds`) declares
    /// through a typedef-name or `decltype` (no function declarator) is
    /// only the type of a non-static member function, of a typedef or
    /// alias-declaration, of a template type argument or of a type
    /// parameter's default; the operand of `^^` declares nothing, so it
    /// may name one ([expr.reflect]; GCC 16.1). Anything else (a non-member function at
    /// namespace or block scope, a static member function, a friend, a
    /// parameter, a type-id) is clang's `err_invalid_qualified_function_type`
    /// in its "of type" form, at the start of the declaration, and the
    /// qualifiers are dropped (clang `GetFullTypeForDeclarator`). A
    /// function declarator's own qualifiers are `act_on_function`'s; a
    /// pointer or reference to such a type is `check_qualified_function`'s.
    pub fn check_qualified_fn_type(
        &mut self,
        ty: QualType,
        ds: &DeclSpec,
        d: &Declarator,
    ) -> QualType {
        if ds.is_typedef()
            || matches!(
                d.site,
                DeclSite::Alias
                    | DeclSite::TemplateArg
                    | DeclSite::TypeParamDefault
                    | DeclSite::Subst
                    | DeclSite::Reflect
            )
            || d.chunks.iter().any(|c| matches!(c, Chunk::Function { .. }))
        {
            return ty;
        }
        let Some(p) = self.ast.types.proto(ty).copied() else { return ty };
        let info = p.info();
        let (q, r) = (info.this_quals(), info.ref_qual());
        if q.is_empty() && r == RefQual::None {
            return ty;
        }
        let member = ds.friend.is_none()
            && match d.qualifier {
                None => d.ctx == DeclaratorContext::Member,
                Some(c) => self.ast.decls.record(c).is_some(),
            };
        if member && !ds.storage.is_some_and(|(s, _)| s == StorageSpec::Static) {
            return ty;
        }
        self.reject_qualified_fn_type(ty, ds.loc, member)
    }

    /// Reports `ty`, a function type with a cv-qualifier-seq or
    /// ref-qualifier, as clang's `err_invalid_qualified_function_type`
    /// "of type" form at `loc` ("static member function of type" when
    /// `static_member`, else "non-member function of type") and returns it
    /// without the qualifiers. Any other type is returned unchanged.
    pub(crate) fn reject_qualified_fn_type(
        &mut self,
        ty: QualType,
        loc: Loc,
        static_member: bool,
    ) -> QualType {
        let Some(p) = self.ast.types.proto(ty).copied() else { return ty };
        let info = p.info();
        let (q, r) = (info.this_quals(), info.ref_qual());
        if q.is_empty() && r == RefQual::None {
            return ty;
        }
        let spelled = crate::class::members::method_quals_spelling(q, r);
        let m = self
            .diag(DiagId::sema_qualified_fn_type, loc)
            .arg(u8::from(static_member))
            .arg(self.ty_arg(ty))
            .arg(spelled);
        self.emit(m);
        let params = self.ast.types.params(&p).to_vec();
        let exc = self.ast.types.exc_spec(&p);
        let info = info.with_this_quals(Quals::NONE).with_ref_qual(RefQual::None);
        self.ast.types.function(p.ret(), &params, info, exc)
    }

    /// The type of `d` given the decl-specifier type `base`
    /// ([dcl.meaning], [dcl.ptr], [dcl.ref], [dcl.array], [dcl.fct]).
    pub fn type_for_declarator(&mut self, base: QualType, d: &Declarator) -> QualType {
        self.type_for_declarator_at(base, d.loc, d)
    }

    /// [`Sema::type_for_declarator`] with the location of the
    /// decl-specifier-seq's type-specifier (where a trailing return type
    /// without `auto` is diagnosed).
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per declarator chunk, as clang's GetFullTypeForDeclarator"
    )]
    pub fn type_for_declarator_at(
        &mut self,
        base: QualType,
        spec_loc: Loc,
        d: &Declarator,
    ) -> QualType {
        // A type-id's redundant parentheses (a new-type-id's are checked
        // by `act_on_new`, before its bounds are split off).
        if d.ctx == DeclaratorContext::TypeName && d.site != DeclSite::New {
            self.warn_redundant_parens(None, d);
        }
        // Decl-specifier nullability qualifiers that belong to a chunk.
        let (base, moved) = self.declarator_nullability(base, spec_loc, d);
        // [dcl.type.class.deduct]/2.
        let ty = self.ctad_declarator_check(base, spec_loc, d);
        // [dcl.spec.auto.general]/2-5.
        let mut ty = self.auto_declarator_check(ty, spec_loc, d);
        if matches!(d.site, DeclSite::TypeParamDefault | DeclSite::Alias) {
            self.pending_chunk_loc = Self::inner_chunk_loc(d);
        }
        let mut prev_ref = false;
        let recover = std::mem::take(&mut self.declarator_int_recovery);
        // A failed chunk: the declarator's type is the error type, or
        // `int` under recovery (checked at the next chunk and the end).
        macro_rules! fail {
            () => {{
                ty = QualType::ERROR;
                continue;
            }};
        }
        for (i, chunk) in d.chunks.iter().enumerate().rev() {
            if self.is_error_type(ty) {
                if !recover {
                    return ty;
                }
                ty = QualType::INT;
            }
            ty = self.declarator_parens(ty, d, i + 1);
            if let Some(which) = self.decltype_auto_compound(ty, chunk) {
                let loc = Self::chunk_loc(chunk);
                let msg = self.diag(DiagId::sema_decltype_auto_compound, loc).arg(which);
                self.emit(msg);
                fail!();
            }
            let t = &self.ast.types;
            match chunk {
                Chunk::Pointer { quals, loc } => {
                    if t.is_reference(ty) {
                        let msg = self
                            .diag(DiagId::sema_pointer_to_reference, *loc)
                            .arg(self.declarator_name(d))
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                        fail!();
                    }
                    if self.check_qualified_function(ty, false, *loc) {
                        fail!();
                    }
                    let p = self.ast.types.pointer_to(ty);
                    ty = if quals.is_empty() { p } else { self.ast.types.qualify(p, *quals) };
                    prev_ref = false;
                }
                Chunk::MemberPointer { qual, quals, loc } => {
                    prev_ref = false;
                    let Some(qual) = qual else { fail!() };
                    let Some(class) = self.member_pointer_class(qual) else {
                        // clang 22 says "member pointer" for an abstract declarator
                        // (clang 19 "'type name'"; no identifier has a space).
                        let msg =
                            if self.clang_compat >= 22 && self.declarator_name(d) == "type name" {
                                self.diag(DiagId::sema_mptr_in_nonclass_22, *loc)
                            } else {
                                self.diag(DiagId::sema_mptr_in_nonclass, *loc)
                                    .arg(self.declarator_name(d))
                            };
                        self.emit(msg);
                        fail!();
                    };
                    let t = &self.ast.types;
                    if t.is_reference(ty) {
                        let msg = self
                            .diag(DiagId::sema_mptr_to_reference, *loc)
                            .arg(self.declarator_name(d))
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                        fail!();
                    }
                    if t.is_void(ty) {
                        let msg =
                            self.diag(DiagId::sema_mptr_to_void, *loc).arg(self.declarator_name(d));
                        self.emit(msg);
                        fail!();
                    }
                    let p = self.ast.types.member_pointer(ty, class.ty());
                    ty = if quals.is_empty() { p } else { self.ast.types.qualify(p, *quals) };
                }
                Chunk::LRef { loc } | Chunk::RRef { loc } => {
                    if t.is_reference(ty) && prev_ref {
                        let msg = self
                            .diag(DiagId::sema_reference_to_reference, *loc)
                            .arg(self.declarator_name(d));
                        self.emit(msg);
                        if !recover {
                            fail!();
                        }
                    }
                    let t = &self.ast.types;
                    if t.is_void(ty) {
                        self.error(DiagId::sema_reference_to_void, *loc);
                        fail!();
                    }
                    if self.check_qualified_function(ty, true, *loc) {
                        fail!();
                    }
                    let t = &self.ast.types;
                    let lref = matches!(chunk, Chunk::LRef { .. });
                    // Reference collapsing through typedefs ([dcl.ref]/7).
                    ty = if let TypeView::LValueRef(inner) = t.canonical_view(ty) {
                        self.ast.types.lvalue_ref_to(inner)
                    } else if let TypeView::RValueRef(inner) = t.canonical_view(ty) {
                        if lref {
                            self.ast.types.lvalue_ref_to(inner)
                        } else {
                            self.ast.types.rvalue_ref_to(inner)
                        }
                    } else if lref {
                        self.ast.types.lvalue_ref_to(ty)
                    } else {
                        self.ast.types.rvalue_ref_to(ty)
                    };
                    prev_ref = true;
                }
                Chunk::Array { size, loc } => {
                    prev_ref = false;
                    // clang `BuildArrayType`: in C++ an array of `void`
                    // has an incomplete element type ([dcl.array]/1).
                    if t.is_void(ty) {
                        let msg = self
                            .diag(DiagId::sema_array_incomplete_type, *loc)
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                        fail!();
                    }
                    if t.is_function(ty) {
                        let msg = self
                            .diag(DiagId::sema_array_of_functions, *loc)
                            .arg(self.declarator_name(d))
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                        fail!();
                    }
                    if t.is_reference(ty) {
                        let msg = self
                            .diag(DiagId::sema_array_of_references, *loc)
                            .arg(self.declarator_name(d))
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                        fail!();
                    }
                    // [dcl.array]/1, /3 (clang `BuildArrayType`): in C++
                    // only an array of unknown bound is rejected here; an
                    // incomplete class element is checked where the array
                    // type must be complete ([basic.types.general]/6).
                    if matches!(t.canonical_view(ty), TypeView::IncompleteArray(_)) {
                        let msg = self
                            .diag(DiagId::sema_array_incomplete_type, *loc)
                            .arg(self.ty_arg(ty));
                        self.emit(self.incomplete_note(msg, ty));
                        fail!();
                    }
                    if self.ast.types.is_sizeless(ty) {
                        let msg = self
                            .diag(DiagId::sema_array_sizeless_element, *loc)
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                        fail!();
                    }
                    if self.require_non_abstract_element(*loc, ty) {
                        fail!();
                    }
                    // §5.8 (clang `BuildArrayType`): an element class with a
                    // flexible array member is a GNU extension.
                    if self.has_flexible_array_member(ty) {
                        let msg = self
                            .diag(DiagId::sema_flexible_array_in_array, *loc)
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                    }
                    ty = match size {
                        None => self.ast.types.incomplete_array(ty),
                        Some(e) if self.expr_is_value_dependent(*e) => {
                            // [temp.dep.constexpr]: checked at instantiation.
                            let p = self.dependent_profile(*e);
                            self.ast.types.dependent_sized_array(ty, *e, p)
                        }
                        Some(e) => match self.array_bound(*e, d) {
                            // clang `BuildArrayType`: a zero-length array is
                            // a GNU extension, but an error in a SFINAE
                            // context (`err_typecheck_zero_array_size`), so
                            // `int[0]` never matches `T[N]`.
                            Some(Ok(0)) if self.inst.stack.is_sfinae() => {
                                let l = self.begin_loc(*e);
                                self.error(DiagId::sema_zero_array_size_sfinae, l);
                                fail!();
                            }
                            // clang `BuildArrayType`: an array of variable
                            // length arrays is one too.
                            Some(Ok(_))
                                if matches!(
                                    self.ast.types.canonical_view(ty),
                                    TypeView::VariableArray { .. }
                                ) =>
                            {
                                self.error(self.vla_ext_id(), *loc);
                                self.ast.types.variable_array(ty, *e)
                            }
                            Some(Ok(0)) => {
                                let l = self.begin_loc(*e);
                                let msg = self.diag(DiagId::sema_zero_length_array, l);
                                self.emit(msg.per_instantiation());
                                self.ast.types.constant_array(ty, 0, None)
                            }
                            Some(Ok(n)) => self.ast.types.constant_array(ty, n, None),
                            Some(Err(size)) => self.ast.types.variable_array(ty, size),
                            None => fail!(),
                        },
                    };
                }
                Chunk::Function {
                    params, variadic, loc, exc, quals, ref_qual, trailing, ..
                } => {
                    prev_ref = false;
                    let mut info = FuncInfo::new()
                        .with_variadic(*variadic)
                        .with_this_quals(*quals)
                        .with_ref_qual(*ref_qual);
                    if let Some((ret, _)) = *trailing {
                        // [dcl.fct]/2: the declared return type is `auto`.
                        if !self.is_plain_auto(ty) {
                            if !self.is_error_type(ty) {
                                let at = spec_loc;
                                let msg = self
                                    .diag(DiagId::sema_trailing_return_without_auto, at)
                                    .arg(self.ty_arg(ty));
                                self.emit(msg);
                            }
                            fail!();
                        }
                        ty = ret;
                        info = info.with_trailing_return(true);
                    }
                    let t = &self.ast.types;
                    if t.is_array(ty) || t.is_function(ty) {
                        let which = if t.is_array(ty) { "0" } else { "1" };
                        // clang: a trailing return type is checked where
                        // the `auto` placeholder is written, otherwise at
                        // the function declarator's '('.
                        let at = if trailing.is_some() { spec_loc } else { *loc };
                        let msg = self
                            .diag(DiagId::sema_func_returning_array_function, at)
                            .arg(which)
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                        if !recover {
                            fail!();
                        }
                        ty = QualType::INT;
                    }
                    // [basic.extended.fp] (clang): `__fp16` is a storage-only
                    // type where the target passes no half values, checked
                    // as clang's `GetFullTypeForDeclarator` (no fix-it).
                    let mut fp16_bad = false;
                    if self.is_fp16_arg_disallowed(ty) {
                        let at = d.name.map_or(d.loc, |n| n.1);
                        let msg = self.diag(DiagId::sema_parameters_retval_fp16, at).arg("1");
                        self.emit(msg);
                        fp16_bad = true;
                    }
                    for p in params {
                        if self.is_fp16_arg_disallowed(p.ty) {
                            let at = p.decl.map_or_else(
                                || p.name.map_or(p.loc, |n| n.1),
                                |pd| self.ast.decls.get(pd).loc(),
                            );
                            let msg = self.diag(DiagId::sema_parameters_retval_fp16, at).arg("0");
                            self.emit(msg);
                            fp16_bad = true;
                        }
                    }
                    if fp16_bad {
                        fail!();
                    }
                    // [depr.volatile.type]/2 (C++20 [dcl.fct]/12): a
                    // volatile-qualified return type is deprecated, at the
                    // function declarator's '(' or a trailing return
                    // type's `auto` (clang's `warn_deprecated_volatile_return`;
                    // not before C++20). A substituted declarator was
                    // checked when its type was rebuilt, at the return
                    // type (clang's `BuildFunctionType`; spec/sema-templates.md
                    // §6.4).
                    if self.std >= kiln_lex::Std::Cxx20
                        && self.inst.stack.is_empty()
                        && self.ast.types.effective_quals(ty).contains(Quals::VOLATILE)
                    {
                        let at = if trailing.is_some() { spec_loc } else { *loc };
                        let msg = self
                            .diag(DiagId::sema_deprecated_volatile_return, at)
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                    }
                    // [depr.volatile.type]/2: and a volatile-qualified
                    // parameter type, at the parameter.
                    if self.std >= kiln_lex::Std::Cxx20 && self.inst.stack.is_empty() {
                        self.check_volatile_params(params);
                    }
                    let ptys: Vec<QualType> = params.iter().map(|p| p.ty).collect();
                    ty = self.ast.types.function(ty, &ptys, info, *exc);
                }
            }
            if !d.nullability.is_empty() || !moved.is_empty() {
                ty = self.chunk_nullability(ty, i, d, &moved);
            }
        }
        // [temp.variadic]/5: only a parameter declares a pack (clang
        // `GetFullTypeForDeclarator` drops the `...`).
        if let Some(e) = d.ellipsis
            && !matches!(d.ctx, DeclaratorContext::Prototype | DeclaratorContext::TypeName)
        {
            self.error(DiagId::sema_ellipsis_in_declarator_not_parameter, e);
        }
        if recover && self.is_error_type(ty) {
            return QualType::INT;
        }
        ty
    }

    /// The class a member-pointer declarator's nested-name-specifier
    /// names ([dcl.mptr]/1, clang's `GetFullTypeForDeclarator`): the type
    /// of its last component as written when that is a class or a
    /// dependent type, a dependent name for a dependent `T::X::`; `None`
    /// for a namespace, `::` or a non-class type.
    fn member_pointer_class(&mut self, qual: &crate::using::Qualifier) -> Option<QualType> {
        match self.ast.types.nns_view(qual.nns) {
            kiln_ast::types::Nns::Type { ty, .. } => {
                (self.is_dependent_type(ty) || self.ast.types.as_record(ty).is_some()).then_some(ty)
            }
            kiln_ast::types::Nns::Identifier { prefix: Some(prefix), name } => {
                Some(self.act_dependent_name(prefix, name, None, TagKeyword::None))
            }
            _ => None,
        }
    }

    /// Evaluates an array bound ([dcl.array]/1): a converted constant
    /// expression of type `std::size_t`; `Err(size)` for a variable length
    /// array (GNU extension) with converted bound `size`.
    fn array_bound(&mut self, e: ExprId, d: &Declarator) -> Option<Result<u64, ExprId>> {
        let ety = self.ast.exprs.get(e).ty();
        let loc = self.begin_loc(e);
        if self.is_error_type(ety) {
            return None;
        }
        let t = &self.ast.types;
        // The lvalue conversion of an `_Atomic(T)` bound is a `T` (clang
        // `DefaultLvalueConversion`).
        let vty = t.as_atomic(ety).unwrap_or(ety);
        let integral =
            t.is_integral(vty) || t.as_enum(vty).is_some_and(|en| !self.is_scoped_enum(en));
        if !integral {
            let msg = self.diag(DiagId::sema_array_size_non_int, loc).arg(self.ty_arg(ety));
            self.emit(msg);
            return None;
        }
        let c = conv::decay(self, e);
        let c = self.act_on_constant_full_expr(c);
        match self.eval_int(c, EvalContext::new(EvalKind::Other, loc)) {
            Ok(v) if v < 0 => {
                match d.name {
                    Some((n, _)) => {
                        let msg = self
                            .diag(DiagId::sema_decl_negative_array_size, loc)
                            .arg(sym_str(self, n));
                        self.emit(msg);
                    }
                    None => self.error(DiagId::sema_typecheck_negative_array_size, loc),
                }
                None
            }
            Ok(v) => {
                if v > i128::from(u64::MAX >> 4) {
                    let msg = self
                        .diag(DiagId::sema_array_too_large, loc)
                        .arg(self.element_count_text(v));
                    self.emit(msg);
                    return None;
                }
                u64::try_from(v).ok().map(Ok)
            }
            Err(f) => self.folded_array_bound(c, d, loc, f),
        }
    }

    /// A bound that is not a constant expression makes a variable length
    /// array; at namespace or class scope, where one is not allowed,
    /// clang folds the bound if it can (`EvaluateAsInt`: a `const double`
    /// or `const _Atomic(int)` variable reads as its value) and makes a
    /// constant array as an extension (`TryToFixInvalidVariablyModifiedType`,
    /// after `-Wvla-cxx-extension` with the failure's notes). In a
    /// function a bound that is not a constant expression makes a
    /// variable length array (clang `BuildArrayType`, after the same
    /// warning) even when it would fold; [`Self::check_vla_var`] rejects or
    /// folds one with static storage duration. Anything else is a
    /// variable length array kiln does not support.
    fn folded_array_bound(
        &mut self,
        c: ExprId,
        d: &Declarator,
        loc: Loc,
        mut f: kiln_oven::EvalFailure,
    ) -> Option<Result<u64, ExprId>> {
        let at = d.name.map_or(d.loc, |(_, l)| l);
        let local = matches!(
            d.ctx,
            DeclaratorContext::Block
                | DeclaratorContext::ForInit
                | DeclaratorContext::Condition
                | DeclaratorContext::TypeName
        ) && self.in_local_scope();
        if local {
            let wloc = Self::fold_lone_invalid(&mut f, loc);
            let w = Self::with_failure_notes(self.diag(self.vla_ext_id(), wloc), f);
            self.emit(w);
            return Some(Err(c));
        }
        // An alias-declaration's type-id is a typedef's declarator
        // (diagnosed at the alias name, by `act_on_alias_decl`).
        let alias = d.site == DeclSite::Alias;
        let file_like =
            alias || matches!(d.ctx, DeclaratorContext::File | DeclaratorContext::Member);
        let folded = if file_like {
            self.fold_rvalue_int(c).filter(|&v| (0..=i128::from(u64::MAX >> 4)).contains(&v))
        } else {
            None
        };
        let Some(v) = folded else {
            if file_like {
                self.vla_at_file_scope(d, loc, at, f);
            } else {
                self.error(DiagId::sema_vla_unsupported, loc);
            }
            return None;
        };
        let wloc = Self::fold_lone_invalid(&mut f, loc);
        let w = Self::with_failure_notes(self.diag(self.vla_ext_id(), wloc), f);
        self.emit(w);
        if alias {
            self.pending_alias_vla = Some(DiagId::sema_vla_folded_to_constant);
        } else {
            self.error(DiagId::sema_vla_folded_to_constant, at);
        }
        u64::try_from(v).ok().map(Ok)
    }

    /// A bound that neither is a constant expression nor folds, in a
    /// namespace-scope or member declarator: clang builds the variable
    /// length array (after `-Wvla-cxx-extension` with the failure's
    /// notes, at the bound) and the declaration rejects it at its name
    /// (`CheckVariableDeclarationType`, `CheckTypedefForVariablyModifiedType`,
    /// `CheckFieldDecl`, `CheckFunctionDeclaration`): a function's type is
    /// variably modified, a non-static data member's size is not
    /// constant, and anything else is a variable length array when every
    /// declarator chunk outside the bound is an array (an array of one is
    /// one too) or a variably modified type otherwise ([dcl.array]/1: the
    /// bound is a converted constant expression).
    fn vla_at_file_scope(
        &mut self,
        d: &Declarator,
        loc: Loc,
        at: Loc,
        mut f: kiln_oven::EvalFailure,
    ) {
        let wloc = Self::fold_lone_invalid(&mut f, loc);
        let w = Self::with_failure_notes(self.diag(self.vla_ext_id(), wloc), f);
        self.emit(w);
        let pos = d
            .chunks
            .iter()
            .position(
                |ch| matches!(ch, Chunk::Array { size: Some(s), .. } if self.begin_loc(*s) == loc),
            )
            .unwrap_or(0);
        let outer = &d.chunks[..pos];
        // clang `BuildArrayType`: the arrays around it are variable length
        // arrays too, each warned at its `[` (at its bound, with the
        // failure's notes, when that is not a constant expression either).
        for ch in outer.iter().rev() {
            let Chunk::Array { size: Some(s), loc: l, .. } = ch else { break };
            if self.expr_is_value_dependent(*s) {
                break;
            }
            let bl = self.begin_loc(*s);
            let c = conv::decay(self, *s);
            let c = self.act_on_constant_full_expr(c);
            let w = match self.eval_int(c, EvalContext::new(EvalKind::Other, bl)) {
                Ok(_) => self.diag(self.vla_ext_id(), *l),
                Err(mut f) => {
                    let wloc = Self::fold_lone_invalid(&mut f, bl);
                    Self::with_failure_notes(self.diag(self.vla_ext_id(), wloc), f)
                }
            };
            self.emit(w);
        }
        let id = if matches!(outer.first(), Some(Chunk::Function { .. })) {
            DiagId::sema_vm_func_decl
        } else if self.declarator_field {
            DiagId::sema_field_variable_size
        } else if outer.iter().all(|ch| matches!(ch, Chunk::Array { .. })) {
            DiagId::sema_vla_decl_in_file_scope
        } else {
            DiagId::sema_vm_decl_in_file_scope
        };
        if d.site == DeclSite::Alias {
            self.pending_alias_vla = Some(id);
        } else {
            self.error(id, at);
        }
    }

    /// clang `ext_vla_cxx` / `ext_vla_cxx_in_gnu_mode` (off by default in
    /// a GNU dialect).
    fn vla_ext_id(&self) -> DiagId {
        if self.gnu { DiagId::sema_vla_cxx_extension_gnu } else { DiagId::sema_vla_cxx_extension }
    }

    /// clang `CheckVariableDeclarationType` for a variable length array:
    /// one with static or thread storage duration has its bound folded
    /// (`TryToFixInvalidVariablyModifiedType`, after the warning
    /// [`Self::folded_array_bound`] gave) or is an error, as is one with
    /// `extern` linkage. Returns the variable's type.
    fn check_vla_var(&mut self, ty: QualType, spec: VarSpec, d: &Declarator) -> QualType {
        let TypeView::VariableArray { elem, size } = self.ast.types.canonical_view(ty) else {
            return ty;
        };
        let at = d.name.map_or(d.loc, |(_, l)| l);
        if spec.storage() == VarStorage::Extern {
            self.error(DiagId::sema_vla_extern_linkage, at);
            return QualType::ERROR;
        }
        if spec.storage() != VarStorage::Static && spec.tls() == TlsKind::None {
            return ty;
        }
        let folded = self
            .fold_rvalue_int(size)
            .filter(|&v| (0..=i128::from(u64::MAX >> 4)).contains(&v))
            .and_then(|v| u64::try_from(v).ok());
        let Some(n) = folded else {
            self.error(DiagId::sema_vla_static_storage, at);
            return QualType::ERROR;
        };
        self.error(DiagId::sema_vla_folded_to_constant, at);
        self.ast.types.constant_array(elem, n, None)
    }

    /// clang `FK_VariableLengthArrayHasInitializer`: a variable length
    /// array cannot be initialized ("variable-sized object may not be
    /// initialized", at the bound). True (and `v` invalid) if rejected.
    fn reject_vla_init(&mut self, v: DeclId, ty: QualType, init: &Initializer) -> bool {
        let TypeView::VariableArray { size, .. } = self.ast.types.canonical_view(ty) else {
            return false;
        };
        let at = self.begin_loc(size);
        self.error(DiagId::sema_vla_no_init, at);
        self.ast.decls.set_flags(v, DeclFlags::INVALID);
        self.discard_initializer(init);
        true
    }

    pub(crate) fn is_scoped_enum(&self, en: DeclId) -> bool {
        let def = self.ast.definition(en).unwrap_or(en);
        self.ast.decls.enumeration(def).is_some_and(EnumData::is_scoped)
    }

    /// [dcl.fct]/5: a parameter of array type "array of T" becomes
    /// "pointer to T"; of function type, "pointer to" that type.
    /// A nullability qualifier on the array (`int a[_Nonnull]`, or a
    /// decl-specifier one) moves to the pointer (clang keeps
    /// `int * _Nonnull`).
    pub(crate) fn adjust_param_type(&mut self, ty: QualType) -> QualType {
        if let TypeView::Attributed { kind, modified, .. } = self.ast.types.view(ty.ty())
            && crate::nullability::is_nullability(kind)
            && self.ast.types.is_array(modified)
        {
            let p = self.adjust_param_type(modified);
            return self.ast.types.attributed(kind, p, p);
        }
        if self.ast.types.is_array(ty) {
            let elem = self.ast.types.element(ty).unwrap_or(QualType::ERROR);
            self.ast.types.pointer_to(elem)
        } else if self.ast.types.is_function(ty) {
            self.ast.types.pointer_to(ty)
        } else {
            ty
        }
    }

    /// A parameter-declaration ([dcl.fct]/3): its type, adjusted
    /// ([dcl.fct]/5: arrays and functions become pointers).
    pub fn act_on_param_declarator(&mut self, ds: &DeclSpec, d: &Declarator) -> ParamInfo {
        // [dcl.stc]/2: no storage class but `register`, which C++17
        // removed and clang accepts as an extension (an error by default,
        // `-Wregister`). Both before the type, in clang's order.
        // A non-type template parameter ([temp.param]/2: no storage
        // class; [dcl.inline]/1, [dcl.constexpr]/1, [dcl.fct.spec]/1):
        // clang's `CheckValidDeclSpecifiers`, one error per specifier in
        // its order, instead of the function parameter's.
        let nttp = d.site == DeclSite::TemplateParam;
        if nttp {
            let locs = [
                ds.storage.map(|(_, l)| l),
                ds.thread_local,
                ds.inline,
                ds.constexpr.or(ds.consteval).or(ds.constinit),
                ds.virtual_,
                ds.explicit,
                ds.noreturn,
            ];
            for l in locs.into_iter().flatten() {
                self.error(DiagId::sema_invalid_decl_spec_in_nontype_parm, l);
            }
        } else {
            match ds.storage {
                Some((StorageSpec::Register, loc)) => {
                    self.emit(self.diag(DiagId::sema_register_storage_class, loc));
                }
                Some((_, loc)) => self.error(DiagId::sema_invalid_storage_class_in_func_decl, loc),
                None => {}
            }
        }
        let base = self.type_from_declspec(ds);
        let base = self.apply_declspec_vector_attrs(base, ds);
        self.warn_redundant_parens(None, d);
        // A pack's erroneous type recovers as `int` (clang's recovery
        // type), so "type 'int' of function parameter pack does not
        // contain any unexpanded parameter packs" follows the type error.
        let errs = self.diags.error_count();
        self.declarator_int_recovery = d.ellipsis.is_some();
        let mut ty = self.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), d);
        self.declarator_int_recovery = false;
        ty = self.check_qualified_fn_type(ty, ds, d);
        let recovered = d.ellipsis.is_some() && self.diags.error_count() != errs;
        // [dcl.constexpr]/1, [dcl.constinit]/1: clang `err_invalid_constexpr`.
        let specs = [ds.constexpr, ds.consteval, ds.constinit];
        if !nttp
            && let Some((which, l)) =
                specs.iter().enumerate().find_map(|(i, l)| l.map(|l| (i as u8, l)))
        {
            let m = self.diag(DiagId::sema_param_constexpr_spec, l).arg(which);
            self.emit(m);
        }
        let mut attrs = self.non_vector_attrs(&ds.attrs);
        attrs.extend(d.attrs.iter().copied());
        // [dcl.align]/1: not on a function parameter.
        self.reject_alignas(&mut attrs, 0);
        ty = self.apply_vector_attrs(ty, &mut attrs, false);
        ty = self.apply_noreturn_attrs(ty, &attrs);
        let declared = ty;
        ty = self.adjust_param_type(ty);
        let mut original_ty = (ty != declared).then_some(declared);
        if let Some(l) = d.ellipsis {
            // [dcl.fct]/16, [temp.variadic]/5: a function parameter pack
            // `T... x` names a pack; else clang's
            // `err_function_parameter_pack_without_parameter_packs`, which
            // prints the declared (unadjusted) type and drops the `...`.
            if self.ast.types.flags(declared.ty()).contains(TypeFlags::UNEXPANDED_PACK) {
                // An array whose bound alone names the pack (`int
                // ...a[Ns]`) expands unadjusted; each element decays when
                // the pack is substituted, as in clang.
                let base = if self.has_unexpanded_pack(ty) {
                    ty
                } else {
                    original_ty = None;
                    declared
                };
                ty = self.act_pack_expansion_type(base, l);
            } else if !self.is_error_type(declared) {
                let m =
                    self.diag(DiagId::sema_param_pack_without_packs, l).arg(self.ty_arg(declared));
                self.emit(m);
            }
        }
        ParamInfo {
            name: d.name,
            ty,
            original_ty,
            loc: d.name_loc(),
            attrs,
            invalid: ds.invalid || d.invalid || recovered,
            default_arg: DefaultArg::None,
            default_loc: Loc::INVALID,
            range: (ds.loc, d.name_loc()),
            type_loc: ds.type_loc.unwrap_or(ds.loc),
            ref_loc: if d.ellipsis.is_none() { Self::outer_ref_loc(d) } else { Loc::INVALID },
            chunk_loc: if d.ellipsis.is_none() { Self::inner_chunk_loc(d) } else { Loc::INVALID },
            decl: None,
            explicit_object: d.explicit_object,
        }
    }

    /// Whether the function declared by `d` with parameters `params` is an
    /// explicit object member function ([dcl.fct]/6, P0847): its first
    /// parameter is an explicit object parameter. Diagnoses one anywhere
    /// else, in a static, virtual or non-member function (`Some(false)`:
    /// the declaration is invalid, as clang's `setInvalidType`), with a
    /// default argument or as a pack. (Its cv- and ref-qualifiers are
    /// [`Sema::drop_invalid_fn_quals`]'.) `None`: no explicit object
    /// parameter.
    pub(crate) fn explicit_object_member(
        &mut self,
        ds: &DeclSpec,
        params: &[ParamInfo],
        method: bool,
    ) -> Option<bool> {
        for p in params.iter().skip(1) {
            if p.explicit_object.is_some() {
                let m = self.diag(DiagId::sema_explicit_object_param_not_first, p.loc).arg(0u8);
                self.emit(m);
            }
        }
        let first = params.first()?;
        let this_loc = first.explicit_object?;
        let which = if !method {
            Some(2u8)
        } else if ds.storage.is_some_and(|(s, _)| s == StorageSpec::Static) {
            Some(0)
        } else if ds.virtual_.is_some() {
            Some(1)
        } else {
            None
        };
        if let Some(w) = which {
            let m =
                self.diag(DiagId::sema_explicit_object_param_nonmember, this_loc).arg(w).arg(0u8);
            self.emit(m);
            return Some(false);
        }
        match first.default_arg {
            DefaultArg::None => {}
            DefaultArg::Unparsed(l, _) => self.error(DiagId::sema_explicit_object_param_default, l),
            DefaultArg::Parsed(e) => {
                let l = if first.default_loc.is_valid() { first.default_loc } else { self.eloc(e) };
                self.error(DiagId::sema_explicit_object_param_default, l);
            }
        }
        if matches!(self.ast.types.view(first.ty.ty()), TypeView::PackExpansion { .. }) {
            self.error(DiagId::sema_explicit_object_param_pack, this_loc);
        }
        Some(true)
    }

    /// [dcl.fct]/6: a cv-qualifier-seq or ref-qualifier is only on the
    /// function type of a non-static member function without an explicit
    /// object parameter (static ones are
    /// [`Sema::check_static_method_quals`]'). clang
    /// (`err_invalid_qualified_function_type`, or
    /// `err_explicit_object_parameter_qualifiers` for an explicit object
    /// member function `explicit_object`) quotes them (cv first) at the
    /// first one written and drops them from the type, so an
    /// instantiation does not report them again.
    pub(crate) fn drop_invalid_fn_quals(
        &mut self,
        ty: QualType,
        d: &Declarator,
        explicit_object: bool,
    ) -> QualType {
        let Some(p) = self.ast.types.proto(ty).copied() else { return ty };
        let info = p.info();
        let (q, r) = (info.this_quals(), info.ref_qual());
        if q.is_empty() && r == RefQual::None {
            return ty;
        }
        let spelled = crate::class::members::method_quals_spelling(q, r);
        let at = first_fn_qual_loc(d).unwrap_or(d.loc);
        let id = if explicit_object {
            DiagId::sema_explicit_object_param_qualifier
        } else {
            DiagId::sema_nonmember_fn_quals
        };
        let msg = self.diag(id, at).arg(spelled);
        self.emit(msg);
        let params = self.ast.types.params(&p).to_vec();
        let exc = self.ast.types.exc_spec(&p);
        let info = info.with_this_quals(Quals::NONE).with_ref_qual(RefQual::None);
        self.ast.types.function(p.ret(), &params, info, exc)
    }

    /// The default argument `= init` of parameter `p` ([dcl.fct.default]),
    /// parsed in place: copy-initialization of the parameter's type
    /// ([dcl.fct.default]/5), stored as [`DefaultArg::Parsed`]. (A default
    /// argument replayed in a complete-class context goes through
    /// [`Sema::act_on_param_default_arg`] instead.)
    /// [dcl.fct.default]/3: "A default argument shall not be specified
    /// for a function parameter pack": clang's "parameter pack cannot
    /// have a default argument" at the `=` `eq`, the argument discarded.
    /// True (and diagnosed) if `ty`, a parameter's type, is a pack.
    pub fn reject_param_pack_default(&mut self, ty: QualType, eq: Loc) -> bool {
        if !matches!(self.ast.types.view(ty.ty()), TypeView::PackExpansion { .. }) {
            return false;
        }
        self.error(DiagId::sema_param_default_argument_on_parameter_pack, eq);
        true
    }

    pub fn act_on_param_default_init(&mut self, p: &mut ParamInfo, init: Initializer) {
        let roots: Vec<ExprId> = match &init {
            Initializer::Expr(e) => vec![*e],
            Initializer::Paren(v, _, _) => v.clone(),
            // (A braced default argument is not searched: a gap.)
            Initializer::List(_) => Vec::new(),
        };
        // [temp.variadic]/12: `void f(int = sizeof(Ts));` (clang
        // reports at the argument's begin).
        let unexpanded = !p.invalid
            && roots.iter().any(|&e| {
                let at = self.begin_loc(e);
                self.diagnose_unexpanded_default_arg(e, at)
            });
        // [dcl.fct.default]/3, after the unexpanded-pack check (clang's
        // `ActOnParamDefaultArgument`); an invalid argument says nothing.
        if !unexpanded
            && !p.invalid
            && !roots.iter().any(|&e| self.has_errors_in(e))
            && self.reject_param_pack_default(p.ty, p.default_loc)
        {
            return;
        }
        if unexpanded || !roots.into_iter().all(|e| self.check_default_arg_refs(e)) {
            p.invalid = true;
            if let Some(d) = p.decl {
                self.ast.decls.set_flags(d, DeclFlags::INVALID);
            }
        }
        if p.invalid || self.is_error_type(p.ty) {
            if let Initializer::Expr(e) = init {
                p.default_arg = DefaultArg::Parsed(e);
            }
            return;
        }
        let e = match init {
            // Initialized at instantiation ([temp.dep.expr]), the list
            // kept as written (WP-192: a designated list needs it).
            Initializer::List(l)
                if self.ast.types.is_dependent(p.ty) || self.init_list_type_dependent(&l) =>
            {
                self.dependent_init_list(&l)
            }
            Initializer::List(mut l) => {
                let scalar = !self.ast.types.is_array(p.ty)
                    && self.ast.types.as_record(p.ty).is_none()
                    && !self.ast.types.is_reference(p.ty);
                if scalar && l.items.len() > 1 {
                    let d = self.diag(DiagId::sema_excess_initializers, p.loc).arg("2");
                    let d = self.with_param_note(d, p);
                    self.emit(d);
                    l.items.truncate(1);
                    self.init_from_list_at(p.ty, &l, Some(p.loc))
                } else {
                    let note = Some(self.with_param_note_only(p));
                    self.init_list_entity(p.ty, &l, p.loc, ConvCtx::Argument, note)
                }
            }
            Initializer::Expr(e) => {
                let note = self.with_param_note_only(p);
                self.convert_default_arg(e, p.ty, p.loc, note)
            }
            Initializer::Paren(v, l, _) => self.recover(p.ty, &v, l),
        };
        self.note_default_init_full_expr();
        let e = self.act_on_full_expr(e);
        // An erroneous default argument makes the parameter, and so its
        // function, invalid (clang's `ActOnParamDefaultArgumentError`).
        if self.has_errors_in(e) {
            p.invalid = true;
            if let Some(d) = p.decl {
                self.ast.decls.set_flags(d, DeclFlags::INVALID);
            }
        }
        p.default_arg = DefaultArg::Parsed(e);
    }

    /// [dcl.fct.default]/9: a parameter shall not appear as a
    /// potentially-evaluated expression in a default argument (clang's
    /// `CheckDefaultArgumentVisitor`: "default argument references
    /// parameter"). False after a diagnostic, for the first such use.
    /// Lambda bodies are not searched (their captures are checked with
    /// the lambda).
    pub fn check_default_arg_refs(&mut self, e: ExprId) -> bool {
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            let n = self.ast.exprs.get(x);
            match n.kind() {
                ExprKind::LambdaExpr => continue,
                ExprKind::DeclRefExpr => {
                    let d = self.ast.exprs.decl(x);
                    if self.kind_of(d) == DeclKind::ParmVar
                        && self.ast.exprs.non_odr_use(x) != kiln_ast::expr::NonOdrUse::Unevaluated
                    {
                        let name = self.ast.decls.get(d).name();
                        let text = self.decl_name_str(name);
                        let msg = self
                            .diag(DiagId::sema_param_default_argument_references_param, n.loc())
                            .arg(text);
                        self.emit(msg);
                        return false;
                    }
                }
                _ => {}
            }
            let mut kids = self.ast.exprs.children(x, &self.ast.lists);
            kids.reverse();
            stack.extend(kids);
        }
        true
    }

    /// Attaches clang's "passing argument to parameter 'p' here" note.
    pub(crate) fn with_param_note(&self, d: kiln_diag::Diag, p: &ParamInfo) -> kiln_diag::Diag {
        d.with_note(self.with_param_note_only(p))
    }

    /// clang's "passing argument to parameter 'p' here" note for the
    /// parameter being declared.
    fn with_param_note_only(&self, p: &ParamInfo) -> kiln_diag::Diag {
        match p.name {
            None => self.diag(DiagId::note_parameter_here, p.loc),
            Some((n, _)) => self
                .diag(DiagId::note_parameter_named_here, p.loc)
                .arg(self.ast.symbols().as_str(n).to_string()),
        }
    }

    /// Copy-initializes a parameter of type `pty` declared at `ploc` from
    /// its default argument `e` ([dcl.fct.default]/5). As clang does
    /// (`InitializationKind::CreateCopy(Param->getLocation(), ..)`), a
    /// failure is reported at the parameter with the "passing argument to
    /// parameter here" note `note`.
    pub(crate) fn convert_default_arg(
        &mut self,
        e: ExprId,
        pty: QualType,
        ploc: Loc,
        note: kiln_diag::Diag,
    ) -> ExprId {
        if self.is_err_expr(e) {
            return e;
        }
        let to = if self.ast.types.is_reference(pty) { pty } else { conv::strip_cv(self, pty) };
        if self.ast.types.as_record(to).is_some()
            && !self.ast.types.is_dependent(to)
            && !self.expr_is_type_dependent(e)
        {
            let why = crate::class::DeletedAs::Param(note.clone());
            let built = if self.ast.types.as_record(self.ety(e)).is_some() {
                self.construct_from_class(e, to, ConvCtx::Argument, ploc, why)
            } else {
                self.convert_to_class(e, to, ConvCtx::Argument, ploc, why)
            };
            if let Some(x) = built {
                return x;
            }
        }
        // [over.over] against the parameter: a failure keeps clang's
        // "passing argument to parameter" note after the candidates.
        let e = if self.ety(e) == QualType::OVERLOAD && self.overload_operand(e).is_some() {
            let at = self.begin_loc(e);
            let ctx = crate::overload::address::AddrCtx::Convert;
            let noted = Some(note.clone());
            let Some(x) = self.resolve_address_of_overloaded_noted(e, pty, true, ctx, at, noted)
            else {
                let loc = self.ast.exprs.get(e).loc();
                return self.recover(pty, &[e], loc);
            };
            x
        } else {
            e
        };
        match conv::implicit_convert(self, e, pty, ConvCtx::Argument) {
            Ok(x) => x,
            Err(err) => {
                let d = err.diag_at(self, ploc).with_note(note);
                self.emit(d);
                let loc = self.ast.exprs.get(e).loc();
                self.recover(pty, &[e], loc)
            }
        }
    }

    /// clang's "passing argument to parameter 'p' here" note (the name is
    /// optional) for the parameter declaration `p`.
    pub(crate) fn param_decl_note(&self, p: DeclId) -> kiln_diag::Diag {
        let n = self.ast.decls.get(p);
        if n.name().is_empty() {
            return self.diag(DiagId::note_parameter_here, n.loc());
        }
        self.diag(DiagId::note_parameter_named_here, n.loc())
            .arg(self.decl_arg(p).trim_matches('\'').to_string())
    }

    /// True if `q` is an undeduced plain `auto` (not `decltype(auto)`,
    /// nor a constrained `C auto`: clang's `isConstrained` check for a
    /// trailing return type, [dcl.fct]/2).
    fn is_plain_auto(&self, q: QualType) -> bool {
        q.quals().is_empty()
            && matches!(
                self.ast.types.view(q.ty()),
                TypeView::Auto { keyword: AutoKeyword::Auto, deduced: None, concept: None, .. }
            )
    }

    /// `decltype(auto)` under a pointer, reference or array declarator
    /// ([dcl.type.auto.deduct]/5): clang's selector (`pointer to`,
    /// `reference to`, `array of`).
    fn decltype_auto_compound(&self, ty: QualType, chunk: &Chunk) -> Option<u8> {
        if !matches!(
            self.ast.types.view(ty.ty()),
            TypeView::Auto { keyword: AutoKeyword::DecltypeAuto, deduced: None, .. }
        ) {
            return None;
        }
        match chunk {
            Chunk::Pointer { .. } | Chunk::MemberPointer { .. } => Some(0),
            Chunk::LRef { .. } | Chunk::RRef { .. } => Some(1),
            Chunk::Array { .. } => Some(2),
            Chunk::Function { .. } => None,
        }
    }

    /// [dcl.fct]/6: a pointer or reference to an abominable function type
    /// (one with a cv-qualifier-seq or ref-qualifier) is ill-formed
    /// (clang's `checkQualifiedFunction`, also run when substitution
    /// rebuilds `T*` / `T&` / `T&&`, a SFINAE error there).
    pub(crate) fn check_qualified_function(
        &mut self,
        ty: QualType,
        reference: bool,
        loc: Loc,
    ) -> bool {
        let Some(p) = self.ast.types.proto(ty).copied() else { return false };
        let info = p.info();
        let (q, r) = (info.this_quals(), info.ref_qual());
        if q.is_empty() && r == RefQual::None {
            return false;
        }
        // The type is named only when written through sugar (a typedef or
        // a substituted parameter), as clang's `isa<FunctionType>`.
        let direct = matches!(self.ast.types.view(ty.ty()), TypeView::Function(_));
        let msg = self
            .diag(DiagId::sema_compound_qualified_function_type, loc)
            .arg(u8::from(reference))
            .arg(u8::from(!direct))
            .arg(self.ty_arg(ty))
            .arg(crate::class::members::method_quals_spelling(q, r));
        self.emit(msg);
        true
    }

    fn chunk_loc(chunk: &Chunk) -> Loc {
        match chunk {
            Chunk::Pointer { loc, .. }
            | Chunk::MemberPointer { loc, .. }
            | Chunk::LRef { loc }
            | Chunk::RRef { loc }
            | Chunk::Array { loc, .. }
            | Chunk::Function { loc, .. } => *loc,
        }
    }

    // -- declarations ------------------------------------------------------------

    /// The context a declarator declares into, and its lexical context.
    pub(crate) fn decl_contexts(&self, d: &Declarator) -> (DeclId, DeclId) {
        match d.qualifier {
            Some(q) => (q, self.ctx),
            None => (self.ctx, self.ctx),
        }
    }

    /// The declarations of `name` a declaration may redeclare (redeclaration
    /// lookup, declaration order).
    pub(crate) fn prev_candidates(&self, name: Symbol, qualifier: Option<DeclId>) -> Vec<DeclId> {
        self.prev_candidates_dn(DeclName::ident(name), qualifier)
    }

    /// [`Sema::prev_candidates`] for any declaration name.
    pub(crate) fn prev_candidates_dn(
        &self,
        name: DeclName,
        qualifier: Option<DeclId>,
    ) -> Vec<DeclId> {
        match qualifier {
            // [class.mfct]/3, [class.static.data]/3: an out-of-line
            // definition redeclares a member of that class itself, never
            // one inherited from a base.
            Some(q) if self.kind_of(q) == DeclKind::CXXRecord => {
                let def = self.ast.definition(q).unwrap_or(q);
                self.lookup_in_dn(def, name, LookupKind::Ordinary).decls
            }
            Some(q) => self.lookup_qualified_dn(q, name, LookupKind::Ordinary).decls,
            None => {
                let mut v = self.current_scope_raw_dn(name);
                v.retain(|&d| self.kind_of(d) != DeclKind::UsingShadow);
                v
            }
        }
    }

    /// The member among `found` (an out-of-line member function
    /// declaration's redeclaration lookup) that is neither a function, a
    /// function template nor a class or enumeration, when no function is
    /// found: a variable, data member, type alias or enumerator, which the
    /// declaration redefines as a different kind of symbol (clang
    /// `MergeFunctionDecl`; a tag name is hidden and matches nothing).
    pub(crate) fn non_function_member(&self, found: &[DeclId]) -> Option<DeclId> {
        let is_fn = |d: DeclId| {
            let k = self.kind_of(self.ast.strip_shadow(d));
            k.payload() == kiln_ast::decl::PayloadClass::Function || k == DeclKind::FunctionTemplate
        };
        if found.iter().any(|&d| is_fn(d)) {
            return None;
        }
        found.iter().copied().find(|&d| {
            !is_tag_kind(self.kind_of(d))
                && !self.ast.decls.get(d).has(DeclFlags::INVALID)
                && self.kind_of(d) != DeclKind::UsingShadow
        })
    }

    /// Previous declarations of `name` that `d` may redeclare.
    pub(crate) fn find_prev(&self, name: Symbol, qualifier: Option<DeclId>) -> Option<DeclId> {
        let found = self.prev_candidates(name, qualifier);
        // A template declaration redeclares a template (its latest
        // pattern); any other declaration skips templates.
        let templ = self.templ.cur.is_some();
        found.into_iter().rev().find_map(|d| {
            let k = self.ast.decls.get(d).kind();
            if is_tag_kind(k) {
                None
            } else if is_template_kind(k) {
                if templ { self.pattern_of(d).map(|p| self.ast.latest(p)) } else { None }
            } else if templ {
                None
            } else {
                Some(d)
            }
        })
    }

    /// The previous declaration an `extern` variable declaration
    /// redeclares: an ordinary previous declaration, else, at block scope,
    /// one in the innermost enclosing namespace ([basic.link]/6), else a
    /// block-scope declaration of the same entity ([`Sema::local_externs`]).
    fn find_prev_linkage(&self, name: Symbol, qualifier: Option<DeclId>) -> Option<DeclId> {
        if let Some(p) = self.find_prev(name, qualifier) {
            return Some(p);
        }
        if qualifier.is_some() {
            return None;
        }
        let ns = self.enclosing_namespace()?;
        if self.in_local_scope()
            && let Some(p) =
                self.lookup_in(ns, name, LookupKind::Ordinary).decls.into_iter().rev().find(|&d| {
                    matches!(self.ast.decls.get(d).kind(), DeclKind::Var | DeclKind::Function)
                })
        {
            return Some(p);
        }
        self.local_externs.iter().rev().find(|&&(n, s, _)| n == ns && s == name).map(|x| x.2)
    }

    /// The previous declaration a function declaration `new` redeclares
    /// ([basic.scope.scope]/4): a corresponding ordinary previous
    /// declaration, else, at block scope, one in the innermost enclosing
    /// namespace ([basic.link]/6), else a block-scope declaration of the
    /// same entity ([`Sema::local_externs`]).
    #[inline(never)]
    fn find_prev_function(
        &mut self,
        dname: DeclName,
        qualifier: Option<DeclId>,
        new: NewFn,
    ) -> Option<DeclId> {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("find_prev_function");
        let mut found = self.prev_candidates_dn(dname, qualifier);
        // A friend of a templated class is not visible to redeclaration
        // lookup until instantiated: a later namespace-scope declaration
        // declares a new function, matched by the friend's
        // instantiations ([temp.friend]/9, [temp.inst]/3; clang leaves a
        // friend of a dependent context out of its namespace's lookup).
        // One that redeclared an earlier function (a qualified friend)
        // stands for it.
        found.retain(|&x| !self.is_dependent_friend_fn(x) || self.ast.first(x) != x);
        if qualifier.is_some() {
            // A declaration whose parameter needed the missing-`typename`
            // recovery is invalid: a qualified redeclaration does not
            // match it (clang 19; clang 22 keeps the recovered
            // declaration valid, spec/sema.md §1.1).
            found.retain(|&x| {
                self.clang_compat >= 22
                    || !self.missing_typename_fns.contains(&x)
                        && !self
                            .pattern_of(x)
                            .is_some_and(|p| self.missing_typename_fns.contains(&p))
            });
        }
        if let Some(p) = self.pick_prev_function(&found, new) {
            return Some(p);
        }
        if qualifier.is_some() {
            return None;
        }
        let ns = self.enclosing_namespace()?;
        let name = dname.as_ident()?;
        if self.in_local_scope() {
            let mut outer = self.lookup_in(ns, name, LookupKind::Ordinary).decls;
            outer.retain(|&d| matches!(self.kind_of(d), DeclKind::Var | DeclKind::Function));
            if let Some(p) = self.pick_prev_function(&outer, new) {
                return Some(p);
            }
        }
        let locals: Vec<DeclId> = self
            .local_externs
            .iter()
            .filter(|&&(n, s, _)| n == ns && s == name)
            .map(|x| x.2)
            .collect();
        self.pick_prev_function(&locals, new)
    }

    /// The previous declaration member function declaration `new`
    /// redeclares among the members of its class named `dname`
    /// (`qualifier`: the class of an out-of-line declaration), and, when
    /// `other_kind` (an out-of-line definition, not a friend), the member
    /// of another kind it would redefine ([`Sema::non_function_member`]).
    #[inline(never)]
    fn member_redecl_prev(
        &mut self,
        dname: DeclName,
        qualifier: Option<DeclId>,
        new: NewFn,
        other_kind: bool,
    ) -> (Option<DeclId>, Option<DeclId>) {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("member_redecl_prev");
        let found = self.prev_candidates_dn(dname, qualifier);
        let p = self.pick_prev_function(&found, new);
        let other = if other_kind { self.non_function_member(&found) } else { None };
        (p, other)
    }

    /// The declarations of `dname` in namespace `ctx` an unqualified friend
    /// declaration may redeclare (redeclaration lookup, declaration order):
    /// hidden friends included, members of its inline namespaces and
    /// friends of templated classes not (see [`Sema::is_dependent_friend_fn`]).
    /// Of a large namespace, only those that can take part in `new`'s
    /// redeclaration search (`Sema::redecl_candidates`, WP-658).
    #[inline(never)]
    fn hidden_friend_redecl_set(
        &mut self,
        ctx: DeclId,
        dname: DeclName,
        new: &NewFn,
    ) -> Vec<DeclId> {
        self.redecl_candidates(ctx, dname, new)
            .into_iter()
            .filter(|&(d, f)| {
                !f.contains(LookupFlags::VIA_INLINE) && !self.is_dependent_friend_fn(d)
            })
            .map(|(d, _)| d)
            .collect()
    }

    /// The innermost namespace (or the translation unit) enclosing the
    /// current context.
    pub(crate) fn enclosing_namespace(&self) -> Option<DeclId> {
        let mut ns = self.ctx;
        for _ in 0..10_000 {
            if matches!(
                self.ast.decls.get(ns).kind(),
                DeclKind::Namespace | DeclKind::TranslationUnit
            ) {
                return Some(ns);
            }
            ns = self.ast.decls.get(ns).ctx()?;
        }
        None
    }

    /// Records a block-scope function or `extern` variable declaration
    /// for later namespace-scope redeclarations. A declaration in a
    /// template pattern is not recorded: only its instantiations
    /// redeclare the entity ([temp.spec.general]/2; clang does not chain
    /// a dependent-context local extern, WP-330).
    fn note_local_extern(&mut self, name: Symbol, d: DeclId) {
        if self.in_local_scope()
            && !self.in_template()
            && let Some(ns) = self.enclosing_namespace()
        {
            self.local_externs.push((ns, name, d));
        }
    }

    /// The declarator forms the parser accepts but sema does not handle yet
    /// (spec/parse.md §9): reports `{what} is not supported yet`, returning
    /// true when the declaration is dropped. Constructors, destructors and
    /// conversion functions are WP-067a, operator functions WP-068b; the
    /// specifiers are reported and ignored.
    pub(crate) fn unsupported_declarator(&mut self, ds: &DeclSpec, d: &Declarator) -> bool {
        if let Some((sp, loc)) = d.special {
            let what = match sp {
                SpecialName::Constructor(_) => "constructors",
                SpecialName::Destructor(_) => "destructors",
                SpecialName::Conversion(_) => "conversion functions",
                SpecialName::Operator(_) => "overloaded operators",
                SpecialName::LiteralOperator(_) => "literal operators",
                SpecialName::DeductionGuide(_) => "deduction guides here",
            };
            self.unsupported(what, loc);
            return true;
        }
        // `explicit` outside a constructor or conversion function is
        // ill-formed ([dcl.fct.spec]/3); on a friend it is WP-068b.
        if let Some(l) = ds.explicit {
            if ds.friend.is_some() {
                self.unsupported("'explicit'", l);
            } else {
                let function = d.is_function_declarator() && !ds.is_typedef();
                self.check_explicit_spec(ds, d, function, false);
            }
        }
        // [dcl.constexpr]/2: `consteval` declares an immediate function.
        // A non-static data member reports it in `act_on_field`.
        let function = d.is_function_declarator() && !ds.is_typedef();
        let field = d.ctx == DeclaratorContext::Member
            && !ds.is_typedef()
            && !matches!(ds.storage, Some((StorageSpec::Static, _)));
        if let Some(l) = ds.consteval
            && !function
            && !field
        {
            self.error(DiagId::sema_consteval_wrong_decl_kind, l);
        }
        // A friend function goes through `act_on_friend`; elsewhere
        // `friend` is not in a class.
        if d.ctx != DeclaratorContext::Member
            && let Some(l) = ds.friend
        {
            self.error(DiagId::sema_friend_invalid_in_context, l);
        }
        // Member functions take cv/ref-qualifiers (WP-066b); a non-member
        // function's are `act_on_function`'s (`drop_invalid_fn_quals`).
        false
    }
    /// [temp.explicit]/1 (clang `ActOnExplicitInstantiation`): an
    /// explicit instantiation cannot be `inline`, and that of a function
    /// cannot be `constexpr` or `consteval`. Diagnosed; the instantiation
    /// proceeds.
    fn check_explicit_inst_specifiers(&mut self, ds: &DeclSpec, ty: QualType) {
        if let Some(l) = ds.inline {
            let msg = self
                .diag(DiagId::sema_explicit_inst_inline, l)
                .range(Span::new(l, l.with_offset(6)));
            self.emit(msg);
        }
        if let Some(l) = ds.constexpr.or(ds.consteval)
            && self.ast.types.is_function(ty)
        {
            self.error(DiagId::sema_explicit_inst_constexpr, l);
        }
    }

    /// `-Wvexing-parse` ([dcl.ambig.res]/1, clang's
    /// `warnAboutAmbiguousFunction`): a block-scope function declaration
    /// whose parentheses could have been a direct initializer, with a
    /// note suggesting how to declare a variable instead.
    fn warn_ambiguous_function(&mut self, ds: &DeclSpec, d: &Declarator, ty: QualType) {
        let Some(Chunk::Function { params, loc, rparen, ambiguous: true, .. }) = d.chunks.first()
        else {
            return;
        };
        let Some(ret) = self.ast.types.proto(ty).map(kiln_ast::types::FuncProto::ret) else {
            return;
        };
        let t = &self.ast.types;
        let record = t.as_record(ret);
        // No ambiguity for `void`; a non-class initializer takes at most
        // one expression, a reference exactly one.
        if t.is_void(ret)
            || (record.is_none() && params.len() > 1)
            || (t.is_reference(ret) && params.len() != 1)
        {
            return;
        }
        if !self.in_local_scope() || ds.storage.is_some() || d.ctx == DeclaratorContext::Condition {
            return;
        }
        let parens = Span::new(*loc, rparen.with_offset(1));
        let (id, note) = if let Some(first) = params.first() {
            let (b, e) = first.range;
            let note = self
                .diag(DiagId::note_additional_parens_for_variable_declaration, b)
                .fixit(FixIt::insert(b, "("))
                .fixit(FixIt::insert(e, ")"));
            (DiagId::parse_vexing_parse, Some(note))
        } else {
            // Empty parentheses value-initialize; without them the object
            // is default-initialized. These agree for an empty class (or
            // one with a user-provided default constructor, M3+).
            let defn = record.and_then(|r| self.ast.decls.record(r)).map(|r| r.defn);
            let note = if defn.is_some_and(|b| {
                b.contains(DefinitionBits::COMPLETE) && b.contains(DefinitionBits::EMPTY)
            }) {
                Some(
                    self.diag(DiagId::note_empty_parens_default_ctor, *loc)
                        .fixit(FixIt::remove(parens)),
                )
            } else {
                let init = self.zero_initializer_fixit(ret, defn);
                let init = if init.is_empty() { "{}".to_owned() } else { init };
                Some(
                    self.diag(DiagId::note_empty_parens_zero_initialize, *loc)
                        .fixit(FixIt::replace(parens, init)),
                )
            };
            (DiagId::parse_empty_parens_vexing, note)
        };
        let mut w = self.diag(id, *loc).range(parens);
        if let Some(n) = note {
            w = w.with_note(n);
        }
        self.emit(w);
    }

    /// clang's `getFixItZeroInitializerForType`: ` = 0`, ` = nullptr`,
    /// `{}`, ... ("" when there is none).
    fn zero_initializer_fixit(&self, t: QualType, defn: Option<DefinitionBits>) -> String {
        let types = &self.ast.types;
        if let Some(bits) = defn {
            if !bits.contains(DefinitionBits::COMPLETE) {
                return String::new();
            }
            // No user-provided default constructor in M3: `T x{}`.
            return "{}".to_owned();
        }
        if types.as_record(t).is_some() {
            return String::new();
        }
        if types.as_enum(t).is_some() {
            return String::new();
        }
        let zero = if types.is_floating(t) {
            "0.0"
        } else if types.is_pointer(t) {
            "nullptr"
        } else {
            match types.builtin(types.canonical(t).without_quals()) {
                Some(BuiltinType::Bool) => "false",
                Some(BuiltinType::Char) => "'\\0'",
                Some(BuiltinType::WChar) => "L'\\0'",
                Some(BuiltinType::Char16) => "u'\\0'",
                Some(BuiltinType::Char32) => "U'\\0'",
                _ => "0",
            }
        };
        format!(" = {zero}")
    }

    /// A declarator in a simple-declaration or member-declaration: creates
    /// the variable, function, typedef or field (spec/sema.md §4.2). The
    /// caller then supplies the initializer ([`Sema::act_on_init`] /
    /// [`Sema::act_on_uninit`]) or the function body.
    #[allow(clippy::too_many_lines, reason = "one flat dispatch over declarator kinds")]
    pub fn act_on_declarator(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: &Declarator,
    ) -> Option<DeclId> {
        self.finish_invented_head();
        if ds.friend.is_none()
            && let Some(nloc) = d.name.map(|n| n.1).or(d.special.map(|s| s.1))
        {
            self.check_member_spec_header(d.qualifier, nloc, ds.loc);
        }
        let head = self.take_template_head(d.qualifier);
        if self.check_template_in_local_class(false, ds.lead_cxx11.unwrap_or(ds.loc)) {
            return None;
        }
        if self.templ.head_mismatch {
            // `template<class T> void A<int>::f()`: diagnosed; clang
            // drops the declaration.
            return None;
        }
        let unqualified;
        let mut dropped_local = false;
        let d = match if d.dropped {
            QualifiedDecl::Keep
        } else {
            self.diagnose_qualified_declaration(ds, d, head.is_some())
        } {
            QualifiedDecl::Keep => d,
            QualifiedDecl::Unqualify => {
                unqualified = Declarator { qualifier: None, ..d.clone() };
                &unqualified
            }
            QualifiedDecl::Drop => {
                self.templ.cur = None;
                return None;
            }
            QualifiedDecl::DropLocal => {
                dropped_local = true;
                unqualified = Declarator { qualifier: None, invalid: true, ..d.clone() };
                &unqualified
            }
        };
        if !d.dropped
            && ds.friend.is_none()
            && let Some((SpecialName::DeductionGuide(t), _)) = d.special
        {
            return self.act_on_deduction_guide(ds, base, d, t);
        }
        let special = !d.dropped && self.handles_special_declarator(ds, d);
        let op_d = if special { None } else { self.operator_declarator(d) };
        let d = op_d.as_ref().unwrap_or(d);
        if !d.dropped && d.decomp.is_some() {
            let ty = self.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), d);
            return self.act_on_decomposition_declarator(ds, ty, d, head);
        }
        if d.dropped || !special && self.unsupported_declarator(ds, d) {
            self.templ.cur = None;
            return None;
        }
        // GNU vector attributes among the decl-specifiers apply to the
        // decl-specifier type (clang `processTypeAttrs(TAL_DeclSpec)`),
        // so `__attribute__((vector_size(8))) short f();` returns a vector.
        let base = self.apply_declspec_vector_attrs(base, ds);
        let base = self.ctad_decl_spec_check(ds, base, d);
        self.warn_redundant_parens(Some((ds, base)), d);
        self.declarator_field = d.ctx == DeclaratorContext::Member
            && !ds.is_typedef()
            && ds.friend.is_none()
            && !matches!(ds.storage, Some((StorageSpec::Static, _)));
        let ty = self.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), d);
        self.declarator_field = false;
        let ty = self.check_qualified_fn_type(ty, ds, d);
        if head.is_some() {
            if ds.is_typedef() {
                self.error(DiagId::sema_typedef_template, d.loc);
                self.templ.cur = None;
            } else if d.ctx == DeclaratorContext::Member
                && !self.ast.types.is_function(ty)
                && !matches!(ds.storage, Some((StorageSpec::Static, _)))
            {
                let n = d.name.map_or(String::new(), |n| format!("'{}'", sym_str(self, n.0)));
                let msg = self.diag(DiagId::sema_member_template_field, d.name_loc()).arg(n);
                self.emit(msg);
                self.templ.cur = None;
            }
        }
        let mut invalid = ds.invalid || d.invalid || self.is_error_type(ty);
        // [temp.mem]/2: a member function template of a local class is
        // diagnosed and invalid, so no call selects it (clang's "no
        // matching member function", without candidates).
        invalid |= self.templ.local_template && self.ast.types.is_function(ty);
        if head.is_none() && self.templ.explicit_inst.is_some() && self.at_explicit_head() {
            self.check_explicit_inst_specifiers(ds, ty);
        }
        if special {
            // [temp.variadic]/12: `S(Ts);`, `operator Ts();`.
            if !invalid && ds.friend.is_none() {
                let ty = match d.special {
                    Some((SpecialName::Conversion(t), _)) if self.has_unexpanded_pack(t) => t,
                    _ => ty,
                };
                invalid = self.diagnose_unexpanded_declarator(ty, "declaration type", d.name_loc());
            }
            return self.act_on_special_member_declarator(ds, ty, d, invalid);
        }
        if head.is_none() && !ds.is_typedef() && self.is_function_spec_declarator(d, ty) {
            match self.act_on_function_spec_declarator(ds, ty, d, invalid) {
                Ok(r) => return r,
                // No template to specialize: a plain, invalid function.
                Err(()) => invalid = true,
            }
        }
        if let Some((ext, _)) = self.templ.explicit_inst.filter(|_| self.at_explicit_head()) {
            self.templ.explicit_inst = None;
            // [temp.explicit]/3: of a variable template or static data
            // member.
            let type_loc = ds.type_loc.unwrap_or(ds.loc);
            // [dcl.constexpr]/9: `constexpr` makes the declared type const.
            let ty =
                if ds.constexpr.is_some() { self.ast.types.qualify(ty, Quals::CONST) } else { ty };
            let attrs: Vec<_> = ds.attrs.iter().chain(&d.attrs).copied().collect();
            self.act_on_explicit_var_inst(d, (ty, type_loc), &attrs, ext.is_some(), invalid);
            return None;
        }
        if let Some(sid) = &d.spec_id {
            return self.act_on_var_spec_declarator(ds, ty, d, sid, head);
        }
        if head.is_none()
            && d.qualifier.is_none()
            && !d.global_qualifier
            && !ds.is_typedef()
            && !self.ast.types.is_function(ty)
            && self.at_explicit_head()
            && let (Some(t), Some((n, _))) = (self.templ.explicit_spec, d.name)
        {
            // `template<> int v;`: no template-id, so the head is
            // extraneous and `v` a plain variable (clang's
            // `err_template_variable_noparams`, [temp.expl.spec]/1).
            self.templ.explicit_spec = None;
            let msg = self.diag(DiagId::sema_template_variable_noparams, t).arg(sym_str(self, n));
            self.emit(msg);
        }
        if !invalid {
            self.warn_ambiguous_function(ds, d, ty);
        }
        // [temp.variadic]/12: `Ts x;`, `Ts f();`, `void g(Ts);` (after
        // the vexing-parse warning, which clang's parser emits; a
        // friend's is checked by `act_on_friend`).
        if !invalid && ds.friend.is_none() {
            let what = if d.ctx == DeclaratorContext::Member
                && !ds.is_typedef()
                && !self.ast.types.is_function(ty)
                && !matches!(ds.storage, Some((StorageSpec::Static, _)))
            {
                "data member type"
            } else {
                "declaration type"
            };
            invalid = self.diagnose_unexpanded_declarator(ty, what, d.name_loc());
        }
        let Some((name, name_loc)) = d.name else {
            self.templ.cur = None;
            if d.ctx == DeclaratorContext::Member && d.bit_width.is_some() {
                let f = self.act_on_field(ds, ty, d, invalid);
                self.set_member_access(f);
                return Some(f);
            }
            self.error(DiagId::parse_expected_unqualified_id, d.loc);
            return None;
        };
        if d.qualifier.is_none() {
            self.check_template_param_shadow(name, name_loc);
        }
        let mut attrs = self.non_vector_attrs(&ds.attrs);
        attrs.extend(d.attrs.iter().copied());
        if let Some(l) = ds.noreturn
            && !ds.is_typedef()
            && self.ast.types.is_function(ty)
        {
            // C11 `_Noreturn` (a clang extension in C++): the function's
            // `[[noreturn]]` ([dcl.attr.noreturn]).
            let node = AttrNode {
                kind: AttrKind::NORETURN,
                syntax: AttrSyntax::Keyword,
                flags: 0,
                loc: l,
                a: 0,
                b: 0,
            };
            attrs.push(self.ast.attrs.push(node));
        }
        if let Some(op) = d.op_name
            && (ds.is_typedef() || !self.ast.types.is_function(ty))
        {
            // [over.oper.general]/6: an operator function is a function.
            self.templ.cur = None;
            if !invalid {
                if ds.is_typedef() {
                    self.error(DiagId::sema_typedef_not_identifier, d.name_loc());
                } else {
                    let m = self
                        .diag(DiagId::sema_operator_overload_variable, d.name_loc())
                        .arg(format!("operator{}", op.spelling()));
                    self.emit(m);
                }
            }
            return None;
        }
        if ds.is_typedef() || !self.ast.types.is_function(ty) {
            self.virtual_on_non_function(ds, d);
            // clang's `DiagnoseFunctionSpecifiers`: `_Noreturn` is dropped.
            if let Some(l) = ds.noreturn {
                self.error(DiagId::sema_noreturn_non_function, l);
            }
        }
        let ty = self.apply_vector_attrs(ty, &mut attrs, ds.is_typedef());
        let ty = self.apply_nothrow_attrs(ty, &mut attrs);
        let ty = self.apply_noreturn_attrs(ty, &attrs);
        // [dcl.spec.general]/3: a function or member declared with no
        // type (`y();`, `struct S { y; };`) is an invalid `int` one, as
        // clang's `ConvertDeclSpecToType`: later uses name it silently.
        invalid |= !ds.has_type_specifier();
        let decl = if ds.is_typedef() {
            self.act_on_typedef(ds, ty, d, name, false)
        } else if self.ast.types.is_function(ty) {
            self.act_on_function(ds, ty, d, name, false)
        } else if d.ctx == DeclaratorContext::Member
            && !matches!(ds.storage, Some((StorageSpec::Static, _)))
        {
            self.act_on_field(ds, ty, d, invalid)
        } else {
            // [dcl.spec.general]/3, [dcl.type.general]/2: a variable whose
            // decl-specifier-seq names no type (`const u7;`, the name is
            // the declarator-id) is invalid after "a type specifier is
            // required", as clang's `ConvertDeclSpecToType`, so it is not
            // checked further (no default-initialization error, WP-330).
            // [dcl.constexpr]/6: a constexpr object is const.
            let ty = if ds.constexpr.is_some() && !self.ast.types.is_reference(ty) {
                self.ast.types.qualify(ty, Quals::CONST)
            } else {
                ty
            };
            let v = self.act_on_var(ds, ty, d, name);
            if let Some(tl) = ds.type_loc
                && self.is_ctad_placeholder(ty)
            {
                let _ = self.templ.ctad_var_type_locs.insert(v, tl);
            }
            v
        };
        if invalid {
            self.ast.decls.set_flags(decl, DeclFlags::INVALID);
        } else if d.ctx == DeclaratorContext::Member
            && let Some(w) = d.bit_width
            && self.ast.decls.field(decl).is_none()
        {
            self.non_field_bit_width(ds, ty, name, d.name_loc(), w);
            self.ast.decls.set_flags(decl, DeclFlags::INVALID);
        }
        if dropped_local {
            self.dropped_qualified_locals.insert(decl);
        }
        if let Some(r) = d.trailing_requires {
            self.act_on_trailing_requires(decl, r);
        }
        self.set_member_access(decl);
        let templ = self.finish_template_pattern(decl);
        if let Some(t) = templ {
            self.set_member_access(t);
            if d.qualifier.is_some() && self.templ.member_spec.is_some() {
                self.specialize_member_template(t);
            }
        }
        self.act_on_asm_label(ds, d, decl, &mut attrs);
        if d.qualifier.is_none() {
            // Operator and conversion function names too: a member
            // `operator[]` hides a using-declared base one; a member
            // template hides a base member template, so the template
            // (not its pattern) is compared ([namespace.udecl]/14).
            let dn = self.ast.decls.get(decl).name();
            self.check_using_shadow_conflict(templ.unwrap_or(decl), dn);
        }
        if !attrs.is_empty() {
            self.check_decl_attrs(decl, &mut attrs);
        }
        self.check_optnone_conflicts(decl, &mut attrs);
        self.check_naked_conflicts(decl, &mut attrs);
        self.finish_visibility_attrs(decl, &mut attrs, false);
        self.finish_decl_alignment(decl, &mut attrs);
        self.add_known_function_attrs(decl, &mut attrs);
        self.recognize_builtin_decl(decl, &mut attrs);
        self.record_gnu_lib_twin(decl);
        if !attrs.is_empty() {
            let _ = self.builder().attach_attrs(decl, &attrs);
        }
        Some(decl)
    }

    /// [`Sema::act_on_declarator`] for a constructor, destructor,
    /// conversion function or member `operator=` (WP-067a,
    /// spec/sema-class.md §4.7).
    fn act_on_special_member_declarator(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        invalid: bool,
    ) -> Option<DeclId> {
        // [dcl.constexpr]/2: a destructor is not an immediate function
        // (clang `err_constexpr_dtor`, consteval).
        if let Some(l) = ds.consteval
            && matches!(d.special, Some((SpecialName::Destructor(_), _)))
        {
            self.error(DiagId::sema_consteval_dtor, l);
        }
        let mut invalid = invalid;
        let Some(decl) = self.act_on_special_declarator(ds, ty, d, &mut invalid) else {
            self.templ.cur = None;
            return None;
        };
        if let Some(r) = d.trailing_requires {
            self.act_on_trailing_requires(decl, r);
        }
        self.set_member_access(decl);
        let templ = self.finish_template_pattern(decl);
        if let Some(t) = templ {
            self.set_member_access(t);
            if d.qualifier.is_some() && self.templ.member_spec.is_some() {
                self.specialize_member_template(t);
            }
        }
        if d.qualifier.is_none()
            && matches!(self.kind_of(decl), DeclKind::CXXConversion | DeclKind::CXXMethod)
        {
            // A conversion function hides a using-declared base one with
            // the same target type, a member `operator=` (template) one
            // with the same signature ([namespace.udecl]/14).
            let dn = self.ast.decls.get(decl).name();
            self.check_using_shadow_conflict(templ.unwrap_or(decl), dn);
        }
        let mut attrs = ds.attrs.clone();
        attrs.extend(d.attrs.iter().copied());
        if !attrs.is_empty() {
            self.check_decl_attrs(decl, &mut attrs);
        }
        self.check_optnone_conflicts(decl, &mut attrs);
        self.check_naked_conflicts(decl, &mut attrs);
        self.finish_visibility_attrs(decl, &mut attrs, false);
        self.act_on_asm_label(ds, d, decl, &mut attrs);
        if !attrs.is_empty() {
            let _ = self.builder().attach_attrs(decl, &attrs);
        }
        if !d.defaulted {
            self.check_consteval_only_fn(decl, d.name_loc());
        }
        Some(decl)
    }

    /// The GNU asm label of declarator `d` declaring `decl`, and the one
    /// `decl` inherits from its previous declaration (clang's
    /// `ActOn{Function,Variable}Declarator` and `mergeDeclAttributes`):
    /// prepends the `AsmLabel` attribute to `attrs` (an inherited one
    /// precedes the declaration's own attributes, as clang orders them)
    /// and records the entity's label on its first declaration for the
    /// mangler. Only functions and variables take one; a typedef's is
    /// ignored, as clang ignores it.
    fn act_on_asm_label(
        &mut self,
        ds: &DeclSpec,
        d: &Declarator,
        decl: DeclId,
        attrs: &mut Vec<AttrId>,
    ) {
        let kind = self.ast.decls.get(decl).kind();
        // A member function takes one as a namespace-scope function does
        // (clang `ActOnFunctionDeclarator`), constructors, destructors
        // and conversion functions included (every structor variant
        // takes the label, spec/abi.md §1).
        if !matches!(
            kind,
            DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
                | DeclKind::Var
        ) {
            return;
        }
        let prev_attr = self.ast.prev(decl).and_then(|p| {
            self.ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(p))
                .find(|&a| self.ast.attrs.get(a).kind == AttrKind::ASM_LABEL)
        });
        let Some((e, span)) = d.asm_label else {
            if let Some(a) = prev_attr {
                let mut node = *self.ast.attrs.get(a);
                node.flags |= 2;
                let id = self.ast.attrs.push(node);
                attrs.insert(0, id);
            }
            return;
        };
        let lit = self.ast.exprs.string_lit(e);
        let loc = self.ast.exprs.get(e).loc();
        if let Some(a) = prev_attr {
            let old = *self.ast.attrs.get(a);
            if old.a != lit.raw() {
                let note = self.diag(DiagId::note_previous_declaration, old.loc);
                let msg = self.diag(DiagId::sema_different_asm_label, d.name_loc());
                self.emit(msg.with_note(note));
            }
        } else if self.ast.prev(decl).is_some()
            && self.ast.decls.get(self.ast.first(decl)).flags().contains(DeclFlags::USED)
        {
            let what = if kind == DeclKind::Var { "0" } else { "1" };
            let msg = self.diag(DiagId::sema_late_asm_label, d.name_loc()).arg(what).range(span);
            self.emit(msg);
        }
        // [dcl.asm] GNU: a label on an automatic variable is ignored.
        if kind == DeclKind::Var
            && self.in_local_scope()
            && ds.storage.is_none()
            && ds.thread_local.is_none()
        {
            let text = String::from_utf8_lossy(self.ast.strings.bytes(lit)).into_owned();
            let msg = self.diag(DiagId::sema_asm_label_on_auto_decl, loc).arg(text);
            self.emit(msg);
        }
        if kind == DeclKind::Var && matches!(ds.storage, Some((StorageSpec::Register, _))) {
            self.check_register_variable(ds, decl, lit, loc);
        }
        let node = AttrNode {
            kind: AttrKind::ASM_LABEL,
            syntax: AttrSyntax::Keyword,
            flags: 0,
            loc,
            a: lit.raw(),
            b: 0,
        };
        let id = self.ast.attrs.push(node);
        attrs.insert(0, id);
        let first = self.ast.first(decl);
        let _ = self.ast.decls.cold.asm_label.insert(first, lit);
    }

    /// A GNU register variable `register T v asm("reg")` ([dcl.asm]; clang
    /// `ActOnVariableDeclarator`). At block scope the register name must be
    /// one the target knows; at file scope also one the target can hold a
    /// global variable in, of the variable's size, and the type must be
    /// integral or a pointer (else the variable is invalid). A dependent
    /// type's size is not checked.
    fn check_register_variable(&mut self, ds: &DeclSpec, v: DeclId, lit: StringLitId, loc: Loc) {
        let arch = self.abi.triple.arch;
        let name = String::from_utf8_lossy(self.ast.strings.bytes(lit)).into_owned();
        let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        if !kiln_abi::asm::is_valid_register_name(arch, &name) {
            self.emit(self.diag(DiagId::sema_asm_unknown_register_name, loc).arg(name));
        } else if !self.in_local_scope() && !self.is_dependent_type(ty) {
            let bits = target::size_align(&self.ast, ty, self.abi()).map(|(s, _)| s * 8);
            let size = bits.map(|b| kiln_abi::asm::global_register_size_mismatch(arch, &name, b));
            let unreserved =
                self.clang_compat >= 22 && !kiln_abi::asm::global_register_reserved_22(arch, &name);
            match size {
                Some(Some(_)) if unreserved => {
                    let msg = self.diag(DiagId::sema_asm_invalid_global_var_reg, loc).arg(name);
                    self.emit(msg);
                }
                Some(None) => {
                    let msg = self.diag(DiagId::sema_asm_invalid_global_var_reg, loc).arg(name);
                    self.emit(msg);
                }
                Some(Some(true)) => {
                    let msg = self.diag(DiagId::sema_asm_register_size_mismatch, loc).arg(name);
                    self.emit(msg);
                }
                _ => {}
            }
        }
        // A dependent type is neither integral nor a pointer (clang 19
        // diagnoses it on x86_64 and crashes on AArch64).
        if self.in_local_scope() || self.is_error_type(ty) {
            return;
        }
        let t = &self.ast.types;
        if !t.is_integral(ty) && !t.is_pointer(ty) {
            // clang 22 reports at the type specifier, clang 19 at the
            // decl-specifier-seq.
            let at = if self.clang_compat >= 22 { ds.type_loc.unwrap_or(ds.loc) } else { ds.loc };
            self.error(DiagId::sema_asm_bad_register_type, at);
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
    }

    /// Reports an unsupported construct (`{what} is not supported yet`).
    pub fn unsupported(&self, what: &str, loc: Loc) {
        let d = self.diag(DiagId::parse_unsupported, loc).arg(what);
        self.emit(d);
    }

    /// Whether function types `a` and `b` are the same apart from their
    /// exception specifications and GNU `noreturn` (which declarations of
    /// one function merge, clang `MergeFunctionDecl`).
    pub(crate) fn same_ignoring_exc_spec(&mut self, a: QualType, b: QualType) -> bool {
        let t = &self.ast.types;
        let (Some(pa), Some(pb)) = (t.proto(a).copied(), t.proto(b).copied()) else {
            return false;
        };
        let (ra, rb) = (t.params(&pa).to_vec(), t.params(&pb).to_vec());
        let (ia, ib) = (pa.info().with_noreturn(false), pb.info().with_noreturn(false));
        let a = self.ast.types.function(pa.ret(), &ra, ia, ExcSpec::None);
        let b = self.ast.types.function(pb.ret(), &rb, ib, ExcSpec::None);
        self.ast.types.same_type(a, b)
    }

    /// The exception specification of function type `q` as written (the
    /// canonical one when `q` is sugar over the function type).
    pub(crate) fn written_exc_spec(&self, q: QualType) -> ExcSpec {
        let t = &self.ast.types;
        t.proto_of(q.ty()).or_else(|| t.proto(q)).map_or(ExcSpec::None, |p| t.exc_spec(p))
    }

    /// `exc` spelled as in a function type (`noexcept`, `throw()`,
    /// `noexcept(expr)`), without the leading space.
    pub(crate) fn exc_spec_spelling(&self, exc: ExcSpec) -> String {
        match exc {
            ExcSpec::DynamicNone => "throw()".to_owned(),
            ExcSpec::NothrowAttr => "__attribute__((nothrow))".to_owned(),
            ExcSpec::NoexceptExpr { expr, .. } => {
                let mut s = String::from("noexcept(");
                kiln_ast::types::TypeNames::write_expr(&self.ast, expr, &mut s);
                s.push(')');
                s
            }
            _ => "noexcept".to_owned(),
        }
    }

    pub(crate) fn note_prev(
        &self,
        msg: kiln_diag::Diag,
        prev: DeclId,
        definition: bool,
    ) -> kiln_diag::Diag {
        let loc = self.ast.decls.get(prev).loc();
        let id = if definition {
            DiagId::note_previous_definition
        } else {
            DiagId::note_previous_declaration
        };
        let msg = msg.with_note(self.diag(id, loc));
        // A member `define_aggregate` injected (spec/reflection.md §6.2).
        match self.ast.decls.cold.injection_point.get(prev) {
            Some(&at) => msg.with_note(self.diag(DiagId::note_kiln_injected_here, at)),
            None => msg,
        }
    }

    /// The qualified name of member `p` as clang's
    /// `getQualifiedNameAsString` prints it: a partial specialization
    /// context with its canonical arguments (`'P<type-parameter-0-0,
    /// int>::g'`), a class template's pattern without any (`'D::g'`).
    fn member_qualified_name(&self, p: DeclId) -> String {
        let q = self.qualified_decl_name(p);
        let Some(pc) = self.ast.decls.get(p).ctx() else { return q };
        let Some((_, info)) = self.ast.partial_spec_info(self.ast.first(pc)) else { return q };
        // Both clangs print the partial specialization's canonical
        // arguments here (its qualified name, not a type).
        let mut desc = self.decl_name(pc);
        let names = crate::names::SemaNames::new(self);
        let args = self.ast.types.canonical_args(info.args);
        kiln_ast::types::write_arg_list(&self.ast.types, &names, args, &mut desc);
        let outer = self.qualified_decl_name(pc);
        let outer = outer.rsplit_once("::").map_or(String::new(), |(o, _)| format!("{o}::"));
        format!("{outer}{desc}::{}", self.decl_name(p))
    }

    /// True if member function `p` and the function type `ty` differ in
    /// their return types only ([over.load]/2).
    fn differs_in_return(&mut self, p: DeclId, ty: QualType) -> bool {
        if self.kind_of(p) != DeclKind::CXXMethod {
            return false;
        }
        let Some(pty) = self.declared_fn_type(p) else { return false };
        match (self.ast.types.proto(pty), self.ast.types.proto(ty)) {
            (Some(a), Some(b)) => !self.ast.types.same_type(a.ret(), b.ret()),
            _ => false,
        }
    }

    /// Adds clang's "forward declaration of 'X'" note to an
    /// incomplete-type error when `ty` (or its array element) is a
    /// declared but undefined class ([basic.types.general]/5).
    pub(crate) fn incomplete_note(&self, msg: kiln_diag::Diag, ty: QualType) -> kiln_diag::Diag {
        let mut t = ty;
        while self.ast.types.is_array(t) {
            match self.ast.types.element(t) {
                Some(e) => t = e,
                None => break,
            }
        }
        let Some(rec) = self.ast.types.as_record(t) else { return msg };
        if let Some(def) = self.ast.definition(rec) {
            // Still being defined (clang `RequireCompleteType` on a tag
            // that `isBeingDefined`).
            if self.defining.contains(&def) {
                // An invalid class (a member of incomplete type) is not
                // reported again (clang).
                if self.invalid_defining.contains(&def) {
                    self.inst.suppress_next_error.set(true);
                    return msg;
                }
                let note = self
                    .diag(DiagId::note_definition_not_complete, self.ast.decls.get(def).loc())
                    .arg(format!("'{}'", self.qualified_decl_name(def)));
                return msg.with_note(note);
            }
            return msg;
        }
        if self.inst.undefined_diagnosed.get() == Some(rec) {
            self.inst.undefined_diagnosed.set(None);
            self.inst.suppress_next_error.set(true);
            return msg;
        }
        // A member class of an implicit specialization declared
        // explicitly specialized: clang moves the instantiation's
        // location to that declaration ([temp.expl.spec]/1).
        let explicit_member = self
            .ast
            .member_spec_info(self.ast.first(rec))
            .is_some_and(|(_, m)| m.kind == kiln_ast::templates::SpecKind::ExplicitSpecialization);
        let at = if explicit_member { self.ast.latest(rec) } else { rec };
        let loc = self.ast.decls.get(at).loc();
        // A specialization prints with its arguments (`'A<char>'`).
        let name = if self.ast.spec_info(rec).is_some() {
            self.decl_ty_str(self.tag_type(rec))
        } else {
            self.qualified_decl_name(rec)
        };
        msg.with_note(self.diag(DiagId::note_forward_declaration, loc).arg(format!("'{name}'")))
    }

    /// P2169: `name` is the placeholder `_` ([basic.scope.scope]/5).
    pub(crate) fn is_placeholder_name(&self, name: Symbol) -> bool {
        self.ast.symbols().as_str(name) == "_"
    }

    /// A name-independent declaration ([basic.scope.scope]/5) at `loc`
    /// that redeclares `_` in its scope: valid, and before C++26 clang's
    /// `-Wc++26-extensions` warning.
    pub(crate) fn diagnose_placeholder_redecl(&self, loc: Loc) {
        if self.std < kiln_lex::Std::Cxx26 {
            self.error(DiagId::sema_ext_placeholder_var, loc);
        }
    }

    pub(crate) fn register_local(&mut self, name: Symbol, d: DeclId) {
        if self.in_local_scope() {
            self.push_local(name, d);
        }
    }

    /// A typedef-name redeclared by a typedef or alias-declaration of
    /// `name` at `loc` with type `underlying`, the previous one being `p`
    /// (`alias_template`: both are alias template patterns). The previous
    /// declaration to chain to, `None` after a diagnostic.
    fn check_typedef_redefinition(
        &mut self,
        p: DeclId,
        underlying: QualType,
        alias_template: bool,
        name: Symbol,
        loc: Loc,
    ) -> Option<DeclId> {
        let pk = self.ast.decls.get(p).kind();
        let pty = self.ast.decls.typedef(p).map_or(QualType::ERROR, |t| t.underlying);
        // [dcl.typedef]/4 with [temp.over.link]/5: dependent types compare
        // by their canonical profiles. A redefinition that is not an alias
        // template's is checked only when neither type is dependent
        // (clang's `isIncompatibleTypedef`); a mismatch is then diagnosed
        // at instantiation.
        let t = &self.ast.types;
        let deferred = !alias_template && (t.is_dependent(pty) || t.is_dependent(underlying));
        if !deferred && !t.same_type(pty, underlying) {
            // The kind is the previous declaration's (clang).
            let msg = self
                .diag(DiagId::sema_redefinition_different_typedef, loc)
                .arg(match (pk, alias_template) {
                    (_, true) => "2",
                    (DeclKind::TypeAlias, false) => "1",
                    (_, false) => "0",
                })
                .diff_args(self, underlying, pty);
            let msg = self.note_prev(msg, p, true);
            self.emit(msg);
            return None;
        }
        if self.kind_of(self.ctx) == DeclKind::CXXRecord {
            // [dcl.typedef]/4, CWG56/424: a typedef-name is not redeclared
            // in class scope.
            let msg = self.diag(DiagId::sema_redefinition, loc).arg(sym_str(self, name));
            let msg = self.note_prev(msg, p, true);
            self.emit(msg);
            return None;
        }
        Some(p)
    }

    /// `typedef T name;` or `using name = T;` ([dcl.typedef]).
    fn act_on_typedef(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        name: Symbol,
        alias: bool,
    ) -> DeclId {
        if let Some((s, loc)) = ds.storage.filter(|(s, _)| *s != StorageSpec::Typedef) {
            let _ = s;
            self.error(DiagId::sema_storage_class_on_typedef, loc);
        }
        let (ctx, lexical) = self.decl_contexts(d);
        // An earlier redefinition that was diagnosed is not the previous
        // declaration (as in clang, which does not chain invalid
        // redeclarations): the later one is compared with the valid one.
        let prev = {
            let found: Vec<DeclId> = match d.qualifier {
                Some(q) => self.lookup_qualified(q, name, LookupKind::Ordinary).decls,
                None => self.lookup_current_scope(name),
            };
            found.into_iter().rev().find(|&p| {
                let n = self.ast.decls.get(p);
                let invalid_typedef = matches!(n.kind(), DeclKind::Typedef | DeclKind::TypeAlias)
                    && n.has(DeclFlags::INVALID);
                !is_tag_kind(n.kind()) && !invalid_typedef
            })
        };
        // `typedef struct { } T;`: T names the class for linkage purposes
        // ([dcl.typedef]/9).
        let anon_tag = ds.tag.filter(|&t| {
            ds.tag_declares
                && d.chunks.is_empty()
                && ds.cv.is_empty()
                && self.ast.decls.get(t).name().is_empty()
                && self.ast.decls.typedef_for_anon(t).is_none()
        });
        let underlying = match anon_tag {
            Some(t) => {
                let tag_ty = self.ast.decls.get(self.ast.first(t)).ty().unwrap_or(QualType::ERROR);
                self.ast.types.elaborated(ds.keyword, None, tag_ty)
            }
            None => ty,
        };
        let mut prev_ok = None;
        // An alias template redeclared ([temp.alias], clang's
        // `ActOnAliasDeclaration`): the same type through equivalent
        // parameters (canonical types compare by depth and index) chains
        // to the earlier pattern.
        let alias_template_prev = prev.and_then(|p| {
            (alias && self.templ.cur.is_some() && self.kind_of(p) == DeclKind::TypeAliasTemplate)
                .then(|| self.pattern_of(p))
                .flatten()
        });
        // [temp.over.link]/6: the heads must be equivalent first (clang
        // drops a redeclaration whose head differs).
        let heads_differ = match (alias_template_prev, self.templ.cur, prev) {
            (Some(_), Some(new), Some(t)) => {
                let old = self.ast.template_info(t).map(|i| i.params);
                old.is_some_and(|old| !self.check_template_heads_match(new, old))
            }
            _ => false,
        };
        let mut invalid = heads_differ;
        let prev = if heads_differ { None } else { alias_template_prev.or(prev) };
        if let Some(p) = prev {
            let pk = self.ast.decls.get(p).kind();
            if matches!(pk, DeclKind::Typedef | DeclKind::TypeAlias)
                && (alias_template_prev.is_some() == self.templ.cur.is_some())
            {
                match self.check_typedef_redefinition(
                    p,
                    underlying,
                    alias_template_prev.is_some(),
                    name,
                    d.name_loc(),
                ) {
                    Some(p) => prev_ok = Some(p),
                    None => invalid = true,
                }
            } else {
                let msg = self
                    .diag(DiagId::sema_redefinition_different_kind, d.name_loc())
                    .arg(sym_str(self, name));
                let msg = self.note_prev(msg, p, true);
                self.emit(msg);
            }
        }
        let kind = if alias { DeclKind::TypeAlias } else { DeclKind::Typedef };
        let mut h = DeclHead::new(ctx, DeclName::ident(name), d.name_loc());
        h.lexical = Some(lexical);
        self.mark_pattern_next();
        let td = self.builder().typedef(h, kind, underlying, prev_ok);
        if underlying.quals().is_empty()
            && matches!(
                self.ast.types.view(underlying.ty()),
                TypeView::Vector { kind: kiln_ast::types::VectorKind::Ext, .. }
            )
        {
            self.ext_vector_typedefs.push(td);
        }
        // The decl-specifiers' start: an instantiation finds the type's
        // dependent names from there (§5.8), as in `typedef int
        // (*f)(typename T::type);`.
        if !alias
            && ds.loc.is_valid()
            && ds.loc != d.name_loc()
            && self.is_instantiation_dependent_type(underlying)
        {
            let _ = self.ast.decls.cold.begin_loc.insert(td, ds.loc);
            self.note_type_spec_loc(td, ds, d.name_loc());
        }
        self.note_ref_loc(td, d, underlying);
        if invalid {
            self.ast.decls.set_flags(td, DeclFlags::INVALID);
        }
        if let Some(t) = anon_tag {
            let _ = self.ast.decls.set_typedef_for_anon(t, td);
        }
        self.register_local(name, td);
        td
    }

    /// Where the type-constraint of the constrained placeholder in the
    /// type `ds d` declares is written: the decl-specifier's, else one in
    /// a trailing return type (`auto (*v)() -> C auto`).
    pub(crate) fn placeholder_constraint_loc(ds: &DeclSpec, d: &Declarator) -> Option<Loc> {
        if ds.auto_constraint.is_some() && ds.constraint_loc.is_valid() {
            Some(ds.constraint_loc)
        } else {
            d.trailing_constraint.is_valid().then_some(d.trailing_constraint)
        }
    }

    /// The outermost `&`/`&&` of `d` when its declared type is a
    /// reference (its first chunk), else invalid.
    pub(crate) fn outer_ref_loc(d: &Declarator) -> Loc {
        match d.chunks.first() {
            Some(Chunk::LRef { loc } | Chunk::RRef { loc }) => *loc,
            _ => Loc::INVALID,
        }
    }

    /// The innermost `*` or `[` of `d`, when that chunk applies to the
    /// decl-specifiers ([`Sema::inner_chunk_locs`]); invalid otherwise.
    pub(crate) fn inner_chunk_loc(d: &Declarator) -> Loc {
        match d.chunks.last() {
            Some(Chunk::Pointer { loc, .. } | Chunk::Array { loc, .. }) => *loc,
            _ => Loc::INVALID,
        }
    }

    /// Records the outermost `&`/`&&` ([`Sema::ref_locs`]) and the
    /// innermost `*`/`[` ([`Sema::inner_chunk_locs`]) of declaration `id`
    /// of dependent type `ty` declared by `d`.
    pub(crate) fn note_ref_loc(&mut self, id: DeclId, d: &Declarator, ty: QualType) {
        let (l, c) = (Self::outer_ref_loc(d), Self::inner_chunk_loc(d));
        if (l.is_valid() || c.is_valid()) && self.is_instantiation_dependent_type(ty) {
            if l.is_valid() {
                self.ref_locs.insert(id, l);
            }
            if c.is_valid() {
                self.inner_chunk_locs.insert(id, c);
            }
        }
    }

    /// Whether return type `ret` is const-qualified after its reference
    /// (`const T &`): an assignment to the call names it
    /// ([`Sema::type_spec_locs`]).
    fn returns_const(&self, ret: QualType) -> bool {
        let t = &self.ast.types;
        let p = match t.canonical_view(ret) {
            TypeView::LValueRef(p) | TypeView::RValueRef(p) => p,
            _ => ret,
        };
        t.effective_quals(p).is_const()
    }

    /// `using name = type-id;` ([dcl.typedef]/2).
    /// Records where `d`'s first type-specifier was written (when not at
    /// its name `name`) for a substituted declarator's function type
    /// ([`Sema::type_spec_locs`]); the caller checks the type is dependent.
    pub(crate) fn note_type_spec_loc(&mut self, d: DeclId, ds: &DeclSpec, name: Loc) {
        if let Some(tl) = ds.type_loc
            && tl.is_valid()
            && tl != name
        {
            self.type_spec_locs.insert(d, tl);
        }
    }

    /// `using name = type-id;` ([dcl.typedef]/2); `type_loc` is where
    /// the type-id starts (invalid when not written).
    pub fn act_on_alias_decl(
        &mut self,
        name: Symbol,
        name_loc: Loc,
        ty: QualType,
        attrs: &[AttrId],
        type_loc: Loc,
    ) -> DeclId {
        if let Some(id) = self.pending_alias_vla.take() {
            self.error(id, name_loc);
        }
        let mut d = Declarator::new(DeclaratorContext::File, name_loc);
        d.name = Some((name, name_loc));
        let ds = DeclSpec::new(name_loc);
        let _ = self.take_template_head(None);
        let _ = self.check_template_in_local_class(false, Loc::INVALID);
        let mut attrs = attrs.to_vec();
        let ty = self.apply_vector_attrs(ty, &mut attrs, true);
        let td = self.act_on_typedef(&ds, ty, &d, name, true);
        // [temp.variadic]/12: `using U = Ts;`.
        if self.diagnose_unexpanded_declarator(ty, "declaration type", name_loc) {
            self.declarator_type_invalid = false;
            self.ast.decls.set_flags(td, DeclFlags::INVALID);
        }
        if type_loc.is_valid() && self.is_instantiation_dependent_type(ty) {
            self.type_spec_locs.insert(td, type_loc);
        }
        let chunk = std::mem::replace(&mut self.pending_chunk_loc, Loc::INVALID);
        if chunk.is_valid() && self.is_instantiation_dependent_type(ty) {
            self.inner_chunk_locs.insert(td, chunk);
        }
        // [class.access.spec]/1: a member alias-declaration has the
        // current access, as a member typedef does.
        self.set_member_access(td);
        if let Some(t) = self.finish_template_pattern(td) {
            self.set_member_access(t);
        }
        self.check_using_shadow_conflict(td, DeclName::ident(name));
        if !attrs.is_empty() {
            // The declarator's subject table ([dcl.attr], spec/sema.md
            // §4.5): an alias-declaration declares a typedef-name.
            self.check_decl_attrs(td, &mut attrs);
            let _ = self.builder().attach_attrs(td, &attrs);
        }
        td
    }

    #[allow(
        clippy::too_many_lines,
        reason = "[dcl.fct.spec], [dcl.link] and redeclaration checks in order"
    )]
    /// A function declarator; `friend` for the declarator of a friend
    /// declaration ([class.friend]/6-7): an unqualified one declares a
    /// function of the innermost enclosing namespace, lexically in the
    /// class, hidden from ordinary lookup unless declared there before
    /// ([namespace.memdef]/3).
    pub(crate) fn act_on_function(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        name: Symbol,
        friend: bool,
    ) -> DeclId {
        let (ctx, lexical) = self.decl_contexts(d);
        let ctx = if friend && d.qualifier.is_none() {
            self.enclosing_namespace().unwrap_or(self.ast.tu)
        } else {
            ctx
        };
        // A member function ([class.mfct]): declared in its class, or
        // defined out of line with the class as qualifier.
        let method = self.kind_of(ctx) == DeclKind::CXXRecord;
        let ctx = if method { self.ast.definition(ctx).unwrap_or(ctx) } else { ctx };
        let in_class = method && d.qualifier.is_none() && !friend;
        let mut spec = FnSpec::new();
        let block = self.in_local_scope();
        let mut invalid_decl = false;
        match ds.storage {
            Some((StorageSpec::Static, loc)) if method && !in_class => {
                self.error(DiagId::sema_static_out_of_class, loc);
            }
            Some((StorageSpec::Static, loc)) => {
                if block {
                    self.error(DiagId::sema_static_block_func, loc);
                } else {
                    spec = spec.with_storage(FnStorage::Static);
                }
            }
            Some((StorageSpec::Extern, loc)) if method => {
                self.error(DiagId::sema_storage_class_for_member, loc);
            }
            Some((StorageSpec::Extern, _)) => spec = spec.with_storage(FnStorage::Extern),
            // In a member declaration clang diagnoses the specifier and
            // keeps the declaration valid; elsewhere (namespace or block
            // scope, out-of-class member definitions) the declaration is
            // invalid (clang's `getFunctionStorageClass`, "illegal storage
            // class on function").
            Some((StorageSpec::Mutable, loc)) if in_class => {
                self.error(DiagId::sema_mutable_function, loc);
            }
            Some((StorageSpec::Register, loc)) if in_class => {
                self.error(DiagId::sema_storage_class_for_member, loc);
            }
            Some((StorageSpec::Register | StorageSpec::Mutable, loc)) => {
                self.error(DiagId::sema_illegal_storage_class_on_function, loc);
                invalid_decl = true;
            }
            _ => {}
        }
        // [class.free]/1: class-specific allocation and deallocation
        // functions are static members.
        if method
            && d.op_name.is_some_and(|op| {
                matches!(
                    op,
                    OverloadedOperator::New
                        | OverloadedOperator::ArrayNew
                        | OverloadedOperator::Delete
                        | OverloadedOperator::ArrayDelete
                )
            })
        {
            spec = spec.with_storage(FnStorage::Static);
        }
        let explicit_object = d
            .function_params()
            .and_then(<[ParamInfo]>::first)
            .is_some_and(|p| p.explicit_object.is_some());
        // clang diagnoses an explicit object parameter of a namespace- or
        // block-scope function as it parses the parameter, before its
        // qualifiers (a friend's, in its class, after them).
        let early_explicit_object = (!method && !friend && explicit_object).then(|| {
            let params = d.function_params().unwrap_or_default();
            self.explicit_object_member(ds, params, false)
        });
        let fn_chunk = matches!(d.chunks.first(), Some(Chunk::Function { .. }));
        let ty = if (!method || explicit_object) && fn_chunk {
            self.drop_invalid_fn_quals(ty, d, method)
        } else if in_class && spec.storage() == FnStorage::Static {
            self.check_static_method_quals(ty, d)
        } else {
            ty
        };
        // [except.spec]/9: a class-specific deallocation function without
        // an exception specification is non-throwing.
        let ty = match d.op_name {
            Some(OverloadedOperator::Delete | OverloadedOperator::ArrayDelete)
                if method && self.written_exc_spec(ty) == ExcSpec::None =>
            {
                self.with_basic_noexcept(ty)
            }
            _ => ty,
        };
        if ds.inline.is_some() {
            spec = spec.with_inline(true);
        }
        if ds.constexpr.is_some() {
            spec = spec.with_constexpr_kind(ConstexprKind::Constexpr).with_implicit_inline(true);
        } else if ds.consteval.is_some() {
            spec = spec.with_constexpr_kind(ConstexprKind::Consteval).with_implicit_inline(true);
        }
        let variadic = self.ast.types.proto(ty).is_some_and(|p| p.info().variadic());
        spec = spec.with_variadic(variadic);
        // [basic.start.main]/2: `main` returns `int`.
        if sym_str(self, name) == "main"
            && self.ast.redecl_context(ctx) == self.ast.tu
            && let Some(p) = self.ast.types.proto(ty)
        {
            let ret = p.ret();
            if !self.ast.types.same_type(ret, QualType::INT) && !self.is_error_type(ret) {
                self.error(DiagId::sema_main_returns_nonint, ds.type_loc.unwrap_or(ds.loc));
            }
            // [basic.start.main]/3: `main` is not constexpr or consteval;
            // clang drops the specifier.
            if let Some((l, which)) =
                ds.constexpr.map(|l| (l, 0u8)).or(ds.consteval.map(|l| (l, 1u8)))
            {
                let m = self.diag(DiagId::sema_constexpr_main, l).arg(which);
                self.emit(m);
                spec = spec.with_constexpr_kind(ConstexprKind::None).with_implicit_inline(false);
            }
        }
        let mut prev_ok = None;
        // A function with an invalid parameter (a `void` parameter, an
        // erroneous default argument) is invalid (clang's
        // `ActOnFunctionDeclarator`).
        invalid_decl |= d.function_params().is_some_and(|ps| ps.iter().any(|p| p.invalid));
        // So is one whose decl-specifiers name no type (`f(int);`,
        // `R::h(int);` after "a type specifier is required", or after an
        // unknown type name; WP-411).
        invalid_decl |= d.special.is_none() && (ds.type_error || !ds.has_type_specifier());
        let mut invalid_redecl = invalid_decl;
        // A redeclaration of an invalid declaration (below).
        let mut prev_invalid = false;
        let late_exc =
            d.late_noexcept.is_some() || matches!(self.written_exc_spec(ty), ExcSpec::Deferred(_));
        let mut delayed_prev = false;
        // A non-dependent friend of a templated class: the function it
        // would redeclare once instantiated, whose exception
        // specification it is checked against now (`pattern_friend_prev`).
        let mut pattern_prev = None;
        // The non-function member an out-of-line member function
        // declaration names (`X S::x();` for `static X x;`).
        let mut other_kind_member = None;
        let fkind = if method { DeclKind::CXXMethod } else { DeclKind::Function };
        let new_fn = NewFn {
            ty,
            is_static: spec.storage() == FnStorage::Static,
            head: self.templ.cur,
            requires: d.trailing_requires,
            enable_if: self.new_fn_enable_if(&d.attrs),
            member_using: false,
        };
        let dname = d.fn_decl_name(name);
        let fname = match d.op_name {
            // clang's `DeclarationName` printing: `operator new`.
            Some(op) if op.spelling().starts_with(char::is_alphabetic) => {
                format!("operator {}", op.spelling())
            }
            Some(op) => format!("operator{}", op.spelling()),
            None => self.decl_name_str(dname),
        };
        // A replaceable global allocation or deallocation function
        // redeclares the implicit one ([new.delete]/2).
        let global_new_delete = d.op_name.is_some_and(|op| {
            !method
                && !friend
                && matches!(
                    op,
                    OverloadedOperator::New
                        | OverloadedOperator::ArrayNew
                        | OverloadedOperator::Delete
                        | OverloadedOperator::ArrayDelete
                )
        });
        // A qualified friend of a namespace function in a templated class
        // whose own head or type is dependent is matched only when the class
        // is instantiated: its template parameters are a level deeper than
        // the namespace template's, and the match may depend on the class's
        // arguments ([temp.friend]/1, [temp.over.link]/6). A name that finds
        // nothing is diagnosed now (clang `ActOnFunctionDeclarator`).
        let templated_friend =
            friend && d.qualifier.is_some() && !method && self.in_templated_class(Some(lexical));
        let deferred_friend =
            templated_friend && (self.templ.cur.is_some() || self.ast.types.is_dependent(ty)) && {
                let found = self.prev_candidates_dn(dname, d.qualifier);
                !found.is_empty()
                    && found.iter().all(|&x| {
                        let k = self.kind_of(self.ast.strip_shadow(x));
                        k.payload() == kiln_ast::decl::PayloadClass::Function
                            || k == DeclKind::FunctionTemplate
                    })
            };
        // An invalid local-class friend redeclares nothing (clang returns
        // after `DiagnoseInvalidRedeclaration`).
        let local_friend = if friend && d.qualifier.is_none() {
            self.check_local_friend(dname, new_fn, d.name_loc())
        } else {
            LocalFriend::NotLocal
        };
        let bad_local_friend = local_friend == LocalFriend::Invalid;
        invalid_redecl |= bad_local_friend;
        // An invalid declaration (an illegal storage class, an invalid
        // parameter) redeclares nothing either: clang checks it against no
        // previous declaration (`ActOnFunctionDeclarator`).
        let prev = if global_new_delete || deferred_friend || invalid_decl || bad_local_friend {
            None
        } else if let LocalFriend::Redeclares(p) = local_friend
            && !self.in_templated_class(Some(lexical))
        {
            // The block-scope declaration it redeclares ([class.friend]/11).
            Some(p)
        } else if friend && d.qualifier.is_none() && self.in_templated_class(Some(lexical)) {
            // A friend of a templated class redeclares nothing until
            // instantiated (see `is_dependent_friend_fn`); a non-dependent
            // one is checked against the using-declarators now (clang).
            if new_fn.head.is_none() && !self.ast.types.is_dependent(ty) {
                let shadows: Vec<DeclId> = self
                    .ast
                    .lookup_local(ctx, dname)
                    .iter_with_inline()
                    .filter(|&(x, via_inline)| {
                        !via_inline && self.kind_of(x) == DeclKind::UsingShadow
                    })
                    .map(|(x, _)| x)
                    .collect();
                invalid_redecl |=
                    self.friend_befriends_using_target(&shadows, new_fn, d.name_loc());
                if !invalid_redecl {
                    pattern_prev = self.pattern_friend_prev(ctx, dname, new_fn);
                }
            }
            None
        } else if friend && d.qualifier.is_none() {
            // Redeclaration lookup in the namespace sees hidden friends,
            // but not the members of its inline namespaces: those are in
            // other scopes ([dcl.meaning.general]/2.3, [namespace.memdef]/3;
            // clang's `FilterLookupForScope` without `AllowInlineNamespace`).
            let mut found = self.hidden_friend_redecl_set(ctx, dname, &new_fn);
            // A using-declarator of the namespace is not redeclared: the
            // friend overloads its target, or, corresponding to it, cannot
            // befriend it ([namespace.udecl]/10, [basic.scope.scope]/4;
            // clang `err_using_decl_friend`).
            let shadows: Vec<DeclId> = found
                .iter()
                .copied()
                .filter(|&x| self.kind_of(x) == DeclKind::UsingShadow)
                .collect();
            found.retain(|&x| self.kind_of(x) != DeclKind::UsingShadow);
            let prev = self.pick_prev_function(&found, new_fn);
            if prev.is_none() {
                invalid_redecl |=
                    self.friend_befriends_using_target(&shadows, new_fn, d.name_loc());
            }
            prev
        } else if method {
            let (p, other) =
                self.member_redecl_prev(dname, d.qualifier, new_fn, !in_class && !friend);
            other_kind_member = other;
            p
        } else {
            self.find_prev_function(dname, d.qualifier, new_fn)
        };
        // [dcl.link]/6: a C-linkage function of another scope.
        let mut extern_c_redecl = false;
        let prev = match prev {
            None if !method
                && !friend
                && !invalid_decl
                && d.qualifier.is_none()
                && self.templ.cur.is_none()
                && dname.as_ident().is_some() =>
            {
                let p = self.extern_c_prev(name, false, new_fn.is_static, d.name_loc());
                extern_c_redecl = p.is_some();
                p
            }
            p => p,
        };
        // `template<class T> struct X; void X();` (clang's IDNS
        // redeclaration lookup; `kind_conflict`).
        if prev.is_none()
            && !method
            && !friend
            && d.qualifier.is_none()
            && self.templ.cur.is_none()
            && dname.as_ident().is_some()
        {
            invalid_redecl |=
                self.diagnose_template_kind_conflict(name, NewKind::Function, d.name_loc());
        }
        let matches_member = prev.is_some_and(|p| {
            self.kind_of(p) == DeclKind::CXXMethod
                && self.ast.decls.get(p).ty().is_some()
                && self.declared_fn_type(p).is_some_and(|pty| self.same_ignoring_exc_spec(pty, ty))
        });
        // An instantiated qualified friend template that matches no
        // template declares nothing visible, without a diagnostic (clang).
        let quiet_friend = deferred_friend
            || friend
                && d.qualifier.is_some()
                && !method
                && prev.is_none()
                && new_fn.head.is_some()
                && self.instantiating_friend_template;
        let mut ty = ty;
        if let Some(op) = d.op_name
            && global_new_delete
        {
            let (t, p) = self.act_on_global_new_delete_decl(op, ty);
            ty = t;
            prev_ok = p;
            if let Some(p) = p {
                let pty = self.declared_fn_type(p).unwrap_or(QualType::ERROR);
                if !self.ast.types.same_type(pty, ty)
                    && self.written_exc_spec(ty) == ExcSpec::None
                    && !self.ast.decls.get(p).has(DeclFlags::INVALID)
                {
                    let delete =
                        matches!(op, OverloadedOperator::Delete | OverloadedOperator::ArrayDelete);
                    let replaceable = self.is_replaceable_global_alloc(p, delete);
                    let (t, err) = self.merge_missing_exc_spec(
                        p,
                        pty,
                        ty,
                        (d.name_loc(), &fname, crate::except::missing_exc_spec_fixit_loc(d)),
                        replaceable,
                    );
                    ty = t;
                    if err {
                        prev_ok = None;
                        invalid_redecl = true;
                    }
                }
            }
        } else if invalid_decl {
            // Already diagnosed and invalid; matched against nothing.
        } else if prev.is_some_and(|p| {
            self.ast.decls.get(p).has(DeclFlags::INVALID)
                && self.kind_of(p).payload() == kiln_ast::decl::PayloadClass::Function
        }) {
            // A redeclaration of an invalid declaration is itself invalid,
            // without a diagnostic (clang's `MergeFunctionDecl` gives up on
            // an invalid old declaration); `pick_prev_function` prefers a
            // valid corresponding declaration.
            invalid_redecl = true;
            prev_invalid = true;
        } else if friend
            && d.qualifier.is_some()
            && !quiet_friend
            && (prev.is_none() || method && !matches_member)
        {
            let msg = self
                .diag(DiagId::sema_friend_def_no_match, d.name_loc())
                .arg(fname.clone())
                .arg(self.qualifier_desc(ctx));
            self.emit(msg);
            invalid_redecl = true;
        } else if let Some(o) = other_kind_member {
            // clang `MergeFunctionDecl`: the member found is not a function
            // (`template<> X C<int>::x();` for a static data member `x`
            // declares a function, [dcl.ambig.res]/1, [temp.expl.spec]/14).
            let msg = self.diag(DiagId::sema_redefinition_different_kind, d.name_loc());
            let msg = self.note_prev(msg.arg(fname.clone()), o, true);
            self.emit(msg);
            invalid_redecl = true;
        } else if method && !in_class && !matches_member
            || !method && !friend && d.qualifier.is_some() && prev.is_none()
        {
            // [dcl.meaning.general]/2: a qualified declarator-id names a
            // previously declared member of the nominated class or
            // namespace.
            let scope = if method
                && self.clang_compat < 22
                && self.ast.partial_spec_info(self.ast.first(ctx)).is_some()
            {
                // clang 19 prints a partial specialization with its
                // canonical arguments (`'P<type-parameter-0-0, int>'`).
                self.qualifier_desc(ctx)
            } else if method {
                // clang prints a class context as its type (`A<T>`,
                // `N::A<long>::C`).
                self.class_context_str(ctx)
            } else {
                self.qualifier_desc(ctx)
            };
            // clang selects "declaration" or "definition" by whether a
            // body follows.
            let id = if self.templ.fn_def_next {
                DiagId::sema_member_def_no_match
            } else {
                DiagId::sema_member_decl_no_match
            };
            let ret_differs = method && prev.is_some_and(|p| self.differs_in_return(p, ty));
            let msg = if let Some(p) = prev.filter(|_| ret_differs) {
                // The member it corresponds to except in the return type
                // (clang's `MergeFunctionDecl`).
                let m = self
                    .diag(DiagId::sema_member_def_ret_type_mismatch, d.name_loc())
                    .arg(self.member_qualified_name(p));
                self.note_prev(m, p, false)
            } else {
                let m = self.diag(id, d.name_loc()).arg(fname.clone()).arg(scope);
                if method { self.with_class_defined_note(m, ctx) } else { m }
            };
            self.emit(msg);
            invalid_redecl = true;
        } else if let Some(p) = prev
            && in_class
            && self.kind_of(p) == DeclKind::CXXMethod
        {
            // [class.mem.general]/5: a member function is declared once
            // in its class; corresponding declarations that differ only in
            // the return type or in `static` cannot be overloaded
            // ([over.load]/2; clang's `MergeFunctionDecl` order).
            let pty = self.declared_fn_type(p).unwrap_or(QualType::ERROR);
            let same_ret = match (self.ast.types.proto(pty), self.ast.types.proto(ty)) {
                (Some(a), Some(b)) => self.ast.types.same_type(a.ret(), b.ret()),
                _ => true,
            };
            let old_static =
                self.ast.decls.function(p).is_some_and(|f| f.spec.storage() == FnStorage::Static);
            let (msg, def) = if !same_ret {
                let def = self.ast.decls.function(p).is_some_and(|f| f.body.is_some());
                (self.diag(DiagId::sema_ovl_diff_return_type, d.name_loc()), def)
            } else if old_static != (spec.storage() == FnStorage::Static) {
                (self.diag(DiagId::sema_ovl_static_nonstatic_member, d.name_loc()), false)
            } else {
                (self.diag(DiagId::sema_member_redeclared, d.name_loc()), false)
            };
            let msg = self.note_prev(msg, p, def);
            self.emit(msg);
            invalid_redecl = true;
        } else if let Some(p) = prev
            && method
            && !friend
            && !in_class
            && self.kind_of(p) == fkind
            && let Some((id, sel)) = self.defaulted_member_definition(p)
        {
            // [class.compare.default]/4, [dcl.fct.def.default]/5: an
            // implicit or explicitly defaulted member is not defined
            // again out of its class.
            self.emit(self.diag(id, d.name_loc()).arg(sel));
            invalid_redecl = true;
        } else if let Some(p) = prev {
            let pn = *self.ast.decls.get(p);
            if pn.kind() == fkind {
                let mut pty = self.declared_fn_type(p).unwrap_or(QualType::ERROR);
                // [except.spec]/13: a previous declaration's specification
                // still uninstantiated is needed now (clang resolves the
                // old one in `CheckEquivalentExceptionSpec`).
                if !self.in_template()
                    && matches!(self.written_exc_spec(pty), ExcSpec::Deferred(_))
                    && self.uninstantiated_exc.contains_key(&self.ast.first(p))
                    && !self.exc_unparsed(p)
                {
                    self.resolve_exception_spec(p, d.name_loc());
                    pty = self.declared_fn_type(p).unwrap_or(QualType::ERROR);
                }
                // So is a defaulted comparison's implicit one, unevaluated
                // from the end of its class ([except.spec]/9, clang
                // `ResolveExceptionSpec` of `EST_Unevaluated`): a
                // redeclaration after the class must match it.
                if !self.in_template()
                    && matches!(self.written_exc_spec(pty), ExcSpec::Deferred(_))
                    && self.special.cmp.default_loc.contains_key(&self.ast.first(p))
                {
                    let _ = self.resolve_cmp_exc(self.ast.first(p));
                    pty = self.declared_fn_type(p).unwrap_or(QualType::ERROR);
                }
                // A GNU `noreturn` function type (spec/types.md §5.4) is
                // inherited by a redeclaration and does not take part in
                // matching (clang `MergeFunctionDecl`).
                let nr = |s: &Self, t: QualType| {
                    s.ast.types.proto(t).is_some_and(|p| p.info().noreturn())
                };
                if nr(self, pty) && !nr(self, ty) {
                    ty = self.with_fn_noreturn(ty, true, false).unwrap_or(ty);
                } else if nr(self, ty) && !nr(self, pty) {
                    pty = self.with_fn_noreturn(pty, true, false).unwrap_or(pty);
                }
                // A friend whose noexcept-specifier waits for the end of
                // its class (or, instantiated, until needed), or one
                // redeclaring such a friend before its class is complete,
                // matches whatever the other declaration has; the
                // specifications are compared once both are known (clang's
                // `DelayedEquivalentExceptionSpecChecks`), never as a
                // missing specification.
                let delayed = late_exc
                    || matches!(self.written_exc_spec(pty), ExcSpec::Deferred(_))
                        && self.same_ignoring_exc_spec(pty, ty);
                // [except.spec]/5: a redeclaration without a specification.
                let mut missing_err = false;
                if !delayed
                    && !self.ast.types.same_type(pty, ty)
                    && self.written_exc_spec(ty) == ExcSpec::None
                    && self.same_ignoring_exc_spec(pty, ty)
                {
                    let (t, err) = self.merge_missing_exc_spec(
                        p,
                        pty,
                        ty,
                        (d.name_loc(), &fname, crate::except::missing_exc_spec_fixit_loc(d)),
                        false,
                    );
                    ty = t;
                    missing_err = err;
                }
                if delayed {
                    delayed_prev = true;
                }
                let cmp = if delayed {
                    let pexc = self.written_exc_spec(pty);
                    self.with_exc_spec(ty, pexc)
                } else {
                    ty
                };
                if missing_err {
                    invalid_redecl = true;
                } else if self.ast.types.same_type(pty, cmp) {
                    prev_ok = Some(p);
                    // [dcl.spec.auto.general]/13: the redeclaration has
                    // the deduced return type.
                    ty = self.adopt_deduced_return(p, ty);
                    let pspec = self.ast.decls.function(p).map(|f| f.spec).unwrap_or_default();
                    if spec.storage() == FnStorage::Static && pspec.storage() != FnStorage::Static {
                        let msg = self
                            .diag(DiagId::sema_static_non_static, d.name_loc())
                            .arg(fname.clone());
                        let msg = self.note_prev(msg, p, false);
                        self.emit(msg);
                    }
                    // [dcl.link]/6: the language linkage is the first
                    // declaration's (clang `err_different_language_linkage`).
                    let def = self.ast.decls.function(p).is_some_and(|f| f.body.is_some());
                    if !method
                        && !friend
                        && self.diagnose_language_linkage_redecl(p, &fname, d.name_loc(), def)
                    {
                        prev_ok = None;
                        invalid_redecl = true;
                    }
                    if pspec.storage() == FnStorage::Static && spec.storage() == FnStorage::None {
                        spec = spec.with_storage(FnStorage::Static).with_storage_inherited(true);
                    }
                    if pspec.is_inline() {
                        spec = spec.with_implicit_inline(true);
                    }
                    // [dcl.constexpr]/1: if any declaration of a function
                    // is `constexpr` or `consteval`, every declaration is
                    // (clang `err_constexpr_redecl_mismatch`).
                    // An invalid previous declaration is not checked.
                    let (ok, nk) = (pspec.constexpr_kind(), spec.constexpr_kind());
                    if ok != nk && !self.ast.decls.get(p).flags().contains(DeclFlags::INVALID) {
                        let msg = self
                            .diag(DiagId::sema_constexpr_redecl_mismatch, d.name_loc())
                            .arg(nk as u8)
                            .arg(fname.clone())
                            .arg(ok as u8);
                        let msg = self.note_prev(msg, p, false);
                        self.emit(msg);
                        prev_ok = None;
                        invalid_redecl = true;
                    }
                } else if self.same_ignoring_exc_spec(pty, ty) {
                    // [except.spec]/5: every declaration of a function with
                    // a non-throwing specification has one, and only those.
                    let msg = self.diag(DiagId::sema_exception_spec_mismatch, d.name_loc());
                    let msg = self.note_prev(msg, p, false);
                    let ne = self.written_exc_spec(ty);
                    self.emit(crate::except::per_instantiation_exc(msg, ne));
                    invalid_redecl = true;
                } else {
                    let same_params = match (self.ast.types.proto(pty), self.ast.types.proto(ty)) {
                        (Some(a), Some(b)) => {
                            let (a, b) = (*a, *b);
                            let (pa, pb) = (
                                self.ast.types.params(&a).to_vec(),
                                self.ast.types.params(&b).to_vec(),
                            );
                            pa.len() == pb.len()
                                && pa.iter().zip(&pb).all(|(x, y)| self.ast.types.same_type(*x, *y))
                                && a.info().variadic() == b.info().variadic()
                        }
                        _ => false,
                    };
                    // clang 22 reports two `extern "C"` declarations
                    // differing in return type as conflicting types
                    // (spec/sema.md §1.1).
                    let both_c = self.clang_compat >= 22
                        && kiln_abi::mangle::is_extern_c(&self.ast, p)
                        && self.ctx_is_extern_c(self.context());
                    let msg = if same_params && !both_c {
                        self.diag(DiagId::sema_ovl_diff_return_type, d.name_loc())
                    } else if extern_c_redecl || both_c {
                        invalid_redecl = true;
                        self.diag(DiagId::sema_conflicting_types, d.name_loc()).arg(fname.clone())
                    } else {
                        self.diag(DiagId::sema_ovl_static_nonstatic_member, d.name_loc())
                    };
                    // clang's `getNoteDiagForInvalidRedeclaration`: the
                    // builtin-type note for a builtin.
                    let def = self.ast.decls.function(p).is_some_and(|f| f.body.is_some());
                    let builtin =
                        self.ast.decls.builtin_id(p).and_then(kiln_ast::decl::builtin::info);
                    let msg = match builtin {
                        Some(info) => {
                            let loc = self.ast.decls.get(p).loc();
                            self.builtin_type_note(msg, &info, loc)
                        }
                        None => self.note_prev(msg, p, def),
                    };
                    self.emit(msg);
                }
            } else {
                let msg = self
                    .diag(DiagId::sema_redefinition_different_kind, d.name_loc())
                    .arg(fname.clone());
                let msg = self.note_prev(msg, p, true);
                self.emit(msg);
            }
        }
        let params: Vec<ParamInfo> = match d.function_params() {
            Some(p) => p.to_vec(),
            None => self.typedef_fn_params(ty, d.name_loc()),
        };
        let eo = match early_explicit_object {
            Some(r) => r,
            None => self.explicit_object_member(ds, &params, method),
        };
        match eo {
            Some(true) => spec = spec.with_explicit_object(true),
            Some(false) => invalid_redecl = true,
            None => {}
        }
        let (defaults, redefined) = self.merge_default_args(prev_ok, block, &params, !prev_invalid);
        if let Some(p) = prev_ok {
            self.check_param_nullability(p, &params);
            self.check_param_indeterminate(p, &params);
        }
        // clang marks the redeclaration invalid and leaves it out of the
        // redeclaration chain; calls ignore it.
        if redefined {
            prev_ok = None;
            invalid_redecl = true;
        }
        let mut h = DeclHead::new(ctx, dname, d.name_loc());
        h.lexical = Some(lexical);
        if friend {
            // [namespace.memdef]/3: visible to ordinary lookup only if
            // declared outside the class (before or after).
            let visible = prev_ok.is_some_and(|p| {
                self.ast.lookup_local(ctx, dname).iter_with_flags().any(|(x, fl)| {
                    (x == p || Some(x) == self.ast.described_template(p))
                        && !fl.contains(LookupFlags::ADL_ONLY)
                })
            });
            let k = if visible || d.qualifier.is_some() && !quiet_friend {
                FriendKind::Declared
            } else {
                FriendKind::Undeclared
            };
            h.misc = h.misc.with_friend(k);
        }
        self.mark_pattern_next();
        let site = crate::class::VirtualSite {
            class: ctx,
            method,
            in_class,
            friend,
            prev: prev_ok,
            name: dname,
        };
        let virt = self.virtual_decl(ds, d, ty, site, &mut spec);
        // [dcl.spec.auto.general]/12 (clang's `err_auto_fn_virtual`).
        if let Some(vloc) = ds.virtual_
            && in_class
            && self.ast.types.proto(ty).is_some_and(|p| self.contains_undeduced_auto(p.ret()))
        {
            self.error(DiagId::sema_auto_fn_virtual, vloc);
        }
        let f = self.builder().function(h, fkind, ty, spec, prev_ok);
        self.finish_virtual_decl(f, &virt);
        if let Some(Chunk::Function { trailing: Some((t, arrow)), .. }) = d.chunks.first()
            && self.is_instantiation_dependent_type(*t)
        {
            let _ = self.templ.trailing_arrow.insert(f, *arrow);
        }
        // Where a substituted signature's return and parameter types are
        // diagnosed (clang `BuildFunctionType` at the declaration's
        // begin, §5.6).
        // A const-qualified return type is also noted there when the call
        // is assigned to ([expr.assign]/1, `ret_type_loc`).
        if let Some(tl) = ds.type_loc
            && tl != d.name_loc()
            && self.ast.types.proto(ty).is_some_and(|p| {
                self.ast.types.is_dependent(p.ret())
                    || self.ast.types.params(p).iter().any(|&q| self.ast.types.is_dependent(q))
                    || self.returns_const(p.ret())
            })
        {
            self.type_spec_locs.insert(f, tl);
        }
        // The return type's innermost `*`/`[` ([`Sema::inner_chunk_locs`]).
        let chunk = Self::inner_chunk_loc(d);
        if chunk.is_valid()
            && self
                .ast
                .types
                .proto(ty)
                .is_some_and(|p| self.is_instantiation_dependent_type(p.ret()))
        {
            self.inner_chunk_locs.insert(f, chunk);
        }
        // A pure virtual function's notes point at its decl-specifiers
        // (clang `getBeginLoc`, §4.11), as does the `->` note on an
        // `operator->` result ([over.ref]).
        if (self.ast.decls.function(f).is_some_and(|x| x.spec.is_pure())
            || self.ast.decls.get(f).name().as_operator() == Some(OverloadedOperator::Arrow))
            && ds.loc.is_valid()
            && ds.loc != self.ast.decls.get(f).loc()
        {
            let _ = self.ast.decls.cold.begin_loc.insert(f, ds.loc);
        }
        if let Some(l) = Self::placeholder_constraint_loc(ds, d) {
            self.inst.concepts.placeholder_locs.insert(f, l);
        }
        if d.missing_typename {
            self.missing_typename_fns.insert(f);
        }
        if invalid_redecl {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        } else if templated_friend {
            self.friend_qualifiers.insert(f, ctx);
        }
        if d.op_name.is_none() && d.lit_suffix.is_none() {
            self.note_local_extern(name, f);
            if !method && !friend && d.qualifier.is_none() {
                self.record_extern_c(name, f);
            }
        }
        // Where a constexpr or consteval defaulted assignment or comparison
        // that cannot be one is diagnosed (`check_defaulted_impl`).
        if (method || friend)
            && ds.constexpr.or(ds.consteval).is_some()
            && ds.loc != d.name_loc()
            && self.std < kiln_lex::Std::Cxx26
        {
            self.constexpr_begin_locs.insert(f, ds.loc);
        }
        self.create_params(f, &params, &defaults);
        self.defer_late_noexcept(f, d);
        if delayed_prev && let Some(p) = prev_ok {
            self.delayed_exc_equiv.push((f, p));
        }
        if let Some(p) = pattern_prev {
            // Compared now, or once its replayed specification is known.
            self.delayed_exc_equiv.push((f, p));
            if !late_exc {
                self.check_delayed_exc_equiv(f);
            }
        }
        if d.lit_suffix.is_some() {
            if !self.ast.decls.get(f).flags().contains(DeclFlags::INVALID)
                && !self.check_literal_operator_decl(f)
            {
                self.ast.decls.set_flags(f, DeclFlags::INVALID);
            }
        } else if let Some(op) = d.op_name {
            if !global_new_delete
                && !self.ast.decls.get(f).flags().contains(DeclFlags::INVALID)
                && !self.check_operator_decl(f)
            {
                self.ast.decls.set_flags(f, DeclFlags::INVALID);
            }
            // A block-scope operator declaration: a local class's friend
            // may redeclare it ([class.friend]/11).
            if !method && !friend {
                self.push_local_operator(op, f);
            }
        } else {
            self.register_local(name, f);
        }
        if !d.defaulted {
            self.check_consteval_only_fn(f, d.name_loc());
        }
        f
    }

    /// The [over.oper] rules for operator function `f` (spec/sema-overload.md
    /// §6.1, clang `CheckOverloadedOperatorDeclaration`): reports the first
    /// violation and returns false. Allocation and deallocation functions
    /// are checked elsewhere.
    pub(crate) fn check_operator_decl(&mut self, f: DeclId) -> bool {
        use OverloadedOperator as O;
        let Some(op) = self.ast.decls.get(f).name().as_operator() else { return true };
        if matches!(op, O::New | O::ArrayNew | O::Delete | O::ArrayDelete) {
            return true;
        }
        let loc = self.ast.decls.get(f).loc();
        let spelled = format!("operator{}", op.spelling());
        let method = self.kind_of(f) == DeclKind::CXXMethod;
        let is_static = method && self.is_static_method(f);
        let params: Vec<DeclId> = self
            .ast
            .decls
            .function(f)
            .map(|fd| self.ast.lists.iter::<DeclId>(fd.params).collect())
            .unwrap_or_default();
        let ptys: Vec<QualType> =
            params.iter().map(|&p| self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR)).collect();
        if is_static {
            if !matches!(op, O::Call | O::Subscript) {
                let m = self.diag(DiagId::sema_operator_overload_static, loc).arg(spelled);
                self.emit(m);
                return false;
            }
        } else if !method {
            // [over.oper.general]/7: a non-member operator function has a
            // parameter of class or enumeration type (or reference to one).
            let ok = ptys.iter().any(|&t| {
                let t = self.non_reference(t);
                self.ast.types.is_dependent(t)
                    || self.ast.types.as_record(t).is_some()
                    || self.ast.types.as_enum(t).is_some()
                    || self.is_error_type(t)
            });
            if !ok {
                let m =
                    self.diag(DiagId::sema_operator_overload_needs_class_or_enum, loc).arg(spelled);
                self.emit(m);
                return false;
            }
        }
        // [over.oper.general]/10: no default arguments, except for `()`
        // and (C++23) `[]`.
        if !matches!(op, O::Call | O::Subscript)
            && let Some(&p) = params.iter().find(|&&p| self.param_has_default(p))
        {
            let ploc = self.ast.decls.get(p).loc();
            let m = self.diag(DiagId::sema_operator_overload_default_arg, ploc).arg(spelled);
            self.emit(m);
            return false;
        }
        let (unary, binary, member) = operator_uses(op);
        let explicit_obj = self.has_explicit_object(f);
        let n = params.len() + usize::from(method && !is_static && !explicit_obj);
        if !matches!(op, O::Call | O::Subscript)
            && (n == 1 && !unary || n == 2 && !binary || !(1..=2).contains(&n))
        {
            let which = match (unary, binary) {
                (true, true) => "2",
                (true, false) => "0",
                _ => "1",
            };
            let m = self
                .diag(DiagId::sema_operator_overload_must_be, loc)
                .arg(spelled)
                .arg(which)
                .arg(n.to_string())
                .arg(if n == 1 { "0" } else { "1" });
            self.emit(m);
            return false;
        }
        let variadic = self.ast.decls.function(f).is_some_and(|x| x.spec.is_variadic());
        if op != O::Call && variadic {
            let m = self.diag(DiagId::sema_operator_overload_variadic, loc).arg(spelled);
            self.emit(m);
            return false;
        }
        if member && !method {
            let m = self.diag(DiagId::sema_operator_overload_must_be_member, loc).arg(spelled);
            self.emit(m);
            return false;
        }
        // [over.inc]/1: the parameter of a postfix `++`/`--` is `int`.
        if matches!(op, O::PlusPlus | O::MinusMinus)
            && n == 2
            && let (Some(&p), Some(&t)) = (params.last(), ptys.last())
        {
            let t = self.ast.types.canonical(t);
            if !self.ast.types.is_dependent(t)
                && !self.ast.types.same_unqualified(t, QualType::INT)
                && !self.is_error_type(t)
            {
                let ploc = self.ast.decls.get(p).loc();
                let m = self
                    .diag(DiagId::sema_operator_overload_post_incdec_must_be_int, ploc)
                    .arg(self.ty_arg(ptys[ptys.len() - 1]))
                    .arg(if op == O::PlusPlus { "0" } else { "1" });
                self.emit(m);
                return false;
            }
        }
        true
    }

    /// The parameters of a function declared without a function
    /// declarator (`F f;` with `typedef void F(int)`, or through
    /// `decltype`, [dcl.fct]/13): one unnamed parameter per parameter
    /// type of `ty`, at the declarator's name (clang
    /// `BuildParmVarDeclForTypedef`).
    fn typedef_fn_params(&self, ty: QualType, loc: Loc) -> Vec<ParamInfo> {
        let Some(p) = self.sugared_proto(ty) else { return Vec::new() };
        self.ast
            .types
            .params(&p)
            .iter()
            .map(|&t| ParamInfo {
                name: None,
                ty: t,
                original_ty: None,
                loc,
                attrs: Vec::new(),
                invalid: false,
                default_arg: DefaultArg::None,
                default_loc: Loc::INVALID,
                range: (loc, loc),
                type_loc: loc,
                ref_loc: Loc::INVALID,
                chunk_loc: Loc::INVALID,
                decl: None,
                explicit_object: None,
            })
            .collect()
    }

    /// Creates the parameter declarations of function `f` from the
    /// declarator's `params` (adopting those created at their point of
    /// declaration, [`Sema::act_on_prototype_param`]), with the merged `defaults`
    /// ([`Sema::merge_default_args`]), and attaches them.
    pub(crate) fn create_params(
        &mut self,
        f: DeclId,
        params: &[ParamInfo],
        defaults: &[Option<ExprId>],
    ) {
        let mut ids = Vec::with_capacity(params.len());
        let mut seen: Vec<Symbol> = Vec::new();
        for (i, p) in params.iter().enumerate() {
            let pname = match p.name {
                Some((n, loc)) => {
                    if seen.contains(&n) {
                        let msg =
                            self.diag(DiagId::sema_param_redefinition, loc).arg(sym_str(self, n));
                        self.emit(msg);
                    }
                    seen.push(n);
                    DeclName::ident(n)
                }
                None => DeclName::EMPTY,
            };
            let idx = u16::try_from(i).unwrap_or(u16::MAX);
            // A function parameter pack ([temp.variadic]/3) has a pack
            // expansion type.
            let pack = matches!(self.ast.types.view(p.ty.ty()), TypeView::PackExpansion { .. });
            let mut flags = if pack { kiln_ast::decl::ParamData::PACK } else { 0 };
            if p.explicit_object.is_some() {
                flags |= kiln_ast::decl::ParamData::EXPLICIT_OBJECT;
            }
            let pd = match p.decl.filter(|d| self.unowned_params.remove(d)) {
                Some(pd) => {
                    // [basic.scope.param]: the parameter named in the rest
                    // of the declarator is the function's.
                    self.ast.decls.set_param_owner(pd, f);
                    self.ast.decls.set_ty(pd, p.ty);
                    pd
                }
                None => self.builder().param(f, pname, p.loc, p.ty, idx, flags),
            };
            if let Some(o) = p.original_ty {
                self.ast.decls.set_param_original_ty(pd, o);
            }
            if p.range.0.is_valid() && p.range.0 != p.loc {
                let _ = self.ast.decls.cold.begin_loc.insert(pd, p.range.0);
                if p.type_loc.is_valid()
                    && p.type_loc != p.loc
                    && self.is_instantiation_dependent_type(p.ty)
                {
                    self.type_spec_locs.insert(pd, p.type_loc);
                }
            }
            // A block-scope function's, where the local-friend notes
            // point (`check_local_friend`).
            if p.type_loc.is_valid()
                && p.type_loc != p.range.0
                && self.kind_of(f) == DeclKind::Function
                && self.in_local_scope()
            {
                self.type_spec_locs.insert(pd, p.type_loc);
            }
            if p.ref_loc.is_valid() && self.is_instantiation_dependent_type(p.ty) {
                self.ref_locs.insert(pd, p.ref_loc);
            }
            if p.chunk_loc.is_valid() && self.is_instantiation_dependent_type(p.ty) {
                self.inner_chunk_locs.insert(pd, p.chunk_loc);
            }
            if p.invalid {
                self.ast.decls.set_flags(pd, DeclFlags::INVALID);
            }
            if !p.attrs.is_empty() {
                let mut attrs = p.attrs.clone();
                self.take_annotations(pd, &mut attrs);
                let range = Span::new(p.range.0, p.range.1);
                self.check_param_nonnull_attrs(p.ty, range, &mut attrs);
                let _ = self.builder().attach_attrs(pd, &attrs);
            }
            match p.default_arg {
                // Replayed in the complete-class context through
                // `act_on_param_default_arg` (spec/sema-class.md §2.7).
                DefaultArg::Unparsed(_, at) => {
                    self.ast.decls.mark_default_arg_pending(pd);
                    self.unparsed_default_args.insert(pd, at);
                }
                // A default parsed in place was converted by
                // `act_on_param_default_init`; `defaults` also holds the
                // inherited ones ([dcl.fct.default]/4).
                DefaultArg::None | DefaultArg::Parsed(_) => {
                    if let Some(e) = defaults[i] {
                        let _ = self.ast.decls.set_default_arg(pd, e);
                    }
                }
            }
            ids.push(pd);
        }
        let _ = self.builder().set_params(f, &ids);
    }

    /// The default arguments of a function declaration's parameters
    /// after merging with the previous declaration `prev` in the same
    /// scope ([dcl.fct.default]/4): a default given again is an error
    /// ("redefinition of default argument"; the redeclaration is then
    /// invalid, the second result), one given earlier is inherited.
    /// Declarations in different scopes (a block-scope redeclaration of
    /// a namespace-scope function) have distinct sets. Then, with
    /// `trailing`, every parameter after one with a default argument
    /// needs one (clang checks no redeclaration of an invalid function:
    /// `void f(int, int = (int)"s"); void f(int = 1, int);` reports only
    /// the cast).
    pub(crate) fn merge_default_args(
        &mut self,
        prev: Option<DeclId>,
        block: bool,
        params: &[ParamInfo],
        trailing: bool,
    ) -> (Vec<Option<ExprId>>, bool) {
        let mut out: Vec<Option<ExprId>> = params
            .iter()
            .map(|p| match p.default_arg {
                DefaultArg::Parsed(e) => Some(e),
                DefaultArg::None | DefaultArg::Unparsed(..) => None,
            })
            .collect();
        let mut redefined = false;
        let cur = self.ctx;
        let old = prev.and_then(|p| {
            self.ast.redecls_of(p).find(|&r| {
                if block {
                    self.ast.decls.lexical_ctx(r) == Some(cur)
                } else {
                    !self.lexically_local(r)
                }
            })
        });
        if let Some(o) = old {
            let olds: Vec<DeclId> = self
                .ast
                .decls
                .function(o)
                .map(|x| self.ast.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default();
            for (i, p) in params.iter().enumerate() {
                let Some(&op) = olds.get(i) else { break };
                let Some(od) = self.ast.decls.param(op).and_then(|x| x.default_arg) else {
                    continue;
                };
                if p.default_arg == DefaultArg::None {
                    out[i] = Some(od);
                } else {
                    // The note names the declaration that wrote it.
                    let origin = self
                        .ast
                        .redecls_of(o)
                        .filter_map(|r| {
                            let f = self.ast.decls.function(r)?;
                            let rp = self.ast.lists.at::<DeclId>(f.params, i)?;
                            (self.ast.decls.param(rp)?.default_arg == Some(od)).then_some(rp)
                        })
                        .last()
                        .unwrap_or(op);
                    let note = self
                        .diag(DiagId::note_previous_definition, self.ast.decls.get(origin).loc());
                    let msg = self.diag(DiagId::sema_default_arg_redefinition, p.loc);
                    self.emit(msg.with_note(note));
                    redefined = true;
                }
            }
        }
        // clang skips the trailing check once a parameter is invalid
        // (`void f(int = 1, u8, int);` with undeclared `u8` reports only
        // the unknown type name).
        if let Some(first) = out.iter().position(Option::is_some)
            && trailing
            && !params.iter().any(|p| p.invalid)
        {
            for (i, p) in params.iter().enumerate().skip(first + 1) {
                // A function parameter pack needs none ([dcl.fct.default]/4).
                if out[i].is_some()
                    || p.default_arg != DefaultArg::None
                    || p.invalid
                    || self.ast.types.kind(p.ty.ty()) == kiln_ast::types::TypeKind::PackExpansion
                {
                    continue;
                }
                let msg = match p.name {
                    Some((n, _)) => self
                        .diag(DiagId::sema_missing_default_arg, p.loc)
                        .arg(self.ast.symbols().as_str(n).to_string()),
                    None => self.diag(DiagId::sema_missing_default_arg_unnamed, p.loc),
                };
                self.emit(msg);
            }
        }
        (out, redefined)
    }

    /// True if `d` was declared inside a function body.
    fn lexically_local(&self, d: DeclId) -> bool {
        let mut c = self.ast.decls.lexical_ctx(d);
        while let Some(x) = c {
            if self.ast.decls.get(x).kind() == DeclKind::Function {
                return true;
            }
            c = self.ast.decls.get(x).ctx();
        }
        false
    }

    /// The decl-specifiers of a non-static data member: `mutable`
    /// ([dcl.stc]/9) gives `FieldData::MUTABLE`; the others are errors.
    /// The second value says the field is invalid (clang
    /// `CheckFieldDecl`: a `mutable` reference or const member).
    fn field_spec_flags(&mut self, ds: &DeclSpec, ty: QualType) -> (u32, bool) {
        let mut flags = 0;
        let mut invalid = false;
        if let Some((s, loc)) = ds.storage {
            match s {
                StorageSpec::Mutable => {
                    // [dcl.stc]/9: not on a const object or a reference.
                    if self.ast.types.is_reference(ty) {
                        self.error(DiagId::sema_mutable_reference, loc);
                        invalid = true;
                    } else if self.ast.types.effective_quals(ty).is_const() {
                        self.error(DiagId::sema_mutable_const, loc);
                        invalid = true;
                    } else {
                        flags |= FieldData::MUTABLE;
                    }
                }
                StorageSpec::Extern | StorageSpec::Register | StorageSpec::Static => {
                    self.error(DiagId::sema_storage_class_for_member, loc);
                }
                StorageSpec::Typedef => {
                    self.error(DiagId::sema_invalid_storage_class_in_func_decl, loc);
                }
            }
        }
        if let Some(loc) = ds.constexpr.or(ds.consteval) {
            self.error(DiagId::sema_constexpr_nonstatic_member, loc);
        }
        if let Some(loc) = ds.consteval {
            self.error(DiagId::sema_consteval_wrong_decl_kind, loc);
        }
        (flags, invalid)
    }

    fn act_on_field(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        invalid: bool,
    ) -> DeclId {
        let ctx = self.ctx;
        let name = d.name.map(|n| n.0);
        let nm = name.map(|n| sym_str(self, n));
        let (mut flags, spec_invalid) = self.field_spec_flags(ds, ty);
        let mut field_invalid = spec_invalid;
        if let Some(style) = d.init_style {
            // A default member initializer ([class.mem.general]/11),
            // replayed after the class is complete.
            flags |= (style as u32) << 2;
            if let Some(c) = self.classes.last_mut()
                && c.record == ctx
            {
                c.bits = c.bits.with(DefinitionBits::HAS_IN_CLASS_INIT);
                c.pending_nsdmi += 1;
            }
        }
        // [basic.scope.scope]/5 (P2169): a non-static data member `_` is
        // name-independent and conflicts with no earlier one.
        let independent = name.is_some_and(|n| self.is_placeholder_name(n));
        // A class or enumeration of the same name is hidden, not
        // redeclared ([basic.scope.hiding]/2; clang `CheckFieldDecl`):
        // `struct generic_request {...} generic_request;`.
        if let Some(n) = name
            && let Some(&prev) = self.lookup_in(ctx, n, LookupKind::Ordinary).decls.first()
            && !is_tag_kind(self.kind_of(prev))
        {
            if independent && self.kind_of(prev) == DeclKind::Field {
                self.diagnose_placeholder_redecl(d.name_loc());
            } else {
                let note =
                    self.diag(DiagId::note_previous_declaration, self.ast.decls.get(prev).loc());
                let msg =
                    self.diag(DiagId::sema_duplicate_member, d.name_loc()).arg(sym_str(self, n));
                self.emit(msg.with_note(note));
                field_invalid = true;
            }
        }
        // [dcl.spec.auto.general]/5: a placeholder is not allowed in a
        // non-static data member (clang's `err_auto_not_allowed`).
        let invalid = invalid || {
            let kw = self.placeholder_keyword(ty);
            if let Some(kw) = kw {
                use crate::templ::ctad_site::site;
                let s = match self.ast.decls.record(ctx).map_or(TagKind::Struct, |r| r.tag) {
                    TagKind::Struct => site::STRUCT_MEMBER,
                    TagKind::Union => site::UNION_MEMBER,
                    TagKind::Class => site::CLASS_MEMBER,
                };
                let msg = self
                    .diag(DiagId::sema_auto_not_allowed, ds.type_loc.unwrap_or(ds.loc))
                    .arg(u8::from(kw == AutoKeyword::DecltypeAuto))
                    .arg(s);
                // Once per declarator of a group (as clang).
                self.emit(msg.allow_duplicate());
            }
            kw.is_some()
        };
        // clang `CheckFieldDecl`'s `RequireCompleteType`: a field of a
        // class template specialization whose instantiation fails is
        // invalid, quietly (and so is its class, §4.3 of
        // sema-templates.md).
        let failed_spec = !invalid
            && !self.ast.types.is_reference(ty)
            && self.instantiation_fails(self.bounded_array_element(ty), d.name_loc());
        let invalid = invalid || failed_spec;
        let ty = if failed_spec || (invalid && self.placeholder_keyword(ty).is_some()) {
            QualType::ERROR
        } else if !invalid && self.ast.types.is_sizeless(ty) {
            let msg = self.diag(DiagId::sema_field_sizeless, d.name_loc()).arg(self.ty_arg(ty));
            self.emit(msg);
            QualType::ERROR
        } else if !invalid
            && !self.ast.types.is_reference(ty)
            && !self.require_complete(self.field_complete_type(ty), d.name_loc())
        {
            // clang checks (and names) the element type of a bounded array
            // (and of a flexible array member, §5.8).
            let elem = self.field_complete_type(ty);
            let msg = self.diag(DiagId::sema_field_incomplete, d.name_loc()).arg(self.ty_arg(elem));
            self.emit(self.incomplete_note(msg, elem));
            // The class becomes invalid: later uses that need it complete
            // before its `}` are quiet (clang `CheckFieldDecl`).
            if let Some(&c) = self.defining.last() {
                let _ = self.invalid_defining.insert(c);
            }
            // clang marks the field invalid: initialization never
            // descends into it (a field of the class's own type would
            // recurse forever).
            QualType::ERROR
        } else if !invalid
            && (self.require_non_abstract(d.name_loc(), ty, crate::class::AbstractUse::Field)
                || self.union_reference_member(ctx, ty, nm.as_deref(), d.name_loc()))
        {
            QualType::ERROR
        } else {
            ty
        };
        let width = d.bit_width.and_then(|w| self.bit_width(w, ty, nm.as_deref(), d.name_loc()));
        // clang `CheckFieldDecl`: a width that fails `VerifyBitField`
        // (every such path has diagnosed) leaves the field invalid.
        field_invalid |= d.bit_width.is_some() && width.is_none();
        field_invalid |= invalid || failed_spec || self.is_error_type(ty);
        let h = DeclHead::new(ctx, name.map_or(DeclName::EMPTY, DeclName::ident), d.name_loc())
            .misc(DeclMisc::new().with_name_independent(independent));
        let f = self.builder().field(h, ty, width, flags);
        if field_invalid {
            // [class.mem]: an invalid field makes its class invalid
            // (clang `ActOnFields`; spec/sema-class.md §2.3).
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
            self.invalidate_defining(ctx);
        }
        // The decl-specifiers' start (clang's `FieldDecl::getBeginLoc`;
        // the Oven's `__builtin_bit_cast` notes point there).
        if ds.loc.is_valid() && ds.loc != d.name_loc() {
            let _ = self.ast.decls.cold.begin_loc.insert(f, ds.loc);
        }
        if self.is_instantiation_dependent_type(ty) {
            self.note_type_spec_loc(f, ds, d.name_loc());
            self.note_ref_loc(f, d, ty);
        }
        if let Some(l) = Self::placeholder_constraint_loc(ds, d) {
            self.inst.concepts.placeholder_locs.insert(f, l);
        }
        if d.init_style.is_some() {
            self.special.nsdmi.insert(f);
        }
        f
    }

    /// [class.union.general]/2: a union has no non-static data member of
    /// reference type (clang `err_union_member_of_reference_type`, at the
    /// name; the field is invalid). True when diagnosed.
    fn union_reference_member(
        &mut self,
        ctx: DeclId,
        ty: QualType,
        name: Option<&str>,
        loc: Loc,
    ) -> bool {
        if !self.ast.types.is_reference(ty)
            || !self.ast.decls.record(ctx).is_some_and(|r| r.tag == TagKind::Union)
        {
            return false;
        }
        let msg = self
            .diag(DiagId::sema_union_member_reference, loc)
            .arg(name.unwrap_or_default().to_owned())
            .arg(self.ty_arg(ty));
        self.emit(msg);
        true
    }

    /// A member declarator with a bit-field width that declared no
    /// field ([class.bit]/3, clang `ActOnCXXMemberDeclarator`): a static
    /// data member, a typedef or a function, each diagnosed at its name
    /// with the width's range. The width is dropped.
    fn non_field_bit_width(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        name: Symbol,
        loc: Loc,
        w: ExprId,
    ) {
        let n = sym_str(self, name);
        let msg = if ds.is_typedef() {
            self.diag(DiagId::sema_typedef_not_bitfield, loc).arg(n)
        } else if self.ast.types.is_function(ty) {
            self.diag(DiagId::sema_bitfield_non_integral, loc).arg(n).arg(self.ty_arg(ty))
        } else {
            self.diag(DiagId::sema_static_not_bitfield, loc).arg(n)
        };
        let msg = msg.range(self.expr_range(w));
        self.emit(msg);
    }

    /// Checks a bit-field width ([class.bit]/1): an integral constant
    /// expression, wrapped in a `ConstantExpr` (clang `VerifyBitField`).
    /// Diagnostics point at the field's location `loc`, as clang's do,
    /// with the width's range where clang gives one (non-integral field
    /// type, zero width), or at the width for a non-integral width.
    fn bit_width(
        &mut self,
        w: ExprId,
        ty: QualType,
        name: Option<&str>,
        loc: Loc,
    ) -> Option<ExprId> {
        if self.is_err_expr(w) {
            return None;
        }
        let t = &self.ast.types;
        if !(self.is_dependent_type(ty)
            || t.is_integral(ty)
            || t.as_enum(ty).is_some()
            || self.is_error_type(ty))
        {
            let msg = match name {
                Some(n) => {
                    self.diag(DiagId::sema_bitfield_non_integral, loc).arg(n).arg(self.ty_arg(ty))
                }
                None => {
                    self.diag(DiagId::sema_anon_bitfield_non_integral, loc).arg(self.ty_arg(ty))
                }
            };
            let msg = msg.range(self.expr_range(w));
            self.emit(msg);
            return None;
        }
        if self.expr_is_value_dependent(w) || self.is_dependent_type(ty) {
            return Some(w);
        }
        // clang `VerifyIntegerConstantExpression`: the width has integral
        // or unscoped enumeration type (a class type converts below).
        let wt = self.ast.exprs.get(w).ty();
        let t = &self.ast.types;
        if !(t.is_integral(wt)
            || t.as_enum(wt).is_some_and(|en| !self.is_scoped_enum(en))
            || t.as_record(wt).is_some()
            || self.is_error_type(wt))
        {
            let msg = self
                .diag(DiagId::sema_ice_not_integral, self.begin_loc(w))
                .arg(self.ty_arg(wt))
                .range(self.expr_range(w));
            self.emit(msg);
            return None;
        }
        let c = conv::decay(self, w);
        let errors = self.diags.error_count();
        let c = self.act_on_constant_full_expr(c);
        if self.diags.error_count() > errors {
            // A member named without an object (clang returns an
            // invalid expression and drops the width).
            return None;
        }
        let v = match self.eval_int(c, EvalContext::new(EvalKind::Other, self.begin_loc(w))) {
            Ok(v) => v,
            // clang `VerifyBitField`: `AllowFold`.
            Err(f) => self.fold_not_ice(c, f, self.begin_loc(w))?,
        };
        if v < 0 {
            let msg = match name {
                Some(n) => {
                    self.diag(DiagId::sema_bitfield_negative_width, loc).arg(n).arg(v.to_string())
                }
                None => {
                    self.diag(DiagId::sema_anon_bitfield_negative_width, loc).arg(v.to_string())
                }
            };
            self.emit(msg);
            return None;
        }
        if v == 0
            && let Some(n) = name
        {
            let msg =
                self.diag(DiagId::sema_bitfield_zero_width, loc).arg(n).range(self.expr_range(w));
            self.emit(msg);
            return None;
        }
        if let Some((bits, _)) = constfold::int_info(&self.ast, ty) {
            // clang warns for every integral type but `bool`.
            if v > i128::from(bits)
                && self.ast.types.builtin(ty) != Some(BuiltinType::Bool)
                && let Some(n) = name
            {
                let msg = self
                    .diag(DiagId::sema_bitfield_width_exceeds, loc)
                    .arg(n)
                    .arg(v.to_string())
                    .arg(bits.to_string())
                    .arg(if bits == 1 { "" } else { "s" });
                self.emit(msg);
            }
        }
        let cv = self.int_const(QualType::INT, v);
        Some(self.builder().constant_expr(c, Some(cv)))
    }

    /// clang `err_typecheck_sclass_fscope`: `register` (without a GNU
    /// `asm` register) on an unqualified variable outside a function (a
    /// member is a field, a static member cannot be `register`); the
    /// variable is invalid.
    pub(crate) fn register_at_file_scope(&self, ds: &DeclSpec, d: &Declarator) -> bool {
        matches!(ds.storage, Some((StorageSpec::Register, _)))
            && d.asm_label.is_none()
            && d.qualifier.is_none()
            && !self.in_local_scope()
    }

    /// The variable's storage class makes it invalid: `register` at file
    /// scope ([`Sema::register_at_file_scope`]) or `mutable` on anything
    /// but a non-static data member ([dcl.stc]/9, clang
    /// `err_mutable_nonmember`, which `var_spec` reports).
    pub(crate) fn storage_invalidates_var(&self, ds: &DeclSpec, d: &Declarator) -> bool {
        matches!(ds.storage, Some((StorageSpec::Mutable, _))) || self.register_at_file_scope(ds, d)
    }

    pub(crate) fn var_spec(&mut self, ds: &DeclSpec, d: &Declarator) -> VarSpec {
        let mut spec = VarSpec::new();
        if let Some(style) = d.init_style {
            spec = spec.with_init_style(style);
        }
        match ds.storage {
            Some((StorageSpec::Static, _)) => spec = spec.with_storage(VarStorage::Static),
            Some((StorageSpec::Extern, _)) => spec = spec.with_storage(VarStorage::Extern),
            Some((StorageSpec::Register, loc)) => {
                spec = spec.with_storage(VarStorage::Register);
                // [dcl.stc]: removed in C++17; clang accepts it as an
                // extension (an error by default, `-Wregister`), silently
                // for a GNU register variable (`asm("reg")`).
                if d.asm_label.is_none() {
                    self.emit(self.diag(DiagId::sema_register_storage_class, loc));
                    if self.register_at_file_scope(ds, d) {
                        self.error(DiagId::sema_illegal_storage_class_on_var, d.name_loc());
                    }
                }
            }
            Some((StorageSpec::Mutable, _)) => {
                // [dcl.stc]/9: only a non-static data member (a field never
                // gets here); at the name, and the variable is invalid
                // (`storage_invalidates_var`).
                self.error(DiagId::sema_mutable_nonmember, d.name_loc());
            }
            _ => {}
        }
        if ds.thread_local.is_some() {
            // clang `VarDecl::getTLSKind`: `__thread` and `_Thread_local`
            // are static TLS (checked by `check_static_tls`), `thread_local`
            // may be dynamically initialized ([basic.stc.thread]).
            let kind =
                if ds.thread_kind == TlsKind::Static { TlsKind::Static } else { TlsKind::Dynamic };
            spec = spec.with_tls(kind);
        }
        if ds.constexpr.is_some() {
            spec = spec.with_constexpr(true);
        }
        if let Some(at) = ds.constinit {
            // [dcl.constinit]/1: only a variable with static or thread
            // storage duration (clang `err_constinit_local_variable`).
            let local = self.in_local_scope()
                && ds.thread_local.is_none()
                && !matches!(spec.storage(), VarStorage::Static | VarStorage::Extern);
            if local {
                self.error(DiagId::sema_constinit_local, at);
            } else {
                spec = spec.with_constinit(true);
            }
        }
        if let Some(l) = ds.inline {
            if self.in_local_scope() {
                self.inline_block_scope(d, l);
            } else {
                spec = spec.with_inline(true);
            }
        }
        spec
    }

    /// [dcl.inline]: `inline` shall not appear on a block scope
    /// declaration (clang `err_inline_declaration_block_scope`, at
    /// `inline`, naming the variable or a structured binding
    /// declaration's first binding); the variable is not inline.
    fn inline_block_scope(&mut self, d: &Declarator, l: Loc) {
        let name = match &d.decomp {
            Some(dc) => dc.ids.first().map(|b| b.name),
            None => d.name.map(|(n, _)| n),
        };
        let Some(name) = name else { return };
        let msg = self
            .diag(DiagId::sema_inline_declaration_block_scope, l)
            .range(Span::new(l, l.with_offset(6)))
            .arg(sym_str(self, name));
        self.emit(msg);
    }

    pub(crate) fn is_var_definition(&self, v: DeclId) -> bool {
        // [basic.def]/2.4: a static data member declared in its class is
        // a definition only when inline (constexpr implies inline).
        if self.is_in_class_static(v) {
            return self.ast.decls.var(v).is_some_and(|x| x.spec.is_inline());
        }
        // [temp.expl.spec]/14: an explicit specialization of a static data
        // member without an initializer is a declaration.
        if self.ast.is_member_spec_declaration(v) {
            return false;
        }
        // [dcl.link]/8: `extern "C" int x;` is treated as `extern`.
        let ext = |x: &kiln_ast::decl::VarData| {
            x.spec.storage() == VarStorage::Extern || self.ast.decls.is_single_line_linkage(v)
        };
        self.ast.decls.var(v).is_some_and(|x| !ext(x) || x.init.is_some())
    }

    /// True while declaring the declaration directly contained in a
    /// linkage specification without braces ([dcl.link]/8).
    fn in_single_line_linkage(&self) -> bool {
        self.kind_of(self.ctx) == DeclKind::LinkageSpec
            && self
                .ast
                .decls
                .scope(self.ctx)
                .is_some_and(|s| s.aux & kiln_ast::decl::ScopeData::LINKAGE_BRACELESS != 0)
    }

    /// For a variable declarator qualified by namespace `qualifier`: when
    /// redeclaration lookup of `name` there finds more than one entity (a
    /// member of the namespace and one of its inline namespaces, or two
    /// inline namespaces; [namespace.def.general]/7, [dcl.meaning]/3), the
    /// first-declared one, which clang's ambiguous lookup reports as
    /// "different kind of symbol" (WP-235).
    fn ambiguous_qualified_prev(&self, name: Symbol, qualifier: Option<DeclId>) -> Option<DeclId> {
        let q = qualifier?;
        if !matches!(self.kind_of(q), DeclKind::Namespace | DeclKind::TranslationUnit) {
            return None;
        }
        let mut firsts: Vec<DeclId> = self
            .prev_candidates(name, Some(q))
            .into_iter()
            .filter(|&d| matches!(self.kind_of(d), DeclKind::Var | DeclKind::Function))
            // An invalid declaration with no type (`R::v;`) merged with
            // nothing and makes no lookup ambiguous (WP-411).
            .filter(|&d| !self.ast.decls.get(d).has(DeclFlags::INVALID))
            .map(|d| self.ast.first(d))
            .collect();
        firsts.sort_unstable();
        firsts.dedup();
        if firsts.len() > 1 { firsts.first().copied() } else { None }
    }

    /// clang's `MergeVarDecl` for a block-scope variable `name` of type
    /// `ty` and storage `storage` at `loc` whose name is already declared
    /// in the block (a variable or a parameter `p`), in clang's order: the
    /// types (unless `p` is invalid), `static` after a declaration with
    /// external linkage, non-`static` after `static`, `extern` after a
    /// declaration without linkage and the reverse, then the redefinition
    /// (spec/sema.md §4.2; WP-235); an `extern` that also finds a
    /// declaration with linkage outside the block is a different kind of
    /// symbol. Only an `extern` redeclaring an `extern` is accepted.
    fn merge_local_var(
        &mut self,
        p: DeclId,
        ty: QualType,
        storage: VarStorage,
        loc: Loc,
        name: Symbol,
    ) {
        let pn = *self.ast.decls.get(p);
        let pty = pn.ty().unwrap_or(QualType::ERROR);
        let old_storage = self.ast.decls.var(p).map(|x| x.spec.storage());
        let old_extern = old_storage == Some(VarStorage::Extern);
        let old_static = old_storage == Some(VarStorage::Static);
        let old_linkage = old_extern || self.dropped_qualified_locals.contains(&p);
        let old_invalid = pn.has(DeclFlags::INVALID) || self.is_error_type(pty);
        let old_def = !old_extern;
        let new_extern = storage == VarStorage::Extern;
        let new_static = storage == VarStorage::Static;
        // A dependent type is checked at instantiation (clang
        // `MergeVarDeclTypes` for a local variable; WP-330).
        let compatible = self.redecl_var_types_compatible(pty, ty)
            || self.is_dependent_type(pty)
            || self.is_dependent_type(ty);
        let n = sym_str(self, name);
        // An `extern` that also finds a declaration with linkage in the
        // enclosing namespace (or an earlier block-scope `extern`) sees an
        // ambiguous set: clang reports a different kind of symbol.
        let outer_linkage = new_extern && !old_linkage && {
            let ns = self.enclosing_namespace();
            ns.is_some_and(|ns| {
                self.lookup_in(ns, name, LookupKind::Ordinary)
                    .decls
                    .iter()
                    .any(|&d| matches!(self.kind_of(d), DeclKind::Var | DeclKind::Function))
                    || self.local_externs.iter().any(|&(n, s, _)| n == ns && s == name)
            })
        };
        let msg = if outer_linkage {
            self.diag(DiagId::sema_redefinition_different_kind, loc).arg(&n)
        } else if !old_invalid && !compatible {
            let id = if new_extern {
                DiagId::sema_redeclaration_different_type
            } else {
                DiagId::sema_redefinition_different_type
            };
            self.diag(id, loc).arg(&n).diff_args(self, ty, pty)
        } else if new_static && old_extern {
            self.diag(DiagId::sema_static_non_static, loc).arg(&n)
        } else if (!new_extern || !old_linkage) && !new_static && old_static {
            self.diag(DiagId::sema_non_static_static, loc).arg(&n)
        } else if new_extern && !old_linkage {
            self.diag(DiagId::sema_extern_non_extern, loc).arg(&n)
        } else if old_linkage && !new_extern {
            self.diag(DiagId::sema_non_extern_extern, loc).arg(&n)
        } else if !new_extern {
            self.diag(DiagId::sema_redefinition, loc).arg(&n)
        } else {
            return;
        };
        let msg = self.note_prev(msg, p, old_def);
        self.emit(msg);
    }

    /// `diagnose_ambiguous_qualified_var`, or, for an unqualified
    /// variable with no earlier declaration (`no_prev`) outside a
    /// template, `template<class T> int V; int V;` (`kind_conflict`).
    /// True when diagnosed.
    fn diagnose_var_kind_conflict(&mut self, name: Symbol, d: &Declarator, no_prev: bool) -> bool {
        if self.diagnose_ambiguous_qualified_var(name, d) {
            return true;
        }
        no_prev
            && d.qualifier.is_none()
            && self.templ.cur.is_none()
            && self.diagnose_template_kind_conflict(name, NewKind::Var, d.name_loc())
    }

    /// `int Q::t;` naming members of several inline namespaces of `Q`:
    /// clang's "redefinition of 't' as different kind of symbol", noted
    /// at one of them (WP-235). True when diagnosed.
    fn diagnose_ambiguous_qualified_var(&mut self, name: Symbol, d: &Declarator) -> bool {
        let Some(rep) = self.ambiguous_qualified_prev(name, d.qualifier) else {
            return false;
        };
        let msg = self
            .diag(DiagId::sema_redefinition_different_kind, d.name_loc())
            .arg(sym_str(self, name));
        let msg = self.note_prev(msg, rep, true);
        self.emit(msg);
        true
    }

    /// Whether variable `name` declared by `d` with `spec` is
    /// name-independent, and its earlier declaration in its scope: a
    /// block-scope variable `_` of automatic storage duration
    /// ([basic.scope.scope]/5, P2169) conflicts with no earlier variable
    /// or binding of its scope.
    fn var_prev_local(
        &mut self,
        spec: VarSpec,
        d: &Declarator,
        name: Symbol,
    ) -> (bool, Option<DeclId>) {
        let independent = d.qualifier.is_none()
            && self.in_local_scope()
            && matches!(spec.storage(), VarStorage::None | VarStorage::Register)
            && spec.tls() == TlsKind::None
            && self.is_placeholder_name(name);
        let local =
            self.find_prev(name, d.qualifier).map(|p| self.valid_prev_var(p, name, d.qualifier));
        if independent
            && local.is_some_and(|p| matches!(self.kind_of(p), DeclKind::Var | DeclKind::Binding))
        {
            self.diagnose_placeholder_redecl(d.name_loc());
            return (true, None);
        }
        (independent, local)
    }

    /// The previous declaration a variable redeclares when lookup finds
    /// `p`: an invalid variable declaration (a redefinition, another
    /// type, no type at all: `R::v;`) is not linked to the entity
    /// (clang's `MergeVarDecl` stops at it), so the latest valid one
    /// lookup finds, if any, is (WP-421).
    fn valid_prev_var(&self, p: DeclId, name: Symbol, qualifier: Option<DeclId>) -> DeclId {
        if self.kind_of(p) != DeclKind::Var || !self.is_invalid_decl(p) {
            return p;
        }
        let found = self.prev_candidates(name, qualifier);
        found
            .into_iter()
            .rev()
            .find(|&d| self.kind_of(d) == DeclKind::Var && !self.is_invalid_decl(d))
            .unwrap_or(p)
    }

    /// A variable `n` redeclared at `at` over `p` of a compatible type:
    /// a second definition (`ext`: the new declaration is `extern`,
    /// `prev_def`: `p` is a definition; [basic.def.odr]/1) or a different
    /// language linkage ([dcl.link]/6, clang
    /// `err_different_language_linkage`). Whether one was reported.
    fn diagnose_var_redecl_conflict(
        &mut self,
        p: DeclId,
        n: &str,
        at: Loc,
        ext: bool,
        prev_def: bool,
    ) -> bool {
        // The same entity defined in another unit's global module
        // fragment: merged, not redefined (spec/modules.md §6).
        if !ext && prev_def && !self.module_merge_hidden_definition(p) {
            let msg = self.diag(DiagId::sema_redefinition, at).arg(n);
            let msg = self.note_prev(msg, p, true);
            self.emit(msg);
            return true;
        }
        self.diagnose_language_linkage_redecl(p, n, at, prev_def)
    }

    /// [dcl.meaning.general]/2.2: a qualified declarator-id names a
    /// member of its namespace; when lookup finds none, clang's
    /// `ActOnVariableDeclarator` reports "no member named" at the name
    /// and the variable is invalid (`int R::h;`, `R::h;`; WP-411).
    fn diagnose_no_member_var(&mut self, ctx: DeclId, d: &Declarator, name: Symbol) -> bool {
        let no_member = d.qualifier.is_some()
            && matches!(self.kind_of(ctx), DeclKind::Namespace | DeclKind::TranslationUnit)
            && self.lookup_qualified(ctx, name, LookupKind::Ordinary).decls.is_empty();
        if no_member {
            let msg = self
                .diag(DiagId::sema_no_member, d.name_loc())
                .arg(sym_str(self, name))
                .arg(self.qualifier_desc(ctx));
            self.emit(msg);
        }
        no_member
    }

    fn act_on_var(&mut self, ds: &DeclSpec, ty: QualType, d: &Declarator, name: Symbol) -> DeclId {
        let (ctx, lexical) = self.decl_contexts(d);
        let spec = self.var_spec(ds, d);
        let ty = self.check_vla_var(ty, spec, d);
        if self.kind_of(ctx) == DeclKind::CXXRecord {
            return self.act_on_static_member(ds, ty, d, name, spec);
        }
        let mut prev_ok = None;
        // A redefinition (or a redeclaration with another type) is invalid
        // (clang `setInvalidDecl`), so a later lookup that also finds the
        // earlier declaration names that one, not an ambiguous set.
        let mut redefined = false;
        let ext = spec.storage() == VarStorage::Extern || self.in_single_line_linkage();
        let (independent, local) = self.var_prev_local(spec, d, name);
        let mut prev = match local {
            None if ext || !self.in_local_scope() => self.find_prev_linkage(name, d.qualifier),
            p => p,
        };
        let ambiguous = self.diagnose_var_kind_conflict(name, d, prev.is_none());
        let no_member = prev.is_none() && !ambiguous && self.diagnose_no_member_var(ctx, d, name);
        // A variable declared with no type is invalid and not merged
        // (clang's `MergeVarDecl` returns at an invalid new declaration):
        // `y; y;` reports the missing type twice, no redefinition; so
        // does one whose type named no type (`zz v;`, `R::f(v);`; WP-411).
        let unmerged = ambiguous || !ds.has_type_specifier() || ds.type_error;
        prev = prev.filter(|_| !unmerged);
        // [dcl.link]/6-7: a C-linkage variable of another scope.
        let prev = match prev {
            None if d.qualifier.is_none() => {
                self.extern_c_prev(name, true, spec.storage() == VarStorage::Static, d.name_loc())
            }
            p => p,
        };
        // An invalid decl-specifier (`typename Q b;`) was reported: the
        // declaration is not merged (clang's invalid `DeclSpec`).
        if let Some(p) = prev.filter(|_| !self.is_error_type(ty)) {
            let pn = *self.ast.decls.get(p);
            let block = local.is_some() && self.in_local_scope();
            if pn.kind() == DeclKind::Var && !block {
                let pty = pn.ty().unwrap_or(QualType::ERROR);
                // A block-scope `extern` of dependent type is checked at
                // instantiation (clang `MergeVarDeclTypes`; WP-330).
                let compatible = self.redecl_var_types_compatible(pty, ty)
                    || self.in_local_scope()
                        && (self.is_dependent_type(pty) || self.is_dependent_type(ty));
                if compatible {
                    let prev_def = self.is_var_definition(p);
                    let n = sym_str(self, name);
                    self.check_tls_redecl(p, spec.tls(), prev_def, d.name_loc(), &n);
                    if self.diagnose_var_redecl_conflict(p, &n, d.name_loc(), ext, prev_def) {
                        redefined = true;
                    } else {
                        prev_ok = Some(p);
                    }
                } else {
                    // clang's `diagnoseVarDeclTypeMismatch`: "redefinition" if
                    // the new declaration is a definition (judged before its
                    // initializer is seen), and the note says whether the
                    // old one was.
                    let id = if ext {
                        DiagId::sema_redeclaration_different_type
                    } else {
                        DiagId::sema_redefinition_different_type
                    };
                    let msg = self
                        .diag(id, d.name_loc())
                        .arg(sym_str(self, name))
                        .diff_args(self, ty, pty);
                    let msg = self.note_prev(msg, p, self.is_var_definition(p));
                    self.emit(msg);
                    redefined = true;
                }
            } else if block && matches!(pn.kind(), DeclKind::Var | DeclKind::ParmVar) {
                let new_invalid = ds.invalid || d.invalid || self.is_error_type(ty);
                if !new_invalid {
                    self.merge_local_var(p, ty, spec.storage(), d.name_loc(), name);
                }
            } else {
                let msg = self
                    .diag(DiagId::sema_redefinition_different_kind, d.name_loc())
                    .arg(sym_str(self, name));
                let msg = self.note_prev(msg, p, true);
                self.emit(msg);
            }
        }
        let ty = prev_ok.map_or(ty, |p| self.redecl_var_type(p, ty));
        let mut h = DeclHead::new(ctx, DeclName::ident(name), d.name_loc())
            .misc(DeclMisc::new().with_name_independent(independent));
        h.lexical = Some(lexical);
        self.mark_pattern_next();
        let v = self.builder().var(h, ty, spec, prev_ok);
        if ambiguous || no_member || redefined || unmerged || self.storage_invalidates_var(ds, d) {
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
        // As for a typedef (§5.8): `static int (*s)(typename T::type);`.
        if ds.loc.is_valid() && ds.loc != d.name_loc() && self.is_instantiation_dependent_type(ty) {
            let _ = self.ast.decls.cold.begin_loc.insert(v, ds.loc);
            self.note_type_spec_loc(v, ds, d.name_loc());
        }
        self.note_ref_loc(v, d, ty);
        self.lambda_mangle.var = Some(v);
        if let Some(l) = Self::placeholder_constraint_loc(ds, d) {
            // The constrained placeholder's diagnostics point here.
            self.inst.concepts.placeholder_locs.insert(v, l);
        }
        self.check_abstract_var(v);
        self.check_void_init_var(v, d);
        self.check_sizeless_var(v, self.in_local_scope());
        self.record_constinit(v, ds);
        self.check_constexpr_var_literal(v);
        self.register_local(name, v);
        if spec.storage() == VarStorage::Extern {
            self.note_local_extern(name, v);
        }
        if d.qualifier.is_none() {
            self.record_extern_c(name, v);
        }
        v
    }

    /// [basic.link]/11: two declarations of one variable may name array
    /// types that differ only in the presence of a major bound.
    pub(crate) fn redecl_var_types_compatible(&self, pty: QualType, ty: QualType) -> bool {
        let t = &self.ast.types;
        t.same_type(pty, ty)
            || t.is_array(pty)
                && t.is_array(ty)
                && (t.array_size(pty).is_none() || t.array_size(ty).is_none())
                && t.element(pty).zip(t.element(ty)).is_some_and(|(a, b)| t.same_type(a, b))
    }

    /// [dcl.array]/8: an array bound omitted by a redeclaration (or by the
    /// definition of a static data member) is the bound of the earlier
    /// declaration of `prev`: `extern int a[4]; int a[] = {1};` declares
    /// `int[4]` (clang `MergeVarDeclTypes`). Otherwise `ty`.
    pub(crate) fn redecl_var_type(&self, prev: DeclId, ty: QualType) -> QualType {
        let t = &self.ast.types;
        if !matches!(t.canonical_view(ty), TypeView::IncompleteArray(_)) {
            return ty;
        }
        let pty = self.ast.decls.get(self.ast.latest(prev)).ty().unwrap_or(QualType::ERROR);
        if t.array_size(pty).is_some() && self.redecl_var_types_compatible(pty, ty) {
            pty
        } else {
            ty
        }
    }

    /// [basic.def]/5, [dcl.init.general]/7: a variable with an initializer
    /// has a complete object type. `void x(1);`, `void v(T::m);` (a
    /// parenthesized dependent name is an initializer, [dcl.ambig.res]/1)
    /// are reported when declared, before the initializer is parsed (clang
    /// `err_typecheck_decl_incomplete_type`); `v` becomes invalid.
    fn check_void_init_var(&mut self, v: DeclId, d: &Declarator) {
        let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        if d.init_style.is_none() || !self.ast.types.is_void(ty) {
            return;
        }
        let msg = self.diag(DiagId::sema_void_var, d.name_loc()).arg(self.ty_arg(ty));
        self.emit(msg);
        self.ast.decls.set_flags(v, DeclFlags::INVALID);
    }

    // -- initializers ---------------------------------------------------------

    /// The initializer of variable `v` ([dcl.init]). Its syntax
    /// ([`Declarator::init_style`]) was recorded when `v` was declared.
    pub fn act_on_init(&mut self, v: DeclId, init: Initializer) {
        let (es, at) = match &init {
            Initializer::Expr(e) => (vec![*e], None),
            Initializer::Paren(es, l, _) => (es.clone(), Some(*l)),
            Initializer::List(l) => (Self::list_exprs(l), Some(l.lbrace)),
        };
        // [temp.variadic]/12: "initializer contains unexpanded parameter
        // pack", at the `(`/`{` of a parenthesized or braced initializer
        // (clang's ParenListExpr/InitListExpr); the variable becomes
        // invalid (its initializer dropped).
        if es.iter().any(|&e| self.diagnose_unexpanded_written(e, "initializer", at)) {
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
        let paren = match &init {
            Initializer::Paren(es, l, _) if es.len() == 1 => Some(*l),
            _ => None,
        };
        self.check_alias_definition(v);
        self.act_on_init_inner(v, init);
        self.check_static_tls(v, paren);
        let constinit_ok = self.check_constinit(v);
        self.report_deferred_immediates(v, constinit_ok);
        self.check_consteval_only_var(v);
        self.initializing = None;
        self.lambda_mangle.var = None;
        if self.ast.decls.decomp_bindings(v).is_some() {
            self.complete_decomposition(v);
        }
    }

    /// Checks the expressions of an initializer that will not initialize
    /// anything (the variable is invalid).
    fn discard_initializer(&mut self, init: &Initializer) {
        let es = match init {
            Initializer::Expr(e) => vec![*e],
            Initializer::Paren(es, ..) => es.clone(),
            Initializer::List(l) => Self::list_exprs(l),
        };
        let outer = std::mem::replace(&mut self.discarding_init, true);
        for e in es {
            self.act_on_full_expr(e);
        }
        self.discarding_init = outer;
    }

    /// Diagnoses initializing `extern` variable `v` (clang
    /// `AddInitializerToDecl`): an error at block scope (false: the
    /// initializer is dropped); otherwise a warning, but not for a const
    /// object or an `extern "C"` variable in C++, nor an instantiation.
    fn check_extern_init(&mut self, v: DeclId, loc: Loc) -> bool {
        use kiln_ast::templates::SpecKind;
        if self.in_local_scope() {
            self.error(DiagId::sema_block_extern_init, loc);
            return false;
        }
        let t = self.base_element(self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR));
        if !self.ast.types.effective_quals(t).is_const()
            && !kiln_abi::mangle::is_extern_c(&self.ast, v)
            && !self.ast.spec_info(v).is_some_and(|(_, i)| {
                !matches!(i.kind, SpecKind::Undeclared | SpecKind::ExplicitSpecialization)
            })
        {
            self.error(DiagId::sema_extern_init, loc);
        }
        true
    }

    /// clang `AddInitializerToDecl`: a bounded array of an incomplete
    /// element (now formable, [dcl.array]/1) is diagnosed under the full
    /// variable type at the variable's name; the variable becomes invalid.
    fn reject_incomplete_array_init(
        &mut self,
        v: DeclId,
        ty: QualType,
        loc: Loc,
        init: &Initializer,
    ) -> bool {
        if !matches!(self.ast.types.canonical_view(ty), TypeView::ConstantArray(_))
            || self.ast.types.is_dependent(ty)
            || self.require_complete(self.bounded_array_element(ty), loc)
        {
            return false;
        }
        let msg = self.diag(DiagId::sema_typecheck_decl_incomplete_type, loc).arg(self.ty_arg(ty));
        self.emit(self.incomplete_note(msg, self.bounded_array_element(ty)));
        self.ast.decls.set_flags(v, DeclFlags::INVALID);
        self.discard_initializer(init);
        true
    }

    /// A dependent initializer, kept as written until instantiation.
    fn set_dependent_var_init(&mut self, v: DeclId, e: ExprId) {
        let e = self.act_on_full_expr(e);
        let _ = self.builder().set_var_init(v, e);
    }

    /// clang `AddInitializerToDecl`: a declaration that was not a
    /// definition before its initializer (`extern int a = 2;`) still
    /// redefines an earlier definition. Diagnoses it and marks `v`
    /// invalid; true when it did.
    fn extern_init_redefines(&mut self, v: DeclId, spec: VarSpec, loc: Loc) -> bool {
        let ext = spec.storage() == VarStorage::Extern || self.ast.decls.is_single_line_linkage(v);
        if !ext || self.in_local_scope() || self.is_in_class_static(v) {
            return false;
        }
        let def = self.ast.redecls_of(v).find(|&x| x != v && self.is_var_definition(x));
        let Some(def) = def else { return false };
        let name = self.ast.decls.get(v).name();
        let msg = self.diag(DiagId::sema_redefinition, loc).arg(self.decl_name_str(name));
        let msg = self.note_prev(msg, def, true);
        self.emit(msg);
        self.ast.decls.set_flags(v, DeclFlags::INVALID);
        true
    }

    /// A list or parenthesized initializer that depends on a template
    /// parameter, or any initializer of a dependent class type: kept for
    /// instantiation; returns whether it was.
    fn defer_dependent_init(&mut self, v: DeclId, ty: QualType, init: &Initializer) -> bool {
        match self.dependent_init_form(ty, init) {
            Some(e) => {
                self.set_dependent_var_init(v, e);
                true
            }
            None => false,
        }
    }

    /// The form kept for instantiation of initializer `init` of an
    /// object of type `ty` (a variable or an init-capture) when it
    /// depends on a template parameter as [`Sema::defer_dependent_init`]
    /// describes; `None` when it is initialized now.
    pub(crate) fn dependent_init_form(
        &mut self,
        ty: QualType,
        init: &Initializer,
    ) -> Option<ExprId> {
        if let Initializer::List(l) = init
            && (self.ast.types.is_dependent(ty) || self.init_list_type_dependent(l))
        {
            // Initialized at instantiation ([temp.dep.expr]); clang keeps
            // the list as written with type `void` (and `int[]` unbounded).
            return Some(self.dependent_init_list(l));
        }
        // A class that is (a member of) the current instantiation is a
        // dependent type ([temp.dep.type]/7): its constructors are chosen
        // at instantiation, as clang's `AddInitializerToDecl` does, since a
        // parameter of that type gives no ranking in the template.
        let dependent_class =
            self.ast.types.is_dependent(ty) && self.ast.types.as_record(ty).is_some();
        if let Initializer::Expr(e) = init
            && dependent_class
        {
            return Some(*e);
        }
        // `T x({a, b})`: a braced-init-list argument is kept for
        // instantiation with a dependent `T` too (it has no type of its
        // own), and with a dependent element it is a type-dependent
        // argument ([temp.dep.expr]/3).
        let (braced, braced_dep) = match init {
            Initializer::Paren(es, ..) => (
                es.iter().any(|&a| self.is_braced_arg(a)),
                es.iter().any(|&a| self.braced_arg_type_dependent(a)),
            ),
            _ => (false, false),
        };
        if let Initializer::Paren(es, l, r) = init
            && (es.len() > 1 || (dependent_class && !es.is_empty()) || braced)
            && (self.ast.types.is_dependent(ty)
                || braced_dep
                || es.iter().any(|&a| self.expr_is_type_dependent(a)))
        {
            // `T x(a, b)` with a dependent `T` or argument: initialized at
            // instantiation, kept as a `ParenListExpr` (clang
            // `ActOnParenListExpr` / `AddInitializerToDecl`). A braced
            // argument keeps its nested lists (`T m({{1, 2}, {3, 4}})`):
            // the placeholder holds only the top-level expressions.
            let es = self.dependent_args(es);
            return Some(self.builder().paren_list(&es, *l, *r));
        }
        None
    }

    /// Initializes variable `v` of type `ty` from `init`. A variable
    /// names itself, even when declared inside the initializer of another
    /// entity (a lambda body); `init_var` is the variable being
    /// initialized (WP-271 `diag_capture`).
    fn perform_var_init(
        &mut self,
        v: DeclId,
        ty: QualType,
        init: Initializer,
        loc: Loc,
        direct: bool,
    ) -> ExprId {
        let entity = self.init_entity.take();
        let var = self.init_var.replace(v);
        let e = self.perform_init(ty, init, loc, direct);
        self.init_entity = entity;
        self.init_var = var;
        e
    }

    /// The `void` half of [`Sema::act_on_init_inner`]: reports a `void`
    /// variable unless `reported` at its declaration, invalidates it and
    /// drops the initializer.
    fn reject_void_init(
        &mut self,
        v: DeclId,
        ty: QualType,
        reported: bool,
        init: &Initializer,
    ) -> bool {
        if !self.ast.types.is_void(ty) {
            return false;
        }
        if !reported {
            let loc = self.ast.decls.get(v).loc();
            let msg = self.diag(DiagId::sema_void_var, loc).arg(self.ty_arg(ty));
            self.emit(msg);
        }
        self.ast.decls.set_flags(v, DeclFlags::INVALID);
        self.discard_initializer(init);
        true
    }

    #[allow(clippy::too_many_lines, reason = "one linear pass over the initializer's checks")]
    fn act_on_init_inner(&mut self, v: DeclId, init: Initializer) {
        let vn = *self.ast.decls.get(v);
        if vn.kind() != DeclKind::Var {
            // clang's `AddInitializerToDecl`: only variables have an
            // initializer ([dcl.init]/1).
            if matches!(vn.kind(), DeclKind::Function | DeclKind::Typedef) {
                self.error(DiagId::sema_illegal_initializer, vn.loc());
                self.ast.decls.set_flags(v, DeclFlags::INVALID);
            }
            if let Initializer::Expr(e) = init {
                self.act_on_full_expr(e);
            }
            return;
        }
        // clang `AddInitializerToDecl`: the initializer of an invalid
        // variable is not checked against its type (`S::name &r = 1;`
        // with no `S::name`: the type recovered to `int`, WP-308).
        if vn.has(DeclFlags::INVALID) {
            self.discard_initializer(&init);
            return;
        }
        let spec = self.ast.decls.var(v).map(|x| x.spec).unwrap_or_default();
        let loc = vn.loc();
        if self.extern_init_redefines(v, spec, loc) {
            self.discard_initializer(&init);
            return;
        }
        if spec.storage() == VarStorage::Extern && !self.check_extern_init(v, loc) {
            return;
        }
        let mut ty = vn.ty().unwrap_or(QualType::ERROR);
        if self.reject_vla_init(v, ty, &init) {
            return;
        }
        let direct = spec.init_style() == kiln_ast::decl::InitStyle::List;
        if self.ast.decls.decomp_bindings(v).is_some()
            && let Some(t) = self.decomp_array_type(ty, &init, direct)
        {
            self.init_decomposition_array(v, t, &init);
            return;
        }
        if self.undeduced_ctad(ty).is_some() {
            // [dcl.type.class.deduct]/1 (WP-071b).
            ty = self.ctad_for_var(v, ty, &init, direct, loc);
            self.ast.decls.set_ty(v, ty);
            if self.is_error_type(ty) {
                self.ast.decls.set_flags(v, DeclFlags::INVALID);
                self.discard_initializer(&init);
                return;
            }
        }
        let deduced = self.contains_undeduced_auto(ty);
        if deduced {
            ty = self.deduce_var_auto(v, ty, &init, loc, direct).unwrap_or(QualType::ERROR);
            self.ast.decls.set_ty(v, ty);
            // A deduced sizeless type is checked as a written one (clang
            // `CheckVariableDeclarationType` after deduction).
            self.check_sizeless_var(v, self.in_local_scope());
            // `auto x = {...};` deduced `std::initializer_list<U>`; the
            // list initializes it below ([dcl.init.list]/3.6, WP-106).
        }
        // [basic.def]/5, [dcl.init.general]/7: an initialized variable has
        // a complete object type. A written `void` was reported when the
        // variable was declared ([`Sema::check_void_init_var`]); `auto x =
        // f();` with a `void` f is reported here (clang
        // `err_typecheck_decl_incomplete_type`). `x` gets no initializer.
        let reported = vn.flags().contains(DeclFlags::INVALID) && !deduced;
        if self.reject_void_init(v, ty, reported, &init) {
            return;
        }
        if self.constexpr_var_non_literal(v) {
            // Reported at the declarator, or here for a deduced type
            // (clang `CheckVariableDeclarationType`); the invalid
            // variable is not initialized (`AddInitializerToDecl`).
            self.check_constexpr_var_literal(v);
            self.discard_initializer(&init);
            return;
        }
        if self.defer_dependent_init(v, ty, &init) {
            return;
        }
        if self.reject_incomplete_array_init(v, ty, loc, &init) {
            return;
        }
        // clang `AddInitializerToDecl`'s `RequireCompleteType`: a class
        // template specialization failing to instantiate here leaves the
        // variable invalid, quietly.
        if self.invalid_by_failed_spec(v, ty, loc) {
            self.discard_initializer(&init);
            return;
        }
        let braced = matches!(init, Initializer::List(_));
        let e = self.perform_var_init(v, ty, init, loc, direct);
        if self.ast.exprs.get(e).kind() == ExprKind::CXXParenListInitExpr {
            // P0960 (clang's `parenlistinit`).
            let style = VarSpec::new().with_init_style(kiln_ast::decl::InitStyle::ParenList);
            self.ast.decls.mark_var(v, style);
        }
        self.check_self_reference(v, e, direct);
        self.note_checked_var_init_full_expr(v);
        let name_loc = self.ast.decls.get(v).loc();
        let e = self.act_on_full_expr_at(e, name_loc);
        let _ = self.builder().set_var_init(v, e);
        // [class.temporary]/6: the temporaries the variable binds live as
        // long as it does (spec/sema-class.md §5.4).
        if !self.is_err_expr(e) {
            self.extend_temporaries(v, e);
            self.check_dangling_pointer_var(v, e);
            // [class.dtor]/15 (spec/sema-class.md §4.8).
            let vty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
            self.check_var_destructor(v, vty, loc);
        }
        // An incomplete array takes its bound from the initializer.
        let ety = self.ast.exprs.get(e).ty();
        if self.ast.types.is_array(ty)
            && self.ast.types.array_size(ty).is_none()
            && self.ast.types.array_size(ety).is_some()
        {
            self.ast.decls.set_ty(v, ety);
        }
        // So does a reference to one bound to a braced list's temporary
        // (P0388R4; clang `InitializationSequence::Perform` updates the
        // declared type: `const int (&h)[] = {1, 2}` is a `const int
        // (&)[2]`, while `const int (&r)[] = arr` keeps its type).
        if braced && let Some(t) = self.ref_to_bound_array(ty, ety) {
            self.ast.decls.set_ty(v, t);
        }
        if spec.is_constexpr() {
            self.check_constexpr_init(v, e);
        } else if !spec.is_inline() && self.is_in_class_static(self.init_written_in(v)) {
            self.check_in_class_static_init(v, e);
        }
        self.warn_constant_evaluated_init(v, e);
    }

    /// `ty` with its referenced array of unknown bound replaced by `ety`,
    /// the bound array type of the temporary the reference binds, if `ty`
    /// is such a reference.
    fn ref_to_bound_array(&mut self, ty: QualType, ety: QualType) -> Option<QualType> {
        let t = &self.ast.types;
        let (rvalue, p) = match t.canonical_view(ty) {
            TypeView::LValueRef(p) => (false, p),
            TypeView::RValueRef(p) => (true, p),
            _ => return None,
        };
        if !matches!(t.canonical_view(p), TypeView::IncompleteArray(_))
            || !t.is_array(ety)
            || t.array_size(ety).is_none()
        {
            return None;
        }
        let t = &mut self.ast.types;
        Some(if rvalue { t.rvalue_ref_to(ety) } else { t.lvalue_ref_to(ety) })
    }

    /// The declaration whose initializer `v` carries: an instantiated
    /// variable template specialization's pattern (a member variable
    /// template defined out of line is not initialized in its class, as
    /// clang's `BuildVariableInstantiation` keeps the pattern's lexical
    /// context), else `v` itself.
    fn init_written_in(&self, v: DeclId) -> DeclId {
        match self.ast.spec_info(v) {
            Some((_, s)) if s.kind != kiln_ast::templates::SpecKind::ExplicitSpecialization => {
                s.from
            }
            _ => v,
        }
    }

    /// `auto x = e;` ([dcl.type.auto.deduct]): deduces through the
    /// `deduce_auto` seam (`templ/auto.rs`) and reports a failure as
    /// clang's `DeduceVariableDeclarationType` does.
    pub(crate) fn deduce_var_auto(
        &mut self,
        v: DeclId,
        declared: QualType,
        init: &Initializer,
        loc: Loc,
        direct: bool,
    ) -> Option<QualType> {
        use crate::templ::auto::AutoFail;
        let (mut items, kind, lbrace) = auto_init_items(init, direct);
        self.resolve_single_overloads(&mut items);
        let overload = items
            .iter()
            .flatten()
            .copied()
            .find(|&e| self.ast.exprs.get(e).ty() == QualType::OVERLOAD);
        let paren_braces = self.auto_paren_braces(init, direct);
        let fail = match (overload, paren_braces) {
            (_, Some(nested)) => AutoFail::ParenBraces(nested),
            (Some(e), None) => AutoFail::Incompatible(e),
            (None, None) => match self.deduce_auto(declared, &items, kind) {
                // clang 19's placeholder-typed specialization
                // (`undeduced_var_spec_use_19`) deduces only itself; any
                // other variable "deduced" from it is rejected (clang 19
                // accepts it and crashes generating code).
                Ok(t) if self.contains_undeduced_auto(t) => {
                    if self.undeduced_var_spec_use_19(v) {
                        self.templ.self_typed_var_specs.push(v);
                        return Some(t);
                    }
                    match items.iter().flatten().next() {
                        Some(&e) => AutoFail::Incompatible(e),
                        None => AutoFail::Error,
                    }
                }
                Ok(t) => {
                    let at = self.inst.concepts.placeholder_locs.get(&v).copied().unwrap_or(loc);
                    // `constexpr C decltype(auto) x = e;`: the constraint
                    // is checked against `decltype(e)`, before `constexpr`
                    // adds its `const` ([dcl.type.auto.deduct]/2,/5).
                    let (checked, ct) = match self.ast.types.canonical_view(declared) {
                        TypeView::Auto { keyword: AutoKeyword::DecltypeAuto, .. }
                            if !self.ast.types.canonical(declared).quals().is_empty() =>
                        {
                            let unq = self.ast.types.unqualified(declared);
                            match self.deduce_auto(unq, &items, kind) {
                                Ok(t0) => (unq, t0),
                                Err(_) => (declared, t),
                            }
                        }
                        _ => (declared, t),
                    };
                    return Some(if self.check_placeholder_constraint(checked, ct, at) {
                        t
                    } else {
                        QualType::ERROR
                    });
                }
                Err(f) => f,
            },
        };
        // A parenthesized initializer's errors point at its `(`.
        let lbrace = match init {
            Initializer::Paren(_, lp, _) => *lp,
            _ => lbrace.unwrap_or(loc),
        };
        let name = self.decl_name(v);
        // An init-capture ([expr.prim.lambda.capture]/6) is reported as
        // clang's `err_init_capture_*` (the capture is named, not its type).
        if self.init_entity == Some(crate::init::entity::CAPTURE)
            && self.report_init_capture_auto_fail(fail, &name, loc, lbrace)
        {
            return None;
        }
        let msg = match fail {
            AutoFail::Error => return Some(QualType::ERROR),
            AutoFail::ParenBraces(nested) => self
                .diag(DiagId::sema_auto_var_init_paren_braces, lbrace)
                .arg(name)
                .arg(self.ty_arg(declared))
                .arg(nested),
            AutoFail::Incompatible(e) => self
                .diag(DiagId::sema_auto_var_deduction_failure, loc)
                .arg(name)
                .arg(self.ty_arg(declared))
                .arg(self.ty_arg(self.ast.exprs.get(e).ty())),
            AutoFail::Multiple(at) => self
                .diag(DiagId::sema_auto_var_init_multiple_expressions, at)
                .arg(name)
                .arg(self.ty_arg(declared)),
            AutoFail::Empty => self
                .diag(DiagId::sema_auto_var_init_no_expression, lbrace)
                .arg(name)
                .arg(self.ty_arg(declared)),
            AutoFail::NoInitializerList => self.diag(DiagId::sema_auto_init_list_deduction, lbrace),
            AutoFail::Conflict(a, b, _) => {
                self.diag(DiagId::sema_auto_inconsistent_deduction, lbrace).diff_args(self, a, b)
            }
            AutoFail::FromInitList => self
                .diag(DiagId::sema_auto_var_deduction_failure_from_init_list, loc)
                .arg(name)
                .arg(self.ty_arg(declared)),
            AutoFail::DecltypeAutoList => {
                self.diag(DiagId::sema_decltype_auto_initializer_list, lbrace)
            }
        };
        self.emit(msg);
        None
    }

    /// [temp.deduct.call]/6 for a placeholder's initializer items: every
    /// member of an overload set deduces `auto`, so nothing is deduced (a
    /// placeholder in a function declarator selecting one member is not
    /// implemented) -- except that a set of one non-template function
    /// deduces from it, explicit template arguments ignored (clang's
    /// `ResolveOverloadForDeduction`; only typo correction builds one, and
    /// the initialization then fails against the deduced type).
    fn resolve_single_overloads(&mut self, items: &mut [Option<ExprId>]) {
        for it in items.iter_mut().flatten() {
            if let Some((_, d, _)) = self.single_overload_candidate(*it) {
                let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
                let at = self.ast.exprs.get(*it).loc();
                *it = self.builder().decl_ref(
                    d,
                    ty,
                    at,
                    None,
                    kiln_ast::expr::NonOdrUse::None,
                    kiln_ast::expr::ExprFlags::NONE,
                );
            }
        }
    }

    /// clang's `err_auto_var_init_paren_braces`: a direct initializer
    /// whose only element is itself braced, `auto x({1});` (0) or
    /// `auto x{{1}};` (1), a type-dependent one included.
    fn auto_paren_braces(&self, init: &Initializer, direct: bool) -> Option<usize> {
        match init {
            Initializer::Paren(es, ..) if es.len() == 1 => {
                (self.ast.exprs.get(es[0]).kind() == ExprKind::InitListExpr).then_some(0usize)
            }
            Initializer::List(l) if direct && l.items.len() == 1 => {
                let it = &l.items[0];
                (it.designator.is_empty() && matches!(it.value, crate::init::InitValue::List(_)))
                    .then_some(1usize)
            }
            _ => None,
        }
    }

    /// Reports placeholder-deduction failure `fail` of init-capture `name`
    /// in clang's `err_init_capture_*` wording; false for a failure with
    /// no capture form (reported as for a variable).
    fn report_init_capture_auto_fail(
        &mut self,
        fail: crate::templ::auto::AutoFail,
        name: &str,
        loc: Loc,
        lbrace: Loc,
    ) -> bool {
        use crate::templ::auto::AutoFail;
        let msg = match fail {
            AutoFail::Incompatible(e) => self
                .diag(DiagId::sema_init_capture_deduction_failure, loc)
                .arg(name)
                .arg(self.ty_arg(self.ast.exprs.get(e).ty())),
            AutoFail::Multiple(at) => {
                self.diag(DiagId::sema_init_capture_multiple_expressions, at).arg(name)
            }
            AutoFail::Empty => self.diag(DiagId::sema_init_capture_no_expression, lbrace).arg(name),
            AutoFail::FromInitList => {
                self.diag(DiagId::sema_init_capture_deduction_failure_from_init_list, loc).arg(name)
            }
            AutoFail::ParenBraces(nested) => {
                self.diag(DiagId::sema_init_capture_paren_braces, lbrace).arg(name).arg(nested)
            }
            _ => return false,
        };
        self.emit(msg);
        true
    }

    /// A `constexpr` variable's initializer must be a constant expression
    /// ([dcl.constexpr]/10); its value is recorded.
    pub(crate) fn check_constexpr_init(&mut self, v: DeclId, e: ExprId) {
        if self.ast.decls.begin_eval(v).is_err() {
            return;
        }
        // clang reports at the variable (`CheckCompleteVariableDeclaration`).
        let loc = self.ast.decls.get(v).loc();
        if self.ast.decls.get(v).has(DeclFlags::INVALID) {
            self.ast.decls.finish_eval(v, None);
            return;
        }
        if self.has_errors_in(e)
            || self.expr_is_value_dependent(e)
            || self.ast.decls.get(v).ty().is_some_and(|t| self.is_dependent_type(t))
        {
            // A dependent initializer is checked at instantiation.
            self.ast.decls.finish_eval(v, None);
            return;
        }
        match self.eval_const(e, EvalContext::init(v, loc)) {
            Ok(cv) => self.ast.decls.finish_eval(v, Some(cv)),
            Err(_) if self.refers_to_invalid_var(e) => self.ast.decls.finish_eval(v, None),
            Err(mut f) => {
                self.ast.decls.finish_eval(v, None);
                let name = self.name_for_diagnostic(v);
                let at = Self::fold_lone_invalid(&mut f, loc);
                let id = if f.kind == kiln_oven::FailKind::Destruction {
                    DiagId::sema_constexpr_var_requires_const_destruction
                } else {
                    DiagId::sema_constexpr_var_non_constant
                };
                let msg = self.diag(id, at).arg(name);
                let msg = Self::with_failure_notes(msg, f);
                self.emit(msg);
            }
        }
    }

    /// Remembers where `constinit` was written on `v`.
    pub(crate) fn record_constinit(&mut self, v: DeclId, ds: &DeclSpec) {
        if let Some(at) = ds.constinit
            && self.ast.decls.var(v).is_some_and(|x| x.spec.is_constinit())
        {
            self.constinit_locs.insert(v, at);
        }
    }

    /// The declaration of `v`'s entity that has `constinit`, and where
    /// (in its pattern, for an instantiated one).
    pub(crate) fn constinit_of(&self, v: DeclId) -> Option<(DeclId, Loc)> {
        let (d, mut at) = if let Some(&at) = self.constinit_locs.get(&v) {
            (v, at)
        } else {
            self.ast.decls.redecl(v)?;
            self.ast.redecls_of(v).find_map(|d| self.constinit_locs.get(&d).map(|&at| (d, at)))?
        };
        let mut x = d;
        while let Some(p) = self.ast.instantiated_from(x) {
            if let Some(&pat) = self.constinit_locs.get(&p) {
                at = pat;
            }
            x = p;
        }
        Some((d, at))
    }

    /// [dcl.constinit]/2: the initializing declaration of a `constinit`
    /// variable has a constant initializer (clang
    /// `CheckCompleteVariableDeclaration`). Only the initialization is
    /// evaluated, and its value is not made usable in constant
    /// expressions; destruction is not checked. Whether `v` is `constinit`
    /// and its initializer was found to be a constant initializer.
    fn check_constinit(&mut self, v: DeclId) -> bool {
        let vn = *self.ast.decls.get(v);
        if vn.kind() != DeclKind::Var || vn.has(DeclFlags::INVALID) {
            return false;
        }
        let Some((c, at)) = self.constinit_of(v) else { return false };
        let loc = vn.loc();
        if c != v {
            let name = self.decl_name(v);
            let note = self.diag(DiagId::note_constinit_missing_here, at);
            let msg = self.diag(DiagId::sema_missing_constinit, loc).arg(name).with_note(note);
            self.emit(msg);
        }
        let Some(e) = self.ast.decls.var(v).and_then(|x| x.init) else { return false };
        if self.has_errors_in(e)
            || self.expr_is_value_dependent(e)
            || vn.ty().is_some_and(|t| self.is_dependent_type(t))
        {
            return false;
        }
        if let Err(mut f) = self.eval_const(e, EvalContext::init(v, loc)) {
            if f.kind == kiln_oven::FailKind::Destruction {
                return true;
            }
            let err_at = Self::fold_lone_invalid(&mut f, loc);
            let note = self.diag(DiagId::note_constinit_specified_here, at);
            let msg = self.diag(DiagId::sema_constinit_non_constant, err_at).with_note(note);
            let msg = Self::with_failure_notes(msg, f);
            self.emit(msg);
            return false;
        }
        true
    }

    /// [dcl.constexpr]/6: a `constexpr` variable has a literal type
    /// ([basic.types.general]/10). clang checks it as soon as the type is
    /// known (`CheckVariableDeclarationType`: at the declarator, or after
    /// deducing a placeholder), reports `RequireLiteralType`'s notes and
    /// marks the variable invalid, so its initializer initializes nothing.
    /// False after a diagnostic.
    pub(crate) fn check_constexpr_var_literal(&mut self, v: DeclId) -> bool {
        let vn = *self.ast.decls.get(v);
        if vn.has(DeclFlags::INVALID) {
            return true;
        }
        // `RequireLiteralType` first requires a complete type, which
        // instantiates a class template specialization ([temp.inst]/2).
        if self.ast.decls.var(v).is_some_and(|x| x.spec.is_constexpr())
            && let Some(q) = vn.ty()
            && !self.is_error_type(q)
            && !self.is_dependent_type(q)
        {
            let _ = self.require_complete_silent(self.base_element(q), vn.loc());
        }
        if !self.constexpr_var_non_literal(v) {
            return true;
        }
        let q = vn.ty().unwrap_or(QualType::ERROR);
        let msg = self.diag(DiagId::sema_constexpr_var_non_literal, vn.loc()).arg(self.ty_arg(q));
        let msg = self.with_non_literal_notes(msg, q);
        self.emit(msg);
        self.ast.decls.set_flags(v, DeclFlags::INVALID);
        false
    }

    /// [dcl.constexpr]/4 (clang `CheckConstexprFunctionDefinition`, which
    /// applies it to every non-static member function): a `constexpr`
    /// member function defined outside an instantiation, in a class with
    /// virtual bases, is an error with a note per virtual base; the
    /// function is invalid. Before C++26 only (P3533R2).
    fn check_constexpr_virtual_base(&mut self, f: DeclId) {
        let kind = self.kind_of(f);
        let member = matches!(
            kind,
            DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
        );
        // P3533R2 lifts the restriction in C++26.
        if !member
            || self.std >= kiln_lex::Std::Cxx26
            || self.is_static_method(f)
            || !self.inst.stack.frames.is_empty()
            || self.ast.decls.get(f).has(DeclFlags::INVALID)
            || !self
                .ast
                .decls
                .function(f)
                .is_some_and(|x| x.spec.constexpr_kind() != ConstexprKind::None)
        {
            return;
        }
        let Some(class) = self.ast.decls.get(f).ctx() else { return };
        let Some(def) = self.ast.definition(class) else { return };
        if !self.defn_bits(def).contains(DefinitionBits::HAS_VIRTUAL_BASES) {
            return;
        }
        let vbases = self.virtual_bases(def);
        if vbases.is_empty() {
            return;
        }
        let tag = self.ast.decls.record(def).map_or(0, |r| 2 * u8::from(r.tag == TagKind::Class));
        let mut msg = self
            .diag(DiagId::sema_constexpr_virtual_base, self.ast.decls.get(f).loc())
            .arg(u8::from(kind == DeclKind::CXXConstructor))
            .arg(tag)
            .arg(u8::from(vbases.len() != 1));
        for (_, b) in vbases {
            msg = msg.with_note(self.diag(DiagId::note_constexpr_virtual_base_here, b.loc));
        }
        self.emit(msg);
        self.ast.decls.set_flags(f, DeclFlags::INVALID);
    }

    /// Whether `v` is a `constexpr` variable of a known, complete
    /// non-literal type ([`Sema::check_constexpr_var_literal`]).
    fn constexpr_var_non_literal(&self, v: DeclId) -> bool {
        if !self.ast.decls.var(v).is_some_and(|x| x.spec.is_constexpr()) {
            return false;
        }
        let Some(q) = self.ast.decls.get(v).ty() else { return false };
        if self.is_error_type(q) || self.is_dependent_type(q) || self.contains_undeduced_auto(q) {
            return false;
        }
        if let Some(r) = self.ast.types.as_record(self.base_element(q)) {
            // An incomplete class is diagnosed as such.
            let complete = self
                .ast
                .definition(r)
                .and_then(|d| self.ast.decls.record(d))
                .is_some_and(|x| x.defn.contains(DefinitionBits::COMPLETE));
            if !complete {
                return false;
            }
        }
        !self.is_literal_type(q)
    }

    pub(crate) fn has_errors_in(&self, e: ExprId) -> bool {
        self.ast.exprs.get(e).kind() == ExprKind::RecoveryExpr
            || self.is_error_type(self.ast.exprs.get(e).ty())
    }

    /// Whether `e` names a variable whose declaration is invalid (a
    /// variable template specialization whose initializer failed to
    /// instantiate), directly or through the initializer of a variable
    /// it names: clang's `DeclRefExpr` error dependence, which silences
    /// the "not a constant expression" diagnostics that would follow the
    /// error already reported. Asked only after an evaluation failed; the
    /// walk is bounded.
    pub(crate) fn refers_to_invalid_var(&self, e: ExprId) -> bool {
        let mut stack = vec![e];
        let mut seen = kiln_source::fx::FxHashSet::default();
        let mut budget = 1usize << 14;
        while let Some(x) = stack.pop() {
            if budget == 0 {
                break;
            }
            budget -= 1;
            if self.ast.exprs.get(x).kind() == ExprKind::DeclRefExpr {
                let d = self.ast.exprs.decl(x);
                let n = self.ast.decls.get(d);
                if n.kind() == DeclKind::Var && seen.insert(d) {
                    if n.has(DeclFlags::INVALID) {
                        return true;
                    }
                    if let Some(init) = self.ast.decls.var(d).and_then(|v| v.init) {
                        stack.push(init);
                    }
                }
                continue;
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        false
    }

    /// A variable declared without an initializer ([dcl.init]/7).
    pub fn act_on_uninit(&mut self, v: DeclId) {
        self.act_on_uninit_inner(v);
        if self.is_var_definition(v) {
            self.check_static_tls(v, None);
            self.check_constinit(v);
        }
        self.check_consteval_only_var(v);
    }

    fn act_on_uninit_inner(&mut self, v: DeclId) {
        self.lambda_mangle.var = None;
        // [class.static.data]/3: the default-initialization of an
        // out-of-line static data member definition is in the scope of
        // its class too, so its constructor's access is checked as from
        // a member (`F<int> F<int>::inst;` with a private default
        // constructor, the reference workload's Metrics).
        self.default_init_var(v);
        self.initializing = None;
    }

    fn default_init_var(&mut self, v: DeclId) {
        if self.ast.decls.decomp_bindings(v).is_some() {
            self.decomposition_requires_init(v);
            return;
        }
        let vn = *self.ast.decls.get(v);
        if vn.kind() != DeclKind::Var
            || self.uninit_array_or_reference(v)
            || vn.has(DeclFlags::INVALID)
            || self.is_member_spec_declaration(v)
        {
            return;
        }
        // An in-class static data member declaration is not a definition
        // ([class.static.data]/3): its type may be incomplete.
        if self.is_in_class_static(v)
            && !self.is_var_definition(v)
            && !self.is_spec_of_out_of_line_def(v)
        {
            // But it cannot deduce a class type without an initializer.
            if let Some(ty) = vn.ty()
                && self.is_ctad_placeholder(ty)
            {
                let _ = self.ctad_uninit(v, ty, vn.loc());
            }
            return;
        }
        // [depr.static.constexpr]: a redundant out-of-class redeclaration
        // of an inline static data member initialized in its class.
        if !self.is_in_class_static(v) {
            let in_class = self
                .ast
                .redecls_of(v)
                .find(|&x| x != v && self.is_in_class_static(x) && self.is_var_definition(x));
            if let Some(x) = in_class {
                if self.ast.decls.var(x).is_some_and(|y| y.spec.is_constexpr()) {
                    self.error(DiagId::sema_redundant_constexpr_static_def, vn.loc());
                }
                return;
            }
            // [class.static.data]/4: the definition of a `const` static
            // data member initialized in its class has no initializer.
            if self
                .ast
                .redecls_of(v)
                .any(|x| x != v && self.ast.decls.var(x).is_some_and(|y| y.init.is_some()))
            {
                return;
            }
        }
        let spec = self.ast.decls.var(v).map(|x| x.spec).unwrap_or_default();
        let ty = vn.ty().unwrap_or(QualType::ERROR);
        let loc = vn.loc();
        let name = self.name_for_diagnostic(v);
        let Some(ty) = self.ctad_uninit(v, ty, loc) else { return };
        // [temp.res.general]/6: checked when instantiated.
        if self.is_error_type(ty) || self.ast.types.is_dependent(ty) {
            return;
        }
        if self.contains_undeduced_auto(ty) {
            // [dcl.spec.auto.general]/4 (clang `err_auto_var_requires_init`).
            // During instantiation the pattern had an initializer whose
            // substitution failed (already reported).
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
            if !self.inst.stack.is_empty() {
                return;
            }
            // clang passes the declaration's name here, not the
            // declaration: a specialization prints without its arguments.
            let name = self.decl_name(v);
            let msg =
                self.diag(DiagId::sema_auto_var_requires_init, loc).arg(name).arg(self.ty_arg(ty));
            self.emit(msg);
            return;
        }
        // [dcl.link]/8: the declaration directly contained in a braceless
        // linkage specification (`extern "C" int x[];`) is one, as `extern`.
        let declaration_only =
            spec.storage() == VarStorage::Extern || self.ast.decls.is_single_line_linkage(v);
        if self.ast.types.is_reference(ty) {
            if !declaration_only {
                // Repeats the template definition's report (WP-318).
                let msg = self.diag(DiagId::sema_reference_var_requires_init, loc).arg(name);
                self.emit(msg.per_instantiation());
            }
            return;
        }
        if declaration_only {
            return;
        }
        if self.uninit_incomplete(v, ty, loc) {
            return;
        }
        let quals = self.ast.types.effective_quals(ty);
        let elem = self.base_element(ty);
        if spec.is_constexpr() {
            // [dcl.constexpr]/6: a default-initialized `constexpr` object
            // of class type is initialized by its default constructor,
            // which must be a constant expression; anything else has no
            // initializer at all.
            if self.element_class(elem).is_some() {
                let errors = self.diags.error_count();
                self.check_class_default_init(v, ty, loc);
                if self.diags.error_count() > errors {
                    return;
                }
                if let Some(init) = self.ast.decls.var(v).and_then(|x| x.init) {
                    self.check_constexpr_init(v, init);
                }
                return;
            }
            let msg = self.diag(DiagId::sema_constexpr_var_non_constant, loc).arg(name);
            self.emit(msg);
            return;
        }
        if self.element_class(elem).is_some() {
            // [dcl.init.general]/7–8, [class.dtor]/15 (WP-067a).
            self.check_class_default_init(v, ty, loc);
        } else if quals.is_const() {
            let msg = self.diag(DiagId::sema_default_init_const, loc).arg(self.ty_arg(ty));
            self.emit(msg);
        }
    }

    /// clang's `ActOnUninitializedDecl` checks before it stops at an
    /// invalid declaration or a dependent type, for a variable definition
    /// without an initializer: an array of unknown bound ([dcl.array]/5,
    /// "definition of variable with array type needs an explicit size or
    /// an initializer"; a `constexpr` one is "must be initialized by a
    /// constant expression"), which makes it invalid; and, for an invalid
    /// variable, a reference type, so `S::name &r;` with no `S::name`
    /// (whose type recovers to `int`, [dcl.type.simple]) reports
    /// "declaration of reference variable 'r' requires an initializer"
    /// after "no type named 'name' in 'S'" (WP-308); and, for a dependent
    /// reference type, `T &r;` in a template pattern reports at the
    /// definition (clang checks the reference before it stops at a
    /// dependent type) and again, with the instantiation note, for each
    /// instantiation (WP-318). True if it reported.
    fn uninit_array_or_reference(&mut self, v: DeclId) -> bool {
        let vn = *self.ast.decls.get(v);
        let Some(ty) = vn.ty() else { return false };
        let spec = self.ast.decls.var(v).map(|x| x.spec).unwrap_or_default();
        if spec.storage() == VarStorage::Extern
            || !self.is_var_definition(v)
            || self.is_error_type(ty)
            || self.contains_undeduced_auto(ty)
        {
            return false;
        }
        if matches!(self.ast.types.canonical_view(ty), TypeView::IncompleteArray(_)) {
            let msg = if spec.is_constexpr() {
                let name = self.name_for_diagnostic(v);
                self.diag(DiagId::sema_constexpr_var_non_constant, vn.loc()).arg(name)
            } else {
                self.diag(DiagId::sema_typecheck_incomplete_array_needs_initializer, vn.loc())
            };
            self.emit(msg);
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
            return true;
        }
        if !self.ast.types.is_reference(ty)
            || !(vn.has(DeclFlags::INVALID) || self.ast.types.is_dependent(ty))
        {
            return false;
        }
        let name = self.decl_name(v);
        let msg = self.diag(DiagId::sema_reference_var_requires_init, vn.loc()).arg(name);
        self.emit(msg);
        true
    }

    // -- function definitions ----------------------------------------------------

    /// [except.spec]/5 at the definition of a templated class
    /// (clang `CheckEquivalentExceptionSpec` from `MergeFunctionDecl`,
    /// which runs for a friend of a dependent context too): the function
    /// of namespace `ns` that non-dependent unqualified friend `new`
    /// redeclares, found as for a friend of a non-templated class. The
    /// friend stays out of its redeclaration chain until instantiated.
    /// The instantiation's own check repeats what was reported here,
    /// which deduplication drops (spec/diag.md §7.4) unless a
    /// `noexcept(expr)` was compared again (`per_instantiation_exc`).
    fn pattern_friend_prev(&mut self, ns: DeclId, dname: DeclName, new: NewFn) -> Option<DeclId> {
        let found: Vec<DeclId> = self
            .redecl_candidates(ns, dname, &new)
            .into_iter()
            .filter(|&(x, f)| {
                !f.contains(LookupFlags::VIA_INLINE)
                    && !self.is_dependent_friend_fn(x)
                    && self.kind_of(x) != DeclKind::UsingShadow
            })
            .map(|(x, _)| x)
            .collect();
        let p = self.pick_prev_function(&found, new)?;
        let pty = self.ast.decls.get(p).ty()?;
        self.same_ignoring_exc_spec(pty, new.ty).then_some(p)
    }

    /// Whether function (template) `f` is a friend declared in a
    /// templated class: it is a pattern that only declares the function
    /// when the class is instantiated ([temp.inst]/4, [temp.friend]), so
    /// another friend declaration does not redeclare it and two such
    /// definitions are not a redefinition before then (libc++ `expected` and its `void` partial
    /// specialization both define `operator==(const expected&, const
    /// unexpected<_E2>&)`; clang's `CheckForFunctionRedefinition`).
    pub(crate) fn is_dependent_friend_fn(&self, f: DeclId) -> bool {
        let f = if self.kind_of(f) == DeclKind::FunctionTemplate {
            self.pattern_of(f).unwrap_or(f)
        } else {
            f
        };
        self.ast.decls.get(f).misc().friend() != FriendKind::None
            && self.in_templated_class(self.ast.decls.lexical_ctx(f))
    }

    /// [namespace.udecl]/10: an unqualified friend function (template)
    /// `new` that corresponds to the target of one of the namespace's
    /// using-declarators `shadows` would redeclare it: "cannot befriend
    /// target of using declaration" at `loc` (clang). Returns whether it
    /// was reported.
    fn friend_befriends_using_target(&mut self, shadows: &[DeclId], new: NewFn, loc: Loc) -> bool {
        for &s in shadows.iter().rev() {
            let t = self.ast.strip_shadow(s);
            let corresponds = match self.kind_of(t) {
                DeclKind::FunctionTemplate => {
                    let head = self.ast.template_info(t).map(|i| i.params);
                    self.pattern_of(t).is_some_and(|p| self.fn_corresponds(p, head, new))
                }
                k if k.payload() == kiln_ast::decl::PayloadClass::Function => {
                    self.fn_corresponds(t, None, new)
                }
                _ => false,
            };
            if corresponds {
                let msg = self
                    .diag(DiagId::sema_using_decl_friend, loc)
                    .with_note(
                        self.diag(DiagId::note_using_decl_target, self.ast.decls.get(t).loc()),
                    )
                    .with_note(self.diag(DiagId::note_using_decl, self.ast.decls.get(s).loc()));
                self.emit(msg);
                return true;
            }
        }
        false
    }

    /// True if lexical context `cur` is, or is nested in, a class template
    /// or partial specialization before any namespace scope. (A partial
    /// specialization is a `CXXRecord` with partial-specialization info:
    /// libstdc++'s `_Node_handle<_Value, _Value, _NodeAlloc>` befriends
    /// `_Rb_tree` before its definition, and that friend must not spell
    /// the injected-class-name, WP-366.)
    pub(crate) fn in_templated_class(&self, mut cur: Option<DeclId>) -> bool {
        while let Some(c) = cur {
            match self.kind_of(c) {
                DeclKind::ClassTemplatePartialSpecialization => return true,
                DeclKind::CXXRecord
                    if self.ast.described_template(c).is_some()
                        || self.ast.partial_spec_info(c).is_some() =>
                {
                    return true;
                }
                DeclKind::TranslationUnit | DeclKind::Namespace => return false,
                _ => {}
            }
            cur = self.ast.decls.lexical_ctx(c);
        }
        false
    }

    /// Starts the definition of function `f` at `{`: checks for a
    /// redefinition, opens the function scope with the parameters.
    pub fn act_on_start_function_def(&mut self, f: DeclId) {
        self.check_alias_definition(f);
        // A deleted function is defined by its first declaration.
        let deleted = Some(self.ast.first(f)).filter(|&p| p != f && self.is_deleted_fn(p));
        if let Some(prev) = deleted.or_else(|| self.ast.definition(f))
            && prev != f
            && !self.is_dependent_friend_fn(f)
            && !self.is_dependent_friend_fn(prev)
            // The same function defined in another unit's global module
            // fragment: merged, its body kept (spec/modules.md §6).
            && (deleted.is_some() || !self.module_merge_hidden_definition(prev))
        {
            let msg = self
                .diag(DiagId::sema_redefinition, self.ast.decls.get(f).loc())
                .arg(self.decl_name(f));
            let msg = self.note_prev(msg, prev, true);
            self.emit(msg);
            // clang `CheckForFunctionRedefinition` invalidates the
            // redefinition (a later redeclaration is not checked against it).
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
        // clang `ActOnStartOfFunctionDef`: a non-library builtin cannot be
        // defined (spec/sema.md §5.4).
        if let Some(id) = self.ast.decls.builtin_id(f)
            && let Some(info) = kiln_ast::decl::builtin::info(id)
        {
            let loc = self.ast.decls.get(f).loc();
            self.emit(self.diag(DiagId::sema_builtin_definition, loc).arg(info.name));
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
        // clang `ActOnStartOfFunctionDef` resolves the exception
        // specification of every definition: a destructor or defaulted
        // special member nothing uses still needs its implicit `noexcept`
        // for codegen's terminate scope ([except.spec]/8, spec/sema.md §4.7).
        if !self.in_template() {
            self.resolve_exception_spec(f, self.ast.decls.get(f).loc());
        }
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        // The return type as written: diagnostics name its typedef.
        let ret = self.sugared_proto(fty).map_or(QualType::ERROR, |p| p.ret());
        let pushed = self.enter_definition_scopes(f);
        self.fn_saved.push(crate::class::SavedFn { func: self.func.take(), pushed });
        let mut fs = Scope::new(ScopeKind::Function, None);
        fs.errors = self.diags.unrecoverable_error_count();
        self.scopes.push(fs);
        self.ctx = f;
        let params: Vec<DeclId> = self
            .ast
            .decls
            .function(f)
            .map(|x| self.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        for p in params {
            let n = self.ast.decls.get(p).name();
            if let Some(sym) = n.as_ident() {
                self.push_local(sym, p);
            }
            let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
            // clang `CheckParmsForFunctionDef`: a parameter whose class
            // template specialization fails to instantiate is invalid.
            if self.invalid_by_failed_spec(p, pty, self.ast.decls.get(p).loc()) {
                continue;
            }
            if !self.require_complete(pty, self.ast.decls.get(p).loc())
                && !self.ast.types.is_reference(pty)
                && !self.is_error_type(pty)
            {
                let msg = self
                    .diag(DiagId::sema_typecheck_decl_incomplete_type, self.ast.decls.get(p).loc())
                    .arg(self.ty_arg(pty));
                self.emit(self.incomplete_note(msg, pty));
            }
            self.mark_callee_destroyed_param(p, pty);
        }
        self.check_abstract_signature(f);
        self.begin_function_body(f, ret);
        // A generic lambda's call operator instantiated later: `this` is
        // the enclosing one, not the closure's ([expr.prim.this]/2).
        let this_ty = if self.is_lambda_call_op(f) {
            self.instantiated_lambda_this(f)
        } else {
            self.method_this_type(f)
        };
        if let Some(t) = this_ty
            && let Some(state) = self.func.as_mut()
        {
            state.this_ty = Some(t);
        }
    }

    /// Finishes the definition of `f` with `body`.
    pub fn act_on_finish_function_def(&mut self, f: DeclId, body: StmtId, rbrace: Loc) {
        self.check_function_try_block(f, body);
        self.check_naked_body(f, body);
        let body = self.end_function_body(body, rbrace);
        // clang `ActOnFinishFunctionBody` -> `checkTypeSupport`.
        if let Some(fty) = self.ast.decls.get(f).ty() {
            self.check_sve_support(fty, self.ast.decls.get(f).loc());
        }
        self.finish_source_locs(f);
        if self.ast.definition(f).is_none() {
            let _ = self.builder().set_body(f, body);
        }
        self.check_constexpr_virtual_base(f);
        self.check_constexpr_body_vars(f, body);
        if self.kind_of(f) == DeclKind::CXXDestructor
            && let Some(class) = self.ast.decls.get(f).ctx()
            && !self.in_template()
        {
            // [class.dtor]/13: the subobject destructors are odr-used by a
            // destructor definition (clang `ActOnFinishFunctionBody`).
            let loc = self.ast.decls.get(f).loc();
            self.mark_base_and_member_dtors_referenced(loc, class);
            self.check_destructor_delete(f, class);
            // [basic.def.odr]/8: and uses the vtable.
            self.mark_vtable_used(class, loc);
        }
        if !self.in_template() {
            self.mark_key_function_vtable_used(f);
        }
        self.pop_scope();
        if let Some(saved) = self.fn_saved.pop() {
            for _ in 0..saved.pushed {
                self.pop_scope();
            }
            self.func = saved.func;
        }
        self.ctx = self
            .ast
            .decls
            .lexical_ctx(f)
            .or_else(|| self.ast.decls.get(f).ctx())
            .unwrap_or(self.ast.tu);
    }
}

// -- tags, enumerations, namespaces, attributes ---------------------------------

impl Sema<'_, '_> {
    /// The type of tag `d` (shared by all its redeclarations).
    pub fn tag_type(&self, d: DeclId) -> QualType {
        self.ast.decls.get(self.ast.first(d)).ty().unwrap_or(QualType::ERROR)
    }

    /// The type an elaborated-type-specifier or class-specifier names:
    /// `struct S` is `'struct S':'S'` ([dcl.type.elab]).
    /// Inside the pattern (or a partial specialization) of a class
    /// template, `class X` naming its injected-class-name is `X<P...>`
    /// ([temp.local]/1), substituted with the class.
    pub fn elaborated_tag_type(&mut self, d: DeclId, keyword: TagKeyword) -> QualType {
        let t = self.injected_tag_type(d).unwrap_or_else(|| self.tag_type(d));
        self.ast.types.elaborated(keyword, None, t)
    }

    /// The injected-class-name type `X<P...>` when tag `d` is the pattern
    /// or a partial specialization of class template `X` and the current
    /// context is inside it ([temp.local]/1); `None` otherwise (a
    /// `template<class U> friend class X;` names the template itself).
    pub(crate) fn injected_tag_type(&mut self, d: DeclId) -> Option<QualType> {
        if self.kind_of(d) != DeclKind::CXXRecord || self.friend_class_template.is_some() {
            return None;
        }
        let first = self.ast.first(d);
        if self.ast.described_template(first).is_none()
            && self.ast.partial_spec_info(first).is_none()
        {
            return None;
        }
        let tmpl = self.injected_template_of(d)?;
        let c = self.injected_class_decl(tmpl)?;
        if self.ast.first(c) != first {
            return None;
        }
        self.injected_class_type(tmpl)
    }

    /// Marks a type-name's declaration referenced ([basic.def.odr]: clang's
    /// `isReferenced` for types): the declaration lookup found, not the
    /// rest of its chain (clang `isThisDeclarationReferenced`).
    pub fn mark_type_referenced(&mut self, d: DeclId) {
        self.ast.decls.set_flags(d, DeclFlags::REFERENCED);
    }

    pub(crate) fn tag_kind_of(keyword: TagKeyword) -> TagKind {
        match keyword {
            TagKeyword::Class => TagKind::Class,
            TagKeyword::Union => TagKind::Union,
            _ => TagKind::Struct,
        }
    }

    pub(crate) fn tag_matches(&self, d: DeclId, keyword: TagKeyword) -> bool {
        let n = self.ast.decls.get(d);
        match keyword {
            TagKeyword::Enum => n.kind() == DeclKind::Enum,
            TagKeyword::Union => self.ast.decls.record(d).is_some_and(|r| r.tag == TagKind::Union),
            _ => self.ast.decls.record(d).is_some_and(|r| r.tag != TagKind::Union),
        }
    }

    /// The scope a new tag declared by `struct S;`-style uses goes into
    /// ([basic.scope.pdecl]/7): the nearest enclosing namespace or block
    /// scope, skipping classes and prototypes for a mere reference.
    fn tag_decl_context(&self, use_: TagUse) -> DeclId {
        if use_ != TagUse::Reference {
            return self.ctx;
        }
        let mut ctx = self.ctx;
        for _ in 0..256 {
            let k = self.ast.decls.get(ctx).kind();
            if matches!(k, DeclKind::CXXRecord | DeclKind::Enum) {
                match self.ast.decls.get(ctx).ctx() {
                    Some(c) => ctx = c,
                    None => break,
                }
            } else {
                break;
            }
        }
        ctx
    }

    /// A class first declared by an elaborated-type-specifier (`struct S*`
    /// in a parameter or template argument, `friend class S;`) into
    /// namespace `h.ctx` ([basic.scope.pdecl]/7, [class.friend]/11): even
    /// when written inside a template, it is a member of the namespace,
    /// neither dependent nor a templated entity (clang gives it the
    /// namespace as its `DeclContext`), so its type is not dependent and
    /// a later definition at namespace scope completes the same
    /// non-dependent type. A class declared in a block or class scope
    /// keeps the enclosing template's dependence.
    fn namespace_tag_record(&mut self, h: DeclHead, tag: TagKind) -> DeclId {
        let ns = self.ast.redecl_context(h.ctx);
        if !matches!(self.kind_of(ns), DeclKind::Namespace | DeclKind::TranslationUnit) {
            return self.builder().record(h, tag, None);
        }
        let saved = std::mem::take(&mut self.ast.templates.dependent_depth);
        let d = self.builder().record(h, tag, None);
        self.ast.templates.dependent_depth = saved;
        d
    }

    /// The promotion type of an enumeration with underlying type `t`
    /// ([conv.prom]/4).
    fn enum_promotion(&self, t: QualType) -> QualType {
        match constfold::int_info(&self.ast, t) {
            Some((bits, signed)) if bits < 32 || (bits == 32 && signed) => QualType::INT,
            Some((32, false)) => QualType::UINT,
            _ => self.ast.types.canonical(t).without_quals(),
        }
    }

    /// The declarations a tag named `n` may refer to or redeclare
    /// ([dcl.type.elab]/2-3); `templ` when it has a template head
    /// ([temp.class]: it redeclares the template's latest pattern).
    fn tag_redecl_lookup(
        &mut self,
        n: Symbol,
        qualifier: Option<DeclId>,
        use_: TagUse,
        templ: bool,
        opaque_enum: bool,
        nloc: Loc,
    ) -> Vec<DeclId> {
        match (qualifier, use_) {
            (None, _) if templ => self
                .lookup_current_scope(n)
                .into_iter()
                .filter(|&d| self.kind_of(d) == DeclKind::ClassTemplate)
                .filter_map(|d| self.pattern_of(d).map(|p| self.ast.latest(p)))
                .collect(),
            // `template<class T> template<class U> struct A<T>::B { ... }`:
            // the member template's latest pattern ([temp.mem.class]).
            (Some(q), _) if templ => self
                .lookup_qualified(q, n, LookupKind::All)
                .decls
                .into_iter()
                .filter(|&d| self.kind_of(d) == DeclKind::ClassTemplate)
                .filter_map(|d| self.pattern_of(d).map(|p| self.ast.latest(p)))
                .collect(),
            (Some(q), _) => self.lookup_qualified(q, n, LookupKind::Tag).decls,
            // `struct X *p;` naming tags of two namespaces, e.g. an inline
            // namespace's and its parent's ([namespace.def.general]/7,
            // [basic.lookup.general]/1): clang's `err_ambiguous_reference`,
            // recovering with the last tag found.
            (None, TagUse::Reference) if !opaque_enum => {
                let found = self.lookup_name(n, LookupKind::Tag);
                if self.is_ambiguous_lookup(&found) {
                    self.diagnose_ambiguous_reference(DeclName::ident(n), &found.decls, true, nloc);
                }
                found.decls
            }
            (None, _) => self
                .lookup_current_scope(n)
                .into_iter()
                .filter(|&d| is_tag_kind(self.ast.decls.get(d).kind()))
                .collect(),
        }
    }

    /// A qualified tag name `A::X` that names no tag of `A`: another kind
    /// of member redefined (`struct A<T>::D` for a member template `D`),
    /// else clang's `err_not_tag_in_scope`.
    fn diag_qualified_tag_not_found(
        &mut self,
        keyword: TagKeyword,
        n: Symbol,
        nloc: Loc,
        q: DeclId,
        use_: TagUse,
        templated: bool,
    ) {
        if use_ == TagUse::Definition
            && let Some(&other) = self.lookup_qualified(q, n, LookupKind::All).decls.first()
        {
            let other = self.ast.first(self.pattern_of(other).unwrap_or(other));
            let msg =
                self.diag(DiagId::sema_redefinition_different_kind, nloc).arg(sym_str(self, n));
            let msg = self.note_prev(msg, other, true);
            self.emit(msg);
            return;
        }
        if templated {
            // A qualified class template declaration names a member
            // template of the nominated scope (clang's `CheckClassTemplate`
            // reports `err_member_decl_does_not_match` as a definition,
            // with or without a body).
            let scope = if self.kind_of(q) == DeclKind::CXXRecord {
                self.class_context_str(q)
            } else {
                self.qualifier_desc(q)
            };
            let msg =
                self.diag(DiagId::sema_member_def_no_match, nloc).arg(sym_str(self, n)).arg(scope);
            self.emit(msg);
            return;
        }
        let kind = match keyword {
            TagKeyword::Union => 2u8,
            TagKeyword::Class => 3,
            TagKeyword::Enum => 4,
            _ => 0,
        };
        let scope = match self.kind_of(q) {
            DeclKind::CXXRecord => self.class_context_str(q),
            DeclKind::Namespace | DeclKind::TranslationUnit => self.qualifier_desc(q),
            _ => format!("'{}'", self.decl_name(q)),
        };
        let msg = self
            .diag(DiagId::sema_not_tag_in_scope, nloc)
            .arg(kind)
            .arg(sym_str(self, n))
            .arg(scope);
        self.emit(msg);
    }

    /// `enum E : T`: the underlying type must be integral ([dcl.enum]/2);
    /// clang (`CheckEnumUnderlyingType`) takes only the standard integer
    /// types, so a bit-precise one is rejected too (spec/types.md §21).
    /// An invalid one recovers to `int`, as clang's `ActOnTag` does. The
    /// parser checks the enum-base at its type (`loc`, as clang); an
    /// instantiated dependent one is checked again in [`Self::act_on_tag`].
    pub fn check_enum_fixed_type(&mut self, fixed: Option<QualType>, loc: Loc) -> Option<QualType> {
        if let Some(f) = fixed {
            let bit_int = matches!(self.ast.types.canonical_view(f), TypeView::BitInt { .. });
            let ok = (self.ast.types.is_integral(f) && !bit_int)
                || self.is_error_type(f)
                || self.ast.types.is_dependent(f);
            if !ok {
                let id = if bit_int {
                    DiagId::sema_enum_bit_int_underlying
                } else {
                    DiagId::sema_enum_invalid_underlying
                };
                let msg = self.diag(id, loc).arg(self.ty_arg(f));
                self.emit(msg);
                return Some(QualType::INT);
            }
        }
        fixed
    }

    /// Checks a definition's member specialization header, then returns
    /// whether a qualified class head is `template<> struct O<long>::I`
    /// ([temp.expl.spec]/1, a member class of an implicit
    /// specialization). The header then belongs to the qualifier, and the
    /// class's own members are ordinary declarations, not explicit
    /// specializations, so the pending explicit specialization is taken.
    fn begin_member_class_spec(
        &mut self,
        name: Option<(Symbol, Loc)>,
        qualifier: Option<DeclId>,
        use_: TagUse,
        loc: Loc,
    ) -> bool {
        if use_ == TagUse::Definition
            && let Some((_, nloc)) = name
        {
            self.check_member_spec_header(qualifier, nloc, loc);
        }
        let spec = qualifier.is_some()
            && matches!(use_, TagUse::Definition | TagUse::Declaration)
            && self.templ.explicit_spec.is_some()
            && self.at_explicit_head();
        if spec {
            self.templ.explicit_spec = None;
        }
        spec
    }

    /// Marks member class `d` explicitly specialized when `spec` (clang
    /// `CheckMemberSpecialization`): its out-of-line members are then
    /// defined without `template<>` ([temp.expl.spec]/5).
    fn finish_member_class_spec(&mut self, spec: bool, d: DeclId) {
        if spec && let Some((i, _)) = self.ast.member_spec_info(self.ast.first(d)) {
            self.ast.templates.update_member_spec(i, |m| {
                m.kind = kiln_ast::templates::SpecKind::ExplicitSpecialization;
            });
        }
    }

    /// The template head a tag declaration consumes: a class's own head,
    /// or none for an enumeration, whose heads (`template<class T> enum
    /// class A<T>::E { ... }`) belong to the qualifier's class templates
    /// ([temp.mem.enum]).
    fn tag_template_head(
        &mut self,
        is_enum: bool,
        qualifier: Option<DeclId>,
        use_: TagUse,
    ) -> Option<kiln_ast::ids::TemplateParamListId> {
        if !is_enum && matches!(use_, TagUse::Definition | TagUse::Declaration) {
            let head = self.take_template_head(qualifier);
            let _ = self.check_template_in_local_class(false, Loc::INVALID);
            return head;
        }
        if is_enum && qualifier.is_some() && use_ == TagUse::Definition {
            let _ = self.take_template_head(qualifier);
            self.templ.cur = None;
        }
        None
    }

    /// An uninitialized definition of `v` whose type `ty` (for a bounded
    /// array, its element type, which clang checks and names) is
    /// incomplete or fails to instantiate: reported, and `v` is invalid.
    fn uninit_incomplete(&mut self, v: DeclId, ty: QualType, loc: Loc) -> bool {
        let elem = self.bounded_array_element(ty);
        let vloc = self.ast.decls.get(v).loc();
        if self.invalid_by_failed_spec(v, elem, vloc) {
            return true;
        }
        if self.require_complete(elem, vloc) {
            return false;
        }
        let msg = if self.ast.types.is_void(elem) {
            self.diag(DiagId::sema_void_var, loc).arg(self.ty_arg(elem))
        } else {
            self.diag(DiagId::sema_typecheck_decl_incomplete_type, loc).arg(self.ty_arg(elem))
        };
        self.emit(self.incomplete_note(msg, elem));
        self.ast.decls.set_flags(v, DeclFlags::INVALID);
        true
    }

    /// A variable or parameter `d` of (non-reference) type `ty` whose class
    /// template specialization fails to instantiate at `loc` is marked
    /// invalid, quietly (clang's `RequireCompleteType` recovery).
    fn invalid_by_failed_spec(&mut self, d: DeclId, ty: QualType, loc: Loc) -> bool {
        if self.ast.types.is_reference(ty) || !self.instantiation_fails(ty, loc) {
            return false;
        }
        self.ast.decls.set_flags(d, DeclFlags::INVALID);
        true
    }

    /// `struct N::X;` outside an explicit specialization or instantiation
    /// ([dcl.type.elab]/1, [dcl.enum]/1: a standalone class or opaque enum
    /// declaration cannot be qualified); clang's
    /// `err_standalone_class_nested_name_specifier` at the qualifier.
    pub fn check_standalone_tag_qualifier(&mut self, keyword: TagKeyword, qual_loc: Loc) {
        if self.at_explicit_head() || self.templ.member_spec.is_some() {
            return;
        }
        let kind = match keyword {
            TagKeyword::Class => 0u8,
            TagKeyword::Union => 2,
            TagKeyword::Enum => 3,
            _ => 1,
        };
        self.emit(self.diag(DiagId::sema_standalone_tag_qualifier, qual_loc).arg(kind));
    }

    /// An unqualified tag `n` that redeclares nothing, naming an entity
    /// of another kind (`kind_conflict`): an elaborated reference to a
    /// non-tag, a class template over a non-template, or a class over a
    /// template or typedef. True when diagnosed.
    fn diagnose_tag_kind_conflict(
        &mut self,
        keyword: TagKeyword,
        n: Symbol,
        use_: TagUse,
        template: bool,
        opaque_enum: bool,
        nloc: Loc,
    ) -> bool {
        match use_ {
            TagUse::Reference if !opaque_enum => {
                let o = self.lookup_name(n, LookupKind::Nested).decls;
                self.diagnose_tag_reference_non_tag(keyword, n, &o, nloc)
            }
            TagUse::Reference => false,
            _ if template => self.diagnose_template_kind_conflict(n, NewKind::ClassTemplate, nloc),
            _ => {
                self.diagnose_template_kind_conflict(n, NewKind::Tag, nloc)
                    || self.diagnose_tag_typedef_conflict(n, nloc)
            }
        }
    }

    /// A class-key or `enum` followed by a name ([dcl.type.elab],
    /// [class.pre], [dcl.enum]). `scoped` is `EnumData::SCOPED[_STRUCT]`
    /// bits; `fixed` the enum-base. Returns the tag declaration.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "one lookup-and-redeclaration sequence shared by every tag use"
    )]
    pub fn act_on_tag(
        &mut self,
        keyword: TagKeyword,
        name: Option<(Symbol, Loc)>,
        qualifier: Option<DeclId>,
        use_: TagUse,
        scoped: u32,
        fixed: Option<QualType>,
        loc: Loc,
    ) -> DeclId {
        let extraneous_head = self.extraneous_tag_head.take().map(|d| self.emit(d)).is_some();
        if use_ == TagUse::Friend {
            return self.act_on_friend_tag(keyword, name, qualifier, scoped, fixed, loc);
        }
        let is_enum = keyword == TagKeyword::Enum;
        let fixed = fixed.or(if is_enum && scoped != 0 { Some(QualType::INT) } else { None });
        let fixed = self.check_enum_fixed_type(fixed, loc);
        let member_class_spec = self.begin_member_class_spec(name, qualifier, use_, loc);
        let head = self.tag_template_head(is_enum, qualifier, use_);
        let mut prev = None;
        let mut redefined = false;
        // A name of another kind (`kind_conflict`): the tag is declared
        // anyway, invalid and hidden from lookup (clang's recovery as an
        // anonymous tag).
        let mut conflict = false;
        let mut respecialized = false;
        if let Some((n, nloc)) = name {
            if qualifier.is_none() && use_ != TagUse::Reference {
                self.check_template_param_shadow(n, nloc);
            }
            let opaque_enum = is_enum && fixed.is_some();
            let found =
                self.tag_redecl_lookup(n, qualifier, use_, head.is_some(), opaque_enum, nloc);
            if found.is_empty() && qualifier.is_none() {
                conflict = self.diagnose_tag_kind_conflict(
                    keyword,
                    n,
                    use_,
                    head.is_some(),
                    opaque_enum,
                    nloc,
                );
            }
            if let Some(&p) = found.last() {
                if self.tag_matches(p, keyword) {
                    prev = Some(p);
                    if extraneous_head && use_ != TagUse::Reference {
                        // Diagnosed as the specialization: no further
                        // redefinition error (clang's recovery).
                        self.diag_spec_member_not_instantiated(p, nloc);
                        respecialized = true;
                    }
                } else {
                    let msg =
                        self.diag(DiagId::sema_use_with_wrong_tag, nloc).arg(sym_str(self, n));
                    let msg = self.note_prev(msg, p, false);
                    self.emit(msg);
                }
            }
            if let (Some(p), true) = (prev, is_enum) {
                self.check_enum_redecl(p, scoped, fixed, nloc, use_);
            }
            if let Some(p) = prev {
                if member_class_spec && !self.check_member_tag_spec(p, nloc) {
                    // [temp.expl.spec]/7, diagnosed: a definition is
                    // still parsed, as an unrelated class.
                    if use_ == TagUse::Declaration {
                        return p;
                    }
                    prev = None;
                } else if use_ == TagUse::Reference {
                    // (Not marked referenced: clang `ActOnTag` marks no
                    // declaration an elaborated-type-specifier names.)
                    self.check_name_access_as(n, qualifier, p, nloc, LookupKind::Tag);
                    self.diagnose_type_name_use(p, nloc);
                    return p;
                } else if use_ == TagUse::Definition {
                    if self.ast.definition(p).is_some() {
                        // Diagnosed as the specialization already
                        // (`respecialized`): no further redefinition error.
                        if self
                            .ast
                            .definition(p)
                            .is_some_and(|d| self.module_merge_hidden_definition(d))
                        {
                            // Silent (spec/modules.md §6): the body is
                            // skipped like a redefinition's.
                            redefined = true;
                        } else if !respecialized {
                            redefined = self.diag_tag_redefinition(p, n, nloc);
                        }
                        prev = None;
                    }
                } else if use_ == TagUse::Declaration
                    && self.ast.decls.get(p).misc().friend() != FriendKind::Undeclared
                    // `template<> template<class U> struct A<int>::B;`
                    // declares the member template specialized
                    // ([temp.expl.spec]/16): a new decl (below).
                    && self.templ.member_spec.is_none()
                    // `template<> struct A<int>::B;` declares the member
                    // explicitly specialized ([temp.expl.spec]/1): a new
                    // decl (below), which an incomplete-type note names.
                    && !member_class_spec
                {
                    // `[extern] template struct C<args>::I;` explicitly
                    // instantiates a member class ([temp.explicit]/3).
                    let inst =
                        qualifier.is_some() && self.act_on_explicit_member_class_inst(p, nloc);
                    // A class template redeclaration is a new decl with a
                    // `ClassTemplateDecl` of its own (clang
                    // `CheckClassTemplate`; `finish_template_pattern`
                    // checks its head, [temp.param]/12, /14), and so is a
                    // class redeclaration `struct N; struct N;` (clang
                    // `ActOnTag`): both join `p`'s chain (below).
                    if inst || head.is_none() && is_enum {
                        self.module_merge_hidden_redecl(p);
                        return p;
                    }
                }
            } else if let Some(q) = qualifier {
                let o = match use_ {
                    TagUse::Reference => self.lookup_qualified(q, n, LookupKind::Nested).decls,
                    _ => Vec::new(),
                };
                if !self.diagnose_tag_reference_non_tag(keyword, n, &o, nloc) {
                    self.diag_qualified_tag_not_found(keyword, n, nloc, q, use_, head.is_some());
                }
            }
        }
        let ctx = qualifier.unwrap_or_else(|| self.tag_decl_context(use_));
        let dname = name.map_or(DeclName::EMPTY, |n| DeclName::ident(n.0));
        let mut h = DeclHead::new(ctx, dname, name.map_or(loc, |n| n.1));
        if redefined {
            h = h.flags(DeclFlags::INVALID);
        }
        h.lexical = Some(if qualifier.is_some() { self.ctx } else { ctx });
        if conflict {
            h.misc = h.misc.with_friend(FriendKind::Undeclared);
            h.flags = h.flags.with(DeclFlags::INVALID);
        }
        let plain_ref = use_ == TagUse::Reference && qualifier.is_none() && prev.is_none();
        let d = self.new_tag_decl(h, keyword, (scoped, fixed), prev, plain_ref);
        self.finish_member_class_spec(member_class_spec, d);
        if qualifier.is_none() {
            self.set_member_access(d);
        }
        let t = self.finish_template_pattern(d);
        if let Some(t) = t {
            if qualifier.is_none() {
                self.set_member_access(t);
            } else if self.templ.member_spec.is_some() {
                self.specialize_member_template(t);
            }
        }
        if redefined {
            // Not visible to later lookups (`diag_tag_redefinition`).
            self.builder().unregister_name(t.unwrap_or(d));
            return d;
        }
        if let Some((n, _)) = name
            && self.in_local_scope()
            && qualifier.is_none()
            && !conflict
        {
            self.push_local(n, d);
        }
        if let Some((n, _)) = name
            && qualifier.is_none()
            && prev.is_none()
            && !conflict
        {
            self.check_using_shadow_conflict(d, DeclName::ident(n));
        }
        d
    }

    /// The declaration `act_on_tag` creates: an enumeration (`scoped`,
    /// `fixed` underlying type), a class a plain elaborated reference
    /// declares in the enclosing namespace, or a class (a template
    /// pattern when a template head is pending).
    fn new_tag_decl(
        &mut self,
        h: DeclHead,
        keyword: TagKeyword,
        (scoped, fixed): (u32, Option<QualType>),
        prev: Option<DeclId>,
        plain_ref: bool,
    ) -> DeclId {
        if keyword == TagKeyword::Enum {
            let fx = fixed.map(|f| (f, self.enum_promotion(f)));
            self.builder().enumeration(h, scoped, fx, prev)
        } else if plain_ref && self.templ.cur.is_none() {
            self.namespace_tag_record(h, Self::tag_kind_of(keyword))
        } else {
            self.mark_pattern_next();
            self.builder().record(h, Self::tag_kind_of(keyword), prev)
        }
    }

    /// [basic.def.odr]/1: diagnoses defining the tag `p` again and
    /// returns true if it has a definition. The new definition is created
    /// invalid and kept out of lookup, so later uses find the first one
    /// (clang's `ActOnTag` makes it anonymous and invalid), and the parser
    /// skips its body ([`Sema::tag_body_skipped`]; spec/sema.md §4.4).
    fn diag_tag_redefinition(&mut self, p: DeclId, n: Symbol, nloc: Loc) -> bool {
        let Some(def) = self.ast.definition(p) else {
            return false;
        };
        let msg = self.diag(DiagId::sema_redefinition, nloc).arg(sym_str(self, n));
        let msg = self.note_prev(msg, def, true);
        self.emit(msg);
        true
    }

    /// True if `d`, just returned by [`Sema::act_on_tag`] for a
    /// definition, is a redefinition whose body the parser skips
    /// (spec/sema.md §4.4; clang neither checks its members nor declares
    /// its enumerators).
    pub fn tag_body_skipped(&self, d: DeclId) -> bool {
        self.ast.decls.get(d).has(DeclFlags::INVALID)
    }

    /// The elaborated-type-specifier of `friend class X;`
    /// ([dcl.type.elab]/4, [class.friend]/11): an unqualified name refers
    /// to a class found without looking beyond the innermost enclosing
    /// namespace, else declares one there, hidden from ordinary lookup.
    fn act_on_friend_tag(
        &mut self,
        keyword: TagKeyword,
        name: Option<(Symbol, Loc)>,
        qualifier: Option<DeclId>,
        scoped: u32,
        fixed: Option<QualType>,
        loc: Loc,
    ) -> DeclId {
        let ns = self.enclosing_namespace().unwrap_or(self.ast.tu);
        if let (Some((n, nloc)), Some(q)) = (name, qualifier)
            && keyword != TagKeyword::Enum
            && self.has_pending_head()
            && let Some(d) = self.act_on_qualified_friend_class_template(keyword, n, nloc, q)
        {
            return d;
        }
        let (Some((n, nloc)), None, false) = (name, qualifier, keyword == TagKeyword::Enum) else {
            return self.act_on_tag(
                keyword,
                name,
                qualifier,
                TagUse::Reference,
                scoped,
                fixed,
                loc,
            );
        };
        let within = |s: &Self, d: DeclId| {
            let mut c = s.ast.decls.get(d).ctx();
            for _ in 0..10_000 {
                match c {
                    Some(x) if s.ast.redecl_context(x) == s.ast.redecl_context(ns) => return true,
                    Some(x) if s.kind_of(x) == DeclKind::Namespace => return false,
                    Some(x) => c = s.ast.decls.get(x).ctx(),
                    None => return false,
                }
            }
            false
        };
        if self.has_pending_head() {
            return self.act_on_friend_class_template(keyword, n, nloc, ns);
        }
        // [temp.local]/6: `friend class T;` naming a template parameter
        // redeclares it (clang's `DiagnoseTemplateParameterShadow` in
        // `ActOnTag`); the friend then declares a new class `T`, so
        // `friend class Ts...;` expands no pack.
        self.check_template_param_shadow(n, nloc);
        let lookup = self.lookup_name(n, LookupKind::Tag);
        let found = lookup.decls;
        // The innermost enclosing namespace's own tag of that name, also
        // through a using-declaration there (`using iterators::core;`,
        // when class-scope lookup found something else first).
        let hidden = || {
            self.ast
                .lookup_local(ns, DeclName::ident(n))
                .iter()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|d| self.ast.using_target(d).unwrap_or(d))
                .find(|&d| is_tag_kind(self.kind_of(d)))
        };
        // [namespace.memdef]/3 limits the search by where the declaration
        // found is: a using-declaration in the innermost enclosing
        // namespace counts, and the friend is its target (boost's
        // `friend class iterator_core_access;` through
        // `using iterators::iterator_core_access;`; clang `ActOnTag`).
        let found_as = lookup.found.last().copied();
        let prev =
            found.last().copied().filter(|&p| within(self, found_as.unwrap_or(p))).or_else(hidden);
        if let Some(p) = prev {
            if self.tag_matches(p, keyword) {
                self.mark_type_referenced(p);
                return p;
            }
            let msg = self.diag(DiagId::sema_use_with_wrong_tag, nloc).arg(sym_str(self, n));
            let msg = self.note_prev(msg, p, false);
            self.emit(msg);
        }
        let mut h = DeclHead::new(ns, DeclName::ident(n), nloc);
        h.lexical = Some(self.ctx);
        h.misc = h.misc.with_friend(FriendKind::Undeclared);
        let d = self.namespace_tag_record(h, Self::tag_kind_of(keyword));
        self.friend_tag = Some(d);
        d
    }

    /// `template<class U> friend class X;` ([temp.friend]/1): `X` names
    /// a class template of the innermost enclosing namespace (its
    /// injected-class-name is not considered), else declares one there,
    /// hidden from ordinary lookup ([namespace.memdef]/3). The template is
    /// left in [`Sema::friend_class_template`] for `act_on_friend`.
    fn act_on_friend_class_template(
        &mut self,
        keyword: TagKeyword,
        n: Symbol,
        nloc: Loc,
        ns: DeclId,
    ) -> DeclId {
        let head = self.take_template_head(None);
        // [namespace.memdef]/3 stops the search at the innermost enclosing
        // namespace, so a member class template of an enclosing class
        // (or its injected-class-name) is found by ordinary lookup and
        // befriended as `friend class C::X;` would be (clang
        // `CheckClassTemplate`; libc++ `elements_view::__iterator`'s
        // `template <bool> friend class __sentinel;`).
        let member = self.lookup_name(n, LookupKind::All).decls.last().and_then(|&d| {
            let t = if self.kind_of(d) == DeclKind::ClassTemplate {
                d
            } else {
                self.injected_template_of(d)?
            };
            let c = self.ast.decls.get(t).ctx()?;
            (self.kind_of(c) == DeclKind::CXXRecord).then_some(t)
        });
        if let Some((t, p)) = member.and_then(|t| self.pattern_of(t).map(|p| (t, p))) {
            self.templ.cur = None;
            if !self.tag_matches(p, keyword) {
                let msg = self.diag(DiagId::sema_use_with_wrong_tag, nloc).arg(sym_str(self, n));
                let msg = self.note_prev(msg, p, false);
                self.emit(msg);
            }
            self.mark_type_referenced(p);
            self.friend_class_template = Some(t);
            return p;
        }
        let prev = self
            .ast
            .lookup_local(ns, DeclName::ident(n))
            .iter()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .find(|&d| is_tag_kind(self.kind_of(d)) || self.kind_of(d) == DeclKind::ClassTemplate);
        let prev = prev.map(|d| self.pattern_of(d).unwrap_or(d));
        if let Some(p) = prev {
            if self.ast.described_template(p).is_some() {
                if self.tag_matches(p, keyword) {
                    // A redeclaration of the template, lexically in the
                    // class (clang's friend `ClassTemplateDecl`); the
                    // befriended entity is its first declaration.
                    let mut h = DeclHead::new(ns, DeclName::ident(n), nloc);
                    h.lexical = Some(self.ctx);
                    h.misc = h.misc.with_friend(FriendKind::Declared);
                    self.mark_pattern_next();
                    let d = self.builder().record(h, Self::tag_kind_of(keyword), Some(p));
                    self.friend_class_template = self.finish_template_pattern(d);
                    return d;
                }
                let msg = self.diag(DiagId::sema_use_with_wrong_tag, nloc).arg(sym_str(self, n));
                let msg = self.note_prev(msg, p, false);
                self.emit(msg);
            } else if head.is_some() {
                // clang: "redefinition of 'X' as different kind of symbol".
                let msg =
                    self.diag(DiagId::sema_redefinition_different_kind, nloc).arg(sym_str(self, n));
                let msg = self.note_prev(msg, p, false);
                self.emit(msg);
                self.templ.cur = None;
                return p;
            }
        }
        let mut h = DeclHead::new(ns, DeclName::ident(n), nloc);
        h.lexical = Some(self.ctx);
        h.misc = h.misc.with_friend(FriendKind::Undeclared);
        self.mark_pattern_next();
        let d = self.builder().record(h, Self::tag_kind_of(keyword), None);
        self.friend_class_template = self.finish_template_pattern(d);
        d
    }

    /// `template<class U> friend class N::X;` ([temp.friend]/1,
    /// [dcl.type.elab]/4): a qualified name befriends the class template
    /// `N::X` that qualified lookup finds (a member template of the
    /// current instantiation too: `template<bool> friend class A<T>::B;`);
    /// no declaration is introduced. The template is left in
    /// [`Sema::friend_class_template`] for `act_on_friend`. Another kind
    /// of entity is clang's "redefinition as different kind of symbol";
    /// nothing found in a namespace is its "friend declaration does not
    /// match". `None` when nothing is found in a class (the caller
    /// reports it).
    fn act_on_qualified_friend_class_template(
        &mut self,
        keyword: TagKeyword,
        n: Symbol,
        nloc: Loc,
        q: DeclId,
    ) -> Option<DeclId> {
        let found = self.lookup_qualified(q, n, LookupKind::All).decls;
        let t = found.iter().copied().find(|&d| self.kind_of(d) == DeclKind::ClassTemplate);
        if t.is_none() && found.is_empty() && self.kind_of(q) != DeclKind::Namespace {
            return None;
        }
        // The friend's own template head declares nothing (clang matches
        // it against the template's only on instantiation).
        let _ = self.take_template_head(None);
        self.templ.cur = None;
        let Some((t, p)) = t.and_then(|t| self.pattern_of(t).map(|p| (t, p))) else {
            let msg = if let Some(&other) = found.first() {
                let msg =
                    self.diag(DiagId::sema_redefinition_different_kind, nloc).arg(sym_str(self, n));
                self.note_prev(msg, other, true)
            } else {
                self.diag(DiagId::sema_friend_def_no_match, nloc)
                    .arg(sym_str(self, n))
                    .arg(self.qualifier_desc(q))
            };
            self.emit(msg);
            // Recovery: an invalid class, hidden from lookup.
            let mut h = DeclHead::new(q, DeclName::ident(n), nloc);
            h.lexical = Some(self.ctx);
            h.misc = h.misc.with_friend(FriendKind::Undeclared);
            let d = self.builder().record(h, Self::tag_kind_of(keyword), None);
            self.ast.decls.set_flags(d, DeclFlags::INVALID);
            return Some(d);
        };
        if !self.tag_matches(p, keyword) {
            let msg = self.diag(DiagId::sema_use_with_wrong_tag, nloc).arg(sym_str(self, n));
            let msg = self.note_prev(msg, p, false);
            self.emit(msg);
        }
        self.mark_type_referenced(p);
        self.friend_class_template = Some(t);
        Some(p)
    }

    fn check_enum_redecl(
        &mut self,
        p: DeclId,
        scoped: u32,
        fixed: Option<QualType>,
        loc: Loc,
        use_: TagUse,
    ) {
        if use_ == TagUse::Reference && fixed.is_none() && scoped == 0 {
            return;
        }
        let Some(pe) = self.ast.decls.enumeration(p).copied() else { return };
        let prev_scoped = pe.flags & EnumData::SCOPED != 0;
        let msg = if prev_scoped != (scoped != 0) {
            Some(self.diag(DiagId::sema_enum_scoped_mismatch, loc).arg(if prev_scoped {
                "1"
            } else {
                "0"
            }))
        } else if pe.is_fixed() && fixed.is_none() {
            Some(self.diag(DiagId::sema_enum_fixed_redecl_unfixed, loc))
        } else if !pe.is_fixed() && fixed.is_some() {
            Some(self.diag(DiagId::sema_enum_unfixed_redecl_fixed, loc))
        } else {
            match (pe.int_type, fixed) {
                (Some(a), Some(b)) if !self.ast.types.same_type(a, b) => Some(
                    self.diag(DiagId::sema_enum_redeclare_type_mismatch, loc)
                        .arg(self.ty_arg(b))
                        .arg(self.ty_arg(a)),
                ),
                _ => None,
            }
        };
        if let Some(m) = msg {
            let m = self.note_prev(m, p, false);
            self.emit(m);
        }
    }

    /// `{` of a class definition: the class becomes the current context.
    pub fn act_on_start_record_definition(&mut self, r: DeclId) {
        // clang's `AddAlignmentAttributesForRecord`: the `#pragma pack`
        // value in effect where the definition begins.
        if self.pack.0 != 0 {
            let _ = self.ast.decls.cold.record_pack.insert(r, self.pack.0);
        }
        let _ = self.builder().begin_record_definition(r);
        let tag = self.ast.decls.record(r).map_or(TagKind::Struct, |x| x.tag);
        self.classes.push(ClassBuild::new(r, tag, self.ctx));
        self.defining.push(r);
        self.scopes.push(Scope::new(ScopeKind::Record, Some(r)));
        self.ctx = r;
    }

    /// `}` of a class definition ([class.mem]/8: complete from here).
    /// `then_semi` says the `}` is followed by `;`: an unnamed class so
    /// followed is an anonymous struct/union ([class.union.anon]), which
    /// must be known before the definition's bits freeze.
    /// A `[[no_unique_address]]` field of empty class type, a subobject
    /// of zero size ([intro.object]/9) that leaves its class empty
    /// ([meta.unary.prop] `is_empty`; clang `FieldDecl::isZeroSize`).
    fn is_zero_size_field(&self, f: DeclId) -> bool {
        let nua =
            self.ast.decls.field(f).is_some_and(FieldData::no_unique_address) || self.has_nua(f);
        nua && self
            .ast
            .decls
            .get(f)
            .ty()
            .and_then(|t| self.ast.types.as_record(t))
            .and_then(|d| self.ast.definition(d))
            .and_then(|d| self.ast.decls.record(d))
            .is_some_and(|x| x.defn.contains(DefinitionBits::EMPTY))
    }

    /// Class `r`, being defined, is invalid (clang `setInvalidDecl` on
    /// the record): an invalid field, base or member class subobject
    /// ([class.mem]; spec/sema-class.md §2.3). Uses that need it
    /// complete before its `}` are quiet, and at its `}` it is flagged
    /// [`DeclFlags::INVALID`], so its layout is clang's size and
    /// alignment 1 (spec/abi.md §1).
    pub(crate) fn invalidate_defining(&mut self, r: DeclId) {
        if self.defining.contains(&r) {
            let _ = self.invalid_defining.insert(r);
        }
    }

    /// A field type that makes its class invalid: a (possibly multi-
    /// dimensional array of) class whose definition is invalid (clang
    /// marks the field invalid; an anonymous struct or union counts).
    fn has_invalid_class_type(&self, f: DeclId) -> bool {
        let Some(t) = self.ast.decls.get(f).ty() else { return false };
        let t = self.field_complete_type(t);
        self.ast
            .types
            .as_record(t)
            .and_then(|d| self.ast.definition(d))
            .is_some_and(|d| self.ast.decls.get(d).has(DeclFlags::INVALID))
    }

    /// The definition bits read off `r`'s fields at its `}`
    /// (`HAS_MUTABLE_FIELDS`, `HAS_VARIANT_MEMBERS`, `EMPTY`), and whether
    /// a field of an invalid class type makes `r` invalid (spec/sema-class.md
    /// §2.3).
    fn record_field_bits(&mut self, r: DeclId, mut bits: DefinitionBits) -> (DefinitionBits, bool) {
        let mut invalid = false;
        let fields: Vec<DeclId> = self.ast.decls.members(r).collect();
        let mut any_named = false;
        for f in fields {
            if self.ast.decls.get(f).kind() != DeclKind::Field {
                continue;
            }
            if self.has_invalid_class_type(f) {
                self.ast.decls.set_flags(f, DeclFlags::INVALID);
                invalid = true;
            }
            let fd = self.ast.decls.field(f).copied();
            if fd.is_some_and(|x| x.flags & FieldData::MUTABLE != 0) {
                bits = bits.with(DefinitionBits::HAS_MUTABLE_FIELDS);
            }
            let anon_union = self
                .ast
                .decls
                .get(f)
                .ty()
                .and_then(|t| self.ast.types.as_record(t))
                .and_then(|d| self.ast.decls.record(d))
                .is_some_and(|x| {
                    x.tag == TagKind::Union && x.defn.contains(DefinitionBits::ANONYMOUS)
                });
            if anon_union {
                bits = bits.with(DefinitionBits::HAS_VARIANT_MEMBERS);
            }
            if (fd.is_none_or(|x| x.bit_width.is_none())
                || !self.ast.decls.get(f).name().is_empty())
                && !self.is_zero_size_field(f)
            {
                any_named = true;
            }
        }
        if !any_named {
            bits = bits.with(DefinitionBits::EMPTY);
        }
        (bits, invalid)
    }

    pub fn act_on_finish_record_definition(&mut self, r: DeclId, then_semi: bool) {
        self.scopes.pop();
        let mut build = self.classes.pop_if(|c| c.record == r);
        self.defining.retain(|&d| d != r);
        let mut invalid = self.invalid_defining.remove(&r);
        self.ctx = build.as_ref().map_or_else(
            || {
                self.ast
                    .decls
                    .lexical_ctx(r)
                    .or_else(|| self.ast.decls.get(r).ctx())
                    .unwrap_or(self.ast.tu)
            },
            |b| b.outer,
        );
        let mut bits = DefinitionBits::COMPLETE
            .with(DefinitionBits::AGGREGATE)
            .with(DefinitionBits::STANDARD_LAYOUT)
            .with(DefinitionBits::TRIVIALLY_COPYABLE)
            .with(DefinitionBits::TRIVIAL_DEFAULT_CTOR)
            .with(DefinitionBits::TRIVIAL_COPY_CTOR)
            .with(DefinitionBits::TRIVIAL_MOVE_CTOR)
            .with(DefinitionBits::TRIVIAL_COPY_ASSIGN)
            .with(DefinitionBits::TRIVIAL_MOVE_ASSIGN)
            .with(DefinitionBits::TRIVIAL_DTOR)
            .with(DefinitionBits::LITERAL)
            .with(DefinitionBits::STRUCTURAL);
        if self.ast.decls.record(r).is_some_and(|x| x.lambda.is_some()) {
            // [expr.prim.lambda.closure]/2: a closure type is not an
            // aggregate.
            bits = bits.without(DefinitionBits::AGGREGATE);
            if self
                .ast
                .decls
                .record(r)
                .is_some_and(|x| x.defn.contains(DefinitionBits::LAMBDA_CAPTURES))
            {
                // A closure with a capture has no default constructor
                // ([expr.prim.lambda.closure]/13), so it is not trivial
                // (clang `hasTrivialDefaultConstructor` needs
                // `hasDefaultConstructor`), and not POD.
                bits = bits
                    .with(DefinitionBits::LAMBDA_CAPTURES)
                    .without(DefinitionBits::TRIVIAL_DEFAULT_CTOR);
            }
        }
        let (mut bits, invalid_field) = self.record_field_bits(r, bits);
        invalid |= invalid_field;
        // §5.8: flexible array members.
        if self.check_flexible_array_members(r) {
            bits = bits.with(DefinitionBits::HAS_FLEXIBLE_ARRAY_MEMBER);
        }
        if self.ast.decls.record(r).is_some_and(|x| x.tag == TagKind::Union) {
            bits = bits.with(DefinitionBits::HAS_VARIANT_MEMBERS);
        }
        if then_semi && self.ast.decls.get(r).name() == DeclName::EMPTY {
            bits = bits.with(DefinitionBits::ANONYMOUS);
        }
        let bits = self.class_bits(r, build.as_ref(), bits);
        let bits = self.virtual_class_bits(r, bits);
        // Special members (spec/sema-class.md §4.1, §4.2, §4.4), after
        // selecting the destructor among prospective ones (sema-class.md §4.7.1).
        self.select_destructor(r);
        let bits = self.special_member_bits(r, bits);
        let bits = self.finish_special_members(r, bits);
        let bits = self.trivial_abi_bits(r, bits);
        // [class.compare.default]/4: the implicit `operator==`s.
        let mut friends =
            build.as_mut().map(|b| std::mem::take(&mut b.friends)).unwrap_or_default();
        self.declare_implicit_equality(r, &mut friends);
        if let Some(b) = &mut build {
            b.friends = friends;
        }
        if let Some(b) = &build {
            let _ = self.builder().set_friends(r, &b.friends);
        }
        self.builder().complete_record_definition(r, bits);
        if invalid {
            self.ast.decls.set_flags(r, DeclFlags::INVALID);
        }
        let own = self.own_aligned(r);
        self.check_alignas_underalignment(r, &own);
        self.hide_inherited_ctors(r);
        self.check_pending_defaulted(r);
        self.act_on_complete_virtual(r);
    }

    /// `{` of an enumeration definition.
    pub fn act_on_start_enum_definition(&mut self, e: DeclId) {
        let _ = self.builder().begin_enum_definition(e);
        let fixed = self
            .ast
            .decls
            .enumeration(e)
            .and_then(|x| if x.is_fixed() { x.int_type } else { None });
        self.enums.push(EnumBuild {
            decl: e,
            fixed,
            last: None,
            enumerators: Vec::new(),
            pre_types: Vec::new(),
        });
        self.scopes.push(Scope::new(ScopeKind::Enum, Some(e)));
        self.ctx = e;
    }

    /// The type of enumerator `d` while its enumeration is being defined
    /// ([dcl.enum]/5), if it is.
    pub(crate) fn enumerator_pre_brace_type(&self, d: DeclId) -> Option<QualType> {
        self.enums.iter().rev().find_map(|b| b.pre_types.iter().find(|x| x.0 == d).map(|x| x.1))
    }

    /// The value and the converted initializer of an enumerator whose
    /// initializer is `ie` ([dcl.enum]/5).
    fn enumerator_init(&mut self, build: &EnumBuild, ie: ExprId) -> (Option<i128>, Option<ExprId>) {
        let e = build.decl;
        let mut value: Option<i128> = None;
        let mut init_e = None;
        if self.expr_is_value_dependent(ie) {
            return (None, Some(ie));
        }
        let ity = self.ast.exprs.get(ie).ty();
        let t = &self.ast.types;
        let ok_ty = t.is_integral(ity)
            || t.as_enum(ity).is_some_and(|en| !self.is_scoped_enum(en) || en == e);
        if !ok_ty && !self.is_error_type(ity) {
            self.error(DiagId::sema_expr_not_ice, self.ast.exprs.get(ie).loc());
        } else if !self.is_error_type(ity) {
            let mut c = conv::lvalue_to_rvalue(self, ie);
            let mut narrowed = None;
            let cty = if let Some(f) = build.fixed {
                // [dcl.enum]/5: a converted constant expression, so a
                // narrowing value is ill-formed ([expr.const]/12).
                narrowed = self.fold_int(c).filter(|&raw| !self.fits(f, raw));
                c = conv::implicit_convert(self, c, f, ConvCtx::Initialization).unwrap_or(c);
                f
            } else {
                let cty = self.ast.exprs.get(c).ty();
                if self.ast.types.as_enum(cty).is_some() { QualType::INT } else { cty }
            };
            let c = self.act_on_constant_full_expr(c);
            // After the full-expression's conversion warnings, as in clang
            // (`BuildConvertedConstantExpression`).
            if let Some(raw) = narrowed {
                let msg = self
                    .diag(DiagId::sema_cce_narrowing_value, self.begin_loc(ie))
                    .arg("enumerator value")
                    .arg(raw.to_string())
                    .arg(self.ty_arg(cty));
                self.emit(msg);
            }
            // A converted constant expression for a fixed underlying
            // type ([dcl.enum]/5), else an integral constant expression.
            let at = self.begin_loc(ie);
            let ctx = if build.fixed.is_some() {
                EvalContext::converted(at)
            } else {
                EvalContext::new(EvalKind::Other, at)
            };
            let v = match self.eval_int(c, ctx) {
                Ok(v) => Some(v),
                // clang `CheckEnumConstant`: a converted constant
                // expression `enumerator value is not a constant
                // expression`; otherwise an integral constant expression
                // that may be folded (`AllowFold`).
                Err(mut f) if build.fixed.is_some() => {
                    let at = Self::fold_lone_invalid(&mut f, at);
                    let msg = self.diag(DiagId::sema_expr_not_cce, at).arg("enumerator value");
                    let msg = Self::with_failure_notes(msg, f);
                    self.emit(msg);
                    None
                }
                Err(f) => self.fold_not_ice(c, f, at),
            };
            if let Some(v) = v {
                let v = self.check_enumerator_range(v, build.fixed, ie);
                value = Some(v);
                let cv = self.int_const(cty, v);
                init_e = Some(self.builder().constant_expr(c, Some(cv)));
            }
        }
        (value, init_e)
    }

    /// An enumerator-definition ([dcl.enum]/2,5).
    pub fn act_on_enumerator(
        &mut self,
        name: Symbol,
        loc: Loc,
        init: Option<ExprId>,
        attrs: &[AttrId],
    ) -> DeclId {
        let Some(build) = self.enums.last().cloned() else {
            return self.ast.tu;
        };
        let e = build.decl;
        let scoped = self.is_scoped_enum(e);
        // Redeclaration ([basic.scope.scope]).
        let clash = if scoped {
            self.lookup_in(e, name, LookupKind::Ordinary).decls.first().copied()
        } else {
            self.lookup_current_scope_outside_enum(name)
        };
        if let Some(p) = clash {
            let id = if self.ast.decls.get(p).kind() == DeclKind::EnumConstant {
                DiagId::sema_redefinition_of_enumerator
            } else {
                DiagId::sema_redefinition
            };
            let msg = self.diag(id, loc).arg(sym_str(self, name));
            let msg = self.note_prev(msg, p, true);
            self.emit(msg);
        }
        // An enumerator of a dependent enumeration (a member of a template)
        // keeps its initializer as written and has no value until
        // instantiated (clang's `CheckEnumConstant`).
        let dependent = self.ast.types.is_dependent(self.tag_type(e));
        if dependent {
            let h = DeclHead::new(e, DeclName::ident(name), loc);
            let d = self.builder().enumerator(h, init);
            let access = self.ast.decls.get(e).misc().access();
            if access != kiln_ast::decl::Access::None {
                self.ast.decls.set_access(d, access);
            }
            if !attrs.is_empty() {
                let mut attrs = attrs.to_vec();
                self.take_annotations(d, &mut attrs);
                let _ = self.builder().attach_attrs(d, &attrs);
            }
            if !scoped && self.in_local_scope() {
                self.push_local(name, d);
            }
            return d;
        }
        let (value, init_e) = match init {
            Some(ie) => self.enumerator_init(&build, ie),
            None => (None, None),
        };
        let v = value.unwrap_or_else(|| match build.last {
            Some(l) => {
                let next = l + 1;
                if let Some(f) = build.fixed
                    && !self.fits(f, next)
                {
                    let msg = self
                        .diag(DiagId::sema_enumerator_too_large, loc)
                        .arg(next.to_string())
                        .arg(self.ty_arg(f));
                    self.emit(msg);
                }
                next
            }
            None => 0,
        });
        let h = DeclHead::new(e, DeclName::ident(name), loc);
        let d = self.builder().enumerator(h, init_e);
        // An enumerator of a member enumeration has the enumeration's
        // access (clang `ActOnEnumConstant`; [class.access]/1).
        let access = self.ast.decls.get(e).misc().access();
        if access != kiln_ast::decl::Access::None {
            self.ast.decls.set_access(d, access);
        }
        let vty = build.fixed.unwrap_or(if self.fits(QualType::INT, v) {
            QualType::INT
        } else {
            QualType::LONG
        });
        let cv = self.int_const(vty, v);
        let _ = self.ast.decls.set_enumerator_value(d, cv);
        // [dcl.enum]/5 (clang `CheckEnumConstant`): until the closing
        // brace an enumerator has the fixed underlying type, if there is
        // one (`enum class E { a = 1, b = a | 2 }` uses the built-in `|`
        // of `int`); otherwise the type of its initializing value, or
        // without an initializer that of the previous enumerator if the
        // incremented value fits, else `int` for the first one or a type
        // wide enough.
        let pre = if let Some(f) = build.fixed {
            Some(self.ast.types.unqualified(f))
        } else if let Some(ie) = init_e {
            Some(self.ast.types.unqualified(self.ast.exprs.get(ie).ty()))
        } else {
            match build.pre_types.last() {
                Some(&(_, t)) if self.fits(t, v) => Some(t),
                _ => Some(vty),
            }
        };
        if !attrs.is_empty() {
            let mut attrs = attrs.to_vec();
            self.take_annotations(d, &mut attrs);
            let _ = self.builder().attach_attrs(d, &attrs);
        }
        if !scoped && self.in_local_scope() {
            self.push_local(name, d);
        }
        if let Some(b) = self.enums.last_mut() {
            b.last = Some(v);
            b.enumerators.push((d, v));
            if let Some(t) = pre {
                b.pre_types.push((d, t));
            }
        }
        d
    }

    /// The underlying and promotion types of an enumeration without a
    /// fixed underlying type ([dcl.enum]/7, [conv.prom]/3), after clang's
    /// `ActOnEnumBody`: the narrowest of `int`, `long`, `long long` when
    /// some value is negative, else of `unsigned int`, `unsigned long`,
    /// `unsigned long long`, promoting to the signed type of the same
    /// width unless the values need every bit. A `packed` enumeration
    /// also considers `signed char` / `short` (`unsigned char` /
    /// `unsigned short`), promoting to `int` (clang
    /// `ASTContext::computeBestEnumTypes`).
    fn best_enum_types(&self, values: &[(DeclId, i128)], packed: bool) -> (QualType, QualType) {
        let mut neg_bits = 0u32;
        let mut pos_bits = 0u32;
        for &(_, v) in values {
            if v < 0 {
                // Minimum signed bits.
                neg_bits = neg_bits.max(129 - (!v).leading_zeros().min(128));
            } else {
                pos_bits = pos_bits.max((128 - v.leading_zeros()).max(1));
            }
        }
        let width = |t: QualType| constfold::int_info(&self.ast, t).map_or(64, |x| x.0);
        if packed {
            let small = if neg_bits > 0 {
                [QualType::SCHAR, QualType::SHORT]
                    .into_iter()
                    .find(|&t| neg_bits <= width(t) && pos_bits < width(t))
            } else {
                [QualType::UCHAR, QualType::USHORT].into_iter().find(|&t| pos_bits <= width(t))
            };
            if let Some(t) = small {
                return (t, QualType::INT);
            }
        }
        let signed = [QualType::INT, QualType::LONG, QualType::LONGLONG];
        let unsigned = [QualType::UINT, QualType::ULONG, QualType::ULONGLONG];
        if neg_bits > 0 {
            let t = signed
                .into_iter()
                .find(|&t| neg_bits <= width(t) && pos_bits < width(t))
                .unwrap_or(QualType::LONGLONG);
            (t, t)
        } else {
            let i = unsigned.iter().position(|&t| pos_bits <= width(t)).unwrap_or(2);
            let t = unsigned[i];
            (t, if pos_bits == width(t) { t } else { signed[i] })
        }
    }

    fn lookup_current_scope_outside_enum(&self, name: Symbol) -> Option<DeclId> {
        let s =
            self.scopes.iter().rev().find(|s| !s.is_transparent() && s.kind != ScopeKind::Enum)?;
        if s.is_local() {
            return s
                .locals
                .iter()
                .rev()
                .find(|e| {
                    e.0 == name && e.2 == Ns::Ordinary && self.kind_of(e.1) != DeclKind::UsingShadow
                })
                .map(|e| e.1);
        }
        let ent = s.entity?;
        self.ast.lookup_local(ent, DeclName::ident(name)).iter().find(|&d| {
            let k = self.ast.decls.get(d).kind();
            !is_tag_kind(k) && k != DeclKind::UsingShadow
        })
    }

    fn fits(&self, ty: QualType, v: i128) -> bool {
        constfold::int_info(&self.ast, ty)
            .is_none_or(|(bits, signed)| constfold::fits(v, bits, signed))
    }

    fn check_enumerator_range(&self, v: i128, fixed: Option<QualType>, e: ExprId) -> i128 {
        if let Some(f) = fixed
            && !self.fits(f, v)
        {
            let msg = self
                .diag(DiagId::sema_enumerator_too_large, self.ast.exprs.get(e).loc())
                .arg(v.to_string())
                .arg(self.ty_arg(f));
            self.emit(msg);
        }
        v
    }

    /// `}` of an enumeration: the underlying type of an enumeration
    /// without a fixed one ([dcl.enum]/7).
    pub fn act_on_finish_enum_definition(&mut self, e: DeclId) {
        self.scopes.pop();
        let Some(build) = self.enums.pop() else { return };
        self.ctx = self
            .ast
            .decls
            .lexical_ctx(e)
            .or_else(|| self.ast.decls.get(e).ctx())
            .unwrap_or(self.ast.tu);
        let dependent = self.ast.types.is_dependent(self.tag_type(e));
        let computed = if build.fixed.is_some() {
            None
        } else if dependent {
            // clang's `ActOnEnumBody` stops here for a dependent
            // enumeration; `int` stands in until it is instantiated.
            Some((QualType::INT, QualType::INT))
        } else {
            let packed = self.find_attr(e, AttrKind::PACKED).is_some();
            let (best, promotion) = self.best_enum_types(&build.enumerators, packed);
            // [dcl.enum]/5 (clang's `ActOnEnumBody`): each initializer is
            // converted to the underlying type.
            for &(d, _) in &build.enumerators {
                let Some(init) = self.ast.decls.enumerator(d).and_then(|x| x.init) else {
                    continue;
                };
                let c = conv::cast_to(self, CastKind::IntegralCast, init, best, None);
                if c != init {
                    self.ast.decls.replace_enumerator_init(d, c);
                }
            }
            Some((best, promotion))
        };
        let _ = self.builder().complete_enum_definition(e, computed);
    }

    // -- namespaces and linkage --------------------------------------------------

    /// `namespace N {` ([namespace.def]); `None` for an unnamed namespace.
    /// `loc` is the `namespace` keyword (where an inline mismatch is
    /// diagnosed for an unnamed namespace) and `lbrace` the `{`, an
    /// unnamed namespace's own location (clang's `NamespaceDecl` location,
    /// which notes point at).
    pub fn act_on_start_namespace(
        &mut self,
        name: Option<(Symbol, Loc)>,
        inline: bool,
        loc: Loc,
        lbrace: Loc,
    ) -> DeclId {
        let mut prev = None;
        if let Some((n, nloc)) = name {
            for p in self.lookup_current_scope(n) {
                if self.ast.decls.get(p).kind() == DeclKind::Namespace {
                    prev = Some(p);
                } else {
                    let msg = self
                        .diag(DiagId::sema_redefinition_different_kind, nloc)
                        .arg(sym_str(self, n));
                    let msg = self.note_prev(msg, p, true);
                    self.emit(msg);
                }
            }
            if prev.is_none() {
                // [namespace.def.general]/3: the name is also searched for
                // in the inline namespace set of the enclosing namespace;
                // a namespace found there is extended (its scope becomes
                // the target scope; libc++'s `namespace std::ranges {`
                // reopens `std::__1::ranges`).
                prev = self
                    .current_scope_with_inline_dn(DeclName::ident(n))
                    .into_iter()
                    .find(|&p| self.ast.decls.get(p).kind() == DeclKind::Namespace);
            }
        } else {
            // [namespace.unnamed]/1: every unnamed namespace of a scope is
            // the same namespace, across the scope's redeclarations and
            // transparent `extern "C++"` blocks.
            let key = self.ast.primary_context(self.ast.redecl_context(self.ctx));
            prev = self.anon_namespaces.get(&key).copied().or_else(|| {
                self.ast.decls.members(self.ctx).find(|&m| {
                    let n = self.ast.decls.get(m);
                    n.kind() == DeclKind::Namespace && n.name() == DeclName::EMPTY
                })
            });
        }
        let inline = match prev {
            Some(p) => self.check_namespace_inline_mismatch(p, inline, name.map_or(loc, |n| n.1)),
            None => inline,
        };
        // The semantic context is the extended namespace's (an inline
        // namespace of `self.ctx`, above); the lexical one is `self.ctx`.
        let sem = prev.and_then(|p| self.ast.decls.get(p).ctx()).unwrap_or(self.ctx);
        let mut h = DeclHead::new(
            sem,
            name.map_or(DeclName::EMPTY, |n| DeclName::ident(n.0)),
            name.map_or(lbrace, |n| n.1),
        );
        if sem != self.ctx {
            h = h.lexical(self.ctx);
        }
        let ns = self.builder().namespace(h, inline, prev);
        if name.is_none() {
            let key = self.ast.primary_context(self.ast.redecl_context(self.ctx));
            self.anon_namespaces.entry(key).or_insert(ns);
        }
        if name.is_none() && prev.is_none() {
            // ... followed by an implicit `using namespace unique;`.
            let _ = self.using_directive(ns, loc, DeclFlags::IMPLICIT);
        }
        let mut scope = Scope::new(ScopeKind::Namespace, Some(ns));
        scope.first_decl = ns.index() + 1;
        self.scopes.push(scope);
        self.ctx = ns;
        ns
    }

    /// [namespace.def.general]/3: `inline` shall appear on the original
    /// definition of an inline namespace and not on an extension of a
    /// non-inline one (clang `DiagnoseNamespaceInlineMismatch`). Omitting
    /// it on an extension of an inline namespace is allowed, but clang
    /// warns (`-Winline-namespace-reopened-noninline`); adding it to an
    /// extension of a non-inline one is an error. Both note the original
    /// definition; the extension takes the original's inlineness, which
    /// is returned.
    fn check_namespace_inline_mismatch(&mut self, prev: DeclId, inline: bool, loc: Loc) -> bool {
        let first = self.ast.first(prev);
        let orig =
            self.ast.decls.namespace(first).is_some_and(kiln_ast::decl::NamespaceData::is_inline);
        if orig != inline {
            let id = if orig {
                DiagId::sema_inline_namespace_reopened_noninline
            } else {
                DiagId::sema_inline_namespace_mismatch
            };
            let msg = self.diag(id, loc);
            let msg = self.note_prev(msg, first, true);
            self.emit(msg);
        }
        orig
    }

    /// `}` of a namespace.
    pub fn act_on_finish_namespace(&mut self, ns: DeclId) {
        self.pop_namespace_visibility(ns, Loc::INVALID);
        self.scopes.pop();
        self.ctx = self.ast.decls.lexical_ctx(ns).unwrap_or(self.ast.tu);
    }

    /// `extern "C"` / `extern "C++"` ([dcl.link]); `lang` is the string
    /// literal's contents.
    pub fn act_on_start_linkage_spec(&mut self, lang: &str, loc: Loc, lang_loc: Loc) -> DeclId {
        let l = match lang {
            "C" => LinkageLang::C,
            "C++" => LinkageLang::Cxx,
            _ => {
                let msg = self
                    .diag(DiagId::parse_unsupported, lang_loc)
                    .arg(format!("language linkage \"{lang}\""));
                self.emit(msg);
                LinkageLang::Cxx
            }
        };
        let ctx = self.ctx;
        let ls = self.builder().linkage_spec(ctx, l, loc);
        self.scopes.push(Scope::new(ScopeKind::LinkageSpec, Some(ls)));
        self.ctx = ls;
        ls
    }

    /// The linkage specification `ls` has no braces: it directly
    /// contains one declaration ([dcl.link]/8).
    pub fn act_on_linkage_spec_braceless(&mut self, ls: DeclId) {
        self.ast.decls.set_linkage_braceless(ls);
    }

    /// The end of a linkage specification.
    pub fn act_on_finish_linkage_spec(&mut self, ls: DeclId) {
        self.scopes.pop();
        self.ctx = self.ast.decls.get(ls).ctx().unwrap_or(self.ast.tu);
    }

    /// `using namespace N;` ([namespace.udir]). The nominated namespace is
    /// stored in the payload word.
    pub fn act_on_using_directive(&mut self, ns: DeclId, loc: Loc) -> DeclId {
        self.using_directive(ns, loc, DeclFlags::empty())
    }

    fn using_directive(&mut self, ns: DeclId, loc: Loc, flags: DeclFlags) -> DeclId {
        let nominated = self.ast.first(ns);
        let d = self.ast.decls.push(
            kiln_ast::decl::NewDecl {
                kind: DeclKind::UsingDirective,
                flags,
                misc: DeclMisc::default(),
                name: DeclName::EMPTY,
                ctx: Some(self.ctx),
                loc,
                ty: None,
            },
            kiln_ast::decl::Payload::None(nominated.raw()),
        );
        if self.ast.decls.member_list(self.ctx).is_some() {
            self.ast.decls.append_member(self.ctx, d);
        }
        if let Some(s) = self.scopes.iter_mut().rev().find(|s| !s.is_transparent()) {
            s.using_dirs.push(nominated);
            // A namespace-scope directive outlives the scope instance: it
            // is seen from every later reopening of the namespace, from
            // instantiations, and through qualified lookup and other
            // directives ([namespace.udir]/4, [namespace.qual]/2).
            if matches!(s.kind, ScopeKind::Namespace | ScopeKind::TranslationUnit)
                && let Some(e) = s.entity
            {
                self.ast.add_using_directive(e, nominated);
            }
        }
        d
    }

    // -- static_assert and empty declarations ------------------------------------

    /// `static_assert(cond, msg);` ([dcl.pre]/10).
    pub fn act_on_static_assert(&mut self, cond: ExprId, msg: Option<ExprId>, loc: Loc) -> DeclId {
        self.static_assert_checked(cond, msg, loc).0
    }

    /// [`Sema::act_on_static_assert`], with whether the declaration failed
    /// (clang `StaticAssertDecl::isFailed`: the condition does not convert,
    /// is not a constant expression, or is `false`).
    pub(crate) fn static_assert_checked(
        &mut self,
        cond: ExprId,
        msg: Option<ExprId>,
        loc: Loc,
    ) -> (DeclId, bool) {
        let mut failed = false;
        // [temp.variadic]/5: an unexpanded pack is ill-formed here
        // (clang's `UPPC_StaticAssertExpression`).
        let cond = if self.diagnose_unexpanded_written(cond, "static assertion", None) {
            failed = true;
            let at = self.begin_loc(cond);
            self.recover_err(&[], at)
        } else {
            cond
        };
        let c = match conv::contextually_convert_to_bool(self, cond) {
            Ok(c) => c,
            Err(err) => {
                self.emit_conv_error(err, self.ast.exprs.get(cond).loc());
                failed = true;
                cond
            }
        };
        let c = self.act_on_constant_full_expr(c);
        let cloc = self.begin_loc(cond);
        // [dcl.pre]/11-12 (P2741): a message that is not a string literal
        // is type-checked now (`size()`, `data()`), and is an unevaluated
        // operand unless the assertion fails: only then is it evaluated.
        let parts = msg.and_then(|m| self.static_assert_message_parts(m));
        // [dcl.pre]/10: a value-dependent condition waits for instantiation,
        // and so does any condition in a template definition (P2593R1,
        // a DR: `else static_assert(false);` in a discarded branch;
        // clang's `CurContext->isDependentContext()`).
        let in_definition =
            self.in_template() || self.ast.decls.get(self.ctx).has(DeclFlags::DEPENDENT_CTX);
        if !self.has_errors_in(c) && !self.expr_is_value_dependent(c) && !in_definition {
            match self.eval_const(c, EvalContext::new(EvalKind::StaticAssert, loc)) {
                Ok(v) if self.const_truth(v) != Some(false) => {
                    if let (Some(m), Some(Some((size, data)))) = (msg, parts) {
                        self.check_static_assert_message(m, size, data, loc);
                    }
                }
                Ok(_) => {
                    failed = true;
                    let text = match (msg, parts) {
                        (Some(m), None) => self.string_literal_text(m),
                        (Some(m), Some(Some((size, data)))) => {
                            self.static_assert_message_text(m, size, data, loc)
                        }
                        _ => None,
                    };
                    // clang's `findFailedBooleanCondition`: the failing
                    // conjunct, printed unless it is a literal.
                    let inner = self.failed_condition(c);
                    let lit = matches!(
                        self.ast.exprs.get(inner).kind(),
                        ExprKind::CXXBoolLiteralExpr | ExprKind::IntegerLiteral
                    );
                    let ik = self.ast.exprs.get(inner).kind();
                    if ik == ExprKind::ConceptSpecializationExpr {
                        // [temp.constr]: the unsatisfied constraint's chain
                        // (clang's `DiagnoseUnsatisfiedConstraint`).
                        let d = match text {
                            Some(t) => {
                                self.diag(DiagId::sema_static_assert_failed_msg, cloc).arg(t)
                            }
                            None => self.diag(DiagId::sema_static_assert_failed, cloc),
                        };
                        let d = d.range(self.expr_range(c));
                        let recs = self.false_atom_records(inner);
                        let mut notes = Vec::new();
                        self.render_records(&recs, &mut notes, 0);
                        let d = notes.into_iter().fold(d, kiln_diag::Diag::with_note);
                        self.emit(d);
                        let ctx = self.ctx;
                        return (self.builder().static_assert(ctx, loc, c, msg), true);
                    }
                    let d = if lit {
                        match text {
                            Some(t) => {
                                self.diag(DiagId::sema_static_assert_failed_msg, cloc).arg(t)
                            }
                            None => self.diag(DiagId::sema_static_assert_failed, cloc),
                        }
                    } else {
                        let req = self.requirement_text(inner);
                        let at = self.begin_loc(inner);
                        match text {
                            Some(t) => self
                                .diag(DiagId::sema_static_assert_requirement_failed_msg, at)
                                .arg(req)
                                .arg(t),
                            None => self
                                .diag(DiagId::sema_static_assert_requirement_failed, at)
                                .arg(req),
                        }
                    };
                    // clang `<< InnerCond->getSourceRange()` (the whole
                    // condition's for a literal).
                    let d = d.range(self.expr_range(if lit { c } else { inner }));
                    let d = self.explain_failed_comparison(d, inner);
                    let d = self.with_trait_explanation(d, inner);
                    self.emit(d);
                }
                // clang does not evaluate a condition that contains
                // errors (a variable whose initializer failed).
                Err(_) if self.refers_to_invalid_var(c) => failed = true,
                Err(mut f) => {
                    failed = true;
                    let at = Self::fold_lone_invalid(&mut f, cloc);
                    let d = self.diag(DiagId::sema_static_assert_not_ice, at);
                    let d = Self::with_failure_notes(d, f);
                    self.emit(d);
                }
            }
        }
        let ctx = self.ctx;
        (self.builder().static_assert(ctx, loc, c, msg), failed)
    }

    /// The `size()` and `data()` calls of `static_assert` message `m`
    /// that is not a string literal ([dcl.pre]/11, P2741; clang
    /// `Sema::EvaluateAsString`): `None` for a string literal or a
    /// dependent or invalid message (nothing to check), `Some(None)` if
    /// it is ill-formed (diagnosed at `m`).
    #[allow(clippy::option_option, reason = "not checked / ill-formed / the calls")]
    fn static_assert_message_parts(&mut self, m: ExprId) -> Option<Option<(ExprId, ExprId)>> {
        if self.ast.exprs.get(m).kind() == ExprKind::StringLiteral
            || self.has_errors_in(m)
            || self.expr_is_value_dependent(m)
        {
            return None;
        }
        let at = self.begin_loc(m);
        let class = self
            .ast
            .types
            .as_record(self.ety(m))
            .filter(|&r| self.kind_of(r) == DeclKind::CXXRecord)
            .map(|r| self.ast.definition(r).unwrap_or(r));
        let Some(class) = class else {
            self.error(DiagId::sema_static_assert_invalid_message, at);
            return Some(None);
        };
        let size_sym = self.intern("size");
        let data_sym = self.intern("data");
        let no_size = self.lookup_in_class(class, size_sym, LookupKind::Ordinary).decls.is_empty();
        let no_data = self.lookup_in_class(class, data_sym, LookupKind::Ordinary).decls.is_empty();
        if no_size || no_data {
            let sel = match (no_size, no_data) {
                (true, true) => "2",
                (true, false) => "0",
                _ => "1",
            };
            let d = self.diag(DiagId::sema_static_assert_missing_member_function, at).arg(sel);
            self.emit(d);
            return Some(None);
        }
        // clang's `BuildExpr`: `m.size()`, `m.data()`.
        let call = |s: &mut Self, sym: Symbol| {
            let f = s.act_on_member(m, sym, at, at, false);
            if s.is_err_expr(f) {
                return None;
            }
            let c = s.act_on_call(f, &[], at, at);
            (!s.is_err_expr(c) && !s.expr_is_value_dependent(c)).then_some(c)
        };
        let size = call(self, size_sym);
        let data = call(self, data_sym);
        let size_t = self.size_type();
        let ccp = self.const_char_ptr(QualType::CHAR);
        let Some(size) = self.static_assert_message_conv(size, size_t, "0", "std::size_t", at)
        else {
            return Some(None);
        };
        let Some(data) = self.static_assert_message_conv(data, ccp, "1", "const char *", at) else {
            return Some(None);
        };
        Some(Some((size, data)))
    }

    /// The `size()` or `data()` call `e` of a `static_assert` message
    /// converted to `to` (clang `BuildConvertedConstantExpression`);
    /// `None` after diagnosing a call that is invalid or does not convert.
    fn static_assert_message_conv(
        &mut self,
        e: Option<ExprId>,
        to: QualType,
        sel: &str,
        to_text: &str,
        at: Loc,
    ) -> Option<ExprId> {
        let converted = e.and_then(|e| {
            if let Ok(c) = conv::implicit_convert(self, e, to, ConvCtx::Initialization) {
                return Some(c);
            }
            let d = self
                .diag(DiagId::sema_cce_not_implicitly_convertible, at)
                .arg(self.ty_arg(self.ety(e)))
                .arg(self.ty_arg(to));
            self.emit(d);
            None
        });
        if converted.is_none() {
            let d = self
                .diag(DiagId::sema_static_assert_invalid_mem_fn_ret_ty, at)
                .arg(sel)
                .arg(to_text);
            self.emit(d);
        }
        converted
    }

    /// The text of the failed `static_assert`'s message `m` from its
    /// `size()` and `data()` calls, evaluated in the Oven
    /// ([dcl.pre]/12); `None` after diagnosing a message that is not a
    /// constant expression (clang 22's wording, at `m`).
    fn static_assert_message_text(
        &mut self,
        m: ExprId,
        size: ExprId,
        data: ExprId,
        loc: Loc,
    ) -> Option<String> {
        let (bytes, r) = self.eval_chars(size, data, EvalContext::new(EvalKind::StaticAssert, loc));
        let ok = r.is_ok();
        if let Err(f) = r {
            let at = self.begin_loc(m);
            let d = self.diag(DiagId::sema_static_assert_message_constexpr, at);
            let d = Self::with_failure_notes(d, f);
            self.emit(d);
        }
        // clang prints what was read before a failing read.
        (ok || !bytes.is_empty()).then(|| escape_diag_text(&bytes))
    }

    /// clang's check of the message of a `static_assert` that holds
    /// (`-Winvalid-static-assert-message`). [dcl.pre]/12 (P2741R3) makes
    /// that message an unevaluated operand, so kiln evaluates it only when
    /// the warning is enabled: it is off by default (clang 19 and 22 make
    /// it an error by default and reject a valid program; spec/sema.md
    /// §4.6, WP-570).
    fn check_static_assert_message(&mut self, m: ExprId, size: ExprId, data: ExprId, loc: Loc) {
        let at = self.begin_loc(m);
        if !self.diags.is_enabled_at(DiagId::sema_static_assert_message_not_constant, at) {
            return;
        }
        let (_, r) = self.eval_chars(size, data, EvalContext::new(EvalKind::StaticAssert, loc));
        if let Err(f) = r {
            let d = self.diag(DiagId::sema_static_assert_message_not_constant, at);
            let d = Self::with_failure_notes(d, f);
            self.emit(d);
        }
    }

    /// The conjunct of the failed condition `c` that evaluates to false
    /// (clang's `findFailedBooleanCondition`), without parentheses and
    /// implicit conversions. Literal conjuncts are skipped.
    pub(crate) fn failed_condition(&mut self, c: ExprId) -> ExprId {
        let mut terms = Vec::new();
        self.conjunction_terms(c, &mut terms);
        for &t in &terms {
            if matches!(
                self.ast.exprs.get(t).kind(),
                ExprKind::CXXBoolLiteralExpr | ExprKind::IntegerLiteral
            ) {
                continue;
            }
            if self.try_fold(t).is_some_and(|v| self.const_truth(v) == Some(false)) {
                return t;
            }
        }
        self.ignore_paren_full_imp_casts(c)
    }

    /// A failed condition as clang's `findFailedBooleanCondition`
    /// describes it (`is_ptr<int>::value`).
    pub(crate) fn requirement_text(&self, c: ExprId) -> String {
        let mut req = String::new();
        kiln_ast::types::TypeNames::write_expr(
            &kiln_ast::dump::AstNames {
                scoped_nns: self.clang_compat >= 22,
                ..kiln_ast::dump::AstNames::new(&self.ast).canonical_expr_types()
            },
            c,
            &mut req,
        );
        req
    }

    fn conjunction_terms(&self, e: ExprId, out: &mut Vec<ExprId>) {
        let x = self.ignore_paren_full_imp_casts(e);
        if self.ast.exprs.get(x).kind() == ExprKind::BinaryOperator
            && self.ast.exprs.binary_op(x) == kiln_ast::expr::BinaryOp::LAnd
            && out.len() < 1024
        {
            let [l, r] = self.ast.exprs.operands(x);
            self.conjunction_terms(l, out);
            self.conjunction_terms(r, out);
        } else {
            out.push(x);
        }
    }

    /// The contents of a narrow string literal, for messages.
    fn string_literal_text(&self, e: ExprId) -> Option<String> {
        self.string_text(e)
    }

    /// clang's "expression evaluates to 'L op R'" note for a failed
    /// `static_assert` of a comparison.
    fn explain_failed_comparison(&mut self, d: kiln_diag::Diag, c: ExprId) -> kiln_diag::Diag {
        let Some((op, l, r)) = self.comparison_parts(c) else { return d };
        if Self::is_literal_expr(&self.ast, l) && Self::is_literal_expr(&self.ast, r) {
            return d;
        }
        let (Some(lv), Some(rv)) = (self.try_fold(l), self.try_fold(r)) else { return d };
        // clang's `ConvertAPValueToString` prints no lvalue or member
        // pointer: a pointer comparison gets no note (`&a == &b`, `&&l ==
        // &&m`).
        let pointer = |v| {
            matches!(
                self.ast.consts.scalar(v),
                Some(
                    kiln_ast::consts::Scalar::Null { .. }
                        | kiln_ast::consts::Scalar::Pointer { .. }
                        | kiln_ast::consts::Scalar::MemberPtr { .. }
                )
            )
        };
        if pointer(lv) || pointer(rv) {
            return d;
        }
        let note = self
            .diag(DiagId::note_expr_evaluates_to, self.ast.exprs.get(c).loc())
            .arg(self.comparison_value_text(lv))
            .arg(op)
            .arg(self.comparison_value_text(rv))
            // clang `DiagnoseStaticAssertDetails`: the operator's range.
            .range(self.expr_range(c));
        d.with_note(note)
    }

    /// An operand's value in the "expression evaluates to" note (clang
    /// `ConvertAPValueToString`): a float as `APFloat::toString` prints
    /// it (`1`, `2.5E+10`, `+Inf`).
    fn comparison_value_text(&self, v: kiln_ast::ids::ConstValueId) -> String {
        match self.ast.consts.scalar(v) {
            Some(kiln_ast::consts::Scalar::Float { kind, bits, .. }) => {
                kiln_ast::consts::fltstr::to_string(kind, bits, 0)
            }
            _ => self.const_display(v),
        }
    }

    /// `;` or a declaration with no declarators (`struct S;`, `int;`).
    /// An unnamed class definition is an anonymous union or struct
    /// ([class.union.anon]); the implicit object it declares (a `Field` in
    /// a class, else a `Var`) is returned so that a block's `DeclStmt`
    /// can own it.
    pub fn act_on_decl_spec_only(&mut self, ds: &DeclSpec) -> Option<DeclId> {
        if let Some(t) = ds.tag {
            let n = *self.ast.decls.get(t);
            if ds.tag_declares
                && n.kind() == DeclKind::CXXRecord
                && self
                    .ast
                    .decls
                    .record(t)
                    .is_some_and(|r| r.defn.contains(DefinitionBits::ANONYMOUS))
                && !ds.invalid
            {
                // An invalid anonymous struct or union (an invalid
                // member) declares nothing: no unnamed object, no
                // injected names, and its class stays valid (clang).
                if n.has(DeclFlags::INVALID) {
                    return None;
                }
                return Some(self.act_on_anonymous_record(ds, t));
            }
            self.warn_free_standing_tag_attrs(ds, t, &ds.attrs);
            return None;
        }
        if ds.is_empty() && !ds.attrs.is_empty() && self.member_spec_class().is_none() {
            // An attribute-declaration (`[[nodiscard]];`, `alignas(8);`,
            // [dcl.pre]/10): its attributes appertain to nothing (clang
            // checks them against the enclosing namespace); in a class
            // it is a declaration without declarators.
            let ctx = self.ctx;
            self.act_on_decl_attrs(ctx, &ds.attrs);
            return None;
        }
        if !ds.invalid {
            // clang 22's declaration range starts at a leading C++11
            // attribute-specifier-seq (`[[x]] int;`, `alignas(4);`).
            let at = match ds.lead_cxx11 {
                Some(a) if self.clang_compat >= 22 => a,
                _ => ds.loc,
            };
            self.error(DiagId::sema_no_declarators, at);
        }
        None
    }

    /// An anonymous union (or, in a class, an anonymous struct)
    /// ([class.union.anon]): declares an unnamed object of the record's
    /// type and injects each member's name into the enclosing scope as an
    /// `IndirectField` (spec/sema.md §4.4).
    fn act_on_anonymous_record(&mut self, ds: &DeclSpec, rec: DeclId) -> DeclId {
        let ctx = self.ctx;
        let union = ds.keyword == TagKeyword::Union;
        let in_class = self.ast.decls.get(ctx).kind() == DeclKind::CXXRecord;
        let local = self.in_local_scope();
        let mut invalid = false;
        if !in_class && !union {
            self.error(DiagId::sema_anon_struct_not_member, ds.loc);
            invalid = true;
        } else if !in_class && !local {
            let anon_ns = self.ast.decls.get(ctx).kind() == DeclKind::Namespace
                && self.ast.decls.get(ctx).name() == DeclName::EMPTY;
            let is_static = matches!(ds.storage, Some((StorageSpec::Static, _)));
            if !is_static && !anon_ns {
                self.error(DiagId::sema_anon_union_not_static, ds.loc);
                invalid = true;
            }
        }
        // [dcl.pre]/5, [class.mem.general]/3: a member-declaration or
        // declaration without declarators declares at least one name; an
        // anonymous union or struct with no member declares none (clang
        // `BuildAnonymousStructOrUnion`, once, for the template's
        // pattern: an instantiation is not diagnosed again).
        if self.inst.stack.is_empty()
            && !self.ast.decls.members(rec).any(|m| self.kind_of(m) == DeclKind::Field)
        {
            let d = self.diag(DiagId::sema_no_declarators, ds.loc);
            let d = match self.specifiers_before_type(ds) {
                Some(end) => d.range(Span::new(ds.loc, end)),
                None => d,
            };
            self.emit(d);
        }
        let ty = self.tag_type(rec);
        let h = DeclHead::new(ctx, DeclName::EMPTY, ds.loc).flags(DeclFlags::IMPLICIT);
        let obj = if in_class {
            // The unnamed member has the access in effect
            // ([class.access.spec]/1; standard-layout compares it).
            let f = self.builder().field(h, ty, None, 0);
            self.set_member_access(f);
            f
        } else {
            let storage = match ds.storage {
                Some((StorageSpec::Static, _)) => VarStorage::Static,
                _ => VarStorage::None,
            };
            let h = if invalid { h.flags(DeclFlags::IMPLICIT.with(DeclFlags::INVALID)) } else { h };
            self.builder().var(h, ty, VarSpec::new().with_storage(storage), None)
        };
        let members: Vec<DeclId> = self.ast.decls.members(rec).collect();
        for m in members {
            let mn = *self.ast.decls.get(m);
            let Some(name) = mn.name().as_ident() else { continue };
            let chain: Vec<DeclId> = match mn.kind() {
                DeclKind::Field => vec![obj, m],
                DeclKind::IndirectField => {
                    let inner = self.ast.indirect_field_chain(m);
                    std::iter::once(obj).chain(self.ast.lists.iter::<DeclId>(inner)).collect()
                }
                _ => continue,
            };
            let prev = if in_class || !local {
                self.ast.lookup_local(ctx, DeclName::ident(name)).iter().collect()
            } else {
                self.lookup_current_scope(name)
            }
            .into_iter()
            .find(|&d| !is_tag_kind(self.kind_of(d)));
            if let Some(p) = prev {
                let msg = self
                    .diag(DiagId::sema_anon_record_member_redecl, mn.loc())
                    .arg(if union { "union" } else { "struct" })
                    .arg(sym_str(self, name))
                    .with_note(
                        self.diag(DiagId::note_previous_declaration, self.ast.decls.get(p).loc()),
                    );
                self.emit(msg);
                continue;
            }
            let fty = mn.ty().unwrap_or(QualType::ERROR);
            let ih = DeclHead::new(ctx, DeclName::ident(name), mn.loc());
            let ind = self.builder().indirect_field(ih, fty, &chain);
            self.register_local(name, ind);
        }
        obj
    }

    /// The end of the decl-specifiers written before the type-specifier
    /// of `ds` (`static const` in `static const union { };`), clang's
    /// `DeclSpec` source range: `None` when the type-specifier comes
    /// first. Read from the source text before the type-specifier.
    fn specifiers_before_type(&self, ds: &DeclSpec) -> Option<Loc> {
        let t = ds.type_loc?;
        let sm = self.diags.sm();
        let (file, pos) = sm.offset_of(t)?;
        let (_, start) = sm.offset_of(ds.loc)?;
        if pos <= start {
            return None;
        }
        let text = sm.text(file).as_bytes();
        let mut i = pos as usize;
        while i > start as usize && text.get(i - 1).is_some_and(u8::is_ascii_whitespace) {
            i -= 1;
        }
        u32::try_from(i).ok().filter(|&o| o > start).map(|o| sm.loc(file, o))
    }

    // -- name classification -----------------------------------------------------

    /// What `name` names in `X<...>::name` when the parser's lookahead
    /// skipped the template-id `X<...>` without forming the
    /// specialization: classified by the primary template's pattern (a
    /// partial or explicit specialization may differ; the parse proper
    /// looks up in the specialization). A name the pattern lacks is
    /// looked up in the explicit, then the partial specializations
    /// ([temp.expl.spec], [temp.spec.partial]): `template<> struct
    /// O<int> { template<class I> using c = I; }; O<int>::c<long> x;`.
    /// So is every name when the primary template is only declared: the
    /// template-id then names an explicit or partial specialization, and
    /// lookup in it finds the member (boost.mpl's `template<class Tag>
    /// struct at_impl;` with `template<> struct at_impl<map_tag> {
    /// template<class M, class K> struct apply ...; }`, named as
    /// `at_impl<map_tag>::apply<M, K>` inside another template).
    fn classify_template_member(&self, tmpl: DeclId, name: Symbol) -> NameClass {
        let Some(p) = self.ast.template_info(tmpl).and_then(|i| i.pattern) else {
            return NameClass::Dependent;
        };
        let in_primary = self.ast.definition(p).and_then(|def| {
            self.lookup_qualified(def, name, LookupKind::Ordinary).decls.first().copied()
        });
        let d = match in_primary {
            Some(d) => d,
            None => match self.specialization_member(tmpl, name) {
                Some(d) => d,
                None => return NameClass::Dependent,
            },
        };
        let k = self.kind_of(self.ast.strip_shadow(d));
        match k {
            DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate => NameClass::TypeTemplate(d),
            DeclKind::VarTemplate => NameClass::VarTemplate(d),
            DeclKind::FunctionTemplate => NameClass::FunctionTemplate(d),
            _ if is_type_kind(k) => NameClass::Type(QualType::DEPENDENT, d),
            _ => NameClass::Value(d),
        }
    }

    /// [`Sema::classify_name`] of `name` in `q`, a member class of a class
    /// template specialization named through a typedef
    /// (`typedef L<int>::P Q; Q::I`, `typedef L<int> D; D::P::I`) whose
    /// definition is not instantiated yet: lookahead cannot instantiate
    /// ([temp.inst]/2 happens when the nested-name-specifier is parsed),
    /// so the name is classified in the member's pattern, as a
    /// template-id qualifier is (`L<int>::P::I`). The specialization
    /// itself (`D::P`) is the WP-276 case above. `None` when `q` is not
    /// such a class.
    fn classify_uninstantiated_member(&self, q: DeclId, name: Symbol) -> Option<NameClass> {
        if self.kind_of(q) != DeclKind::CXXRecord
            || self.ast.definition(q).is_some()
            || self.ast.spec_info(q).is_some()
        {
            return None;
        }
        let p = self.ast.instantiated_from(q)?;
        let def = self.ast.definition(p)?;
        let found = self.lookup_qualified(def, name, LookupKind::Ordinary);
        let Some(&d) = found.decls.first() else { return Some(NameClass::Dependent) };
        let k = self.kind_of(self.ast.strip_shadow(d));
        Some(match k {
            DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate => NameClass::TypeTemplate(d),
            DeclKind::VarTemplate => NameClass::VarTemplate(d),
            DeclKind::FunctionTemplate => NameClass::FunctionTemplate(d),
            _ if is_type_kind(k) => NameClass::Type(QualType::DEPENDENT, d),
            _ => NameClass::Value(d),
        })
    }

    /// `A<args>::name` with `A` an alias template, for the lookahead: the
    /// alias's pattern names a class (template) specialization, whose
    /// primary pattern's member is classified as for `X<args>::name`
    /// (WP-278; libc++ `ElementsIter<R>::iterator_concept` as a template
    /// argument). `None` when the pattern names no class template.
    fn classify_alias_template_member(&self, tmpl: DeclId, name: Symbol) -> Option<NameClass> {
        let mut tmpl = tmpl;
        for _ in 0..64 {
            let p = self.ast.template_info(tmpl).and_then(|i| i.pattern)?;
            let mut ty = self.ast.decls.get(p).ty()?;
            // The pattern's type is its own typedef sugar over what it names.
            for _ in 0..64 {
                ty = match self.ast.types.view(ty.ty()) {
                    TypeView::Typedef { underlying, .. } => underlying,
                    TypeView::Elaborated { named, .. } => named,
                    _ => break,
                };
            }
            let TypeView::TemplateSpecialization { name: tn, .. } = self.ast.types.view(ty.ty())
            else {
                return None;
            };
            let t = self.ast.types.template_decl(tn)?;
            match self.kind_of(t) {
                DeclKind::ClassTemplate => return Some(self.classify_template_member(t, name)),
                DeclKind::TypeAliasTemplate => tmpl = t,
                _ => return None,
            }
        }
        None
    }

    /// For the lookahead of `X<args>::m::name` (WP-668): when member `m`
    /// of class template `X`'s pattern is a typedef or alias naming a
    /// class template specialization (`using jp = ptr<S>;`), that class
    /// template, in which `name` is then classified as for
    /// `ptr<args>::name` (clang annotates `X<args>::m::` by instantiating
    /// `X<args>`; the lookahead cannot). `None` for any other member.
    pub fn member_typedef_class_template(&self, m: DeclId) -> Option<DeclId> {
        if !matches!(self.kind_of(m), DeclKind::Typedef | DeclKind::TypeAlias) {
            return None;
        }
        let mut ty = self.ast.decls.get(m).ty()?;
        for _ in 0..64 {
            ty = match self.ast.types.view(ty.ty()) {
                TypeView::Typedef { underlying, .. } => underlying,
                TypeView::Elaborated { named, .. } => named,
                _ => break,
            };
        }
        let TypeView::TemplateSpecialization { name: tn, .. } = self.ast.types.view(ty.ty()) else {
            return None;
        };
        let t = self.ast.types.template_decl(tn)?;
        (self.kind_of(t) == DeclKind::ClassTemplate).then_some(t)
    }

    /// The first member named `name` of an explicit or partial
    /// specialization of `tmpl` (the lookahead fallback of
    /// [`Sema::classify_template_member`]).
    fn specialization_member(&self, tmpl: DeclId, name: Symbol) -> Option<DeclId> {
        use kiln_ast::templates::SpecKind;
        let first = self.ast.first(tmpl);
        let t = &self.ast.templates;
        let explicit = t.specs_of(first).iter().copied().filter(|&s| {
            self.ast.spec_info(s).is_some_and(|(_, i)| i.kind == SpecKind::ExplicitSpecialization)
        });
        let partial = t.partials_of(first).iter().filter_map(|&i| t.partial(i).map(|p| p.partial));
        explicit.chain(partial).find_map(|s| {
            let def = self.ast.definition(s)?;
            self.lookup_qualified(def, name, LookupKind::Ordinary).decls.first().copied()
        })
    }

    /// The context a qualifier `q` given to [`Sema::classify_name`] is
    /// looked up in: a typedef qualifier names its class or enumeration,
    /// as in `nested_name_from_found` ([basic.lookup.qual.general]/1).
    pub(crate) fn type_name_qualifier(&self, q: DeclId) -> DeclId {
        match self.kind_of(q) {
            // A typedef naming the current instantiation names its class.
            DeclKind::Typedef | DeclKind::TypeAlias if self.is_dependent_ctx(q) => {
                self.typedef_current_instantiation(q).unwrap_or(q)
            }
            DeclKind::Typedef | DeclKind::TypeAlias => {
                let ty = self.ast.decls.get(q).ty().unwrap_or(QualType::ERROR);
                let r = self.ast.types.as_record(ty).or_else(|| self.ast.types.as_enum(ty));
                r.map_or(q, |r| self.ast.definition(r).unwrap_or(r))
            }
            _ => q,
        }
    }

    /// What an identifier names here (for parser decisions,
    /// [basic.lookup.unqual]); `qualifier` is the context named by a
    /// preceding nested-name-specifier.
    pub fn classify_name(&self, name: Symbol, qualifier: Option<DeclId>) -> NameClass {
        let qualifier = qualifier.map(|q| self.type_name_qualifier(q));
        if let Some(q) = qualifier
            && self.kind_of(q) == DeclKind::ClassTemplate
        {
            return self.classify_template_member(q, name);
        }
        // A typedef naming a class template specialization that is not
        // instantiated yet (`typedef X<int> T; T::name<...>`): the
        // lookahead cannot instantiate, so classify by the template as
        // for `X<int>::name` (WP-276).
        if let Some(q) = qualifier
            && self.kind_of(q) == DeclKind::CXXRecord
            && self.ast.definition(q).is_none()
            && let Some((_, spec)) = self.ast.spec_info(q)
            && spec.kind != kiln_ast::templates::SpecKind::ExplicitSpecialization
        {
            return self.classify_template_member(spec.template, name);
        }
        if let Some(q) = qualifier
            && self.kind_of(q) == DeclKind::TypeAliasTemplate
            && let Some(c) = self.classify_alias_template_member(q, name)
        {
            return c;
        }
        if let Some(q) = qualifier
            && let Some(c) = self.classify_uninstantiated_member(q, name)
        {
            return c;
        }
        if let Some(q) = qualifier
            && self.is_dependent_ctx(q)
        {
            return NameClass::Dependent;
        }
        let found = match qualifier {
            Some(q) => self.lookup_qualified(q, name, LookupKind::Ordinary),
            None => self.lookup_name(name, LookupKind::Ordinary),
        };
        self.classify_found(&found)
    }

    /// The injected-class-name of a class template specialization that
    /// `name` (in `qualifier`, or unqualified) finds outside that
    /// specialization's own scope, e.g. a base class's through a derived
    /// class (`B::V` or `V` in `struct B : V<long>`): not followed by `<`
    /// it names the specialization's class type ([temp.local]/1,
    /// [class.qual]/2), while [`Sema::classify_name`] gives the class
    /// template (for `V<int>`). `None` for any other name, and for the
    /// injected-class-name of the class whose scope we are in, which
    /// [`Sema::injected_class_type`] spells.
    pub fn injected_specialization_name(
        &self,
        name: Symbol,
        qualifier: Option<DeclId>,
    ) -> Option<NameClass> {
        let found = match qualifier.map(|q| self.type_name_qualifier(q)) {
            Some(q) if matches!(self.kind_of(q), DeclKind::CXXRecord) => {
                if self.is_dependent_ctx(q) {
                    return None;
                }
                self.lookup_qualified(q, name, LookupKind::Ordinary)
            }
            Some(_) => return None,
            None => self.lookup_name(name, LookupKind::Ordinary),
        };
        if found.ambiguity.is_some() {
            return None;
        }
        let &[d] = found.decls.as_slice() else { return None };
        if self.kind_of(d) != DeclKind::CXXRecord
            || self.ast.decls.get(d).name().as_ident() != Some(name)
        {
            return None;
        }
        let (_, spec) = self.ast.spec_info(self.ast.first(d))?;
        if self
            .injected_class_decl(spec.template)
            .is_some_and(|c| self.ast.first(c) == self.ast.first(d))
        {
            return None;
        }
        Some(NameClass::Type(self.tag_type(d), d))
    }

    /// [`Sema::classify_name`] of a name followed by `::`: lookup of a
    /// name before `::` considers only namespaces, types and templates
    /// whose specializations are types ([basic.lookup.qual]/1), so a
    /// variable `fs` does not hide the namespace alias in `fs::path(..)`.
    pub fn classify_nested_name(&self, name: Symbol, qualifier: Option<DeclId>) -> NameClass {
        let class = self.classify_name(name, qualifier);
        if !matches!(class, NameClass::Value(_) | NameClass::FunctionTemplate(_)) {
            return class;
        }
        let q = qualifier.map(|q| self.type_name_qualifier(q));
        let found = match q {
            Some(q) => self.lookup_qualified(q, name, LookupKind::Nested),
            None => self.lookup_name(name, LookupKind::Nested),
        };
        if found.decls.is_empty() {
            return class;
        }
        self.classify_found(&found)
    }

    /// The [`NameClass`] of a non-empty ordinary (or nested-name) lookup
    /// result.
    pub(crate) fn classify_found(&self, found: &crate::lookup::Found) -> NameClass {
        let Some(&d) = found.decls.first() else { return NameClass::Undeclared };
        // An ambiguous lookup that found a type recovers with the
        // earliest type found ([basic.lookup.general]/1; clang's
        // `getTypeName`): the parser commits to a type-name and
        // [`Sema::diagnose_ambiguous_name`] reports the ambiguity once.
        let d = self.ambiguous_type_recovery(found).unwrap_or(d);
        let k = self.ast.decls.get(d).kind();
        if k == DeclKind::Namespace {
            return NameClass::Namespace(d);
        }
        if k == DeclKind::TemplateTemplateParm && self.templ.keyless_ttps.contains(&d) {
            // Names no template (clang 22): "no template named 'T'".
            return NameClass::Undeclared;
        }
        if k == DeclKind::NamespaceAlias {
            // [namespace.alias]/2: an alias names its namespace.
            return NameClass::Namespace(self.ast.resolve_alias(d));
        }
        // The injected-class-name of a class template's pattern names the
        // template ([temp.local]/1): `X` alone is `X<P...>`, `X<A>` a
        // template-id.
        if k == DeclKind::CXXRecord
            && let Some(t) = self.injected_template_of(d)
        {
            return NameClass::TypeTemplate(t);
        }
        if is_type_kind(k) {
            let ty = if matches!(
                k,
                DeclKind::Typedef
                    | DeclKind::TypeAlias
                    | DeclKind::TemplateTypeParm
                    | DeclKind::UnresolvedUsingTypename
            ) {
                self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR)
            } else {
                self.tag_type(d)
            };
            return NameClass::Type(ty, d);
        }
        match k {
            DeclKind::ClassTemplate
            | DeclKind::TypeAliasTemplate
            | DeclKind::TemplateTemplateParm
            | DeclKind::BuiltinTemplate => {
                return NameClass::TypeTemplate(d);
            }
            DeclKind::VarTemplate => return NameClass::VarTemplate(d),
            DeclKind::Concept => return NameClass::Concept(d),
            _ => {}
        }
        if let Some(&f) = found
            .decls
            .iter()
            .find(|&&x| self.kind_of(self.ast.strip_shadow(x)) == DeclKind::FunctionTemplate)
        {
            return NameClass::FunctionTemplate(self.ast.strip_shadow(f));
        }
        NameClass::Value(d)
    }

    /// True if `found`, the result of an ordinary lookup, is ambiguous
    /// ([basic.lookup.general]/1, [namespace.udir]/6, [class.member.lookup]/6):
    /// distinct entities that are not one overload set. A set with a
    /// dependent using-declarator is resolved at instantiation.
    pub(crate) fn is_ambiguous_lookup(&self, found: &crate::lookup::Found) -> bool {
        if found.ambiguity.is_some() {
            return true;
        }
        // Declarations of one entity, and type names of one type, are
        // not ambiguous ([basic.lookup.general]/1, [dcl.typedef]/3;
        // clang `resolveKind`'s `UniqueTypes`). An invalid declaration (a
        // redefinition) is ignored, as `resolveKind` does.
        let mut types: Vec<QualType> = Vec::new();
        let mut decls: Vec<DeclId> = Vec::new();
        for &x in &found.decls {
            let d = self.ast.strip_shadow(x);
            if self.ast.decls.get(d).has(DeclFlags::INVALID) {
                continue;
            }
            if let Some(c) = self.designated_type(d) {
                if !types.contains(&c) {
                    types.push(c);
                }
            } else if !decls.contains(&self.ast.first(d)) {
                decls.push(self.ast.first(d));
            }
        }
        types.len() + decls.len() > 1
            && !found.decls.iter().any(|&x| {
                matches!(
                    self.kind_of(self.ast.strip_shadow(x)),
                    DeclKind::UnresolvedUsingValue
                        | DeclKind::UnresolvedUsingTypename
                        | DeclKind::UnresolvedUsingIfExists
                )
            })
            && !found.decls.iter().all(|&x| {
                let k = self.kind_of(self.ast.strip_shadow(x));
                k.is_function() || k == DeclKind::FunctionTemplate
            })
    }

    /// The canonical type a type declaration `d` designates (a typedef's
    /// underlying type, a class's type), the identity of a type name for
    /// lookup ([class.member.lookup]/3, [dcl.typedef]/3); `None` for a
    /// non-type or a dependent using-declaration.
    pub(crate) fn designated_type(&self, d: DeclId) -> Option<QualType> {
        let k = self.kind_of(d);
        if !is_type_kind(k) || k == DeclKind::UnresolvedUsingTypename {
            return None;
        }
        let ty =
            if matches!(k, DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::TemplateTypeParm) {
                self.ast.decls.get(d).ty()?
            } else {
                self.tag_type(d)
            };
        Some(self.ast.types.canonical(ty))
    }

    /// The declaration an ambiguous lookup `found` recovers with when one
    /// of the candidates is a type: the earliest such declaration, as
    /// clang's `getTypeName` picks it (`None` if `found` is not ambiguous
    /// or holds no type).
    fn ambiguous_type_recovery(&self, found: &crate::lookup::Found) -> Option<DeclId> {
        if found.decls.len() < 2 || !self.is_ambiguous_lookup(found) {
            return None;
        }
        found
            .decls
            .iter()
            .copied()
            .filter(|&x| is_type_kind(self.kind_of(self.ast.strip_shadow(x))))
            .min_by_key(|x| x.index())
    }

    /// Reports an ambiguous ordinary lookup of `name` (after `qualifier`)
    /// where the parser commits to it: a type-name in a decl-specifier,
    /// or an identifier that starts a statement (clang's `ClassifyName`).
    /// Distinct entities are clang's `err_ambiguous_reference` with a
    /// "candidate found by name lookup" note each; an ambiguous class
    /// member lookup is [`Sema::diagnose_member_ambiguity`]'s. True if the
    /// lookup was ambiguous ([basic.lookup.general]/1).
    pub fn diagnose_ambiguous_name(
        &mut self,
        name: Symbol,
        qualifier: Option<DeclId>,
        loc: Loc,
    ) -> bool {
        let qualifier = qualifier.map(|q| self.type_name_qualifier(q));
        if qualifier
            .is_some_and(|q| self.kind_of(q) == DeclKind::ClassTemplate || self.is_dependent_ctx(q))
        {
            return false;
        }
        let found = match qualifier {
            Some(q) => self.lookup_qualified(q, name, LookupKind::Ordinary),
            None => self.lookup_name(name, LookupKind::Ordinary),
        };
        if !self.is_ambiguous_lookup(&found) {
            return false;
        }
        if !self.diagnose_member_ambiguity(&found, DeclName::ident(name), loc) {
            self.diagnose_ambiguous_reference(
                DeclName::ident(name),
                &found.decls,
                qualifier.is_none(),
                loc,
            );
        }
        true
    }

    /// An ambiguous lookup `decls` of `_` ([basic.scope.scope]/5, P2169)
    /// that clang words as a placeholder ambiguity: the first declaration
    /// (in source order) is name-independent.
    pub(crate) fn placeholder_ambiguity(&self, decls: &[DeclId]) -> bool {
        decls.len() > 1
            && decls
                .iter()
                .min_by_key(|&&d| self.ast.decls.get(d).loc())
                .is_some_and(|&d| self.ast.decls.get(d).misc().name_independent())
    }

    /// clang's `err_reference_to_placeholder_ambiguous` for `decls` at
    /// `loc`, with a "placeholder declared here" note each, in source
    /// order when `ascending` (member access) else latest first.
    pub(crate) fn diagnose_placeholder_ambiguity(
        &self,
        name: String,
        decls: &[DeclId],
        ascending: bool,
        loc: Loc,
    ) {
        let mut locs: Vec<Loc> = decls.iter().map(|&d| self.ast.decls.get(d).loc()).collect();
        locs.sort_unstable();
        if !ascending {
            locs.reverse();
        }
        let mut msg = self.diag(DiagId::sema_placeholder_ambiguous, loc).arg(name);
        for l in locs {
            msg = msg.with_note(self.diag(DiagId::note_placeholder_declared, l));
        }
        self.emit(msg);
    }

    /// clang's `err_ambiguous_reference` for distinct entities `decls`
    /// found by one lookup of `name` ([namespace.udir]/6), with a note
    /// per candidate in clang's order.
    pub(crate) fn diagnose_ambiguous_reference(
        &self,
        name: DeclName,
        decls: &[DeclId],
        unqualified: bool,
        loc: Loc,
    ) {
        let text = self.decl_name_str(name);
        if self.placeholder_ambiguity(decls) {
            // [basic.scope.scope]/5 (P2169), latest first (clang).
            self.diagnose_placeholder_ambiguity(text, decls, false, loc);
            return;
        }
        let mut msg = self.diag(DiagId::sema_ambiguous_reference, loc).arg(text);
        for c in self.ambiguity_order(decls, unqualified) {
            let n = self
                .diag(DiagId::note_ambiguous_candidate, self.ast.decls.get(c).loc())
                .arg(self.qualified_decl_name(c));
            msg = msg.with_note(n);
        }
        self.emit(msg);
    }

    /// The context a nested-name-specifier component `name::` names
    /// ([basic.lookup.qual]), diagnosing failure.
    pub fn act_on_nested_name(
        &mut self,
        prefix: Option<DeclId>,
        name: Symbol,
        loc: Loc,
    ) -> Option<DeclId> {
        self.act_on_nested_name_spelled(prefix, name, loc).map(|r| r.0)
    }

    /// [`Sema::act_on_nested_name`], also returning the declaration the
    /// name found (a namespace alias or typedef as written), for building
    /// the nested-name-specifier ([`Sema::nns_component`]).
    pub fn act_on_nested_name_spelled(
        &mut self,
        prefix: Option<DeclId>,
        name: Symbol,
        loc: Loc,
    ) -> Option<(DeclId, DeclId)> {
        self.act_on_nested_name_written(prefix.map(|q| (q, None)), name, loc)
    }

    /// [`Sema::act_on_nested_name_spelled`] after a prefix given with its
    /// specifier as written, if known (for the diagnostics that print it).
    pub(crate) fn act_on_nested_name_written(
        &mut self,
        prefix: Option<(DeclId, Option<kiln_ast::ids::NnsId>)>,
        name: Symbol,
        loc: Loc,
    ) -> Option<(DeclId, DeclId)> {
        let found = match prefix {
            Some((q, _)) => self.lookup_qualified(q, name, LookupKind::Nested),
            None => self.lookup_name(name, LookupKind::Nested),
        };
        self.nested_name_from_found_written(prefix, name, &found, loc)
    }

    /// [`Sema::act_on_nested_name_spelled`] once lookup of `name` (after
    /// `prefix`) found `found`.
    pub(crate) fn nested_name_from_found(
        &mut self,
        prefix: Option<DeclId>,
        name: Symbol,
        found: &crate::lookup::Found,
        loc: Loc,
    ) -> Option<(DeclId, DeclId)> {
        self.nested_name_from_found_written(prefix.map(|q| (q, None)), name, found, loc)
    }

    /// [`Sema::nested_name_from_found`] after a prefix given with its
    /// specifier as written, if known.
    fn nested_name_from_found_written(
        &mut self,
        written: Option<(DeclId, Option<kiln_ast::ids::NnsId>)>,
        name: Symbol,
        found: &crate::lookup::Found,
        loc: Loc,
    ) -> Option<(DeclId, DeclId)> {
        let prefix = written.map(|w| w.0);
        let Some(&d) = found.decls.first() else {
            // [basic.lookup.qual]/1 ignores names that are not types,
            // templates or namespaces; one that ordinary lookup finds is
            // "not a class, namespace, or enumeration" (clang's
            // `BuildCXXNestedNameSpecifier` second lookup).
            let ordinary = match prefix {
                Some(q) => self.lookup_qualified(q, name, LookupKind::Ordinary),
                None => self.lookup_name(name, LookupKind::Ordinary),
            };
            if !ordinary.decls.is_empty() {
                self.not_a_nested_name(name, &ordinary.decls, loc);
                return None;
            }
            let msg = match prefix {
                Some(q) => self
                    .diag(DiagId::sema_no_member, loc)
                    .arg(sym_str(self, name))
                    .arg(self.qualifier_desc(q)),
                None => self.diag(DiagId::sema_undeclared_var_use, loc).arg(sym_str(self, name)),
            };
            self.emit(msg);
            return None;
        };
        // A typedef naming the current instantiation (boost's
        // `typedef basic_pointerbuf<charT, BufferT> this_type;` and then
        // `typename this_type::pos_type`) nests as the class itself
        // ([temp.dep.type]/1), as `X<P...>::` does.
        let typedef_ci = if matches!(self.kind_of(d), DeclKind::Typedef | DeclKind::TypeAlias) {
            self.typedef_current_instantiation(d)
        } else {
            None
        };
        let ctx = match self.ast.decls.get(d).kind() {
            DeclKind::Namespace => Some(self.ast.first(d)),
            DeclKind::NamespaceAlias => Some(self.ast.resolve_alias(d)),
            DeclKind::Typedef | DeclKind::TypeAlias if typedef_ci.is_some() => {
                self.mark_type_referenced(d);
                typedef_ci
            }
            // A class named only as a qualifier is not marked referenced
            // (clang's `isReferenced`).
            DeclKind::CXXRecord | DeclKind::Enum => {
                // [expr.prim.id.qual]/1, [temp.inst]/3: a member class of
                // a class template specialization named before `::` is
                // instantiated (`O<int>::M::N`).
                if self.ast.definition(d).is_none() && self.ast.instantiated_from(d).is_some() {
                    let poi = self.nns_poi(loc);
                    let _ = self.instantiate_class(d, poi, true);
                }
                Some(self.ast.definition(d).unwrap_or(d))
            }
            // A dependent `using typename B<T>::x;` member nests like a
            // template parameter (`x::value`, WP-179).
            DeclKind::TemplateTypeParm
            | DeclKind::TemplateTemplateParm
            | DeclKind::UnresolvedUsingTypename => Some(d),
            DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate => {
                let what = if self.kind_of(d) == DeclKind::ClassTemplate {
                    "class template"
                } else {
                    "alias template"
                };
                // Printed with the specifier as written (`'::N::Y'`; the
                // prefix's qualified name when that is not known), with
                // the "template is declared here" note ([temp.names]/1;
                // clang's `diagnoseMissingTemplateArguments`).
                let q = match written {
                    Some((_, Some(n))) => self.nns_str(n),
                    Some((q, None)) if self.kind_of(q) != DeclKind::TranslationUnit => {
                        format!("{}::", self.qualified_decl_name(q))
                    }
                    _ => String::new(),
                };
                let note = self.diag(DiagId::note_template_decl_here, self.conflict_loc(d));
                let msg = self
                    .diag(DiagId::sema_template_requires_args, loc)
                    .arg(what)
                    .arg(format!("'{q}{}'", sym_str(self, name)))
                    .with_note(note);
                self.emit(msg);
                None
            }
            DeclKind::Typedef | DeclKind::TypeAlias if self.is_dependent_ctx(d) => Some(d),
            DeclKind::Typedef | DeclKind::TypeAlias => {
                self.mark_type_referenced(d);
                let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
                // [temp.inst]/2: a specialization named through an alias
                // is instantiated like one named by its template-id.
                if (self.undefined_spec(ty).is_some() || self.undefined_member_enum(ty).is_some())
                    && !self.require_complete_nns(ty, loc)
                {
                    return None;
                }
                let r = self.ast.types.as_record(ty).or_else(|| self.ast.types.as_enum(ty));
                if r.is_none() {
                    let msg = self
                        .diag(DiagId::sema_expected_class_or_namespace, loc)
                        .arg(self.ty_arg(ty));
                    self.emit(msg);
                }
                r.map(|r| self.ast.definition(r).unwrap_or(r))
            }
            _ => {
                self.not_a_nested_name(name, &found.decls, loc);
                None
            }
        };
        // [class.access]/4: a class member named before `::` is
        // access-checked like any other name.
        if ctx.is_some() {
            self.check_found_access(found, loc);
        }
        ctx.map(|c| (c, d))
    }

    /// The class of the current instantiation that typedef `d` names
    /// (`X<P...>` for the class template `X` being defined), if any.
    pub(crate) fn typedef_current_instantiation(&self, d: DeclId) -> Option<DeclId> {
        let ty = self.ast.decls.get(d).ty()?;
        if !self.is_dependent_type(ty) {
            return None;
        }
        let canon = self.ast.types.canonical(ty);
        let TypeView::TemplateSpecialization { name, .. } = self.ast.types.view(canon.ty()) else {
            return None;
        };
        let tmpl = self.ast.types.template_decl(name)?;
        self.current_instantiation(tmpl, canon)
    }

    /// `'x' is not a class, namespace, or enumeration` for a
    /// nested-name-specifier component `x::` that names something else,
    /// with clang's `declared here` note when lookup found one declaration
    /// that is not a function template (clang's `LookupResult` treats a
    /// function template, like several functions, as an overload set).
    fn not_a_nested_name(&mut self, name: Symbol, decls: &[DeclId], loc: Loc) {
        let mut msg = self
            .diag(DiagId::sema_expected_class_or_namespace, loc)
            .arg(format!("'{}'", sym_str(self, name)));
        if let [d] = decls
            && self.kind_of(*d) != DeclKind::FunctionTemplate
        {
            msg = msg.with_note(
                self.diag(DiagId::note_entity_declared_at, self.ast.decls.get(*d).loc())
                    .arg(format!("'{}'", sym_str(self, name))),
            );
        }
        self.emit(msg);
    }

    /// Class `q` printed as its injected-class-name type when it is a
    /// class template's pattern or a partial specialization (see
    /// [`Sema::qualifier_desc`]); `None` for any other class.
    fn current_instantiation_desc(&self, q: DeclId) -> Option<String> {
        let first = self.ast.first(q);
        // clang 22 prints the injected-class-name fully scoped (`'N::V<T>'`,
        // `'H::O<U>'`), a partial specialization with its arguments as
        // written (`'N::V<T *>'`).
        let c22 = self.clang_compat >= 22;
        let name = |s: &Self| {
            if c22 {
                let mut n = String::new();
                kiln_ast::types::TypeNames::write_decl_name(
                    &crate::names::SemaNames::new(s),
                    q,
                    true,
                    &mut n,
                );
                n
            } else {
                s.decl_name(q)
            }
        };
        if let Some((_, info)) = self.ast.partial_spec_info(first) {
            let mut s = name(self);
            let names = crate::names::SemaNames::new(self);
            let args = if c22 { info.args } else { self.ast.types.canonical_args(info.args) };
            kiln_ast::types::write_arg_list(&self.ast.types, &names, args, &mut s);
            return Some(s);
        }
        let t = self.ast.described_template(first)?;
        let params: Vec<String> = self
            .template_params(t)
            .iter()
            .map(|&p| {
                let mut n = self.decl_name(p);
                if self.ast.template_parm(p).is_some_and(|x| x.pack) {
                    n.push_str("...");
                }
                n
            })
            .collect();
        Some(format!("{}<{}>", name(self), params.join(", ")))
    }

    /// How a qualifier context is named in "no member named 'x' in ...".
    pub fn qualifier_desc(&self, q: DeclId) -> String {
        match self.ast.decls.get(q).kind() {
            DeclKind::Namespace => format!("namespace '{}'", self.qualified_decl_name(q)),
            DeclKind::TranslationUnit => "the global namespace".to_owned(),
            // A class template's pattern or a partial specialization is
            // printed as its injected-class-name type (clang's
            // `getTypeDeclType`, an `InjectedClassNameType`): unqualified,
            // the pattern with its parameters (`'V<T>'`, a pack `'P<T...>'`),
            // a partial specialization with its canonical arguments
            // (`'V<type-parameter-0-0 *>'`) ([temp.dep.type]/1).
            DeclKind::CXXRecord if self.current_instantiation_desc(q).is_some() => {
                format!("'{}'", self.current_instantiation_desc(q).unwrap_or_default())
            }
            // An explicit specialization prints its arguments as written.
            DeclKind::CXXRecord
                if let Some(s) = crate::names::SemaNames::new(self).explicit_spec_context(q) =>
            {
                format!("'{s}'")
            }
            // clang prints the context's qualified name (the canonical
            // type: `'N::B'`, not `'B' (aka 'N::B')`).
            _ => {
                let t = self.tag_type(q);
                format!("'{}'", self.ty_canonical_str(t))
            }
        }
    }

    // -- attributes ----------------------------------------------------------------

    /// One attribute ([dcl.attr]); `scope` is `gnu` for `gnu::x`, and
    /// `GNU` syntax is `__attribute__((x))`. Returns `None` for ignored
    /// attributes. Encoding of arguments: spec/abi.md §9 Q1, spec/sema.md
    /// §4.5.
    pub fn act_on_attribute(
        &mut self,
        syntax: AttrSyntax,
        scope: Option<Symbol>,
        name: Symbol,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let raw = sym_str(self, name);
        if scope.is_some() || raw.starts_with("__") {
            self.attr_spellings.insert(loc, (scope, name));
        }
        let a = self.attribute_node(syntax, scope, name, args, loc)?;
        if scope.is_some_and(|s| norm_attr_scope(&sym_str(self, s)) == "gnu") {
            self.ast.attrs.set_flags(a, AttrNode::GNU_SCOPED);
        }
        Some(a)
    }

    /// [`Sema::act_on_attribute`] without the scope flag.
    #[allow(clippy::too_many_lines)]
    fn attribute_node(
        &mut self,
        syntax: AttrSyntax,
        scope: Option<Symbol>,
        name: Symbol,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let raw = sym_str(self, name);
        let scope_s = scope.map(|s| norm_attr_scope(&sym_str(self, s)).to_owned());
        let n = raw.trim_start_matches("__").trim_end_matches("__").to_owned();
        let shown = self.attr_shown_parts(syntax, scope, name);
        let gnu = syntax == AttrSyntax::Gnu || scope_s.as_deref() == Some("gnu");
        let std = scope_s.is_none() && syntax != AttrSyntax::Gnu;
        let clang_scope = scope_s.as_deref() == Some("clang");
        // The thread-safety family: known, ignored, no diagnostic (WP-606).
        if crate::decl_attrs::is_thread_safety_attr(&n, syntax, clang_scope, self.clang_compat) {
            return None;
        }
        // Sanitizer and lifetime-analysis attributes: likewise (WP-667).
        if crate::decl_attrs::is_ignored_clang_attr(
            &n,
            syntax,
            scope_s.as_deref(),
            self.clang_compat,
        ) {
            return None;
        }
        // clang 19 names only the attribute, not its scope.
        if scope_s.as_deref().is_some_and(|s| s != "gnu")
            && !(clang_scope
                && matches!(
                    n.as_str(),
                    "using_if_exists"
                        | "fallthrough"
                        | "exclude_from_explicit_instantiation"
                        | "flag_enum"
                        | "enum_extensibility"
                        | "preferred_name"
                        | "coro_only_destroy_when_complete"
                        | "optnone"
                        | "minsize"
                        | "disable_tail_calls"
                        | "uninitialized"
                        | "trivial_abi"
                        | "type_visibility"
                        | "min_vector_width"
                ))
        {
            let msg = self.diag(DiagId::parse_unknown_attribute, loc).arg(shown);
            self.emit(msg);
            return None;
        }
        // [dcl.attr.indet] is C++26 (clang trunk's `LangOpts<"CPlusPlus26">`
        // spelling): below it, `indeterminate` is an unknown attribute.
        let Some(kind) = attr_kind_of(&n, std, gnu, clang_scope, syntax)
            .filter(|&k| k != AttrKind::INDETERMINATE || self.std >= kiln_lex::Std::Cxx26)
        else {
            let id = if gnu && n == "format" {
                DiagId::parse_attribute_ignored
            } else {
                DiagId::parse_unknown_attribute
            };
            let msg = self.diag(id, loc).arg(shown);
            self.emit(msg);
            return None;
        };
        if let std::ops::ControlFlow::Break(r) =
            self.attribute_with_args(kind, syntax, &raw, args, loc)
        {
            return r;
        }
        if kind == AttrKind::ABI_TAG {
            return self.abi_tag_attribute(syntax, &n, args, loc);
        }
        if kind == AttrKind::DIAGNOSE_IF {
            return self.diagnose_if_attribute(syntax, args, loc);
        }
        if kind == AttrKind::TARGET {
            return self.target_attribute(syntax, &n, args, loc);
        }
        if matches!(kind, AttrKind::WEAKREF | AttrKind::ALIAS | AttrKind::SECTION) {
            return self.weakref_alias_attribute(kind, syntax, &n, args, loc);
        }
        if kind == AttrKind::MIN_VECTOR_WIDTH {
            return self.min_vector_width_attribute(syntax, args, loc);
        }
        if kind == AttrKind::ENABLE_IF {
            return self.enable_if_attribute(syntax, args, loc);
        }
        if kind == AttrKind::ENUM_EXTENSIBILITY {
            return self.enum_extensibility_attribute(syntax, args, loc);
        }
        if kind == AttrKind::INIT_PRIORITY {
            return self.init_priority_attribute(syntax, &shown, args, loc);
        }
        if kind == AttrKind::VISIBILITY || kind == AttrKind::TYPE_VISIBILITY {
            return self.visibility_attribute(kind, syntax, &shown, args, loc);
        }
        // `preferred_name(typedef-name)`: the type in `b`, checked
        // against its class template in `check_decl_attrs`.
        if kind == AttrKind::PREFERRED_NAME {
            let [AttrArg::Type(t)] = args else {
                let msg = self
                    .diag(DiagId::parse_attribute_wrong_arg_count, loc)
                    .arg("preferred_name")
                    .arg("1");
                self.emit(msg);
                return None;
            };
            let node = AttrNode { kind, syntax, flags: 0, loc, a: 0, b: t.to_raw() };
            return Some(self.ast.attrs.push(node));
        }
        let takes = match kind {
            AttrKind::NODISCARD | AttrKind::DEPRECATED | AttrKind::ASSUME => 1,
            AttrKind::ALIGNED => {
                usize::from(syntax != AttrSyntax::Keyword)
                    + usize::from(syntax == AttrSyntax::Keyword)
            }
            _ => 0,
        };
        // [dcl.attr.assume]/1: exactly one argument (clang's
        // `ActOnCXXAssumeAttr`).
        if args.len() > takes || (kind == AttrKind::ASSUME && args.is_empty()) {
            // clang 22 names a `[[scope::x]]` attribute with its scope.
            let name = if syntax == AttrSyntax::Cxx11 && scope.is_some() && self.clang_compat >= 22
            {
                shown.clone()
            } else {
                n.clone()
            };
            let msg = self
                .diag(DiagId::parse_attribute_wrong_arg_count, loc)
                .arg(name)
                .arg(if takes == 0 { "0" } else { "1" });
            self.emit(msg);
            return None;
        }
        let (mut a, mut b) = (0u32, 0u32);
        match (kind, args.first()) {
            (AttrKind::ALIGNED, Some(AttrArg::Expr(e))) => {
                let e = self.check_alignment_expr(*e, loc)?;
                a = e.raw();
            }
            (AttrKind::ALIGNED, Some(AttrArg::Type(t))) => b = t.to_raw(),
            // The assumption as written; converted by
            // `act_on_attributed_stmt` once its statement is known.
            (AttrKind::ASSUME, Some(AttrArg::Expr(e))) => a = e.raw(),
            (AttrKind::NODISCARD | AttrKind::DEPRECATED, Some(AttrArg::Expr(e))) => {
                if self.string_lit_of(*e).is_none() {
                    let msg = self
                        .diag(DiagId::parse_attribute_requires_string, self.ast.exprs.get(*e).loc())
                        .arg(n);
                    self.emit(msg);
                    return None;
                }
                a = e.raw();
            }
            (_, Some(AttrArg::Type(_) | AttrArg::Ident(..))) => {
                self.error(DiagId::parse_expected_expression, loc);
                return None;
            }
            _ => {}
        }
        if kind == AttrKind::ALIGNED && args.is_empty() && syntax == AttrSyntax::Keyword {
            self.error(DiagId::parse_expected_expression, loc);
            return None;
        }
        let flags = if kind == AttrKind::FALLTHROUGH && scope.is_some() {
            AttrNode::FALLTHROUGH_SCOPED
        } else {
            0
        };
        let node = AttrNode { kind, syntax, flags, loc, a, b };
        Some(self.ast.attrs.push(node))
    }

    /// `enum_extensibility(open|closed)` (clang
    /// `handleEnumExtensibilityAttr`): exactly one argument, an identifier
    /// naming a supported kind; `a` is 1 for `open`.
    fn enum_extensibility_attribute(
        &mut self,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let name = "'enum_extensibility'";
        // An argument already diagnosed (`(open, closed)`: undeclared
        // `closed`) drops the attribute silently.
        if args.iter().any(|a| {
            matches!(a, AttrArg::Expr(e) if self.ast.exprs.get(*e).deps().contains(kiln_ast::expr::ExprDeps::ERROR))
        }) {
            return None;
        }
        let id = match args {
            [AttrArg::Ident(id, _)] => *id,
            [AttrArg::Expr(_) | AttrArg::Type(_)] => {
                let msg = self.diag(DiagId::sema_attribute_param_not_ident, loc).arg(name).arg("0");
                self.emit(msg);
                return None;
            }
            _ => {
                let msg = self
                    .diag(DiagId::parse_attribute_wrong_arg_count, loc)
                    .arg("enum_extensibility")
                    .arg("1");
                self.emit(msg);
                return None;
            }
        };
        let a = match sym_str(self, id).as_str() {
            "open" => 1,
            "closed" => 0,
            other => {
                let msg = self
                    .diag(DiagId::sema_attribute_arg_not_supported, loc)
                    .arg(name)
                    .arg(format!("'{other}'"));
                self.emit(msg);
                return None;
            }
        };
        let node = AttrNode { kind: AttrKind::ENUM_EXTENSIBILITY, syntax, flags: 0, loc, a, b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// GNU `target("features")` (clang `handleTargetAttr`): exactly one
    /// string literal, kept as its `StringLitId` in `a`. The feature
    /// names are not checked (clang's `warn_unsupported_target_attribute`
    /// is a follow-up); codegen reads them on `AArch64`.
    fn target_attribute(
        &mut self,
        syntax: AttrSyntax,
        n: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let [arg] = args else {
            let msg =
                self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg(n.to_owned()).arg("1");
            self.emit(msg);
            return None;
        };
        let lit = match arg {
            AttrArg::Expr(e) => self.string_lit_of(*e),
            AttrArg::Type(_) | AttrArg::Ident(..) => None,
        };
        let Some(lit) = lit else {
            let msg = self.diag(DiagId::parse_attribute_requires_string, loc).arg(n.to_owned());
            self.emit(msg);
            return None;
        };
        let a = self.ast.exprs.string_lit(lit).raw();
        let node = AttrNode { kind: AttrKind::TARGET, syntax, flags: 0, loc, a, b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// GNU `weakref` / `weakref("target")`, `alias("target")` and
    /// `section("name")` (clang `handleWeakRefAttr`, `handleAliasAttr`,
    /// `handleSectionAttr`): `weakref` takes at most one argument, the
    /// others exactly one, a string literal (`expected
    /// string literal` at the argument otherwise; an identifier is not
    /// looked up). `a` is the literal's `StringLitId` (0 for a bare
    /// `weakref`); the declaration's checks do the rest
    /// ([`Sema::check_weak_attrs`]).
    fn weakref_alias_attribute(
        &mut self,
        kind: AttrKind,
        syntax: AttrSyntax,
        n: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let weakref = kind == AttrKind::WEAKREF;
        if weakref && args.len() > 1 {
            let msg =
                self.diag(DiagId::sema_attribute_too_many_arguments_one, loc).arg("'weakref'");
            self.emit(msg);
            return None;
        }
        if !weakref && args.len() != 1 {
            let msg =
                self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg(n.to_owned()).arg("1");
            self.emit(msg);
            return None;
        }
        let a = match args.first() {
            None => 0,
            Some(arg) => {
                let (lit, at) = match arg {
                    AttrArg::Expr(e) => (self.string_lit_of(*e), self.begin_loc(*e)),
                    AttrArg::Ident(_, at) => (None, *at),
                    AttrArg::Type(_) => (None, loc),
                };
                let Some(lit) = lit else {
                    let msg =
                        self.diag(DiagId::parse_attribute_requires_string, at).arg(n.to_owned());
                    self.emit(msg);
                    return None;
                };
                self.ast.exprs.string_lit(lit).raw()
            }
        };
        let node = AttrNode { kind, syntax, flags: 0, loc, a, b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// clang `min_vector_width(N)` (`handleMinVectorWidthAttr`): exactly
    /// one integer constant, kept in `a` (saturated at `u32::MAX`).
    fn min_vector_width_attribute(
        &mut self,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let [arg] = args else {
            let msg = self
                .diag(DiagId::parse_attribute_wrong_arg_count, loc)
                .arg("min_vector_width")
                .arg("1");
            self.emit(msg);
            return None;
        };
        let v = match arg {
            AttrArg::Expr(e) => self.fold_int(*e),
            AttrArg::Type(_) | AttrArg::Ident(..) => None,
        };
        let Some(v) = v else {
            let msg = self
                .diag(DiagId::sema_attribute_requires_integer_constant, loc)
                .arg("'min_vector_width'");
            self.emit(msg);
            return None;
        };
        let a = u32::try_from(v.max(0)).unwrap_or(u32::MAX);
        let node = AttrNode { kind: AttrKind::MIN_VECTOR_WIDTH, syntax, flags: 0, loc, a, b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// GNU `abi_tag("t", ...)`: string literals, kept sorted and unique
    /// (clang `handleAbiTagAttr`) as one interned symbol in `a`, the tags
    /// separated by `,` (spec/abi.md §8.15). A tag-less `abi_tag` (only
    /// meaningful on an inline namespace) is dropped.
    fn abi_tag_attribute(
        &mut self,
        syntax: AttrSyntax,
        n: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let mut tags: Vec<String> = Vec::new();
        for arg in args {
            let lit = match arg {
                AttrArg::Expr(e) => self.string_lit_of(*e),
                AttrArg::Type(_) | AttrArg::Ident(..) => None,
            };
            let Some(lit) = lit else {
                let msg = self.diag(DiagId::parse_attribute_requires_string, loc).arg(n.to_owned());
                self.emit(msg);
                return None;
            };
            let id = self.ast.exprs.string_lit(lit);
            tags.push(String::from_utf8_lossy(self.ast.strings.bytes(id)).into_owned());
        }
        if tags.is_empty() {
            return None;
        }
        tags.sort();
        tags.dedup();
        let a = self.intern(&tags.join(",")).index();
        let node = AttrNode { kind: AttrKind::ABI_TAG, syntax, flags: 0, loc, a, b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// The operand `arg` of `alignas`, written at `begin` and followed by
    /// `...` at `ellipsis` ([dcl.align]/1, [temp.variadic]/5): a pack
    /// expansion, expanded into one alignment per element when the
    /// attribute is instantiated; without `...` the operand may not name
    /// a pack ([temp.variadic]/12, clang's "expression contains
    /// unexpanded parameter pack", a type-id being an `alignof` operand).
    /// `None` drops the attribute.
    pub fn act_on_alignas_operand(
        &mut self,
        arg: AttrArg,
        ellipsis: Option<Loc>,
        begin: Loc,
    ) -> Option<AttrArg> {
        match (arg, ellipsis) {
            (AttrArg::Type(t), Some(l)) => Some(AttrArg::Type(self.act_pack_expansion_type(t, l))),
            (AttrArg::Expr(e), Some(l)) => Some(AttrArg::Expr(self.act_pack_expansion(e, l))),
            (AttrArg::Type(t), None) => {
                use crate::templ::pack::PackOperand;
                (!self.diagnose_unexpanded_pack(PackOperand::Type(t), "expression", begin))
                    .then_some(arg)
            }
            (AttrArg::Expr(e), None) => {
                use crate::templ::pack::PackOperand;
                (!self.diagnose_unexpanded_pack(PackOperand::Expr(e), "expression", begin))
                    .then_some(arg)
            }
            (AttrArg::Ident(..), _) => Some(arg),
        }
    }

    /// An alignment argument ([dcl.align]/2): an integral constant
    /// expression whose value is a power of two (0 is ignored).
    pub(crate) fn check_alignment_expr(&mut self, e: ExprId, at: Loc) -> Option<ExprId> {
        if self.expr_is_value_dependent(e) {
            return Some(e);
        }
        let loc = self.ast.exprs.get(e).loc();
        let c = conv::lvalue_to_rvalue(self, e);
        let c = self.act_on_constant_full_expr(c);
        let r = self.eval_int(c, EvalContext::new(EvalKind::Other, self.begin_loc(e)));
        if let Ok(v) = r {
            // Reported at the attribute (clang's `AddAlignedAttr`).
            if v != 0 && (v < 0 || (v & (v - 1)) != 0) {
                self.error(DiagId::sema_alignment_not_power_of_two, at);
                return None;
            }
            if v > 1 << 32 {
                let msg =
                    self.diag(DiagId::sema_alignment_too_big, at).arg((1u64 << 32).to_string());
                self.emit(msg);
                return None;
            }
            let cv = self.int_const(self.ast.exprs.get(c).ty(), v);
            Some(self.builder().constant_expr(c, Some(cv)))
        } else if let Err(mut f) = r {
            let at = Self::fold_lone_invalid(&mut f, loc);
            let d = self.diag(DiagId::sema_expr_not_ice, at);
            let d = Self::with_failure_notes(d, f);
            self.emit(d);
            None
        } else {
            None
        }
    }

    /// The attributes of a class-head or enum-head (`struct alignas(8)
    /// [[nodiscard]] S`, [class.pre]/1, [dcl.enum]/1).
    /// `use_` is the head's use: a declaration or definition merges its
    /// alignment attributes with the previous declaration's
    /// ([dcl.align]/6, [`Sema::merge_aligned_redecl`]).
    pub fn act_on_tag_attrs(&mut self, d: DeclId, attrs: &[AttrId], use_: TagUse) {
        let mut attrs = attrs.to_vec();
        if !attrs.is_empty() {
            self.check_decl_attrs(d, &mut attrs);
        }
        if matches!(use_, TagUse::Declaration | TagUse::Definition) {
            self.finish_visibility_attrs(d, &mut attrs, true);
        }
        if self.ast.decls.get(d).kind() == DeclKind::Enum {
            // `alignas` on an enumeration is rejected ([dcl.align]/1).
        } else if use_ == TagUse::Declaration
            && (self.ast.decls.record(d).is_some_and(|r| r.defn.contains(DefinitionBits::COMPLETE))
                || !self.ast.lists.is_empty(self.ast.attrs.of_decl(d)))
        {
            // `struct S;` names the existing declaration.
            self.merge_aligned_folded_redecl(d, &mut attrs);
        } else if matches!(use_, TagUse::Declaration | TagUse::Definition) {
            self.merge_aligned_redecl(d, &mut attrs, use_ == TagUse::Definition);
        }
        if !attrs.is_empty() {
            let _ = self.builder().attach_attrs(d, &attrs);
        }
    }

    /// The GNU attributes after the closing brace of class `r`'s
    /// definition (`struct S { ... } __attribute__((packed));`): clang
    /// `ParseCXXMemberSpecification` parses them there and
    /// `ActOnFinishCXXMemberSpecification` applies them to the class
    /// itself, before it is complete, so `packed` / `aligned` shape its
    /// layout as when written after the class-key. Likewise an
    /// enumeration's (clang `ParseEnumBody` -> `ActOnEnumBody`, ahead of
    /// its underlying type).
    pub fn act_on_record_trailing_attrs(&mut self, r: DeclId, attrs: &[AttrId]) {
        let mut attrs = attrs.to_vec();
        self.check_decl_attrs(r, &mut attrs);
        self.builder().append_attrs(r, &attrs);
    }

    /// clang `handlePreferredName`: `preferred_name` applies to a class
    /// template (else it is ignored with a warning) and names a typedef
    /// for one of its specializations (else an error); kept when valid.
    /// Type names print it ([`kiln_ast::types::TypeNames::preferred_name`]).
    pub(crate) fn check_preferred_name(&mut self, d: DeclId, node: AttrNode) -> bool {
        let tmpl = (self.ast.decls.get(d).kind() == DeclKind::CXXRecord)
            .then(|| self.ast.described_template(d))
            .flatten();
        let Some(tmpl) = tmpl else {
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                .arg("'preferred_name'")
                .arg("class templates");
            self.emit(msg);
            return false;
        };
        let Some(t) = QualType::from_raw(node.b) else { return false };
        if self.is_error_type(t) {
            return false;
        }
        let ts = &self.ast.types;
        let mut named = t;
        while let TypeView::Elaborated { named: n, .. } = ts.view(named.ty()) {
            named = n;
        }
        let typedef = match ts.view(named.ty()) {
            TypeView::Typedef { decl, .. } if t.quals().is_empty() => Some(decl),
            _ => None,
        };
        let of_tmpl = ts
            .as_record(t)
            .and_then(|r| self.ast.spec_info(r))
            .is_some_and(|(_, i)| self.ast.first(i.template) == self.ast.first(tmpl));
        if typedef.is_some() && of_tmpl {
            return true;
        }
        let mut msg = self
            .diag(DiagId::sema_attribute_preferred_name_arg_invalid, node.loc)
            .arg(self.ty_arg(t))
            .arg(format!("'{}'", self.decl_name(tmpl)));
        if let Some(td) = typedef {
            let td_note = self
                .diag(DiagId::note_entity_declared_at, self.ast.decls.get(td).loc())
                .arg(format!("'{}'", self.decl_name(td)));
            msg = msg.with_note(td_note);
        }
        self.emit(msg);
        false
    }

    /// [dcl.align]/1: an `alignas` specifier applies to a variable that
    /// is not a function parameter, a `catch` variable or declared
    /// `register`, to a non-bit-field data member, or to a class; clang
    /// rejects it (an error, and the specifier is dropped) elsewhere: on
    /// any other kind of declaration "only applies to variables, data
    /// members and tag types" (`diagAppertainsToDecl` for the keyword),
    /// on the excluded variables, bit-fields and enumerations "cannot be
    /// applied to ..." (`validateAlignasAppliedType`).
    pub(crate) fn check_alignas_subject(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        let kind = self.ast.decls.get(d).kind();
        let which = match kind {
            DeclKind::Var
            | DeclKind::VarTemplateSpecialization
            | DeclKind::VarTemplatePartialSpecialization => self.ast.decls.var(d).and_then(|v| {
                if v.spec.is_exception_var() {
                    Some(2u8)
                } else if v.spec.storage() == VarStorage::Register {
                    Some(1)
                } else {
                    None
                }
            }),
            DeclKind::ParmVar => Some(0),
            DeclKind::Field => {
                self.ast.decls.field(d).is_some_and(|f| f.bit_width.is_some()).then_some(3)
            }
            DeclKind::Enum => Some(4),
            DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization => None,
            _ => Some(u8::MAX),
        };
        if let Some(which) = which {
            self.reject_alignas(attrs, which);
        }
    }

    /// Drops (and reports) the `alignas` specifiers in `attrs`: `which`
    /// selects the "cannot be applied to" subject of
    /// `sema_alignas_wrong_decl_type`, `u8::MAX` the "only applies to"
    /// error.
    fn reject_alignas(&mut self, attrs: &mut Vec<AttrId>, which: u8) {
        attrs.retain(|&a| {
            let node = *self.ast.attrs.get(a);
            if node.kind != AttrKind::ALIGNED || node.syntax != AttrSyntax::Keyword {
                return true;
            }
            let msg = if which == u8::MAX {
                self.diag(DiagId::sema_attribute_wrong_decl_type_error, node.loc)
                    .arg("'alignas'")
                    .arg("variables, data members and tag types")
            } else {
                self.diag(DiagId::sema_alignas_wrong_decl_type, node.loc)
                    .arg("'alignas'")
                    .arg(which)
            };
            self.emit(msg);
            false
        });
    }

    /// GNU `malloc` / `alloc_size` / `alloc_align` on function `d`: the
    /// return type must be a pointer (`alloc_align`: or a reference), and
    /// the indices (one-based; index 1 of an implicit object member
    /// function is `this`) must name integer parameters (clang
    /// `handleRestrictAttr` / `handleAllocSizeAttr` / `AddAllocAlignAttr`).
    pub(crate) fn check_alloc_attr(&mut self, d: DeclId, node: AttrNode) {
        let Some(proto) = self.ast.decls.get(d).ty().and_then(|t| self.ast.types.proto(t)) else {
            return;
        };
        let name = match node.kind {
            AttrKind::MALLOC => "'malloc'",
            AttrKind::ALLOC_SIZE => "'alloc_size'",
            _ => "'alloc_align'",
        };
        let t = &self.ast.types;
        let ret = proto.ret();
        let refs = node.kind == AttrKind::ALLOC_ALIGN;
        if !(t.is_pointer(ret) || t.is_dependent(ret) || (refs && t.is_reference(ret))) {
            let id = if refs {
                DiagId::sema_attribute_pointers_refs_only
            } else {
                DiagId::sema_attribute_pointers_only
            };
            let msg = self.diag(id, node.loc).arg(name);
            self.emit(msg);
            return;
        }
        if node.kind == AttrKind::MALLOC {
            return;
        }
        let params = self.ast.types.params(proto).to_vec();
        let this = u32::from(self.is_implicit_object_member(d));
        for (i, raw) in [node.a, node.b].into_iter().enumerate() {
            let Some(e) = ExprId::from_raw(raw) else { continue };
            let n = self.ast.exprs.int_value(e).unwrap_or(0);
            let pos = (i + 1).to_string();
            if this == 1 && n == 1 {
                let msg = self.diag(DiagId::sema_attribute_implicit_this_arg, node.loc).arg(name);
                self.emit(msg);
                return;
            }
            let k = n.checked_sub(1 + u64::from(this)).and_then(|k| usize::try_from(k).ok());
            let Some(&p) = k.and_then(|k| params.get(k)) else {
                let msg = self
                    .diag(DiagId::sema_attribute_param_out_of_bounds, node.loc)
                    .arg(name)
                    .arg(pos);
                self.emit(msg);
                return;
            };
            // clang `checkParamIsIntegerType`: a dependent parameter is
            // not an integer either. alloc_align also takes
            // `std::align_val_t` (`AddAllocAlignAttr`, `Type::isAlignValT`;
            // libstdc++ 16 `operator new` overloads); alloc_size does not.
            if !(self.ast.types.is_integral(p) || refs && self.is_align_val_t(p)) {
                let at = self.ast.exprs.get(e).loc();
                let msg = self.diag(DiagId::sema_attribute_param_integers_only, at).arg(name);
                self.emit(msg);
                return;
            }
        }
    }

    /// clang `Type::isAlignValT`: `q` is (sugar for) an enumeration
    /// named `align_val_t` in namespace `std`.
    fn is_align_val_t(&self, q: QualType) -> bool {
        self.ast.types.as_enum(q).is_some_and(|d| {
            self.ast.decls.get(d).name().as_ident() == Some(self.intern("align_val_t"))
                && self.is_in_std_namespace(d)
        })
    }

    /// GNU `format(archetype, F, A)` on function `d` (WP-186; clang
    /// `handleFormatAttr`): `F` names a parameter (one-based, index 1 of
    /// an implicit object member function being `this`, which cannot be
    /// the format string) of pointer-to-character type; `A` is 0 or the
    /// position of the `...` (a non-variadic function gets
    /// `-Wgcc-compat`'s warning and `A` must then be one past the last
    /// parameter); `strftime` requires `A` = 0. False drops the
    /// attribute (after an error).
    pub(crate) fn check_format_decl_attr(
        &mut self,
        d: DeclId,
        kind: DeclKind,
        node: AttrNode,
    ) -> bool {
        if !kind.is_function() {
            let shown = format!("'{}'", self.attr_shown(&node, "format"));
            let what = "Objective-C methods, blocks, and non-K&R-style functions";
            let msg = self.diag(DiagId::sema_attribute_wrong_decl_type, node.loc).arg(shown);
            self.emit(msg.arg(what.to_owned()));
            return false;
        }
        if node.flags & 1 != 0 {
            return true;
        }
        let Some(proto) = self.ast.decls.get(d).ty().and_then(|t| self.ast.types.proto(t)) else {
            return true;
        };
        let name = "'format'";
        let this = u32::from(self.is_implicit_object_member(d));
        let params = self.ast.types.params(proto).to_vec();
        let mut num = u32::try_from(params.len()).unwrap_or(u32::MAX - 2) + this;
        let (f, first) = (node.b & 0xffff, node.b >> 16);
        let oob = |s: &mut Self, pos: &str| {
            let msg =
                s.diag(DiagId::sema_attribute_param_out_of_bounds, node.loc).arg(name).arg(pos);
            s.emit(msg);
            false
        };
        if f < 1 || f > num {
            return oob(self, "2");
        }
        if this == 1 && f == 1 {
            let msg = self.diag(DiagId::sema_format_attribute_implicit_this, node.loc);
            self.emit(msg);
            return false;
        }
        let pty = params[(f - 1 - this) as usize];
        let t = &self.ast.types;
        let is_char = |q: QualType| {
            matches!(
                t.builtin(t.canonical(q).without_quals()),
                Some(BuiltinType::Char | BuiltinType::SChar | BuiltinType::UChar)
            )
        };
        let char_ptr = t.is_pointer(pty) && t.pointee(pty).is_some_and(is_char);
        if !t.is_dependent(pty) && !char_ptr {
            let msg = self.diag(DiagId::sema_format_attribute_not_string, node.loc);
            self.emit(msg);
            return false;
        }
        if first != 0 {
            if proto.variadic() {
                num += 1;
            } else {
                let at = self.ast.decls.get(d).loc();
                let msg = self.diag(DiagId::sema_gcc_requires_variadic, at).arg(name);
                self.emit(msg);
            }
        }
        if node.a == crate::vector::FORMAT_STRFTIME {
            if first != 0 {
                let msg = self.diag(DiagId::sema_format_strftime_third_param, node.loc);
                self.emit(msg);
                return false;
            }
        } else if first != 0 && first != num {
            return oob(self, "3");
        }
        true
    }

    /// `#pragma pack` and other unsupported pragmas.
    pub fn act_on_unsupported_pragma(&self, what: &str, loc: Loc) {
        self.unsupported(&format!("'#pragma {what}'"), loc);
    }

    /// `#pragma pack(args)` at `loc` ([cpp.pragma]; clang's
    /// `ActOnPragmaPack`); `args` is the argument list's tokens spelled
    /// without spaces. The forms clang accepts silently are applied:
    /// `()` resets, `(N)` sets, `(push[, id][, N])` saves the current
    /// value then sets `N`, `(pop[, id])` restores the value saved by the
    /// innermost push (labelled `id`, popping everything above it; no
    /// effect if no entry has that label). `N` is 1, 2, 4, 8 or 16; 0
    /// means no packing. Forms clang diagnoses (`show`, an invalid `N`,
    /// pop on an empty stack, an unknown action, pop with both a label
    /// and `N`) are reported as not supported (spec/sema-class.md §6).
    pub fn act_on_pragma_pack(&mut self, loc: Loc, args: String) {
        if !self.apply_pragma_pack(&args) {
            self.act_on_unsupported_pragma("pack", loc);
        }
        self.pragma_packs.push((loc, args));
    }

    /// Applies `#pragma pack(args)` to [`Sema::pack`]; false (and no
    /// change) for a form clang diagnoses.
    fn apply_pragma_pack(&mut self, args: &str) -> bool {
        let value = |s: &str| -> Option<u16> {
            let n: u16 = s.parse().ok()?;
            (n == 0 || (n.is_power_of_two() && n <= 16)).then_some(n)
        };
        let is_ident = |s: &str| {
            s.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        };
        let parts: Vec<&str> = if args.is_empty() { Vec::new() } else { args.split(',').collect() };
        match parts.as_slice() {
            [] => self.pack.0 = 0,
            [n] if value(n).is_some() => self.pack.0 = value(n).unwrap_or(0),
            ["push", rest @ ..] if rest.len() <= 2 => {
                let (label, n) = match rest {
                    [] => (None, None),
                    [x] if is_ident(x) => (Some((*x).to_owned()), None),
                    [x] => (None, Some(value(x))),
                    [l, x] if is_ident(l) => (Some((*l).to_owned()), Some(value(x))),
                    _ => return false,
                };
                if n == Some(None) {
                    return false;
                }
                let current = self.pack.0;
                self.pack.1.push((label, current));
                if let Some(Some(n)) = n {
                    self.pack.0 = n;
                }
            }
            ["pop"] => match self.pack.1.pop() {
                Some((_, v)) => self.pack.0 = v,
                None => return false,
            },
            ["pop", l] if is_ident(l) => {
                if self.pack.1.is_empty() {
                    return false;
                }
                if let Some(i) = self.pack.1.iter().rposition(|(x, _)| x.as_deref() == Some(*l)) {
                    self.pack.0 = self.pack.1[i].1;
                    self.pack.1.truncate(i);
                }
            }
            ["pop", n] => {
                let Some(n) = value(n) else { return false };
                let Some((_, _)) = self.pack.1.pop() else { return false };
                self.pack.0 = n;
            }
            _ => return false,
        }
        true
    }
}

/// Whether operator `op` can be unary, can be binary, and must be a
/// member ([over.oper], clang `OperatorKinds.def`).
pub(crate) const fn operator_uses(op: OverloadedOperator) -> (bool, bool, bool) {
    use OverloadedOperator as O;
    match op {
        O::Plus
        | O::Minus
        | O::Star
        | O::Amp
        | O::PlusPlus
        | O::MinusMinus
        | O::New
        | O::Delete
        | O::ArrayNew
        | O::ArrayDelete => (true, true, false),
        O::Tilde | O::Exclaim | O::Coawait => (true, false, false),
        O::Equal | O::Subscript | O::Call => (false, true, true),
        O::Arrow => (true, false, true),
        _ => (false, true, false),
    }
}

impl Sema<'_, '_> {
    /// Class `c` as clang prints a class declaration context in a
    /// diagnostic (`ak_declcontext`: the class's type): the
    /// injected-class-name `A<T>` of a pattern, else the class with its
    /// scope (`N::A<long>::C`).
    /// clang 22 follows `err_member_decl_does_not_match` naming a class
    /// with `note_defined_here` at the class (spec/sema.md §1.1); clang 19
    /// gives no note.
    pub(crate) fn with_class_defined_note(
        &self,
        d: kiln_diag::Diag,
        ctx: DeclId,
    ) -> kiln_diag::Diag {
        if self.clang_compat < 22
            || !matches!(self.kind_of(ctx).payload(), kiln_ast::decl::PayloadClass::Record)
        {
            return d;
        }
        let decl = self.ast.decls.get(ctx);
        let note = self
            .diag(DiagId::note_entity_defined_here, decl.loc())
            .arg(self.decl_name_str(decl.name()));
        d.with_note(note)
    }

    pub(crate) fn class_context_str(&mut self, c: DeclId) -> String {
        let t = self.this_class_type(c);
        let injected = t != self.tag_type(c);
        let names = crate::names::SemaNames::new(self);
        let opts = kiln_ast::types::PrintOptions {
            // clang 22 scopes the injected class name too (`'N::A<T>'`).
            full_scope: !injected || self.clang_compat >= 22,
            ..kiln_ast::types::PrintOptions::SUGARED
        };
        format!("'{}'", kiln_ast::types::print_type(&self.ast.types, &names, t, opts))
    }
}

/// The attribute `n` (its `__` affixes removed) names, given whether it
/// was written as a standard (`std`), GNU (`gnu`) or `clang::` attribute;
/// `None` for an unknown or ignored one.
/// An attribute scope as clang normalizes it (`normalizeAttrScopeName`):
/// the reserved spellings `__gnu__` and `_Clang` name `gnu` and `clang`
/// (`[[__gnu__::__always_inline__]]` is `[[gnu::always_inline]]`).
pub(crate) fn norm_attr_scope(s: &str) -> &str {
    match s {
        "__gnu__" => "gnu",
        "_Clang" => "clang",
        _ => s,
    }
}

fn attr_kind_of(
    n: &str,
    std: bool,
    gnu: bool,
    clang_scope: bool,
    syntax: AttrSyntax,
) -> Option<AttrKind> {
    let gnu_syntax = syntax == AttrSyntax::Gnu;
    Some(match (n, std, gnu) {
        ("nodiscard", true, _) | ("warn_unused_result", _, true) => AttrKind::NODISCARD,
        ("maybe_unused", true, _) => AttrKind::MAYBE_UNUSED,
        ("unused", _, true) => AttrKind::UNUSED,
        ("deprecated", _, _) => AttrKind::DEPRECATED,
        ("noreturn", _, _) => AttrKind::NORETURN,
        ("fallthrough", _, _) => AttrKind::FALLTHROUGH,
        ("assume", true, _) => AttrKind::ASSUME,
        ("likely", true, _) => AttrKind::LIKELY,
        ("unlikely", true, _) => AttrKind::UNLIKELY,
        ("no_unique_address", true, _) => AttrKind::NO_UNIQUE_ADDRESS,
        ("carries_dependency", true, _) => AttrKind::CARRIES_DEPENDENCY,
        ("indeterminate", true, _) => AttrKind::INDETERMINATE,
        ("aligned", _, true) | ("alignas", _, _) => AttrKind::ALIGNED,
        ("packed", _, true) => AttrKind::PACKED,
        ("used", _, true) => AttrKind::USED,
        ("noinline", _, true) => AttrKind::NOINLINE,
        ("always_inline", _, true) => AttrKind::ALWAYS_INLINE,
        ("gnu_inline", _, true) => AttrKind::GNU_INLINE,
        ("weak", _, true) => AttrKind::WEAK,
        // clang `GCC<"weakref">`, `GCC<"alias">`.
        ("weakref", _, true) => AttrKind::WEAKREF,
        ("alias", _, true) => AttrKind::ALIAS,
        // clang `GCC<"section">`.
        ("section", _, true) => AttrKind::SECTION,
        // clang `GNU<"analyzer_noreturn">`: no `[[...]]` spelling.
        ("analyzer_noreturn", _, _) if gnu_syntax => AttrKind::ANALYZER_NORETURN,
        ("const", _, true) => AttrKind::CONST,
        ("pure", _, true) => AttrKind::PURE,
        ("cold", _, true) => AttrKind::COLD,
        ("hot", _, true) => AttrKind::HOT,
        ("naked", _, true) => AttrKind::NAKED,
        ("may_alias", _, true) => AttrKind::MAY_ALIAS,
        ("init_priority", _, true) => AttrKind::INIT_PRIORITY,
        ("visibility", _, true) => AttrKind::VISIBILITY,
        // clang `Clang<"type_visibility">`.
        ("type_visibility", _, _) if gnu_syntax || clang_scope => AttrKind::TYPE_VISIBILITY,
        ("nonnull", _, true) => AttrKind::NONNULL,
        ("returns_nonnull", _, true) => AttrKind::RETURNS_NONNULL,
        // clang `Clang<"optnone">` / `Clang<"minsize">` spellings:
        // `__attribute__((x))` and `[[clang::x]]`, not `[[gnu::x]]`.
        ("optnone", _, _) if gnu_syntax || clang_scope => AttrKind::OPTNONE,
        ("minsize", _, _) if gnu_syntax || clang_scope => AttrKind::MINSIZE,
        // clang `Clang<"disable_tail_calls">` (WP-682).
        ("disable_tail_calls", _, _) if gnu_syntax || clang_scope => AttrKind::DISABLE_TAIL_CALLS,
        ("uninitialized", _, _) if gnu_syntax || clang_scope => AttrKind::UNINITIALIZED,
        // clang `Clang<"trivial_abi">`.
        ("trivial_abi", _, _) if gnu_syntax || clang_scope => AttrKind::TRIVIAL_ABI,
        ("flag_enum", _, gnu) if gnu || clang_scope => AttrKind::FLAG_ENUM,
        ("enum_extensibility", _, gnu) if gnu || clang_scope => AttrKind::ENUM_EXTENSIBILITY,
        ("preferred_name", _, gnu) if gnu || clang_scope => AttrKind::PREFERRED_NAME,
        ("coro_only_destroy_when_complete", _, gnu) if gnu || clang_scope => {
            AttrKind::CORO_ONLY_DESTROY_WHEN_COMPLETE
        }
        ("malloc", _, true) => AttrKind::MALLOC,
        ("alloc_size", _, true) => AttrKind::ALLOC_SIZE,
        ("alloc_align", _, true) => AttrKind::ALLOC_ALIGN,
        ("ext_vector_type", _, true) => AttrKind::EXT_VECTOR_TYPE,
        ("vector_size", _, true) => AttrKind::VECTOR_SIZE,
        // clang `Clang<"neon_vector_type">` / `Clang<"neon_polyvector_type">`.
        ("neon_vector_type", _, _) if gnu_syntax || clang_scope => AttrKind::NEON_VECTOR_TYPE,
        ("neon_polyvector_type", _, _) if gnu_syntax || clang_scope => {
            AttrKind::NEON_POLYVECTOR_TYPE
        }
        ("using_if_exists", _, true) => AttrKind::USING_IF_EXISTS,
        ("abi_tag", _, true) => AttrKind::ABI_TAG,
        // clang `GNU<"enable_if">`: no `[[...]]` spelling.
        ("enable_if", _, _) if gnu_syntax => AttrKind::ENABLE_IF,
        // clang `GNU<"diagnose_if">`: likewise.
        ("diagnose_if", _, _) if gnu_syntax => AttrKind::DIAGNOSE_IF,
        ("using_if_exists", _, _) if clang_scope => AttrKind::USING_IF_EXISTS,
        ("exclude_from_explicit_instantiation", _, gnu) if gnu || clang_scope => {
            AttrKind::EXCLUDE_FROM_EXPLICIT_INSTANTIATION
        }
        ("format", _, true) => AttrKind::FORMAT,
        ("nothrow", _, true) => AttrKind::NOTHROW,
        // clang `GCC<"target">`.
        ("target", _, true) => AttrKind::TARGET,
        // WP-630: clang `GCC<"nodebug">` and `Clang<"min_vector_width">`.
        ("nodebug", _, true) => AttrKind::NODEBUG,
        ("min_vector_width", _, _) if gnu_syntax || clang_scope => AttrKind::MIN_VECTOR_WIDTH,
        _ => return None,
    })
}

impl Sema<'_, '_> {
    /// Whether `ty` is `__fp16` and the target passes no half values in
    /// parameters or return values (clang `allowHalfArgsAndReturns`:
    /// true on `AArch64`, false on x86-64).
    pub(crate) fn is_fp16_arg_disallowed(&self, ty: QualType) -> bool {
        self.abi.triple.arch == kiln_abi::target::Arch::X86_64
            && self.ast.types.canonical(ty).ty() == QualType::HALF.ty()
    }
}

/// `bytes` as clang's diagnostic engine prints a string argument
/// (`pushEscapedString`): valid UTF-8 as is, except that an unprintable
/// code point other than a tab or newline prints as `<U+XXXX>`; a byte
/// that is not valid UTF-8 prints as `<XX>`.
fn escape_diag_text(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len());
    for chunk in bytes.utf8_chunks() {
        for c in chunk.valid().chars() {
            if c.is_control() && c != '\t' && c != '\n' {
                let _ = write!(out, "<U+{:04X}>", u32::from(c));
            } else {
                out.push(c);
            }
        }
        for b in chunk.invalid() {
            let _ = write!(out, "<{b:02X}>");
        }
    }
    out
}

/// The deduction form of a variable's initializer `init`: its items (a
/// braced list's elements), the form, and a braced list's `{`.
fn auto_init_items(
    init: &Initializer,
    direct: bool,
) -> (Vec<Option<ExprId>>, crate::templ::auto::AutoInit, Option<Loc>) {
    use crate::templ::auto::{AutoInit, auto_list_items};
    match init {
        Initializer::Expr(e) => (vec![Some(*e)], AutoInit::Expr, None),
        Initializer::Paren(es, ..) => {
            (es.iter().copied().map(Some).collect(), AutoInit::Paren, None)
        }
        Initializer::List(l) if direct => {
            (auto_list_items(l, false), AutoInit::DirectList, Some(l.lbrace))
        }
        Initializer::List(l) => (auto_list_items(l, true), AutoInit::CopyList, Some(l.lbrace)),
    }
}

/// The first of the cv-qualifiers and ref-qualifier written on function
/// declarator `d` (clang points its qualifier errors there).
pub(crate) fn first_fn_qual_loc(d: &Declarator) -> Option<Loc> {
    d.fn_qual_locs.iter().flatten().min().copied()
}
